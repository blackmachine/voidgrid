//! Input - РјРѕРґСѓР»СЊ РѕР±СЂР°Р±РѕС‚РєРё РїРѕР»СЊР·РѕРІР°С‚РµР»СЊСЃРєРѕРіРѕ РІРІРѕРґР° Рё window chrome
//! 
//! TODO:
//! - [ ] Separate Polling (Raw Input) from Events
//! - [ ] Create unified InputState structure
//! - [ ] Implement Event Queue (Click, Char, Resize)
//! - [ ] Decouple from Raylib (make backend-agnostic)

use raylib::prelude::*;

/// РџРѕР»СѓС‡РёС‚СЊ РіР»РѕР±Р°Р»СЊРЅСѓСЋ РїРѕР·РёС†РёСЋ РјС‹С€Рё РЅР° СЌРєСЂР°РЅРµ (window_pos + mouse_pos_in_window)
fn get_global_mouse_pos(rl: &RaylibHandle) -> (i32, i32) {
    let win_pos = rl.get_window_position();
    let mouse_pos = rl.get_mouse_position();
    (
        win_pos.x as i32 + mouse_pos.x as i32,
        win_pos.y as i32 + mouse_pos.y as i32,
    )
}

/// РљСЂР°Р№/СѓРіРѕР» РѕРєРЅР° РґР»СЏ resize
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResizeEdge {
    None,
    Left,
    Right,
    Top,
    Bottom,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl ResizeEdge {
    fn cursor_type(self) -> MouseCursor {
        match self {
            ResizeEdge::None => MouseCursor::MOUSE_CURSOR_DEFAULT,
            ResizeEdge::Left | ResizeEdge::Right => MouseCursor::MOUSE_CURSOR_RESIZE_EW,
            ResizeEdge::Top | ResizeEdge::Bottom => MouseCursor::MOUSE_CURSOR_RESIZE_NS,
            ResizeEdge::TopLeft | ResizeEdge::BottomRight => MouseCursor::MOUSE_CURSOR_RESIZE_NWSE,
            ResizeEdge::TopRight | ResizeEdge::BottomLeft => MouseCursor::MOUSE_CURSOR_RESIZE_NESW,
        }
    }
}

/// РЎРѕСЃС‚РѕСЏРЅРёРµ РѕРєРЅР° Рё СЃРёСЃС‚РµРјРЅС‹С… СЌР»РµРјРµРЅС‚РѕРІ СѓРїСЂР°РІР»РµРЅРёСЏ
pub struct WindowChrome {
    /// Р Р°Р·РјРµСЂ РєРЅРѕРїРѕРє
    button_size: i32,
    /// РЁРёСЂРёРЅР° drag handle
    drag_width: i32,
    /// РћС‚СЃС‚СѓРї РѕС‚ РєСЂР°СЏ
    margin: i32,
    /// РџСЂРµРґС‹РґСѓС‰РёР№ СЂР°Р·РјРµСЂ РѕРєРЅР° (РґР»СЏ РѕС‚СЃР»РµР¶РёРІР°РЅРёСЏ resize)
    prev_size: (i32, i32),
    /// Р¤Р»Р°Рі maximized (РѕРєРѕРЅРЅС‹Р№, СЃ С‚Р°СЃРєР±Р°СЂРѕРј)
    is_maximized: bool,
    /// Р¤Р»Р°Рі fullscreen (Р±РµР· С‚Р°СЃРєР±Р°СЂР°)
    is_fullscreen: bool,
    /// Р Р°Р·РјРµСЂ РѕРєРЅР° РґРѕ maximize/fullscreen (РґР»СЏ РІРѕСЃСЃС‚Р°РЅРѕРІР»РµРЅРёСЏ)
    windowed_size: (i32, i32),
    /// РџРѕР·РёС†РёСЏ РѕРєРЅР° РґРѕ maximize/fullscreen
    windowed_pos: (i32, i32),
    /// Dragging state
    is_dragging: bool,
    /// РЎРјРµС‰РµРЅРёРµ РѕС‚ РіР»РѕР±Р°Р»СЊРЅРѕР№ РїРѕР·РёС†РёРё РјС‹С€Рё РґРѕ Р»РµРІРѕРіРѕ РІРµСЂС…РЅРµРіРѕ СѓРіР»Р° РѕРєРЅР°
    drag_offset: (i32, i32),
    /// Resize state
    is_resizing: bool,
    resize_edge: ResizeEdge,
    resize_start_size: (i32, i32),
    resize_start_pos: (i32, i32),
    resize_start_mouse: (i32, i32),
    /// Р—РѕРЅР° РѕР±РЅР°СЂСѓР¶РµРЅРёСЏ РєСЂР°С‘РІ (РІ РїРёРєСЃРµР»СЏС…)
    edge_size: i32,
    /// РњРёРЅРёРјР°Р»СЊРЅС‹Р№ СЂР°Р·РјРµСЂ РѕРєРЅР°
    min_size: (i32, i32),
    /// Р’С‹СЃРѕС‚Р° Р·РѕРЅС‹ РґР»СЏ РїРѕРєР°Р·Р° chrome
    chrome_zone_height: i32,
    /// Chrome РІРёРґРёРј
    chrome_visible: bool,
}

impl WindowChrome {
    pub fn new(window_width: i32, window_height: i32) -> Self {
        Self {
            button_size: 24,
            drag_width: 60,
            margin: 4,
            prev_size: (window_width, window_height),
            is_maximized: false,
            is_fullscreen: false,
            windowed_size: (window_width, window_height),
            windowed_pos: (100, 100),
            is_dragging: false,
            drag_offset: (0, 0),
            is_resizing: false,
            resize_edge: ResizeEdge::None,
            resize_start_size: (0, 0),
            resize_start_pos: (0, 0),
            resize_start_mouse: (0, 0),
            edge_size: 6,
            min_size: (200, 100),
            chrome_zone_height: 40,
            chrome_visible: false,
        }
    }
    
    /// РџСЂРѕРІРµСЂРёС‚СЊ, Р±С‹Р»Рѕ Р»Рё РёР·РјРµРЅРµРЅРёРµ СЂР°Р·РјРµСЂР° РѕРєРЅР°
    pub fn check_resize(&mut self, rl: &RaylibHandle) -> Option<(i32, i32)> {
        let current = (rl.get_screen_width(), rl.get_screen_height());
        if current != self.prev_size {
            self.prev_size = current;
            Some(current)
        } else {
            None
        }
    }
    
    /// РџРѕР»СѓС‡РёС‚СЊ СЂРµРєРѕРјРµРЅРґСѓРµРјС‹Р№ СЂР°Р·РјРµСЂ Р±СѓС„РµСЂР° РґР»СЏ С‚РµРєСѓС‰РµРіРѕ РѕРєРЅР°
    pub fn recommended_buffer_size(&self, rl: &RaylibHandle, tile_w: u32, tile_h: u32) -> (u32, u32) {
        let screen_w = rl.get_screen_width() as u32;
        let screen_h = rl.get_screen_height() as u32;
        (screen_w / tile_w, screen_h / tile_h)
    }
    
    /// РћРїСЂРµРґРµР»РёС‚СЊ РєСЂР°Р№/СѓРіРѕР» РїРѕРґ РєСѓСЂСЃРѕСЂРѕРј
    fn detect_edge(&self, mouse_pos: Vector2, screen_w: i32, screen_h: i32) -> ResizeEdge {
        let mx = mouse_pos.x as i32;
        let my = mouse_pos.y as i32;
        let e = self.edge_size;
        
        let on_left = mx < e;
        let on_right = mx >= screen_w - e;
        let on_top = my < e;
        let on_bottom = my >= screen_h - e;
        
        match (on_left, on_right, on_top, on_bottom) {
            (true, _, true, _) => ResizeEdge::TopLeft,
            (true, _, _, true) => ResizeEdge::BottomLeft,
            (_, true, true, _) => ResizeEdge::TopRight,
            (_, true, _, true) => ResizeEdge::BottomRight,
            (true, _, _, _) => ResizeEdge::Left,
            (_, true, _, _) => ResizeEdge::Right,
            (_, _, true, _) => ResizeEdge::Top,
            (_, _, _, true) => ResizeEdge::Bottom,
            _ => ResizeEdge::None,
        }
    }
    
    /// РћР±СЂР°Р±РѕС‚Р°С‚СЊ РІРІРѕРґ Рё РІРµСЂРЅСѓС‚СЊ true РµСЃР»Рё РЅСѓР¶РЅРѕ Р·Р°РєСЂС‹С‚СЊ РѕРєРЅРѕ
    pub fn update(&mut self, rl: &mut RaylibHandle) -> bool {
        let mouse_pos = rl.get_mouse_position();
        let mouse_pressed = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
        let mouse_down = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
        let mouse_released = rl.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT);
        
        let screen_w = rl.get_screen_width();
        let screen_h = rl.get_screen_height();
        
        // РћР±РЅРѕРІР»СЏРµРј РІРёРґРёРјРѕСЃС‚СЊ chrome
        self.chrome_visible = mouse_pos.y < self.chrome_zone_height as f32 
            || self.is_dragging 
            || self.is_resizing;
        
        // РљРЅРѕРїРєР° Р·Р°РєСЂС‹С‚РёСЏ [X]
        let close_rect = Rectangle::new(
            (screen_w - self.margin - self.button_size) as f32,
            self.margin as f32,
            self.button_size as f32,
            self.button_size as f32,
        );
        
        // РљРЅРѕРїРєР° maximize [в›¶]
        let maximize_rect = Rectangle::new(
            (screen_w - self.margin * 2 - self.button_size * 2) as f32,
            self.margin as f32,
            self.button_size as f32,
            self.button_size as f32,
        );
        
        // РљРЅРѕРїРєР° minimize [_]
        let minimize_rect = Rectangle::new(
            (screen_w - self.margin * 3 - self.button_size * 3) as f32,
            self.margin as f32,
            self.button_size as f32,
            self.button_size as f32,
        );
        
        // Drag handle (СЃР»РµРІР° РѕС‚ РєРЅРѕРїРѕРє)
        let drag_rect = Rectangle::new(
            (screen_w - self.margin * 4 - self.button_size * 3 - self.drag_width) as f32,
            self.margin as f32,
            self.drag_width as f32,
            self.button_size as f32,
        );
        
        // РћРїСЂРµРґРµР»СЏРµРј РєСЂР°Р№ РґР»СЏ resize (С‚РѕР»СЊРєРѕ РµСЃР»Рё РЅРµ maximized/fullscreen)
        let edge = if !self.is_maximized && !self.is_fullscreen && !self.is_dragging {
            self.detect_edge(mouse_pos, screen_w, screen_h)
        } else {
            ResizeEdge::None
        };
        
        // РњРµРЅСЏРµРј РєСѓСЂСЃРѕСЂ
        if !self.is_resizing {
            rl.set_mouse_cursor(edge.cursor_type());
        }
        
        // РћР±СЂР°Р±РѕС‚РєР° resize
        if self.is_resizing {
            if mouse_released {
                self.is_resizing = false;
                rl.set_mouse_cursor(MouseCursor::MOUSE_CURSOR_DEFAULT);
            } else if mouse_down {
                let (global_mx, global_my) = get_global_mouse_pos(rl);
                let dmx = global_mx - self.resize_start_mouse.0;
                let dmy = global_my - self.resize_start_mouse.1;
                
                let (mut new_w, mut new_h) = self.resize_start_size;
                let (mut new_x, mut new_y) = self.resize_start_pos;
                
                match self.resize_edge {
                    ResizeEdge::Right => {
                        new_w += dmx;
                    }
                    ResizeEdge::Bottom => {
                        new_h += dmy;
                    }
                    ResizeEdge::Left => {
                        new_w -= dmx;
                        new_x += dmx;
                    }
                    ResizeEdge::Top => {
                        new_h -= dmy;
                        new_y += dmy;
                    }
                    ResizeEdge::BottomRight => {
                        new_w += dmx;
                        new_h += dmy;
                    }
                    ResizeEdge::BottomLeft => {
                        new_w -= dmx;
                        new_x += dmx;
                        new_h += dmy;
                    }
                    ResizeEdge::TopRight => {
                        new_w += dmx;
                        new_h -= dmy;
                        new_y += dmy;
                    }
                    ResizeEdge::TopLeft => {
                        new_w -= dmx;
                        new_x += dmx;
                        new_h -= dmy;
                        new_y += dmy;
                    }
                    ResizeEdge::None => {}
                }
                
                // РџСЂРёРјРµРЅСЏРµРј РјРёРЅРёРјР°Р»СЊРЅС‹Р№ СЂР°Р·РјРµСЂ
                if new_w < self.min_size.0 {
                    let diff = self.min_size.0 - new_w;
                    new_w = self.min_size.0;
                    // РљРѕСЂСЂРµРєС‚РёСЂСѓРµРј РїРѕР·РёС†РёСЋ РµСЃР»Рё resize СЃР»РµРІР°
                    if matches!(self.resize_edge, ResizeEdge::Left | ResizeEdge::TopLeft | ResizeEdge::BottomLeft) {
                        new_x -= diff;
                    }
                }
                if new_h < self.min_size.1 {
                    let diff = self.min_size.1 - new_h;
                    new_h = self.min_size.1;
                    // РљРѕСЂСЂРµРєС‚РёСЂСѓРµРј РїРѕР·РёС†РёСЋ РµСЃР»Рё resize СЃРІРµСЂС…Сѓ
                    if matches!(self.resize_edge, ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight) {
                        new_y -= diff;
                    }
                }
                
                rl.set_window_size(new_w, new_h);
                rl.set_window_position(new_x, new_y);
            }
        }
        // РћР±СЂР°Р±РѕС‚РєР° dragging
        else if self.is_dragging {
            if mouse_released {
                self.is_dragging = false;
            } else if mouse_down && !self.is_maximized && !self.is_fullscreen {
                let (global_mx, global_my) = get_global_mouse_pos(rl);
                let new_x = global_mx - self.drag_offset.0;
                let new_y = global_my - self.drag_offset.1;
                rl.set_window_position(new_x, new_y);
            }
        }
        
        if mouse_pressed {
            // РџСЂРёРѕСЂРёС‚РµС‚: РєРЅРѕРїРєРё > drag > resize
            if self.chrome_visible {
                if close_rect.check_collision_point_rec(mouse_pos) {
                    return true; // Р—Р°РєСЂС‹С‚СЊ
                }
                
                if maximize_rect.check_collision_point_rec(mouse_pos) {
                    self.toggle_maximize(rl);
                    return false;
                }
                
                if minimize_rect.check_collision_point_rec(mouse_pos) {
                    unsafe { raylib::ffi::MinimizeWindow(); }
                    return false;
                }
                
                if drag_rect.check_collision_point_rec(mouse_pos) && !self.is_maximized && !self.is_fullscreen {
                    self.is_dragging = true;
                    self.drag_offset = (mouse_pos.x as i32, mouse_pos.y as i32);
                    return false;
                }
            }
            
            // Resize Р·Р° РіСЂР°РЅРёС†С‹
            if edge != ResizeEdge::None {
                self.is_resizing = true;
                self.resize_edge = edge;
                self.resize_start_size = (screen_w, screen_h);
                let pos = rl.get_window_position();
                self.resize_start_pos = (pos.x as i32, pos.y as i32);
                self.resize_start_mouse = get_global_mouse_pos(rl);
            }
        }
        
        // F11 РґР»СЏ fullscreen (РїРѕРІРµСЂС… С‚Р°СЃРєР±Р°СЂР°)
        if rl.is_key_pressed(KeyboardKey::KEY_F11) {
            self.toggle_fullscreen(rl);
        }
        
        // F10 РґР»СЏ maximize (СЃ С‚Р°СЃРєР±Р°СЂРѕРј)
        if rl.is_key_pressed(KeyboardKey::KEY_F10) {
            self.toggle_maximize(rl);
        }
        
        // Escape РґР»СЏ РІС‹С…РѕРґР° РёР· maximize/fullscreen РёР»Рё Р·Р°РєСЂС‹С‚РёСЏ
        if rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
            if self.is_fullscreen {
                self.toggle_fullscreen(rl);
            } else if self.is_maximized {
                self.toggle_maximize(rl);
            } else {
                return true; // Р—Р°РєСЂС‹С‚СЊ
            }
        }
        
        false
    }
    
    fn toggle_maximize(&mut self, rl: &mut RaylibHandle) {
        if self.is_fullscreen {
            // РЎРЅР°С‡Р°Р»Р° РІС‹С…РѕРґРёРј РёР· fullscreen
            self.toggle_fullscreen(rl);
        }
        
        if self.is_maximized {
            // Р’РѕСЃСЃС‚Р°РЅР°РІР»РёРІР°РµРј РѕРєРЅРѕ
            rl.set_window_size(self.windowed_size.0, self.windowed_size.1);
            rl.set_window_position(self.windowed_pos.0, self.windowed_pos.1);
            self.is_maximized = false;
        } else {
            // РЎРѕС…СЂР°РЅСЏРµРј С‚РµРєСѓС‰РёРµ СЂР°Р·РјРµСЂС‹
            self.windowed_size = (rl.get_screen_width(), rl.get_screen_height());
            let pos = rl.get_window_position();
            self.windowed_pos = (pos.x as i32, pos.y as i32);
            
            // РџРѕР»СѓС‡Р°РµРј СЂР°Р·РјРµСЂ СЂР°Р±РѕС‡РµР№ РѕР±Р»Р°СЃС‚Рё РјРѕРЅРёС‚РѕСЂР°
            let monitor = unsafe { raylib::ffi::GetCurrentMonitor() };
            let monitor_w = unsafe { raylib::ffi::GetMonitorWidth(monitor) };
            let monitor_h = unsafe { raylib::ffi::GetMonitorHeight(monitor) };
            
            // РџСЂРёРјРµСЂРЅР°СЏ РІС‹СЃРѕС‚Р° С‚Р°СЃРєР±Р°СЂР°
            let taskbar_height = 40;
            
            // Р Р°Р·РІРѕСЂР°С‡РёРІР°РµРј РЅР° РІРµСЃСЊ СЌРєСЂР°РЅ (РѕРєРѕРЅРЅС‹Р№ СЂРµР¶РёРј, СЃ С‚Р°СЃРєР±Р°СЂРѕРј)
            rl.set_window_position(0, 0);
            rl.set_window_size(monitor_w, monitor_h - taskbar_height);
            
            self.is_maximized = true;
        }
    }
    
    fn toggle_fullscreen(&mut self, rl: &mut RaylibHandle) {
        if self.is_maximized {
            // РЎРЅР°С‡Р°Р»Р° РІС‹С…РѕРґРёРј РёР· maximize
            self.is_maximized = false;
        }
        
        if self.is_fullscreen {
            // Р’РѕСЃСЃС‚Р°РЅР°РІР»РёРІР°РµРј РѕРєРЅРѕ
            rl.set_window_size(self.windowed_size.0, self.windowed_size.1);
            rl.set_window_position(self.windowed_pos.0, self.windowed_pos.1);
            self.is_fullscreen = false;
        } else {
            // РЎРѕС…СЂР°РЅСЏРµРј С‚РµРєСѓС‰РёРµ СЂР°Р·РјРµСЂС‹ (РµСЃР»Рё РµС‰С‘ РЅРµ СЃРѕС…СЂР°РЅРµРЅС‹)
            if !self.is_maximized {
                self.windowed_size = (rl.get_screen_width(), rl.get_screen_height());
                let pos = rl.get_window_position();
                self.windowed_pos = (pos.x as i32, pos.y as i32);
            }
            
            // РџРѕР»СѓС‡Р°РµРј РїРѕР»РЅС‹Р№ СЂР°Р·РјРµСЂ РјРѕРЅРёС‚РѕСЂР°
            let monitor = unsafe { raylib::ffi::GetCurrentMonitor() };
            let monitor_w = unsafe { raylib::ffi::GetMonitorWidth(monitor) };
            let monitor_h = unsafe { raylib::ffi::GetMonitorHeight(monitor) };
            
            // Р Р°Р·РІРѕСЂР°С‡РёРІР°РµРј РЅР° РІРµСЃСЊ СЌРєСЂР°РЅ (РїРѕРІРµСЂС… С‚Р°СЃРєР±Р°СЂР°)
            rl.set_window_position(0, 0);
            rl.set_window_size(monitor_w, monitor_h);
            
            self.is_fullscreen = true;
        }
    }
    
    /// РћС‚СЂРёСЃРѕРІР°С‚СЊ СЌР»РµРјРµРЅС‚С‹ СѓРїСЂР°РІР»РµРЅРёСЏ РѕРєРЅРѕРј
    pub fn draw<D: RaylibDraw>(&self, d: &mut D) {
        // РќРµ СЂРёСЃСѓРµРј РµСЃР»Рё chrome СЃРєСЂС‹С‚
        if !self.chrome_visible {
            return;
        }
        
        // РСЃРїРѕР»СЊР·СѓРµРј unsafe РґР»СЏ РїРѕР»СѓС‡РµРЅРёСЏ СЂР°Р·РјРµСЂРѕРІ СЌРєСЂР°РЅР° С‡РµСЂРµР· FFI
        let screen_w = unsafe { raylib::ffi::GetScreenWidth() };
        
        // Р¤РѕРЅ РєРЅРѕРїРѕРє
        let bg_color = Color::new(40, 40, 40, 220);
        let hover_color = Color::new(80, 80, 80, 220);
        let close_hover_color = Color::new(200, 60, 60, 220);
        let drag_color = Color::new(60, 60, 60, 220);
        let drag_hover_color = Color::new(90, 90, 90, 220);
        
        let mouse_pos = unsafe {
            let pos = raylib::ffi::GetMousePosition();
            Vector2::new(pos.x, pos.y)
        };
        
        // Drag handle (СЃР»РµРІР° РѕС‚ РєРЅРѕРїРѕРє)
        let drag_x = screen_w - self.margin * 4 - self.button_size * 3 - self.drag_width;
        let drag_rect = Rectangle::new(
            drag_x as f32,
            self.margin as f32,
            self.drag_width as f32,
            self.button_size as f32,
        );
        
        let drag_bg = if drag_rect.check_collision_point_rec(mouse_pos) || self.is_dragging {
            drag_hover_color
        } else {
            drag_color
        };
        
        d.draw_rectangle(
            drag_x,
            self.margin,
            self.drag_width,
            self.button_size,
            drag_bg,
        );
        
        // Р РёСЃСѓРµРј С‚РѕС‡РєРё РЅР° drag handle
        let dots_y = self.margin + self.button_size / 2;
        for i in 0..3 {
            let dot_x = drag_x + 15 + i * 15;
            d.draw_circle(dot_x, dots_y - 4, 2.0, Color::LIGHTGRAY);
            d.draw_circle(dot_x, dots_y + 4, 2.0, Color::LIGHTGRAY);
        }
        
        // РљРЅРѕРїРєР° minimize [_]
        let min_x = screen_w - self.margin * 3 - self.button_size * 3;
        let min_rect = Rectangle::new(
            min_x as f32,
            self.margin as f32,
            self.button_size as f32,
            self.button_size as f32,
        );
        
        let min_bg = if min_rect.check_collision_point_rec(mouse_pos) {
            hover_color
        } else {
            bg_color
        };
        
        d.draw_rectangle(
            min_x,
            self.margin,
            self.button_size,
            self.button_size,
            min_bg,
        );
        
        // РРєРѕРЅРєР° minimize (Р»РёРЅРёСЏ РІРЅРёР·Сѓ)
        let mnx = min_x + self.button_size / 2;
        let mny = self.margin + self.button_size / 2;
        d.draw_line(mnx - 6, mny + 4, mnx + 6, mny + 4, Color::WHITE);
        
        // РљРЅРѕРїРєР° maximize
        let max_x = screen_w - self.margin * 2 - self.button_size * 2;
        let max_rect = Rectangle::new(
            max_x as f32,
            self.margin as f32,
            self.button_size as f32,
            self.button_size as f32,
        );
        
        let max_bg = if max_rect.check_collision_point_rec(mouse_pos) {
            hover_color
        } else {
            bg_color
        };
        
        d.draw_rectangle(
            max_x,
            self.margin,
            self.button_size,
            self.button_size,
            max_bg,
        );
        
        // РРєРѕРЅРєР° maximize/restore
        let mx = max_x + self.button_size / 2;
        let my = self.margin + self.button_size / 2;
        let s = 6;
        
        if self.is_maximized || self.is_fullscreen {
            // РРєРѕРЅРєР° restore (РґРІР° РїСЂСЏРјРѕСѓРіРѕР»СЊРЅРёРєР°)
            d.draw_rectangle_lines(mx - s + 2, my - s, s + 2, s + 2, Color::WHITE);
            d.draw_rectangle_lines(mx - s, my - s + 2, s + 2, s + 2, Color::WHITE);
        } else {
            // РРєРѕРЅРєР° maximize (РѕРґРёРЅ РїСЂСЏРјРѕСѓРіРѕР»СЊРЅРёРє)
            d.draw_rectangle_lines(mx - s, my - s, s * 2, s * 2, Color::WHITE);
        }
        
        // РљРЅРѕРїРєР° Р·Р°РєСЂС‹С‚РёСЏ [X]
        let close_x = screen_w - self.margin - self.button_size;
        let close_rect = Rectangle::new(
            close_x as f32,
            self.margin as f32,
            self.button_size as f32,
            self.button_size as f32,
        );
        
        let close_bg = if close_rect.check_collision_point_rec(mouse_pos) {
            close_hover_color
        } else {
            bg_color
        };
        
        d.draw_rectangle(
            close_x,
            self.margin,
            self.button_size,
            self.button_size,
            close_bg,
        );
        
        // РљСЂРµСЃС‚РёРє
        let cx = close_x + self.button_size / 2;
        let cy = self.margin + self.button_size / 2;
        let cs = 6;
        d.draw_line(cx - cs, cy - cs, cx + cs, cy + cs, Color::WHITE);
        d.draw_line(cx + cs, cy - cs, cx - cs, cy + cs, Color::WHITE);
    }
    
    /// РўРµРєСѓС‰РµРµ СЃРѕСЃС‚РѕСЏРЅРёРµ maximized
    pub fn is_maximized(&self) -> bool {
        self.is_maximized
    }
    
    /// РўРµРєСѓС‰РµРµ СЃРѕСЃС‚РѕСЏРЅРёРµ fullscreen
    pub fn is_fullscreen(&self) -> bool {
        self.is_fullscreen
    }
    
    /// РўРµРєСѓС‰РµРµ СЃРѕСЃС‚РѕСЏРЅРёРµ dragging
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }
    
    /// Chrome РІРёРґРёРј?
    pub fn is_chrome_visible(&self) -> bool {
        self.chrome_visible
    }
}

