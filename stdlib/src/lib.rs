use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use snsx_runtime::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Color(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Canvas {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Color>,
}

impl Canvas {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            pixels: vec![Color(255, 255, 255); width * height],
        }
    }

    pub fn put(&mut self, x: usize, y: usize, color: Color) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = color;
        }
    }

    pub fn line(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, color: Color) {
        let mut x = x0 as isize;
        let mut y = y0 as isize;
        let dx = (x1 as isize - x0 as isize).abs();
        let dy = -((y1 as isize - y0 as isize).abs());
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            self.put(x as usize, y as usize, color);
            if x == x1 as isize && y == y1 as isize {
                break;
            }
            let twice = 2 * err;
            if twice >= dy {
                err += dy;
                x += sx;
            }
            if twice <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    pub fn write_ppm(&self, path: &std::path::Path) -> Result<()> {
        let mut bytes = format!("P3\n{} {}\n255\n", self.width, self.height);
        for pixel in &self.pixels {
            bytes.push_str(&format!("{} {} {}\n", pixel.0, pixel.1, pixel.2));
        }
        std::fs::write(path, bytes)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DomNode {
    Text(String),
    Element {
        tag: String,
        attrs: HashMap<String, String>,
        children: Vec<DomNode>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutBox {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
    pub node: DomNode,
}

pub fn simple_layout(root: &DomNode, width: usize) -> Vec<LayoutBox> {
    let mut boxes = Vec::new();
    walk_layout(root, 0, 0, width, &mut boxes);
    boxes
}

fn walk_layout(
    node: &DomNode,
    x: usize,
    y: usize,
    width: usize,
    out: &mut Vec<LayoutBox>,
) -> usize {
    let height = match node {
        DomNode::Text(text) => text.lines().count().max(1),
        DomNode::Element { children, .. } => {
            let mut cursor = y + 1;
            for child in children {
                cursor = walk_layout(child, x + 2, cursor, width.saturating_sub(2), out);
            }
            cursor.saturating_sub(y)
        }
    };
    out.push(LayoutBox {
        x,
        y,
        width,
        height,
        node: node.clone(),
    });
    y + height + 1
}

pub fn render_ascii(root: &DomNode) -> String {
    let layout = simple_layout(root, 80);
    let mut lines =
        vec![String::new(); layout.iter().map(|b| b.y + b.height + 1).max().unwrap_or(1)];
    for item in layout {
        while lines[item.y].len() < item.x {
            lines[item.y].push(' ');
        }
        let label = match &item.node {
            DomNode::Text(text) => text.clone(),
            DomNode::Element { tag, .. } => format!("<{}>", tag),
        };
        lines[item.y].push_str(&label);
    }
    lines.join("\n")
}

pub struct ModuleRegistry {
    modules: HashMap<&'static str, &'static str>,
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self {
            modules: HashMap::from([
                ("std.io", include_str!("../modules/io.snsx")),
                ("std.math", include_str!("../modules/math.snsx")),
                ("std.ai", include_str!("../modules/ai.snsx")),
                ("std.db", include_str!("../modules/db.snsx")),
                ("std.net", include_str!("../modules/net.snsx")),
                ("std.gfx", include_str!("../modules/gfx.snsx")),
                ("std.web", include_str!("../modules/web.snsx")),
                ("std.design", include_str!("../modules/design.snsx")),
                ("std.error", include_str!("../modules/error.snsx")),
                ("std.sentinel", include_str!("../modules/sentinel.snsx")),
            ]),
        }
    }

    pub fn names(&self) -> Vec<&'static str> {
        let mut names = self.modules.keys().copied().collect::<Vec<_>>();
        names.sort_unstable();
        names
    }

    pub fn source(&self, name: &str) -> Result<&'static str> {
        self.modules
            .get(name)
            .copied()
            .ok_or_else(|| anyhow!("unknown stdlib module '{name}'"))
    }
}

pub fn value_to_lines(value: &Value) -> Vec<String> {
    match value {
        Value::Array(items) => items.iter().map(ToString::to_string).collect(),
        other => vec![other.to_string()],
    }
}
