#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone)]
pub enum Event {
    MousePress { x: f32, y: f32, button: MouseButton },
    MouseRelease { x: f32, y: f32, button: MouseButton },
    MouseMove { x: f32, y: f32 },
    KeyPress { key: u32 },
    FileDrop { path: String },
    WindowResize { width: i32, height: i32 },
}

pub struct EventQueue {
    pub frame_events: Vec<Event>,
}

impl EventQueue {
    pub fn new() -> Self {
        Self {
            frame_events: Vec::new(),
        }
    }

    pub fn push(&mut self, event: Event) {
        self.frame_events.push(event);
    }

    pub fn clear(&mut self) {
        self.frame_events.clear();
    }
}