/// РРЅС„РѕСЂРјР°С†РёСЏ Рѕ РєР»РёРєРµ РІ Р±СѓС„РµСЂ
#[derive(Debug, Clone, Copy)]
pub struct BufferClick {
    pub x: u32,
    pub y: u32,
    pub button: MouseButton,
}

/// Р’СЃРїРѕРјРѕРіР°С‚РµР»СЊРЅС‹Рµ С„СѓРЅРєС†РёРё РґР»СЏ РІРІРѕРґР°
pub struct Input;

impl Input {
    /// РџРѕР»СѓС‡РёС‚СЊ РїРѕР·РёС†РёСЋ РјС‹С€Рё РєР°Рє (i32, i32)
    pub fn mouse_pos(rl: &RaylibHandle) -> (i32, i32) {
        let pos = rl.get_mouse_position();
        (pos.x as i32, pos.y as i32)
    }
    
    /// РџСЂРѕРІРµСЂРёС‚СЊ, РЅР°Р¶Р°С‚Р° Р»Рё РєРЅРѕРїРєР° РјС‹С€Рё
    pub fn mouse_pressed(rl: &RaylibHandle, button: MouseButton) -> bool {
        rl.is_mouse_button_pressed(button)
    }
    
    /// РџСЂРѕРІРµСЂРёС‚СЊ, СѓРґРµСЂР¶РёРІР°РµС‚СЃСЏ Р»Рё РєРЅРѕРїРєР° РјС‹С€Рё
    pub fn mouse_down(rl: &RaylibHandle, button: MouseButton) -> bool {
        rl.is_mouse_button_down(button)
    }
    
