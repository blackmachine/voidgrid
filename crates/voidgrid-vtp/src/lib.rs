#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VtpColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Debug, Clone)]
pub enum VtpCommand {
    SetBuffer(String),
    SetCursor { x: u32, y: u32 },
    SetFgColor(VtpColor),
    SetBgColor(VtpColor),
    SetVariant(u8),
    PrintChar(u32),
    PrintString(String),
    LoadScene { size: u32, data: Vec<u8> },
}

pub struct VtpParser {
    buffer: Vec<u8>,
}

impl Default for VtpParser {
    fn default() -> Self {
        Self::new()
    }
}

impl VtpParser {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn push_bytes(&mut self, new_data: &[u8]) {
        self.buffer.extend_from_slice(new_data);
    }

    pub fn next_command(&mut self) -> Option<VtpCommand> {
        if self.buffer.is_empty() {
            return None;
        }

        let opcode = self.buffer[0];

        let needed = match opcode {
            0x01 => {
                if self.buffer.len() < 3 { return None; }
                3 + u16::from_le_bytes([self.buffer[1], self.buffer[2]]) as usize
            }
            0x02 | 0x03 | 0x04 | 0x10 => 5,
            0x05 => 2,
            0x11 => {
                if self.buffer.len() < 3 { return None; }
                3 + u16::from_le_bytes([self.buffer[1], self.buffer[2]]) as usize
            }
            0x20 => {
                if self.buffer.len() < 5 { return None; }
                5 + u32::from_le_bytes([self.buffer[1], self.buffer[2], self.buffer[3], self.buffer[4]]) as usize
            }
            _ => {
                self.buffer.remove(0);
                return self.next_command();
            }
        };

        if self.buffer.len() < needed {
            return None;
        }

        let command = match opcode {
            0x01 => {
                let len = u16::from_le_bytes([self.buffer[1], self.buffer[2]]) as usize;
                let name = String::from_utf8_lossy(&self.buffer[3..3 + len]).into_owned();
                VtpCommand::SetBuffer(name)
            }
            0x02 => {
                let x = u16::from_le_bytes([self.buffer[1], self.buffer[2]]) as u32;
                let y = u16::from_le_bytes([self.buffer[3], self.buffer[4]]) as u32;
                VtpCommand::SetCursor { x, y }
            }
            0x03 => VtpCommand::SetFgColor(VtpColor { r: self.buffer[1], g: self.buffer[2], b: self.buffer[3], a: self.buffer[4] }),
            0x04 => VtpCommand::SetBgColor(VtpColor { r: self.buffer[1], g: self.buffer[2], b: self.buffer[3], a: self.buffer[4] }),
            0x05 => VtpCommand::SetVariant(self.buffer[1]),
            0x10 => VtpCommand::PrintChar(u32::from_le_bytes([self.buffer[1], self.buffer[2], self.buffer[3], self.buffer[4]])),
            0x11 => {
                let len = u16::from_le_bytes([self.buffer[1], self.buffer[2]]) as usize;
                let text = String::from_utf8_lossy(&self.buffer[3..3 + len]).into_owned();
                VtpCommand::PrintString(text)
            }
            0x20 => {
                let size = u32::from_le_bytes([self.buffer[1], self.buffer[2], self.buffer[3], self.buffer[4]]) as usize;
                VtpCommand::LoadScene { size: size as u32, data: self.buffer[5..5 + size].to_vec() }
            }
            _ => unreachable!(),
        };

        self.buffer.drain(0..needed);
        Some(command)
    }
}
