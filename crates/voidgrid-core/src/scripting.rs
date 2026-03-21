use rhai::{Engine, Scope, AST, Map, Array, Dynamic};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use crate::terminal::Action;
use crate::events::{Event, MouseButton};
use raylib::prelude::Color;

pub struct ScriptEngine {
    engine: Engine,
    scope: Scope<'static>,
    asts: HashMap<String, AST>,
    states: HashMap<String, Dynamic>,
    action_queue: Arc<Mutex<Vec<Action>>>,
    buffer_sizes: Arc<Mutex<HashMap<String, (u32, u32)>>>,
    /// Скрипты, в которых нет функции update (чтобы не спамить ошибкой каждый кадр)
    missing_update: HashSet<String>,
    /// Скрипты, в которых нет функции init
    missing_init: HashSet<String>,
    /// Ошибки, которые уже были залогированы (антиспам для runtime errors в update)
    logged_errors: HashMap<String, u64>,
    /// Счётчик кадров для throttle
    frame_count: u64,
}

/// Проверяет, является ли ошибка Rhai "function not found" для конкретной функции.
/// Не глотает другие "not found" ошибки (переменные, свойства и т.д.)
fn is_fn_not_found(err: &rhai::EvalAltResult, fn_name: &str) -> bool {
    matches!(err, rhai::EvalAltResult::ErrorFunctionNotFound(name, _) if name.starts_with(fn_name))
}

/// Рекурсивно разворачивает вложенные ошибки Rhai для более информативного вывода
fn format_rhai_error(err: &rhai::EvalAltResult) -> String {
    match err {
        rhai::EvalAltResult::ErrorInFunctionCall(fn_name, _src, inner, pos) => {
            format!("in function '{}' {}: {}", fn_name, pos, format_rhai_error(inner))
        }
        other => other.to_string(),
    }
}

impl ScriptEngine {
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_max_expr_depths(0, 0);
        let action_queue = Arc::new(Mutex::new(Vec::new()));
        let buffer_sizes = Arc::new(Mutex::new(HashMap::new()));

        let q_buf = action_queue.clone();
        engine.register_fn("set_buffer", move |name: rhai::ImmutableString| {
            q_buf.lock().unwrap().push(Action::SetBuffer(name.to_string()));
        });

        // --- Debug/Logging functions ---

        engine.register_fn("log", move |logstring: rhai::ImmutableString| {
            println!("[rhai] {}", logstring);
        });

        engine.register_fn("log", move |value: Dynamic| {
            println!("[rhai] {:?}", value);
        });

        engine.register_fn("dbg", move |value: Dynamic| -> Dynamic {
            println!("[rhai:dbg] {:?}", value);
            value
        });

        engine.register_fn("dump", move |label: rhai::ImmutableString, value: Dynamic| -> Dynamic {
            println!("[rhai:dump] {} = {:?}", label, value);
            value
        });

        engine.register_fn("type_of_dbg", move |value: Dynamic| -> rhai::ImmutableString {
            let t = value.type_name().to_string();
            println!("[rhai:type] {}", t);
            t.into()
        });

        // --- Rendering functions ---

        let q_cur = action_queue.clone();
        engine.register_fn("set_cursor", move |x: i64, y: i64| {
            q_cur.lock().unwrap().push(Action::SetCursor(x as u32, y as u32));
        });

        let q_bg = action_queue.clone();
        engine.register_fn("set_bg", move |r: i64, g: i64, b: i64, a: i64| {
            q_bg.lock().unwrap().push(Action::SetBgColor(Color::new(r as u8, g as u8, b as u8, a as u8)));
        });

        let q_print = action_queue.clone();
        engine.register_fn("write_text", move |text: rhai::ImmutableString| {
            q_print.lock().unwrap().push(Action::PrintString(text.to_string()));
        });

        let q_variant = action_queue.clone();
        engine.register_fn("set_variant", move |variant: rhai::ImmutableString| {
            q_variant.lock().unwrap().push(Action::SetVariantByName(variant.to_string()));
        });

        let q_print_var = action_queue.clone();
        engine.register_fn("write_text", move |text: rhai::ImmutableString, variant: rhai::ImmutableString| {
            let mut q = q_print_var.lock().unwrap();
            q.push(Action::SetVariantByName(variant.to_string()));
            q.push(Action::PrintString(text.to_string()));
            q.push(Action::SetVariantByName("default".to_string()));
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
            states: HashMap::new(),
            action_queue,
            buffer_sizes,
            missing_update: HashSet::new(),
            missing_init: HashSet::new(),
            logged_errors: HashMap::new(),
            frame_count: 0,
        }
    }

