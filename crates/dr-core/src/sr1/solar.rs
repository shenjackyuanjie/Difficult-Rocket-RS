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
mod tests {
    use super::*;

    // 按真实 SmolarSystem.xml 缩小：保留 Sun/Smearth/Smoon 层级、轨道和海洋山脉数据。
    const SMOLAR: &str = r#"<SolarSystem name="Smolar System" description="small">
      <Planet name="Sun" gravity="274.0" radius="69634200.0"><Terrain maxHeight="0"/>
        <Children><Planet name="Smearth" gravity="9.798000" radius="637100.0" mapColor="103,157,255">
          <Orbit w="1.993303" e="0.016711" prograde="1" a="12559826100.0" v="-3.013170"/>
          <Terrain maxHeight="3250.0" minHeight="-1000.0" noise="2.0" waterDensity="75">
            <Ranges><Range startAngle="20" endAngle="89" minHeight="-2000" maxHeight="-1000"/>
              <Range startAngle="91" endAngle="93" minHeight="4000" maxHeight="8000"/></Ranges>
          </Terrain><Atmosphere height="70000" surfacePressure="1.0" graphicType="1"/>
          <Children><Planet name="Smoon" gravity="1.622000" radius="173710.0">
            <Orbit w="1.993303" e="0.054900" prograde="1" a="12500000.0" v="-2.138368"/>
            <Terrain maxHeight="500.0" noise="3.5"/></Planet></Children>
        </Planet></Children>
      </Planet></SolarSystem>"#;

    #[test]
    fn smolar_resource_subset_preserves_tree_and_orbits() {
        let system = SolarSystem::from_xml(SMOLAR).unwrap();
        assert_eq!(
            system
                .planets()
                .iter()
                .map(|p| p.name.as_str())
                .collect::<Vec<_>>(),
            ["Sun", "Smearth", "Smoon"]
        );
        let earth = system.planet("Smearth").unwrap();
        assert_eq!(earth.radius, 637100.0);
        assert_eq!(earth.orbit.unwrap().v, -3.013170);
        assert!(earth.launch_enabled);
        assert!(earth.terrain.has_water);
        assert_eq!(earth.terrain.ranges.len(), 2);
        assert_eq!(earth.atmosphere.unwrap().surface_pressure, 1.0);
        assert_eq!(
            earth.mass(),
            637100.0 * 637100.0 * 9.798 / GRAVITATIONAL_CONSTANT
        );
    }

    #[test]
    fn missing_or_invalid_planet_values_keep_pc_defaults() {
        let system = SolarSystem::from_xml(r#"<SolarSystem><Planet gravity="bad" radius="bad" launchEnabled="bad"><Orbit/><Atmosphere/><Terrain/></Planet></SolarSystem>"#).unwrap();
        assert_eq!(system.name, "");
        assert_eq!(system.root.gravity, 0.0);
        assert_eq!(system.root.radius, 0.0);
        assert!(system.root.launch_enabled);
        assert_eq!(system.root.orbit, Some(Orbit::default()));
        assert_eq!(system.root.atmosphere, Some(Atmosphere::default()));
        assert_eq!(system.root.map_color, Color::BLACK);
    }

    #[test]
    fn missing_terrain_is_an_error_and_only_first_root_is_loaded() {
        assert!(matches!(
            SolarSystem::from_xml("<SolarSystem><Planet/></SolarSystem>"),
            Err(Error::InvalidData(_))
        ));
        let system = SolarSystem::from_xml("<SolarSystem><Planet name='first'><Terrain/></Planet><Planet name='ignored'/></SolarSystem>").unwrap();
        assert_eq!(system.root.name, "first");
    }

    #[test]
    fn terrain_casts_only_pc_float_fields_and_sorts_angle_endpoints() {
        let system = SolarSystem::from_xml(r#"<SolarSystem><Planet><Terrain noise="16777217" maxHeight="-16777217" minHeight="-16777217" waterDensity="0.1"><Ranges><Range startAngle="110" endAngle="80" minHeight="16777217" maxHeight="16777219"/></Ranges><Layers><Layer depth="16777217" waveLength="42" waveHeight="-2"/></Layers></Terrain></Planet></SolarSystem>"#).unwrap();
        let terrain = &system.root.terrain;
        assert_eq!(terrain.noise, 16777216.0_f32);
        assert_eq!(terrain.max_height, -16777216.0_f32);
        assert_eq!(terrain.water_level, -16777217.0);
        assert_eq!(terrain.base_height, terrain.max_height);
        assert!(!terrain.has_water);
        assert_eq!(terrain.ranges[0].start_angle, 80.0);
        assert_eq!(terrain.ranges[0].end_angle, 110.0);
        assert_eq!(terrain.ranges[0].min_height, 16777216.0_f32);
        assert_eq!(terrain.layers[0].depth, 16777217.0);
    }

    #[test]
    fn terrain_positive_heights_and_water_threshold_follow_native_branches() {
        let system = SolarSystem::from_xml("<SolarSystem><Planet><Terrain maxHeight='10' minHeight='20' waterDensity='0.100000001'/></Planet></SolarSystem>").unwrap();
        assert_eq!(system.root.terrain.base_height, 0.0);
        assert_eq!(system.root.terrain.water_level, 0.0);
        assert!(system.root.terrain.has_water);
    }

    #[test]
    fn color_skips_empty_items_truncates_components_and_requires_exact_alpha_count() {
        assert_eq!(Color::from_attribute(None), Color::BLACK);
        assert_eq!(Color::from_attribute(Some("1,2")), Color::BLACK);
        assert_eq!(
            Color::from_attribute(Some(",257,,-1,3tail,4,")),
            Color {
                r: 1,
                g: 255,
                b: 3,
                a: 4
            }
        );
        assert_eq!(Color::from_attribute(Some("1,2,invalid,4,5")).a, 255);
        assert_eq!(Color::from_attribute(Some("1,2,invalid,4,5")).b, 0);
    }

    #[test]
    fn duplicate_names_keep_first_preorder_match_and_children_filter_by_name() {
        let system = SolarSystem::from_xml("<SolarSystem><Planet name='same'><Terrain/><Children><Other/><Planet name='same'><Terrain/></Planet></Children></Planet></SolarSystem>").unwrap();
        assert_eq!(system.planets().len(), 2);
        assert!(std::ptr::eq(system.planet("same").unwrap(), &system.root));
    }

    #[test]
    fn ranges_and_layers_read_all_elements_after_first_named_entry() {
        let system = SolarSystem::from_xml("<SolarSystem><Planet><Terrain><Ranges><Other startAngle='99'/><Range startAngle='4' endAngle='1'/><Other startAngle='9' endAngle='6'/></Ranges><Layers><Other/><Layer depth='1'/><Other depth='2'/></Layers></Terrain></Planet></SolarSystem>").unwrap();
        assert_eq!(system.root.terrain.ranges.len(), 2);
        assert_eq!(system.root.terrain.ranges[1].start_angle, 6.0);
        assert_eq!(system.root.terrain.layers.len(), 2);
        assert_eq!(system.root.terrain.layers[1].depth, 2.0);
    }
}
