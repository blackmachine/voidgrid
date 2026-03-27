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
pub mod terminal;
pub mod events;
pub mod scripting;
pub mod ui;
pub mod asset_manager;
pub mod resource_pack;
pub mod pack_loader;
pub mod virtual_tree;
pub mod vfx;
pub use voidgrid_vtp as vtp;

use raylib::prelude::*;
use grids::Grids;
use renderer::Renderer;
use hierarchy::RenderItem;

/// Р¤Р°СЃР°Рґ РґР»СЏ РґРІРёР¶РєР° VoidGrid.
/// РћР±СЉРµРґРёРЅСЏРµС‚ СѓРїСЂР°РІР»РµРЅРёРµ РґР°РЅРЅС‹РјРё (Grids) Рё РѕС‚СЂРёСЃРѕРІРєСѓ (Renderer).
pub struct VoidGrid {
    pub grids: Grids,
    pub renderer: Renderer,
    pub terminal: terminal::TerminalState,
    pub events: events::EventQueue,
}

impl VoidGrid {
    /// РЎРѕР·РґР°С‚СЊ РЅРѕРІС‹Р№ СЌРєР·РµРјРїР»СЏСЂ РґРІРёР¶РєР°
    pub fn new() -> Self {
        Self {
            grids: Grids::new(),
            renderer: Renderer::new(),
            terminal: terminal::TerminalState::new(),
            events: events::EventQueue::new(),
        }
    }
    
    /// РРЅРёС†РёР°Р»РёР·Р°С†РёСЏ СЃРёСЃС‚РµРјРЅС‹С… СЂРµСЃСѓСЂСЃРѕРІ (РЅР°РїСЂРёРјРµСЂ, РјР°СЃРєРё)
    /// Р РµРєРѕРјРµРЅРґСѓРµС‚СЃСЏ РІС‹Р·С‹РІР°С‚СЊ СЃСЂР°Р·Сѓ РїРѕСЃР»Рµ СЃРѕР·РґР°РЅРёСЏ
    pub fn init(
        &mut self,
        provider: &mut dyn crate::resource_pack::ResourceProvider,
        rl: &mut RaylibHandle,
        thread: &RaylibThread
    ) {
        // Р—Р°РіСЂСѓР¶Р°РµРј РІСЃС‚СЂРѕРµРЅРЅС‹Р№ С€РµР№РґРµСЂ РјР°СЃРєРё (РїРѕРєР° РёР· С„Р°Р№Р»Р°)
        if let Err(e) = self.renderer.load_mask_shader(provider, rl, thread, "assets/mask.fs") {
            eprintln!("VoidGrid Warning: Failed to load mask shader: {}", e);
        }
    }
    
    /// РћС‚СЂРёСЃРѕРІРєР° РІСЃРµРіРѕ РґРµСЂРµРІР° Р±СѓС„РµСЂРѕРІ
    pub fn draw(&mut self, d: &mut RaylibDrawHandle, render_list: &[RenderItem]) {
        self.renderer.draw(d, &mut self.grids, render_list);
    }
    
    /// РџСЂРµРґРІР°СЂРёС‚РµР»СЊРЅС‹Р№ СЂРµРЅРґРµСЂ (РґР»СЏ С€РµР№РґРµСЂРѕРІ)
    pub fn render_offscreen(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, render_list: &[RenderItem]) {
        self.renderer.render_offscreen(rl, thread, &mut self.grids, render_list);
    }

    /// VFX pass: render scene to texture, apply bloom, etc.
    /// Call after render_offscreen(), before draw().
    pub fn render_vfx(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        render_list: &[RenderItem],
        screen_w: u32,
        screen_h: u32,
        clear_color: Color,
    ) {
        self.renderer.render_vfx(rl, thread, &mut self.grids, render_list, screen_w, screen_h, clear_color);
    }
}

impl Default for VoidGrid {
    fn default() -> Self {
        Self::new()
    }
}



