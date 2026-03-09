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

    // Р—Р°РїСѓСЃРєР°РµРј СЃРµСЂРІРµСЂ РЅР° РїРѕСЂС‚Сѓ 8585
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

    // РќР°С‡Р°Р»СЊРЅС‹Рµ СЂР°Р·РјРµСЂС‹ Р±СѓС„РµСЂР° РІ СЃРёРјРІРѕР»Р°С…
    let mut buf_w: u32 = 64;
    let mut buf_h: u32 = 32;

    // РРЅРёС†РёР°Р»РёР·Р°С†РёСЏ РѕРєРЅР° (undecorated)
    let (mut rl, thread) = raylib::init()
        .size(800, 600)
        .title("Grids TUI System")
        .undecorated()
        .resizable()
        .build();

    rl.set_target_fps(60);

    // Window chrome (РєРЅРѕРїРєРё Р·Р°РєСЂС‹С‚РёСЏ, maximize Рё drag handle)
    let mut chrome = WindowChrome::new(800, 600);

    // ========================================================================
    // РРЅРёС†РёР°Р»РёР·Р°С†РёСЏ Grids
    // ========================================================================

    // РЎРѕР·РґР°РµРј С„Р°СЃР°Рґ
    let mut vg = VoidGrid::new();

    // РЎРѕР·РґР°РµРј РїСЂРѕРІР°Р№РґРµСЂ СЂРµСЃСѓСЂСЃРѕРІ
    let zip_file = std::fs::File::open("crtdemo.vpk")
        .expect("РќРµ СѓРґР°Р»РѕСЃСЊ РЅР°Р№С‚Рё С„Р°Р№Р» crtdemo.vpk");

    let mut provider = voidgrid::resource_pack::ZipProvider::new(zip_file)
        .expect("РќРµ СѓРґР°Р»РѕСЃСЊ РїСЂРѕС‡РёС‚Р°С‚СЊ СЃС‚СЂСѓРєС‚СѓСЂСѓ ZIP-Р°СЂС…РёРІР°");

    // РРЅРёС†РёР°Р»РёР·РёСЂСѓРµРј (Р·Р°РіСЂСѓР·РєР° С€РµР№РґРµСЂРѕРІ Рё С‚.Рґ.)
    vg.init(&mut provider, &mut rl, &thread);

    // РРЅРёС†РёР°Р»РёР·РёСЂСѓРµРј РёРµСЂР°СЂС…РёСЋ
    let mut hierarchy = Hierarchy::new();

    // 2. Р—Р°РіСЂСѓР¶Р°РµРј СЃС†РµРЅСѓ РёР· РјР°РЅРёС„РµСЃС‚Р°
    let buffers = PackLoader::load_pack(
        &mut vg, 
        &mut hierarchy, 
        &mut provider, 
        "manifest.json", 
        &mut rl, 
        &thread
    ).expect("Failed to load scene from manifest");

    // 3. РР·РІР»РµРєР°РµРј РєР»СЋС‡Рё Р±СѓС„РµСЂРѕРІ РґР»СЏ РёСЃРїРѕР»СЊР·РѕРІР°РЅРёСЏ РІ РіР»Р°РІРЅРѕРј С†РёРєР»Рµ
    let main_buf = buffers["main_buf"];
    let back_buf = buffers["back_buf"];
    let drop_zone_buf = buffers["drop_zone_buf"];
    let shader_demo_buf = buffers["shader_demo_buf"];

    // 4. РџРѕР»СѓС‡Р°РµРј СЂР°Р·РјРµСЂС‹ С‚Р°Р№Р»Р° РёР· РєРѕСЂРЅРµРІРѕРіРѕ Р±СѓС„РµСЂР° РґР»СЏ РЅР°СЃС‚СЂРѕР№РєРё РѕРєРЅР°
    let main_glyphset = vg.grids.get(main_buf).unwrap().glyphset();
    let (tile_w, tile_h) = vg.grids.assets.glyphset_size(main_glyphset).unwrap();

    // РљРѕСЂСЂРµРєС‚РёСЂСѓРµРј СЂР°Р·РјРµСЂ РѕРєРЅР°
    let window_w = buf_w * tile_w;
    let window_h = buf_h * tile_h;
    rl.set_window_size(window_w as i32, window_h as i32);

    // Р’РѕСЃСЃС‚Р°РЅР°РІР»РёРІР°РµРј РґРѕСЃС‚СѓРї Рє С€РµР№РґРµСЂСѓ РґР»СЏ Р°РЅРёРјР°С†РёРё РІ С†РёРєР»Рµ
    let chromatic_shader = vg.grids.assets.load_shader(&mut provider, &mut rl, &thread, "assets/chromatic.fs").expect("Failed to load chromatic shader");

    // Drop zone state
    let mut drop_zone = DropZone::new();

    // Р’С‹РІРѕРґРёРј СЃРѕРґРµСЂР¶РёРјРѕРµ СЂРµРµСЃС‚СЂР° РґР»СЏ РѕС‚Р»Р°РґРєРё
    vg.grids.assets.debug_print_registry();

    let mut start_time: Instant;
    start_time = Instant::now();
    let current_time = start_time.elapsed().as_secs_f32();






// РРЅРёС†РёР°Р»РёР·РёСЂСѓРµРј РїР°СЂСЃРµСЂ VTP
let mut vtp_parser = VtpParser::new();
    let mut vtp_active_buffer = None;
    let mut vtp_cursor_x = 0;
    let mut vtp_cursor_y = 0;
    let mut vtp_fg_color = Color::WHITE;
    let mut vtp_bg_color = Color::BLANK;
    let mut vtp_variant_id = 0;

