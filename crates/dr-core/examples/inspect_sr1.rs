//! 用原生语义读取实际资源，不启动编辑器、不修改输入文件。

use dr_core::sr1::{catalog::Catalog, runtime::Runtime, ship::Ship, solar::SolarSystem, xml};
use std::{env, error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = env::args_os().skip(1).map(PathBuf::from).collect();
    if !(2..=3).contains(&args.len()) {
        return Err(
            "用法：inspect_sr1 <PartList.xml> <Ship或Runtime.xml> [SolarSystem.xml]".into(),
        );
    }
    let catalog = Catalog::from_xml(&fs::read_to_string(&args[0])?)?;
    let source = fs::read_to_string(&args[1])?;
    let element = xml::parse(&source)?;
    println!("原生目录：{} 种部件", catalog.parts.len());
    match element.name.as_str() {
        "Ship" => {
            let ship = Ship::from_editor_element(&element, &catalog)?;
            let output = ship.to_editor_xml()?;
            let saved = Ship::from_editor_xml(&output, &catalog)?;
            println!(
                "船体：version={}，主树 {} 个输入部件 / {} 个可达部件，{} 组分离树，默认Pod={:?}",
                ship.version,
                ship.tree.parts().len(),
                ship.tree.traversal().count(),
                ship.disconnected.len(),
                ship.selected_pod_id()
            );
            println!(
                "规范保存：{} 字节，版本 {}，主树 {} 个部件（只在内存中验证，不写文件）",
                output.len(),
                saved.version,
                saved.tree.parts().len()
            );
        }
        "Runtime" => {
            let path = args
                .get(2)
                .ok_or("检查 Runtime 绑定需要显式提供 SolarSystem.xml")?;
            let system = SolarSystem::from_xml(&fs::read_to_string(path)?)?;
            let runtime = Runtime::from_element(&element, &catalog)?;
            let bindings = runtime.bind_solar_system(&system)?;
            println!(
                "运行态：time={}，{} 个XML节点，{} 艘船，星系资源请求={:?}",
                runtime.time,
                runtime.nodes.len(),
                runtime.ship_nodes().count(),
                runtime.solar_system
            );
            println!(
                "绑定：{} 个星体，{} 个船只引用；载入选择请求 shipId={} podId={}（未冒充当前实际选择）",
                bindings.planets.len(),
                bindings.ships.len(),
                runtime.ship_id,
                runtime.pod_id
            );
            println!(
                "仅加载普通 Runtime；挑战类型应由关卡上下文显式传给 API，此工具不按属性猜测。未推进轨道或写出运行态快照。"
            );
        }
        other => return Err(format!("不支持的数据根：{other}").into()),
    }
    Ok(())
}
