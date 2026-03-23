use std::collections::HashMap;
use raylib::prelude::*;
use crate::atlas::{Atlas, AtlasDescriptor};
use crate::palette::{Palette, PaletteConfig};
use crate::shader::ShaderData;
use crate::types::{AtlasKey, PaletteKey, ShaderKey};
use crate::resource_pack::ResourceProvider;

/// Load a single PNG sprite sheet as an Atlas.
pub fn load_png(
    provider: &mut dyn ResourceProvider,
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    file_path: &str,
    tile_w: u32,
    tile_h: u32,
    cols: u32,
) -> Result<Atlas, Box<dyn std::error::Error>> {
    let bytes = provider.read_bytes(file_path)?;

    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("png");
    let ext_dot = format!(".{}", ext);

    let image = Image::load_image_from_mem(&ext_dot, &bytes)
        .map_err(|e| format!("Failed to load image '{}': {}", file_path, e))?;

    let texture = rl.load_texture_from_image(thread, &image)
        .map_err(|e| format!("Failed to create texture '{}': {}", file_path, e))?;

    unsafe {
        raylib::ffi::SetTextureFilter(
            *texture.as_ref(),
            raylib::ffi::TextureFilter::TEXTURE_FILTER_POINT as i32,
        );
    }

    Ok(Atlas { texture, tile_w, tile_h, cols })
}

/// Parse an atlas descriptor JSON file.
pub fn load_atlas_descriptor(
    provider: &mut dyn ResourceProvider,
    config_path: &str,
) -> Result<AtlasDescriptor, Box<dyn std::error::Error>> {
    let json_str = provider.read_string(config_path)?;
    let descriptor: AtlasDescriptor = serde_json::from_str(&json_str)?;
    Ok(descriptor)
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

/// Cache for loaded resources to prevent duplication.
pub struct AssetCache {
    /// PNG file path → AtlasKey
    pub pngs: HashMap<String, AtlasKey>,
    pub palettes: HashMap<String, PaletteKey>,
    pub shaders: HashMap<String, ShaderKey>,
}

impl AssetCache {
    pub fn new() -> Self {
        Self {
            pngs: HashMap::new(),
            palettes: HashMap::new(),
            shaders: HashMap::new(),
        }
    }
}
