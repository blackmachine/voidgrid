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
use voidgrid::resource_pack::ResourceProvider;
fn main() {
    puffin::set_scopes_on(true);

    // Р вЂ”Р В°Р С—РЎС“РЎРѓР С”Р В°Р ВµР С РЎРѓР ВµРЎР‚Р Р†Р ВµРЎР‚ Р Р…Р В° Р С—Р С•РЎР‚РЎвЂљРЎС“ 8585
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

    // Р СњР В°РЎвЂЎР В°Р В»РЎРЉР Р…РЎвЂ№Р Вµ РЎР‚Р В°Р В·Р СР ВµРЎР‚РЎвЂ№ Р В±РЎС“РЎвЂћР ВµРЎР‚Р В° Р Р† РЎРѓР С‘Р СР Р†Р С•Р В»Р В°РЎвЂ¦
    let mut buf_w: u32 = 64;
    let mut buf_h: u32 = 32;

    // Р ВР Р…Р С‘РЎвЂ Р С‘Р В°Р В»Р С‘Р В·Р В°РЎвЂ Р С‘РЎРЏ Р С•Р С”Р Р…Р В° (undecorated)
    let (mut rl, thread) = raylib::init()
        .size(800, 600)
        .title("Grids TUI System")
        .undecorated()
        .resizable()
        .build();

    rl.set_target_fps(60);

    // Window chrome (Р С”Р Р…Р С•Р С—Р С”Р С‘ Р В·Р В°Р С”РЎР‚РЎвЂ№РЎвЂљР С‘РЎРЏ, maximize Р С‘ drag handle)
    let mut chrome = WindowChrome::new(800, 600);

    // ========================================================================
    // Р ВР Р…Р С‘РЎвЂ Р С‘Р В°Р В»Р С‘Р В·Р В°РЎвЂ Р С‘РЎРЏ Grids
    // ========================================================================

    // Р РЋР С•Р В·Р Т‘Р В°Р ВµР С РЎвЂћР В°РЎРѓР В°Р Т‘
    let mut vg = VoidGrid::new();

    // Р РЋР С•Р В·Р Т‘Р В°Р ВµР С Р С—РЎР‚Р С•Р Р†Р В°Р в„–Р Т‘Р ВµРЎР‚ РЎР‚Р ВµРЎРѓРЎС“РЎР‚РЎРѓР С•Р Р†
    let zip_file = std::fs::File::open("crtdemo.vpk")
        .expect("Р СњР Вµ РЎС“Р Т‘Р В°Р В»Р С•РЎРѓРЎРЉ Р Р…Р В°Р в„–РЎвЂљР С‘ РЎвЂћР В°Р в„–Р В» crtdemo.vpk");

    let mut provider = voidgrid_resource_packs::ZipProvider::new(zip_file)
        .expect("Р СњР Вµ РЎС“Р Т‘Р В°Р В»Р С•РЎРѓРЎРЉ Р С—РЎР‚Р С•РЎвЂЎР С‘РЎвЂљР В°РЎвЂљРЎРЉ РЎРѓРЎвЂљРЎР‚РЎС“Р С”РЎвЂљРЎС“РЎР‚РЎС“ ZIP-Р В°РЎР‚РЎвЂ¦Р С‘Р Р†Р В°");

    // Р ВР Р…Р С‘РЎвЂ Р С‘Р В°Р В»Р С‘Р В·Р С‘РЎР‚РЎС“Р ВµР С (Р В·Р В°Р С–РЎР‚РЎС“Р В·Р С”Р В° РЎв‚¬Р ВµР в„–Р Т‘Р ВµРЎР‚Р С•Р Р† Р С‘ РЎвЂљ.Р Т‘.)
    vg.init(&mut provider, &mut rl, &thread);

    // Р ВР Р…Р С‘РЎвЂ Р С‘Р В°Р В»Р С‘Р В·Р С‘РЎР‚РЎС“Р ВµР С Р С‘Р ВµРЎР‚Р В°РЎР‚РЎвЂ¦Р С‘РЎР‹
    let mut hierarchy = Hierarchy::new();

    // 2. Р вЂ”Р В°Р С–РЎР‚РЎС“Р В¶Р В°Р ВµР С РЎРѓРЎвЂ Р ВµР Р…РЎС“ Р С‘Р В· Р СР В°Р Р…Р С‘РЎвЂћР ВµРЎРѓРЎвЂљР В°
    let pack = PackLoader::load_pack(
        &mut vg, 
        &mut hierarchy, 
        &mut provider, 
        "manifest.json", 
        &mut rl, 
        &thread
    ).expect("Failed to load scene from manifest");

    // 3. Р ВР В·Р Р†Р В»Р ВµР С”Р В°Р ВµР С Р С”Р В»РЎР‹РЎвЂЎР С‘ Р В±РЎС“РЎвЂћР ВµРЎР‚Р С•Р Р† Р Т‘Р В»РЎРЏ Р С‘РЎРѓР С—Р С•Р В»РЎРЉР В·Р С•Р Р†Р В°Р Р…Р С‘РЎРЏ Р Р† Р С–Р В»Р В°Р Р†Р Р…Р С•Р С РЎвЂ Р С‘Р С”Р В»Р Вµ
    vg.terminal.register_buffers(pack.buffers.clone());
    let main_buf = pack.buffers["main_buf"];
    let back_buf = pack.buffers["back_buf"];
    let drop_zone_buf = pack.buffers["drop_zone_buf"];
    let shader_demo_buf = pack.buffers["shader_demo_buf"];

    // 4. Р СџР С•Р В»РЎС“РЎвЂЎР В°Р ВµР С РЎР‚Р В°Р В·Р СР ВµРЎР‚РЎвЂ№ РЎвЂљР В°Р в„–Р В»Р В° Р С‘Р В· Р С”Р С•РЎР‚Р Р…Р ВµР Р†Р С•Р С–Р С• Р В±РЎС“РЎвЂћР ВµРЎР‚Р В° Р Т‘Р В»РЎРЏ Р Р…Р В°РЎРѓРЎвЂљРЎР‚Р С•Р в„–Р С”Р С‘ Р С•Р С”Р Р…Р В°
    let main_glyphset = vg.grids.get(main_buf).unwrap().glyphset();
    let (tile_w, tile_h) = vg.grids.assets.glyphset_size(main_glyphset).unwrap();

    // Р С™Р С•РЎР‚РЎР‚Р ВµР С”РЎвЂљР С‘РЎР‚РЎС“Р ВµР С РЎР‚Р В°Р В·Р СР ВµРЎР‚ Р С•Р С”Р Р…Р В°
    let window_w = buf_w * tile_w;
    let window_h = buf_h * tile_h;
    rl.set_window_size(window_w as i32, window_h as i32);

    // Р вЂ™Р С•РЎРѓРЎРѓРЎвЂљР В°Р Р…Р В°Р Р†Р В»Р С‘Р Р†Р В°Р ВµР С Р Т‘Р С•РЎРѓРЎвЂљРЎС“Р С— Р С” РЎв‚¬Р ВµР в„–Р Т‘Р ВµРЎР‚РЎС“ Р Т‘Р В»РЎРЏ Р В°Р Р…Р С‘Р СР В°РЎвЂ Р С‘Р С‘ Р Р† РЎвЂ Р С‘Р С”Р В»Р Вµ
    let chromatic_shader = vg.grids.assets.load_shader(&mut provider, &mut rl, &thread, "assets/chromatic.fs").expect("Failed to load chromatic shader");

    // Drop zone state
    let mut drop_zone = DropZone::new();

    // Р вЂ™РЎвЂ№Р Р†Р С•Р Т‘Р С‘Р С РЎРѓР С•Р Т‘Р ВµРЎР‚Р В¶Р С‘Р СР С•Р Вµ РЎР‚Р ВµР ВµРЎРѓРЎвЂљРЎР‚Р В° Р Т‘Р В»РЎРЏ Р С•РЎвЂљР В»Р В°Р Т‘Р С”Р С‘
    vg.grids.assets.debug_print_registry();

    let mut start_time: Instant;
    start_time = Instant::now();
    let current_time = start_time.elapsed().as_secs_f32();






// Р ВР Р…Р С‘РЎвЂ Р С‘Р В°Р В»Р С‘Р В·Р С‘РЎР‚РЎС“Р ВµР С Р С—Р В°РЎР‚РЎРѓР ВµРЎР‚ VTP
let mut vtp_parser = VtpParser::new();
    // Terminal state moved to vg.terminal

// Р С™Р В°Р Р…Р В°Р В» Р Т‘Р В»РЎРЏ Р С—Р ВµРЎР‚Р ВµР Т‘Р В°РЎвЂЎР С‘ РЎРѓРЎвЂ№РЎР‚РЎвЂ№РЎвЂ¦ Р В±Р В°Р в„–РЎвЂљ Р С‘Р В· РЎРѓР ВµРЎвЂљР С‘ Р Р† Р С–Р В»Р В°Р Р†Р Р…РЎвЂ№Р в„– РЎвЂ Р С‘Р С”Р В»
let (tx, rx) = mpsc::channel::<Vec<u8>>();

// Р В¤Р С•Р Р…Р С•Р Р†РЎвЂ№Р в„– Р С—Р С•РЎвЂљР С•Р С” TCP-РЎРѓР ВµРЎР‚Р Р†Р ВµРЎР‚Р В°
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
                            // Р С›РЎвЂљР С—РЎР‚Р В°Р Р†Р В»РЎРЏР ВµР С Р С—РЎР‚Р С•РЎвЂЎР С‘РЎвЂљР В°Р Р…Р Р…РЎвЂ№Р Вµ Р В±Р В°Р в„–РЎвЂљРЎвЂ№ Р Р† Р С–Р В»Р В°Р Р†Р Р…РЎвЂ№Р в„– Р С—Р С•РЎвЂљР С•Р С”
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
    // Р вЂњР вЂєР С’Р вЂ™Р СњР В«Р в„ў Р В¦Р ВР С™Р вЂє
    // ========================================================================

            // --- Rhai Initialization ---
    let mut script_engine = voidgrid::scripting::ScriptEngine::new();
    
    // РђРІС‚Рѕ-Р·Р°РіСЂСѓР·РєР° РІСЃРµС… СЃРєСЂРёРїС‚РѕРІ РёР· РјР°РЅРёС„РµСЃС‚Р°
    for (name, code) in &pack.scripts {
        if let Err(e) = script_engine.load_script(name, code) {
            eprintln!("Failed to load pack script '{}': {}", name, e);
        }
    }

    // Р’РђР–РќРћ: Р’С‹Р·С‹РІР°РµРј run_init() С‚РѕР»СЊРєРѕ РџРћРЎР›Р• Р·Р°РіСЂСѓР·РєРё РІСЃРµС… СЃРєСЂРёРїС‚РѕРІ РёР· РїР°РєР°!
    script_engine.run_init();
    // ---------------------------
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
        // --- Р С›РЎвЂЎР С‘РЎРѓРЎвЂљР С”Р В° Р В±РЎС“РЎвЂћР ВµРЎР‚Р С•Р Р† ---
        if is_resized {
            vg.grids.clear_buffer(main_buf);
            is_resized = false;
        }
        //
        vg.grids.clear_buffer(drop_zone_buf);
        vg.grids.clear_buffer(shader_demo_buf);

        // --- Р С›Р В±РЎР‚Р В°Р В±Р С•РЎвЂљР С”Р В° window chrome ---
        if chrome.update(&mut rl) {
            break;
        }

        // --- Р С›Р В±РЎР‚Р В°Р В±Р С•РЎвЂљР С”Р В° drag-n-drop ---
        if let Some(filename) = drop_zone.update(&vg.events.frame_events) {
            println!("Dropped: {}", filename);
        }

        // --- Р СџРЎР‚Р С•Р Р†Р ВµРЎР‚Р С”Р В° resize Р С‘ Р С•Р В±Р Р…Р С•Р Р†Р В»Р ВµР Р…Р С‘Р Вµ Р В±РЎС“РЎвЂћР ВµРЎР‚Р В° ---
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

        // Р С›Р В±РЎР‚Р В°Р В±Р В°РЎвЂљРЎвЂ№Р Р†Р В°Р ВµР С Р Р†РЎРѓР Вµ Р С—Р В°Р С”Р ВµРЎвЂљРЎвЂ№, Р С—РЎР‚Р С‘РЎв‚¬Р ВµР Т‘РЎв‚¬Р С‘Р Вµ Р С‘Р В· РЎРѓР ВµРЎвЂљР С‘ Р В·Р В° РЎРЊРЎвЂљР С•РЎвЂљ Р С”Р В°Р Т‘РЎР‚
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
// --- Р РЋРЎвЂљРЎР‚Р С•Р С”Р В° РЎРѓРЎвЂљР В°РЎвЂљРЎС“РЎРѓР В° ---
                // --- Execute Rhai Script Frame ---
        script_engine.sync_state(&vg.grids, &pack.buffers);
        script_engine.run_update(current_time, &vg.events.frame_events);
        
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

        // --- Р вЂРЎС“РЎвЂћР ВµРЎР‚ РЎРѓ РЎв‚¬Р ВµР в„–Р Т‘Р ВµРЎР‚Р С•Р С ---

        let status_text = format!("VOIDGRID _ {:.2}", current_time);

        if ((current_time * 5.0).floor() % 2.0) == 0.0 {
            vg.grids
                .print(main_buf)
                .at(4, 3)
                .fg(Color::new(0, 255, 127, 255))
                .write("TWELVE\nCATHODE\nTELEVISION TUBES\n")
                .write(("FLICKERING\n", "inverted")) // Р ВРЎРѓР С—РЎР‚Р В°Р Р†Р В»Р ВµР Р…Р С•: РЎС“Р В±РЎР‚Р В°Р Р…РЎвЂ№ Р В»Р С‘РЎв‚¬Р Р…Р С‘Р Вµ РЎРѓР С”Р С•Р В±Р С”Р С‘, Р Р…Р С• Р В·Р Т‘Р ВµРЎРѓРЎРЉ Р С”Р С•РЎР‚РЎвЂљР ВµР В¶ Р Р…РЎС“Р В¶Р ВµР Р… Р Т‘Р В»РЎРЏ Printable
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

        // --- Drop zone Р В±РЎС“РЎвЂћР ВµРЎР‚ ---
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

        // --- Р С›РЎвЂљРЎР‚Р С‘РЎРѓР С•Р Р†Р С”Р В° ---
        puffin::profile_scope!("Prepare Render");
        // Р С’Р Р…Р С‘Р СР С‘РЎР‚РЎС“Р ВµР С РЎРѓР СР ВµРЎвЂ°Р ВµР Р…Р С‘Р Вµ chromatic aberration
        let aberration = (vg.renderer.shader_time() * 3.0).sin() * 1.5 + 1.5; // Р С•РЎвЂљ 1 Р Т‘Р С• 5 Р С—Р С‘Р С”РЎРѓР ВµР В»Р ВµР в„–
        vg.grids.assets
            .set_shader_float(chromatic_shader, "offset", aberration);

        // Р РЋР С•Р В±Р С‘РЎР‚Р В°Р ВµР С РЎРѓР С—Р С‘РЎРѓР С•Р С” РЎР‚Р ВµР Р…Р Т‘Р ВµРЎР‚Р С‘Р Р…Р С–Р В° Р С‘Р В· Р С‘Р ВµРЎР‚Р В°РЎР‚РЎвЂ¦Р С‘Р С‘
        let render_list = hierarchy.collect_render_list(|b| {
            if let Some(buf) = vg.grids.get(b) {
                if let Some((tw, th)) = vg.grids.assets.glyphset_size(buf.glyphset()) {
                    return (buf.w, buf.h, tw, th);
                }
            }
            (0, 0, 1, 1)
        });

        // Р вЂќР Р†РЎС“РЎвЂ¦Р С—РЎР‚Р С•РЎвЂ¦Р С•Р Т‘Р Р…РЎвЂ№Р в„– РЎР‚Р ВµР Р…Р Т‘Р ВµРЎР‚: РЎРѓР Р…Р В°РЎвЂЎР В°Р В»Р В° Р В±РЎС“РЎвЂћР ВµРЎР‚РЎвЂ№ РЎРѓ РЎв‚¬Р ВµР в„–Р Т‘Р ВµРЎР‚Р В°Р СР С‘ Р Р† Р С‘РЎвЂ¦ РЎвЂљР ВµР С”РЎРѓРЎвЂљРЎС“РЎР‚РЎвЂ№
        vg.render_offscreen(&mut rl, &thread, &render_list);

        // Р СџР С•РЎвЂљР С•Р С РЎР‚Р С‘РЎРѓРЎС“Р ВµР С Р Р†РЎРѓРЎвЂ Р Р…Р В° РЎРЊР С”РЎР‚Р В°Р Р…
        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::new(8, 8, 8, 255));
            puffin::profile_scope!("Offscreen Render");
            // draw РЎР‚Р С‘РЎРѓРЎС“Р ВµРЎвЂљ Р Т‘Р ВµРЎР‚Р ВµР Р†Р С• + Р С—РЎР‚Р С‘Р СР ВµР Р…РЎРЏР ВµРЎвЂљ РЎв‚¬Р ВµР в„–Р Т‘Р ВµРЎР‚РЎвЂ№ Р С” Р В±РЎС“РЎвЂћР ВµРЎР‚Р В°Р С (РЎвЂЎР ВµРЎР‚Р ВµР В· РЎвЂћР В°РЎРѓР В°Р Т‘)
            vg.draw(&mut d, &render_list);

            chrome.draw(&mut d);
            // d.draw_fps(10, 10);
        }
    }
}













