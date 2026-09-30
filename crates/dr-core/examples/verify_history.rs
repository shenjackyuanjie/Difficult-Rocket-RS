//! 真实船体连续编辑及撤销重做的快照预算验收，不写入源文档。
use dr_core::{EditorCommand, EditorHistory, load_ship, ship_from_xml, ship_to_xml};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("请提供船体路径")?;
    let mut ship = load_ship(path)?;
    let original = ship.clone();
    let key = ship.keyed_parts().next().ok_or("船体为空")?.0;
    let mut history = EditorHistory::default();
    let start = Instant::now();
    for _ in 0..400 {
        let active = !ship.part_at(key).unwrap().active;
        history.execute(&mut ship, EditorCommand::SetActive(key.id, active).at(key))?;
    }
    assert_eq!(ship, original);
    let edit_ms = start.elapsed().as_secs_f64() * 1000.0;
    let retained = history.undo_len();
    let bytes = history.reserved_bytes();
    assert!(bytes <= 64 * 1024 * 1024 || retained == 1);
    assert!(retained <= 256);
    let start = Instant::now();
    for step in 1..=retained {
        assert!(history.undo(&mut ship));
        assert_eq!(
            ship.part_at(key).unwrap().active,
            original.part_at(key).unwrap().active ^ (step % 2 == 1)
        );
    }
    assert!(!history.undo(&mut ship));
    for _ in 0..retained {
        assert!(history.redo(&mut ship));
    }
    assert!(!history.redo(&mut ship));
    let navigation_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(ship, original);
    assert_eq!(history.reserved_bytes(), bytes);
    assert_eq!(ship_from_xml(&ship_to_xml(&ship)?)?, original);
    println!(
        "{{\"parts\":{},\"edits\":400,\"retained_steps\":{retained},\"reserved_bytes\":{bytes},\"edit_ms\":{edit_ms:.3},\"undo_redo_ms\":{navigation_ms:.3}}}",
        ship.all_parts().count()
    );
    Ok(())
}
