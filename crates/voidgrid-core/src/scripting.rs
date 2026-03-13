use rhai::{Engine, Scope, AST, Map, Array, Dynamic};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::terminal::Action;
use crate::events::{Event, MouseButton};
use raylib::prelude::Color;

pub struct ScriptEngine {
    engine: Engine,
    scope: Scope<'static>,
    asts: HashMap<String, AST>,
    action_queue: Arc<Mutex<Vec<Action>>>,
    buffer_sizes: Arc<Mutex<HashMap<String, (u32, u32)>>>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_max_expr_depths(0, 0); // РћС‚РєР»СЋС‡Р°РµРј РїР°СЂР°РЅРѕРёРґР°Р»СЊРЅС‹Рµ Р»РёРјРёС‚С‹
        let action_queue = Arc::new(Mutex::new(Vec::new()));
        let buffer_sizes = Arc::new(Mutex::new(HashMap::new()));

        let q_buf = action_queue.clone();
        engine.register_fn("set_buffer", move |name: rhai::ImmutableString| {
            q_buf.lock().unwrap().push(Action::SetBuffer(name.to_string()));
        });

        let q_cur = action_queue.clone();
        engine.register_fn("set_cursor", move |x: i64, y: i64| {
            q_cur.lock().unwrap().push(Action::SetCursor(x as u32, y as u32));
        });

        let q_print = action_queue.clone();
        engine.register_fn("write_text", move |text: rhai::ImmutableString| {
            q_print.lock().unwrap().push(Action::PrintString(text.to_string()));
        });

        let q_fg = action_queue.clone();
        engine.register_fn("set_fg", move |r: i64, g: i64, b: i64, a: i64| {
            q_fg.lock().unwrap().push(Action::SetFgColor(Color::new(r as u8, g as u8, b as u8, a as u8)));
        });

        engine.register_fn("get_system_time", || -> rhai::ImmutableString {
            chrono::Local::now().format("%H:%M:%S").to_string().into()
        });

        let b_w = buffer_sizes.clone();
        engine.register_fn("get_buffer_width", move |name: rhai::ImmutableString| -> i64 {
            b_w.lock().unwrap().get(name.as_str()).map(|&(w, _)| w as i64).unwrap_or(0)
        });

        let b_h = buffer_sizes.clone();
        engine.register_fn("get_buffer_height", move |name: rhai::ImmutableString| -> i64 {
            b_h.lock().unwrap().get(name.as_str()).map(|&(_, h)| h as i64).unwrap_or(0)
        });

