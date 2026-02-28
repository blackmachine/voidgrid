use std::collections::HashMap;
use std::time::Instant;

use raylib::prelude::*;
use raylib::ffi::{
    BeginBlendMode, EndBlendMode, DrawTexturePro,
    Vector2 as FfiVector2, Rectangle as FfiRectangle,
};

use crate::grids::Grids;
use crate::types::{BufferKey, ShaderKey, Blend};

/// Максимальная глубина вложенности буферов
const MAX_BUFFER_DEPTH: u8 = 8;

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
    pub fn set_buffer_shader_padded(
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
            self.set_buffer_shader_padded(rl, thread, grids, buffer, shader, padding);
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
        grids: &Grids,
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
                self.draw_single_buffer(&mut texture_d, grids, *buffer_key, padding, padding, 1.0);
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
        &self,
        d: &mut D,
        grids: &Grids,
        buffer_key: BufferKey,
        screen_x: i32,
        screen_y: i32,
        opacity: f32,
    ) {
        let buffer = match grids.buffers.get(buffer_key) {
            Some(b) => b,
            None => return,
        };
        
        if !buffer.visible { return; }
        
        let glyphset = match grids.glyphsets.get(buffer.glyphset()) {
            Some(g) => g,
            None => return,
        };
        
        let effective_opacity = opacity * buffer.opacity;
        let tile_w = glyphset.tile_w as f32;
        let tile_h = glyphset.tile_h as f32;
        
        let mut current_blend: Option<Blend> = None;
        
        for y in 0..buffer.h {
            for x in 0..buffer.w {
                if let Some(ch) = buffer.get_char_ref(x, y) {
                    let dst_x = screen_x as f32 + (x as f32 * tile_w);
                    let dst_y = screen_y as f32 + (y as f32 * tile_h);
                    
                    let fg_with_opacity = Color::new(ch.fcolor.r, ch.fcolor.g, ch.fcolor.b, (ch.fcolor.a as f32 * effective_opacity) as u8);
                    let bg_with_opacity = Color::new(ch.bcolor.r, ch.bcolor.g, ch.bcolor.b, (ch.bcolor.a as f32 * effective_opacity) as u8);
                    
                    // Resolve global_id from LUT
                    let global_id = glyphset.luts.get(ch.variant_id as usize)
                        .and_then(|lut: &Vec<u32>| lut.get(ch.code as usize))
                        .copied()
                        .unwrap_or(glyphset.default_global_id);

                    // Get physical glyph from registry
                    let (atlas_key, physical_glyph) = grids.global_registry.entries[global_id as usize];
                    let atlas = &grids.atlases[atlas_key];
                    let (src, _, _) = atlas.get_glyph_source(physical_glyph);

                    let transformed_src = ch.transform.apply_to_src(src);
                    let rotation = ch.transform.rotation.degrees();
                    
                    let origin = if rotation != 0.0 { Vector2::new(tile_w / 2.0, tile_h / 2.0) } else { Vector2::zero() };
                    let (final_dst_x, final_dst_y) = if rotation != 0.0 { (dst_x + tile_w / 2.0, dst_y + tile_h / 2.0) } else { (dst_x, dst_y) };
                    
                    // Фон
                    if bg_with_opacity.a > 0 {
                        if current_blend != Some(ch.bg_blend) {
                            if current_blend.is_some() { unsafe { EndBlendMode(); } }
                            unsafe { BeginBlendMode(ch.bg_blend.to_ffi()); }
                            current_blend = Some(ch.bg_blend);
                        }
                        d.draw_rectangle(dst_x as i32, dst_y as i32, tile_w as i32, tile_h as i32, bg_with_opacity);
                    }
                    
                    // Глиф
                    if fg_with_opacity.a > 0 {
                        if current_blend != Some(ch.fg_blend) {
                            if current_blend.is_some() { unsafe { EndBlendMode(); } }
                            unsafe { BeginBlendMode(ch.fg_blend.to_ffi()); }
                            current_blend = Some(ch.fg_blend);
                        }
                        
                        unsafe {
                            DrawTexturePro(
                                *atlas.texture.as_ref(),
                                transformed_src.into(),
                                FfiRectangle { x: final_dst_x, y: final_dst_y, width: tile_w, height: tile_h },
                                FfiVector2 { x: origin.x, y: origin.y },
                                rotation,
                                std::mem::transmute::<Color, raylib::ffi::Color>(fg_with_opacity),
                            );
                        }
                    }
                }
            }
        }
        
        if current_blend.is_some() { unsafe { EndBlendMode(); } }
    }

    fn draw_internal_skip_shaders<D: RaylibDraw>(
        &self,
        d: &mut D,
        grids: &Grids,
        buffer_key: BufferKey,
        screen_x: i32,
        screen_y: i32,
        parent_opacity: f32,
        depth: u8,
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
        
        let effective_opacity = parent_opacity * buffer.opacity;
        let tile_w = gs.tile_w as f32;
        let tile_h = gs.tile_h as f32;
        
        if !self.buffer_shaders.contains_key(&buffer_key) {
            self.draw_single_buffer(d, grids, buffer_key, screen_x, screen_y, effective_opacity);
        }
        
        let mut children: Vec<_> = grids.attachments.iter()
            .filter(|a| a.parent == buffer_key)
            .collect();
        children.sort_by_key(|a| a.z_index);
        
        for att in children {
            let child_x = screen_x + (att.x as f32 * tile_w) as i32;
            let child_y = screen_y + (att.y as f32 * tile_h) as i32;
            self.draw_internal_skip_shaders(d, grids, att.child, child_x, child_y, effective_opacity, depth - 1);
        }
    }
}