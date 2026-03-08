use std::collections::HashMap;
use std::time::Instant;

use raylib::prelude::*;
use raylib::ffi::{
    BeginBlendMode, EndBlendMode,
    rlGetTextureIdDefault, rlBegin, rlEnd, rlColor4ub, rlTexCoord2f, rlVertex2f, rlSetTexture,
    RL_QUADS,
    Mesh, Material,
    UploadMesh, UnloadMesh, DrawMesh, UpdateMeshBuffer,
    LoadMaterialDefault,
};

use crate::grids::Grids;
use crate::types::{BufferKey, ShaderKey, Blend, GlyphsetKey};
use crate::types::Rotation;
use crate::hierarchy::RenderItem;
use crate::resource_pack::ResourceProvider;

/// Кэшированный батч отрисовки
struct Batch {
    texture_id: u32,
    blend: Blend,
    mesh: Mesh,
    
    // CPU buffers for rebuilding (capacity reuse)
    vertices: Vec<f32>,   // x, y, z
    texcoords: Vec<f32>,  // u, v
    colors: Vec<u8>,      // r, g, b, a
    
    // Track GPU buffer capacity to decide between Update and Re-upload
    gpu_capacity: usize, // in vertices
}

impl Batch {
    fn new() -> Self {
        Self {
            texture_id: 0,
            blend: Blend::Alpha,
            mesh: unsafe { std::mem::zeroed() },
            vertices: Vec::new(),
            texcoords: Vec::new(),
            colors: Vec::new(),
            gpu_capacity: 0,
        }
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        // Ensure we don't double-free CPU memory, as we own the Vecs
        self.mesh.vertices = std::ptr::null_mut();
        self.mesh.texcoords = std::ptr::null_mut();
        self.mesh.colors = std::ptr::null_mut();
        unsafe { UnloadMesh(self.mesh); }
    }
}

/// Данные буферного шейдера (RenderTexture + настройки)
pub struct BufferShaderData {
    pub shader: ShaderKey,
    pub padding: u32,
    pub render_texture: RenderTexture2D,
}

/// Система рендеринга
pub struct Renderer {
    // Mask shader (internal)
    mask_shader: Option<Shader>,
    loc_mask_tex: i32,
    loc_mask_src_rect: i32,
    loc_mask_tex_size: i32,
    loc_glyph_src_rect: i32,
    loc_glyph_tex_size: i32,
    loc_use_mask: i32,
    loc_bg_color: i32,

    // Post-process
    post_process_texture: Option<RenderTexture2D>,

    // Buffer shaders
    buffer_shaders: HashMap<BufferKey, BufferShaderData>,
    
    // Geometry Cache
    buffer_batches: HashMap<BufferKey, Vec<Batch>>,
    
    // Shared resources
    default_material: Option<Material>,

