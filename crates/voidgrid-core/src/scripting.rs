use rhai::{Engine, Scope, AST, Map, Array, Dynamic};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use crate::terminal::Action;
use crate::events::{Event, MouseButton};
use crate::hierarchy::RenderItem;
use raylib::prelude::Color;

/// Снимок данных буфера для чтения из скриптов
struct BufferSnapshot {
    w: u32,
    h: u32,
    /// (char_code, fg [r,g,b,a], bg [r,g,b,a])
    cells: Vec<(u32, [u8; 4], [u8; 4])>,
}

/// Экранная позиция буфера и размер тайла (для mouse_to_cell)
type ScreenInfo = (i32, i32, i32, i32); // (screen_x, screen_y, tile_w, tile_h)

pub struct ScriptEngine {
    engine: Engine,
    scope: Scope<'static>,
    asts: HashMap<String, AST>,
    states: HashMap<String, Dynamic>,
    action_queue: Arc<Mutex<Vec<Action>>>,
    buffer_sizes: Arc<Mutex<HashMap<String, (u32, u32)>>>,
    buffer_data: Arc<Mutex<HashMap<String, BufferSnapshot>>>,
    buffer_screen: Arc<Mutex<HashMap<String, ScreenInfo>>>,
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

        // --- Character output ---

        let q_char = action_queue.clone();
        engine.register_fn("print_char", move |code: i64| {
            q_char.lock().unwrap().push(Action::PrintChar(code as u32));
        });

        // --- Buffer management ---

        let q_clear = action_queue.clone();
        engine.register_fn("clear_buffer", move |name: rhai::ImmutableString| {
            q_clear.lock().unwrap().push(Action::ClearBuffer(name.to_string()));
        });

        let q_vis = action_queue.clone();
        engine.register_fn("set_buffer_visible", move |name: rhai::ImmutableString, visible: bool| {
            q_vis.lock().unwrap().push(Action::SetBufferVisible(name.to_string(), visible));
        });

        let q_opa = action_queue.clone();
        engine.register_fn("set_buffer_opacity", move |name: rhai::ImmutableString, opacity: f64| {
            q_opa.lock().unwrap().push(Action::SetBufferOpacity(name.to_string(), opacity as f32));
        });

        let q_z = action_queue.clone();
        engine.register_fn("set_buffer_z", move |name: rhai::ImmutableString, z: i64| {
            q_z.lock().unwrap().push(Action::SetBufferZ(name.to_string(), z as i32));
        });

        // --- Query functions ---

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

        // --- Cell reading ---

        let buffer_data: Arc<Mutex<HashMap<String, BufferSnapshot>>> = Arc::new(Mutex::new(HashMap::new()));

        let bd = buffer_data.clone();
        engine.register_fn("get_cell", move |buf_name: rhai::ImmutableString, x: i64, y: i64| -> Dynamic {
            let data = bd.lock().unwrap();
            let Some(snap) = data.get(buf_name.as_str()) else {
                return Dynamic::UNIT;
            };
            if x < 0 || y < 0 || x as u32 >= snap.w || y as u32 >= snap.h {
                return Dynamic::UNIT;
            }
            let idx = (y as u32 * snap.w + x as u32) as usize;
            let (code, fg, bg) = &snap.cells[idx];
            let mut map = Map::new();
            map.insert("code".into(), Dynamic::from(*code as i64));
            map.insert("char".into(), Dynamic::from(char::from_u32(*code).unwrap_or(' ').to_string()));
            map.insert("fg_r".into(), Dynamic::from(fg[0] as i64));
            map.insert("fg_g".into(), Dynamic::from(fg[1] as i64));
            map.insert("fg_b".into(), Dynamic::from(fg[2] as i64));
            map.insert("fg_a".into(), Dynamic::from(fg[3] as i64));
            map.insert("bg_r".into(), Dynamic::from(bg[0] as i64));
            map.insert("bg_g".into(), Dynamic::from(bg[1] as i64));
            map.insert("bg_b".into(), Dynamic::from(bg[2] as i64));
            map.insert("bg_a".into(), Dynamic::from(bg[3] as i64));
            Dynamic::from_map(map)
        });

