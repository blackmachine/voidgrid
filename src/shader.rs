use std::collections::HashMap;
use raylib::prelude::*;

/// Значение uniform-переменной шейдера
#[derive(Debug, Clone)]
pub enum UniformValue {
    Float(f32),
    Vec2(f32, f32),
    Vec3(f32, f32, f32),
    Vec4(f32, f32, f32, f32),
    Int(i32),
}

/// Обёртка над шейдером с кэшированными uniform locations
pub struct ShaderData {
    pub shader: Shader,
    pub name: String,
    uniforms: HashMap<String, UniformValue>,
    locations: HashMap<String, i32>,
    // Auto-uniform locations
    loc_tex_size: i32,
    loc_time: i32,
    loc_resolution: i32,
}

impl std::fmt::Debug for ShaderData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShaderData")
            .field("name", &self.name)
            .field("uniforms", &self.uniforms)
            .finish()
    }
}

impl ShaderData {
    pub fn new(shader: Shader, name: impl Into<String>) -> Self {
        let name = name.into();
        let loc_tex_size = shader.get_shader_location("texSize");
        let loc_time = shader.get_shader_location("time");
        let loc_resolution = shader.get_shader_location("resolution");
        
        Self {
            shader,
            name,
            uniforms: HashMap::new(),
            locations: HashMap::new(),
            loc_tex_size,
            loc_time,
            loc_resolution,
        }
    }
    
    pub fn get_location(&mut self, name: &str) -> i32 {
        if let Some(&loc) = self.locations.get(name) {
            loc
        } else {
            let loc = self.shader.get_shader_location(name);
            self.locations.insert(name.to_string(), loc);
            loc
        }
    }
    
    pub fn set_uniform(&mut self, name: &str, value: UniformValue) {
        self.uniforms.insert(name.to_string(), value);
    }
    
    pub fn apply_uniforms(&mut self) {
        let uniforms: Vec<_> = self.uniforms.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        
        for (name, value) in uniforms {
            let loc = self.get_location(&name);
            if loc >= 0 {
                unsafe {
                    match value {
                        UniformValue::Float(v) => raylib::ffi::SetShaderValue(*self.shader.as_ref(), loc, &v as *const f32 as *const _, raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32),
                        UniformValue::Vec2(x, y) => raylib::ffi::SetShaderValue(*self.shader.as_ref(), loc, [x, y].as_ptr() as *const _, raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32),
                        UniformValue::Vec3(x, y, z) => raylib::ffi::SetShaderValue(*self.shader.as_ref(), loc, [x, y, z].as_ptr() as *const _, raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC3 as i32),
                        UniformValue::Vec4(x, y, z, w) => raylib::ffi::SetShaderValue(*self.shader.as_ref(), loc, [x, y, z, w].as_ptr() as *const _, raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC4 as i32),
                        UniformValue::Int(v) => raylib::ffi::SetShaderValue(*self.shader.as_ref(), loc, &v as *const i32 as *const _, raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_INT as i32),
                    }
                }
            }
        }
    }
    
    pub fn apply_auto_uniforms(&self, tex_size: (f32, f32), time: f32, resolution: (f32, f32)) {
        unsafe {
            if self.loc_tex_size >= 0 {
                raylib::ffi::SetShaderValue(*self.shader.as_ref(), self.loc_tex_size, [tex_size.0, tex_size.1].as_ptr() as *const _, raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32);
            }
            if self.loc_time >= 0 {
                raylib::ffi::SetShaderValue(*self.shader.as_ref(), self.loc_time, &time as *const f32 as *const _, raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_FLOAT as i32);
            }
            if self.loc_resolution >= 0 {
                raylib::ffi::SetShaderValue(*self.shader.as_ref(), self.loc_resolution, [resolution.0, resolution.1].as_ptr() as *const _, raylib::ffi::ShaderUniformDataType::SHADER_UNIFORM_VEC2 as i32);
            }
        }
    }
}