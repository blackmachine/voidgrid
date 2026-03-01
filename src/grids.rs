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
use crate::buffer::{Attachment, Buffer, OrphanedChild};
use crate::palette::Palette;
use crate::shader::{ShaderData, UniformValue};
use crate::assets::{self, AssetCache};
use crate::global_registry::GlobalGlyphRegistry;
use crate::glyphset::Glyphset;

// ============================================================================
// Grids - главная структура
// ============================================================================

/// Максимальная глубина вложенности буферов
const MAX_BUFFER_DEPTH: u8 = 8;

/// Главная структура управления буферами, атласами и палитрами
pub struct Grids {
    pub(crate) atlases: SlotMap<AtlasKey, Atlas>,
    pub(crate) glyphsets: SlotMap<GlyphsetKey, Glyphset>,
    pub(crate) buffers: SlotMap<BufferKey, Buffer>,
    pub(crate) palettes: SlotMap<PaletteKey, Palette>,
    pub(crate) shaders: SlotMap<ShaderKey, ShaderData>,
    /// Связи родитель-потомок (вынесены из Buffer)
    pub(crate) attachments: Vec<Attachment>,
    pub(crate) global_registry: GlobalGlyphRegistry,
    
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
            glyphsets: SlotMap::with_key(),
            buffers: SlotMap::with_key(),
            palettes: SlotMap::with_key(),
            shaders: SlotMap::with_key(),
            attachments: Vec::new(),
            global_registry: GlobalGlyphRegistry::new(),
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
    // Glyphsets
    // ========================================================================

    pub fn create_glyphset_from_atlas(&mut self, name: &str, atlas_key: AtlasKey) -> GlyphsetKey {
        let atlas = self.atlases.get(atlas_key).expect("Atlas not found");
        let tile_w = atlas.config.tile_width;
        let tile_h = atlas.config.tile_height;
        let default_glyph = atlas.config.default_glyph;
        
        // Register default glyph
        let default_global_id = self.global_registry.register_glyph(atlas_key, default_glyph);
        
        let mut glyphset = Glyphset::new(name.to_string(), tile_w, tile_h, default_global_id);
        
        // Initialize variant 0 (default)
        glyphset.luts.push(vec![default_global_id; 65536]);
        glyphset.variant_names.insert("default".to_string(), 0);

        // Process semantic groups
        // Мы клонируем ключи, чтобы не держать заимствование atlas
        let groups: Vec<(String, crate::atlas::SemanticGroup)> = atlas.config.semantic_groups.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for (group_name, group_data) in groups {
            // 1. Разворачиваем Source в список имен и (опционально) кодов
            let (names, mut codes) = self.expand_source(&group_data.source);
            
            // Если коды не определены (виртуальные имена), пытаемся определить их через вариант "default"
            if codes.iter().any(|c| c.is_none()) {
                if let Some(default_mapping) = group_data.variants.get("default") {
                    let start_id = match default_mapping {
                        crate::atlas::VariantMapping::StartId(id) => *id,
                        crate::atlas::VariantMapping::Range([start, _]) => *start,
                    };
                    // Присваиваем коды последовательно, начиная со start_id
                    for (i, code_opt) in codes.iter_mut().enumerate() {
                        if code_opt.is_none() {
                            *code_opt = Some(start_id + i as u32);
                        }
                    }
                } else {
                    println!("Warning: Semantic group '{}' has virtual names but no 'default' variant to assign codes.", group_name);
                    continue;
                }
            }

            // Собираем финальный список кодов
            let final_codes: Vec<u32> = codes.into_iter().flatten().collect();
            
            if names.len() != final_codes.len() {
                println!("Warning: Group '{}' source mismatch.", group_name);
                continue;
            }

            // Регистрируем имена и группу в Glyphset
            glyphset.named_groups.insert(group_name.clone(), final_codes.clone());
            for (name, &code) in names.iter().zip(&final_codes) {
                glyphset.named_codes.insert(name.clone(), code);
            }

            // 2. Обрабатываем варианты
            for (variant_name, mapping) in &group_data.variants {
                // Получаем или создаем ID варианта
                let variant_id = *glyphset.variant_names.entry(variant_name.clone()).or_insert_with(|| {
                    let id = glyphset.luts.len() as u8;
                    glyphset.luts.push(vec![default_global_id; 65536]);
                    id
                });

                // Определяем физические глифы для этого варианта
                let start_glyph = match mapping {
                    crate::atlas::VariantMapping::StartId(id) => *id,
                    crate::atlas::VariantMapping::Range([start, end]) => {
                        let count = end - start + 1;
                        if count as usize != names.len() {
                            println!("Warning: Group '{}' variant '{}' range size mismatch.", group_name, variant_name);
                        }
                        *start
                    }
                };

                // Заполняем LUT
                for (i, &code) in final_codes.iter().enumerate() {
                    let physical_glyph = start_glyph + i as u32;
                    let global_id = self.global_registry.register_glyph(atlas_key, physical_glyph);
                    if (code as usize) < 65536 {
                        glyphset.luts[variant_id as usize][code as usize] = global_id;
                    }
                }
            }
        }
        
        self.glyphsets.insert(glyphset)
    }
    