    /// РџРѕР»СѓС‡РёС‚СЊ РїСЂРѕРєСЂСѓС‚РєСѓ РєРѕР»С‘СЃРёРєР°
    pub fn mouse_wheel(rl: &RaylibHandle) -> f32 {
        rl.get_mouse_wheel_move()
    }
    
    /// РџСЂРѕРІРµСЂРёС‚СЊ РЅР°Р¶Р°С‚РёРµ РєР»Р°РІРёС€Рё
    pub fn key_pressed(rl: &RaylibHandle, key: KeyboardKey) -> bool {
        rl.is_key_pressed(key)
    }
    
    /// РџСЂРѕРІРµСЂРёС‚СЊ СѓРґРµСЂР¶Р°РЅРёРµ РєР»Р°РІРёС€Рё
    pub fn key_down(rl: &RaylibHandle, key: KeyboardKey) -> bool {
        rl.is_key_down(key)
    }
    
    /// РџСЂРѕРІРµСЂРёС‚СЊ, Р±С‹Р»Рё Р»Рё Р±СЂРѕС€РµРЅС‹ С„Р°Р№Р»С‹
    pub fn is_file_dropped(rl: &RaylibHandle) -> bool {
        rl.is_file_dropped()
    }
}

/// РРЅС„РѕСЂРјР°С†РёСЏ Рѕ drag-n-drop
#[derive(Debug, Clone, Default)]
pub struct DropZone {
    /// РџРѕСЃР»РµРґРЅРёР№ Р±СЂРѕС€РµРЅРЅС‹Р№ С„Р°Р№Р»
    pub last_dropped: Option<String>,
}

