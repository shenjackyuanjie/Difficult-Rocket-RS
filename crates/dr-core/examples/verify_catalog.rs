//! 核对真实 PartList 的全部已存在元素和属性，输出只在内存中生成。
use dr_core::{catalog_from_xml, catalog_to_xml, load_catalog};
use quick_xml::{Reader, events::Event};
use std::{collections::BTreeMap, fs};

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
    let path = std::env::args().nth(1).ok_or("请提供 PartList.xml")?;
    let catalog = load_catalog(&path)?;
    let xml = catalog_to_xml(&catalog)?;
    let restored = catalog_from_xml(&xml)?;
    assert_eq!(restored.types, catalog.types);
    assert_eq!(restored.name, catalog.name);
    let fields = xml_fields(&xml)?;
    let mut checked = 0;
    for (key, value) in xml_fields(&fs::read_to_string(&path)?)? {
        if key.ends_with("/@xmlns") {
            continue;
        }
        let saved = fields
            .get(&key)
            .ok_or_else(|| format!("目录字段丢失：{key}"))?;
        if !same_value(&key, &value, saved) {
            return Err(format!("目录字段改变：{key}").into());
        }
        checked += 1;
    }
    println!(
        "目录往返通过：{} 类部件，{checked} 个原始元素/属性，静态规格与顺序均保留。",
        catalog.types.len()
    );
    Ok(())
}