    /// Вспомогательный метод для развертывания источника
    fn expand_source(&self, source: &crate::atlas::SourceType) -> (Vec<String>, Vec<Option<u32>>) {
        match source {
            crate::atlas::SourceType::List(list) => {
                // Список строк (виртуальные имена), коды пока неизвестны
                (list.clone(), vec![None; list.len()])
            },
            crate::atlas::SourceType::String(s) => {
                if s == "ascii_lower" {
                    let chars: Vec<char> = ('a'..='z').collect();
                    let names = chars.iter().map(|c| c.to_string()).collect();
                    let codes = chars.iter().map(|c| Some(*c as u32)).collect();
                    (names, codes)
                } else if s == "ascii_upper" {
                    let chars: Vec<char> = ('A'..='Z').collect();
                    let names = chars.iter().map(|c| c.to_string()).collect();
                    let codes = chars.iter().map(|c| Some(*c as u32)).collect();
                    (names, codes)
                } else if s == "digits" {
                    let chars: Vec<char> = ('0'..='9').collect();
                    let names = chars.iter().map(|c| c.to_string()).collect();
                    let codes = chars.iter().map(|c| Some(*c as u32)).collect();
                    (names, codes)
                } else if let Some(stripped) = s.strip_prefix("chars:") {
                    let chars: Vec<char> = stripped.chars().collect();
                    let names = chars.iter().map(|c| c.to_string()).collect();
                    let codes = chars.iter().map(|c| Some(*c as u32)).collect();
                    (names, codes)
                } else {
                    // Неизвестный пресет или просто строка
                    (vec![s.clone()], vec![None])
                }
            }
        }
    }

    pub fn glyphset_size(&self, key: GlyphsetKey) -> Option<(u32, u32)> {
        self.glyphsets.get(key).map(|g| (g.tile_w, g.tile_h))
    }

    /// Получить семантический код по имени (например, "arrow_left" -> 201)
    pub fn resolve_code(&self, key: GlyphsetKey, name: &str) -> Option<u32> {
        self.glyphsets.get(key).and_then(|gs| gs.named_codes.get(name).copied())
    }

    /// Получить список кодов для группы (например, "arrows" -> [201, 202])
    pub fn resolve_group(&self, key: GlyphsetKey, group: &str) -> Option<&Vec<u32>> {
        self.glyphsets.get(key).and_then(|gs| gs.named_groups.get(group))
    }