    // Time tracking
    start_time: Instant,
    current_time: f32,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            mask_shader: None,
            loc_mask_tex: -1,
            loc_mask_src_rect: -1,
            loc_mask_tex_size: -1,
            loc_glyph_src_rect: -1,
            loc_glyph_tex_size: -1,
            loc_use_mask: -1,
            loc_bg_color: -1,
            post_process_texture: None,
            buffer_shaders: HashMap::new(),
            buffer_batches: HashMap::new(),
            default_material: None,
            start_time: Instant::now(),
            current_time: 0.0,
        }
    }

    /// Загрузить шейдер маски (internal)
    pub fn load_mask_shader(
        &mut self,
        provider: &mut dyn ResourceProvider,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        shader_path: &str,
    ) -> Result<(), String> {
        let code = provider.read_string(shader_path).map_err(|e| e.to_string())?;
        let shader = rl.load_shader_from_memory(thread, None, Some(&code));
        
        self.loc_mask_tex = shader.get_shader_location("texture1");
        self.loc_mask_src_rect = shader.get_shader_location("maskSrcRect");
        self.loc_mask_tex_size = shader.get_shader_location("maskTexSize");
        self.loc_glyph_src_rect = shader.get_shader_location("glyphSrcRect");
        self.loc_glyph_tex_size = shader.get_shader_location("glyphTexSize");
        self.loc_use_mask = shader.get_shader_location("useMask");
        self.loc_bg_color = shader.get_shader_location("bgColor");
        
        self.mask_shader = Some(shader);
        Ok(())
    }

    /// Установить шейдер для буфера с padding
    pub fn attach_shader(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        grids: &Grids,
        buffer: BufferKey,
        shader: ShaderKey,
        padding: u32,
    ) {
        // Вычисляем размер текстуры
        let (width, height) = if let Some(buf) = grids.buffers.get(buffer) {
            if let Some(gs) = grids.assets.glyphsets.get(buf.glyphset()) {
                let w = buf.w * gs.tile_w + padding * 2;
                let h = buf.h * gs.tile_h + padding * 2;
                (w as i32, h as i32)
            } else {
                return;
            }
        } else {
            return;
        };
        
        // Создаём RenderTexture
        if let Ok(rt) = rl.load_render_texture(thread, width as u32, height as u32) {
            // Включаем билинейную фильтрацию для плавных эффектов шейдера
            unsafe {
                raylib::ffi::SetTextureFilter(
                    *rt.texture().as_ref(),
                    raylib::ffi::TextureFilter::TEXTURE_FILTER_BILINEAR as i32,
                );
            }
            self.buffer_shaders.insert(buffer, BufferShaderData {
                shader,
                padding,
                render_texture: rt,
            });
        }
    }

    /// Обновить RenderTexture буфера (при resize)
    pub fn update_buffer_shader_texture(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        grids: &Grids,
        buffer: BufferKey,
    ) {
        if let Some(data) = self.buffer_shaders.get(&buffer) {
            let shader = data.shader;
            let padding = data.padding;
            // Пересоздаём текстуру с новым размером
            self.attach_shader(rl, thread, grids, buffer, shader, padding);
        }
    }

    /// Убрать шейдер с буфера
    pub fn clear_buffer_shader(&mut self, buffer: BufferKey) {
        self.buffer_shaders.remove(&buffer);
    }

    /// Получить текущее время шейдеров
    pub fn shader_time(&self) -> f32 {
        self.current_time
    }

    /// Проход 1: рендерит все буферы с шейдерами в их текстуры
    pub fn render_offscreen(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        grids: &mut Grids,
        render_list: &[RenderItem],
    ) {
        // Обновляем время
        self.current_time = self.start_time.elapsed().as_secs_f32();
        
        // Проходим по плоскому списку и рендерим буферы с шейдерами в их текстуры
        for item in render_list {
            let buffer_key = item.buffer;
            
            // Если у буфера есть шейдер, рендерим его содержимое в текстуру
            if let Some(shader_data) = self.buffer_shaders.get_mut(&buffer_key) {
                let padding = shader_data.padding as i32;
                // Получаем указатель на текстуру (через raw pointer чтобы обойти borrow checker)
                let rt_ptr = &mut shader_data.render_texture as *mut RenderTexture2D;
            
                let rt = unsafe { &mut *rt_ptr };
                let mut texture_d = rl.begin_texture_mode(thread, rt);
                texture_d.clear_background(Color::BLANK);
                // Рисуем буфер в текстуру
                // Важно: рисуем в (padding, padding), так как это локальная отрисовка в текстуру
                self.draw_single_buffer(&mut texture_d, grids, buffer_key, padding, padding, 1.0, true);
            }
        }
    }

    /// Проход 2: рисует дерево буферов с применением шейдеров
    pub fn draw(
        &mut self,
        d: &mut RaylibDrawHandle,
        grids: &mut Grids,
        render_list: &[RenderItem],
    ) {
        for item in render_list {
            if self.buffer_shaders.contains_key(&item.buffer) {
                // Если есть шейдер, рисуем результат из текстуры (Pass 1)
                self.draw_buffer_with_shader(d, grids, item.buffer, item.screen_x, item.screen_y);
            } else {
                // Иначе рисуем буфер напрямую
                self.draw_single_buffer(d, grids, item.buffer, item.screen_x, item.screen_y, item.opacity, false);
            }
        }
    }

    /// Подготовить post-process текстуру
    pub fn prepare_post_process(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        width: u32,
        height: u32,
    ) {
        let need_recreate = self.post_process_texture.as_ref()
            .map(|t| t.texture().width != width as i32 || t.texture().height != height as i32)
            .unwrap_or(true);
        
        if need_recreate {
            if let Ok(rt) = rl.load_render_texture(thread, width, height) {
                self.post_process_texture = Some(rt);
            }
        }
    }

    /// Завершить post-process
    pub fn end_post_process(&mut self, d: &mut RaylibDrawHandle, grids: &mut Grids) {
        if let (Some(shader_key), Some(ref rt)) = (grids.post_process_shader, &self.post_process_texture) {
            self.current_time = self.start_time.elapsed().as_secs_f32();
            
            let tex_w = rt.texture().width as f32;
            let tex_h = rt.texture().height as f32;
            let screen_w = d.get_screen_width() as f32;
            let screen_h = d.get_screen_height() as f32;
            
            if let Some(shader_data) = grids.assets.shaders.get_mut(shader_key) {
                shader_data.apply_uniforms();
                shader_data.apply_auto_uniforms(
                    (tex_w, tex_h),
                    self.current_time,
                    (screen_w, screen_h),
                );
                
                {
                    let mut shader_mode = d.begin_shader_mode(&mut shader_data.shader);
                    shader_mode.draw_texture_rec(
                        rt.texture(),
                        Rectangle::new(0.0, tex_h, tex_w, -tex_h),
                        Vector2::new(0.0, 0.0),
                        Color::WHITE,
                    );
                }
            }
        }
    }

    pub fn post_process_texture_mut(&mut self) -> Option<&mut RenderTexture2D> {
        self.post_process_texture.as_mut()
    }

    // --- Внутренние методы ---

    fn draw_buffer_with_shader(&mut self, d: &mut RaylibDrawHandle, grids: &mut Grids, buffer: BufferKey, screen_x: i32, screen_y: i32) {
        self.current_time = self.start_time.elapsed().as_secs_f32();
        
        if let Some(shader_data) = self.buffer_shaders.get(&buffer) {
            let shader_key = shader_data.shader;
            let padding = shader_data.padding as i32;
            let tex_w = shader_data.render_texture.texture().width as f32;
            let tex_h = shader_data.render_texture.texture().height as f32;
            
            if let Some(shader) = grids.assets.shaders.get_mut(shader_key) {
                shader.apply_uniforms();
                shader.apply_auto_uniforms(
                    (tex_w, tex_h),
                    self.current_time,
                    (d.get_screen_width() as f32, d.get_screen_height() as f32),
                );
                
                let shader_ptr = &mut shader.shader as *mut Shader;
                
                if let Some(rt_data) = self.buffer_shaders.get(&buffer) {
                    let rt = &rt_data.render_texture;
                    unsafe {
                        let mut shader_mode = d.begin_shader_mode(&mut *shader_ptr);
                        shader_mode.draw_texture_rec(
                            rt.texture(),
                            Rectangle::new(0.0, tex_h, tex_w, -tex_h),
                            Vector2::new((screen_x - padding) as f32, (screen_y - padding) as f32),
                            Color::WHITE,
                        );
                    }
                }
            }
        }
    }

    fn draw_single_buffer<D: RaylibDraw>(
        &mut self,
        _d: &mut D,
        grids: &mut Grids,
        buffer_key: BufferKey,
        screen_x: i32,
        screen_y: i32,
        opacity: f32,
        force_rebuild: bool,
    ) {
        // 1. Check buffer state
        let (is_dirty, glyphset_key, buf_w, buf_h, is_dynamic) = if let Some(buf) = grids.buffers.get(buffer_key) {
            if !buf.visible { return; }
            (buf.dirty || force_rebuild, buf.glyphset(), buf.w, buf.h, buf.dynamic)
        } else {
            return;
        };

        // --- PATH A: IMMEDIATE MODE (Dynamic) ---
        if is_dynamic {
            self.draw_immediate(grids, buffer_key, glyphset_key, buf_w, buf_h, opacity, screen_x, screen_y);
            return;
        }

        // --- PATH B: CACHED MESH (Static) ---
        
        // Ensure default material is loaded (lazy init)
        if self.default_material.is_none() {
            unsafe {
                self.default_material = Some(LoadMaterialDefault());
            }
        }

        if is_dirty || !self.buffer_batches.contains_key(&buffer_key) {
            self.rebuild_buffer_batch(grids, buffer_key, glyphset_key, buf_w, buf_h);
            if let Some(buf) = grids.buffers.get_mut(buffer_key) {
                buf.dirty = false;
            }
        }

        // 2. Draw from cache
        if let Some(batches) = self.buffer_batches.get(&buffer_key) {
            let effective_opacity = opacity; // We assume buffer.opacity is handled by caller or baked? 
            // Actually caller passes: parent_opacity * buffer.opacity.
            // We apply this to the cached colors.

            if let Some(material) = &mut self.default_material {
                // Set material color for opacity
                let opacity_col = Color::WHITE.alpha(effective_opacity);
                
                unsafe {
                    (*material.maps).color = opacity_col.into();
                }

                // Create transform matrix
                let transform = Matrix::translate(screen_x as f32, screen_y as f32, 0.0).into();

                for batch in batches {
                    if batch.mesh.vertexCount > 0 {
                        // Set texture
                        unsafe { (*material.maps).texture.id = batch.texture_id; }
                        
                        unsafe {
                            BeginBlendMode(batch.blend.to_ffi());
                            DrawMesh(batch.mesh, *material, transform);
                            EndBlendMode();
                        }
                    }
                }
            }
        }
    }
    
    fn draw_immediate(
        &self,
        grids: &Grids,
        buffer_key: BufferKey,
        glyphset_key: GlyphsetKey,
        w: u32,
        h: u32,
        opacity: f32,
        screen_x: i32,
        screen_y: i32,
    ) {
        let buffer = match grids.buffers.get(buffer_key) {
            Some(b) => b,
            None => return,
        };
        let glyphset = match grids.assets.glyphsets.get(glyphset_key) {
            Some(g) => g,
            None => return,
        };
        
        let tile_w = glyphset.tile_w as f32;
        let tile_h = glyphset.tile_h as f32;
        let effective_opacity = opacity * buffer.opacity;

        // --- PASS 1: BACKGROUNDS ---
        unsafe {
            rlBegin(RL_QUADS as i32);
            rlSetTexture(rlGetTextureIdDefault());
            
            for y in 0..h {
                for x in 0..w {
                    if let Some(ch) = buffer.get_char_ref(x, y) {
                        let bg_alpha = (ch.bcolor.a as f32 * effective_opacity) as u8;
                        if bg_alpha > 0 {
                            let dst_x = screen_x as f32 + (x as f32 * tile_w);
                            let dst_y = screen_y as f32 + (y as f32 * tile_h);
                            
                            rlColor4ub(ch.bcolor.r, ch.bcolor.g, ch.bcolor.b, bg_alpha);
                            rlTexCoord2f(0.0, 0.0);
                            rlVertex2f(dst_x, dst_y);
                            rlVertex2f(dst_x, dst_y + tile_h);
                            rlVertex2f(dst_x + tile_w, dst_y + tile_h);
                            rlVertex2f(dst_x + tile_w, dst_y);
                        }
                    }
                }
            }
            rlEnd();
        }

        // --- PASS 2: FOREGROUNDS ---
        let mut current_tex = 0;
        let mut current_blend = Blend::Alpha;
        let mut batch_active = false;

        unsafe { BeginBlendMode(current_blend.to_ffi()); }

        for y in 0..h {
            for x in 0..w {
                if let Some(ch) = buffer.get_char_ref(x, y) {
                    let fg_alpha = (ch.fcolor.a as f32 * effective_opacity) as u8;
                    if fg_alpha == 0 { continue; }

                    let global_id = glyphset.luts.get(ch.variant_id as usize)
                        .and_then(|lut: &Vec<u32>| lut.get(ch.code as usize))
                        .copied()
                        .unwrap_or(glyphset.default_global_id);
                    
                    let (atlas_key, physical_glyph) = grids.assets.global_registry.entries[global_id as usize];
                    let atlas = &grids.assets.atlases[atlas_key];
                    let (src, _, _) = atlas.get_glyph_source(physical_glyph);
                    let tex_id = atlas.texture.id;

                    if tex_id != current_tex || ch.fg_blend != current_blend {
                        if batch_active { unsafe { rlEnd(); } batch_active = false; }
                        if ch.fg_blend != current_blend {
                            unsafe { EndBlendMode(); BeginBlendMode(ch.fg_blend.to_ffi()); }
                            current_blend = ch.fg_blend;
                        }
                        if tex_id != current_tex {
                            unsafe { rlSetTexture(tex_id); }
                            current_tex = tex_id;
                        }
                    }

                    if !batch_active { unsafe { rlBegin(RL_QUADS as i32); } batch_active = true; }

                    let dst_x = screen_x as f32 + (x as f32 * tile_w);
                    let dst_y = screen_y as f32 + (y as f32 * tile_h);
                    let (tex_w, tex_h) = atlas.texture_size();
                    let mut u_min = src.x / tex_w;
                    let mut v_min = src.y / tex_h;
                    let mut u_max = (src.x + src.width) / tex_w;
                    let mut v_max = (src.y + src.height) / tex_h;

                    if ch.transform.flip_h { std::mem::swap(&mut u_min, &mut u_max); }
                    if ch.transform.flip_v { std::mem::swap(&mut v_min, &mut v_max); }

                    unsafe {
                        rlColor4ub(ch.fcolor.r, ch.fcolor.g, ch.fcolor.b, fg_alpha);
                        rlTexCoord2f(u_min, v_min); rlVertex2f(dst_x, dst_y);
                        rlTexCoord2f(u_min, v_max); rlVertex2f(dst_x, dst_y + tile_h);
                        rlTexCoord2f(u_max, v_max); rlVertex2f(dst_x + tile_w, dst_y + tile_h);
                        rlTexCoord2f(u_max, v_min); rlVertex2f(dst_x + tile_w, dst_y);
                    }
                }
            }
        }
        unsafe {
            if batch_active { rlEnd(); }
            EndBlendMode();
        }
    }

    fn rebuild_buffer_batch(
        &mut self,
        grids: &Grids,
        buffer_key: BufferKey,
        glyphset_key: GlyphsetKey,
        w: u32,
        h: u32,
    ) {
        let buffer = match grids.buffers.get(buffer_key) {
            Some(b) => b,
            None => return,
        };
        
        let glyphset = match grids.assets.glyphsets.get(glyphset_key) {
            Some(g) => g,
            None => return,
        };
        
        let tile_w = glyphset.tile_w as f32;
        let tile_h = glyphset.tile_h as f32;

        // Get or create batch list for this buffer
        let batches = self.buffer_batches.entry(buffer_key).or_default();
        let mut batch_idx = 0;
        
        // --- PASS 1: BACKGROUNDS ---
        // Ensure we have a batch for backgrounds
        if batch_idx >= batches.len() { batches.push(Batch::new()); }
        let bg_batch = &mut batches[batch_idx];
        
        // Reset batch for reuse
        bg_batch.texture_id = unsafe { rlGetTextureIdDefault() };
        bg_batch.blend = Blend::Alpha;
        bg_batch.vertices.clear();
        bg_batch.texcoords.clear();
        bg_batch.colors.clear();
        
        for y in 0..h {
            for x in 0..w {
                if let Some(ch) = buffer.get_char_ref(x, y) {
                    if ch.bcolor.a > 0 {
                        let dst_x = x as f32 * tile_w;
                        let dst_y = y as f32 * tile_h;
                        
                        // Triangle 1 (TL, BL, BR)
                        bg_batch.vertices.extend_from_slice(&[dst_x, dst_y, 0.0]);
                        bg_batch.vertices.extend_from_slice(&[dst_x, dst_y + tile_h, 0.0]);
                        bg_batch.vertices.extend_from_slice(&[dst_x + tile_w, dst_y + tile_h, 0.0]);
                        
                        // Triangle 2 (BR, TR, TL)
                        bg_batch.vertices.extend_from_slice(&[dst_x + tile_w, dst_y + tile_h, 0.0]);
                        bg_batch.vertices.extend_from_slice(&[dst_x + tile_w, dst_y, 0.0]);
                        bg_batch.vertices.extend_from_slice(&[dst_x, dst_y, 0.0]);
                        
                        // 6 UVs (center of white pixel)
                        for _ in 0..6 { bg_batch.texcoords.extend_from_slice(&[0.0, 0.0]); }
                        
                        // 6 Colors
                        for _ in 0..6 { bg_batch.colors.extend_from_slice(&[ch.bcolor.r, ch.bcolor.g, ch.bcolor.b, ch.bcolor.a]); }
                    }
                }
            }
        }
        
        if !bg_batch.vertices.is_empty() {
            batch_idx += 1;
        }

        // --- PASS 2: FOREGROUNDS ---
        let mut current_tex = 0;
        let mut current_blend = Blend::Alpha;
        
        // Helper to get current foreground batch
        // We can't use a closure easily due to borrow checker with `batches` and `batch_idx`
        // So we manage index manually.

        for y in 0..h {
            for x in 0..w {
                if let Some(ch) = buffer.get_char_ref(x, y) {
                    if ch.fcolor.a == 0 { continue; }

                    // Resolve global_id and Atlas
                    let global_id = glyphset.luts.get(ch.variant_id as usize)
                        .and_then(|lut: &Vec<u32>| lut.get(ch.code as usize))
                        .copied()
                        .unwrap_or(glyphset.default_global_id);
                    
                    let (atlas_key, physical_glyph) = grids.assets.global_registry.entries[global_id as usize];
                    let atlas = &grids.assets.atlases[atlas_key];
                    let (src, _, _) = atlas.get_glyph_source(physical_glyph);
                    let tex_id = atlas.texture.id;
                    
                    // State change check
                    if tex_id != current_tex || ch.fg_blend != current_blend {
                        // If we were building a batch and it has data, move to next
                        if batch_idx < batches.len() && !batches[batch_idx].vertices.is_empty() {
                            batch_idx += 1;
                        }
                        
                        // Ensure batch exists
                        if batch_idx >= batches.len() { batches.push(Batch::new()); }
                        
                        // Initialize new batch state
                        let batch = &mut batches[batch_idx];
                        batch.texture_id = tex_id;
                        batch.blend = ch.fg_blend;
                        batch.vertices.clear();
                        batch.texcoords.clear();
                        batch.colors.clear();
                        
                        current_tex = tex_id;
                        current_blend = ch.fg_blend;
                    }
                    
                    let batch = &mut batches[batch_idx];
                    
                    // Calculate Vertices & UVs
                    let dst_x = x as f32 * tile_w;
                    let dst_y = y as f32 * tile_h;
                    
                    // UVs
                    let (tex_w, tex_h) = atlas.texture_size();
                    let mut u_min = src.x / tex_w;
                    let mut v_min = src.y / tex_h;
                    let mut u_max = (src.x + src.width) / tex_w;
                    let mut v_max = (src.y + src.height) / tex_h;
                    
                    if ch.transform.flip_h { std::mem::swap(&mut u_min, &mut u_max); }
                    if ch.transform.flip_v { std::mem::swap(&mut v_min, &mut v_max); }
                    
                    // Vertices (Local to tile)
                    // TL, BL, BR, TR
                    let mut v_tl = (0.0, 0.0);
                    let mut v_bl = (0.0, tile_h);
                    let mut v_br = (tile_w, tile_h);
                    let mut v_tr = (tile_w, 0.0);
                    
                    // Rotation (around center)
                    if ch.transform.rotation != Rotation::None {
                        let cx = tile_w * 0.5;
                        let cy = tile_h * 0.5;
                        let rot_rad = ch.transform.rotation.degrees().to_radians();
                        let cos_r = rot_rad.cos();
                        let sin_r = rot_rad.sin();
                        
                        let rotate = |(vx, vy): (f32, f32)| -> (f32, f32) {
                            let dx = vx - cx;
                            let dy = vy - cy;
                            (
                                cx + dx * cos_r - dy * sin_r,
                                cy + dx * sin_r + dy * cos_r
                            )
                        };
                        
                        v_tl = rotate(v_tl);
                        v_bl = rotate(v_bl);
                        v_br = rotate(v_br);
                        v_tr = rotate(v_tr);
                    }
                    
                    // 6 Colors
                    for _ in 0..6 {
                        batch.colors.extend_from_slice(&[ch.fcolor.r, ch.fcolor.g, ch.fcolor.b, ch.fcolor.a]);
                    }
                    
                    // Tri 1 UVs (TL, BL, BR)
                    batch.texcoords.extend_from_slice(&[u_min, v_min]);
                    batch.texcoords.extend_from_slice(&[u_min, v_max]);
                    batch.texcoords.extend_from_slice(&[u_max, v_max]);
                    // Tri 2 UVs (BR, TR, TL)
                    batch.texcoords.extend_from_slice(&[u_max, v_max]);
                    batch.texcoords.extend_from_slice(&[u_max, v_min]);
                    batch.texcoords.extend_from_slice(&[u_min, v_min]);
                    
                    // Tri 1 Vertices (TL, BL, BR)
                    batch.vertices.extend_from_slice(&[dst_x + v_tl.0, dst_y + v_tl.1, 0.0]);
                    batch.vertices.extend_from_slice(&[dst_x + v_bl.0, dst_y + v_bl.1, 0.0]);
                    batch.vertices.extend_from_slice(&[dst_x + v_br.0, dst_y + v_br.1, 0.0]);
                    
                    // Tri 2 Vertices (BR, TR, TL)
                    batch.vertices.extend_from_slice(&[dst_x + v_br.0, dst_y + v_br.1, 0.0]);
                    batch.vertices.extend_from_slice(&[dst_x + v_tr.0, dst_y + v_tr.1, 0.0]);
                    batch.vertices.extend_from_slice(&[dst_x + v_tl.0, dst_y + v_tl.1, 0.0]);
                }
            }
        }
        
        // Finalize last batch index
        if batch_idx < batches.len() && !batches[batch_idx].vertices.is_empty() {
            batch_idx += 1;
        }
        
        // Remove unused batches (if buffer content shrank)
        batches.truncate(batch_idx);
        
        // Upload to GPU
        for batch in batches.iter_mut() {
            let vertex_count = batch.vertices.len() / 3;
            batch.mesh.vertexCount = vertex_count as i32;
            batch.mesh.triangleCount = (vertex_count / 3) as i32; // 3 vertices per tri
            
            if vertex_count > batch.gpu_capacity {
                // Reallocate
                unsafe {
                    // Ensure pointers are null before Unload so it doesn't free Vec memory
                    batch.mesh.vertices = std::ptr::null_mut();
                    batch.mesh.texcoords = std::ptr::null_mut();
                    batch.mesh.colors = std::ptr::null_mut();

                    UnloadMesh(batch.mesh); // Free old VBOs
                    
                    // Reset IDs because they are now invalid/freed
                    batch.mesh.vaoId = 0;
                    batch.mesh.vboId = std::ptr::null_mut();

                    // Assign pointers for UploadMesh
                    batch.mesh.vertices = batch.vertices.as_mut_ptr();
                    batch.mesh.texcoords = batch.texcoords.as_mut_ptr();
                    batch.mesh.colors = batch.colors.as_mut_ptr();

                    UploadMesh(&mut batch.mesh, true); // Allocate new VBOs (dynamic)
                }
                batch.gpu_capacity = vertex_count;
            } else {
                // Update existing
                unsafe {
                    UpdateMeshBuffer(batch.mesh, 0, batch.vertices.as_ptr() as *const _, (batch.vertices.len() * 4) as i32, 0);
                    UpdateMeshBuffer(batch.mesh, 1, batch.texcoords.as_ptr() as *const _, (batch.texcoords.len() * 4) as i32, 0);
                    UpdateMeshBuffer(batch.mesh, 3, batch.colors.as_ptr() as *const _, (batch.colors.len() * 1) as i32, 0);
                }
            }
            
            // Clear pointers so UnloadMesh doesn't try to free our Vec memory later
            batch.mesh.vertices = std::ptr::null_mut();
            batch.mesh.texcoords = std::ptr::null_mut();
            batch.mesh.colors = std::ptr::null_mut();
        }
    }
}