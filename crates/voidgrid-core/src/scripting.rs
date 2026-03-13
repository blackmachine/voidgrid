use rhai::{Engine, Scope, AST};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::terminal::Action;
use raylib::prelude::Color;

pub struct ScriptEngine {
    engine: Engine,
    scope: Scope<'static>,
    asts: HashMap<String, AST>,
    action_queue: Arc<Mutex<Vec<Action>>>,
}

impl ScriptEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        let action_queue = Arc::new(Mutex::new(Vec::new()));

        engine.register_fn("get_system_time", || -> rhai::ImmutableString {
            chrono::Local::now().format("%H:%M:%S").to_string().into()
        });

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

        Self {
            engine,
            scope: Scope::new(),
            asts: HashMap::new(),
            action_queue,
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
        // Р§С‚РѕР±С‹ РёР·Р±РµР¶Р°С‚СЊ РїСЂРѕР±Р»РµРј СЃ borrow checker РїСЂРё РёС‚РµСЂР°С†РёРё Рё РјСѓС‚Р°С†РёРё scope, 
        // РєР»РѕРЅРёСЂСѓРµРј AST (РІ Rhai AST РєР»РѕРЅРёСЂСѓРµС‚СЃСЏ РѕС‡РµРЅСЊ РґРµС€РµРІРѕ, СЌС‚Рѕ Arc РІРЅСѓС‚СЂРё)
        let asts_to_run: Vec<AST> = self.asts.values().cloned().collect();
        for ast in asts_to_run {
            if let Err(e) = self.engine.run_ast_with_scope(&mut self.scope, &ast) {
                eprintln!("Script Init Error: {}", e);
            }
        }
    }

    pub fn run_update(&mut self, time: f32) {
        let asts_to_run: Vec<AST> = self.asts.values().cloned().collect();
        for ast in asts_to_run {
            let result: Result<(), Box<rhai::EvalAltResult>> = self.engine.call_fn(&mut self.scope, &ast, "update", (time as f64,));
            if let Err(e) = result {
                let err_str = e.to_string();
                if !err_str.contains("not found") { // РРіРЅРѕСЂРёСЂСѓРµРј СЃРєСЂРёРїС‚С‹, РІ РєРѕС‚РѕСЂС‹С… РїСЂРѕСЃС‚Рѕ РЅРµС‚ С„СѓРЅРєС†РёРё update
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

