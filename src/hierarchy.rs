use slotmap::{new_key_type, SlotMap};
use crate::types::BufferKey;

new_key_type! {
    pub struct NodeKey;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

impl Anchor {
    pub fn offset(&self, w: i32, h: i32) -> (i32, i32) {
        match self {
            Anchor::TopLeft => (0, 0),
            Anchor::TopCenter => (w / 2, 0),
            Anchor::TopRight => (w, 0),
            Anchor::CenterLeft => (0, h / 2),
            Anchor::Center => (w / 2, h / 2),
            Anchor::CenterRight => (w, h / 2),
            Anchor::BottomLeft => (0, h),
            Anchor::BottomCenter => (w / 2, h),
            Anchor::BottomRight => (w, h),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZPolicy {
    Relative(i32), // Parent Z + Local Z
    Absolute(i32), // Exact Z
    Inherit,       // Parent Z
}

impl Default for ZPolicy {
    fn default() -> Self {
        ZPolicy::Relative(0)
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    pub buffer: Option<BufferKey>,
    pub parent: Option<NodeKey>,
    pub children: Vec<NodeKey>,
    
    // Transform
    pub local_x: i32,
    pub local_y: i32,
    pub anchor: Anchor, // Point on parent
    pub pivot: Anchor,  // Point on self
    
    // Sorting
    pub z_policy: ZPolicy,
    
    // State
    pub visible: bool,
    pub opacity: f32,
}

impl Default for Node {
    fn default() -> Self {
        Self {
            buffer: None,
            parent: None,
            children: Vec::new(),
            local_x: 0,
            local_y: 0,
            anchor: Anchor::TopLeft,
            pivot: Anchor::TopLeft,
            z_policy: ZPolicy::default(),
            visible: true,
            opacity: 1.0,
        }
    }
}

/// Элемент, готовый к отрисовке (результат Layout Pass)
#[derive(Debug, Clone)]
pub struct RenderItem {
    pub buffer: BufferKey,
    pub screen_x: i32,
    pub screen_y: i32,
    pub z_index: i32,
    pub opacity: f32,
}

pub struct Hierarchy {
    pub nodes: SlotMap<NodeKey, Node>,
    pub root: Option<NodeKey>,
}

impl Hierarchy {
    pub fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            root: None,
        }
    }
    
    pub fn create_node(&mut self, buffer: Option<BufferKey>) -> NodeKey {
        self.nodes.insert(Node {
            buffer,
            ..Default::default()
        })
    }
    
    pub fn collect_render_list<F>(&self, get_size: F) -> Vec<RenderItem>
    where F: Fn(BufferKey) -> (u32, u32) {
        let mut list = Vec::new();
        if let Some(root) = self.root {
            // Предполагаем, что root находится в (0,0) и имеет размер буфера (если есть)
            let (w, h) = if let Some(buf) = self.nodes.get(root).and_then(|n| n.buffer) {
                get_size(buf)
            } else {
                (0, 0)
            };
            
            self.process_node(root, 0, 0, w as i32, h as i32, 0, 1.0, &get_size, &mut list);
        }
        // Сортируем по Z-index для правильного порядка отрисовки
        list.sort_by_key(|item| item.z_index);
        list
    }
    
    #[allow(clippy::too_many_arguments)]
    fn process_node<F>(
        &self,
        node_key: NodeKey,
        parent_x: i32,
        parent_y: i32,
        parent_w: i32,
        parent_h: i32,
        parent_z: i32,
        parent_opacity: f32,
        get_size: &F,
        list: &mut Vec<RenderItem>
    ) where F: Fn(BufferKey) -> (u32, u32) {
        let node = match self.nodes.get(node_key) {
            Some(n) => n,
            None => return,
        };
        
        if !node.visible { return; }
        
        let (w, h) = if let Some(buf) = node.buffer {
            let (bw, bh) = get_size(buf);
            (bw as i32, bh as i32)
        } else {
            (0, 0)
        };
        
        let (anchor_x, anchor_y) = node.anchor.offset(parent_w, parent_h);
        let (pivot_x, pivot_y) = node.pivot.offset(w, h);
        
        let screen_x = parent_x + anchor_x + node.local_x - pivot_x;
        let screen_y = parent_y + anchor_y + node.local_y - pivot_y;
        
        let z_index = match node.z_policy {
            ZPolicy::Relative(z) => parent_z + z,
            ZPolicy::Absolute(z) => z,
            ZPolicy::Inherit => parent_z,
        };
        
        let opacity = parent_opacity * node.opacity;
        
        if let Some(buffer) = node.buffer {
            list.push(RenderItem {
                buffer,
                screen_x,
                screen_y,
                z_index,
                opacity,
            });
        }
        
        for &child in &node.children {
            self.process_node(child, screen_x, screen_y, w, h, z_index, opacity, get_size, list);
        }
    }
}