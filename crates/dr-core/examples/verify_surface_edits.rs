//! 真实目录连接面的有限非中心抽样；不把离散样本当成连续接触面的穷尽证明。
use dr_core::{
    CommandError, Connection, EditorCommand, EditorHistory, LinkKind, Part, PartCatalog, PartKey,
    PartKind, PartType, Ship, Vec2d, connections, intersects, load_catalog, part_world_attach,
    ship_from_xml, ship_to_xml,
};
use std::collections::HashSet;

const EPSILON: f64 = 1e-6;
const FRACTIONS: [f64; 2] = [0.2, 0.8];
const COMMON_ANGLES: [f64; 2] = [0.0, 0.37];

#[derive(Default)]
struct Coverage {
    compatible: usize,
    centered: usize,
    collision: usize,
    committed: usize,
    oblique: usize,
    surface_pairs: HashSet<(usize, usize)>,
    xml_pairs: HashSet<(usize, usize)>,
}

fn pose(kind: &PartType, id: i64, orientation: usize, common_angle: f64) -> Part {
    let mut part = kind.instantiate(id, (0.0, 0.0));
    part.editor_angle = (orientation % 4) as i32;
    part.angle = part.editor_angle as f64 * std::f64::consts::FRAC_PI_2 + common_angle;
    part.flip_x = orientation & 4 != 0;
    part.flip_y = orientation & 8 != 0;
    part
}

fn point(segment: (Vec2d, Vec2d), fraction: f64) -> Vec2d {
    Vec2d {
        x: segment.0.x + fraction * (segment.1.x - segment.0.x),
        y: segment.0.y + fraction * (segment.1.y - segment.0.y),
    }
}

fn align(source: &mut Part, a: Vec2d, b: Vec2d) {
    source.x += b.x - a.x;
    source.y += b.y - a.y;
}

fn command(st: &PartType, source_id: i64, si: usize, ti: usize) -> EditorCommand {
    let parent = PartKey::new(0, 1, 0);
    let child = PartKey::new(0, source_id, 0);
    let kind = if st.attach_points[si].dock {
        LinkKind::Dock {
            connector: if st.kind == PartKind::DockConnector {
                child
            } else {
                parent
            },
        }
    } else {
        LinkKind::Normal {
            parent_attach: ti as i32 + 1,
            child_attach: si as i32 + 1,
        }
    };
    EditorCommand::ConnectParts {
        parent,
        child,
        kind,
    }
}

fn verify_endpoints(ship: &Ship, catalog: &PartCatalog) {
    for connection in &ship.connections {
        // Dock 的渲染线连接部件中心，不应误认成连接面端点；其接触面在提交前另验。
        if matches!(connection, Connection::Normal { .. }) {
            let (a, b) = connections::positions(ship, catalog, connection).unwrap();
            assert!(
                a.distance(b) < EPSILON,
                "提交后连接端点分离：{connection:?}"
            );
        }
    }
}

fn verify_pair(
    catalog: &PartCatalog,
    source: Part,
    target: Part,
    si: usize,
    ti: usize,
    xml: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let st = catalog.get(&source.part_type).unwrap();
    let tt = catalog.get(&target.part_type).unwrap();
    let a = connections::segment(&source, st, &st.attach_points[si]);
    let b = connections::segment(&target, tt, &tt.attach_points[ti]);
    let (a, b) = connections::closest_points(a, b);
    assert!(
        a.distance(b) < EPSILON,
        "连接面没有实际接触：{} → {}",
        st.id,
        tt.id
    );
    let candidates = connections::candidates(&source, st, &target, tt, EPSILON);
    let candidate = candidates
        .iter()
        .find(|c| c.source_index == si && c.target_index == ti)
        .unwrap_or_else(|| panic!("非中心候选遗漏：{} → {}，面 {si} → {ti}", st.id, tt.id));
    assert!(
        candidate.position.distance(Vec2d {
            x: source.x,
            y: source.y
        }) < EPSILON
    );
    let command = command(st, source.id, si, ti);
    let mut ship = Ship {
        parts: vec![target, source],
        ..Ship::default()
    };
    let before = ship.clone();
    let mut history = EditorHistory::with_limit(1);
    history.execute_with_catalog(&mut ship, catalog, command)?;
    assert_eq!(ship.connections.len(), 1);
    assert_eq!(
        ship.parts, before.parts,
        "连接提交不得改变预先验收的实体落点"
    );
    verify_endpoints(&ship, catalog);
    let after = ship.clone();
    assert!(history.undo(&mut ship));
    assert_eq!(ship, before);
    assert!(!history.can_undo());
    assert!(history.redo(&mut ship));
    assert_eq!(ship, after);
    if xml {
        assert_eq!(ship_from_xml(&ship_to_xml(&ship)?)?, ship);
    }
    Ok(())
}