// РљР°РЅР°Р» РґР»СЏ РїРµСЂРµРґР°С‡Рё СЃС‹СЂС‹С… Р±Р°Р№С‚ РёР· СЃРµС‚Рё РІ РіР»Р°РІРЅС‹Р№ С†РёРєР»
let (tx, rx) = mpsc::channel::<Vec<u8>>();

// Р¤РѕРЅРѕРІС‹Р№ РїРѕС‚РѕРє TCP-СЃРµСЂРІРµСЂР°
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
                            // РћС‚РїСЂР°РІР»СЏРµРј РїСЂРѕС‡РёС‚Р°РЅРЅС‹Рµ Р±Р°Р№С‚С‹ РІ РіР»Р°РІРЅС‹Р№ РїРѕС‚РѕРє
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
    // Р“Р›РђР’РќР«Р™ Р¦РРљР›
    // ========================================================================

    let mut is_resized = false;


    while !rl.window_should_close() {
        puffin::GlobalProfiler::lock().new_frame();
        puffin::profile_scope!("Main Loop");
        // --- РћС‡РёСЃС‚РєР° Р±СѓС„РµСЂРѕРІ ---
        if is_resized {
            vg.grids.clear_buffer(main_buf);
            is_resized = false;
        }
        //
        vg.grids.clear_buffer(drop_zone_buf);
        vg.grids.clear_buffer(shader_demo_buf);

        // --- РћР±СЂР°Р±РѕС‚РєР° window chrome ---
        if chrome.update(&mut rl) {
            break;
        }

        // --- РћР±СЂР°Р±РѕС‚РєР° drag-n-drop ---
        if let Some(filename) = drop_zone.update(&mut rl) {
            println!("Dropped: {}", filename);
        }

        // --- РџСЂРѕРІРµСЂРєР° resize Рё РѕР±РЅРѕРІР»РµРЅРёРµ Р±СѓС„РµСЂР° ---
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

        // РћР±СЂР°Р±Р°С‚С‹РІР°РµРј РІСЃРµ РїР°РєРµС‚С‹, РїСЂРёС€РµРґС€РёРµ РёР· СЃРµС‚Рё Р·Р° СЌС‚РѕС‚ РєР°РґСЂ
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

        // --- РЎС‚СЂРѕРєР° СЃС‚Р°С‚СѓСЃР° ---
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

        // --- Р‘СѓС„РµСЂ СЃ С€РµР№РґРµСЂРѕРј ---

        let status_text = format!("VOIDGRID _ {:.2}", current_time);

        if ((current_time * 5.0).floor() % 2.0) == 0.0 {
            vg.grids
                .print(main_buf)
                .at(4, 3)
                .fg(Color::new(0, 255, 127, 255))
                .write("TWELVE\nCATHODE\nTELEVISION TUBES\n")
                .write(("FLICKERING\n", "inverted")) // РСЃРїСЂР°РІР»РµРЅРѕ: СѓР±СЂР°РЅС‹ Р»РёС€РЅРёРµ СЃРєРѕР±РєРё, РЅРѕ Р·РґРµСЃСЊ РєРѕСЂС‚РµР¶ РЅСѓР¶РµРЅ РґР»СЏ Printable
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

        // --- Drop zone Р±СѓС„РµСЂ ---
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

        // --- РћС‚СЂРёСЃРѕРІРєР° ---
        puffin::profile_scope!("Prepare Render");
        // РђРЅРёРјРёСЂСѓРµРј СЃРјРµС‰РµРЅРёРµ chromatic aberration
        let aberration = (vg.renderer.shader_time() * 3.0).sin() * 1.5 + 1.5; // РѕС‚ 1 РґРѕ 5 РїРёРєСЃРµР»РµР№
        vg.grids.assets
            .set_shader_float(chromatic_shader, "offset", aberration);

        // РЎРѕР±РёСЂР°РµРј СЃРїРёСЃРѕРє СЂРµРЅРґРµСЂРёРЅРіР° РёР· РёРµСЂР°СЂС…РёРё
        let render_list = hierarchy.collect_render_list(|b| {
            if let Some(buf) = vg.grids.get(b) {
                if let Some((tw, th)) = vg.grids.assets.glyphset_size(buf.glyphset()) {
                    return (buf.w, buf.h, tw, th);
                }
            }
            (0, 0, 1, 1)
        });

        // Р”РІСѓС…РїСЂРѕС…РѕРґРЅС‹Р№ СЂРµРЅРґРµСЂ: СЃРЅР°С‡Р°Р»Р° Р±СѓС„РµСЂС‹ СЃ С€РµР№РґРµСЂР°РјРё РІ РёС… С‚РµРєСЃС‚СѓСЂС‹
        vg.render_offscreen(&mut rl, &thread, &render_list);

        // РџРѕС‚РѕРј СЂРёСЃСѓРµРј РІСЃС‘ РЅР° СЌРєСЂР°РЅ
        {
            let mut d = rl.begin_drawing(&thread);
            d.clear_background(Color::new(8, 8, 8, 255));
            puffin::profile_scope!("Offscreen Render");
            // draw СЂРёСЃСѓРµС‚ РґРµСЂРµРІРѕ + РїСЂРёРјРµРЅСЏРµС‚ С€РµР№РґРµСЂС‹ Рє Р±СѓС„РµСЂР°Рј (С‡РµСЂРµР· С„Р°СЃР°Рґ)
            vg.draw(&mut d, &render_list);

            chrome.draw(&mut d);
            // d.draw_fps(10, 10);
        }
    }
}

