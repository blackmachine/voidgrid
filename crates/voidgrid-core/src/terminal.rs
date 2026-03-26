use std::collections::HashMap;
use raylib::prelude::Color;
use crate::grids::Grids;
use crate::types::{BufferKey, Character, Blend, Transform};

#[derive(Debug, Clone)]
pub enum Action {
    SetBuffer(String),
    SetCursor(u32, u32),
    SetFgColor(Color),
    SetBgColor(Color),
    SetVariant(u8),
    SetVariantByName(String),
    PrintChar(u32),
    PrintString(String),
    ClearBuffer(String),
    SetBufferVisible(String, bool),
    SetBufferOpacity(String, f32),
    SetBufferZ(String, i32),
}

pub struct TerminalState {
    pub active_buffer: Option<BufferKey>,
    pub cursor_x: u32,
    pub cursor_y: u32,
    pub fg_color: Color,
    pub bg_color: Color,
    pub variant_id: u8,
    pub buffer_map: HashMap<String, BufferKey>,
}

impl TerminalState {
    pub fn new() -> Self {
        Self {
            active_buffer: None,
            cursor_x: 0,
            cursor_y: 0,
            fg_color: Color::WHITE,
            bg_color: Color::BLANK,
            variant_id: 0,
            buffer_map: HashMap::new(),
        }
    }

    pub fn register_buffers(&mut self, buffers: HashMap<String, BufferKey>) {
        self.buffer_map.extend(buffers);
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
            Action::SetFgColor(c) => self.fg_color = c,
            Action::SetBgColor(c) => self.bg_color = c,
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
                        grids.set_char(
                            key, 
                            self.cursor_x, 
                            self.cursor_y, 
                            Character::full(ch as u32, self.variant_id, self.fg_color, self.bg_color, Blend::Alpha, Blend::Alpha, Transform::default(), None)
                        );
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
                            grids.set_char(
                                key,
                                self.cursor_x,
                                self.cursor_y,
                                Character::full(
                                    ch as u32, self.variant_id,
                                    self.fg_color, self.bg_color,
                                    Blend::Alpha, Blend::Alpha,
                                    Transform::default(), None,
                                ),
                            );
                            self.cursor_x += 1;
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
