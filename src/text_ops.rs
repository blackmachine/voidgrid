use raylib::prelude::Color;
use crate::grids::Grids;
use crate::types::{BufferKey, Blend, Mask, WriteDirection, Transform, Character};

/// Трейт для операций вывода текста
pub trait TextOps {
    fn write_string(
        &mut self,
        buffer: BufferKey,
        x: u32,
        y: u32,
        text: &str,
        fcolor: Color,
        bcolor: Color,
    );

    fn write_string_full(
        &mut self,
        buffer: BufferKey,
        x: u32,
        y: u32,
        text: &str,
        fcolor: Color,
        bcolor: Color,
        fg_blend: Blend,
        bg_blend: Blend,
        base_transform: Transform,
        mask: Option<Mask>,
        direction: WriteDirection,
        auto_rotate: bool,
    );

    fn write_string_variant(
        &mut self,
        buffer: BufferKey,
        x: u32,
        y: u32,
        text: &str,
        fcolor: Color,
        bcolor: Color,
        variant: &str,
    );

    fn put_icon(
        &mut self,
        buffer: BufferKey,
        x: u32,
        y: u32,
        icon_name: &str,
        fcolor: Color,
        bcolor: Color,
    );
}

impl TextOps for Grids {
    fn write_string(
        &mut self,
        buffer: BufferKey,
        x: u32,
        y: u32,
        text: &str,
        fcolor: Color,
        bcolor: Color,
    ) {
        self.write_string_full(buffer, x, y, text, fcolor, bcolor, 
            Blend::Alpha, Blend::Alpha, Transform::default(), None, 
            WriteDirection::Right, false);
    }

    fn write_string_full(
        &mut self,
        buffer: BufferKey,
        x: u32,
        y: u32,
        text: &str,
        fcolor: Color,
        bcolor: Color,
        fg_blend: Blend,
        bg_blend: Blend,
        base_transform: Transform,
        mask: Option<Mask>,
        direction: WriteDirection,
        auto_rotate: bool,
    ) {
        // Получаем buffer и его default_variant_id
        let (_glyphset_key, default_variant_id) = match self.buffers.get(buffer) {
            Some(b) => (b.glyphset(), b.default_variant_id),
            None => return,
        };
        
        let (dx, dy) = direction.delta();
        let mut cx = x as i32;
        let mut cy = y as i32;
        
        // Определяем трансформацию для символов
        let char_transform = if auto_rotate {
            Transform {
                rotation: direction.rotation(),
                flip_h: base_transform.flip_h,
                flip_v: base_transform.flip_v,
            }
        } else {
            base_transform
        };
        
        // Собираем символы для записи
        let chars: Vec<(u32, u32, Character)> = text.chars()
            .filter_map(|ch| {
                if ch == '\n' {
                    match direction {
                        WriteDirection::Right => { cy += 1; cx = x as i32; }
                        WriteDirection::Left => { cy -= 1; cx = x as i32; }
                        WriteDirection::Down => { cx -= 1; cy = y as i32; }
                        WriteDirection::Up => { cx += 1; cy = y as i32; }
                    }
                    None
                } else {
                    let code = ch as u32;
                    let pos_x = cx as u32;
                    let pos_y = cy as u32;
                    cx += dx;
                    cy += dy;
                    Some((pos_x, pos_y, Character::full(
                        code, default_variant_id, fcolor, bcolor, fg_blend, bg_blend, char_transform, mask
                    )))
                }
            })
            .collect();
        
        // Записываем в буфер
        if let Some(buf) = self.buffers.get_mut(buffer) {
            for (x, y, ch) in chars {
                buf.set(x, y, ch);
            }
        }
    }

    fn write_string_variant(
        &mut self,
        buffer: BufferKey,
        x: u32,
        y: u32,
        text: &str,
        fcolor: Color,
        bcolor: Color,
        variant: &str,
    ) {
        let glyphset_key = match self.buffers.get(buffer) {
            Some(b) => b.glyphset(),
            None => return,
        };
        let glyphset = match self.glyphsets.get(glyphset_key) {
            Some(g) => g,
            None => return,
        };
        
        let mut cx = x;
        let mut cy = y;
        
        let variant_id = *glyphset.variant_names.get(variant).unwrap_or(&0);

        let chars: Vec<(u32, u32, Character)> = text.chars()
            .filter_map(|ch| {
                if ch == '\n' {
                    cy += 1;
                    cx = x;
                    None
                } else {
                    let code = ch as u32;
                    let pos_x = cx;
                    let pos_y = cy;
                    cx += 1;
                    Some((pos_x, pos_y, Character::full(
                        code, variant_id, fcolor, bcolor, Blend::Alpha, Blend::Alpha, Transform::default(), None
                    )))
                }
            })
            .collect();
        
        if let Some(buf) = self.buffers.get_mut(buffer) {
            for (x, y, ch) in chars {
                buf.set(x, y, ch);
            }
        }
    }

    fn put_icon(
        &mut self,
        buffer: BufferKey,
        x: u32,
        y: u32,
        icon_name: &str,
        fcolor: Color,
        bcolor: Color,
    ) {
        let (glyphset_key, default_variant_id) = match self.buffers.get(buffer) {
            Some(b) => (b.glyphset(), b.default_variant_id),
            None => return,
        };
        
        if let Some(code) = self.resolve_code(glyphset_key, icon_name) {
             if let Some(buf) = self.buffers.get_mut(buffer) {
                buf.set(x, y, Character::new(code, default_variant_id, fcolor, bcolor));
             }
        }
    }
}