    pub fn load_script(&mut self, name: &str, script_text: &str) -> Result<(), String> {
        match self.engine.compile(script_text) {
            Ok(ast) => {
                // Анализируем, какие функции определены в скрипте
                let functions: Vec<String> = ast.iter_functions()
                    .map(|f| format!("{}({})", f.name, f.params.join(", ")))
                    .collect();

                let has_init = ast.iter_functions().any(|f| f.name == "init");
                let has_update = ast.iter_functions().any(|f| f.name == "update");

                println!("[script:load] '{}' compiled OK", name);
                if !functions.is_empty() {
                    println!("[script:load]   functions: {}", functions.join(", "));
                }
                if !has_init {
                    println!("[script:load]   note: no init() — script won't receive init call");
                    self.missing_init.insert(name.to_string());
                }
                if !has_update {
                    println!("[script:load]   WARNING: no update() — script won't run per-frame");
                    self.missing_update.insert(name.to_string());
                }

                self.asts.insert(name.to_string(), ast);
                self.states.insert(name.to_string(), Dynamic::from(Map::new()));
                Ok(())
            }
            Err(e) => {
                let msg = format!("Script compile error [{}]: {}", name, e);
                eprintln!("[script:error] {}", msg);
                Err(msg)
            }
        }
    }

    pub fn run_init(&mut self) {
        let keys: Vec<String> = self.asts.keys().cloned().collect();

        for name in keys {
            let ast = self.asts.get(&name).unwrap().clone();

            // Выполняем top-level код скрипта
            if let Err(e) = self.engine.run_ast_with_scope(&mut self.scope, &ast) {
                eprintln!("[script:error] Top-level eval error [{}]: {}", name, format_rhai_error(&e));
            }

            // Пропускаем вызов init() если его нет в скрипте
            if self.missing_init.contains(&name) {
                continue;
            }

            if let Some(state) = self.states.get_mut(&name) {
                let options = rhai::CallFnOptions::new().bind_this_ptr(state);
                let result: Result<(), _> = self.engine.call_fn_with_options(
                    options,
                    &mut self.scope,
                    &ast,
                    "init",
                    ()
                );

                match result {
                    Ok(()) => println!("[script:init] '{}' init() OK", name),
                    Err(e) => {
                        if is_fn_not_found(&e, "init") {
                            // Не нашли init — это нормально, но мы уже предупредили при загрузке
                        } else {
                            eprintln!("[script:error] init() error [{}]: {}", name, format_rhai_error(&e));
                        }
                    }
                }
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

    pub fn run_update(&mut self, time: f32, frame_events: &[Event]) {
        self.frame_count += 1;
        self.scope.set_or_push("TIME", time as f64);
        self.scope.set_or_push("FRAME", self.frame_count as i64);

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
                _ => {}
            }
            if !map.is_empty() {
                rhai_events.push(Dynamic::from_map(map));
            }
        }

        let keys: Vec<String> = self.asts.keys().cloned().collect();
        for name in keys {
            // Пропускаем скрипты без update()
            if self.missing_update.contains(&name) {
                continue;
            }

            let ast = self.asts.get(&name).unwrap().clone();

            if let Some(state) = self.states.get_mut(&name) {
                let options = rhai::CallFnOptions::new().bind_this_ptr(state);
                let result: Result<(), _> = self.engine.call_fn_with_options(
                    options,
                    &mut self.scope,
                    &ast,
                    "update",
                    (rhai_events.clone(),)
                );

                if let Err(e) = result {
                    if is_fn_not_found(&e, "update") {
                        // Помечаем чтобы больше не вызывать
                        self.missing_update.insert(name.clone());
                        continue;
                    }

                    // Throttle: логируем одну и ту же ошибку не чаще раза в 180 кадров (~3 сек при 60fps)
                    let err_msg = format_rhai_error(&e);
                    let error_key = format!("{}:{}", name, err_msg);
                    let last_logged = self.logged_errors.get(&error_key).copied().unwrap_or(0);

                    if self.frame_count - last_logged >= 180 {
                        eprintln!("[script:error] update() error [{}]: {}", name, err_msg);
                        self.logged_errors.insert(error_key, self.frame_count);
                    }
                }
            }
        }
    }

    pub fn take_actions(&self) -> Vec<Action> {
        let mut q = self.action_queue.lock().unwrap();
        std::mem::take(&mut *q)
    }

    /// Возвращает список загруженных скриптов и их состояние (для внешней диагностики)
    pub fn script_info(&self) -> Vec<(String, bool, bool)> {
        self.asts.keys().map(|name| {
            let has_init = !self.missing_init.contains(name);
            let has_update = !self.missing_update.contains(name);
            (name.clone(), has_init, has_update)
        }).collect()
    }
}
