//! Input - модуль обработки пользовательского ввода и window chrome
//! 
//! TODO:
//! - [ ] Separate Polling (Raw Input) from Events
//! - [ ] Create unified InputState structure
//! - [ ] Implement Event Queue (Click, Char, Resize)
//! - [ ] Decouple from Raylib (make backend-agnostic)

use raylib::prelude::*;

/// Получить глобальную позицию мыши на экране (window_pos + mouse_pos_in_window)
fn get_global_mouse_pos(rl: &RaylibHandle) -> (i32, i32) {
    let win_pos = rl.get_window_position();
    let mouse_pos = rl.get_mouse_position();
    (
        win_pos.x as i32 + mouse_pos.x as i32,
        win_pos.y as i32 + mouse_pos.y as i32,
    )
}

/// Край/угол окна для resize
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

/// Состояние окна и системных элементов управления
pub struct WindowChrome {
    /// Размер кнопок
    button_size: i32,
    /// Ширина drag handle
    drag_width: i32,
    /// Отступ от края
    margin: i32,
    /// Предыдущий размер окна (для отслеживания resize)
    prev_size: (i32, i32),
    /// Флаг maximized (оконный, с таскбаром)
    is_maximized: bool,
    /// Флаг fullscreen (без таскбара)
    is_fullscreen: bool,
    /// Размер окна до maximize/fullscreen (для восстановления)
    windowed_size: (i32, i32),
    /// Позиция окна до maximize/fullscreen
    windowed_pos: (i32, i32),
    /// Dragging state
    is_dragging: bool,
    /// Смещение от глобальной позиции мыши до левого верхнего угла окна
    drag_offset: (i32, i32),
    /// Resize state
    is_resizing: bool,
    resize_edge: ResizeEdge,
    resize_start_size: (i32, i32),
    resize_start_pos: (i32, i32),
    resize_start_mouse: (i32, i32),
    /// Зона обнаружения краёв (в пикселях)
    edge_size: i32,
    /// Минимальный размер окна
    min_size: (i32, i32),
    /// Высота зоны для показа chrome
    chrome_zone_height: i32,
    /// Chrome видим
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
    
    /// Проверить, было ли изменение размера окна
    pub fn check_resize(&mut self, rl: &RaylibHandle) -> Option<(i32, i32)> {
        let current = (rl.get_screen_width(), rl.get_screen_height());
        if current != self.prev_size {
            self.prev_size = current;
            Some(current)
        } else {
            None
        }
    }
    
    /// Получить рекомендуемый размер буфера для текущего окна
    pub fn recommended_buffer_size(&self, rl: &RaylibHandle, tile_w: u32, tile_h: u32) -> (u32, u32) {
        let screen_w = rl.get_screen_width() as u32;
        let screen_h = rl.get_screen_height() as u32;
        (screen_w / tile_w, screen_h / tile_h)
    }
    
    /// Определить край/угол под курсором
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
    
