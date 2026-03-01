use std::collections::HashMap;
use crate::global_registry::GlobalGlyphRegistry;

#[derive(Clone, Debug)]
pub struct Glyphset {
    pub name: String,
    pub tile_w: u32,
    pub tile_h: u32,
    
    /// Mapping of variant names to their IDs (e.g., "bold" -> 1)
    pub variant_names: HashMap<String, u8>,
    
    /// LUT for O(1) rendering. 
    /// Outer vector index: variant_id.
    /// Inner vector index: code.
    /// Value: global_id from GlobalGlyphRegistry.
    pub luts: Vec<Vec<u32>>,
    
    /// Additional dictionary for UI elements ("ui/borders:top_left" -> code)
    pub namespace_map: HashMap<String, u32>,
    
    pub default_global_id: u32,
}

impl Glyphset {
    pub fn new(name: String, tile_w: u32, tile_h: u32, default_global_id: u32) -> Self {
        Self {
            name,
            tile_w,
            tile_h,
            variant_names: HashMap::new(),
            luts: Vec::new(),
            namespace_map: HashMap::new(),
            default_global_id,
        }
    }
}