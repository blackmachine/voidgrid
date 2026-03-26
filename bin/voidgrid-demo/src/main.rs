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
use voidgrid::terminal::Action;
use voidgrid::events::{Event, MouseButton};
use voidgrid::types::Character;
// use voidgrid::resource_pack::ResourceProvider;

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
  


    // let zip_file = std::fs::File::open("crtdemo.vpk")
    //     .expect("Не удалось найти файл crtdemo.vpk");

    // let mut provider = voidgrid_resource_packs::ZipProvider::new(zip_file)
    //     .expect("Не удалось прочитать структуру ZIP-архива");

      let mut provider = voidgrid_resource_packs::DirProvider::new("res");


    // Инициализируем (загрузка шейдеров и т.д.)
    vg.init(&mut provider, &mut rl, &thread);

    // Инициализируем иерархию
    let mut hierarchy = Hierarchy::new();

    // 2. Загружаем сцену из манифеста
    let pack = PackLoader::load_pack(
        &mut vg, 
        &mut hierarchy, 
        &mut provider, 
        "manifest.toml", 
        &mut rl, 
        &thread
    ).expect("Failed to load scene from manifest");

    // 3. Извлекаем ключи буферов для использования в главном цикле
    vg.terminal.register_buffers(pack.buffers.clone());
    let main_buf = pack.buffers["main_buf"];
    let back_buf = pack.buffers["back_buf"];
    let tx_buf = pack.buffers["tx_buf"];
    let drop_zone_buf = pack.buffers["drop_zone_buf"];
    let shader_demo_buf = pack.buffers["shader_demo_buf"];

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

    // Debug: print registry stats
    println!("Global registry: {} entries", vg.grids.assets.global_registry.entries.len());

    let mut start_time: Instant;
    start_time = Instant::now();
    let current_time = start_time.elapsed().as_secs_f32();

    // Инициализируем парсер VTP
    let mut vtp_parser = VtpParser::new();
    // Terminal state moved to vg.terminal

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

    // --- Rhai Initialization ---
    let mut script_engine = voidgrid::scripting::ScriptEngine::new();
    
    // Авто-загрузка всех скриптов из манифеста
    for (name, code) in &pack.scripts {
        if let Err(e) = script_engine.load_script(name, code) {
            eprintln!("Failed to load pack script '{}': {}", name, e);
        }
    }

    // ВАЖНО: Вызываем run_init() только ПОСЛЕ загрузки всех скриптов из пака!
    script_engine.run_init();
    // ---------------------------
    // !! render_list предыдущего кадра — нужен для mouse_to_cell() в скриптах.
    // !! Без этого mouse_to_cell() всегда возвращает (), потому что не знает
    // !! экранных координат буферов. Обновляется в конце кадра после collect_render_list().
    let mut prev_render_list: Vec<voidgrid::hierarchy::RenderItem> = Vec::new();

    let mut is_resized = false;

    while !rl.window_should_close() {

        // --- Event Polling Adapter ---
        vg.events.clear();

        if rl.is_window_resized() {
            vg.events.push(Event::WindowResize { width: rl.get_screen_width(), height: rl.get_screen_height() });
        }
        
        let mouse_pos = rl.get_mouse_position();
        let mouse_delta = rl.get_mouse_delta();
        if mouse_delta.x != 0.0 || mouse_delta.y != 0.0 {
            vg.events.push(Event::MouseMove { x: mouse_pos.x, y: mouse_pos.y });
        }
        
        use raylib::consts::MouseButton as RaylibMouse;
        if rl.is_mouse_button_pressed(RaylibMouse::MOUSE_BUTTON_LEFT) {
            vg.events.push(Event::MousePress { x: mouse_pos.x, y: mouse_pos.y, button: MouseButton::Left });
        }
        if rl.is_mouse_button_released(RaylibMouse::MOUSE_BUTTON_LEFT) {
            vg.events.push(Event::MouseRelease { x: mouse_pos.x, y: mouse_pos.y, button: MouseButton::Left });
        }
        if rl.is_mouse_button_pressed(RaylibMouse::MOUSE_BUTTON_RIGHT) {
            vg.events.push(Event::MousePress { x: mouse_pos.x, y: mouse_pos.y, button: MouseButton::Right });
        }
        if rl.is_mouse_button_released(RaylibMouse::MOUSE_BUTTON_RIGHT) {
            vg.events.push(Event::MouseRelease { x: mouse_pos.x, y: mouse_pos.y, button: MouseButton::Right });
        }
        
        while let Some(key) = rl.get_key_pressed() {
            vg.events.push(Event::KeyPress { key: key as u32 });
        }
        
        if rl.is_file_dropped() {
            let files = rl.load_dropped_files();
            for path in files.paths() {
                vg.events.push(Event::FileDrop { path: path.to_string() });
            }
        }
        // -----------------------------
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
        if let Some(filename) = drop_zone.update(&vg.events.frame_events) {
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

                // 1. Обновляем логическую сетку
                let _ = vg.grids.resize_buffer(main_buf, buf_w, buf_h);
                let _ = vg.grids.resize_buffer(back_buf, buf_w/2, buf_h/4);

                // 2. ВАЖНО: Обновляем физические текстуры (RenderTexture) для шейдеров!
                // Если этого не сделать, старая текстура растянется на новый экран.
                vg.renderer.update_buffer_shader_texture(&mut rl, &thread, &vg.grids, main_buf);
                vg.renderer.update_buffer_shader_texture(&mut rl, &thread, &vg.grids, back_buf);
            }
        }
        let current_time = start_time.elapsed().as_secs_f32();

        // Обрабатываем все пакеты, пришедшие из сети за этот кадр
        while let Ok(network_data) = rx.try_recv() {
            vtp_parser.push_bytes(&network_data);
            while let Some(cmd) = vtp_parser.next_command() {
                let action = match cmd {
                    VtpCommand::SetBuffer(name) => Action::SetBuffer(name),
                    VtpCommand::SetCursor { x, y } => Action::SetCursor(x, y),
                    VtpCommand::SetFgColor(c) => Action::SetFgColor(Color::new(c.r, c.g, c.b, c.a)),
                    VtpCommand::SetBgColor(c) => Action::SetBgColor(Color::new(c.r, c.g, c.b, c.a)),
                    VtpCommand::SetVariant(v) => Action::SetVariant(v),
                    VtpCommand::PrintChar(code) => Action::PrintChar(code),
                    VtpCommand::PrintString(text) => Action::PrintString(text),
                    _ => continue,
                };
                vg.terminal.apply_action(&mut vg.grids, action);
            }
        }

        // --- Execute Rhai Script Frame ---
        script_engine.sync_screen_positions(&prev_render_list, &vg.grids, &pack.buffers);
        script_engine.sync_state(&vg.grids, &pack.buffers);
        script_engine.run_update(current_time, &vg.events.frame_events);

        vg.grids
                .print(tx_buf)
                .at(15, 10)
                .fg(Color::new(255, 255, 255, 255))
                .bg(Color::new(32, 32, 32, 255))
                .writeln(("Text block", "bold"))
                .writeln(("This is the text block,\nplease enjoy.", "thin"));

        for action in script_engine.take_actions() {
            vg.terminal.apply_action(&mut vg.grids, action);
        }
        if let Some((w, _h)) = vg.grids.buffer_size(main_buf) {
            vg.grids
                .print(main_buf)
                .at(w.saturating_sub(29), 1)
                .fg(Color::new(0, 255, 127, 255))
                .write(("[ESC]", "inverted"))
                .write(" EXIT ")
                .write(("[F11]", "inverted"))
                .write(" FULLSCREEN ");

////////////////////////////////////////////////////////


            let ch = rand::rng().random_range(33..=90);
            let alpha = rand::rng().random_range(0..=32);

            let cx = (w as f32) * 0.5*0.5;
            let cy = (_h as f32) / 8.0;

            let seed: u64 = 42;

            let mut rng = StdRng::seed_from_u64(seed);
            

            for x in 0..w {
                for y in 0.._h {
                    // let ch = rng.random_range(33..=90);
                    let ch = rand::rng().random_range(33..=90);

                    let luma = (((cx - x as f32).powi(2) + ((cy - y as f32)*2.0).powi(2)).sqrt() * 0.25 - current_time * 3.0).sin()* 64.0 + 64.0;
                    let alpha = luma;

                    vg.grids.set_char(
                        back_buf,
                        x,
                        y,
                        Character::new(ch, 0, Color::new(0, 255, 0, alpha as u8), Color::BLANK),
                    );
                }
            }

////////////////////////////////////////////////////////


        }

        // --- Буфер с шейдером ---

        let status_text = format!("VOIDGRID _ {:.2}", current_time);

        if ((current_time * 5.0).floor() % 2.0) == 0.0 {
            vg.grids
                .print(main_buf)
                .at(4, 3)
                .fg(Color::new(0, 255, 127, 255))
                .write("TWELVE\nCATHODE\nTELEVISION TUBES\n")
                // Исправлено: убраны лишние скобки, но здесь кортеж нужен для Printable
                .write(("FLICKERING\n", "inverted")) 
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

        
        puffin::profile_scope!("Prepare Render");

        // Update shader params
        let aberration = (vg.renderer.shader_time() * 3.0).sin() * 1.5 + 1.5; // от 1 до 5 пикселей
        vg.grids.assets.set_shader_float(chromatic_shader, "offset", aberration);

        // Собираем список рендеринга из иерархии
        let render_list = hierarchy.collect_render_list(|b| {
            if let Some(buf) = vg.grids.get(b) {
                if let Some((tw, th)) = vg.grids.assets.glyphset_size(buf.glyphset()) {
                    return (buf.w, buf.h, tw, th);
                }
            }
            (0, 0, 1, 1)
        });

        // PRE-DRAW (image buffers for shaders, etc)
        vg.render_offscreen(&mut rl, &thread, &render_list);

        // DRAW
        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::new(8, 8, 8, 255));
            puffin::profile_scope!("Offscreen Render");
            vg.draw(&mut d, &render_list); // draw рисует дерево + применяет шейдеры к буферам (через фасад)
            chrome.draw(&mut d);
            // d.draw_fps(10, 10);
        }

        // Сохраняем render_list для mouse_to_cell() в следующем кадре
        prev_render_list = render_list;
    }
}