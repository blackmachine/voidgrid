use std::collections::HashMap;
use raylib::prelude::*;
use slotmap::SlotMap;
use serde::Deserialize;

use crate::types::*;
use crate::atlas::{Atlas, AtlasDescriptor, AtlasNode};
use crate::palette::Palette;
use crate::shader::{ShaderData, UniformValue};
use crate::assets::{self, AssetCache};
use crate::global_registry::GlobalGlyphRegistry;
use crate::glyphset::Glyphset;
use crate::virtual_tree::VirtualTree;
use crate::resource_pack::ResourceProvider;

// ============================================================================
// Compose rules for glyphset assembly
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct RemapConfig {
    pub src: [u32; 2],
    pub dst: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ComposeRule {
    pub node: String,
    #[serde(default)]
    pub remap: Option<RemapConfig>,
    #[serde(default)]
    pub as_variant: Option<String>,
}

// ============================================================================
// Intermediate types for glyphset composition
// ============================================================================

struct CollectedVariant {
    variant_name: Option<String>,
    layers: Vec<CollectedLayer>,
}

struct CollectedLayer {
    file: String,
    pairs: Vec<(u32, u32)>,  // (byte_value, sprite_index)
}

// ============================================================================
// AssetManager
// ============================================================================

pub struct AssetManager {
    pub atlases: SlotMap<AtlasKey, Atlas>,
    pub glyphsets: SlotMap<GlyphsetKey, Glyphset>,
    pub palettes: SlotMap<PaletteKey, Palette>,
    pub shaders: SlotMap<ShaderKey, ShaderData>,
    pub global_registry: GlobalGlyphRegistry,
    pub descriptors: HashMap<String, AtlasDescriptor>,
    pub tree: VirtualTree,

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
            descriptors: HashMap::new(),
            tree: VirtualTree::new(),
            cache: AssetCache::new(),
        }
    }

    // ========================================================================
    // Shaders
    // ========================================================================

    pub fn load_shader(
        &mut self,
        provider: &mut dyn ResourceProvider,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        fragment_path: &str
    ) -> Result<ShaderKey, String> {
        if let Some(&key) = self.cache.shaders.get(fragment_path) {
            return Ok(key);
        }
        let shader_data = assets::load_shader(provider, rl, thread, fragment_path)?;
        let key = self.shaders.insert(shader_data);
        self.cache.shaders.insert(fragment_path.to_string(), key);
        Ok(key)
    }

    pub fn shader(&self, key: ShaderKey) -> Option<&ShaderData> {
        self.shaders.get(key)
    }

    pub fn shader_mut(&mut self, key: ShaderKey) -> Option<&mut ShaderData> {
        self.shaders.get_mut(key)
    }

    pub fn set_shader_float(&mut self, key: ShaderKey, name: &str, value: f32) {
        if let Some(shader) = self.shaders.get_mut(key) {
            shader.set_uniform(name, UniformValue::Float(value));
        }
    }

    pub fn set_shader_vec2(&mut self, key: ShaderKey, name: &str, value: (f32, f32)) {
        if let Some(shader) = self.shaders.get_mut(key) {
            shader.set_uniform(name, UniformValue::Vec2(value.0, value.1));
        }
    }

    pub fn set_shader_vec3(&mut self, key: ShaderKey, name: &str, value: (f32, f32, f32)) {
        if let Some(shader) = self.shaders.get_mut(key) {
            shader.set_uniform(name, UniformValue::Vec3(value.0, value.1, value.2));
        }
    }

    pub fn set_shader_vec4(&mut self, key: ShaderKey, name: &str, value: (f32, f32, f32, f32)) {
        if let Some(shader) = self.shaders.get_mut(key) {
            shader.set_uniform(name, UniformValue::Vec4(value.0, value.1, value.2, value.3));
        }
    }

    pub fn set_shader_int(&mut self, key: ShaderKey, name: &str, value: i32) {
        if let Some(shader) = self.shaders.get_mut(key) {
            shader.set_uniform(name, UniformValue::Int(value));
        }
    }

    // ========================================================================
    // Palettes
    // ========================================================================

    pub fn create_palette(&mut self, name: impl Into<String>) -> PaletteKey {
        self.palettes.insert(Palette::new(name))
    }

    pub fn load_palette(
        &mut self,
        provider: &mut dyn ResourceProvider,
        path: &str
    ) -> Result<PaletteKey, Box<dyn std::error::Error>> {
        if let Some(&key) = self.cache.palettes.get(path) {
            return Ok(key);
        }
        let palette = assets::load_palette(provider, path)?;
        let key = self.palettes.insert(palette);
        self.cache.palettes.insert(path.to_string(), key);
        Ok(key)
    }

    pub fn palette(&self, key: PaletteKey) -> Option<&Palette> {
        self.palettes.get(key)
    }

    pub fn palette_mut(&mut self, key: PaletteKey) -> Option<&mut Palette> {
        self.palettes.get_mut(key)
    }

    pub fn resolve_color(&self, color_ref: ColorRef) -> Color {
        match color_ref {
            ColorRef::Direct(c) => c,
            ColorRef::Indexed { palette, index } | ColorRef::Named { palette, index } => {
                self.palettes.get(palette)
                    .and_then(|p| p.get(index as usize))
                    .unwrap_or(Color::MAGENTA)
            }
        }
    }

    // ========================================================================
    // Atlas descriptors & PNG loading
    // ========================================================================

    /// Load an atlas descriptor from JSON and all its referenced PNG files.
    pub fn load_atlas_descriptor(
        &mut self,
        provider: &mut dyn ResourceProvider,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        config_path: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let descriptor = assets::load_atlas_descriptor(provider, config_path)?;
        let name = descriptor.name.clone();

        // Load all referenced PNG files
        for file_path in descriptor.referenced_files() {
            self.load_png(provider, rl, thread, &file_path, &descriptor)?;
        }

        self.descriptors.insert(name.clone(), descriptor);
        Ok(name)
    }

    /// Load a PNG file as an Atlas entry, using cache to deduplicate.
    /// tile_w/tile_h/cols come from the layer that references this PNG.
    fn load_png(
        &mut self,
        provider: &mut dyn ResourceProvider,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        file_path: &str,
        descriptor: &AtlasDescriptor,
    ) -> Result<AtlasKey, Box<dyn std::error::Error>> {
        if let Some(&key) = self.cache.pngs.get(file_path) {
            return Ok(key);
        }

        // Find tile size from the first layer referencing this file
        let source = descriptor.nodes.values()
            .flat_map(|n| n.layers.iter())
            .find(|l| l.source.file == file_path)
            .map(|l| &l.source)
            .ok_or_else(|| format!("No layer references file '{}'", file_path))?;

        let atlas = assets::load_png(
            provider, rl, thread, file_path,
            source.w, source.h, source.cols,
        )?;
        let key = self.atlases.insert(atlas);
        self.cache.pngs.insert(file_path.to_string(), key);
        Ok(key)
    }

    /// Get AtlasKey for a loaded PNG file path.
    pub fn png_key(&self, file_path: &str) -> Option<AtlasKey> {
        self.cache.pngs.get(file_path).copied()
    }

    pub fn atlas(&self, key: AtlasKey) -> Option<&Atlas> {
        self.atlases.get(key)
    }

    // ========================================================================
    // Glyphset composition
    // ========================================================================

    pub fn glyphset_size(&self, key: GlyphsetKey) -> Option<(u32, u32)> {
        self.glyphsets.get(key).map(|g| (g.tile_w, g.tile_h))
    }

    /// Compose a glyphset from tree nodes according to compose rules.
    pub fn compose_glyphset(
        &mut self,
        name: &str,
        rules: &[ComposeRule],
    ) -> Result<GlyphsetKey, String> {
        // Determine tile size from first rule
        let (tile_w, tile_h) = self.resolve_tile_size(rules)
            .ok_or_else(|| format!("Cannot determine tile size for glyphset '{}'", name))?;

        // Register a default glyph (we'll use byte 0 from the first available atlas)
        let default_global_id = self.find_default_global_id(rules);

        let mut glyphset = Glyphset::new(name.to_string(), tile_w, tile_h, default_global_id);

        for rule in rules {
            self.apply_compose_rule(&mut glyphset, rule)?;
        }

        glyphset.apply_fallbacks();
        Ok(self.glyphsets.insert(glyphset))
    }

    fn resolve_tile_size(&self, rules: &[ComposeRule]) -> Option<(u32, u32)> {
        for rule in rules {
            let nodes = self.tree.resolve_with_variants(&rule.node, &self.descriptors);
            if let Some(resolved) = nodes.first() {
                if let Some(size) = resolved.descriptor.tile_size() {
                    return Some(size);
                }
            }
        }
        None
    }

    fn find_default_global_id(&mut self, rules: &[ComposeRule]) -> u32 {
        // Phase 1: find the default sprite info (immutable borrows)
        let mut default_info: Option<(String, u32)> = None; // (file, sprite)
        'outer: for rule in rules {
            let nodes = self.tree.resolve_with_variants(&rule.node, &self.descriptors);
            for resolved in &nodes {
                if resolved.variant_name.is_some() { continue; }
                let default_byte = resolved.descriptor.default_byte;
                for layer in &resolved.node.layers {
                    for (byte, sprite) in layer.byte_sprite_pairs() {
                        if byte == default_byte {
                            default_info = Some((layer.source.file.clone(), sprite));
                            break 'outer;
                        }
                    }
                }
            }
        }

        // Phase 2: register (mutable borrow)
        if let Some((file, sprite)) = default_info {
            if let Some(&atlas_key) = self.cache.pngs.get(&file) {
                return self.global_registry.register_glyph(atlas_key, sprite);
            }
        }
        0
    }

    fn apply_compose_rule(
        &mut self,
        glyphset: &mut Glyphset,
        rule: &ComposeRule,
    ) -> Result<(), String> {
        // Phase 1: resolve and collect data (immutable borrow of self)
        let collected = self.collect_rule_data(rule)?;

        // Phase 2: populate LUTs (mutable borrow of self.global_registry + self.cache)
        for cv in &collected {
            let variant_id = match &cv.variant_name {
                Some(v) => glyphset.ensure_variant(v),
                None => 0,
            };
            for cl in &cv.layers {
                let atlas_key = self.cache.pngs.get(&cl.file)
                    .copied()
                    .ok_or_else(|| format!("PNG '{}' not loaded", cl.file))?;

                for &(byte, sprite) in &cl.pairs {
                    let dest_byte = if let Some(ref r) = rule.remap {
                        if byte < r.src[0] || byte > r.src[1] {
                            continue;
                        }
                        r.dst + (byte - r.src[0])
                    } else {
                        byte
                    };

                    if (dest_byte as usize) < 65536 {
                        let global_id = self.global_registry.register_glyph(atlas_key, sprite);
                        glyphset.luts[variant_id as usize][dest_byte as usize] = global_id;
                    }
                }
            }
        }
        Ok(())
    }

    fn collect_rule_data(&self, rule: &ComposeRule) -> Result<Vec<CollectedVariant>, String> {
        let mut collected = Vec::new();

        if let Some(ref variant_name) = rule.as_variant {
            let resolved = self.tree.resolve(&rule.node, &self.descriptors)
                .ok_or_else(|| format!("Node '{}' not found in virtual tree", rule.node))?;
            collected.push(CollectedVariant {
                variant_name: Some(variant_name.clone()),
                layers: Self::collect_layers(resolved.node),
            });
        } else {
            let nodes = self.tree.resolve_with_variants(&rule.node, &self.descriptors);
            if nodes.is_empty() {
                return Err(format!("Node '{}' not found in virtual tree", rule.node));
            }
            for resolved in &nodes {
                collected.push(CollectedVariant {
                    variant_name: resolved.variant_name.map(|s| s.to_string()),
                    layers: Self::collect_layers(resolved.node),
                });
            }
        }

        Ok(collected)
    }

    fn collect_layers(node: &AtlasNode) -> Vec<CollectedLayer> {
        node.layers.iter().map(|layer| {
            CollectedLayer {
                file: layer.source.file.clone(),
                pairs: layer.byte_sprite_pairs(),
            }
        }).collect()
    }
}
