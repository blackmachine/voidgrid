use std::collections::HashMap;
use raylib::prelude::Color;
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaletteConfig {
    pub name: String,
    pub colors: Vec<PaletteColor>,
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
    
    pub fn from_config(config: PaletteConfig) -> Self {
        let mut palette = Self::new(config.name);
        for color in config.colors {
            palette.add_color(color);
        }
        palette
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
    
    pub fn to_config(&self) -> PaletteConfig {
        PaletteConfig {
            name: self.name.clone(),
            colors: self.colors.clone(),
        }
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