    /// Обработать ввод и вернуть true если нужно закрыть окно
    pub fn update(&mut self, rl: &mut RaylibHandle) -> bool {
        let mouse_pos = rl.get_mouse_position();
        let mouse_pressed = rl.is_mouse_button_pressed(MouseButton::MOUSE_BUTTON_LEFT);
        let mouse_down = rl.is_mouse_button_down(MouseButton::MOUSE_BUTTON_LEFT);
        let mouse_released = rl.is_mouse_button_released(MouseButton::MOUSE_BUTTON_LEFT);
        
        let screen_w = rl.get_screen_width();
        let screen_h = rl.get_screen_height();
        
        // Обновляем видимость chrome
        self.chrome_visible = mouse_pos.y < self.chrome_zone_height as f32 
            || self.is_dragging 
            || self.is_resizing;
        
        // Кнопка закрытия [X]
        let close_rect = Rectangle::new(
            (screen_w - self.margin - self.button_size) as f32,
            self.margin as f32,
            self.button_size as f32,
            self.button_size as f32,
        );
        
        // Кнопка maximize [⛶]
        let maximize_rect = Rectangle::new(
            (screen_w - self.margin * 2 - self.button_size * 2) as f32,
            self.margin as f32,
            self.button_size as f32,
            self.button_size as f32,
        );
        
        // Кнопка minimize [_]
        let minimize_rect = Rectangle::new(
            (screen_w - self.margin * 3 - self.button_size * 3) as f32,
            self.margin as f32,
            self.button_size as f32,
            self.button_size as f32,
        );
        
        // Drag handle (слева от кнопок)
        let drag_rect = Rectangle::new(
            (screen_w - self.margin * 4 - self.button_size * 3 - self.drag_width) as f32,
            self.margin as f32,
            self.drag_width as f32,
            self.button_size as f32,
        );
        
        // Определяем край для resize (только если не maximized/fullscreen)
        let edge = if !self.is_maximized && !self.is_fullscreen && !self.is_dragging {
            self.detect_edge(mouse_pos, screen_w, screen_h)
        } else {
            ResizeEdge::None
        };
        
        // Меняем курсор
        if !self.is_resizing {
            rl.set_mouse_cursor(edge.cursor_type());
        }
        
        // Обработка resize
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
                
                // Применяем минимальный размер
                if new_w < self.min_size.0 {
                    let diff = self.min_size.0 - new_w;
                    new_w = self.min_size.0;
                    // Корректируем позицию если resize слева
                    if matches!(self.resize_edge, ResizeEdge::Left | ResizeEdge::TopLeft | ResizeEdge::BottomLeft) {
                        new_x -= diff;
                    }
                }
                if new_h < self.min_size.1 {
                    let diff = self.min_size.1 - new_h;
                    new_h = self.min_size.1;
                    // Корректируем позицию если resize сверху
                    if matches!(self.resize_edge, ResizeEdge::Top | ResizeEdge::TopLeft | ResizeEdge::TopRight) {
                        new_y -= diff;
                    }
                }
                
                rl.set_window_size(new_w, new_h);
                rl.set_window_position(new_x, new_y);
            }
        }
        // Обработка dragging
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
            // Приоритет: кнопки > drag > resize
            if self.chrome_visible {
                if close_rect.check_collision_point_rec(mouse_pos) {
                    return true; // Закрыть
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
            
            // Resize за границы
            if edge != ResizeEdge::None {
                self.is_resizing = true;
                self.resize_edge = edge;
                self.resize_start_size = (screen_w, screen_h);
                let pos = rl.get_window_position();
                self.resize_start_pos = (pos.x as i32, pos.y as i32);
                self.resize_start_mouse = get_global_mouse_pos(rl);
            }
        }
        
        // F11 для fullscreen (поверх таскбара)
        if rl.is_key_pressed(KeyboardKey::KEY_F11) {
            self.toggle_fullscreen(rl);
        }
        
        // F10 для maximize (с таскбаром)
        if rl.is_key_pressed(KeyboardKey::KEY_F10) {
            self.toggle_maximize(rl);
        }
        
        // Escape для выхода из maximize/fullscreen или закрытия
        if rl.is_key_pressed(KeyboardKey::KEY_ESCAPE) {
            if self.is_fullscreen {
                self.toggle_fullscreen(rl);
            } else if self.is_maximized {
                self.toggle_maximize(rl);
            } else {
                return true; // Закрыть
            }
        }
        
        false
    }
    
    fn toggle_maximize(&mut self, rl: &mut RaylibHandle) {
        if self.is_fullscreen {
            // Сначала выходим из fullscreen
            self.toggle_fullscreen(rl);
        }
        
        if self.is_maximized {
            // Восстанавливаем окно
            rl.set_window_size(self.windowed_size.0, self.windowed_size.1);
            rl.set_window_position(self.windowed_pos.0, self.windowed_pos.1);
            self.is_maximized = false;
        } else {
            // Сохраняем текущие размеры
            self.windowed_size = (rl.get_screen_width(), rl.get_screen_height());
            let pos = rl.get_window_position();
            self.windowed_pos = (pos.x as i32, pos.y as i32);
            
            // Получаем размер рабочей области монитора
            let monitor = unsafe { raylib::ffi::GetCurrentMonitor() };
            let monitor_w = unsafe { raylib::ffi::GetMonitorWidth(monitor) };
            let monitor_h = unsafe { raylib::ffi::GetMonitorHeight(monitor) };
            
            // Примерная высота таскбара
            let taskbar_height = 40;
            
            // Разворачиваем на весь экран (оконный режим, с таскбаром)
            rl.set_window_position(0, 0);
            rl.set_window_size(monitor_w, monitor_h - taskbar_height);
            
            self.is_maximized = true;
        }
    }
    
    fn toggle_fullscreen(&mut self, rl: &mut RaylibHandle) {
        if self.is_maximized {
            // Сначала выходим из maximize
            self.is_maximized = false;
        }
        
        if self.is_fullscreen {
            // Восстанавливаем окно
            rl.set_window_size(self.windowed_size.0, self.windowed_size.1);
            rl.set_window_position(self.windowed_pos.0, self.windowed_pos.1);
            self.is_fullscreen = false;
        } else {
            // Сохраняем текущие размеры (если ещё не сохранены)
            if !self.is_maximized {
                self.windowed_size = (rl.get_screen_width(), rl.get_screen_height());
                let pos = rl.get_window_position();
                self.windowed_pos = (pos.x as i32, pos.y as i32);
            }
            
            // Получаем полный размер монитора
            let monitor = unsafe { raylib::ffi::GetCurrentMonitor() };
            let monitor_w = unsafe { raylib::ffi::GetMonitorWidth(monitor) };
            let monitor_h = unsafe { raylib::ffi::GetMonitorHeight(monitor) };
            
            // Разворачиваем на весь экран (поверх таскбара)
            rl.set_window_position(0, 0);
            rl.set_window_size(monitor_w, monitor_h);
            
            self.is_fullscreen = true;
        }
    }
    
    /// Отрисовать элементы управления окном
    pub fn draw<D: RaylibDraw>(&self, d: &mut D) {
        // Не рисуем если chrome скрыт
        if !self.chrome_visible {
            return;
        }
        
        // Используем unsafe для получения размеров экрана через FFI
        let screen_w = unsafe { raylib::ffi::GetScreenWidth() };
        
        // Фон кнопок
        let bg_color = Color::new(40, 40, 40, 220);
        let hover_color = Color::new(80, 80, 80, 220);
        let close_hover_color = Color::new(200, 60, 60, 220);
        let drag_color = Color::new(60, 60, 60, 220);
        let drag_hover_color = Color::new(90, 90, 90, 220);
        
        let mouse_pos = unsafe {
            let pos = raylib::ffi::GetMousePosition();
            Vector2::new(pos.x, pos.y)
        };
        
        // Drag handle (слева от кнопок)
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
        
        // Рисуем точки на drag handle
        let dots_y = self.margin + self.button_size / 2;
        for i in 0..3 {
            let dot_x = drag_x + 15 + i * 15;
            d.draw_circle(dot_x, dots_y - 4, 2.0, Color::LIGHTGRAY);
            d.draw_circle(dot_x, dots_y + 4, 2.0, Color::LIGHTGRAY);
        }
        
        // Кнопка minimize [_]
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
        
        // Иконка minimize (линия внизу)
        let mnx = min_x + self.button_size / 2;
        let mny = self.margin + self.button_size / 2;
        d.draw_line(mnx - 6, mny + 4, mnx + 6, mny + 4, Color::WHITE);
        
        // Кнопка maximize
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
        
        // Иконка maximize/restore
        let mx = max_x + self.button_size / 2;
        let my = self.margin + self.button_size / 2;
        let s = 6;
        
        if self.is_maximized || self.is_fullscreen {
            // Иконка restore (два прямоугольника)
            d.draw_rectangle_lines(mx - s + 2, my - s, s + 2, s + 2, Color::WHITE);
            d.draw_rectangle_lines(mx - s, my - s + 2, s + 2, s + 2, Color::WHITE);
        } else {
            // Иконка maximize (один прямоугольник)
            d.draw_rectangle_lines(mx - s, my - s, s * 2, s * 2, Color::WHITE);
        }
        
        // Кнопка закрытия [X]
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
        
        // Крестик
        let cx = close_x + self.button_size / 2;
        let cy = self.margin + self.button_size / 2;
        let cs = 6;
        d.draw_line(cx - cs, cy - cs, cx + cs, cy + cs, Color::WHITE);
        d.draw_line(cx + cs, cy - cs, cx - cs, cy + cs, Color::WHITE);
    }
    
    /// Текущее состояние maximized
    pub fn is_maximized(&self) -> bool {
        self.is_maximized
    }
    
    /// Текущее состояние fullscreen
    pub fn is_fullscreen(&self) -> bool {
        self.is_fullscreen
    }
    
    /// Текущее состояние dragging
    pub fn is_dragging(&self) -> bool {
        self.is_dragging
    }
    
    /// Chrome видим?
    pub fn is_chrome_visible(&self) -> bool {
        self.chrome_visible
    }
}

