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

// ============================================================================
// Safe wrappers for raylib FFI
// ============================================================================

/// RAII-обёртка для raylib blend mode
struct BlendGuard;

impl BlendGuard {
    fn new(mode: Blend) -> Self {
        unsafe { BeginBlendMode(mode.to_ffi()); }
        Self
    }
}

impl Drop for BlendGuard {
    fn drop(&mut self) {
        unsafe { EndBlendMode(); }
    }
}

/// RAII-обёртка: отключает color blending, включает обратно при drop
struct NoBlendGuard;

impl NoBlendGuard {
    fn new() -> Self {
        unsafe { raylib::ffi::rlDisableColorBlend(); }
        Self
    }
}

impl Drop for NoBlendGuard {
    fn drop(&mut self) {
        unsafe { raylib::ffi::rlEnableColorBlend(); }
    }
}

/// RAII-обёртка для rlgl immediate-mode quad batch
struct QuadBatch;

impl QuadBatch {
    fn begin(texture_id: u32) -> Self {
        unsafe {
            rlSetTexture(texture_id);
            rlBegin(RL_QUADS as i32);
        }
        Self
    }

    #[allow(dead_code)]
    fn switch_texture(&self, texture_id: u32) {
        unsafe {
            rlEnd();
            rlSetTexture(texture_id);
            rlBegin(RL_QUADS as i32);
        }
    }

    fn color_quad(&self, x: f32, y: f32, w: f32, h: f32, r: u8, g: u8, b: u8, a: u8) {
        unsafe {
            rlColor4ub(r, g, b, a);
            rlTexCoord2f(0.0, 0.0); rlVertex2f(x, y);
            rlTexCoord2f(0.0, 0.0); rlVertex2f(x, y + h);
            rlTexCoord2f(0.0, 0.0); rlVertex2f(x + w, y + h);
            rlTexCoord2f(0.0, 0.0); rlVertex2f(x + w, y);
        }
    }

    fn textured_quad(
        &self,
        x: f32, y: f32, w: f32, h: f32,
        u_min: f32, v_min: f32, u_max: f32, v_max: f32,
        r: u8, g: u8, b: u8, a: u8,
    ) {
        unsafe {
            rlColor4ub(r, g, b, a);
            rlTexCoord2f(u_min, v_min); rlVertex2f(x, y);
            rlTexCoord2f(u_min, v_max); rlVertex2f(x, y + h);
            rlTexCoord2f(u_max, v_max); rlVertex2f(x + w, y + h);
            rlTexCoord2f(u_max, v_min); rlVertex2f(x + w, y);
        }
    }
}

impl Drop for QuadBatch {
    fn drop(&mut self) {
        unsafe { rlEnd(); }
    }
}

/// Safe wrapper: ID текстуры по умолчанию
fn default_texture_id() -> u32 {
    unsafe { rlGetTextureIdDefault() }
}

/// Safe wrapper: загрузка материала по умолчанию
fn load_default_material() -> Material {
    unsafe { LoadMaterialDefault() }
}

/// Safe wrapper: установка diffuse-цвета материала
fn set_material_color(material: &mut Material, color: Color) {
    unsafe { (*material.maps).color = color.into(); }
}

/// Safe wrapper: установка текстуры материала по ID
fn set_material_texture(material: &mut Material, texture_id: u32) {
    unsafe { (*material.maps).texture.id = texture_id; }
}

/// Safe wrapper: SetTextureFilter(BILINEAR) для RenderTexture
fn set_texture_filter_bilinear(rt: &RenderTexture2D) {
    unsafe {
        raylib::ffi::SetTextureFilter(
            *rt.texture().as_ref(),
            raylib::ffi::TextureFilter::TEXTURE_FILTER_BILINEAR as i32,
        );
    }
}

// ============================================================================
// DynamicMesh — safe-обёртка для жизненного цикла GPU-меша
// ============================================================================

