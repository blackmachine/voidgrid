use std::collections::HashMap;
use raylib::prelude::Color;
use crate::grids::Grids;
use crate::types::{BufferKey, PaletteKey, Character, Blend, Transform};

#[derive(Debug, Clone)]
pub enum Action {
    SetBuffer(String),
    SetCursor(u32, u32),
    SetFgColor(Color),
    SetBgColor(Color),
    SetFgIndexed(u16),
    SetBgIndexed(u16),
    SetVariant(u8),
    SetVariantByName(String),
    PrintChar(u32),
    PrintString(String),
    ClearBuffer(String),
    SetBufferVisible(String, bool),
    SetBufferOpacity(String, f32),
    SetBufferZ(String, i32),
    SetBufferPalette(String, String),
    PaletteSetColor(String, u16, u8, u8, u8, u8),
    PaletteCycle(String, u16, u16),
}

pub struct TerminalState {
    pub active_buffer: Option<BufferKey>,
    pub cursor_x: u32,
    pub cursor_y: u32,
    pub fg_color: Color,
    pub bg_color: Color,
    pub fg_ref: Option<u16>,
    pub bg_ref: Option<u16>,
    pub variant_id: u8,
    pub buffer_map: HashMap<String, BufferKey>,
    pub palette_map: HashMap<String, PaletteKey>,
}

impl TerminalState {
    pub fn new() -> Self {
        Self {
            active_buffer: None,
            cursor_x: 0,
            cursor_y: 0,
            fg_color: Color::WHITE,
            bg_color: Color::BLANK,
            fg_ref: None,
            bg_ref: None,
            variant_id: 0,
            buffer_map: HashMap::new(),
            palette_map: HashMap::new(),
        }
    }

    pub fn register_buffers(&mut self, buffers: HashMap<String, BufferKey>) {
        self.buffer_map.extend(buffers);
    }

    pub fn register_palettes(&mut self, palettes: HashMap<String, PaletteKey>) {
        self.palette_map.extend(palettes);
    }

