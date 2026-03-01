use rand::Rng;
use raylib::prelude::*;

// Используем модули из нашей библиотеки (предполагаем имя крейта "grids")
use grids::VoidGrid;
use grids::types::{Character, GlyphsetKey};
use grids::text_ops::TextOps;
use grids::input::{WindowChrome, DropZone};

fn main() {
    // Начальные размеры буфера в символах
    let mut buf_w: u32 = 64;
    let mut buf_h: u32 = 32;
    
    // Инициализация окна (undecorated)
    let (mut rl, thread) = raylib::init()
        .size(800, 600)
        .title("Grids TUI System")
        .undecorated()
        .resizable()
        .build();
    
    rl.set_target_fps(60);
    
    // Window chrome (кнопки закрытия, maximize и drag handle)
    let mut chrome = WindowChrome::new(800, 600);

    // ========================================================================
    // Инициализация Grids
    // ========================================================================
    
    // Создаем фасад
    let mut vg = VoidGrid::new();
    
    // Инициализируем (загрузка шейдеров и т.д.)
    vg.init(&mut rl, &thread);
    
    // Загружаем атласы
    let crt = vg.grids.load_atlas(&mut rl, &thread, "assets/crt.json")
        .expect("Failed to load CRT atlas");
    
    let huge = vg.grids.load_atlas(&mut rl, &thread, "assets/huge.json")
        .expect("Failed to load huge atlas");

    // Монтируем атласы в виртуальную файловую систему
    vg.grids.mount_atlas("fonts/crt", crt);
    vg.grids.mount_atlas("fonts/huge", huge);

    // Создаем Glyphsets (предполагаем наличие хелпера для создания из атласа)
    let crt_gs = vg.grids.create_glyphset_from_atlas("crt", crt);
    let huge_gs = vg.grids.create_glyphset_from_atlas("huge", huge);

    // Получаем размер тайла (теперь из Glyphset)
    let (tile_w, tile_h) = vg.grids.glyphset_size(crt_gs).unwrap();
    
    // Корректируем размер окна
    let window_w = buf_w * tile_w;
    let window_h = buf_h * tile_h;
    rl.set_window_size(window_w as i32, window_h as i32);

    // ========================================================================
    // Загружаем шейдер chromatic aberration
    // ========================================================================
    
    let chromatic_shader = vg.grids.load_shader(&mut rl, &thread, "assets/chromatic.fs")
        .expect("Failed to load chromatic shader");

    // ========================================================================
    // Создаём буферы
    // ========================================================================
    
    let main_buf = vg.grids.create_buffer("main", buf_w, buf_h, crt_gs);
    let back_buf = vg.grids.create_buffer("back", buf_w/4, buf_h/4, huge_gs);
    vg.grids.set_buffer_z(back_buf, -1);
    vg.grids.set_buffer_z(main_buf, 1);
    
    // Drop zone буфер (маленький, внизу)
    let drop_zone_buf = vg.grids.create_buffer("drop_zone", 40, 1, crt_gs);
    
    // Буфер с шейдером chromatic aberration
    let shader_demo_buf = vg.grids.create_buffer("shader_demo", 40, 1, crt_gs);
    vg.renderer.set_buffer_shader_padded(&mut rl, &thread, &vg.grids, shader_demo_buf, chromatic_shader, 4);

    // Привязываем дочерние буферы
    vg.grids.attach(main_buf, (0, 0), back_buf);
    vg.grids.attach_z(main_buf, (2, buf_h - 2), drop_zone_buf, 100);
    // Буфер с шейдером теперь тоже в иерархии!
    vg.grids.attach(main_buf, (4, 9), shader_demo_buf);
    
    // Drop zone state
    let mut drop_zone = DropZone::new();

    // Выводим содержимое реестра для отладки
    vg.grids.debug_print_registry();

    // ========================================================================
    // ГЛАВНЫЙ ЦИКЛ
    // ========================================================================
    while !rl.window_should_close() {
        
        // --- Очистка буферов ---
        vg.grids.clear_buffer(main_buf);
        vg.grids.clear_buffer(drop_zone_buf);
        vg.grids.clear_buffer(shader_demo_buf);
        
        // --- Обработка window chrome ---
        if chrome.update(&mut rl) {
            break;
        }
        
        // --- Обработка drag-n-drop ---
        if let Some(filename) = drop_zone.update(&mut rl) {
            println!("Dropped: {}", filename);
        }
        
        // --- Проверка resize и обновление буфера ---
        if let Some((new_w, new_h)) = chrome.check_resize(&rl) {
            let new_buf_w: u32 = (new_w as u32) / tile_w;
            let new_buf_h: u32 = (new_h as u32) / tile_h;
            
            if new_buf_w != buf_w || new_buf_h != buf_h {
                buf_w = new_buf_w.max(1);
                buf_h = new_buf_h.max(1);
                
                let _ = vg.grids.resize_buffer(main_buf, buf_w, buf_h);
                let _ = vg.grids.resize_buffer(back_buf, buf_w/4, buf_h/4);

                // Обновляем текстуры шейдеров, если размеры буферов изменились
                // (в данном примере shader_demo_buf не меняет размер, но если бы менял - нужно вызвать это)
                // vg.renderer.update_buffer_shader_texture(&mut rl, &thread, &vg.grids, shader_demo_buf);
                
                // Перемещаем drop zone вниз
                vg.grids.move_child(main_buf, drop_zone_buf, (2, buf_h - 2));
            }
        }
        
        // --- Строка статуса ---
        if let Some((w, _h)) = vg.grids.buffer_size(main_buf) {
            vg.grids.write_string(
                main_buf, w.saturating_sub(35), 0,
                "   [ESC] EXIT   [F11] FULLSCREEN   ",
                Color::new(0, 255, 127, 255),
                Color::new(0, 40, 20, 255),
            );
        }
        
        // --- Основной текст ---
        vg.grids.write_string(
            main_buf, 4, 4,
            "TWELVE CATHODE TELEVISION\nTUBES FLICKERING NAKEDLY\nON ONE SIDE AND FOUR SPEAKERS\nHUMMING ON THE OTHER...",
            Color::new(0, 255, 127, 255),
            Color::BLANK,
        );
        
        // --- Буфер с шейдером ---
        vg.grids.write_string(
            shader_demo_buf, 0, 0,
            " THIS TEXT HAS CHROMATIC ABERRATION ",
            Color::new(255, 255, 255, 255),
            Color::new(40, 40, 80, 255),
        );

        // --- Фоновый буфер — случайные глифы ---
        let rx = rand::rng().random_range(0..16);
        let ry = rand::rng().random_range(0..8);
        let ch = rand::rng().random_range(0..=15);
        let alpha = rand::rng().random_range(0..=32);
        // Character::new теперь принимает (code, variant_id, fg, bg)
        vg.grids.set_char(back_buf, rx, ry, Character::new(ch, 0, Color::new(0, 255, 0, alpha), Color::BLANK));

        // --- Демонстрация иконок (стрелки из huge.json) ---
        // back_buf использует huge_gs, в котором определена группа "arrows"
        vg.grids.put_icon(back_buf, 0, 0, "arrow_left", Color::YELLOW, Color::BLANK);
        vg.grids.put_icon(back_buf, 1, 0, "arrow_right", Color::YELLOW, Color::BLANK);
        vg.grids.put_icon(back_buf, 2, 0, "arrow_up", Color::YELLOW, Color::BLANK);
        vg.grids.put_icon(back_buf, 3, 0, "arrow_down", Color::YELLOW, Color::BLANK);

        // --- Drop zone буфер ---
        {
            let drop_text = if let Some(filename) = drop_zone.last_file() {
                format!(" DROPPED: {} ", filename)
            } else {
                " DROP FILE HERE ".to_string()
            };
            
            let text: String = drop_text.chars().take(38).collect();
            
            vg.grids.write_string(
                drop_zone_buf, 0, 0,
                &text,
                Color::WHITE,
                Color::new(180, 40, 40, 255),
            );
        }

        // --- Отрисовка ---
        
        // Анимируем смещение chromatic aberration
        let aberration = (vg.renderer.shader_time() * 3.0).sin() * 2.0 + 3.0; // от 1 до 5 пикселей
        vg.grids.set_shader_float(chromatic_shader, "offset", aberration);
        
        // Двухпроходный рендер: сначала буферы с шейдерами в их текстуры
        vg.render_offscreen(&mut rl, &thread, main_buf, 0, 0);
        
        // Потом рисуем всё на экран
        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::new(8, 8, 8, 255));
            
            // draw рисует дерево + применяет шейдеры к буферам (через фасад)
            vg.draw(&mut d, main_buf, 0, 0);
            
            chrome.draw(&mut d);
        }
    }
}
