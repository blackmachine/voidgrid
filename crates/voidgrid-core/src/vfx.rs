//! VFX pipeline — standalone post-processing effects (bloom, etc.)
//!
//! Self-contained module that operates on finished render textures.
//! Can be removed without affecting the core renderer.

use raylib::prelude::*;
use crate::resource_pack::ResourceProvider;

const BLOOM_MAX_MIPS: usize = 6;

// ============================================================================
// BloomSettings
// ============================================================================

/// Tunable parameters for the bloom effect
pub struct BloomSettings {
    // Pseudo-linearization
    pub gamma: f32,
    pub bright_boost: f32,
    pub threshold: f32,
    pub knee: f32,
    pub sat_start: f32,
    pub sat_end: f32,
    pub desat_strength: f32,
    // Upsample
    pub sample_scale: f32,
    // Composite
    pub intensity: f32,
    pub bloom_gamma: f32,       // power curve on bloom before composite
    pub bloom_saturation: f32,  // saturation of bloom layer
}

impl Default for BloomSettings {
    fn default() -> Self {
        Self {
            gamma: 2.0,
            bright_boost: 2.0,
            threshold: 0.5,
            knee: 0.2,
            sat_start: 0.6,
            sat_end: 1.0,
            desat_strength: 0.5,
            sample_scale: 1.0,
            intensity: 1.0,
            bloom_gamma: 1.0,
            bloom_saturation: 1.0,
        }
    }
}

// ============================================================================
// Cached uniform locations
// ============================================================================

struct UniformLocs {
    texel_size: i32,
    mode: i32,
    texture1: i32,
    gamma: i32,
    bright_boost: i32,
    threshold: i32,
    knee: i32,
    sat_start: i32,
    sat_end: i32,
    desat_strength: i32,
    sample_scale: i32,
    intensity: i32,
    bloom_gamma: i32,
    bloom_saturation: i32,
}

impl UniformLocs {
    fn resolve(shader: &Shader) -> Self {
        Self {
            texel_size: shader.get_shader_location("texelSize"),
            mode: shader.get_shader_location("uMode"),
            texture1: shader.get_shader_location("texture1"),
            gamma: shader.get_shader_location("uGamma"),
            bright_boost: shader.get_shader_location("uBrightBoost"),
            threshold: shader.get_shader_location("uThreshold"),
            knee: shader.get_shader_location("uKnee"),
            sat_start: shader.get_shader_location("uSatStart"),
            sat_end: shader.get_shader_location("uSatEnd"),
            desat_strength: shader.get_shader_location("uDesatStrength"),
            sample_scale: shader.get_shader_location("uSampleScale"),
            intensity: shader.get_shader_location("uIntensity"),
            bloom_gamma: shader.get_shader_location("uBloomGamma"),
            bloom_saturation: shader.get_shader_location("uBloomSaturation"),
        }
    }
}

// ============================================================================
// VfxPipeline
// ============================================================================

pub struct VfxPipeline {
    shader: Shader,
    locs: UniformLocs,
    mip_chain: Vec<RenderTexture2D>,
    output: RenderTexture2D,
    source_size: (u32, u32),
    pub settings: BloomSettings,
    pub enabled: bool,
}

impl VfxPipeline {
    pub fn new(
        provider: &mut dyn ResourceProvider,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let code = provider.read_string("assets/vfx_bloom.fs")
            .map_err(|e| format!("Failed to read vfx_bloom.fs: {}", e))?;
        let shader = rl.load_shader_from_memory(thread, None, Some(&code));
        let locs = UniformLocs::resolve(&shader);

        let mip_chain = create_mip_chain(rl, thread, width, height)?;
        let output = rl.load_render_texture(thread, width, height)
            .map_err(|e| format!("Failed to create VFX output: {}", e))?;
        set_bilinear(&output);

        Ok(Self {
            shader,
            locs,
            mip_chain,
            output,
            source_size: (width, height),
            settings: BloomSettings::default(),
            enabled: true,
        })
    }

    /// Resize all internal textures if source dimensions changed.
    pub fn resize(&mut self, rl: &mut RaylibHandle, thread: &RaylibThread, width: u32, height: u32) {
        if self.source_size == (width, height) {
            return;
        }
        self.mip_chain.clear();
        if let Ok(chain) = create_mip_chain(rl, thread, width, height) {
            self.mip_chain = chain;
        }
        if let Ok(rt) = rl.load_render_texture(thread, width, height) {
            set_bilinear(&rt);
            self.output = rt;
        }
        self.source_size = (width, height);
    }

