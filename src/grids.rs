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

use raylib::prelude::*;
use slotmap::SlotMap;

use crate::types::*;
use crate::types::Transform;
use crate::atlas::Atlas;
use crate::buffer::{Buffer, Attachment, OrphanedChild};
use crate::palette::Palette;
use crate::shader::{ShaderData, UniformValue};
use crate::assets::{self, AssetCache};

// ============================================================================
// Grids - главная структура
// ============================================================================

/// Максимальная глубина вложенности буферов
const MAX_BUFFER_DEPTH: u8 = 8;

/// Главная структура управления буферами, атласами и палитрами
pub struct Grids {
    pub(crate) atlases: SlotMap<AtlasKey, Atlas>,
    pub(crate) buffers: SlotMap<BufferKey, Buffer>,
    palettes: SlotMap<PaletteKey, Palette>,
    pub(crate) shaders: SlotMap<ShaderKey, ShaderData>,
    /// Связи родитель-потомок (вынесены из Buffer)
    pub(crate) attachments: Vec<Attachment>,
    
    // Post-process
    pub(crate) post_process_shader: Option<ShaderKey>,
    
    // Resource Cache
    cache: AssetCache,
}

impl Grids {
    /// Создать новый экземпляр
    pub fn new() -> Self {
        Self {
            atlases: SlotMap::with_key(),
            buffers: SlotMap::with_key(),
            palettes: SlotMap::with_key(),
            shaders: SlotMap::with_key(),
            attachments: Vec::new(),
            post_process_shader: None,
            cache: AssetCache::new(),
        }
    }
    
    // ========================================================================
    // Шейдеры
    // ========================================================================
    
    /// Загрузить шейдер из файла
    pub fn load_shader(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, fragment_path: &str) -> Result<ShaderKey, String> {
        if let Some(&key) = self.cache.shaders.get(fragment_path) {
            return Ok(key);
        }
        
        let shader_data = assets::load_shader_from_file(rl, thread, fragment_path)?;
        let key = self.shaders.insert(shader_data);
        self.cache.shaders.insert(fragment_path.to_string(), key);
        Ok(key)
    }
    
    /// Получить шейдер
    pub fn shader(&self, key: ShaderKey) -> Option<&ShaderData> {
        self.shaders.get(key)
    }
    
    /// Получить шейдер (mutable)
    pub fn shader_mut(&mut self, key: ShaderKey) -> Option<&mut ShaderData> {
        self.shaders.get_mut(key)
    }
    
    /// Установить float uniform
    pub fn set_shader_float(&mut self, key: ShaderKey, name: &str, value: f32) {
        if let Some(shader) = self.shaders.get_mut(key) {
            shader.set_uniform(name, UniformValue::Float(value));
        }
    }
    
    /// Установить vec2 uniform
    pub fn set_shader_vec2(&mut self, key: ShaderKey, name: &str, value: (f32, f32)) {
        if let Some(shader) = self.shaders.get_mut(key) {
            shader.set_uniform(name, UniformValue::Vec2(value.0, value.1));
        }
    }
    
    /// Установить vec3 uniform
    pub fn set_shader_vec3(&mut self, key: ShaderKey, name: &str, value: (f32, f32, f32)) {
        if let Some(shader) = self.shaders.get_mut(key) {
            shader.set_uniform(name, UniformValue::Vec3(value.0, value.1, value.2));
        }
    }
    
    /// Установить vec4 uniform
    pub fn set_shader_vec4(&mut self, key: ShaderKey, name: &str, value: (f32, f32, f32, f32)) {
        if let Some(shader) = self.shaders.get_mut(key) {
            shader.set_uniform(name, UniformValue::Vec4(value.0, value.1, value.2, value.3));
        }
    }
    
    /// Установить int uniform
    pub fn set_shader_int(&mut self, key: ShaderKey, name: &str, value: i32) {
        if let Some(shader) = self.shaders.get_mut(key) {
            shader.set_uniform(name, UniformValue::Int(value));
        }
    }
    
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
    
    /// Создать новую палитру
    pub fn create_palette(&mut self, name: impl Into<String>) -> PaletteKey {
        self.palettes.insert(Palette::new(name))
    }
    
    /// Загрузить палитру из JSON
    pub fn load_palette(&mut self, path: &str) -> Result<PaletteKey, Box<dyn std::error::Error>> {
        if let Some(&key) = self.cache.palettes.get(path) {
            return Ok(key);
        }
        
        let palette = assets::load_palette_from_file(path)?;
        let key = self.palettes.insert(palette);
        self.cache.palettes.insert(path.to_string(), key);
        Ok(key)
    }
    
