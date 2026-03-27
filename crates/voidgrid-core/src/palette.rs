use std::collections::HashMap;
use raylib::prelude::Color;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct PaletteColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
    pub name: Option<String>,
}

impl PaletteColor {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a, name: None }
    }

    pub fn named(name: impl Into<String>, r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a, name: Some(name.into()) }
    }

    pub fn to_color(&self) -> Color {
        Color::new(self.r, self.g, self.b, self.a)
    }

    pub fn to_hex(&self) -> String {
        if self.a == 255 {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        } else {
            format!("#{:02X}{:02X}{:02X}{:02X}", self.r, self.g, self.b, self.a)
        }
    }
}

fn parse_hex(hex: &str) -> Result<(u8, u8, u8, u8), String> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
            Ok((r, g, b, 255))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).map_err(|e| e.to_string())?;
            let g = u8::from_str_radix(&hex[2..4], 16).map_err(|e| e.to_string())?;
            let b = u8::from_str_radix(&hex[4..6], 16).map_err(|e| e.to_string())?;
            let a = u8::from_str_radix(&hex[6..8], 16).map_err(|e| e.to_string())?;
            Ok((r, g, b, a))
        }
        _ => Err(format!("invalid hex color length: {}", hex.len())),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ColorEntry {
    Short(String),
    Named { hex: String, name: Option<String> },
}

impl ColorEntry {
    fn into_palette_color(self) -> Result<PaletteColor, String> {
        match self {
            ColorEntry::Short(hex) => {
                let (r, g, b, a) = parse_hex(&hex)?;
                Ok(PaletteColor::new(r, g, b, a))
            }
            ColorEntry::Named { hex, name } => {
                let (r, g, b, a) = parse_hex(&hex)?;
                Ok(PaletteColor { r, g, b, a, name })
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct PaletteConfig {
    pub name: String,
    colors: Vec<ColorEntry>,
}

impl PaletteConfig {
    pub fn into_colors(self) -> Result<(String, Vec<PaletteColor>), String> {
        let colors = self.colors.into_iter()
            .map(|e| e.into_palette_color())
            .collect::<Result<Vec<_>, _>>()?;
        Ok((self.name, colors))
    }
}

#[derive(Debug, Clone)]
pub struct Palette {
    pub name: String,
    pub colors: Vec<PaletteColor>,
    name_index: HashMap<String, usize>,
}

impl Palette {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            colors: Vec::new(),
            name_index: HashMap::new(),
        }
    }
    
    pub fn from_config(config: PaletteConfig) -> Result<Self, String> {
        let (name, colors) = config.into_colors()?;
        let mut palette = Self::new(name);
        for color in colors {
            palette.add_color(color);
        }
        Ok(palette)
    }
    
    pub fn add_color(&mut self, color: PaletteColor) -> usize {
        let index = self.colors.len();
        if let Some(ref name) = color.name {
            self.name_index.insert(name.clone(), index);
        }
        self.colors.push(color);
        index
    }
    
    pub fn get(&self, index: usize) -> Option<Color> {
        self.colors.get(index).map(|c| c.to_color())
    }
    
    pub fn get_by_name(&self, name: &str) -> Option<Color> {
        self.name_index.get(name)
            .and_then(|&idx| self.colors.get(idx))
            .map(|c| c.to_color())
    }

    pub fn index_of_name(&self, name: &str) -> Option<usize> {
        self.name_index.get(name).copied()
    }

    pub fn find_by_rgba(&self, r: u8, g: u8, b: u8, a: u8) -> Option<usize> {
        self.colors.iter().position(|c| c.r == r && c.g == g && c.b == b && c.a == a)
    }

    pub fn set_color(&mut self, index: usize, color: PaletteColor) {
        if index < self.colors.len() {
            // Remove old name from index if it had one
            if let Some(ref old_name) = self.colors[index].name {
                self.name_index.remove(old_name);
            }
            // Add new name to index
            if let Some(ref name) = color.name {
                self.name_index.insert(name.clone(), index);
            }
            self.colors[index] = color;
        }
    }

    pub fn len(&self) -> usize {
        self.colors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty()
    }
    
    pub fn to_toml(&self) -> String {
        let mut out = format!("name = \"{}\"\n\ncolors = [\n", self.name);
        for color in &self.colors {
            let hex = color.to_hex();
            if let Some(ref name) = color.name {
                out.push_str(&format!("    {{ hex = \"{}\", name = \"{}\" }},\n", hex, name));
            } else {
                out.push_str(&format!("    \"{}\",\n", hex));
            }
        }
        out.push_str("]\n");
        out
    }
    
    pub fn cycle(&mut self, start: usize, end: usize) {
        if start >= end || end > self.colors.len() {
            return;
        }
        let last = self.colors[end - 1].clone();
        for i in (start + 1..end).rev() {
            self.colors[i] = self.colors[i - 1].clone();
        }
        self.colors[start] = last;
        self.rebuild_name_index();
    }
    
    fn rebuild_name_index(&mut self) {
        self.name_index.clear();
        for (i, color) in self.colors.iter().enumerate() {
            if let Some(ref name) = color.name {
                self.name_index.insert(name.clone(), i);
            }
        }
    }
}