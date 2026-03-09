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
    
    pub fn set_root(&mut self, buffer: Option<BufferKey>) -> NodeKey {
        let key = self.create_node(buffer);
        self.root = Some(key);
        key
    }

    pub fn attach(&mut self, buffer: Option<BufferKey>) -> NodeBuilder {
        let key = self.create_node(buffer);
        NodeBuilder {
            hierarchy: self,
            node_key: key,
        }
    }

    pub fn create_node(&mut self, buffer: Option<BufferKey>) -> NodeKey {
        self.nodes.insert(Node {
            buffer,
            ..Default::default()
        })
    }
    
    pub fn collect_render_list<F>(&self, get_info: F) -> Vec<RenderItem>
    where F: Fn(BufferKey) -> (u32, u32, u32, u32) {
        let mut list = Vec::new();
        if let Some(root) = self.root {
            let (cols, rows, tile_w, tile_h) = if let Some(buf) = self.nodes.get(root).and_then(|n| n.buffer) {
                get_info(buf)
            } else {
                (0, 0, 1, 1)
            };
            // Передаем root-параметры как родительские (parent_x=0, parent_y=0)
            self.process_node(root, 0, 0, cols as i32, rows as i32, tile_w as i32, tile_h as i32, 0, 1.0, &get_info, &mut list);
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
        parent_cols: i32,
        parent_rows: i32,
        parent_tile_w: i32,
        parent_tile_h: i32,
        parent_z: i32,
        parent_opacity: f32,
        get_info: &F,
        list: &mut Vec<RenderItem>
    ) where F: Fn(BufferKey) -> (u32, u32, u32, u32) {
        let node = match self.nodes.get(node_key) {
            Some(n) => n,
            None => return,
        };
        
        if !node.visible { return; }
        
        let (cols, rows, tile_w, tile_h) = if let Some(buf) = node.buffer {
            let (c, r, tw, th) = get_info(buf);
            (c as i32, r as i32, tw as i32, th as i32)
        } else {
            (0, 0, 1, 1) // фоллбэк для пустых нод
        };
        
        // Anchor (привязка к родителю) вычисляется в сетке родителя
        let (anchor_cols, anchor_rows) = node.anchor.offset(parent_cols, parent_rows);
        let anchor_px_x = anchor_cols * parent_tile_w;
        let anchor_px_y = anchor_rows * parent_tile_h;

        // Смещение (local_x/y) задано в тайлах родителя
        let local_px_x = node.local_x * parent_tile_w;
        let local_px_y = node.local_y * parent_tile_h;

        // Pivot (собственная точка привязки) вычисляется в собственной сетке
        let (pivot_cols, pivot_rows) = node.pivot.offset(cols, rows);
        let pivot_px_x = pivot_cols * tile_w;
        let pivot_px_y = pivot_rows * tile_h;

        let screen_x = parent_x + anchor_px_x + local_px_x - pivot_px_x;
        let screen_y = parent_y + anchor_px_y + local_px_y - pivot_px_y;
        
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
            self.process_node(child, screen_x, screen_y, cols, rows, tile_w, tile_h, z_index, opacity, get_info, list);
        }
    }
}

pub struct NodeBuilder<'a> {
    hierarchy: &'a mut Hierarchy,
    node_key: NodeKey,
}

impl<'a> NodeBuilder<'a> {
    pub fn to(self, parent: NodeKey) -> Self {
        if let Some(node) = self.hierarchy.nodes.get_mut(self.node_key) {
            node.parent = Some(parent);
        }
        if let Some(parent_node) = self.hierarchy.nodes.get_mut(parent) {
            parent_node.children.push(self.node_key);
        }
        self
    }

    pub fn at(self, x: i32, y: i32) -> Self {
        if let Some(node) = self.hierarchy.nodes.get_mut(self.node_key) {
            node.local_x = x;
            node.local_y = y;
        }
        self
    }

    pub fn with_z(self, policy: ZPolicy) -> Self {
        if let Some(node) = self.hierarchy.nodes.get_mut(self.node_key) {
            node.z_policy = policy;
        }
        self
    }

    pub fn anchor(self, anchor: Anchor) -> Self {
        if let Some(node) = self.hierarchy.nodes.get_mut(self.node_key) {
            node.anchor = anchor;
        }
        self
    }

    pub fn pivot(self, pivot: Anchor) -> Self {
        if let Some(node) = self.hierarchy.nodes.get_mut(self.node_key) {
            node.pivot = pivot;
        }
        self
    }

    pub fn key(self) -> NodeKey {
        self.node_key
    }
}