    /// Получить палитру
    pub fn palette(&self, key: PaletteKey) -> Option<&Palette> {
        self.palettes.get(key)
    }
    
    /// Получить палитру (mutable)
    pub fn palette_mut(&mut self, key: PaletteKey) -> Option<&mut Palette> {
        self.palettes.get_mut(key)
    }
    
    /// Разрешить ColorRef в Color
    pub fn resolve_color(&self, color_ref: ColorRef) -> Color {
        match color_ref {
            ColorRef::Direct(c) => c,
            ColorRef::Indexed { palette, index } | ColorRef::Named { palette, index } => {
                self.palettes.get(palette)
                    .and_then(|p| p.get(index as usize))
                    .unwrap_or(Color::MAGENTA) // Ошибка — ярко видно
            }
        }
    }
    
    // ========================================================================
    // Атласы
    // ========================================================================
    
    /// Загрузить атлас из JSON-файла
    pub fn load_atlas(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, config_path: &str) -> Result<AtlasKey, Box<dyn std::error::Error>> {
        if let Some(&key) = self.cache.atlases.get(config_path) {
            return Ok(key);
        }
        
        let atlas = assets::load_atlas_from_file(rl, thread, config_path)?;
        let key = self.atlases.insert(atlas);
        self.cache.atlases.insert(config_path.to_string(), key);
        Ok(key)
    }
    
    /// Получить атлас по ключу
    pub fn atlas(&self, key: AtlasKey) -> Option<&Atlas> {
        self.atlases.get(key)
    }
    
    /// Получить размер тайла атласа
    pub fn tile_size(&self, key: AtlasKey) -> Option<(u32, u32)> {
        self.atlases.get(key).map(|a| (a.config.tile_width, a.config.tile_height))
    }
    
    /// Получить default_glyph атласа
    pub fn default_glyph(&self, key: AtlasKey) -> u32 {
        self.atlases.get(key).map(|a| a.config.default_glyph).unwrap_or(0)
    }
    
    // ========================================================================
    // Буферы
    // ========================================================================
    
    /// Создать новый буфер
    pub fn create_buffer(
        &mut self,
        name: impl Into<String>,
        w: u32,
        h: u32,
        atlas: AtlasKey,
    ) -> BufferKey {
        let default_glyph = self.default_glyph(atlas);
        let fill = Character::blank(default_glyph);
        let buffer = Buffer::new(name, w, h, atlas, 0, fill);
        self.buffers.insert(buffer)
    }
    