        Self {
            engine,
            scope: Scope::new(),
            asts: HashMap::new(),
            action_queue,
            buffer_sizes,
        }
    }

    pub fn load_script(&mut self, name: &str, script_text: &str) -> Result<(), String> {
        match self.engine.compile(script_text) {
            Ok(ast) => {
                self.asts.insert(name.to_string(), ast);
                Ok(())
            }
            Err(e) => Err(format!("Script compile error [{}]: {}", name, e)),
        }
    }

    pub fn run_init(&mut self) {
        let asts_to_run: Vec<AST> = self.asts.values().cloned().collect();
        for ast in asts_to_run {
            if let Err(e) = self.engine.run_ast_with_scope(&mut self.scope, &ast) {
                eprintln!("Script Init Error: {}", e);
            }
        }
    }

    pub fn sync_state(&self, grids: &crate::grids::Grids, buffer_map: &HashMap<String, crate::types::BufferKey>) {
        let mut sizes = self.buffer_sizes.lock().unwrap();
        sizes.clear();
        for (name, &key) in buffer_map {
            if let Some((w, h)) = grids.buffer_size(key) {
                sizes.insert(name.clone(), (w, h));
            }
        }
    }

    // РўР•РџР•Р Р¬ РџР РРќРРњРђР•Рў РњРђРЎРЎРР’ РЎРћР‘Р«РўРР™!
        pub fn run_update(&mut self, time: f32, frame_events: &[Event]) {
        // --- РћР±РЅРѕРІР»СЏРµРј РіР»РѕР±Р°Р»СЊРЅСѓСЋ РїР°РјСЏС‚СЊ СЃРєСЂРёРїС‚РѕРІ (Scope) ---
        // Р­С‚Рё РїРµСЂРµРјРµРЅРЅС‹Рµ Р±СѓРґСѓС‚ РґРѕСЃС‚СѓРїРЅС‹ РІ Р»СЋР±РѕРј СЃРєСЂРёРїС‚Рµ РєР°Рє РєРѕРЅСЃС‚Р°РЅС‚С‹!
        self.scope.set_or_push("TIME", time as f64);
        
        // ---
        // 1. РљРѕРЅРІРµСЂС‚РёСЂСѓРµРј Rust Events РІ Rhai Array of Maps
        let mut rhai_events = Array::new();
        for ev in frame_events {
            let mut map = Map::new();
            match ev {
                Event::MousePress { x, y, button } => {
                    map.insert("type".into(), Dynamic::from("MousePress"));
                    map.insert("x".into(), Dynamic::from(*x as f64));
                    map.insert("y".into(), Dynamic::from(*y as f64));
                    let btn = match button { MouseButton::Left => "Left", MouseButton::Right => "Right", MouseButton::Middle => "Middle" };
                    map.insert("button".into(), Dynamic::from(btn));
                }
                Event::MouseRelease { x, y, button } => {
                    map.insert("type".into(), Dynamic::from("MouseRelease"));
                    map.insert("x".into(), Dynamic::from(*x as f64));
                    map.insert("y".into(), Dynamic::from(*y as f64));
                    let btn = match button { MouseButton::Left => "Left", MouseButton::Right => "Right", MouseButton::Middle => "Middle" };
                    map.insert("button".into(), Dynamic::from(btn));
                }
                Event::WindowResize { width, height } => {
                    map.insert("type".into(), Dynamic::from("WindowResize"));
                    map.insert("width".into(), Dynamic::from(*width as i64));
                    map.insert("height".into(), Dynamic::from(*height as i64));
                }
                Event::KeyPress { key } => {
                    map.insert("type".into(), Dynamic::from("KeyPress"));
                    map.insert("key".into(), Dynamic::from(*key as i64));
                }
                Event::FileDrop { path } => {
                    map.insert("type".into(), Dynamic::from("FileDrop"));
                    map.insert("path".into(), Dynamic::from(path.clone()));
                }
                _ => {} // MouseMove РїРѕРєР° РёРіРЅРѕСЂРёСЂСѓРµРј, С‡С‚РѕР±С‹ РЅРµ СЃРїР°РјРёС‚СЊ РІ СЃРєСЂРёРїС‚ РєР°Р¶РґС‹Р№ РєР°РґСЂ
            }
            if !map.is_empty() {
                rhai_events.push(Dynamic::from_map(map));
            }
        }

        // 2. Р’С‹Р·С‹РІР°РµРј СЃРєСЂРёРїС‚С‹, РїРµСЂРµРґР°РІР°СЏ РёРј РІСЂРµРјСЏ Рё РјР°СЃСЃРёРІ СЃРѕР±С‹С‚РёР№
        let asts_to_run: Vec<AST> = self.asts.values().cloned().collect();
        for ast in asts_to_run {
            let result: Result<(), Box<rhai::EvalAltResult>> = self.engine.call_fn(
                &mut self.scope, 
                &ast, 
                "update", 
                (rhai_events.clone(),) // РџРµСЂРµРґР°РµРј 2 Р°СЂРіСѓРјРµРЅС‚Р°!
            );
            
            if let Err(e) = result {
                let err_str = e.to_string();
                if !err_str.contains("not found") && !err_str.contains("functions") {
                    eprintln!("Rhai Update Error: {}", err_str);
                }
            }
        }
    }

    pub fn take_actions(&self) -> Vec<Action> {
        let mut q = self.action_queue.lock().unwrap();
        let actions = q.clone();
        q.clear();
        actions
    }
}


