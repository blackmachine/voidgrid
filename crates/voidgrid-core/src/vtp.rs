use raylib::prelude::Color;
use crate::types::{BufferKey, Character, Blend, Transform};
use crate::grids::Grids;
use crate::text_ops::TextOps;
use std::collections::HashMap;

// Опкоды команд VTP
const CMD_SET_BUFFER: u8 = 0x01;
const CMD_SET_CURSOR: u8 = 0x02;
const CMD_SET_FG_COLOR: u8 = 0x03;
const CMD_SET_BG_COLOR: u8 = 0x04;
const CMD_SET_VARIANT: u8 = 0x05;
const CMD_PRINT_CHAR: u8 = 0x10;
const CMD_PRINT_STRING: u8 = 0x11;
const CMD_LOAD_SCENE: u8 = 0x20;

/// Состояние терминала VTP
pub struct VtpState {
    pub active_buffer: Option<BufferKey>,
    pub cursor_x: u32,
    pub cursor_y: u32,
    pub fg_color: Color,
    pub bg_color: Color,
    pub variant_id: u8,
}

impl Default for VtpState {
    fn default() -> Self {
        Self {
            active_buffer: None,
            cursor_x: 0,
            cursor_y: 0,
            fg_color: Color::WHITE,
            bg_color: Color::BLANK,
            variant_id: 0,
        }
    }
}

/// Парсер протокола VTP (State Machine)
pub struct VtpParser {
    pub state: VtpState,
    buffer: Vec<u8>,
}

impl VtpParser {
    pub fn new() -> Self {
        Self {
            state: VtpState::default(),
            buffer: Vec::new(),
        }
    }

    pub fn process(
        &mut self,
        grids: &mut Grids,
        buffer_map: &HashMap<String, BufferKey>,
        new_data: &[u8],
    ) {
        self.buffer.extend_from_slice(new_data);

        while !self.buffer.is_empty() {
            let opcode = self.buffer[0];
            
            // Вычисляем необходимую длину пакета для текущей команды
            let needed = match opcode {
                CMD_SET_BUFFER => {
                    if self.buffer.len() < 3 { break; }
                    let len = u16::from_le_bytes([self.buffer[1], self.buffer[2]]) as usize;
                    3 + len
                }
                CMD_SET_CURSOR => 5,
                CMD_SET_FG_COLOR => 5,
                CMD_SET_BG_COLOR => 5,
                CMD_SET_VARIANT => 2,
                CMD_PRINT_CHAR => 5,
                CMD_PRINT_STRING => {
                    if self.buffer.len() < 3 { break; }
                    let len = u16::from_le_bytes([self.buffer[1], self.buffer[2]]) as usize;
                    3 + len
                }
                CMD_LOAD_SCENE => {
                    if self.buffer.len() < 5 { break; }
                    let size = u32::from_le_bytes([self.buffer[1], self.buffer[2], self.buffer[3], self.buffer[4]]) as usize;
                    5 + size
                }
                _ => {
                    // Неизвестный опкод, пропускаем 1 байт
                    self.buffer.remove(0);
                    continue;
                }
            };

            // Если данных недостаточно, ждем следующей порции
            if self.buffer.len() < needed {
                break;
            }

            // Выполняем команду
            match opcode {
                CMD_SET_BUFFER => {
                    let len = u16::from_le_bytes([self.buffer[1], self.buffer[2]]) as usize;
                    let name_bytes = &self.buffer[3..3 + len];
                    let name = String::from_utf8_lossy(name_bytes);
                    if let Some(&key) = buffer_map.get(name.as_ref()) {
                        self.state.active_buffer = Some(key);
                        self.state.cursor_x = 0;
                        self.state.cursor_y = 0;
                    }
                }
                CMD_SET_CURSOR => {
                    self.state.cursor_x = u16::from_le_bytes([self.buffer[1], self.buffer[2]]) as u32;
                    self.state.cursor_y = u16::from_le_bytes([self.buffer[3], self.buffer[4]]) as u32;
                }
                CMD_SET_FG_COLOR => {
                    self.state.fg_color = Color::new(self.buffer[1], self.buffer[2], self.buffer[3], self.buffer[4]);
                }
                CMD_SET_BG_COLOR => {
                    self.state.bg_color = Color::new(self.buffer[1], self.buffer[2], self.buffer[3], self.buffer[4]);
                }
                CMD_SET_VARIANT => {
                    self.state.variant_id = self.buffer[1];
                }
                CMD_PRINT_CHAR => {
                    let code = u32::from_le_bytes([self.buffer[1], self.buffer[2], self.buffer[3], self.buffer[4]]);
                    if let Some(key) = self.state.active_buffer {
                        if let Some(ch) = char::from_u32(code) {
                            grids.set_char(key, self.state.cursor_x, self.state.cursor_y, Character::full(ch as u32, self.state.variant_id, self.state.fg_color, self.state.bg_color, Blend::Alpha, Blend::Alpha, Transform::default(), None));
                            self.state.cursor_x += 1;
                        }
                    }
                }
                CMD_PRINT_STRING => {
                    let len = u16::from_le_bytes([self.buffer[1], self.buffer[2]]) as usize;
                    let text_bytes = &self.buffer[3..3 + len];
                    let text = String::from_utf8_lossy(text_bytes);
                    if let Some(key) = self.state.active_buffer {
                        grids.print(key).at(self.state.cursor_x, self.state.cursor_y).color(self.state.fg_color, self.state.bg_color).write(text.as_ref());
                        self.state.cursor_x += text.chars().count() as u32;
                    }
                }
                _ => {}
            }

            // Удаляем обработанные байты
            self.buffer.drain(0..needed);
        }
    }
}