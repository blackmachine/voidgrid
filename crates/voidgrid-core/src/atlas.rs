use std::collections::HashMap;
use raylib::prelude::{Texture2D, Rectangle};
use serde::Deserialize;

// ============================================================================
// Atlas — physical PNG sprite sheet (one per PNG file)
// Used by the renderer for O(1) glyph source lookups.
// ============================================================================

pub struct Atlas {
    pub texture: Texture2D,
    pub tile_w: u32,
    pub tile_h: u32,
    pub cols: u32,
}

impl std::fmt::Debug for Atlas {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Atlas")
            .field("tile_w", &self.tile_w)
            .field("tile_h", &self.tile_h)
            .field("cols", &self.cols)
            .field("texture", &"<Texture2D>")
            .finish()
    }
}

impl Atlas {
    pub fn get_glyph_source(&self, glyph: u32) -> (Rectangle, f32, f32) {
        let tile_w = self.tile_w as f32;
        let tile_h = self.tile_h as f32;
        let glyph_x = (glyph % self.cols) as f32 * tile_w;
        let glyph_y = (glyph / self.cols) as f32 * tile_h;
        (Rectangle::new(glyph_x, glyph_y, tile_w, tile_h), tile_w, tile_h)
    }

    pub fn texture_size(&self) -> (f32, f32) {
        (self.texture.width as f32, self.texture.height as f32)
    }
}

// ============================================================================
// AtlasDescriptor — virtual assembly of byte→sprite from PNG fragments.
// Parsed from atlas JSON. References one or more PNG files.
// ============================================================================

/// Source PNG reference within a layer.
#[derive(Debug, Clone, Deserialize)]
pub struct LayerSource {
    pub file: String,
    pub w: u32,
    pub h: u32,
    pub cols: u32,
    #[serde(default)]
    pub start: u32,
}

/// How bytes map to sprites in this layer.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum LayerMapping {
    /// Contiguous byte range → sequential sprites from source.start
    Range { bytes: [u32; 2] },
    /// Each char's Unicode codepoint → sequential sprites from source.start
    Chars { chars: String },
    /// Explicit per-entry mapping: byte_value → sprite_index
    Entries { entries: Vec<[u32; 2]> },
}

/// A single layer within an atlas node.
/// Layers are applied sequentially; later layers overwrite earlier ones.
#[derive(Debug, Clone, Deserialize)]
pub struct AtlasLayer {
    #[serde(flatten)]
    pub mapping: LayerMapping,
    pub source: LayerSource,
}

/// A node within the atlas descriptor's internal tree.
/// Nodes represent addressable regions of the byte space.
#[derive(Debug, Clone, Deserialize)]
pub struct AtlasNode {
    pub layers: Vec<AtlasLayer>,
}

/// Atlas descriptor parsed from JSON.
/// Defines the virtual assembly of one or more PNG files into a byte→sprite mapping.
#[derive(Debug, Clone, Deserialize)]
pub struct AtlasDescriptor {
    pub name: String,
    #[serde(default)]
    pub default_byte: u32,
    /// Internal tree structure. Keys: "/" for root, "/:inverted" for variants, etc.
    /// If empty or absent, treated as single root node with identity mapping.
    #[serde(default)]
    pub nodes: HashMap<String, AtlasNode>,
}

impl AtlasDescriptor {
    /// Get the tile size from the first layer of the first node.
    /// All layers in a descriptor must share the same tile size.
    pub fn tile_size(&self) -> Option<(u32, u32)> {
        self.nodes.values()
            .flat_map(|n| n.layers.first())
            .map(|l| (l.source.w, l.source.h))
            .next()
    }

    /// Collect all unique PNG file paths referenced by this descriptor.
    pub fn referenced_files(&self) -> Vec<String> {
        let mut files: Vec<String> = Vec::new();
        for node in self.nodes.values() {
            for layer in &node.layers {
                if !files.contains(&layer.source.file) {
                    files.push(layer.source.file.clone());
                }
            }
        }
        files
    }

    /// Get root node ("/") and all variant nodes (starting with "/:").
    /// Returns (path_suffix, node) pairs.
    pub fn root_and_variants(&self) -> Vec<(&str, &AtlasNode)> {
        let mut result = Vec::new();
        if let Some(root) = self.nodes.get("/") {
            result.push(("/", root));
        }
        let mut variant_keys: Vec<&String> = self.nodes.keys()
            .filter(|k| k.starts_with("/:"))
            .collect();
        variant_keys.sort();
        for key in variant_keys {
            result.push((key.as_str(), &self.nodes[key]));
        }
        result
    }

    /// Get child nodes (starting with "/" but not "/:" and not "/").
    pub fn child_nodes(&self) -> Vec<(&str, &AtlasNode)> {
        let mut result: Vec<(&str, &AtlasNode)> = self.nodes.iter()
            .filter(|(k, _)| k.as_str() != "/" && !k.starts_with("/:") && k.starts_with('/'))
            .map(|(k, v)| (k.as_str(), v))
            .collect();
        result.sort_by_key(|(k, _)| *k);
        result
    }
}

impl AtlasLayer {
    /// Iterate over (byte_value, sprite_index) pairs produced by this layer.
    pub fn byte_sprite_pairs(&self) -> Vec<(u32, u32)> {
        match &self.mapping {
            LayerMapping::Range { bytes } => {
                let [from, to] = *bytes;
                (from..=to)
                    .enumerate()
                    .map(|(i, byte)| (byte, self.source.start + i as u32))
                    .collect()
            }
            LayerMapping::Chars { chars } => {
                chars.chars()
                    .enumerate()
                    .map(|(i, ch)| (ch as u32, self.source.start + i as u32))
                    .collect()
            }
            LayerMapping::Entries { entries } => {
                entries.iter()
                    .map(|&[byte, sprite]| (byte, sprite))
                    .collect()
            }
        }
    }
}
