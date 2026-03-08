use crate::types::{GlyphsetKey, Character};

pub struct Buffer {
    pub name: String,
    pub w: u32,
    pub h: u32,
    pub z_index: i32,
    pub visible: bool,
    pub dynamic: bool,
    pub opacity: f32,
    pub default_variant_id: u8,
    pub(crate) data: Vec<Character>,
    pub(crate) glyphset: GlyphsetKey,
    pub(crate) dirty: bool,
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
    pub fn new(name: impl Into<String>, w: u32, h: u32, glyphset: GlyphsetKey, z_index: i32, fill: Character) -> Self {
        let size = (w * h) as usize;
        Self {
            name: name.into(),
            w, h, z_index,
            visible: true,
            dynamic: false,
            opacity: 1.0,
            default_variant_id: 0,
            data: vec![fill; size],
            dirty: true,
            glyphset,
        }
    }

    #[inline]
    pub(crate) fn index(&self, x: u32, y: u32) -> Option<usize> {
        if x >= self.w || y >= self.h { return None; }
        Some((y * self.w + x) as usize)
    }

    pub fn set(&mut self, x: u32, y: u32, ch: Character) {
        if let Some(i) = self.index(x, y) {
            if self.data[i] != ch {
                self.dirty = true;
                self.data[i] = ch;
            }
        }
    }

    pub fn get(&self, x: u32, y: u32) -> Option<Character> {
        self.index(x, y).map(|i| self.data[i].clone())
    }

    pub(crate) fn get_char_ref(&self, x: u32, y: u32) -> Option<&Character> {
        self.index(x, y).map(|i| &self.data[i])
    }
    
    pub fn clear(&mut self, fill: Character) {
        self.dirty = true;
        self.data.fill(fill);
    }

    pub fn glyphset(&self) -> GlyphsetKey {
        self.glyphset
    }
    
    pub fn set_glyphset(&mut self, glyphset: GlyphsetKey) {
        self.dirty = true;
        self.glyphset = glyphset;
    }
}