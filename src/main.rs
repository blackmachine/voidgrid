use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use std::time::Instant;

use raylib::prelude::*;

use grids::input::{DropZone, WindowChrome};
use grids::text_ops::TextOps;
use grids::types::Character;
use grids::VoidGrid;
use grids::hierarchy::{Hierarchy, ZPolicy, Anchor};
use grids::resource_pack::{DirProvider, ResourceProvider};

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
    let mut provider = DirProvider::new(".");

    // Инициализируем (загрузка шейдеров и т.д.)
    vg.init(&mut provider, &mut rl, &thread);

    // Загружаем атласы
    let crt = vg
        .grids.assets
        .load_atlas(&mut provider, &mut rl, &thread, "assets/crt.json")
        .expect("Failed to load CRT atlas");

    // Монтируем атласы в виртуальную файловую систему
    vg.grids.assets.mount_atlas("fonts/crt", crt);

    // Создаем основной глифсет из CRT
    let gs_crt = vg.grids.assets.create_glyphset_from_atlas("composite", crt);
    // "Вливаем" в него HUGE атлас (теперь в composite_gs есть и буквы CRT, и стрелки HUGE)
    // vg.grids.merge_atlas(composite_gs, huge);

    // Получаем размер тайла (теперь из Glyphset)
    let (tile_w, tile_h) = vg.grids.assets.glyphset_size(gs_crt).unwrap();

    // Корректируем размер окна
    let window_w = buf_w * tile_w;
    let window_h = buf_h * tile_h;

    rl.set_window_size(window_w as i32, window_h as i32);

    // ========================================================================
    // Загружаем шейдер chromatic aberration
    // ========================================================================

    let chromatic_shader = vg
        .grids.assets
        .load_shader(&mut provider, &mut rl, &thread, "assets/chromatic.fs")
        .expect("Failed to load chromatic shader");

    // ========================================================================
    // Создаём буферы
    // ========================================================================

    // Инициализируем иерархию
    let mut hierarchy = Hierarchy::new();

    // Используем BufferBuilder
    let main_buf = vg
        .grids
        .buffer("main", buf_w, buf_h, gs_crt)
        .z_index(1)
        // .dynamic(true)
        .build();
    
    // Создаем корневой узел
    let root_node = hierarchy.set_root(Some(main_buf));

    let back_buf = vg
        .grids
        .buffer("back", buf_w, buf_h, gs_crt)
        .z_index(-1)
        .dynamic(true) // <--- Включаем immediate mode для этого буфера
        .build();
    
    // Привязываем back_buf к main_buf
    hierarchy.attach(Some(back_buf))
        .to(root_node)
        .with_z(ZPolicy::Relative(-1)); // Рисуем под родителем

    // Drop zone буфер (маленький, внизу)
    let drop_zone_buf = vg
        .grids
        .buffer("drop_zone", 40, 1, gs_crt)
        .z_index(100)
        .dynamic(true)
        .build();
    
    // Привязываем drop_zone
    let drop_node = hierarchy.attach(Some(drop_zone_buf))
        .to(root_node)
        .anchor(Anchor::BottomLeft)
        .at(2, -2)
        .with_z(ZPolicy::Absolute(100))
        .key();

    // Буфер с шейдером chromatic aberration
    let shader_demo_buf = vg
        .grids
        .buffer("shader_demo", 40, 1, gs_crt)
        .dynamic(true)
        .build();
    
    hierarchy.attach(Some(shader_demo_buf))
        .to(root_node)
        .at(4, 9);

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
    vg.grids.assets.debug_print_registry();

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
                // vg.grids.move_child(main_buf, drop_zone_buf, (2, buf_h - 2));
            }
        }
        let current_time = start_time.elapsed().as_secs_f32();
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

            // Character::new теперь принимает (code, variant_id, fg, bg)
            // vg.grids.set_char(
            //     back_buf,
            //     rx,
            //     ry,
            //     Character::new(ch, 0, Color::new(0, 255, 0, alpha), Color::BLANK),
            // );
        }

        // --- Буфер с шейдером ---

        let status_text = format!("VOIDGRID _ {:.2}", current_time);

        if ((current_time * 5.0).floor() % 2.0) == 0.0 {
            vg.grids
                .print(main_buf)
                .at(4, 18)
                .fg(Color::new(0, 255, 127, 255))
                .write("TWELVE\nCATHODE\nTELEVISION TUBES\n")
                .write(("FLICKERING\n", "inverted")) // Исправлено: убраны лишние скобки, но здесь кортеж нужен для Printable
                .write("NAKEDLY\nON ONE SIDE\nAND FOUR SPEAKERS\nHUMMING ON\nTHE OTHER...");
        } else {
            vg.grids
                .print(main_buf)
                .at(4, 18)
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
            d.draw_fps(10, 10);
        }
    }
}
