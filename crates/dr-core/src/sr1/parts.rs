//! SR1 PC 的部件工厂状态、加载归一与本体保存字段。
//!
//! 不使用编辑器 `model::Part` 的缺省值。尤其 Pod/Tank 不调用通用状态加载，
//! fuel/throttle 保持 f64，Lander/Parachute 的部分字段实际为 f32。
//! 原始元素仅供诊断；`to_element` 只写 PC 保存器支持的字段。

use super::catalog::{Catalog, PartClass};
use super::scalars::{read_bool, read_f32, read_f64, read_i32, write_bool, write_f32, write_f64};
use super::staging::Staging;
use super::xml::{self, Element};
use super::{Error, Result};

/// 比较阈值来自 PC 中由 float 常量提升得到的 double，而非十进制 f64 常量。
pub const THROTTLE_ZERO_THRESHOLD: f64 = 0.01_f32 as f64;
pub const THROTTLE_FULL_THRESHOLD: f64 = 0.99_f32 as f64;

/// 只在严格越过阈值时吸附到 0/1，不先将输入舍入成 f32。
pub fn normalize_throttle(value: f64) -> f64 {
    if value > THROTTLE_FULL_THRESHOLD {
        1.0
    } else if value < THROTTLE_ZERO_THRESHOLD {
        0.0
    } else {
        value
    }
}

