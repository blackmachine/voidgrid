use std::collections::HashMap;
use raylib::prelude::*;
use crate::atlas::{Atlas, AtlasConfig};
use crate::palette::{Palette, PaletteConfig};
use crate::shader::ShaderData;
use crate::types::{AtlasKey, PaletteKey, ShaderKey};
use crate::resource_pack::ResourceProvider;

pub fn load_atlas(
    provider: &mut dyn ResourceProvider,
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    config_path: &str,
) -> Result<Atlas, Box<dyn std::error::Error>> {
    // 1. Читаем конфиг атласа
    let json_str = provider.read_string(config_path)?;
    let config: AtlasConfig = serde_json::from_str(&json_str)?;
    
    // 2. Читаем байты текстуры
    let bytes = provider.read_bytes(&config.texture_path)?;
    
    // Определяем расширение для корректной загрузки (или дефолт .png)
    let ext = std::path::Path::new(&config.texture_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png");
    let ext_dot = format!(".{}", ext);

    // 3. Загружаем Image из памяти
    let image = Image::load_image_from_mem(&ext_dot, &bytes)
        .map_err(|e| format!("Failed to load image from memory '{}': {}", config.texture_path, e))?;

    // 4. Создаем Texture2D
    let texture = rl.load_texture_from_image(thread, &image)
        .map_err(|e| format!("Failed to create texture from image '{}': {}", config.texture_path, e))?;
    
    unsafe { 
        raylib::ffi::SetTextureFilter(*texture.as_ref(), raylib::ffi::TextureFilter::TEXTURE_FILTER_POINT as i32);
    }
    
    Ok(Atlas { config, texture })
}

pub fn load_palette(
    provider: &mut dyn ResourceProvider,
    path: &str
) -> Result<Palette, Box<dyn std::error::Error>> {
    let json_str = provider.read_string(path)?;
    let config: PaletteConfig = serde_json::from_str(&json_str)?;
    Ok(Palette::from_config(config))
}

pub fn load_shader(
    provider: &mut dyn ResourceProvider,
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    fragment_path: &str,
) -> Result<ShaderData, String> {
    let fragment_code = provider.read_string(fragment_path)
        .map_err(|e| format!("Failed to read shader file: {}", e))?;
        
    let shader = rl.load_shader_from_memory(thread, None, Some(&fragment_code));
    let name = std::path::Path::new(fragment_path).file_stem()
        .and_then(|n| n.to_str()).unwrap_or("unnamed").to_string();
    Ok(ShaderData::new(shader, name))
}

/// Кэш загруженных ресурсов для предотвращения дублирования
pub struct AssetCache {
    pub atlases: HashMap<String, AtlasKey>,
    pub palettes: HashMap<String, PaletteKey>,
    pub shaders: HashMap<String, ShaderKey>,
}

impl AssetCache {
    pub fn new() -> Self {
        Self {
            atlases: HashMap::new(),
            palettes: HashMap::new(),
            shaders: HashMap::new(),
        }
    }
}