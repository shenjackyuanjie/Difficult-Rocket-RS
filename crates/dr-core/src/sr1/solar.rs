//! SR1 PC 星系资源读取及已证实的数据归一，不包含轨道推进或地形生成。

use super::scalars::{read_bool, read_f64, read_i32};
use super::xml::{self, Element};
use super::{Error, Result};

/// PC 资源加载器在计算星体质量时使用的常量（VA 0x535260）。
pub const GRAVITATIONAL_CONSTANT: f64 = 6.67384e-11;

#[derive(Clone, Debug, PartialEq)]
pub struct SolarSystem {
    pub name: String,
    pub description: String,
    /// PC 只使用 SolarSystem 下的首个 Planet 作为根。
    pub root: Planet,
}

impl SolarSystem {
    pub fn from_xml(source: &str) -> Result<Self> {
        Self::from_element(&xml::parse(source)?)
    }

    pub fn from_element(element: &Element) -> Result<Self> {
        element.require_name("SolarSystem")?;
        let root = element
            .child("Planet")
            .ok_or_else(|| Error::InvalidData("SolarSystem 缺少根 Planet".into()))?;
        Ok(Self {
            name: element.attr("name").unwrap_or_default().into(),
            description: element.attr("description").unwrap_or_default().into(),
            root: Planet::from_element(root)?,
        })
    }

    /// 资源树前序遍历；名称查询沿用原始顺序，重复名称不会被字典覆盖。
    pub fn planets(&self) -> Vec<&Planet> {
        let mut planets = Vec::new();
        self.root.collect(&mut planets);
        planets
    }

