//! 对原版船体库执行完整模型往返校验，不修改输入文件。
use dr_core::{load_ship, ship_from_xml, ship_to_xml};
use quick_xml::{Reader, events::Event};
use std::{collections::BTreeMap, env, fs, path::PathBuf};

/// 按元素名称及同名兄弟序号记录属性，避免不同容器输出顺序影响比较。
fn xml_fields(xml: &str) -> Result<BTreeMap<String, String>, Box<dyn std::error::Error>> {
    let mut reader = Reader::from_str(xml);
    let mut stack = vec![(String::new(), BTreeMap::<String, usize>::new())];
    let mut fields = BTreeMap::new();
    loop {
        let event = reader.read_event()?;
        let is_start = matches!(event, Event::Start(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                let name = std::str::from_utf8(element.name().as_ref())?.to_owned();
                let (parent, siblings) = stack.last_mut().unwrap();
                let index = siblings.entry(name.clone()).or_default();
                let path = format!("{parent}/{name}[{index}]");
                *index += 1;
                fields.insert(path.clone(), String::new());
                for attribute in element.attributes() {
                    let attribute = attribute?;
                    let key = std::str::from_utf8(attribute.key.as_ref())?;
                    let value = attribute.decode_and_unescape_value(reader.decoder())?;
                    fields.insert(format!("{path}/@{key}"), value.into_owned());
                }
                if is_start {
                    stack.push((path, BTreeMap::new()));
                }
            }
            Event::End(_) => {
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(fields)
}

fn same_value(key: &str, original: &str, saved: &str) -> bool {
    if original == saved {
        return true;
    }
    if key.ends_with("/@name") || key.ends_with("/@partType") {
        return false;
    }
    if let (Ok(a), Ok(b)) = (original.parse::<i128>(), saved.parse::<i128>()) {
        return a == b;
    }
    matches!((original.parse::<f64>(), saved.parse::<f64>()), (Ok(a), Ok(b)) if a == b)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env::args().nth(1).ok_or("请提供船体目录")?);
    let mut paths = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut checked = 0;
    let mut parts = 0;
    let mut rejected = 0;
    for path in paths {
        if path.extension().is_none_or(|extension| extension != "xml") {
            continue;
        }
        let ship = match load_ship(&path) {
            Ok(ship) => ship,
            Err(error) => {
                eprintln!("拒绝读取 {}: {error}", path.display());
                rejected += 1;
                continue;
            }
        };
        let xml = ship_to_xml(&ship)?;
        assert!(xml.starts_with("<Ship "), "根节点错误: {}", path.display());
        let restored = ship_from_xml(&xml)?;
        assert_eq!(ship, restored, "往返数据丢失: {}", path.display());
        let original = xml_fields(&fs::read_to_string(&path)?)?;
        let saved = xml_fields(&xml)?;
        for (key, value) in original {
            let result = saved
                .get(&key)
                .ok_or_else(|| format!("{} 丢失原始字段 {key}", path.display()))?;
            if !same_value(&key, &value, result) {
                return Err(format!("{} 原始字段发生变化：{key}", path.display()).into());
            }
        }
        checked += 1;
        parts += ship.parts.len()
            + ship
                .disconnected
                .iter()
                .map(|g| g.parts.len())
                .sum::<usize>();
    }
    assert!(checked > 0, "目录中没有可校验的 XML 船体");
    println!(
        "校验通过：{checked} 个船体，{parts} 个部件，模型及原始 XML 元素、属性均通过往返检查。"
    );
    if rejected > 0 {
        return Err(format!("另有 {rejected} 个输入无法解析，详见上述错误").into());
    }
    Ok(())
}
