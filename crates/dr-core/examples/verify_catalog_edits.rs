//! 枚举真实目录中全部部件对、双方四向旋转及双轴镜像，验收连接提交与撤销。
use dr_core::{
    EditorCommand, EditorHistory, LinkKind, Part, PartKey, PartKind, PartType, Ship, connections,
    intersects, load_catalog, part_world_attach, ship_from_xml, ship_to_xml,
};

fn pose(kind: &PartType, id: i64, orientation: usize) -> Part {
    let mut part = kind.instantiate(id, (0.0, 0.0));
    part.editor_angle = (orientation % 4) as i32;
    part.angle = part.editor_angle as f64 * std::f64::consts::FRAC_PI_2;
    part.flip_x = orientation & 4 != 0;
    part.flip_y = orientation & 8 != 0;
    part
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let catalog = load_catalog(std::env::args().nth(1).ok_or("请提供部件目录路径")?)?;
    let mut poses = 0;
    for kind in &catalog.types {
        for orientation in 0..16 {
            let part = pose(kind, 1, orientation);
            let ship = Ship {
                parts: vec![part],
                ..Ship::default()
            };
            assert_eq!(ship_from_xml(&ship_to_xml(&ship)?)?, ship);
            poses += 1;
        }
    }
    let mut compatible = 0;
    let mut collision = 0;
    let mut committed = 0;
    let mut xml_pairs = 0;
    for st in &catalog.types {
        for tt in &catalog.types {
            let mut xml_checked = false;
            for so in 0..16 {
                for to in 0..16 {
                    let source = pose(st, 2, so);
                    let target = pose(tt, 1, to);
                    for (si, sa) in st.attach_points.iter().enumerate() {
                        for (ti, ta) in tt.attach_points.iter().enumerate() {
                            if !connections::compatible(&source, st, sa, &target, tt, ta) {
                                continue;
                            }
                            compatible += 1;
                            let a = part_world_attach(&source, sa);
                            let b = part_world_attach(&target, ta);
                            let mut source = source.clone();
                            source.x += b.x - a.x;
                            source.y += b.y - a.y;
                            let candidates =
                                connections::candidates(&source, st, &target, tt, 1e-5);
                            assert!(
                                candidates
                                    .iter()
                                    .any(|c| c.source_index == si && c.target_index == ti),
                                "候选遗漏 {} {so} → {} {to}: {si} {ti}",
                                st.id,
                                tt.id
                            );
                            if intersects(&source, st, &target, tt) {
                                collision += 1;
                                continue;
                            }
                            let parent = PartKey::new(0, 1, 0);
                            let child = PartKey::new(0, 2, 0);
                            let kind = if sa.dock {
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
                            let mut ship = Ship {
                                parts: vec![target.clone(), source],
                                ..Ship::default()
                            };
                            let before = ship.clone();
                            let mut history = EditorHistory::with_limit(1);
                            history.execute_with_catalog(
                                &mut ship,
                                &catalog,
                                EditorCommand::ConnectParts {
                                    parent,
                                    child,
                                    kind,
                                },
                            )?;
                            assert_eq!(ship.connections.len(), 1);
                            let after = ship.clone();
                            assert!(history.undo(&mut ship));
                            assert_eq!(ship, before);
                            assert!(history.redo(&mut ship));
                            assert_eq!(ship, after);
                            if !xml_checked {
                                assert_eq!(ship_from_xml(&ship_to_xml(&ship)?)?, ship);
                                xml_checked = true;
                                xml_pairs += 1;
                            }
                            committed += 1;
                        }
                    }
                }
            }
        }
    }
    println!(
        "目录验收通过：{} 种部件，{poses} 个姿态 XML 往返，{} 种有序部件对，{compatible} 个兼容连接姿态，{collision} 个实体碰撞落点，{committed} 次连接提交与撤销重做，{xml_pairs} 种可连接部件对 XML 往返",
        catalog.types.len(),
        catalog.types.len().pow(2)
    );
    Ok(())
}