    /// Apply bloom to `source`. Result available via `output_texture()`.
    pub fn apply(
        &mut self,
        rl: &mut RaylibHandle,
        thread: &RaylibThread,
        source: &RenderTexture2D,
    ) {
        if !self.enabled || self.mip_chain.len() < 2 {
            return;
        }

        let src_w = source.texture().width as f32;
        let src_h = source.texture().height as f32;

        self.resize(rl, thread, src_w as u32, src_h as u32);
        self.upload_settings();

        let num_mips = self.mip_chain.len();

        // ── Pass 0: Prefilter (source → mip[0]) ─────────────────────────
        {
            set_int(&self.shader, self.locs.mode, 0);
            set_vec2(&self.shader, self.locs.texel_size, 1.0 / src_w, 1.0 / src_h);

            let dst_w = self.mip_chain[0].texture().width as f32;
            let dst_h = self.mip_chain[0].texture().height as f32;

            let mut td = rl.begin_texture_mode(thread, &mut self.mip_chain[0]);
            td.clear_background(Color::BLANK);
            {
                let _nb = NoBlendGuard::new();
                let mut sm = td.begin_shader_mode(&mut self.shader);
                sm.draw_texture_pro(
                    source.texture(),
                    Rectangle::new(0.0, src_h, src_w, -src_h),
                    Rectangle::new(0.0, 0.0, dst_w, dst_h),
                    Vector2::new(0.0, 0.0),
                    0.0,
                    Color::WHITE,
                );
            }
        }

        // ── Downsample chain: mip[i] → mip[i+1] ────────────────────────
        for i in 0..num_mips - 1 {
            let mw = self.mip_chain[i].texture().width as f32;
            let mh = self.mip_chain[i].texture().height as f32;

            set_int(&self.shader, self.locs.mode, 1);
            set_vec2(&self.shader, self.locs.texel_size, 1.0 / mw, 1.0 / mh);

            let (left, right) = self.mip_chain.split_at_mut(i + 1);
            let src_tex = &left[i];
            let dst = &mut right[0];
            let dst_w = dst.texture().width as f32;
            let dst_h = dst.texture().height as f32;

            let mut td = rl.begin_texture_mode(thread, dst);
            td.clear_background(Color::BLANK);
            {
                let _nb = NoBlendGuard::new();
                let mut sm = td.begin_shader_mode(&mut self.shader);
                sm.draw_texture_pro(
                    src_tex.texture(),
                    Rectangle::new(0.0, mh, mw, -mh),
                    Rectangle::new(0.0, 0.0, dst_w, dst_h),
                    Vector2::new(0.0, 0.0),
                    0.0,
                    Color::WHITE,
                );
            }
        }

        // ── Upsample chain: mip[i+1] → ADD to mip[i] ───────────────────
        for i in (0..num_mips - 1).rev() {
            let (left, right) = self.mip_chain.split_at_mut(i + 1);
            let dst = &mut left[i];
            let src_tex = &right[0];

            let mw = src_tex.texture().width as f32;
            let mh = src_tex.texture().height as f32;
            let dst_w = dst.texture().width as f32;
            let dst_h = dst.texture().height as f32;

            set_int(&self.shader, self.locs.mode, 2);
            set_vec2(&self.shader, self.locs.texel_size, 1.0 / mw, 1.0 / mh);

            let mut td = rl.begin_texture_mode(thread, dst);
            // No clear — additive blend accumulates onto downsample content
            {
                let _blend = AdditiveBlendGuard::new();
                let mut sm = td.begin_shader_mode(&mut self.shader);
                sm.draw_texture_pro(
                    src_tex.texture(),
                    Rectangle::new(0.0, mh, mw, -mh),
                    Rectangle::new(0.0, 0.0, dst_w, dst_h),
                    Vector2::new(0.0, 0.0),
                    0.0,
                    Color::WHITE,
                );
            }
        }

        // ── Composite: two-pass (avoids texture1 binding issues) ─────────
        {
            let out_w = self.output.texture().width as f32;
            let out_h = self.output.texture().height as f32;
            let mip0_w = self.mip_chain[0].texture().width as f32;
            let mip0_h = self.mip_chain[0].texture().height as f32;

            let mut td = rl.begin_texture_mode(thread, &mut self.output);
            td.clear_background(Color::BLANK);

            // Pass 1: blit original scene (no shader, no blend)
            {
                let _nb = NoBlendGuard::new();
                td.draw_texture_pro(
                    source.texture(),
                    Rectangle::new(0.0, src_h, src_w, -src_h),
                    Rectangle::new(0.0, 0.0, out_w, out_h),
                    Vector2::new(0.0, 0.0),
                    0.0,
                    Color::WHITE,
                );
            }

            // Pass 2: bloom layer on top (shader mode 3 + additive blend)
            {
                set_int(&self.shader, self.locs.mode, 3);
                set_vec2(&self.shader, self.locs.texel_size, 1.0 / mip0_w, 1.0 / mip0_h);

                let _blend = AdditiveBlendGuard::new();
                let mut sm = td.begin_shader_mode(&mut self.shader);
                sm.draw_texture_pro(
                    self.mip_chain[0].texture(),
                    Rectangle::new(0.0, mip0_h, mip0_w, -mip0_h),
                    Rectangle::new(0.0, 0.0, out_w, out_h),
                    Vector2::new(0.0, 0.0),
                    0.0,
                    Color::WHITE,
                );
            }
        }
    }

