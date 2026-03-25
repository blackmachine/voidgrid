#![allow(dead_code)]

use std::collections::HashMap;
use anyhow::{Context, Result};
use raylib::prelude::*;
use serde::Deserialize;

use crate::VoidGrid;
use crate::asset_manager::ComposeRule;
use crate::hierarchy::{Hierarchy, Anchor, NodeKey};
use crate::resource_pack::ResourceProvider;
use crate::types::{BufferKey, ShaderKey, GlyphsetKey};

#[derive(Deserialize)]
struct ManifestDTO {
    name: String,
    version: String,
    assets: AssetConfigDTO,
    #[serde(default)]
    mounts: HashMap<String, String>,
    glyphsets: HashMap<String, GlyphsetConfigDTO>,
    scene: SceneDTO,
}

#[derive(Deserialize)]
struct AssetConfigDTO {
    atlases: HashMap<String, String>,
    shaders: HashMap<String, String>,
    #[serde(default)]
    scripts: HashMap<String, String>,
}

#[derive(Deserialize)]
struct GlyphsetConfigDTO {
    compose: Vec<ComposeRule>,
}

#[derive(Deserialize)]
struct SceneDTO {
    nodes: Vec<NodeDTO>,
}

#[derive(Deserialize)]
struct NodeDTO {
    id: String,
    #[serde(rename = "type")]
    type_name: String,
    name: String,
    width: u32,
    height: u32,
    glyphset: String,
    z_index: Option<i32>,
    dynamic: Option<bool>,
    parent: Option<String>,
    anchor: Option<String>,
    local_x: Option<i32>,
    local_y: Option<i32>,
    shader: Option<String>,
    shaders: Option<Vec<String>>,
    shader_padding: Option<u32>,
}

pub struct LoadedPack {
    pub buffers: HashMap<String, BufferKey>,
    pub scripts: HashMap<String, String>,
}

pub struct PackLoader;

impl PackLoader {
    pub fn load_pack(
        vg: &mut VoidGrid,
        hierarchy: &mut Hierarchy,
        provider: &mut dyn ResourceProvider,
        manifest_path: &str,
        rl: &mut RaylibHandle,
        thread: &RaylibThread
    ) -> Result<LoadedPack> {
        let content = provider.read_string(manifest_path)
            .context("Failed to read manifest file")?;
        let manifest: ManifestDTO = crate::assets::parse_config(manifest_path, &content)
            .map_err(|e| anyhow::anyhow!("Failed to parse manifest: {}", e))?;

        // 1. Load atlas descriptors (and their PNGs)
        for (name, path) in &manifest.assets.atlases {
            let loaded_name = vg.grids.assets.load_atlas_descriptor(provider, rl, thread, path)
                .map_err(|e| anyhow::anyhow!("Failed to load atlas '{}': {}", name, e))?;
            // If the descriptor's internal name differs from the manifest key,
            // register under the manifest key as well
            if &loaded_name != name {
                if let Some(desc) = vg.grids.assets.descriptors.get(&loaded_name).cloned() {
                    vg.grids.assets.descriptors.insert(name.clone(), desc);
                }
            }
        }

        // 2. Load shaders
        let mut shader_map: HashMap<String, ShaderKey> = HashMap::new();
        for (name, path) in &manifest.assets.shaders {
            let key = vg.grids.assets.load_shader(provider, rl, thread, path)
                .map_err(|e| anyhow::anyhow!("Failed to load shader '{}': {}", name, e))?;
            shader_map.insert(name.clone(), key);
        }

        // 3. Build mount tree
        for (mount_path, descriptor_name) in &manifest.mounts {
            vg.grids.assets.tree.mount(mount_path, descriptor_name);
        }

        // 4. Compose glyphsets
        let mut glyphset_map: HashMap<String, GlyphsetKey> = HashMap::new();
        for (name, config) in &manifest.glyphsets {
            let gs_key = vg.grids.assets.compose_glyphset(name, &config.compose)
                .map_err(|e| anyhow::anyhow!("Failed to compose glyphset '{}': {}", name, e))?;
            glyphset_map.insert(name.clone(), gs_key);
        }

        // 5. Build scene
        let mut node_keys: HashMap<String, NodeKey> = HashMap::new();
        let mut buffer_keys: HashMap<String, BufferKey> = HashMap::new();

        for node in &manifest.scene.nodes {
            let gs_key = *glyphset_map.get(&node.glyphset)
                .ok_or_else(|| anyhow::anyhow!("Glyphset '{}' not found for node '{}'", node.glyphset, node.name))?;

            let mut buf_builder = vg.grids.buffer(&node.name, node.width, node.height, gs_key);
            if let Some(z) = node.z_index { buf_builder = buf_builder.z_index(z); }
            if let Some(d) = node.dynamic { buf_builder = buf_builder.dynamic(d); }
            let buf_key = buf_builder.build();
            buffer_keys.insert(node.id.clone(), buf_key);

            let mut builder = hierarchy.attach(Some(buf_key));

            if let (Some(x), Some(y)) = (node.local_x, node.local_y) {
                builder = builder.at(x, y);
            }

            if let Some(anchor_str) = &node.anchor {
                let anchor = match anchor_str.as_str() {
                    "TopLeft" => Anchor::TopLeft,
                    "TopCenter" => Anchor::TopCenter,
                    "TopRight" => Anchor::TopRight,
                    "CenterLeft" => Anchor::CenterLeft,
                    "Center" => Anchor::Center,
                    "CenterRight" => Anchor::CenterRight,
                    "BottomLeft" => Anchor::BottomLeft,
                    "BottomCenter" => Anchor::BottomCenter,
                    "BottomRight" => Anchor::BottomRight,
                    _ => Anchor::TopLeft,
                };
                builder = builder.anchor(anchor);
            }

            let node_key = if let Some(parent_id) = &node.parent {
                if let Some(parent_key) = node_keys.get(parent_id) {
                    builder.to(*parent_key).key()
                } else {
                    eprintln!("Warning: Parent '{}' not found for node '{}'.", parent_id, node.name);
                    builder.key()
                }
            } else {
                let key = builder.key();
                hierarchy.root = Some(key);
                key
            };
            node_keys.insert(node.id.clone(), node_key);

            let padding = node.shader_padding.unwrap_or(0);

            if let Some(shader_name) = &node.shader {
                if let Some(shader_key) = shader_map.get(shader_name) {
                    vg.renderer.attach_shader(rl, thread, &vg.grids, buf_key, *shader_key, padding);
                }
            }

            if let Some(shader_names) = &node.shaders {
                for shader_name in shader_names {
                    if let Some(shader_key) = shader_map.get(shader_name) {
                        vg.renderer.attach_shader(rl, thread, &vg.grids, buf_key, *shader_key, padding);
                    } else {
                        eprintln!("Warning: Shader '{}' not found in manifest assets.", shader_name);
                    }
                }
            }
        }

        // 6. Load scripts
        let mut loaded_scripts = HashMap::new();
        for (name, path) in &manifest.assets.scripts {
            let code = provider.read_string(path)
                .map_err(|e| anyhow::anyhow!("Failed to load script '{}' at {}: {}", name, path, e))?;
            loaded_scripts.insert(name.clone(), code);
        }

        Ok(LoadedPack {
            buffers: buffer_keys,
            scripts: loaded_scripts,
        })
    }
}
