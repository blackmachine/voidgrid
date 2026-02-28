use std::collections::HashMap;
use raylib::prelude::{Texture2D, Rectangle};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum MappingRule {
    #[serde(rename = "range")]
    Range {
        start: char,
        end: char,
        glyph: u32,
        #[serde(default)]
        alternatives: HashMap<String, u32>,
    },
    #[serde(rename = "single")]
    Single {
        char: char,
        glyph: u32,
        #[serde(default)]
        alternatives: HashMap<String, u32>,
    },
    #[serde(rename = "string")]
    String {
        chars: String,
        glyph: u32,
        #[serde(default)]
        alternatives: HashMap<String, u32>,
    },
}

#[derive(Debug, Clone)]
struct CharMapping {
    glyph: u32,
    alternatives: HashMap<String, u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AtlasConfig {
    pub tile_width: u32,
    pub tile_height: u32,
    pub columns: u32,
    pub texture_path: String,
    pub default_glyph: u32,
    pub mappings: Vec<MappingRule>,
    
    #[serde(skip)]
    char_map: HashMap<char, CharMapping>,
}

impl AtlasConfig {
    pub fn build_char_map(&mut self) {
        self.char_map.clear();
        for rule in &self.mappings {
            let alternatives = match rule {
                MappingRule::Range { alternatives, .. } => alternatives,
                MappingRule::Single { alternatives, .. } => alternatives,
                MappingRule::String { alternatives, .. } => alternatives,
            }.clone();
            
            match rule {
                MappingRule::Range { start, end, glyph, .. } => {
                    for (i, code) in (*start as u32..=*end as u32).enumerate() {
                        if let Some(ch) = char::from_u32(code) {
                            self.char_map.insert(ch, CharMapping { glyph: glyph + i as u32, alternatives: alternatives.clone() });
                        }
                    }
                }
                MappingRule::Single { char, glyph, .. } => {
                    self.char_map.insert(*char, CharMapping { glyph: *glyph, alternatives: alternatives.clone() });
                }
                MappingRule::String { chars, glyph, .. } => {
                    for (i, ch) in chars.chars().enumerate() {
                        self.char_map.insert(ch, CharMapping { glyph: glyph + i as u32, alternatives: alternatives.clone() });
                    }
                }
            }
        }
    }
    
    pub fn map_char(&self, ch: char) -> u32 {
        self.char_map.get(&ch).map(|m| m.glyph).unwrap_or(self.default_glyph)
    }
    
    pub fn map_char_variant(&self, ch: char, variant: &str) -> u32 {
        if let Some(mapping) = self.char_map.get(&ch) {
            if let Some(&offset) = mapping.alternatives.get(variant) {
                return mapping.glyph + offset;
            }
            return mapping.glyph;
        }
        self.default_glyph
    }
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