impl DropZone {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// РћР±РЅРѕРІРёС‚СЊ СЃРѕСЃС‚РѕСЏРЅРёРµ drop zone
    /// Р’РѕР·РІСЂР°С‰Р°РµС‚ Some(filename) РµСЃР»Рё С„Р°Р№Р» Р±С‹Р» Р±СЂРѕС€РµРЅ
    pub fn update(&mut self, events: &[crate::events::Event]) -> Option<String> {
        for event in events {
            if let crate::events::Event::FileDrop { path } = event {
                let filename = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path)
                    .to_string();
                
                self.last_dropped = Some(filename.clone());
                return Some(filename.clone());
            }
        }
        None
    }
    
    /// РџРѕР»СѓС‡РёС‚СЊ РёРјСЏ РїРѕСЃР»РµРґРЅРµРіРѕ Р±СЂРѕС€РµРЅРЅРѕРіРѕ С„Р°Р№Р»Р°
    pub fn last_file(&self) -> Option<&str> {
        self.last_dropped.as_deref()
    }
    
    /// РћС‡РёСЃС‚РёС‚СЊ РёРЅС„РѕСЂРјР°С†РёСЋ Рѕ РїРѕСЃР»РµРґРЅРµРј С„Р°Р№Р»Рµ
    pub fn clear(&mut self) {
        self.last_dropped = None;
    }
}

