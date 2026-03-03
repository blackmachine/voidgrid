use std::collections::HashMap;
use std::time::Instant;

use raylib::prelude::*;
use raylib::ffi::{
    BeginBlendMode, EndBlendMode,
    rlBegin, rlEnd, rlColor4ub, rlTexCoord2f, rlVertex2f, rlSetTexture,
    rlGetTextureIdDefault, rlPushMatrix, rlPopMatrix, rlTranslatef,
    RL_QUADS,
};

use crate::grids::Grids;
use crate::types::{BufferKey, ShaderKey, Blend, GlyphsetKey};
use crate::types::Rotation;
/// Максимальная глубина вложенности буферов
const MAX_BUFFER_DEPTH: u8 = 8;

/// Кэшированный батч отрисовки
struct Batch {
    texture_id: u32,
    blend: Blend,
    vertices: Vec<f32>,   // x, y (local)
    texcoords: Vec<f32>,  // u, v
    colors: Vec<Color>,   // Base color (without opacity applied)
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
            start_time: Instant::now(),
            current_time: 0.0,
        }
    }

    /// Загрузить шейдер маски (internal)
    pub fn load_mask_shader(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        shader_path: &str,
    ) -> Result<(), String> {
        let shader = rl.load_shader(thread, None, Some(shader_path));
        
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
            if let Some(gs) = grids.glyphsets.get(buf.glyphset()) {
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
        root: BufferKey,
        screen_x: i32,
        screen_y: i32,
    ) {
        // Обновляем время
        self.current_time = self.start_time.elapsed().as_secs_f32();
        
        // Собираем буферы с шейдерами и их позиции
        let shader_buffers = self.collect_shader_buffers(grids, root, screen_x, screen_y, MAX_BUFFER_DEPTH);
        
        // Рендерим каждый буфер с шейдером в его текстуру
        for (buffer_key, _sx, _sy) in &shader_buffers {
            let padding = self.buffer_shaders.get(buffer_key).map(|d| d.padding).unwrap_or(0) as i32;
            
            // Получаем указатель на текстуру
            let rt_ptr = self.buffer_shaders.get_mut(buffer_key)
                .map(|data| &mut data.render_texture as *mut RenderTexture2D);
            
            if let Some(rt_ptr) = rt_ptr {
                let rt = unsafe { &mut *rt_ptr };
                let mut texture_d = rl.begin_texture_mode(thread, rt);
                texture_d.clear_background(Color::BLANK);
                // Рисуем буфер в текстуру
                self.draw_single_buffer(&mut texture_d, grids, *buffer_key, padding, padding, 1.0, true);
            }
        }
    }

    /// Проход 2: рисует дерево буферов с применением шейдеров
    pub fn draw(
        &mut self,
        d: &mut RaylibDrawHandle,
        grids: &mut Grids,
        root: BufferKey,
        screen_x: i32,
        screen_y: i32,
    ) {
        // Собираем буферы с шейдерами
        let shader_buffers = self.collect_shader_buffers(grids, root, screen_x, screen_y, MAX_BUFFER_DEPTH);
        
        // Рисуем основное дерево (буферы с шейдерами пропускаются)
        self.draw_internal_skip_shaders(d, grids, root, screen_x, screen_y, 1.0, MAX_BUFFER_DEPTH);
        
        // Рисуем буферы с шейдерами
        for (buffer_key, sx, sy) in shader_buffers {
            self.draw_buffer_with_shader(d, grids, buffer_key, sx, sy);
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
            
            if let Some(shader_data) = grids.shaders.get_mut(shader_key) {
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

    fn collect_shader_buffers(
        &self,
        grids: &Grids,
        root: BufferKey,
        screen_x: i32,
        screen_y: i32,
        depth: u8,
    ) -> Vec<(BufferKey, i32, i32)> {
        let mut result = Vec::new();
        self.collect_shader_buffers_internal(grids, root, screen_x, screen_y, depth, &mut result);
        result
    }
    
    fn collect_shader_buffers_internal(
        &self,
        grids: &Grids,
        buffer_key: BufferKey,
        screen_x: i32,
        screen_y: i32,
        depth: u8,
        result: &mut Vec<(BufferKey, i32, i32)>,
    ) {
        if depth == 0 { return; }
        
        let buffer = match grids.buffers.get(buffer_key) {
            Some(b) => b,
            None => return,
        };
        
        if !buffer.visible { return; }
        
        let gs = match grids.glyphsets.get(buffer.glyphset()) {
            Some(g) => g,
            None => return,
        };
        
        let tile_w = gs.tile_w as i32;
        let tile_h = gs.tile_h as i32;
        
        if self.buffer_shaders.contains_key(&buffer_key) {
            result.push((buffer_key, screen_x, screen_y));
        }
        
        let mut children: Vec<_> = grids.attachments.iter()
            .filter(|a| a.parent == buffer_key)
            .collect();
        children.sort_by_key(|a| a.z_index);
        
        for att in children {
            let child_x = screen_x + (att.x as i32 * tile_w);
            let child_y = screen_y + (att.y as i32 * tile_h);
            self.collect_shader_buffers_internal(grids, att.child, child_x, child_y, depth - 1, result);
        }
    }

    fn draw_buffer_with_shader(&mut self, d: &mut RaylibDrawHandle, grids: &mut Grids, buffer: BufferKey, screen_x: i32, screen_y: i32) {
        self.current_time = self.start_time.elapsed().as_secs_f32();
        
        if let Some(shader_data) = self.buffer_shaders.get(&buffer) {
            let shader_key = shader_data.shader;
            let padding = shader_data.padding as i32;
            let tex_w = shader_data.render_texture.texture().width as f32;
            let tex_h = shader_data.render_texture.texture().height as f32;
            
            if let Some(shader) = grids.shaders.get_mut(shader_key) {
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
        // 1. Check if we need to rebuild the cache
        let (is_dirty, glyphset_key, buf_w, buf_h) = if let Some(buf) = grids.buffers.get(buffer_key) {
            if !buf.visible { return; }
            (buf.dirty || force_rebuild, buf.glyphset(), buf.w, buf.h)
        } else {
            return;
        };

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

            unsafe {
                rlPushMatrix();
                rlTranslatef(screen_x as f32, screen_y as f32, 0.0);

                for batch in batches {
                    // Setup state
                    rlSetTexture(batch.texture_id);
                    BeginBlendMode(batch.blend.to_ffi());
                    rlBegin(RL_QUADS as i32);

                    let count = batch.vertices.len() / 2;
                    for i in 0..count {
                        let col = &batch.colors[i];
                        // Apply dynamic opacity to cached color
                        let alpha = (col.a as f32 * effective_opacity) as u8;
                        
                        rlColor4ub(col.r, col.g, col.b, alpha);
                        rlTexCoord2f(batch.texcoords[i*2], batch.texcoords[i*2+1]);
                        rlVertex2f(batch.vertices[i*2], batch.vertices[i*2+1]);
                    }

                    rlEnd();
                    EndBlendMode();
                }

                rlPopMatrix();
            }
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
        
        let glyphset = match grids.glyphsets.get(glyphset_key) {
            Some(g) => g,
            None => return,
        };
        
        let tile_w = glyphset.tile_w as f32;
        let tile_h = glyphset.tile_h as f32;

        let mut batches: Vec<Batch> = Vec::new();
        
        // --- PASS 1: BACKGROUNDS ---
        let mut bg_verts = Vec::new();
        let mut bg_uvs = Vec::new();
        let mut bg_cols = Vec::new();

        for y in 0..h {
            for x in 0..w {
                if let Some(ch) = buffer.get_char_ref(x, y) {
                    if ch.bcolor.a > 0 {
                        let dst_x = x as f32 * tile_w;
                        let dst_y = y as f32 * tile_h;
                        
                        // 4 vertices
                        bg_verts.push(dst_x); bg_verts.push(dst_y);
                        bg_verts.push(dst_x); bg_verts.push(dst_y + tile_h);
                        bg_verts.push(dst_x + tile_w); bg_verts.push(dst_y + tile_h);
                        bg_verts.push(dst_x + tile_w); bg_verts.push(dst_y);
                        
                        // 4 UVs (center of white pixel)
                        for _ in 0..4 { bg_uvs.push(0.0); bg_uvs.push(0.0); }
                        
                        // 4 Colors
                        for _ in 0..4 { bg_cols.push(ch.bcolor); }
                    }
                }
            }
        }
        
        if !bg_verts.is_empty() {
            batches.push(Batch {
                texture_id: unsafe { rlGetTextureIdDefault() },
                blend: Blend::Alpha, // Assuming alpha for BG
                vertices: bg_verts,
                texcoords: bg_uvs,
                colors: bg_cols,
            });
        }

        // --- PASS 2: FOREGROUNDS ---
        // Temporary storage for current batch
        let mut current_tex = 0;
        let mut current_blend = Blend::Alpha;
        let mut fg_verts = Vec::new();
        let mut fg_uvs = Vec::new();
        let mut fg_cols = Vec::new();

        let flush_batch = |batches: &mut Vec<Batch>, tex: u32, blend: Blend, v: &mut Vec<f32>, uv: &mut Vec<f32>, c: &mut Vec<Color>| {
            if !v.is_empty() {
                batches.push(Batch {
                    texture_id: tex,
                    blend,
                    vertices: std::mem::take(v),
                    texcoords: std::mem::take(uv),
                    colors: std::mem::take(c),
                });
            }
        };

        for y in 0..h {
            for x in 0..w {
                if let Some(ch) = buffer.get_char_ref(x, y) {
                    if ch.fcolor.a == 0 { continue; }

                    // Resolve global_id and Atlas
                    let global_id = glyphset.luts.get(ch.variant_id as usize)
                        .and_then(|lut: &Vec<u32>| lut.get(ch.code as usize))
                        .copied()
                        .unwrap_or(glyphset.default_global_id);
                    
                    let (atlas_key, physical_glyph) = grids.global_registry.entries[global_id as usize];
                    let atlas = &grids.atlases[atlas_key];
                    let (src, _, _) = atlas.get_glyph_source(physical_glyph);
                    let tex_id = atlas.texture.id;
                    
                    // State change check
                    if tex_id != current_tex || ch.fg_blend != current_blend {
                        flush_batch(&mut batches, current_tex, current_blend, &mut fg_verts, &mut fg_uvs, &mut fg_cols);
                        current_tex = tex_id;
                        current_blend = ch.fg_blend;
                    }
                    
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
                    
                    fg_cols.push(ch.fcolor); fg_cols.push(ch.fcolor); fg_cols.push(ch.fcolor); fg_cols.push(ch.fcolor);
                    
                    fg_uvs.push(u_min); fg_uvs.push(v_min);
                    fg_uvs.push(u_min); fg_uvs.push(v_max);
                    fg_uvs.push(u_max); fg_uvs.push(v_max);
                    fg_uvs.push(u_max); fg_uvs.push(v_min);
                    
                    fg_verts.push(dst_x + v_tl.0); fg_verts.push(dst_y + v_tl.1);
                    fg_verts.push(dst_x + v_bl.0); fg_verts.push(dst_y + v_bl.1);
                    fg_verts.push(dst_x + v_br.0); fg_verts.push(dst_y + v_br.1);
                    fg_verts.push(dst_x + v_tr.0); fg_verts.push(dst_y + v_tr.1);
                }
            }
        }
        
        flush_batch(&mut batches, current_tex, current_blend, &mut fg_verts, &mut fg_uvs, &mut fg_cols);
        
        self.buffer_batches.insert(buffer_key, batches);
    }

    fn draw_internal_skip_shaders<D: RaylibDraw>(
        &mut self,
        d: &mut D,
        grids: &mut Grids,
        buffer_key: BufferKey,
        screen_x: i32,
        screen_y: i32,
        parent_opacity: f32,
        depth: u8,
    ) {
        if depth == 0 { return; }
        
        // let buffer = match grids.buffers.get(buffer_key) {
        //     Some(b) => b,
        //     None => return,
        let (visible, opacity, glyphset_key) = if let Some(buf) = grids.buffers.get(buffer_key) {
            (buf.visible, buf.opacity, buf.glyphset())
        } else {
            return;

        };
         
        
        // if !buffer.visible { return; }
        
        // let gs = match grids.glyphsets.get(buffer.glyphset()) {
        //     Some(g) => g,
        //     None => return,
        // };
        
        if !visible {return;}

        let (tile_w, tile_h) = if let Some(gs) = grids.glyphsets.get(glyphset_key) {
            (gs.tile_w as f32, gs.tile_h as f32)
        } else {
            return;
        };
        

        // let effective_opacity = parent_opacity * buffer.opacity;
        // let tile_w = gs.tile_w as f32;
        // let tile_h = gs.tile_h as f32;
        let effective_opacity = parent_opacity * opacity;

        
        if !self.buffer_shaders.contains_key(&buffer_key) {
            self.draw_single_buffer(d, grids, buffer_key, screen_x, screen_y, effective_opacity, false);
        }
        
        let mut children: Vec<_> = grids.attachments.iter()
            .filter(|a| a.parent == buffer_key)
            .cloned()
            .collect();
        children.sort_by_key(|a| a.z_index);
        
        for att in children {
            let child_x = screen_x + (att.x as f32 * tile_w) as i32;
            let child_y = screen_y + (att.y as f32 * tile_h) as i32;
            self.draw_internal_skip_shaders(d, grids, att.child, child_x, child_y, effective_opacity, depth - 1);
        }
    }
}