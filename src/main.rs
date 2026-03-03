use rand::Rng;
use std::time::Instant;

use raylib::prelude::*;

use grids::input::{DropZone, WindowChrome};
use grids::text_ops::TextOps;
use grids::types::{Character, GlyphsetKey};
use grids::VoidGrid;

fn main() {
    puffin::set_scopes_on(true);

    // Запускаем сервер на порту 8585
    let _puffin_server = match puffin_http::Server::new("127.0.0.1:8585") {
        Ok(server) => {
            println!("Puffin server started on http://127.0.0.1:8585");
            Some(server)
        }
        Err(err) => {
            eprintln!("Failed to start puffin server: {}", err);
            None
        }
    };

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
    let crt = vg
        .grids
        .load_atlas(&mut rl, &thread, "assets/crt.json")
        .expect("Failed to load CRT atlas");

    // Монтируем атласы в виртуальную файловую систему
    vg.grids.mount_atlas("fonts/crt", crt);

    // Создаем основной глифсет из CRT
    let gs_crt = vg.grids.create_glyphset_from_atlas("composite", crt);
    // "Вливаем" в него HUGE атлас (теперь в composite_gs есть и буквы CRT, и стрелки HUGE)
    // vg.grids.merge_atlas(composite_gs, huge);

    // Получаем размер тайла (теперь из Glyphset)
    let (tile_w, tile_h) = vg.grids.glyphset_size(gs_crt).unwrap();

    // Корректируем размер окна
    let window_w = buf_w * tile_w;
    let window_h = buf_h * tile_h;

    rl.set_window_size(window_w as i32, window_h as i32);

    // ========================================================================
    // Загружаем шейдер chromatic aberration
    // ========================================================================

    let chromatic_shader = vg
        .grids
        .load_shader(&mut rl, &thread, "assets/chromatic.fs")
        .expect("Failed to load chromatic shader");

    // ========================================================================
    // Создаём буферы
    // ========================================================================

    // Используем BufferBuilder
    let main_buf = vg
        .grids
        .buffer("main", buf_w, buf_h, gs_crt)
        .z_index(1)
        // .dynamic(true)
        .build();

    let back_buf = vg
        .grids
        .buffer("back", buf_w, buf_h, gs_crt)
        .z_index(-1)
        .dynamic(true) // <--- Включаем immediate mode для этого буфера
        .attach_to(main_buf, 0, 0)
        .build();

    // Drop zone буфер (маленький, внизу)
    let drop_zone_buf = vg
        .grids
        .buffer("drop_zone", 40, 1, gs_crt)
        .z_index(100)
        .dynamic(true)
        .attach_to(main_buf, 2, buf_h - 2)
        .build();

    // Буфер с шейдером chromatic aberration
    let shader_demo_buf = vg
        .grids
        .buffer("shader_demo", 40, 1, gs_crt)
        .dynamic(true)
        .attach_to(main_buf, 4, 9)
        .build();

    vg.renderer.attach_shader(
        &mut rl,
        &thread,
        &vg.grids,
        shader_demo_buf,
        chromatic_shader,
        4,
    );

    // Drop zone state
    let mut drop_zone = DropZone::new();

    // Выводим содержимое реестра для отладки
    vg.grids.debug_print_registry();

    let mut start_time: Instant;
    start_time = Instant::now();
    let current_time = start_time.elapsed().as_secs_f32();

    // ========================================================================
    // ГЛАВНЫЙ ЦИКЛ
    // ========================================================================

    let mut is_resized = false;

    while !rl.window_should_close() {
        puffin::GlobalProfiler::lock().new_frame();
        puffin::profile_scope!("Main Loop");
        // --- Очистка буферов ---
        if is_resized {
            vg.grids.clear_buffer(main_buf);
            is_resized = false;
        }
        //
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
            is_resized = true;

            let new_buf_w: u32 = (new_w as u32) / tile_w;
            let new_buf_h: u32 = (new_h as u32) / tile_h;

            if new_buf_w != buf_w || new_buf_h != buf_h {
                buf_w = new_buf_w.max(1);
                buf_h = new_buf_h.max(1);

                let _ = vg.grids.resize_buffer(main_buf, buf_w, buf_h);
                let _ = vg.grids.resize_buffer(back_buf, buf_w, buf_h);

                // Обновляем текстуры шейдеров, если размеры буферов изменились
                // (в данном примере shader_demo_buf не меняет размер, но если бы менял - нужно вызвать это)
                // vg.renderer.update_buffer_shader_texture(&mut rl, &thread, &vg.grids, shader_demo_buf);

                // Перемещаем drop zone вниз
                vg.grids.move_child(main_buf, drop_zone_buf, (2, buf_h - 2));
            }
        }

        // --- Строка статуса ---
        if let Some((w, _h)) = vg.grids.buffer_size(main_buf) {
            vg.grids
                .print(main_buf)
                .at(w.saturating_sub(29), 0)
                .fg(Color::new(0, 255, 127, 255))
                .write(("[ESC]", "inverted"))
                .write(" EXIT ")
                .write(("[F11]", "inverted"))
                .write(" FULLSCREEN ");
            // --- Фоновый буфер — случайные глифы ---
            let rx = rand::rng().random_range(0..=w);
            let ry = rand::rng().random_range(0..=_h);
            let ch = rand::rng().random_range(33..=90);
            let alpha = rand::rng().random_range(0..=32);

            // Character::new теперь принимает (code, variant_id, fg, bg)
            vg.grids.set_char(
                back_buf,
                rx,
                ry,
                Character::new(ch, 0, Color::new(0, 255, 0, alpha), Color::BLANK),
            );
        }


        // --- Буфер с шейдером ---

        let current_time = start_time.elapsed().as_secs_f32();
        let status_text = format!("VOIDGRID _ {:.2}", current_time);

        if ((current_time * 5.0).floor() % 2.0) == 0.0 {
            vg.grids
                .print(main_buf)
                .at(4, 18)
                .fg(Color::new(0, 255, 127, 255))
                .write("TWELVE\nCATHODE\nTELEVISION TUBES\n")
                .write(("FLICKERING\n", "inverted"))
                .write("NAKEDLY\nON ONE SIDE\nAND FOUR SPEAKERS\nHUMMING ON\nTHE OTHER...");
        } else {
            vg.grids
                .print(main_buf)
                .at(4, 18)
                .fg(Color::new(0, 255, 127, 255))
                .write("TWELVE\nCATHODE\nTELEVISION TUBES\n")
                .write(("FLICKERING\n"))
                .write("NAKEDLY\nON ONE SIDE\nAND FOUR SPEAKERS\nHUMMING ON\nTHE OTHER...");
        }

        vg.grids
            .print(shader_demo_buf)
            .color(Color::WHITE, Color::new(16, 16, 16, 255))
            .write(status_text);

        // --- Drop zone буфер ---
        {
            let drop_text = if let Some(filename) = drop_zone.last_file() {
                format!(" DROPPED: {} ", filename)
            } else {
                " DROP FILE HERE ".to_string()
            };

            let text: String = drop_text.chars().take(38).collect();

            vg.grids.write_string(
                drop_zone_buf,
                0,
                0,
                &text,
                Color::WHITE,
                Color::new(180, 40, 40, 255),
            );
        }

        // --- Отрисовка ---
        puffin::profile_scope!("Prepare Render");
        // Анимируем смещение chromatic aberration
        let aberration = (vg.renderer.shader_time() * 3.0).sin() * 1.5 + 1.5; // от 1 до 5 пикселей
        vg.grids
            .set_shader_float(chromatic_shader, "offset", aberration);

        // Двухпроходный рендер: сначала буферы с шейдерами в их текстуры
        vg.render_offscreen(&mut rl, &thread, main_buf, 0, 0);

        // Потом рисуем всё на экран
        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::new(8, 8, 8, 255));
            puffin::profile_scope!("Offscreen Render");
            // draw рисует дерево + применяет шейдеры к буферам (через фасад)
            vg.draw(&mut d, main_buf, 0, 0);

            chrome.draw(&mut d);
            d.draw_fps(10, 10);
        }
    }
}