    /// Создать буфер с заданным z_index
    pub fn create_buffer_z(
        &mut self,
        name: impl Into<String>,
        w: u32,
        h: u32,
        atlas: AtlasKey,
        z_index: i32,
    ) -> BufferKey {
        let default_glyph = self.default_glyph(atlas);
        let fill = Character::blank(default_glyph);
        let buffer = Buffer::new(name, w, h, atlas, z_index, fill);
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
        // Удаляем все связи с этим буфером
        self.attachments.retain(|a| a.parent != key && a.child != key);
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
    pub fn set_buffer_variant(&mut self, key: BufferKey, variant: Option<impl Into<String>>) {
        if let Some(buf) = self.buffers.get_mut(key) {
            buf.default_variant = variant.map(|v| v.into());
        }
    }
    
    /// Установить z_index буфера
    pub fn set_buffer_z(&mut self, key: BufferKey, z_index: i32) {
        if let Some(buf) = self.buffers.get_mut(key) {
            buf.z_index = z_index;
        }
    }
    
    /// Изменить размер буфера, сохраняя содержимое
    /// Возвращает список потомков, оказавшихся за пределами нового размера
    pub fn resize_buffer(&mut self, buffer: BufferKey, new_w: u32, new_h: u32) -> Vec<OrphanedChild> {
        let default_glyph = self.buffers.get(buffer)
            .and_then(|b| self.atlases.get(b.atlas))
            .map(|a| a.config.default_glyph)
            .unwrap_or(0);
        
        let mut orphaned = Vec::new();
        
        if let Some(buf) = self.buffers.get_mut(buffer) {
            let old_data = std::mem::take(&mut buf.data);
            let old_w = buf.w;
            let old_h = buf.h;
            
            // Новый буфер данных
            let fill = Character::blank(default_glyph);
            buf.data = vec![fill; (new_w * new_h) as usize];
            buf.w = new_w;
            buf.h = new_h;
            
            // Копируем что влезает
            for y in 0..old_h.min(new_h) {
                for x in 0..old_w.min(new_w) {
                    let old_idx = (y * old_w + x) as usize;
                    let new_idx = (y * new_w + x) as usize;
                    buf.data[new_idx] = old_data[old_idx].clone();
                }
            }
        }
        
        // Находим осиротевших потомков
        for att in &self.attachments {
            if att.parent == buffer && (att.x >= new_w || att.y >= new_h) {
                orphaned.push(OrphanedChild {
                    position: (att.x, att.y),
                    buffer: att.child,
                });
            }
        }
        
        orphaned
    }
    
    /// Получить размер буфера
    pub fn buffer_size(&self, buffer: BufferKey) -> Option<(u32, u32)> {
        self.buffers.get(buffer).map(|b| (b.w, b.h))
    }
    
    // ========================================================================
    // Связи родитель-потомок
    // ========================================================================
    
    /// Привязать дочерний буфер к ячейке родителя
    pub fn attach(&mut self, parent: BufferKey, pos: (u32, u32), child: BufferKey) {
        self.attach_z(parent, pos, child, 0);
    }
    
    /// Привязать дочерний буфер с заданным z_index
    pub fn attach_z(&mut self, parent: BufferKey, pos: (u32, u32), child: BufferKey, z_index: i32) {
        // Проверяем, нет ли уже такой связи
        let exists = self.attachments.iter().any(|a| 
            a.parent == parent && a.child == child && a.x == pos.0 && a.y == pos.1
        );
        
        if !exists {
            self.attachments.push(Attachment {
                parent,
                child,
                x: pos.0,
                y: pos.1,
                z_index,
            });
            // Сортируем по z_index для правильного порядка отрисовки
            self.attachments.sort_by_key(|a| a.z_index);
        }
    }
    
    /// Отвязать конкретный дочерний буфер от ячейки
    pub fn detach(&mut self, parent: BufferKey, pos: (u32, u32), child: BufferKey) {
        self.attachments.retain(|a| 
            !(a.parent == parent && a.child == child && a.x == pos.0 && a.y == pos.1)
        );
    }
    
    /// Отвязать все дочерние буферы от ячейки
    pub fn detach_all_at(&mut self, parent: BufferKey, pos: (u32, u32)) {
        self.attachments.retain(|a| 
            !(a.parent == parent && a.x == pos.0 && a.y == pos.1)
        );
    }
    
    /// Отвязать все дочерние буферы от родителя
    pub fn detach_all_children(&mut self, parent: BufferKey) {
        self.attachments.retain(|a| a.parent != parent);
    }
    
    /// Переместить дочерний буфер в другую ячейку
    pub fn move_child(&mut self, parent: BufferKey, child: BufferKey, new_pos: (u32, u32)) {
        for att in &mut self.attachments {
            if att.parent == parent && att.child == child {
                att.x = new_pos.0;
                att.y = new_pos.1;
                return;
            }
        }
    }
    
    /// Получить список дочерних буферов в ячейке
    pub fn children_at(&self, parent: BufferKey, pos: (u32, u32)) -> Vec<BufferKey> {
        self.attachments.iter()
            .filter(|a| a.parent == parent && a.x == pos.0 && a.y == pos.1)
            .map(|a| a.child)
            .collect()
    }
    
    /// Получить все связи родителя
    pub fn all_children(&self, parent: BufferKey) -> Vec<((u32, u32), BufferKey)> {
        self.attachments.iter()
            .filter(|a| a.parent == parent)
            .map(|a| ((a.x, a.y), a.child))
            .collect()
    }
    
    /// Найти позицию дочернего буфера у родителя
    pub fn find_child_position(&self, parent: BufferKey, child: BufferKey) -> Option<(u32, u32)> {
        self.attachments.iter()
            .find(|a| a.parent == parent && a.child == child)
            .map(|a| (a.x, a.y))
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
                buf.data[i].transform = transform;
            }
        }
    }
    
    /// Установить маску для символа
    pub fn set_char_mask(&mut self, buffer: BufferKey, x: u32, y: u32, mask: Option<Mask>) {
        if let Some(buf) = self.buffers.get_mut(buffer) {
            if let Some(i) = buf.index(x, y) {
                buf.data[i].mask = mask;
            }
        }
    }
    
    /// Установить blend mode для символа
    pub fn set_char_blend(&mut self, buffer: BufferKey, x: u32, y: u32, fg_blend: Blend, bg_blend: Blend) {
        if let Some(buf) = self.buffers.get_mut(buffer) {
            if let Some(i) = buf.index(x, y) {
                buf.data[i].fg_blend = fg_blend;
                buf.data[i].bg_blend = bg_blend;
            }
        }
    }
    
    /// Установить blend mode для области
    pub fn set_area_blend(&mut self, buffer: BufferKey, x: u32, y: u32, w: u32, h: u32, fg_blend: Blend, bg_blend: Blend) {
        if let Some(buf) = self.buffers.get_mut(buffer) {
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
        let default_glyph = self.buffers.get(buffer)
            .and_then(|b| self.atlases.get(b.atlas))
            .map(|a| a.config.default_glyph)
            .unwrap_or(0);
        
        if let Some(buf) = self.buffers.get_mut(buffer) {
            buf.clear(Character::blank(default_glyph));
        }
    }
    
    /// Очистить буфер и всех потомков (рекурсивно)
    pub fn clear_tree(&mut self, root: BufferKey) {
        self.clear_tree_internal(root, MAX_BUFFER_DEPTH);
    }
    
    fn clear_tree_internal(&mut self, buffer: BufferKey, depth: u8) {
        if depth == 0 {
            return;
        }
        
        // Очищаем сам буфер
        let default_glyph = self.buffers.get(buffer)
            .and_then(|b| self.atlases.get(b.atlas))
            .map(|a| a.config.default_glyph)
            .unwrap_or(0);
        
        if let Some(buf) = self.buffers.get_mut(buffer) {
            buf.clear(Character::blank(default_glyph));
        }
        
        // Собираем ключи детей
        let children: Vec<BufferKey> = self.attachments.iter()
            .filter(|a| a.parent == buffer)
            .map(|a| a.child)
            .collect();
        
        // Очищаем детей рекурсивно
        for child in children {
            self.clear_tree_internal(child, depth - 1);
        }
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
        let atlas = self.atlases.get(buf.atlas)?;
        
        let tile_w = atlas.config.tile_width as i32;
        let tile_h = atlas.config.tile_height as i32;
        
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
    
    /// Найти буфер под курсором (включая дочерние)
    pub fn buffer_at_point(
        &self,
        root: BufferKey,
        screen_x: i32,
        screen_y: i32,
        buf_screen_x: i32,
        buf_screen_y: i32,
    ) -> Option<(BufferKey, u32, u32)> {
        self.buffer_at_point_internal(root, screen_x, screen_y, buf_screen_x, buf_screen_y, MAX_BUFFER_DEPTH)
    }
    
    fn buffer_at_point_internal(
        &self,
        buffer_key: BufferKey,
        screen_x: i32,
        screen_y: i32,
        buf_screen_x: i32,
        buf_screen_y: i32,
        depth: u8,
    ) -> Option<(BufferKey, u32, u32)> {
        if depth == 0 {
            return None;
        }
        
        let buffer = self.buffers.get(buffer_key)?;
        
        // Пропускаем невидимые буферы
        if !buffer.visible {
            return None;
        }
        
        let atlas = self.atlases.get(buffer.atlas)?;
        
        let tile_w = atlas.config.tile_width as f32;
        let tile_h = atlas.config.tile_height as f32;
        
        // Собираем детей этого буфера
        let children: Vec<_> = self.attachments.iter()
            .filter(|a| a.parent == buffer_key)
            .collect();
        
        // Сначала проверяем дочерние (они поверх) — в обратном порядке z_index
        for att in children.iter().rev() {
            let child_screen_x = buf_screen_x + (att.x as f32 * tile_w) as i32;
            let child_screen_y = buf_screen_y + (att.y as f32 * tile_h) as i32;
            
            if let Some(result) = self.buffer_at_point_internal(
                att.child,
                screen_x,
                screen_y,
                child_screen_x,
                child_screen_y,
                depth - 1,
            ) {
                return Some(result);
            }
        }
        
        // Проверяем сам буфер
        if let Some((cx, cy)) = self.screen_to_cell(buffer_key, screen_x, screen_y, buf_screen_x, buf_screen_y) {
            return Some((buffer_key, cx, cy));
        }
        
        None
    }
}

impl Default for Grids {
    fn default() -> Self {
        Self::new()
    }
}
