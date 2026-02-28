use crate::types::{AtlasKey, BufferKey, Character};

pub struct Buffer {
    pub name: String,
    pub w: u32,
    pub h: u32,
    pub z_index: i32,
    pub visible: bool,
    pub opacity: f32,
    pub default_variant: Option<String>,
    pub(crate) data: Vec<Character>,
    pub(crate) atlas: AtlasKey,
}

impl std::fmt::Debug for Buffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Buffer")
            .field("name", &self.name)
            .field("w", &self.w)
            .field("h", &self.h)
            .finish()
    }
}

impl Buffer {
    pub fn new(name: impl Into<String>, w: u32, h: u32, atlas: AtlasKey, z_index: i32, fill: Character) -> Self {
        let size = (w * h) as usize;
        Self {
            name: name.into(),
            w, h, z_index,
            visible: true,
            opacity: 1.0,
            default_variant: None,
            data: vec![fill; size],
            atlas,
        }
    }

    #[inline]
    pub(crate) fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.w || y >= self.h { return None; }
        Some((y * self.w + x) as usize)
    }

    pub fn set(&mut self, x: u32, y: u32, ch: Character) {
        if let Some(i) = self.index(x, y) {
            self.data[i] = ch;
        }
    }

    pub fn get(&self, x: u32, y: u32) -> Option<Character> {
        self.index(x, y).map(|i| self.data[i].clone())
    }

    pub(crate) fn get_char_ref(&self, x: u32, y: u32) -> Option<&Character> {
        self.index(x, y).map(|i| &self.data[i])
    }
    
    pub fn clear(&mut self, fill: Character) {
        self.data.fill(fill);
    }

    pub fn atlas(&self) -> AtlasKey {
        self.atlas
    }
    
    pub fn set_atlas(&mut self, atlas: AtlasKey) {
        self.atlas = atlas;
    }
}

#[derive(Debug, Clone)]
pub struct Attachment {
    pub parent: BufferKey,
    pub child: BufferKey,
    pub x: u32,
    pub y: u32,
    pub z_index: i32,
}

#[derive(Debug, Clone)]
pub struct OrphanedChild {
    pub position: (u32, u32),
    pub buffer: BufferKey,
}