use std::collections::HashMap;
use raylib::prelude::{Texture2D, Rectangle};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum SourceType {
    String(String),
    List(Vec<String>),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum VariantMapping {
    StartId(u32),
    Range([u32; 2]),
}

#[derive(Debug, Clone, Deserialize)]
pub struct SemanticGroup {
    pub source: SourceType,
    pub variants: HashMap<String, VariantMapping>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AtlasConfig {
    pub tile_width: u32,
    pub tile_height: u32,
    pub columns: u32,
    pub texture_path: String,
    pub default_glyph: u32,
    
    #[serde(default)]
    pub semantic_groups: HashMap<String, SemanticGroup>,
}

impl AtlasConfig {
}

pub struct Atlas {
    pub config: AtlasConfig,
    pub texture: Texture2D,
}

impl std::fmt::Debug for Atlas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Atlas")
            .field("config", &self.config)
            .field("texture", &"<Texture2D>")
            .finish()
    }
}

impl Atlas {
    pub fn get_glyph_source(&self, glyph: u32) -> (Rectangle, f32, f32) {
        let tile_w = self.config.tile_width as f32;
        let tile_h = self.config.tile_height as f32;
        let cols = self.config.columns;
        let glyph_x = (glyph % cols) as f32 * tile_w;
        let glyph_y = (glyph / cols) as f32 * tile_h;
        (Rectangle::new(glyph_x, glyph_y, tile_w, tile_h), tile_w, tile_h)
    }
    
    pub fn texture_size(&self) -> (f32, f32) {
        (self.texture.width as f32, self.texture.height as f32)
    }
}