struct DynamicMesh {
    mesh: Mesh,
    vertices: Vec<f32>,
    texcoords: Vec<f32>,
    colors: Vec<u8>,
    gpu_capacity: usize,
}

impl DynamicMesh {
    fn new() -> Self {
        Self {
            // Все поля Mesh — целые числа и указатели; нули валидны для всех
            mesh: unsafe { std::mem::zeroed() },
            vertices: Vec::new(),
            texcoords: Vec::new(),
            colors: Vec::new(),
            gpu_capacity: 0,
        }
    }

    fn clear(&mut self) {
        self.vertices.clear();
        self.texcoords.clear();
        self.colors.clear();
    }

    fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    fn vertex_count(&self) -> usize {
        self.vertices.len() / 3
    }

    /// Добавить quad из 4 вершин (TL, BL, BR, TR) + UV + цвет
    fn push_quad(
        &mut self,
        positions: [(f32, f32); 4],
        uvs: [(f32, f32); 4],
        color: [u8; 4],
    ) {
        let [tl, bl, br, tr] = positions;
        let [uv_tl, uv_bl, uv_br, uv_tr] = uvs;

        // Triangle 1: TL, BL, BR
        self.vertices.extend_from_slice(&[tl.0, tl.1, 0.0]);
        self.vertices.extend_from_slice(&[bl.0, bl.1, 0.0]);
        self.vertices.extend_from_slice(&[br.0, br.1, 0.0]);
        // Triangle 2: BR, TR, TL
        self.vertices.extend_from_slice(&[br.0, br.1, 0.0]);
        self.vertices.extend_from_slice(&[tr.0, tr.1, 0.0]);
        self.vertices.extend_from_slice(&[tl.0, tl.1, 0.0]);

        self.texcoords.extend_from_slice(&[uv_tl.0, uv_tl.1]);
        self.texcoords.extend_from_slice(&[uv_bl.0, uv_bl.1]);
        self.texcoords.extend_from_slice(&[uv_br.0, uv_br.1]);
        self.texcoords.extend_from_slice(&[uv_br.0, uv_br.1]);
        self.texcoords.extend_from_slice(&[uv_tr.0, uv_tr.1]);
        self.texcoords.extend_from_slice(&[uv_tl.0, uv_tl.1]);

        for _ in 0..6 {
            self.colors.extend_from_slice(&color);
        }
    }

    /// Загрузить/обновить данные на GPU
    fn upload(&mut self) {
        let vc = self.vertex_count();
        self.mesh.vertexCount = vc as i32;
        self.mesh.triangleCount = (vc / 3) as i32;

        if vc == 0 { return; }

        if vc > self.gpu_capacity {
            unsafe {
                // Отцепляем CPU-указатели перед выгрузкой, чтобы raylib не пытался их освободить
                self.mesh.vertices = std::ptr::null_mut();
                self.mesh.texcoords = std::ptr::null_mut();
                self.mesh.colors = std::ptr::null_mut();
                UnloadMesh(self.mesh);

                self.mesh.vaoId = 0;
                self.mesh.vboId = std::ptr::null_mut();

                self.mesh.vertices = self.vertices.as_mut_ptr();
                self.mesh.texcoords = self.texcoords.as_mut_ptr();
                self.mesh.colors = self.colors.as_mut_ptr();
                UploadMesh(&mut self.mesh, true);
            }
            self.gpu_capacity = vc;
        } else {
            unsafe {
                UpdateMeshBuffer(self.mesh, 0, self.vertices.as_ptr() as *const _, (self.vertices.len() * 4) as i32, 0);
                UpdateMeshBuffer(self.mesh, 1, self.texcoords.as_ptr() as *const _, (self.texcoords.len() * 4) as i32, 0);
                UpdateMeshBuffer(self.mesh, 3, self.colors.as_ptr() as *const _, self.colors.len() as i32, 0);
            }
        }

        // Отцепляем CPU-указатели — Vec'ы владеют памятью, не mesh
        self.mesh.vertices = std::ptr::null_mut();
        self.mesh.texcoords = std::ptr::null_mut();
        self.mesh.colors = std::ptr::null_mut();
    }

