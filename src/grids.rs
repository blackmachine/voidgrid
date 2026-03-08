//! Grids - система управления текстовыми буферами, атласами и палитрами
//!
//! Версия 4.6:
//! - Post-process шейдеры (глобальные)
//! - Buffer шейдеры с padding
//! - Shader uniforms (float, vec2, vec3, vec4)
//! - Auto-uniforms: texSize, time, resolution
//! - Drag-n-drop поддержка
//!
//! Версия 4.5:
//! - Glyph alternatives
//! - Z-index сортировка детей
//!
//! Версия 4.3-4.4:
//! - Opacity и visibility буфера
//! - Transform (rotation + flip) для символа
//! - Направление записи строки
//! - Палитры с именованными цветами
//! - Attachments вынесены в отдельную структуру

use crate::types::*;
use crate::buffer::Buffer;
use crate::asset_manager::AssetManager;
use crate::buffer_manager::{BufferManager, BufferBuilder};

// ============================================================================
// Grids - главная структура
// ============================================================================

/// Главная структура управления буферами, атласами и палитрами
pub struct Grids {
    pub buffers: BufferManager,
    pub assets: AssetManager,
    
    // Post-process
    pub(crate) post_process_shader: Option<ShaderKey>,
}

impl Grids {
    /// Создать новый экземпляр
    pub fn new() -> Self {
        Self {
            buffers: BufferManager::new(),
            assets: AssetManager::new(),
            post_process_shader: None,
        }
    }
    
    // ========================================================================
    // Шейдеры
    // ========================================================================
    
    /// Установить post-process шейдер
    pub fn set_post_process(&mut self, shader: ShaderKey) {
        self.post_process_shader = Some(shader);
    }
    
    /// Убрать post-process шейдер
    pub fn clear_post_process(&mut self) {
        self.post_process_shader = None;
    }
    
    // ========================================================================
    // Палитры
    // ========================================================================
    
    // ========================================================================
    // Атласы
    // ========================================================================
    
    // ========================================================================
    // Glyphsets
    // ========================================================================
    // ========================================================================
    // Буферы
    // ========================================================================
    
    /// Создать новый буфер (Proxy)
    pub fn create_buffer(
        &mut self,
        name: impl Into<String>,
        w: u32,
        h: u32,
        glyphset: GlyphsetKey,
    ) -> BufferKey {
        self.buffers.create_buffer(name, w, h, glyphset)
    }

    /// Начать создание буфера через Builder (Proxy)
    pub fn buffer(&mut self, name: impl Into<String>, w: u32, h: u32, glyphset: GlyphsetKey) -> BufferBuilder<'_> {
        self.buffers.buffer(name, w, h, glyphset)
    }
    
    /// Создать буфер с заданным z_index (Proxy)
    pub fn create_buffer_z(
        &mut self,
        name: impl Into<String>,
        w: u32,
        h: u32,
        glyphset: GlyphsetKey,
        z_index: i32,
    ) -> BufferKey {
        self.buffers.create_buffer_z(name, w, h, glyphset, z_index)
    }
    
    /// Получить буфер (immutable) (Proxy)
    pub fn get(&self, key: BufferKey) -> Option<&Buffer> {
        self.buffers.get(key)
    }
    
    /// Получить буфер (mutable) (Proxy)
    pub fn get_mut(&mut self, key: BufferKey) -> Option<&mut Buffer> {
        self.buffers.get_mut(key)
    }
    
    /// Удалить буфер (Proxy)
    pub fn remove_buffer(&mut self, key: BufferKey) -> Option<Buffer> {
        self.buffers.remove_buffer(key)
    }
    
    /// Установить видимость буфера (Proxy)
    pub fn set_visible(&mut self, key: BufferKey, visible: bool) {
        self.buffers.set_visible(key, visible);
    }
    
    /// Установить прозрачность буфера (Proxy)
    pub fn set_opacity(&mut self, key: BufferKey, opacity: f32) {
        self.buffers.set_opacity(key, opacity);
    }
    
    /// Установить вариант по умолчанию для буфера (Proxy)
    pub fn set_buffer_variant(&mut self, key: BufferKey, variant: Option<impl Into<String>>) {
        self.buffers.set_buffer_variant(&self.assets, key, variant);
    }
    
    /// Установить режим динамического обновления (Proxy)
    pub fn set_buffer_dynamic(&mut self, key: BufferKey, dynamic: bool) {
        self.buffers.set_buffer_dynamic(key, dynamic);
    }

    /// Установить z_index буфера (Proxy)
    pub fn set_buffer_z(&mut self, key: BufferKey, z_index: i32) {
        self.buffers.set_buffer_z(key, z_index);
    }
    
    /// Изменить размер буфера (Proxy)
    pub fn resize_buffer(&mut self, buffer: BufferKey, new_w: u32, new_h: u32) {
        self.buffers.resize_buffer(buffer, new_w, new_h);
    }
    
    /// Получить размер буфера (Proxy)
    pub fn buffer_size(&self, buffer: BufferKey) -> Option<(u32, u32)> {
        self.buffers.buffer_size(buffer)
    }
    
    /// Установить символ в буфер (Proxy)
    pub fn set_char(&mut self, buffer: BufferKey, x: u32, y: u32, ch: Character) {
        self.buffers.set_char(buffer, x, y, ch);
    }
    
    /// Установить трансформацию для символа (Proxy)
    pub fn set_char_transform(&mut self, buffer: BufferKey, x: u32, y: u32, transform: Transform) {
        self.buffers.set_char_transform(buffer, x, y, transform);
    }
    
    /// Установить маску для символа (Proxy)
    pub fn set_char_mask(&mut self, buffer: BufferKey, x: u32, y: u32, mask: Option<Mask>) {
        self.buffers.set_char_mask(buffer, x, y, mask);
    }
    
    /// Очистить буфер (Proxy)
    pub fn clear_buffer(&mut self, buffer: BufferKey) {
        self.buffers.clear_buffer(buffer);
    }
    
    // ========================================================================
    // Hit Testing
    // ========================================================================
    
    /// Преобразовать экранные координаты в ячейку буфера
    pub fn screen_to_cell(
        &self,
        buffer: BufferKey,
        screen_x: i32,
        screen_y: i32,
        buf_screen_x: i32,
        buf_screen_y: i32,
    ) -> Option<(u32, u32)> {
        let buf = self.buffers.get(buffer)?;
        let gs = self.assets.glyphsets.get(buf.glyphset)?;
        
        let tile_w = gs.tile_w as i32;
        let tile_h = gs.tile_h as i32;
        
        let local_x = screen_x - buf_screen_x;
        let local_y = screen_y - buf_screen_y;
        
        if local_x < 0 || local_y < 0 {
            return None;
        }
        
        let cx = (local_x / tile_w) as u32;
        let cy = (local_y / tile_h) as u32;
        
        if cx < buf.w && cy < buf.h {
            Some((cx, cy))
        } else {
            None
        }
    }
}

impl Default for Grids {
    fn default() -> Self {
        Self::new()
    }
}
