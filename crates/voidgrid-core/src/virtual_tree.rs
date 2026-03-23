use std::collections::HashMap;
use crate::atlas::{AtlasDescriptor, AtlasNode};

/// A resolved node from the virtual tree: an atlas descriptor + the internal node path.
pub struct ResolvedNode<'a> {
    pub descriptor_name: &'a str,
    pub descriptor: &'a AtlasDescriptor,
    pub node: &'a AtlasNode,
    /// The variant name if this is a variant node (e.g., "inverted" from "/:inverted")
    pub variant_name: Option<&'a str>,
}

/// Virtual tree of mounted atlas descriptors.
/// Flat map of mount paths → descriptor names.
///
/// Example:
///   "/fonts/crt"    → "crt"
///   "/icons/noise"  → "noiseicons"
///
/// Resolution: given a query path like "/fonts/crt/:inverted",
/// finds mount at "/fonts/crt", looks up descriptor "crt",
/// resolves internal node "/:inverted".
pub struct VirtualTree {
    /// mount_path → descriptor_name
    mounts: HashMap<String, String>,
}

impl VirtualTree {
    pub fn new() -> Self {
        Self {
            mounts: HashMap::new(),
        }
    }

    pub fn mount(&mut self, path: &str, descriptor_name: &str) {
        let clean = path.trim_end_matches('/');
        self.mounts.insert(clean.to_string(), descriptor_name.to_string());
    }

    /// Resolve a query path to a descriptor node.
    ///
    /// Path format examples:
    ///   "/fonts/crt"           → mount "/fonts/crt", node "/"
    ///   "/fonts/crt/:inverted" → mount "/fonts/crt", node "/:inverted"
    ///   "/fonts/crt/cyrillic"  → mount "/fonts/crt/cyrillic", node "/"
    ///                            OR mount "/fonts/crt", node "/cyrillic" (if child node exists)
    ///   "/icons/noise/mediaplay" → mount "/icons/noise", node "/mediaplay"
    pub fn resolve<'a>(
        &'a self,
        path: &str,
        descriptors: &'a HashMap<String, AtlasDescriptor>,
    ) -> Option<ResolvedNode<'a>> {
        let clean = path.trim_end_matches('/');

        // Try exact mount match first
        if let Some(desc_name) = self.mounts.get(clean) {
            if let Some(desc) = descriptors.get(desc_name) {
                if let Some(node) = desc.nodes.get("/") {
                    return Some(ResolvedNode {
                        descriptor_name: desc_name,
                        descriptor: desc,
                        node,
                        variant_name: None,
                    });
                }
            }
        }

        // Try progressively shorter prefixes
        let mut prefix = clean;
        while let Some(slash_pos) = prefix.rfind('/') {
            let remainder = &clean[slash_pos..];
            prefix = &clean[..slash_pos];
            if prefix.is_empty() { continue; }

            if let Some(desc_name) = self.mounts.get(prefix) {
                if let Some(desc) = descriptors.get(desc_name) {
                    // remainder is like "/:inverted" or "/mediaplay" or "/sub/path"
                    // Try as internal node path
                    if let Some(node) = desc.nodes.get(remainder) {
                        // Get variant name from the descriptor's owned key
                        let variant_name = desc.nodes.keys()
                            .find(|k| k.as_str() == remainder)
                            .and_then(|k| k.strip_prefix("/:"));
                        return Some(ResolvedNode {
                            descriptor_name: desc_name,
                            descriptor: desc,
                            node,
                            variant_name,
                        });
                    }
                }
            }
        }

        None
    }

    /// Resolve a mount path and return ALL nodes for it:
    /// the root node "/" plus all variant nodes "/:*".
    /// This is used when composing a glyphset from a mount point
    /// to automatically include all variants.
    pub fn resolve_with_variants<'a>(
        &'a self,
        path: &str,
        descriptors: &'a HashMap<String, AtlasDescriptor>,
    ) -> Vec<ResolvedNode<'a>> {
        let clean = path.trim_end_matches('/');
        let mut results = Vec::new();

        // Handle variant-specific paths (e.g., "/fonts/crt/:inverted")
        if clean.contains("/:") {
            if let Some(resolved) = self.resolve(clean, descriptors) {
                results.push(resolved);
            }
            return results;
        }

        // Find the descriptor for this mount
        let desc_name = match self.mounts.get(clean) {
            Some(n) => n,
            None => {
                // Try as a child node of a parent mount
                if let Some(resolved) = self.resolve(clean, descriptors) {
                    results.push(resolved);
                }
                return results;
            }
        };

        let desc = match descriptors.get(desc_name) {
            Some(d) => d,
            None => return results,
        };

        // Add root node and all variants
        for (key, node) in &desc.nodes {
            if key == "/" {
                results.push(ResolvedNode {
                    descriptor_name: desc_name,
                    descriptor: desc,
                    node,
                    variant_name: None,
                });
            } else if let Some(variant) = key.strip_prefix("/:") {
                results.push(ResolvedNode {
                    descriptor_name: desc_name,
                    descriptor: desc,
                    node,
                    variant_name: Some(variant),
                });
            }
            // Child nodes (like "/mediaplay") are NOT included — they're separate mount points
        }

        results
    }

    pub fn mounts(&self) -> &HashMap<String, String> {
        &self.mounts
    }
}
