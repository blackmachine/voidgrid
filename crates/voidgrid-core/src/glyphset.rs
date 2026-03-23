use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Glyphset {
    pub name: String,
    pub tile_w: u32,
    pub tile_h: u32,

    /// Mapping of variant names to their IDs (e.g., "bold" -> 1)
    pub variant_names: HashMap<String, u8>,

    /// LUT for O(1) rendering.
    /// Outer vector index: variant_id.
    /// Inner vector index: code (0..65535).
    /// Value: global_id from GlobalGlyphRegistry.
    pub luts: Vec<Vec<u32>>,

    pub default_global_id: u32,
}

/// Sentinel value for unmapped glyphs in LUTs.
pub const UNMAPPED: u32 = u32::MAX;

impl Glyphset {
    pub fn new(name: String, tile_w: u32, tile_h: u32, default_global_id: u32) -> Self {
        let mut gs = Self {
            name,
            tile_w,
            tile_h,
            variant_names: HashMap::new(),
            luts: Vec::new(),
            default_global_id,
        };
        // Always create the default variant (id=0)
        gs.variant_names.insert("default".to_string(), 0);
        gs.luts.push(vec![UNMAPPED; 65536]);
        gs
    }

    /// Get or create a variant, returning its ID.
    pub fn ensure_variant(&mut self, name: &str) -> u8 {
        if let Some(&id) = self.variant_names.get(name) {
            return id;
        }
        let id = self.luts.len() as u8;
        self.variant_names.insert(name.to_string(), id);
        self.luts.push(vec![UNMAPPED; 65536]);
        id
    }

    /// Fill unmapped LUT slots with fallbacks.
    /// Default variant: fill with default_global_id.
    /// Other variants: fill with default variant's value.
    pub fn apply_fallbacks(&mut self) {
        // Fix default variant
        for code in 0..65536 {
            if self.luts[0][code] == UNMAPPED {
                self.luts[0][code] = self.default_global_id;
            }
        }
        // Fix other variants (fallback to default)
        let default_lut = self.luts[0].clone();
        for variant_id in 1..self.luts.len() {
            for code in 0..65536 {
                if self.luts[variant_id][code] == UNMAPPED {
                    self.luts[variant_id][code] = default_lut[code];
                }
            }
        }
    }
}