    /// The output texture containing the composited result.
    pub fn output_texture(&self) -> &RenderTexture2D {
        &self.output
    }

    fn upload_settings(&self) {
        let s = &self.settings;
        set_float(&self.shader, self.locs.gamma, s.gamma);
        set_float(&self.shader, self.locs.bright_boost, s.bright_boost);
        set_float(&self.shader, self.locs.threshold, s.threshold);
        set_float(&self.shader, self.locs.knee, s.knee);
        set_float(&self.shader, self.locs.sat_start, s.sat_start);
        set_float(&self.shader, self.locs.sat_end, s.sat_end);
        set_float(&self.shader, self.locs.desat_strength, s.desat_strength);
        set_float(&self.shader, self.locs.sample_scale, s.sample_scale);
        set_float(&self.shader, self.locs.intensity, s.intensity);
        set_float(&self.shader, self.locs.bloom_gamma, s.bloom_gamma);
        set_float(&self.shader, self.locs.bloom_saturation, s.bloom_saturation);
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn create_mip_chain(
    rl: &mut RaylibHandle,
    thread: &RaylibThread,
    width: u32,
    height: u32,
) -> Result<Vec<RenderTexture2D>, String> {
    let mut chain = Vec::new();
    let mut w = width / 2;
    let mut h = height / 2;

    for _ in 0..BLOOM_MAX_MIPS {
        if w < 2 || h < 2 {
            break;
        }
        let rt = rl.load_render_texture(thread, w, h)
            .map_err(|e| format!("Failed to create bloom mip {}x{}: {}", w, h, e))?;
        set_bilinear(&rt);
        chain.push(rt);
        w /= 2;
        h /= 2;
    }

    if chain.len() < 2 {
        return Err("Source too small for bloom".into());
    }
    Ok(chain)
}

fn set_bilinear(rt: &RenderTexture2D) {
    unsafe {
        raylib::ffi::SetTextureFilter(
            *rt.texture().as_ref(),
            raylib::ffi::TextureFilter::TEXTURE_FILTER_BILINEAR as i32,
        );
    }
}

fn set_float(shader: &Shader, loc: i32, val: f32) {
    if loc >= 0 {
        unsafe {
            raylib::ffi::SetShaderValue(
                *shader.as_ref(), loc,
                &val as *const f32 as *const _,
                raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32,
            );
        }
    }
}

fn set_int(shader: &Shader, loc: i32, val: i32) {
    if loc >= 0 {
        unsafe {
            raylib::ffi::SetShaderValue(
                *shader.as_ref(), loc,
                &val as *const i32 as *const _,
                raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_INT as i32,
            );
        }
    }
}

fn set_vec2(shader: &Shader, loc: i32, x: f32, y: f32) {
    if loc >= 0 {
        unsafe {
            raylib::ffi::SetShaderValue(
                *shader.as_ref(), loc,
                [x, y].as_ptr() as *const _,
                raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32,
            );
        }
    }
}

/// RAII guard: disables color blending, re-enables on drop
struct NoBlendGuard;

impl NoBlendGuard {
    fn new() -> Self {
        unsafe { raylib::ffi::rlDisableColorBlend(); }
        Self
    }
}

impl Drop for NoBlendGuard {
    fn drop(&mut self) {
        unsafe { raylib::ffi::rlEnableColorBlend(); }
    }
}

/// RAII guard: enables additive blend mode, restores on drop
struct AdditiveBlendGuard;

impl AdditiveBlendGuard {
    fn new() -> Self {
        unsafe { raylib::ffi::BeginBlendMode(raylib::ffi::BlendMode::BLEND_ADDITIVE as i32); }
        Self
    }
}

impl Drop for AdditiveBlendGuard {
    fn drop(&mut self) {
        unsafe { raylib::ffi::EndBlendMode(); }
    }
}
