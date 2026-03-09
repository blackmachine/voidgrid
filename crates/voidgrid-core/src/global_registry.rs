use std::collections::HashMap;
use crate::types::AtlasKey;

pub struct GlobalGlyphRegistry {
    /// O(1) access to physical glyphs. Index = global_id.
    /// Stores (AtlasKey, physical_glyph_index).
    pub entries: Vec<(AtlasKey, u32)>, 
    
    /// Flat path cache for lookups (already resolved from tree logic).
    pub path_cache: HashMap<String, u32>,

    /// Reverse lookup for deduplication: (AtlasKey, local_glyph) -> global_id
    reverse_lookup: HashMap<(AtlasKey, u32), u32>,
}

impl GlobalGlyphRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            path_cache: HashMap::new(),
            reverse_lookup: HashMap::new(),
        }
    }

    pub fn register_glyph(&mut self, atlas: AtlasKey, local_glyph: u32) -> u32 {
        if let Some(&id) = self.reverse_lookup.get(&(atlas, local_glyph)) {
            return id;
        }
        let global_id = self.entries.len() as u32;
        self.entries.push((atlas, local_glyph));
        self.reverse_lookup.insert((atlas, local_glyph), global_id);
        global_id
    }

    pub fn map_path(&mut self, path: String, global_id: u32) {
        self.path_cache.insert(path, global_id);
    }

    pub fn query(&self, path: &str) -> Option<u32> {
        self.path_cache.get(path).copied()
    }
}