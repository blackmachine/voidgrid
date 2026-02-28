use std::fs;
use std::collections::HashMap;
use raylib::prelude::*;
use crate::atlas::{Atlas, AtlasConfig};
use crate::palette::{Palette, PaletteConfig};
use crate::shader::ShaderData;
use crate::types::{AtlasKey, PaletteKey, ShaderKey};

pub fn load_atlas_from_file(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    config_path: &str,
) -> Result<Atlas, Box<dyn std::error::Error>> {
    let json_str = fs::read_to_string(config_path)?;
    let mut config: AtlasConfig = serde_json::from_str(&json_str)?;
    config.build_char_map();
    
    let texture = rl.load_texture(thread, &config.texture_path)
        .map_err(|e| format!("Failed to load texture '{}': {}", config.texture_path, e))?;
    
    unsafe { 
        raylib::ffi::SetTextureFilter(*texture.as_ref(), raylib::ffi::TextureFilter::TEXTURE_FILTER_POINT as i32);
    }
    
    Ok(Atlas { config, texture })
}

pub fn load_palette_from_file(path: &str) -> Result<Palette, Box<dyn std::error::Error>> {
    let json_str = fs::read_to_string(path)?;
    let config: PaletteConfig = serde_json::from_str(&json_str)?;
    Ok(Palette::from_config(config))
}

pub fn load_shader_from_file(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    fragment_path: &str,
) -> Result<ShaderData, String> {
    let shader = rl.load_shader(thread, None, Some(fragment_path));
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