/// PC 的比较分支相当于先选 `fuel < capacity ? fuel : capacity` 再取正值。
/// 不使用 Rust `clamp`：负容量不会 panic，NaN 的比较分支也不相同。
pub fn clamp_loaded_fuel(fuel: f64, capacity: f64) -> f64 {
    let capped = if capacity > fuel { fuel } else { capacity };
    if capped > 0.0 { capped } else { 0.0 }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PodState {
    pub throttle: f64,
    pub name: String,
    /// 输入没有自身 Staging 时保持 None，便于 Ship 进行旧根迁移。
    /// PC 保存器仍会把 None 对应的内部空表写成 Staging。
    pub staging: Option<Staging>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FuelState {
    pub fuel: f64,
    pub capacity: f64,
}

impl FuelState {
    /// 原生构造先装满；没有运行态 Tank/Engine 子元素时不执行加载裁剪。
    pub fn new(capacity: f64) -> Self {
        Self {
            fuel: capacity,
            capacity,
        }
    }

    fn load(element: Option<&Element>, capacity: f64) -> Self {
        let mut state = Self::new(capacity);
        if let Some(element) = element {
            state.fuel = clamp_loaded_fuel(read_f64(element.attr("fuel"), 0.0), capacity);
        }
        state
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParachuteState {
    pub chute_x: f64,
    pub chute_y: f64,
    pub chute_angle: f32,
    pub chute_height: f32,
    pub inflation: f32,
    pub inflate: bool,
    pub deployed: bool,
    pub rope: bool,
}

impl Default for ParachuteState {
    /// 构造的 inflation 为 0.1f32；Load 会以自己的 0 初值覆盖该字段。
    fn default() -> Self {
        Self {
            chute_x: 0.0,
            chute_y: 0.0,
            chute_angle: 0.0,
            chute_height: 0.0,
            inflation: 0.1,
            inflate: false,
            deployed: false,
            rope: false,
        }
    }
}

impl ParachuteState {
    fn load(element: &Element) -> Self {
        Self {
            chute_x: read_f64(element.attr("chuteX"), 0.0),
            chute_y: read_f64(element.attr("chuteY"), 0.0),
            chute_angle: read_f32(element.attr("chuteAngle"), 0.0),
            chute_height: read_f32(element.attr("chuteHeight"), 0.0),
            inflation: read_f32(element.attr("inflation"), 0.0),
            inflate: read_bool(element.attr("inflate"), false),
            deployed: read_bool(element.attr("deployed"), false),
            rope: read_bool(element.attr("rope"), false),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LanderState {
    pub lower: bool,
    pub raise: bool,
    pub length: f32,
    pub leg_angle: f32,
}

impl LanderState {
    /// ObjectCreator 构造长度为目录 minLength 窄化为 f32，腿角为零。
    pub fn new(min_length: f64) -> Self {
        Self {
            length: min_length as f32,
            ..Self::default()
        }
    }

    fn load(element: &Element) -> Self {
        Self {
            lower: read_bool(element.attr("lower"), false),
            raise: read_bool(element.attr("raise"), false),
            // 两个属性各自用局部 0 初值，不回退到构造的 minLength。
            length: read_f32(element.attr("length"), 0.0),
            leg_angle: read_f32(element.attr("legAngle"), 0.0),
        }
    }
}

/// 太阳能板运行态，不是 Lander 的起落腿。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SolarState {
    pub extension: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PartState {
    /// Detacher、DockPort、DockConnector、Wheel、Rcs、Nosecone 同样只存通用字段。
    Generic,
    Pod(PodState),
    Tank(FuelState),
    Engine(FuelState),
    Parachute(ParachuteState),
    Lander(LanderState),
    Solar(SolarState),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub id: i32,
    pub part_type: String,
    pub class: PartClass,
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub angle_v: f64,
    pub editor_angle: i32,
    pub flipped_x: bool,
    pub flipped_y: bool,
    /// 对应 XML 的 activated，而非 active。
    pub active: bool,
    pub exploded: bool,
    pub state: PartState,
    source: Element,
}

impl Part {
    /// 按目录实际工厂类别加载。没有匹配的目录项时返回明确错误，不伪造静态数据。
    ///
    /// 只读取属性状态。Pod 分级引用在 Ship 建树后按其子树二阶段过滤。
    pub fn from_element(element: &Element, catalog: &Catalog) -> Result<Self> {
        element.require_name("Part")?;
        let type_id = element.attr("partType").unwrap_or("");
        let part_type = catalog
            .get(type_id)
            .ok_or_else(|| Error::InvalidData(format!("部件引用的目录类型不存在：{type_id}")))?;
        let state = match part_type.class {
            PartClass::Pod => {
                let mut pod = PodState::default();
                if let Some(element) = element.child("Pod") {
                    pod.throttle = normalize_throttle(read_f64(element.attr("throttle"), 0.0));
                    pod.name = element.attr("name").unwrap_or("").to_owned();
                    pod.staging = element
                        .child("Staging")
                        .map(Staging::from_element)
                        .transpose()?;
                }
                PartState::Pod(pod)
            }
            PartClass::Tank => PartState::Tank(FuelState::load(
                element.child("Tank"),
                part_type.fuel_capacity().unwrap_or(0.0),
            )),
            PartClass::Engine => PartState::Engine(FuelState::load(
                element.child("Engine"),
                part_type.fuel_capacity().unwrap_or(0.0),
            )),
            PartClass::Parachute => PartState::Parachute(ParachuteState::load(element)),
            PartClass::Lander => PartState::Lander(LanderState::load(element)),
            PartClass::Solar => PartState::Solar(SolarState {
                // Solar 与直接 %f 扫描不同：%lf 后用 cvtpd2ps 窄化。
                extension: read_f64(element.attr("extension"), 0.0) as f32,
            }),
            _ => PartState::Generic,
        };
        let reads_base = !matches!(state, PartState::Pod(_) | PartState::Tank(_));
        Ok(Self {
            id: read_i32(element.attr("id"), 0),
            part_type: part_type.id.clone(),
            class: part_type.class,
            x: read_f64(element.attr("x"), 0.0),
            y: read_f64(element.attr("y"), 0.0),
            angle: read_f64(element.attr("angle"), 0.0),
            angle_v: read_f64(element.attr("angleV"), 0.0),
            editor_angle: read_i32(element.attr("editorAngle"), 0),
            flipped_x: reads_base && read_bool(element.attr("flippedX"), false),
            flipped_y: reads_base && read_bool(element.attr("flippedY"), false),
            active: reads_base && read_bool(element.attr("activated"), false),
            exploded: reads_base && read_bool(element.attr("exploded"), false),
            state,
            source: element.clone(),
        })
    }

    pub fn from_xml(source: &str, catalog: &Catalog) -> Result<Self> {
        Self::from_element(&xml::parse(source)?, catalog)
    }

    /// 诊断用完整输入；不把其中未消费的属性和子元素当成本体保存结果。
    pub fn source_element(&self) -> &Element {
        &self.source
    }

    pub fn pod(&self) -> Option<&PodState> {
        match &self.state {
            PartState::Pod(pod) => Some(pod),
            _ => None,
        }
    }

    pub fn pod_mut(&mut self) -> Option<&mut PodState> {
        match &mut self.state {
            PartState::Pod(pod) => Some(pod),
            _ => None,
        }
    }

    /// 本体规范化保存，不保留输入词法、未知字段，也不运行物理更新。
    pub fn to_element(&self) -> Element {
        let mut element = Element::new("Part");
        element.set_attr("partType", &self.part_type);
        element.set_attr("id", self.id.to_string());
        element.set_attr("x", write_f64(self.x));
        element.set_attr("y", write_f64(self.y));
        element.set_attr("angle", write_f64(self.angle));
        element.set_attr("angleV", write_f64(self.angle_v));
        element.set_attr("editorAngle", self.editor_angle.to_string());
        if !matches!(self.state, PartState::Pod(_) | PartState::Tank(_)) {
            element.set_attr("activated", write_bool(self.active));
            element.set_attr("exploded", write_bool(self.exploded));
            element.set_attr("flippedX", write_bool(self.flipped_x));
            element.set_attr("flippedY", write_bool(self.flipped_y));
        }
        match &self.state {
            PartState::Pod(pod) => {
                let mut child = Element::new("Pod");
                child.set_attr("throttle", write_f64(pod.throttle));
                child.set_attr("name", &pod.name);
                child.children.push(
                    pod.staging
                        .as_ref()
                        .map(Staging::to_element)
                        .unwrap_or_else(|| Staging::default().to_element()),
                );
                element.children.push(child);
            }
            PartState::Tank(fuel) | PartState::Engine(fuel) => {
                let mut child = Element::new(if matches!(self.state, PartState::Tank(_)) {
                    "Tank"
                } else {
                    "Engine"
                });
                child.set_attr("fuel", write_f64(fuel.fuel));
                element.children.push(child);
            }
            PartState::Parachute(chute) => {
                element.set_attr("chuteX", write_f64(chute.chute_x));
                element.set_attr("chuteY", write_f64(chute.chute_y));
                element.set_attr("chuteAngle", write_f32(chute.chute_angle));
                element.set_attr("chuteHeight", write_f32(chute.chute_height));
                element.set_attr("inflation", write_f32(chute.inflation));
                element.set_attr("inflate", write_bool(chute.inflate));
                element.set_attr("deployed", write_bool(chute.deployed));
                element.set_attr("rope", write_bool(chute.rope));
            }
            PartState::Lander(lander) => {
                element.set_attr("lower", write_bool(lander.lower));
                element.set_attr("raise", write_bool(lander.raise));
                element.set_attr("length", write_f32(lander.length));
                element.set_attr("legAngle", write_f32(lander.leg_angle));
            }
            PartState::Solar(solar) => {
                element.set_attr("extension", write_f32(solar.extension));
            }
            PartState::Generic => {}
        }
        element
    }

    pub fn to_xml(&self) -> Result<String> {
        xml::to_string(&self.to_element())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        // PC PartList/DefaultShip 的字段缩小样本，不依赖本机游戏路径。
        Catalog::from_xml(r#"<PartTypes>
            <PartType id="pod-1" name="Pod" type="pod"><AttachPoints/></PartType>
            <PartType id="fueltank-2" name="Tank" type="tank"><Tank fuel="3000" dryMass="0.85"/><AttachPoints/></PartType>
            <PartType id="engine-1" name="Engine" type="engine"><Engine/><Tank fuel="12.125"/><AttachPoints/></PartType>
            <PartType id="solid-1" name="Solid" type="engine"><Engine/><AttachPoints/></PartType>
            <PartType id="lander-1" name="Lander" type="lander"><Lander minLength="2.5"/><AttachPoints/></PartType>
            <PartType id="parachute-1" name="Parachute" type="parachute"><AttachPoints/></PartType>
            <PartType id="solar-1" name="Solar" type="solar"><Solar/><AttachPoints/></PartType>
            <PartType id="strut-1" name="Strut" type="strut"><AttachPoints/></PartType>
        </PartTypes>"#).unwrap()
    }

    fn load(source: &str) -> Part {
        Part::from_xml(source, &catalog()).unwrap()
    }

    fn fuel(part: &Part) -> &FuelState {
        match &part.state {
            PartState::Tank(fuel) | PartState::Engine(fuel) => fuel,
            _ => panic!("非燃料状态"),
        }
    }

    #[test]
    fn part_common_fields_keep_native_widths_prefixes_and_independent_angles() {
        let part = load(
            r#"<Part partType="strut-1" id="-9tail" x="16777217.25x" y="-2.5" angle="0.125" angleV="-0.25" editorAngle="3tail" flippedX="1" flippedY="true" activated="-2" exploded="bad"/>"#,
        );
        assert_eq!(part.id, -9);
        assert_eq!(part.x, 16777217.25);
        assert_eq!(part.y, -2.5);
        assert_eq!(part.angle, 0.125);
        assert_eq!(part.angle_v, -0.25);
        assert_eq!(part.editor_angle, 3);
        assert!(part.flipped_x && part.flipped_y && part.active);
        assert!(!part.exploded);
        let saved = part.to_element();
        assert_eq!(saved.attr("angle"), Some("0.125000"));
        assert_eq!(saved.attr("editorAngle"), Some("3"));
        assert_eq!(saved.attr("activated"), Some("1"));
    }

    #[test]
    fn part_missing_or_bad_common_values_use_loader_zero() {
        let part =
            load(r#"<Part partType="strut-1" id="bad" x="bad" angleV="bad" editorAngle="bad"/>"#);
        assert_eq!((part.id, part.editor_angle), (0, 0));
        assert_eq!(
            (part.x, part.y, part.angle, part.angle_v),
            (0.0, 0.0, 0.0, 0.0)
        );
        assert!(!part.active && !part.exploded && !part.flipped_x && !part.flipped_y);
    }

    #[test]
    fn fuel_missing_child_keeps_capacity_but_present_child_missing_value_loads_zero() {
        for (id, child, capacity) in [
            ("fueltank-2", "Tank", 3000.0),
            ("engine-1", "Engine", 12.125),
        ] {
            assert_eq!(
                fuel(&load(&format!("<Part partType=\"{id}\"/>"))).fuel,
                capacity
            );
            for value in ["", " fuel=\"bad\""] {
                let part = load(&format!("<Part partType=\"{id}\"><{child}{value}/></Part>"));
                assert_eq!(fuel(&part).fuel, 0.0);
                assert_eq!(fuel(&part).capacity, capacity);
            }
        }
    }

    #[test]
    fn fuel_uses_f64_and_clips_only_the_matching_class_child() {
        for (input, expected) in [
            ("-1", 0.0),
            ("3001", 3000.0),
            ("2999.9999999", 2999.9999999),
        ] {
            let part = load(&format!(
                "<Part partType=\"fueltank-2\"><Tank fuel=\"{input}\"/></Part>"
            ));
            assert_eq!(fuel(&part).fuel, expected);
        }
        let part = load(
            r#"<Part partType="fueltank-2"><Engine fuel="1"/><Tank fuel="7.125"/><Tank fuel="9"/></Part>"#,
        );
        assert_eq!(fuel(&part).fuel, 7.125);
        let part = load(r#"<Part partType="solid-1"><Engine fuel="100"/></Part>"#);
        assert_eq!(fuel(&part).fuel, 0.0);
        assert_eq!(clamp_loaded_fuel(1.0, -1.0), 0.0);
        assert_eq!(clamp_loaded_fuel(f64::NAN, 10.0), 10.0);
        assert_eq!(clamp_loaded_fuel(1.0, f64::NAN), 0.0);
    }

    #[test]
    fn pod_and_tank_skip_base_fields_while_engine_loads_them() {
        for id in ["pod-1", "fueltank-2", "engine-1"] {
            let part = load(&format!(
                "<Part partType=\"{id}\" activated=\"1\" exploded=\"1\" flippedX=\"1\" flippedY=\"1\"/>"
            ));
            let reads_base = id == "engine-1";
            assert_eq!(part.active, reads_base);
            assert_eq!(part.exploded, reads_base);
            assert_eq!(part.flipped_x, reads_base);
            assert_eq!(part.flipped_y, reads_base);
            assert_eq!(part.to_element().attr("activated").is_some(), reads_base);
        }
    }

    #[test]
    fn throttle_uses_promoted_float_thresholds_and_strict_inequalities() {
        let low = THROTTLE_ZERO_THRESHOLD;
        let high = THROTTLE_FULL_THRESHOLD;
        assert_eq!(normalize_throttle(low), low);
        assert_eq!(normalize_throttle(high), high);
        assert_eq!(normalize_throttle(f64::from_bits(low.to_bits() - 1)), 0.0);
        assert_eq!(normalize_throttle(f64::from_bits(high.to_bits() + 1)), 1.0);
        assert_eq!(normalize_throttle(0.99), 0.99);
        assert_eq!(normalize_throttle(-1.0), 0.0);
        assert_eq!(normalize_throttle(10.0), 1.0);
        assert!(normalize_throttle(f64::NAN).is_nan());
        let part =
            load(r#"<Part partType="pod-1"><Pod throttle="0.990000005" name="名字"/></Part>"#);
        assert_eq!(part.pod().unwrap().throttle, 0.990000005);
    }

    #[test]
    fn pod_staging_presence_supports_migration_and_save_always_writes_a_table() {
        let mut part = load(r#"<Part partType="pod-1"/>"#);
        assert!(part.pod().unwrap().staging.is_none());
        let saved = part.to_element();
        let pod = saved.child("Pod").unwrap();
        assert_eq!(pod.attr("throttle"), Some("0.000000"));
        assert_eq!(
            pod.child("Staging").unwrap().attr("currentStage"),
            Some("0")
        );
        let staged = load(
            r#"<Part partType="pod-1"><Pod><Staging currentStage="7"><Step><Activate Id="-1" moved="0"/></Step></Staging></Pod></Part>"#,
        );
        let staging = staged.pod().unwrap().staging.as_ref().unwrap();
        assert_eq!(staging.current_stage, 7);
        assert_eq!(staging.steps[0].parts[0].part_id, -1);
        assert!(staging.steps[0].parts[0].moved);
        part.pod_mut().unwrap().name = "测试".into();
        assert_eq!(
            part.to_element().child("Pod").unwrap().attr("name"),
            Some("测试")
        );
    }

    #[test]
    fn parachute_load_overrides_constructor_and_preserves_mixed_float_widths() {
        assert_eq!(ParachuteState::default().inflation, 0.1_f32);
        let empty = load(r#"<Part partType="parachute-1"/>"#);
        let PartState::Parachute(empty) = empty.state else {
            panic!("非降落伞")
        };
        assert_eq!(empty.inflation, 0.0);
        let part = load(
            r#"<Part partType="parachute-1" chuteX="16777217" chuteY="-1.25" chuteAngle="16777217" chuteHeight="-3" inflation="1.25" inflate="true" deployed="1" rope="-1"/>"#,
        );
        let PartState::Parachute(chute) = &part.state else {
            panic!("非降落伞")
        };
        assert_eq!(chute.chute_x, 16777217.0);
        assert_eq!(chute.chute_angle, 16777216.0_f32);
        assert_eq!(chute.chute_height, -3.0);
        assert_eq!(chute.inflation, 1.25);
        assert!(chute.inflate && chute.deployed && chute.rope);
        let saved = part.to_element();
        assert_eq!(saved.attr("chuteX"), Some("16777217.000000"));
        assert_eq!(saved.attr("chuteAngle"), Some("16777216.000000"));
    }

    #[test]
    fn lander_load_and_save_all_four_fields_without_clamping_or_mirror_rewrite() {
        assert_eq!(LanderState::new(2.5).length, 2.5);
        let empty = load(r#"<Part partType="lander-1"/>"#);
        let PartState::Lander(empty) = empty.state else {
            panic!("非Lander")
        };
        assert_eq!(empty.length, 0.0);
        let part = load(
            r#"<Part partType="lander-1" lower="1" raise="true" length="-2.125" legAngle="16777217" flippedX="true"/>"#,
        );
        let PartState::Lander(lander) = &part.state else {
            panic!("非Lander")
        };
        assert_eq!(lander.length, -2.125);
        assert_eq!(lander.leg_angle, 16777216.0_f32);
        let saved = part.to_element();
        assert_eq!(saved.attr("lower"), Some("1"));
        assert_eq!(saved.attr("raise"), Some("1"));
        assert_eq!(saved.attr("length"), Some("-2.125000"));
        assert_eq!(saved.attr("legAngle"), Some("16777216.000000"));
        assert_eq!(saved.attr("flippedX"), Some("1"));
    }

    #[test]
    fn solar_extension_is_a_narrowed_double_and_is_not_clamped() {
        let part = load(r#"<Part partType="solar-1" extension="16777217"/>"#);
        let PartState::Solar(solar) = &part.state else {
            panic!("非Solar")
        };
        assert_eq!(solar.extension, 16777216.0_f32);
        assert_eq!(part.to_element().attr("extension"), Some("16777216.000000"));
        let part = load(r#"<Part partType="solar-1" extension="-2.5"/>"#);
        assert_eq!(part.to_element().attr("extension"), Some("-2.500000"));
    }

    #[test]
    fn source_retains_unused_data_but_native_save_does_not_copy_it() {
        let part = load(
            r#"<Part partType="strut-1" unused="42"><Tank fuel="100"/><Mod key="保留"/></Part>"#,
        );
        assert_eq!(part.source_element().attr("unused"), Some("42"));
        assert!(part.source_element().child("Mod").is_some());
        assert!(part.to_element().attr("unused").is_none());
        assert!(part.to_element().children.is_empty());
    }

    #[test]
    fn direct_float_scan_differs_from_double_scan_then_narrowing_at_overflow_midpoint() {
        // 同版 CRT 的 %f 金样：直接扫描是 MAX，先 %lf 后窄化则为无穷大。
        let value = "3.4028235677973366e38";
        let chute = load(&format!(
            "<Part partType=\"parachute-1\" chuteAngle=\"{value}\"/>"
        ));
        let PartState::Parachute(chute) = chute.state else {
            panic!("非降落伞")
        };
        assert_eq!(chute.chute_angle.to_bits(), f32::MAX.to_bits());
        let solar = load(&format!(
            "<Part partType=\"solar-1\" extension=\"{value}\"/>"
        ));
        let PartState::Solar(solar) = solar.state else {
            panic!("非Solar")
        };
        assert_eq!(solar.extension, f32::INFINITY);
    }

    #[test]
    fn factory_reports_wrong_root_and_unresolved_type() {
        assert!(matches!(
            Part::from_xml("<Ship/>", &catalog()),
            Err(Error::WrongRoot { .. })
        ));
        assert!(matches!(
            Part::from_xml("<Part partType=\"missing\"/>", &catalog()),
            Err(Error::InvalidData(_))
        ));
    }
}