    /// Отрисовать меш с заданным материалом и трансформацией
    fn draw(&self, material: &mut Material, transform: Matrix) {
        if self.mesh.vertexCount > 0 {
            unsafe { DrawMesh(self.mesh, *material, transform.into()); }
        }
    }
}

impl Drop for DynamicMesh {
    fn drop(&mut self) {
        self.mesh.vertices = std::ptr::null_mut();
        self.mesh.texcoords = std::ptr::null_mut();
        self.mesh.colors = std::ptr::null_mut();
        unsafe { UnloadMesh(self.mesh); }
    }
}

// ============================================================================
// Batch — кэшированный батч отрисовки (без собственного unsafe)
// ============================================================================

struct Batch {
    texture_id: u32,
    blend: Blend,
    mesh: DynamicMesh,
}

impl Batch {
    fn new() -> Self {
        Self {
            texture_id: 0,
            blend: Blend::Alpha,
            mesh: DynamicMesh::new(),
        }
    }
}

// ============================================================================
// BufferShaderData
// ============================================================================

/// Данные буферного шейдера (цепочка шейдеров + ping-pong текстуры)
pub struct BufferShaderData {
    pub shaders: Vec<ShaderKey>,
    pub padding: u32,
    pub textures: [RenderTexture2D; 2],
    pub final_texture_idx: usize,
}

// ============================================================================
// Renderer
// ============================================================================

/// Система рендеринга
pub struct Renderer {
    mask_shader: Option<Shader>,
    loc_mask_tex: i32,
    loc_mask_src_rect: i32,
    loc_mask_tex_size: i32,
    loc_glyph_src_rect: i32,
    loc_glyph_tex_size: i32,
    loc_use_mask: i32,
    loc_bg_color: i32,
    post_process_texture: Option<RenderTexture2D>,
    buffer_shaders: HashMap<BufferKey, BufferShaderData>,
    buffer_batches: HashMap<BufferKey, Vec<Batch>>,
    default_material: Option<Material>,
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

    /// Добавить шейдер в цепочку буфера
    pub fn attach_shader(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        grids: &Grids,
        buffer: BufferKey,
        shader: ShaderKey,
        padding: u32,
    ) {
        if let Some(data) = self.buffer_shaders.get_mut(&buffer) {
            data.shaders.push(shader);
            return;
        }

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

        if let (Ok(rt0), Ok(rt1)) = (
            rl.load_render_texture(thread, width as u32, height as u32),
            rl.load_render_texture(thread, width as u32, height as u32)
        ) {
            set_texture_filter_bilinear(&rt0);
            set_texture_filter_bilinear(&rt1);
            self.buffer_shaders.insert(buffer, BufferShaderData {
                shaders: vec![shader],
                padding,
                textures: [rt0, rt1],
                final_texture_idx: 0,
            });
        }
    }

    pub fn update_buffer_shader_texture(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        grids: &Grids,
        buffer: BufferKey,
    ) {
        if let Some(data) = self.buffer_shaders.remove(&buffer) {
            let shaders = data.shaders.clone();
            let padding = data.padding;

            if let Some(first) = shaders.first() {
                self.attach_shader(rl, thread, grids, buffer, *first, padding);
                if let Some(new_data) = self.buffer_shaders.get_mut(&buffer) {
                    new_data.shaders = shaders;
                }
            }
        }
    }

    pub fn clear_buffer_shader(&mut self, buffer: BufferKey) {
        self.buffer_shaders.remove(&buffer);
    }

    pub fn shader_time(&self) -> f32 {
        self.current_time
    }

