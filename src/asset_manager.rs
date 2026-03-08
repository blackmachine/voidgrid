use raylib::prelude::*;
use slotmap::SlotMap;

use crate::types::*;
use crate::atlas::Atlas;
use crate::palette::Palette;
use crate::shader::{ShaderData, UniformValue};
use crate::assets::{self, AssetCache};
use crate::global_registry::GlobalGlyphRegistry;
use crate::glyphset::Glyphset;

pub struct AssetManager {
    pub atlases: SlotMap<AtlasKey, Atlas>,
    pub glyphsets: SlotMap<GlyphsetKey, Glyphset>,
    pub palettes: SlotMap<PaletteKey, Palette>,
    pub shaders: SlotMap<ShaderKey, ShaderData>,
    pub global_registry: GlobalGlyphRegistry,
    
    // Resource Cache
    cache: AssetCache,
}

impl AssetManager {
    pub fn new() -> Self {
        Self {
            atlases: SlotMap::with_key(),
            glyphsets: SlotMap::with_key(),
            palettes: SlotMap::with_key(),
            shaders: SlotMap::with_key(),
            global_registry: GlobalGlyphRegistry::new(),
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
        
        // Инициализируем и заполняем данными из первого атласа
        Self::merge_atlas_internal(&self.atlases, &mut self.global_registry, &mut glyphset, atlas_key);
        
        self.glyphsets.insert(glyphset)
    }

    /// Добавить (влить) атлас в существующий глифсет
    pub fn merge_atlas(&mut self, glyphset_key: GlyphsetKey, atlas_key: AtlasKey) {
        if let Some(glyphset) = self.glyphsets.get_mut(glyphset_key) {
            Self::merge_atlas_internal(&self.atlases, &mut self.global_registry, glyphset, atlas_key);
        }
    }

    /// Внутренняя функция слияния. 
    fn merge_atlas_internal(
        atlases: &SlotMap<AtlasKey, Atlas>,
        registry: &mut GlobalGlyphRegistry,
        glyphset: &mut Glyphset,
        atlas_key: AtlasKey
    ) {
        let atlas = atlases.get(atlas_key).expect("Atlas not found");
        
        // Sentinel for unmapped glyphs
        const UNMAPPED: u32 = u32::MAX;

        // Ensure default variant exists
        if glyphset.luts.is_empty() {
            glyphset.luts.push(vec![UNMAPPED; 65536]);
            glyphset.variant_names.insert("default".to_string(), 0);
        }

        // Process semantic groups
        // Мы клонируем ключи, чтобы не держать заимствование atlas
        let groups: Vec<(String, crate::atlas::SemanticGroup)> = atlas.config.semantic_groups.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();

        for (group_name, group_data) in groups {
            // 1. Разворачиваем Source в список имен и (опционально) кодов
            let (names, mut codes) = Self::expand_source(&group_data.source);
            
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
                    glyphset.luts.push(vec![UNMAPPED; 65536]);
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
                    let global_id = registry.register_glyph(atlas_key, physical_glyph);
                    if (code as usize) < 65536 {
                        glyphset.luts[variant_id as usize][code as usize] = global_id;
                    }
                }
            }
        }
        
        // Post-process: Apply fallbacks
        // 1. Fix default variant (fill holes with atlas default)
        for code in 0..65536 {
            if glyphset.luts[0][code] == UNMAPPED {
                glyphset.luts[0][code] = glyphset.default_global_id;
            }
        }
        
        // 2. Fix other variants (fill holes with default variant)
        let default_lut = glyphset.luts[0].clone();
        for variant_id in 1..glyphset.luts.len() {
            for code in 0..65536 {
                if glyphset.luts[variant_id][code] == UNMAPPED {
                    glyphset.luts[variant_id][code] = default_lut[code];
                }
            }
        }
    }
    
    /// Вспомогательный метод для развертывания источника
    fn expand_source(source: &crate::atlas::SourceType) -> (Vec<String>, Vec<Option<u32>>) {
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
            let (names, mut codes) = Self::expand_source(&group_data.source);
            
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
            let (prefix, filename) = if let Some(idx) = path.rfind('/') {
                // Корректировка для символа '/', который создает путь вида "...//"
                // Если перед найденным слэшем стоит еще один слэш, значит это разделитель + имя файла "/"
                if idx > 0 && path.as_bytes()[idx - 1] == b'/' {
                    (&path[0..idx - 1], &path[idx..])
                } else {
                    (&path[0..idx], &path[idx + 1..])
                }
            } else {
                ("<root>", path.as_str())
            };
            
            let variant = if let Some(colon_idx) = filename.rfind(':') {
                if filename == ":" { "default" } else { &filename[colon_idx + 1..] }
            } else {
                "default"
            };
            
            let group_key = if variant == "default" {
                prefix.to_string()
            } else {
                format!("{}:{}", prefix, variant)
            };
            
            *mounts.entry(group_key).or_default() += 1;
        }
        
        println!("Mounted Paths (Virtual Filesystem):");
        let mut sorted_mounts: Vec<_> = mounts.into_iter().collect();
        sorted_mounts.sort_by(|a, b| a.0.cmp(&b.0));
        
        for (group, count) in sorted_mounts {
            println!("  '{}/*' -> {} items", group, count);
        }
        println!("=================================");
    }
}