    pub fn apply_action(&mut self, grids: &mut Grids, action: Action) {
        match action {
            Action::SetBuffer(name) => {
                if let Some(&key) = self.buffer_map.get(&name) {
                    self.active_buffer = Some(key);
                    self.cursor_x = 0;
                    self.cursor_y = 0;
                }
            }
            Action::SetCursor(x, y) => {
                self.cursor_x = x;
                self.cursor_y = y;
            }
            Action::SetFgColor(c) => { self.fg_color = c; self.fg_ref = None; }
            Action::SetBgColor(c) => { self.bg_color = c; self.bg_ref = None; }
            Action::SetFgIndexed(idx) => {
                self.fg_ref = Some(idx);
                // Resolve color from active buffer's palette
                if let Some(buf_key) = self.active_buffer {
                    if let Some(buf) = grids.get(buf_key) {
                        if let Some(pal_key) = buf.palette() {
                            if let Some(c) = grids.assets.palette(pal_key).and_then(|p| p.get(idx as usize)) {
                                self.fg_color = c;
                            }
                        }
                    }
                }
            }
            Action::SetBgIndexed(idx) => {
                self.bg_ref = Some(idx);
                if let Some(buf_key) = self.active_buffer {
                    if let Some(buf) = grids.get(buf_key) {
                        if let Some(pal_key) = buf.palette() {
                            if let Some(c) = grids.assets.palette(pal_key).and_then(|p| p.get(idx as usize)) {
                                self.bg_color = c;
                            }
                        }
                    }
                }
            }
            Action::SetVariant(v) => self.variant_id = v,
            Action::SetVariantByName(name) => {
                if let Some(buf_key) = self.active_buffer {
                    if let Some(buf) = grids.buffers.get(buf_key) {
                        let gs_key = buf.glyphset();
                        if let Some(gs) = grids.assets.glyphsets.get(gs_key) {
                            if let Some(&id) = gs.variant_names.get(&name) {
                                self.variant_id = id;
                            } else {
                                self.variant_id = 0; // Фолбэк на дефолт, если опечатались в названии
                            }
                        }
                    }
                }
            }
            Action::PrintChar(code) => {
                if let Some(key) = self.active_buffer {
                    if let Some(ch) = char::from_u32(code) {
                        let mut character = Character::full(ch as u32, self.variant_id, self.fg_color, self.bg_color, Blend::Alpha, Blend::Alpha, Transform::default(), None);
                        character.fg_ref = self.fg_ref;
                        character.bg_ref = self.bg_ref;
                        grids.set_char(key, self.cursor_x, self.cursor_y, character);
                        self.cursor_x += 1;
                    }
                }
            }
            Action::PrintString(text) => {
                if let Some(key) = self.active_buffer {
                    let start_x = self.cursor_x;
                    for ch in text.chars() {
                        if ch == '\n' {
                            self.cursor_y += 1;
                            self.cursor_x = start_x;
                        } else {
                            let mut character = Character::full(
                                ch as u32, self.variant_id,
                                self.fg_color, self.bg_color,
                                Blend::Alpha, Blend::Alpha,
                                Transform::default(), None,
                            );
                            character.fg_ref = self.fg_ref;
                            character.bg_ref = self.bg_ref;
                            grids.set_char(key, self.cursor_x, self.cursor_y, character);
                            self.cursor_x += 1;
                        }
                    }
                }
            }
            Action::SetBufferPalette(buf_name, pal_name) => {
                if let Some(&buf_key) = self.buffer_map.get(&buf_name) {
                    let pal_key = if pal_name.is_empty() {
                        None
                    } else {
                        self.palette_map.get(&pal_name).copied()
                    };
                    if let Some(buf) = grids.get_mut(buf_key) {
                        buf.set_palette(pal_key);
                    }
                    // Refresh colors if palette is set
                    if let Some(pk) = pal_key {
                        if let Some(palette) = grids.assets.palette(pk).cloned() {
                            if let Some(buf) = grids.get_mut(buf_key) {
                                buf.refresh_palette_colors(&palette);
                            }
                        }
                    }
                }
            }
            Action::PaletteSetColor(pal_name, index, r, g, b, a) => {
                if let Some(&pal_key) = self.palette_map.get(&pal_name) {
                    use crate::palette::PaletteColor;
                    if let Some(pal) = grids.assets.palette_mut(pal_key) {
                        pal.set_color(index as usize, PaletteColor::new(r, g, b, a));
                    }
                    // Refresh all buffers using this palette
                    if let Some(palette) = grids.assets.palette(pal_key).cloned() {
                        for (_, &bk) in &self.buffer_map {
                            if let Some(buf) = grids.get_mut(bk) {
                                if buf.palette() == Some(pal_key) {
                                    buf.refresh_palette_colors(&palette);
                                }
                            }
                        }
                    }
                }
            }
            Action::PaletteCycle(pal_name, start, end) => {
                if let Some(&pal_key) = self.palette_map.get(&pal_name) {
                    if let Some(pal) = grids.assets.palette_mut(pal_key) {
                        pal.cycle(start as usize, end as usize);
                    }
                    // Refresh all buffers using this palette
                    if let Some(palette) = grids.assets.palette(pal_key).cloned() {
                        for (_, &bk) in &self.buffer_map {
                            if let Some(buf) = grids.get_mut(bk) {
                                if buf.palette() == Some(pal_key) {
                                    buf.refresh_palette_colors(&palette);
                                }
                            }
                        }
                    }
                }
            }
            Action::ClearBuffer(name) => {
                if let Some(&key) = self.buffer_map.get(&name) {
                    grids.clear_buffer(key);
                }
            }
            Action::SetBufferVisible(name, visible) => {
                if let Some(&key) = self.buffer_map.get(&name) {
                    grids.set_visible(key, visible);
                }
            }
            Action::SetBufferOpacity(name, opacity) => {
                if let Some(&key) = self.buffer_map.get(&name) {
                    grids.set_opacity(key, opacity);
                }
            }
            Action::SetBufferZ(name, z) => {
                if let Some(&key) = self.buffer_map.get(&name) {
                    grids.set_buffer_z(key, z);
                }
            }
        }
    }
}
