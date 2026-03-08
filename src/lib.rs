pub mod types;
pub mod buffer_manager;
pub mod atlas;
pub mod palette;
pub mod shader;
pub mod buffer;
pub mod assets;
pub mod grids;
pub mod renderer;
pub mod text_ops;
pub mod input;
pub mod global_registry;
pub mod glyphset;
pub mod hierarchy;
pub mod ui;
pub mod asset_manager;
pub mod resource_pack;

use raylib::prelude::*;
use grids::Grids;
use renderer::Renderer;
use hierarchy::RenderItem;

/// Фасад для движка VoidGrid.
/// Объединяет управление данными (Grids) и отрисовку (Renderer).
pub struct VoidGrid {
    pub grids: Grids,
    pub renderer: Renderer,
}

impl VoidGrid {
    /// Создать новый экземпляр движка
    pub fn new() -> Self {
        Self {
            grids: Grids::new(),
            renderer: Renderer::new(),
        }
    }
    
    /// Инициализация системных ресурсов (например, маски)
    /// Рекомендуется вызывать сразу после создания
    pub fn init(
        &mut self,
        provider: &mut dyn crate::resource_pack::ResourceProvider,
        rl: &mut RaylibHandle,
        thread: &RaylibThread
    ) {
        // Загружаем встроенный шейдер маски (пока из файла)
        if let Err(e) = self.renderer.load_mask_shader(provider, rl, thread, "assets/mask.fs") {
            eprintln!("VoidGrid Warning: Failed to load mask shader: {}", e);
        }
    }
    
    /// Отрисовка всего дерева буферов
    pub fn draw(&mut self, d: &mut RaylibDrawHandle, render_list: &[RenderItem]) {
        self.renderer.draw(d, &mut self.grids, render_list);
    }
    
    /// Предварительный рендер (для шейдеров)
    pub fn render_offscreen(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, render_list: &[RenderItem]) {
        self.renderer.render_offscreen(rl, thread, &mut self.grids, render_list);
    }
}

impl Default for VoidGrid {
    fn default() -> Self {
        Self::new()
    }
}