    /// Монтировать атлас в виртуальное дерево путей.
    /// Генерирует пути вида "{prefix}/{char}" и "{prefix}/{char}:{variant}".
    pub fn mount_atlas(&mut self, prefix: &str, atlas_key: AtlasKey) {
        let atlas = self.atlases.get(atlas_key).expect("Atlas not found");
        
        // Clone groups to avoid borrowing issues
        let groups: Vec<(String, crate::atlas::SemanticGroup)> = atlas.config.semantic_groups.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
            
        for (_group_name, group_data) in groups {
            let (names, mut codes) = self.expand_source(&group_data.source);
            
            // Resolve codes if needed (same logic as create_glyphset)
            if codes.iter().any(|c| c.is_none()) {
                 if let Some(default_mapping) = group_data.variants.get("default") {
                    let start_id = match default_mapping {
                        crate::atlas::VariantMapping::StartId(id) => *id,
                        crate::atlas::VariantMapping::Range([start, _]) => *start,
                    };
                    for (i, code_opt) in codes.iter_mut().enumerate() {
                        if code_opt.is_none() {
                            *code_opt = Some(start_id + i as u32);
                        }
                    }
                }
            }
            
            let final_codes: Vec<u32> = codes.into_iter().flatten().collect();
            
            // Iterate variants
            for (variant_name, mapping) in &group_data.variants {
                 let start_glyph = match mapping {
                    crate::atlas::VariantMapping::StartId(id) => *id,
                    crate::atlas::VariantMapping::Range([start, _]) => *start,
                };
                
                for (i, name) in names.iter().enumerate() {
                    if i >= final_codes.len() { break; }
                    let physical_glyph = start_glyph + i as u32;
                    let gid = self.global_registry.register_glyph(atlas_key, physical_glyph);
                    
                    let path = if variant_name == "default" {
                        format!("{}/{}", prefix, name)
                    } else {
                        format!("{}/{}:{}", prefix, name, variant_name)
                    };
                    
                    self.global_registry.map_path(path, gid);
                }
            }
        }
        
        println!("Mounted atlas {:?} at '{}'", atlas_key, prefix);
    }

    /// Вывести отладочную информацию о реестре глифов с именами атласов
    pub fn debug_print_registry(&self) {
        println!("=== Global Glyph Registry Debug ===");
        println!("Physical Glyphs: {} entries registered.", self.global_registry.entries.len());
        
        // Группируем пути по префиксам (имитация директорий)
        let mut mounts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        
        for path in self.global_registry.path_cache.keys() {
            // Ищем последний разделитель
            let prefix = if let Some(idx) = path.rfind('/') {
                // Корректировка для символа '/', который создает путь вида "...//"
                // Если перед найденным слэшем стоит еще один слэш, значит это разделитель + имя файла "/"
                if idx > 0 && path.as_bytes()[idx - 1] == b'/' {
                    &path[0..idx - 1]
                } else {
                    &path[0..idx]
                }
            } else {
                "<root>"
            };
            *mounts.entry(prefix.to_string()).or_default() += 1;
        }
        
        println!("Mounted Paths (Virtual Filesystem):");
        let mut sorted_mounts: Vec<_> = mounts.into_iter().collect();
        sorted_mounts.sort_by(|a, b| a.0.cmp(&b.0));
        
        for (prefix, count) in sorted_mounts {
            println!("  '{}/*' -> {} items", prefix, count);
        }
        println!("=================================");
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
        glyphset: GlyphsetKey,
    ) -> BufferKey {
        let default_code = 32; // Space
        let fill = Character::blank(default_code);
        let buffer = Buffer::new(name, w, h, glyphset, 0, fill);
        self.buffers.insert(buffer)
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
        let variant_id = if let Some(v) = variant {
            let v_str = v.into();
            let gs_key = self.buffers.get(key).map(|b| b.glyphset).unwrap();
            self.glyphsets.get(gs_key).and_then(|gs| gs.variant_names.get(&v_str)).copied().unwrap_or(0)
        } else { 0 };

        if let Some(buf) = self.buffers.get_mut(key) {
            buf.default_variant_id = variant_id;
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
        let default_code = 32;
        
        let mut orphaned = Vec::new();
        
        if let Some(buf) = self.buffers.get_mut(buffer) {
            let old_data = std::mem::take(&mut buf.data);
            let old_w = buf.w;
            let old_h = buf.h;
            
            // Новый буфер данных
            let fill = Character::blank(default_code);
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
        let default_code = 32;
        
        if let Some(buf) = self.buffers.get_mut(buffer) {
            buf.clear(Character::blank(default_code));
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
        let default_code = 32;
        
        if let Some(buf) = self.buffers.get_mut(buffer) {
            buf.clear(Character::blank(default_code));
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
        let gs = self.glyphsets.get(buf.glyphset)?;
        
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
        
        let gs = self.glyphsets.get(buffer.glyphset)?;
        
        let tile_w = gs.tile_w as f32;
        let tile_h = gs.tile_h as f32;
        
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
