use raylib::prelude::{Color, Rectangle, BlendMode};
use slotmap::new_key_type;

// ============================================================================
// Ключи (SlotMap)
// ============================================================================

new_key_type! {
    /// Ключ для доступа к атласу
    pub struct AtlasKey;
    /// Ключ для доступа к буферу
    pub struct BufferKey;
    /// Ключ для доступа к палитре
    pub struct PaletteKey;
    /// Ключ для доступа к шейдеру
    pub struct ShaderKey;
    /// Ключ для доступа к глифсету
    pub struct GlyphsetKey;
}

// ============================================================================
// Blend Mode
// ============================================================================

/// Режим наложения (обёртка над raylib BlendMode)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum Blend {
    #[default]
    Alpha,
    Additive,
    Multiplied,
    AddColors,
    SubtractColors,
}

impl Blend {
    pub fn to_ffi(self) -> i32 {
        match self {
            Blend::Alpha => BlendMode::BLEND_ALPHA as i32,
            Blend::Additive => BlendMode::BLEND_ADDITIVE as i32,
            Blend::Multiplied => BlendMode::BLEND_MULTIPLIED as i32,
            Blend::AddColors => BlendMode::BLEND_ADD_COLORS as i32,
            Blend::SubtractColors => BlendMode::BLEND_SUBTRACT_COLORS as i32,
        }
    }
}

// ============================================================================
// Transform (Rotation + Flip)
// ============================================================================

/// Поворот глифа (по часовой стрелке)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum Rotation {
    #[default]
    None,    // 0°
    Cw90,    // 90°
    Cw180,   // 180°
    Cw270,   // 270°
}

impl Rotation {
    /// Угол в градусах для raylib
    pub fn degrees(self) -> f32 {
        match self {
            Rotation::None => 0.0,
            Rotation::Cw90 => 90.0,
            Rotation::Cw180 => 180.0,
            Rotation::Cw270 => 270.0,
        }
    }
}

/// Трансформация глифа (поворот + отзеркаливание)
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Transform {
    pub rotation: Rotation,
    pub flip_h: bool,
    pub flip_v: bool,
}

impl Transform {
    pub fn new() -> Self {
        Self::default()
    }
    
    pub fn rotated(rotation: Rotation) -> Self {
        Self { rotation, flip_h: false, flip_v: false }
    }
    
    pub fn flipped_h() -> Self {
        Self { rotation: Rotation::None, flip_h: true, flip_v: false }
    }
    
    pub fn flipped_v() -> Self {
        Self { rotation: Rotation::None, flip_h: false, flip_v: true }
    }
    
    pub fn with_rotation(mut self, rotation: Rotation) -> Self {
        self.rotation = rotation;
        self
    }
    
    pub fn with_flip_h(mut self, flip: bool) -> Self {
        self.flip_h = flip;
        self
    }
    
    pub fn with_flip_v(mut self, flip: bool) -> Self {
        self.flip_v = flip;
        self
    }
    
    /// Применить трансформацию к source rectangle
    pub fn apply_to_src(&self, src: Rectangle) -> Rectangle {
        let mut w = src.width;
        let mut h = src.height;
        
        if self.flip_h { w = -w; }
        if self.flip_v { h = -h; }
        
        Rectangle::new(src.x, src.y, w, h)
    }
}

// ============================================================================
// Направление записи
// ============================================================================

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum WriteDirection {
    #[default]
    Right,
    Down,
    Left,
    Up,
}

impl WriteDirection {
    pub fn delta(self) -> (i32, i32) {
        match self {
            WriteDirection::Right => (1, 0),
            WriteDirection::Down => (0, 1),
            WriteDirection::Left => (-1, 0),
            WriteDirection::Up => (0, -1),
        }
    }
    
    pub fn rotation(self) -> Rotation {
        match self {
            WriteDirection::Right => Rotation::None,
            WriteDirection::Down => Rotation::Cw90,
            WriteDirection::Left => Rotation::Cw180,
            WriteDirection::Up => Rotation::Cw270,
        }
    }
}

// ============================================================================
// Маска и Цвет
// ============================================================================

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Mask {
    pub atlas: AtlasKey,
    pub glyph: u32,
}

impl Mask {
    pub fn new(atlas: AtlasKey, glyph: u32) -> Self {
        Self { atlas, glyph }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ColorRef {
    Direct(Color),
    Indexed { palette: PaletteKey, index: u8 },
    Named { palette: PaletteKey, index: u8 },
}

impl Default for ColorRef {
    fn default() -> Self { Self::Direct(Color::WHITE) }
}

impl From<Color> for ColorRef {
    fn from(c: Color) -> Self { Self::Direct(c) }
}

// ============================================================================
// Character
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct Character {
    pub code: u32,
    pub variant_id: u8,
    pub fcolor: Color,
    pub bcolor: Color,
    pub fg_ref: Option<u16>,
    pub bg_ref: Option<u16>,
    pub fg_blend: Blend,
    pub bg_blend: Blend,
    pub transform: Transform,
    pub mask: Option<Mask>,
}

impl Character {
    pub fn new(code: u32, variant_id: u8, fcolor: Color, bcolor: Color) -> Self {
        Self::full(code, variant_id, fcolor, bcolor, Blend::Alpha, Blend::Alpha, Transform::default(), None)
    }

    pub fn full(code: u32, variant_id: u8, fcolor: Color, bcolor: Color, fg_blend: Blend, bg_blend: Blend, transform: Transform, mask: Option<Mask>) -> Self {
        Self { code, variant_id, fcolor, bcolor, fg_ref: None, bg_ref: None, fg_blend, bg_blend, transform, mask }
    }

    pub fn blank(default_code: u32) -> Self {
        Self::new(default_code, 0, Color::DARKGRAY, Color::BLANK)
    }
}