/// Информация о клике в буфер
#[derive(Debug, Clone, Copy)]
pub struct BufferClick {
    pub x: u32,
    pub y: u32,
    pub button: MouseButton,
}

/// Вспомогательные функции для ввода
pub struct Input;

impl Input {
    /// Получить позицию мыши как (i32, i32)
    pub fn mouse_pos(rl: &RaylibHandle) -> (i32, i32) {
        let pos = rl.get_mouse_position();
        (pos.x as i32, pos.y as i32)
    }
    
    /// Проверить, нажата ли кнопка мыши
    pub fn mouse_pressed(rl: &RaylibHandle, button: MouseButton) -> bool {
        rl.is_mouse_button_pressed(button)
    }
    
    /// Проверить, удерживается ли кнопка мыши
    pub fn mouse_down(rl: &RaylibHandle, button: MouseButton) -> bool {
        rl.is_mouse_button_down(button)
    }
    
    /// Получить прокрутку колёсика
    pub fn mouse_wheel(rl: &RaylibHandle) -> f32 {
        rl.get_mouse_wheel_move()
    }
    
    /// Проверить нажатие клавиши
    pub fn key_pressed(rl: &RaylibHandle, key: KeyboardKey) -> bool {
        rl.is_key_pressed(key)
    }
    
