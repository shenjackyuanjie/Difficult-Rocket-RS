//! 在真实重复 ID 样本上验证实例编辑、撤销重做及 XML 往返，不写入样本。
use dr_core::{EditorCommand, EditorHistory, load_ship, ship_from_xml, ship_to_xml};
use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::PathBuf,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = PathBuf::from(env::args().nth(1).ok_or("请提供船体目录")?);
    let mut paths = fs::read_dir(directory)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<Vec<_>, _>>()?;
    paths.sort();
    let mut samples = 0;
    let mut checked = 0;
    let mut rejected = 0;
    for path in paths {
        if path.extension().is_none_or(|ext| ext != "xml") {
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
        let mut counts = HashMap::<i64, usize>::new();
        for part in ship.all_parts() {
            *counts.entry(part.id).or_default() += 1;
        }
        let mut covered = HashSet::new();
        // 每组抽取一个重复 ID，并额外覆盖所有同组重复实例。
        let keys: Vec<_> = ship
            .keyed_parts()
            .filter(|(key, _)| counts[&key.id] > 1)
            .filter(|(key, _)| key.occurrence > 0 || covered.insert(key.group))
            .map(|(key, _)| key)
            .collect();
        if keys.is_empty() {
            continue;
        }
        for key in &keys {
            let mut edited = ship.clone();
            let mut expected = ship.clone();
            let part = expected.part_at_mut(*key).unwrap();
            part.active = !part.active;
            let active = part.active;
            let mut history = EditorHistory::with_limit(1);
            history.execute(
                &mut edited,
                EditorCommand::SetActive(key.id, active).at(*key),
            )?;
            assert_eq!(edited, expected, "错误实例被修改：{} {key}", path.display());
            assert!(history.undo(&mut edited));
            assert_eq!(edited, ship, "撤销未恢复原始数据：{} {key}", path.display());
            assert!(history.redo(&mut edited));
            assert_eq!(edited, expected);
            assert_eq!(
                ship_from_xml(&ship_to_xml(&edited)?)?,
                expected,
                "编辑后的 XML 往返失败：{} {key}",
                path.display()
            );
            checked += 1;
        }
        samples += 1;
        println!("通过 {}：{} 个实例", path.display(), keys.len());
    }
    assert!(checked > 0, "未找到重复 ID 样本");
    println!(
        "分组编辑校验通过：{samples} 个真实重复 ID 样本，{checked} 个实例；所有操作仅在内存执行。"
    );
    if rejected > 0 {
        return Err(format!("另有 {rejected} 个输入无法解析，详见上述错误").into());
    }
    Ok(())
}
