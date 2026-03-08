use slotmap::SlotMap;

use crate::types::*;
use crate::buffer::Buffer;
use crate::asset_manager::AssetManager;

// ============================================================================
// BufferManager
// ============================================================================

pub struct BufferManager {
    pub buffers: SlotMap<BufferKey, Buffer>,
}

impl BufferManager {
    pub fn new() -> Self {
        Self {
            buffers: SlotMap::with_key(),
        }
    }

    /// Создать новый буфер
    pub fn create_buffer(
        &mut self,
        name: impl Into<String>,
        w: u32,
        h: u32,
        glyphset: GlyphsetKey,
    ) -> BufferKey {
        let default_code = 32; // Space
        let fill = Character::blank(default_code);
        let buffer = Buffer::new(name, w, h, glyphset, 0, fill);
        self.buffers.insert(buffer)
    }

    /// Начать создание буфера через Builder
    pub fn buffer(&mut self, name: impl Into<String>, w: u32, h: u32, glyphset: GlyphsetKey) -> BufferBuilder<'_> {
        BufferBuilder::new(self, name, w, h, glyphset)
    }
    
    /// Создать буфер с заданным z_index
    pub fn create_buffer_z(
        &mut self,
        name: impl Into<String>,
        w: u32,
        h: u32,
        glyphset: GlyphsetKey,
        z_index: i32,
    ) -> BufferKey {
        let default_code = 32;
        let fill = Character::blank(default_code);
        let buffer = Buffer::new(name, w, h, glyphset, z_index, fill);
        self.buffers.insert(buffer)
    }
    
    /// Получить буфер (immutable)
    pub fn get(&self, key: BufferKey) -> Option<&Buffer> {
        self.buffers.get(key)
    }
    
    /// Получить буфер (mutable)
    pub fn get_mut(&mut self, key: BufferKey) -> Option<&mut Buffer> {
        self.buffers.get_mut(key)
    }
    
    /// Удалить буфер
    pub fn remove_buffer(&mut self, key: BufferKey) -> Option<Buffer> {
        self.buffers.remove(key)
    }
    
    /// Установить видимость буфера
    pub fn set_visible(&mut self, key: BufferKey, visible: bool) {
        if let Some(buf) = self.buffers.get_mut(key) {
            buf.visible = visible;
        }
    }
    
    /// Установить прозрачность буфера
    pub fn set_opacity(&mut self, key: BufferKey, opacity: f32) {
        if let Some(buf) = self.buffers.get_mut(key) {
            buf.opacity = opacity.clamp(0.0, 1.0);
        }
    }
    
    /// Установить вариант по умолчанию для буфера
    /// Требует AssetManager для разрешения имени варианта
    pub fn set_buffer_variant(&mut self, assets: &AssetManager, key: BufferKey, variant: Option<impl Into<String>>) {
        let variant_id = if let Some(v) = variant {
            let v_str = v.into();
            let gs_key = self.buffers.get(key).map(|b| b.glyphset).unwrap();
            assets.glyphsets.get(gs_key).and_then(|gs| gs.variant_names.get(&v_str)).copied().unwrap_or(0)
        } else { 0 };

        if let Some(buf) = self.buffers.get_mut(key) {
            buf.dirty = true;
            buf.default_variant_id = variant_id;
        }
    }
    
    /// Установить режим динамического обновления (отключает кэширование VBO)
    pub fn set_buffer_dynamic(&mut self, key: BufferKey, dynamic: bool) {
        if let Some(buf) = self.buffers.get_mut(key) {
            buf.dynamic = dynamic;
        }
    }

    /// Установить z_index буфера
    pub fn set_buffer_z(&mut self, key: BufferKey, z_index: i32) {
        if let Some(buf) = self.buffers.get_mut(key) {
            buf.z_index = z_index;
        }
    }
    
    /// Изменить размер буфера, сохраняя содержимое
    pub fn resize_buffer(&mut self, buffer: BufferKey, new_w: u32, new_h: u32) {
        let default_code = 32;
        
        if let Some(buf) = self.buffers.get_mut(buffer) {
            let old_data = std::mem::take(&mut buf.data);
            let old_w = buf.w;
            let old_h = buf.h;
            
            // Новый буфер данных
            let fill = Character::blank(default_code);
            buf.data = vec![fill; (new_w * new_h) as usize];
            buf.w = new_w;
            buf.h = new_h;
            buf.dirty = true;
            
            // Копируем что влезает
            for y in 0..old_h.min(new_h) {
                for x in 0..old_w.min(new_w) {
                    let old_idx = (y * old_w + x) as usize;
                    let new_idx = (y * new_w + x) as usize;
                    buf.data[new_idx] = old_data[old_idx].clone();
                }
            }
        }
    }
    
    /// Получить размер буфера
    pub fn buffer_size(&self, buffer: BufferKey) -> Option<(u32, u32)> {
        self.buffers.get(buffer).map(|b| (b.w, b.h))
    }
    
    // ========================================================================
    // Запись в буфер
    // ========================================================================
    
    /// Установить символ в буфер
    pub fn set_char(&mut self, buffer: BufferKey, x: u32, y: u32, ch: Character) {
        if let Some(buf) = self.buffers.get_mut(buffer) {
            buf.set(x, y, ch);
        }
    }
    
    /// Установить трансформацию для символа
    pub fn set_char_transform(&mut self, buffer: BufferKey, x: u32, y: u32, transform: Transform) {
        if let Some(buf) = self.buffers.get_mut(buffer) {
            if let Some(i) = buf.index(x, y) {
                buf.dirty = true;
                buf.data[i].transform = transform;
            }
        }
    }
    
    /// Установить маску для символа
    pub fn set_char_mask(&mut self, buffer: BufferKey, x: u32, y: u32, mask: Option<Mask>) {
        if let Some(buf) = self.buffers.get_mut(buffer) {
            if let Some(i) = buf.index(x, y) {
                buf.dirty = true;
                buf.data[i].mask = mask;
            }
        }
    }
    
    /// Установить blend mode для символа
    pub fn set_char_blend(&mut self, buffer: BufferKey, x: u32, y: u32, fg_blend: Blend, bg_blend: Blend) {
        if let Some(buf) = self.buffers.get_mut(buffer) {
            if let Some(i) = buf.index(x, y) {
                buf.dirty = true;
                buf.data[i].fg_blend = fg_blend;
                buf.data[i].bg_blend = bg_blend;
            }
        }
    }
    
    /// Установить blend mode для области
    pub fn set_area_blend(&mut self, buffer: BufferKey, x: u32, y: u32, w: u32, h: u32, fg_blend: Blend, bg_blend: Blend) {
        if let Some(buf) = self.buffers.get_mut(buffer) {
            buf.dirty = true;
            for cy in y..y + h {
                for cx in x..x + w {
                    if let Some(i) = buf.index(cx, cy) {
                        buf.data[i].fg_blend = fg_blend;
                        buf.data[i].bg_blend = bg_blend;
                    }
                }
            }
        }
    }
    
    /// Установить маску для области
    pub fn set_area_mask(&mut self, buffer: BufferKey, x: u32, y: u32, w: u32, h: u32, mask: Option<Mask>) {
        if let Some(buf) = self.buffers.get_mut(buffer) {
            buf.dirty = true;
            for cy in y..y + h {
                for cx in x..x + w {
                    if let Some(i) = buf.index(cx, cy) {
                        buf.data[i].mask = mask;
                    }
                }
            }
        }
    }
    
    /// Установить трансформацию для области
    pub fn set_area_transform(&mut self, buffer: BufferKey, x: u32, y: u32, w: u32, h: u32, transform: Transform) {
        if let Some(buf) = self.buffers.get_mut(buffer) {
            buf.dirty = true;
            for cy in y..y + h {
                for cx in x..x + w {
                    if let Some(i) = buf.index(cx, cy) {
                        buf.data[i].transform = transform;
                    }
                }
            }
        }
    }
    
    /// Очистить буфер
    pub fn clear_buffer(&mut self, buffer: BufferKey) {
        let default_code = 32;
        
        if let Some(buf) = self.buffers.get_mut(buffer) {
            buf.clear(Character::blank(default_code));
        }
    }
}

impl Default for BufferManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Buffer Builder
// ============================================================================

pub struct BufferBuilder<'a> {
    manager: &'a mut BufferManager,
    name: String,
    w: u32,
    h: u32,
    glyphset: GlyphsetKey,
    z_index: i32,
    dynamic: bool,
    visible: bool,
    opacity: f32,
}

impl<'a> BufferBuilder<'a> {
    pub fn new(manager: &'a mut BufferManager, name: impl Into<String>, w: u32, h: u32, glyphset: GlyphsetKey) -> Self {
        Self {
            manager,
            name: name.into(),
            w, h, glyphset,
            z_index: 0,
            dynamic: false,
            visible: true,
            opacity: 1.0,
        }
    }

    pub fn z_index(mut self, z: i32) -> Self {
        self.z_index = z;
        self
    }

    pub fn dynamic(mut self, dynamic: bool) -> Self {
        self.dynamic = dynamic;
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }

    pub fn build(self) -> BufferKey {
        let key = self.manager.create_buffer_z(&self.name, self.w, self.h, self.glyphset, self.z_index);
        self.manager.set_buffer_dynamic(key, self.dynamic);
        self.manager.set_visible(key, self.visible);
        self.manager.set_opacity(key, self.opacity);
        
        key
    }
}