    pub fn planet(&self, name: &str) -> Option<&Planet> {
        self.root.find(name)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Planet {
    pub name: String,
    pub description: String,
    pub icon: String,
    pub map_color: Color,
    pub gravity: f64,
    pub radius: f64,
    pub launch_enabled: bool,
    pub orbit: Option<Orbit>,
    pub atmosphere: Option<Atmosphere>,
    pub terrain: Terrain,
    pub children: Vec<Planet>,
}

impl Planet {
    pub fn from_element(element: &Element) -> Result<Self> {
        element.require_name("Planet")?;
        let terrain = element.child("Terrain").ok_or_else(|| {
            Error::InvalidData(format!("Planet {:?} 缺少 Terrain", element.attr("name")))
        })?;
        let children = element
            .child("Children")
            .map(|children| {
                children
                    .children_named("Planet")
                    .map(Self::from_element)
                    .collect()
            })
            .unwrap_or_else(|| Ok(Vec::new()))?;
        Ok(Self {
            name: element.attr("name").unwrap_or_default().into(),
            description: element.attr("description").unwrap_or_default().into(),
            icon: element.attr("icon").unwrap_or_default().into(),
            map_color: Color::from_attribute(element.attr("mapColor")),
            gravity: read_f64(element.attr("gravity"), 0.0),
            radius: read_f64(element.attr("radius"), 0.0),
            launch_enabled: read_bool(element.attr("launchEnabled"), true),
            orbit: element.child("Orbit").map(Orbit::from_element),
            atmosphere: element.child("Atmosphere").map(Atmosphere::from_element),
            terrain: Terrain::from_element(terrain),
            children,
        })
    }

    /// PC 读取后执行 radius² × gravity / G；不在此增加正数检查或裁剪。
    pub fn mass(&self) -> f64 {
        self.radius * self.radius * self.gravity / GRAVITATIONAL_CONSTANT
    }

    fn collect<'a>(&'a self, result: &mut Vec<&'a Planet>) {
        result.push(self);
        for child in &self.children {
            child.collect(result);
        }
    }

    fn find(&self, name: &str) -> Option<&Planet> {
        if self.name == name {
            Some(self)
        } else {
            self.children.iter().find_map(|child| child.find(name))
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Orbit {
    pub w: f64,
    pub e: f64,
    pub prograde: bool,
    pub a: f64,
    /// 星系资源中的初始值，Runtime 的有效 trueAnomaly 可以覆盖此值。
    pub v: f64,
}

impl Orbit {
    fn from_element(element: &Element) -> Self {
        Self {
            w: read_f64(element.attr("w"), 0.0),
            e: read_f64(element.attr("e"), 0.0),
            prograde: read_bool(element.attr("prograde"), false),
            a: read_f64(element.attr("a"), 0.0),
            v: read_f64(element.attr("v"), 0.0),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Atmosphere {
    pub graphic_type: i32,
    pub height: f64,
    pub surface_pressure: f64,
}

impl Atmosphere {
    fn from_element(element: &Element) -> Self {
        Self {
            graphic_type: read_i32(element.attr("graphicType"), 0),
            height: read_f64(element.attr("height"), 0.0),
            surface_pressure: read_f64(element.attr("surfacePressure"), 0.0),
        }
    }
}

/// PC 颜色读入：跳过空逗号项，至少三项时取 RGB；恰好四项才取 alpha。
/// 分量经十进制整数前缀读取后截为 u8，不做 0..255 饱和裁剪。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Self = Self {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };

    pub fn from_attribute(source: Option<&str>) -> Self {
        let values: Vec<&str> = source
            .unwrap_or_default()
            .split(',')
            .filter(|s| !s.is_empty())
            .collect();
        if values.len() < 3 {
            return Self::BLACK;
        }
        Self {
            r: read_i32(Some(values[0]), 0) as u8,
            g: read_i32(Some(values[1]), 0) as u8,
            b: read_i32(Some(values[2]), 0) as u8,
            a: if values.len() == 4 {
                read_i32(Some(values[3]), 0) as u8
            } else {
                255
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Terrain {
    /// PC 先按 f64 解析，再转为 f32。
    pub noise: f32,
    pub color: Color,
    pub texture: String,
    pub water_density: f64,
    pub has_water: bool,
    pub max_height: f32,
    pub min_height: f32,
    /// maxHeight > 0 时为 0，否则为 maxHeight 的 f32 值。
    pub base_height: f32,
    /// minHeight > 0 时为 0，否则保留读取的 f64 值。
    pub water_level: f64,
    pub ranges: Vec<TerrainRange>,
    pub layers: Vec<TerrainLayer>,
}

impl Terrain {
    fn from_element(element: &Element) -> Self {
        let max_height = read_f64(element.attr("maxHeight"), 0.0);
        let min_height = read_f64(element.attr("minHeight"), 0.0);
        let water_density = read_f64(element.attr("waterDensity"), 0.0);
        Self {
            noise: read_f64(element.attr("noise"), 0.0) as f32,
            color: Color::from_attribute(element.attr("color")),
            texture: element.attr("texture").unwrap_or_default().into(),
            water_density,
            has_water: water_density > 0.1,
            max_height: max_height as f32,
            min_height: min_height as f32,
            base_height: if max_height > 0.0 {
                0.0
            } else {
                max_height as f32
            },
            water_level: if min_height > 0.0 { 0.0 } else { min_height },
            // PC 从首个 Range/Layer 开始，然后依次读取后续元素，而非再次按名称过滤。
            ranges: elements_from_first(element.child("Ranges"), "Range")
                .map(TerrainRange::from_element)
                .collect(),
            layers: elements_from_first(element.child("Layers"), "Layer")
                .map(TerrainLayer::from_element)
                .collect(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TerrainRange {
    pub start_angle: f64,
    pub end_angle: f64,
    pub min_height: f32,
    pub max_height: f32,
}

impl TerrainRange {
    fn from_element(element: &Element) -> Self {
        let start = read_f64(element.attr("startAngle"), 0.0);
        let end = read_f64(element.attr("endAngle"), 0.0);
        // 保持 COMISD + JBE 分支的取值顺序，包括 NaN 时选择 end 的边界。
        Self {
            start_angle: if end > start { start } else { end },
            end_angle: if start > end { start } else { end },
            min_height: read_f64(element.attr("minHeight"), 0.0) as f32,
            max_height: read_f64(element.attr("maxHeight"), 0.0) as f32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TerrainLayer {
    pub depth: f64,
    pub top_color: Color,
    pub bottom_color: Color,
    pub wave_length: f64,
    pub wave_height: f64,
}

impl TerrainLayer {
    fn from_element(element: &Element) -> Self {
        Self {
            depth: read_f64(element.attr("depth"), 0.0),
            top_color: Color::from_attribute(element.attr("topColor")),
            bottom_color: Color::from_attribute(element.attr("bottomColor")),
            wave_length: read_f64(element.attr("waveLength"), 0.0),
            wave_height: read_f64(element.attr("waveHeight"), 0.0),
        }
    }
}

fn elements_from_first<'a>(
    parent: Option<&'a Element>,
    name: &str,
) -> impl Iterator<Item = &'a Element> {
    let children = parent.map(|p| p.children.as_slice()).unwrap_or_default();
    let start = children
        .iter()
        .position(|child| child.name == name)
        .unwrap_or(children.len());
    children[start..].iter()
}

#[cfg(test)]
mod tests;
