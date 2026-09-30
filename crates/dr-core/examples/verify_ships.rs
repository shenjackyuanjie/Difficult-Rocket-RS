//! 对原版船体库执行完整模型往返校验，不修改输入文件。
use dr_core::{load_ship, ship_from_xml, ship_to_xml};
use std::{env, fs, path::PathBuf};

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
        checked += 1;
        parts += ship.parts.len()
            + ship
                .disconnected
                .iter()
                .map(|g| g.parts.len())
                .sum::<usize>();
    }
    assert!(checked > 0, "目录中没有可校验的 XML 船体");
    println!("校验通过：{checked} 个船体，{parts} 个部件，包含断开组与分级状态。");
    if rejected > 0 {
        return Err(format!("另有 {rejected} 个输入无法解析，详见上述错误").into());
    }
    Ok(())
}