    /// Проверить удержание клавиши
    pub fn key_down(rl: &RaylibHandle, key: KeyboardKey) -> bool {
        rl.is_key_down(key)
    }
    
    /// Проверить, были ли брошены файлы
    pub fn is_file_dropped(rl: &RaylibHandle) -> bool {
        rl.is_file_dropped()
    }
}

/// Информация о drag-n-drop
#[derive(Debug, Clone, Default)]
pub struct DropZone {
    /// Последний брошенный файл
    pub last_dropped: Option<String>,
}

impl DropZone {
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Обновить состояние drop zone
    /// Возвращает Some(filename) если файл был брошен
    pub fn update(&mut self, rl: &mut RaylibHandle) -> Option<String> {
        if rl.is_file_dropped() {
            let files = rl.load_dropped_files();
            
            if files.count() > 0 {
                // Получаем первый файл
                if let Some(path_str) = files.paths().first() {
                    let path = path_str.to_string();
                    
                    // Извлекаем имя файла из пути
                    let filename = std::path::Path::new(&path)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(&path)
                        .to_string();
                    
                    self.last_dropped = Some(filename.clone());
                    return Some(filename);
                }
            }
        }
        
        None
    }
    
    /// Получить имя последнего брошенного файла
    pub fn last_file(&self) -> Option<&str> {
        self.last_dropped.as_deref()
    }
    
    /// Очистить информацию о последнем файле
    pub fn clear(&mut self) {
        self.last_dropped = None;
    }
}