    pub fn render_offscreen(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        grids: &mut Grids,
        render_list: &[RenderItem],
    ) {
        self.current_time = self.start_time.elapsed().as_secs_f32();

        let mut processed = Vec::new();

        for item in render_list {
            let buffer_key = item.buffer;

            if processed.contains(&buffer_key) {
                continue;
            }
            processed.push(buffer_key);

            // Временно извлекаем данные — self свободен для draw_single_buffer
            if let Some(mut shader_data) = self.buffer_shaders.remove(&buffer_key) {
                let padding = shader_data.padding as i32;
                let shaders = shader_data.shaders.clone();

                // Pass 0: Отрисовка оригинального буфера в текстуру 0
                {
                    let mut texture_d = rl.begin_texture_mode(thread, &mut shader_data.textures[0]);
                    texture_d.clear_background(Color::BLANK);
                    self.draw_single_buffer(&mut texture_d, grids, buffer_key, padding, padding, 1.0, true);
                }

                let mut read_idx: usize = 0;
                let mut write_idx: usize = 1;

                // Ping-pong по цепочке шейдеров
                for shader_key in shaders {
                    let tex_w = shader_data.textures[0].texture().width as f32;
                    let tex_h = shader_data.textures[0].texture().height as f32;

                    if let Some(shader_obj) = grids.assets.shaders.get_mut(shader_key) {
                        shader_obj.apply_uniforms();
                        shader_obj.apply_auto_uniforms(
                            (tex_w, tex_h),
                            self.current_time,
                            (tex_w, tex_h),
                        );

                        // split_at_mut даёт безопасный одновременный доступ к двум элементам
                        let (left, right) = shader_data.textures.split_at_mut(1);
                        let (read_tex, write_rt) = if read_idx == 0 {
                            (&left[0], &mut right[0])
                        } else {
                            (&right[0], &mut left[0])
                        };

                        let mut texture_d = rl.begin_texture_mode(thread, write_rt);
                        texture_d.clear_background(Color::BLANK);

                        {
                            // NoBlendGuard: шейдер заменяет пиксели, а не смешивает
                            // Drop order: shader_mode первый, затем _no_blend
                            let _no_blend = NoBlendGuard::new();
                            let mut shader_mode = texture_d.begin_shader_mode(&mut shader_obj.shader);
                            shader_mode.draw_texture_rec(
                                read_tex.texture(),
                                Rectangle::new(0.0, tex_h, tex_w, -tex_h),
                                Vector2::new(0.0, 0.0),
                                Color::WHITE,
                            );
                        }
                    }
                    std::mem::swap(&mut read_idx, &mut write_idx);
                }

                shader_data.final_texture_idx = read_idx;

                // Возвращаем данные на место
                self.buffer_shaders.insert(buffer_key, shader_data);
            }
        }
    }

    pub fn draw(
        &mut self,
        d: &mut RaylibDrawHandle,
        grids: &mut Grids,
        render_list: &[RenderItem],
    ) {
        for item in render_list {
            if self.buffer_shaders.contains_key(&item.buffer) {
                self.draw_buffer_with_shader(d, grids, item.buffer, item.screen_x, item.screen_y);
            } else {
                self.draw_single_buffer(d, grids, item.buffer, item.screen_x, item.screen_y, item.opacity, false);
            }
        }
    }

    pub fn prepare_post_process(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, width: u32, height: u32) {
        let need_recreate = self.post_process_texture.as_ref()
            .map(|t| t.texture().width != width as i32 || t.texture().height != height as i32)
            .unwrap_or(true);

        if need_recreate {
            if let Ok(rt) = rl.load_render_texture(thread, width, height) {
                self.post_process_texture = Some(rt);
            }
        }
    }