/// 用原版长梁和竖向分离器隔离验证同边不同落点可连接、相同落点不可重复占用。
fn verify_occupancy(catalog: &PartCatalog) -> Result<usize, Box<dyn std::error::Error>> {
    let beam = catalog
        .get("strut-1")
        .ok_or("同边占用样本需要原版 strut-1")?;
    let small = catalog
        .get("detacher-1")
        .ok_or("同边占用样本需要原版 detacher-1")?;
    let ti = beam
        .attach_points
        .iter()
        .position(|a| a.location == "Top")
        .ok_or("strut-1 缺少 Top 连接面")?;
    let si = small
        .attach_points
        .iter()
        .position(|a| a.location == "BottomCenter")
        .ok_or("detacher-1 缺少 BottomCenter 连接点")?;
    for common_angle in COMMON_ANGLES {
        let target = pose(beam, 1, 0, common_angle);
        let target_surface = connections::segment(&target, beam, &beam.attach_points[ti]);
        let mut parts = vec![target.clone()];
        for (index, fraction) in FRACTIONS.into_iter().enumerate() {
            let mut source = pose(small, index as i64 + 2, 0, common_angle);
            assert!(connections::compatible(
                &source,
                small,
                &small.attach_points[si],
                &target,
                beam,
                &beam.attach_points[ti]
            ));
            let a = part_world_attach(&source, &small.attach_points[si]);
            align(&mut source, a, point(target_surface, fraction));
            assert!(
                parts.iter().all(|other| !intersects(
                    &source,
                    small,
                    other,
                    catalog.get(&other.part_type).unwrap()
                )),
                "同边样本实体不应重叠"
            );
            parts.push(source);
        }
        let mut ship = Ship {
            parts,
            ..Ship::default()
        };
        let before = ship.clone();
        let mut history = EditorHistory::with_limit(1);
        history.execute_with_catalog(
            &mut ship,
            catalog,
            EditorCommand::Batch(vec![command(small, 2, si, ti), command(small, 3, si, ti)]),
        )?;
        assert_eq!(ship.connections.len(), 2);
        verify_endpoints(&ship, catalog);
        let first = connections::positions(&ship, catalog, &ship.connections[0])
            .unwrap()
            .0;
        let second = connections::positions(&ship, catalog, &ship.connections[1])
            .unwrap()
            .0;
        assert!(first.distance(second) > EPSILON);
        let after = ship.clone();
        assert!(history.undo(&mut ship));
        assert_eq!(ship, before);
        assert!(!history.can_undo());
        assert!(history.redo(&mut ship));
        assert_eq!(ship, after);
        assert_eq!(ship_from_xml(&ship_to_xml(&ship)?)?, ship);

        // 故意载入重叠的第三个实例，只检验连接占用；它不能被当作合法几何样本提交。
        let mut duplicate = ship.parts[1].clone();
        duplicate.id = 4;
        assert!(intersects(&duplicate, small, &ship.parts[1], small));
        ship.parts.push(duplicate.clone());
        let candidate = connections::candidates(&duplicate, small, &target, beam, EPSILON)
            .into_iter()
            .find(|c| c.source_index == si && c.target_index == ti)
            .unwrap();
        assert!(!connections::available(
            &ship, catalog, &duplicate, small, &target, beam, &candidate, None
        ));
        let before_failed = ship.clone();
        let mut failed_history = EditorHistory::with_limit(1);
        failed_history.execute(&mut ship, EditorCommand::SetActive(4, true))?;
        assert!(failed_history.undo(&mut ship));
        assert!(
            matches!(failed_history.execute_with_catalog(&mut ship, catalog,
            command(small, 4, si, ti)), Err(CommandError::InvalidConnection(message))
                if message.contains("占用"))
        );
        assert_eq!(ship, before_failed);
        assert!(failed_history.can_redo());
        assert!(!failed_history.can_undo());
    }
    Ok(COMMON_ANGLES.len())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = load_catalog(std::env::args().nth(1).ok_or("请提供原版部件目录路径")?)?;
    let mut coverage = Coverage::default();
    for (source_kind, st) in catalog.types.iter().enumerate() {
        for (target_kind, tt) in catalog.types.iter().enumerate() {
            for common_angle in COMMON_ANGLES {
                for so in 0..16 {
                    // 目标侧只抽样原姿态及旋转 90° + X 镜像；不重复整个笛卡尔姿态积。
                    for to in [0, 5] {
                        let source = pose(st, 2, so, common_angle);
                        let target = pose(tt, 1, to, common_angle);
                        for (si, sa) in st.attach_points.iter().enumerate() {
                            for (ti, ta) in tt.attach_points.iter().enumerate() {
                                let a = connections::segment(&source, st, sa);
                                let b = connections::segment(&target, tt, ta);
                                if a.0.distance(a.1) < EPSILON && b.0.distance(b.1) < EPSILON {
                                    continue;
                                }
                                if !connections::compatible(&source, st, sa, &target, tt, ta) {
                                    continue;
                                }
                                coverage.compatible += 1;
                                coverage.surface_pairs.insert((source_kind, target_kind));
                                for sf in FRACTIONS {
                                    for tf in FRACTIONS {
                                        let mut placed = source.clone();
                                        align(&mut placed, point(a, sf), point(b, tf));
                                        if part_world_attach(&placed, sa)
                                            .distance(part_world_attach(&target, ta))
                                            < EPSILON
                                        {
                                            coverage.centered += 1;
                                            continue;
                                        }
                                        if intersects(&placed, st, &target, tt) {
                                            coverage.collision += 1;
                                            continue;
                                        }
                                        let xml = !coverage
                                            .xml_pairs
                                            .contains(&(source_kind, target_kind));
                                        verify_pair(&catalog, placed, target.clone(), si, ti, xml)?;
                                        coverage.xml_pairs.insert((source_kind, target_kind));
                                        coverage.committed += 1;
                                        if common_angle != 0.0 {
                                            coverage.oblique += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(
        coverage.committed > 0 && coverage.oblique > 0,
        "没有覆盖合法非中心及任意角度样本"
    );
    assert_eq!(
        coverage.compatible * FRACTIONS.len().pow(2),
        coverage.centered + coverage.collision + coverage.committed
    );
    let occupancy = verify_occupancy(&catalog)?;
    println!(
        "沿边非中心验收通过：{} 种部件、{} 种有序部件对遍历，{} 种含兼容连接面的部件对，{} 个兼容面姿态 × 4 个分数对；{} 个意外中心重合样本排除，{} 个实际轮廓碰撞落点过滤（未提交），{} 次合法 ConnectParts/撤销/重做，其中 {} 次共同旋转 0.37 弧度；{} 种合法部件对 XML 往返；{occupancy} 个真实长梁同边双落点/重复占用拒绝样本通过。",
        catalog.types.len(),
        catalog.types.len().pow(2),
        coverage.surface_pairs.len(),
        coverage.compatible,
        coverage.centered,
        coverage.collision,
        coverage.committed,
        coverage.oblique,
        coverage.xml_pairs.len()
    );
    println!(
        "覆盖限制：双方沿段参数仅取 0.2/0.8，源姿态取四向旋转和双轴镜像 16 组合，目标仅取 0/5 两姿态，共同角度仅取 0/0.37；零长度连接点会出现重复采样。按现有核心碰撞策略处理 ignoreEditorIntersections；验证载入姿态的几何，不代表禁转部件可以执行旋转命令。不穷尽连续接触位置、角度、全部多部件碰撞或所有共享连接组组合。Dock 接触由 segment/closest_points 检查，不要求其中心渲染线两端重合。"
    );
    Ok(())
}
