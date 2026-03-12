use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use std::time::Instant;
use std::net::TcpListener;
use std::sync::mpsc;
use std::thread;
use std::io::Read;

use raylib::prelude::*;

use voidgrid::input::{DropZone, WindowChrome};
use voidgrid::text_ops::TextOps;

use voidgrid::VoidGrid;
use voidgrid::hierarchy::Hierarchy;
use voidgrid::pack_loader::PackLoader;
use voidgrid_vtp::{VtpParser, VtpCommand};
use voidgrid::types::{Character, Blend, Transform};

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

    // Создаем провайдер ресурсов
    let zip_file = std::fs::File::open("crtdemo.vpk")
        .expect("Не удалось найти файл crtdemo.vpk");

    let mut provider = voidgrid_resource_packs::ZipProvider::new(zip_file)
        .expect("Не удалось прочитать структуру ZIP-архива");

    // Инициализируем (загрузка шейдеров и т.д.)
    vg.init(&mut provider, &mut rl, &thread);

    // Инициализируем иерархию
    let mut hierarchy = Hierarchy::new();

    // 2. Загружаем сцену из манифеста
    let buffers = PackLoader::load_pack(
        &mut vg, 
        &mut hierarchy, 
        &mut provider, 
        "manifest.json", 
        &mut rl, 
        &thread
    ).expect("Failed to load scene from manifest");

    // 3. Извлекаем ключи буферов для использования в главном цикле
    let main_buf = buffers["main_buf"];
    let back_buf = buffers["back_buf"];
    let drop_zone_buf = buffers["drop_zone_buf"];
    let shader_demo_buf = buffers["shader_demo_buf"];

    // 4. Получаем размеры тайла из корневого буфера для настройки окна
    let main_glyphset = vg.grids.get(main_buf).unwrap().glyphset();
    let (tile_w, tile_h) = vg.grids.assets.glyphset_size(main_glyphset).unwrap();

    // Корректируем размер окна
    let window_w = buf_w * tile_w;
    let window_h = buf_h * tile_h;
    rl.set_window_size(window_w as i32, window_h as i32);

    // Восстанавливаем доступ к шейдеру для анимации в цикле
    let chromatic_shader = vg.grids.assets.load_shader(&mut provider, &mut rl, &thread, "assets/chromatic.fs").expect("Failed to load chromatic shader");

    // Drop zone state
    let mut drop_zone = DropZone::new();

    // Выводим содержимое реестра для отладки
    vg.grids.assets.debug_print_registry();

    let mut start_time: Instant;
    start_time = Instant::now();
    let current_time = start_time.elapsed().as_secs_f32();






// Инициализируем парсер VTP
let mut vtp_parser = VtpParser::new();
    let mut vtp_active_buffer = None;
    let mut vtp_cursor_x = 0;
    let mut vtp_cursor_y = 0;
    let mut vtp_fg_color = Color::WHITE;
    let mut vtp_bg_color = Color::BLANK;
    let mut vtp_variant_id = 0;

// Канал для передачи сырых байт из сети в главный цикл
let (tx, rx) = mpsc::channel::<Vec<u8>>();

// Фоновый поток TCP-сервера
thread::spawn(move || {
    let listener = TcpListener::bind("127.0.0.1:8080").expect("Failed to bind TCP port 8080");
    println!("VTP Server listening on 127.0.0.1:8080");

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                println!("VTP Client connected!");
                let mut buffer = [0u8; 4096];
                loop {
                    match stream.read(&mut buffer) {
                        Ok(0) => {
                            println!("VTP Client disconnected");
                            break;
                        }
                        Ok(n) => {
                            // Отправляем прочитанные байты в главный поток
                            if tx.send(buffer[..n].to_vec()).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("TCP read error: {}", e);
                            break;
                        }
                    }
                }
            }
            Err(e) => eprintln!("TCP connection failed: {}", e),
        }
    }
});




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

            }
        }
        let current_time = start_time.elapsed().as_secs_f32();

        // Обрабатываем все пакеты, пришедшие из сети за этот кадр