    pub fn end_post_process(&mut self, d: &mut RaylibDrawHandle, grids: &mut Grids) {
        if let (Some(shader_key), Some(ref rt)) = (grids.post_process_shader, &self.post_process_texture) {
            self.current_time = self.start_time.elapsed().as_secs_f32();

            let tex_w = rt.texture().width as f32;
            let tex_h = rt.texture().height as f32;
            let screen_w = d.get_screen_width() as f32;
            let screen_h = d.get_screen_height() as f32;

            if let Some(shader_data) = grids.assets.shaders.get_mut(shader_key) {
                shader_data.apply_uniforms();
                shader_data.apply_auto_uniforms((tex_w, tex_h), self.current_time, (screen_w, screen_h));

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

    fn draw_buffer_with_shader(&mut self, d: &mut RaylibDrawHandle, _grids: &mut Grids, buffer: BufferKey, screen_x: i32, screen_y: i32) {
        if let Some(shader_data) = self.buffer_shaders.get(&buffer) {
            let padding = shader_data.padding as i32;
            let final_idx = shader_data.final_texture_idx;
            let tex_w = shader_data.textures[final_idx].texture().width as f32;
            let tex_h = shader_data.textures[final_idx].texture().height as f32;

            let rt = &shader_data.textures[final_idx];

            d.draw_texture_rec(
                rt.texture(),
                Rectangle::new(0.0, tex_h, tex_w, -tex_h),
                Vector2::new((screen_x - padding) as f32, (screen_y - padding) as f32),
                Color::WHITE,
            );
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
        let (is_dirty, glyphset_key, buf_w, buf_h, is_dynamic) = if let Some(buf) = grids.buffers.get(buffer_key) {
            if !buf.visible { return; }
            (buf.dirty || force_rebuild, buf.glyphset(), buf.w, buf.h, buf.dynamic)
        } else {
            return;
        };

        if is_dynamic {
            self.draw_immediate(grids, buffer_key, glyphset_key, buf_w, buf_h, opacity, screen_x, screen_y);
            return;
        }

        if self.default_material.is_none() {
            self.default_material = Some(load_default_material());
        }

        if is_dirty || !self.buffer_batches.contains_key(&buffer_key) {
            self.rebuild_buffer_batch(grids, buffer_key, glyphset_key, buf_w, buf_h);
            if let Some(buf) = grids.buffers.get_mut(buffer_key) {
                buf.dirty = false;
            }
        }

        if let Some(batches) = self.buffer_batches.get(&buffer_key) {
            if let Some(material) = &mut self.default_material {
                set_material_color(material, Color::WHITE.alpha(opacity));
                let transform = Matrix::translate(screen_x as f32, screen_y as f32, 0.0);

                for batch in batches {
                    set_material_texture(material, batch.texture_id);
                    let _blend = BlendGuard::new(batch.blend);
                    batch.mesh.draw(material, transform);
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
        let buffer = match grids.buffers.get(buffer_key) { Some(b) => b, None => return, };
        let glyphset = match grids.assets.glyphsets.get(glyphset_key) { Some(g) => g, None => return, };

        let tile_w = glyphset.tile_w as f32;
        let tile_h = glyphset.tile_h as f32;
        let effective_opacity = opacity * buffer.opacity;

        // BG pass
        {
            let batch = QuadBatch::begin(default_texture_id());
            for y in 0..h {
                for x in 0..w {
                    if let Some(ch) = buffer.get_char_ref(x, y) {
                        let bg_alpha = (ch.bcolor.a as f32 * effective_opacity) as u8;
                        if bg_alpha > 0 {
                            let dst_x = screen_x as f32 + (x as f32 * tile_w);
                            let dst_y = screen_y as f32 + (y as f32 * tile_h);
                            batch.color_quad(dst_x, dst_y, tile_w, tile_h,
                                ch.bcolor.r, ch.bcolor.g, ch.bcolor.b, bg_alpha);
                        }
                    }
                }
            }
        } // batch drop → rlEnd

        // FG pass
        let mut current_tex: u32 = 0;
        let mut current_blend = Blend::Alpha;
        // Drop order (reverse of declaration): batch → blend_guard
        let mut _blend_guard = Some(BlendGuard::new(current_blend));
        let mut batch: Option<QuadBatch> = None;

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
                    // TODO: remove — temporary debug trace for Unicode glyph rendering
                    if ch.code == 0xF8 {
                        static LOGGED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
                        if !LOGGED.swap(true, std::sync::atomic::Ordering::Relaxed) {
                            eprintln!("[DEBUG render] ø: code={}, variant={}, global_id={}, physical_glyph={}", ch.code, ch.variant_id, global_id, physical_glyph);
                        }
                    }
                    let atlas = &grids.assets.atlases[atlas_key];
                    let (src, _, _) = atlas.get_glyph_source(physical_glyph);
                    let tex_id = atlas.texture.id;

                    if tex_id != current_tex || ch.fg_blend != current_blend {
                        batch = None; // drop → rlEnd (перед сменой blend)
                        if ch.fg_blend != current_blend {
                            _blend_guard = None; // drop → EndBlendMode
                            _blend_guard = Some(BlendGuard::new(ch.fg_blend));
                            current_blend = ch.fg_blend;
                        }
                        current_tex = tex_id;
                    }

                    let b = batch.get_or_insert_with(|| QuadBatch::begin(current_tex));

                    let dst_x = screen_x as f32 + (x as f32 * tile_w);
                    let dst_y = screen_y as f32 + (y as f32 * tile_h);
                    let (tex_w, tex_h) = atlas.texture_size();
                    let mut u_min = src.x / tex_w;
                    let mut v_min = src.y / tex_h;
                    let mut u_max = (src.x + src.width) / tex_w;
                    let mut v_max = (src.y + src.height) / tex_h;

                    if ch.transform.flip_h { std::mem::swap(&mut u_min, &mut u_max); }
                    if ch.transform.flip_v { std::mem::swap(&mut v_min, &mut v_max); }

                    b.textured_quad(
                        dst_x, dst_y, tile_w, tile_h,
                        u_min, v_min, u_max, v_max,
                        ch.fcolor.r, ch.fcolor.g, ch.fcolor.b, fg_alpha,
                    );
                }
            }
        }
        // batch drop → rlEnd, затем blend_guard drop → EndBlendMode
    }

    fn rebuild_buffer_batch(
        &mut self,
        grids: &Grids,
        buffer_key: BufferKey,
        glyphset_key: GlyphsetKey,
        w: u32,
        h: u32,
    ) {
        let buffer = match grids.buffers.get(buffer_key) { Some(b) => b, None => return, };
        let glyphset = match grids.assets.glyphsets.get(glyphset_key) { Some(g) => g, None => return, };

        let tile_w = glyphset.tile_w as f32;
        let tile_h = glyphset.tile_h as f32;

        let batches = self.buffer_batches.entry(buffer_key).or_default();
        let mut batch_idx = 0;

        // BG batch
        if batch_idx >= batches.len() { batches.push(Batch::new()); }
        let bg_batch = &mut batches[batch_idx];

        bg_batch.texture_id = default_texture_id();
        bg_batch.blend = Blend::Alpha;
        bg_batch.mesh.clear();

        for y in 0..h {
            for x in 0..w {
                if let Some(ch) = buffer.get_char_ref(x, y) {
                    if ch.bcolor.a > 0 {
                        let dst_x = x as f32 * tile_w;
                        let dst_y = y as f32 * tile_h;

                        bg_batch.mesh.push_quad(
                            [(dst_x, dst_y), (dst_x, dst_y + tile_h), (dst_x + tile_w, dst_y + tile_h), (dst_x + tile_w, dst_y)],
                            [(0.0, 0.0); 4],
                            [ch.bcolor.r, ch.bcolor.g, ch.bcolor.b, ch.bcolor.a],
                        );
                    }
                }
            }
        }

        if !bg_batch.mesh.is_empty() { batch_idx += 1; }

        // FG batches
        let mut current_tex: u32 = 0;
        let mut current_blend = Blend::Alpha;

        for y in 0..h {
            for x in 0..w {
                if let Some(ch) = buffer.get_char_ref(x, y) {
                    if ch.fcolor.a == 0 { continue; }

                    let global_id = glyphset.luts.get(ch.variant_id as usize)
                        .and_then(|lut: &Vec<u32>| lut.get(ch.code as usize))
                        .copied()
                        .unwrap_or(glyphset.default_global_id);

                    let (atlas_key, physical_glyph) = grids.assets.global_registry.entries[global_id as usize];
                    let atlas = &grids.assets.atlases[atlas_key];
                    let (src, _, _) = atlas.get_glyph_source(physical_glyph);
                    let tex_id = atlas.texture.id;

                    if tex_id != current_tex || ch.fg_blend != current_blend {
                        if batch_idx < batches.len() && !batches[batch_idx].mesh.is_empty() {
                            batch_idx += 1;
                        }
                        if batch_idx >= batches.len() { batches.push(Batch::new()); }

                        let batch = &mut batches[batch_idx];
                        batch.texture_id = tex_id;
                        batch.blend = ch.fg_blend;
                        batch.mesh.clear();

                        current_tex = tex_id;
                        current_blend = ch.fg_blend;
                    }

                    let batch = &mut batches[batch_idx];
                    let dst_x = x as f32 * tile_w;
                    let dst_y = y as f32 * tile_h;

                    let (tex_w, tex_h) = atlas.texture_size();
                    let mut u_min = src.x / tex_w;
                    let mut v_min = src.y / tex_h;
                    let mut u_max = (src.x + src.width) / tex_w;
                    let mut v_max = (src.y + src.height) / tex_h;

                    if ch.transform.flip_h { std::mem::swap(&mut u_min, &mut u_max); }
                    if ch.transform.flip_v { std::mem::swap(&mut v_min, &mut v_max); }

                    let mut v_tl = (0.0f32, 0.0f32);
                    let mut v_bl = (0.0, tile_h);
                    let mut v_br = (tile_w, tile_h);
                    let mut v_tr = (tile_w, 0.0);

                    if ch.transform.rotation != Rotation::None {
                        let cx = tile_w * 0.5;
                        let cy = tile_h * 0.5;
                        let rot_rad = ch.transform.rotation.degrees().to_radians();
                        let cos_r = rot_rad.cos();
                        let sin_r = rot_rad.sin();

                        let rotate = |(vx, vy): (f32, f32)| -> (f32, f32) {
                            let dx = vx - cx;
                            let dy = vy - cy;
                            (cx + dx * cos_r - dy * sin_r, cy + dx * sin_r + dy * cos_r)
                        };

                        v_tl = rotate(v_tl);
                        v_bl = rotate(v_bl);
                        v_br = rotate(v_br);
                        v_tr = rotate(v_tr);
                    }

                    batch.mesh.push_quad(
                        [(dst_x + v_tl.0, dst_y + v_tl.1), (dst_x + v_bl.0, dst_y + v_bl.1),
                         (dst_x + v_br.0, dst_y + v_br.1), (dst_x + v_tr.0, dst_y + v_tr.1)],
                        [(u_min, v_min), (u_min, v_max), (u_max, v_max), (u_max, v_min)],
                        [ch.fcolor.r, ch.fcolor.g, ch.fcolor.b, ch.fcolor.a],
                    );
                }
            }
        }

        if batch_idx < batches.len() && !batches[batch_idx].mesh.is_empty() {
            batch_idx += 1;
        }

        batches.truncate(batch_idx);

        for batch in batches.iter_mut() {
            batch.mesh.upload();
        }
    }
}