        // --- Hit testing ---

        let buffer_screen: Arc<Mutex<HashMap<String, ScreenInfo>>> = Arc::new(Mutex::new(HashMap::new()));

        let bs = buffer_screen.clone();
        let bs_sizes = buffer_sizes.clone();
        engine.register_fn("mouse_to_cell", move |buf_name: rhai::ImmutableString, mx: f64, my: f64| -> Dynamic {
            let screen = bs.lock().unwrap();
            let Some(&(sx, sy, tw, th)) = screen.get(buf_name.as_str()) else {
                return Dynamic::UNIT;
            };
            let local_x = mx as i32 - sx;
            let local_y = my as i32 - sy;
            if local_x < 0 || local_y < 0 || tw == 0 || th == 0 {
                return Dynamic::UNIT;
            }
            let cx = local_x / tw;
            let cy = local_y / th;
            // Проверяем границы буфера
            let sizes = bs_sizes.lock().unwrap();
            if let Some(&(w, h)) = sizes.get(buf_name.as_str()) {
                if cx as u32 >= w || cy as u32 >= h {
                    return Dynamic::UNIT;
                }
            }
            let mut map = Map::new();
            map.insert("x".into(), Dynamic::from(cx as i64));
            map.insert("y".into(), Dynamic::from(cy as i64));
            Dynamic::from_map(map)
        });

        Self {
            engine,
            scope: Scope::new(),
            asts: HashMap::new(),
            states: HashMap::new(),
            action_queue,
            buffer_sizes,
            buffer_data,
            buffer_screen,
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

        let mut data = self.buffer_data.lock().unwrap();
        data.clear();

        for (name, &key) in buffer_map {
            if let Some(buf) = grids.get(key) {
                sizes.insert(name.clone(), (buf.w, buf.h));

                let mut cells = Vec::with_capacity((buf.w * buf.h) as usize);
                for y in 0..buf.h {
                    for x in 0..buf.w {
                        if let Some(ch) = buf.get(x, y) {
                            cells.push((
                                ch.code,
                                [ch.fcolor.r, ch.fcolor.g, ch.fcolor.b, ch.fcolor.a],
                                [ch.bcolor.r, ch.bcolor.g, ch.bcolor.b, ch.bcolor.a],
                            ));
                        } else {
                            cells.push((32, [255, 255, 255, 255], [0, 0, 0, 0]));
                        }
                    }
                }
                data.insert(name.clone(), BufferSnapshot { w: buf.w, h: buf.h, cells });
            }
        }
    }

    /// Синхронизирует экранные позиции буферов из render list предыдущего кадра.
    /// Вызывать перед run_update(), чтобы mouse_to_cell() работал корректно.
    pub fn sync_screen_positions(
        &self,
        render_list: &[RenderItem],
        grids: &crate::grids::Grids,
        buffer_map: &HashMap<String, crate::types::BufferKey>,
    ) {
        // Обратный маппинг: BufferKey → имя
        let key_to_name: HashMap<crate::types::BufferKey, &str> = buffer_map.iter()
            .map(|(name, &key)| (key, name.as_str()))
            .collect();

        let mut screen = self.buffer_screen.lock().unwrap();
        screen.clear();

        for item in render_list {
            if let Some(&name) = key_to_name.get(&item.buffer) {
                if let Some(buf) = grids.get(item.buffer) {
                    if let Some((tw, th)) = grids.assets.glyphset_size(buf.glyphset()) {
                        screen.insert(name.to_string(), (item.screen_x, item.screen_y, tw as i32, th as i32));
                    }
                }
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
                Event::MouseMove { x, y } => {
                    map.insert("type".into(), Dynamic::from("MouseMove"));
                    map.insert("x".into(), Dynamic::from(*x as f64));
                    map.insert("y".into(), Dynamic::from(*y as f64));
                }
                Event::FileDrop { path } => {
                    map.insert("type".into(), Dynamic::from("FileDrop"));
                    map.insert("path".into(), Dynamic::from(path.clone()));
                }
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