while let Ok(network_data) = rx.try_recv() {
            vtp_parser.push_bytes(&network_data);
            
            while let Some(cmd) = vtp_parser.next_command() {
                match cmd {
                    VtpCommand::SetBuffer(name) => {
                        if let Some(&key) = buffers.get(&name) {
                            vtp_active_buffer = Some(key);
                            vtp_cursor_x = 0;
                            vtp_cursor_y = 0;
                        }
                    }
                    VtpCommand::SetCursor { x, y } => {
                        vtp_cursor_x = x;
                        vtp_cursor_y = y;
                    }
                    VtpCommand::SetFgColor(c) => vtp_fg_color = Color::new(c.r, c.g, c.b, c.a),
                    VtpCommand::SetBgColor(c) => vtp_bg_color = Color::new(c.r, c.g, c.b, c.a),
                    VtpCommand::SetVariant(v) => vtp_variant_id = v,
                    VtpCommand::PrintChar(code) => {
                        if let Some(key) = vtp_active_buffer {
                            if let Some(ch) = char::from_u32(code) {
                                vg.grids.set_char(
                                    key, vtp_cursor_x, vtp_cursor_y, 
                                    Character::full(ch as u32, vtp_variant_id, vtp_fg_color, vtp_bg_color, Blend::Alpha, Blend::Alpha, Transform::default(), None)
                                );
                                vtp_cursor_x += 1;
                            }
                        }
                    }
                    VtpCommand::PrintString(text) => {
                        if let Some(key) = vtp_active_buffer {
                            vg.grids.print(key)
                                .at(vtp_cursor_x, vtp_cursor_y)
                                .color(vtp_fg_color, vtp_bg_color)
                                .write(text.as_ref());
                            vtp_cursor_x += text.chars().count() as u32;
                        }
                    }
                    _ => {}
                }
            }
        }

        // --- Строка статуса ---
        if let Some((w, _h)) = vg.grids.buffer_size(main_buf) {
            vg.grids
                .print(main_buf)
                .at(w.saturating_sub(29), 1)
                .fg(Color::new(0, 255, 127, 255))
                .write(("[ESC]", "inverted"))
                .write(" EXIT ")
                .write(("[F11]", "inverted"))
                .write(" FULLSCREEN ");

            let ch = rand::rng().random_range(33..=90);
            let alpha = rand::rng().random_range(0..=32);

            let cx = (w as f32) * 0.5;
            let cy = (_h as f32) / 2.0;

            let seed: u64 = 42;

            let mut rng = StdRng::seed_from_u64(seed);

            for x in 0..w {
                for y in 0.._h {
                    let ch = rng.random_range(33..=90);
                    // let alpha = rand::rng().random_range(0..=32);

                    let luma = (((cx - x as f32).powi(2) + (cy - y as f32).powi(2)).sqrt() * 0.25
                        - current_time * 3.0)
                        .sin()
                        * 16.0
                        + 16.0;
                    let alpha = luma;

                    vg.grids.set_char(
                        back_buf,
                        x,
                        y,
                        Character::new(ch, 0, Color::new(0, 255, 0, alpha as u8), Color::BLANK),
                    );
                }
            }


        }

        // --- Буфер с шейдером ---

        let status_text = format!("VOIDGRID _ {:.2}", current_time);

        if ((current_time * 5.0).floor() % 2.0) == 0.0 {
            vg.grids
                .print(main_buf)
                .at(4, 3)
                .fg(Color::new(0, 255, 127, 255))
                .write("TWELVE\nCATHODE\nTELEVISION TUBES\n")
                .write(("FLICKERING\n", "inverted")) // Исправлено: убраны лишние скобки, но здесь кортеж нужен для Printable
                .write("NAKEDLY\nON ONE SIDE\nAND FOUR SPEAKERS\nHUMMING ON\nTHE OTHER...");
        } else {
            vg.grids
                .print(main_buf)
                .at(4, 3)
                .fg(Color::new(0, 255, 127, 255))
                .write("TWELVE\nCATHODE\nTELEVISION TUBES\n")
                .write("FLICKERING\n")
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
        vg.grids.assets
            .set_shader_float(chromatic_shader, "offset", aberration);

        // Собираем список рендеринга из иерархии
        let render_list = hierarchy.collect_render_list(|b| {
            if let Some(buf) = vg.grids.get(b) {
                if let Some((tw, th)) = vg.grids.assets.glyphset_size(buf.glyphset()) {
                    return (buf.w, buf.h, tw, th);
                }
            }
            (0, 0, 1, 1)
        });

        // Двухпроходный рендер: сначала буферы с шейдерами в их текстуры
        vg.render_offscreen(&mut rl, &thread, &render_list);

        // Потом рисуем всё на экран
        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::new(8, 8, 8, 255));
            puffin::profile_scope!("Offscreen Render");
            // draw рисует дерево + применяет шейдеры к буферам (через фасад)
            vg.draw(&mut d, &render_list);

            chrome.draw(&mut d);
            // d.draw_fps(10, 10);
        }
    }
}


