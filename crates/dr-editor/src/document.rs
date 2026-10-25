//! 编辑文档、保存点与历史事务；不依赖窗口输入。
use crate::audio;
use bevy::prelude::*;
use dr_core::{
    EditorCommand, EditorHistory, PartCatalog, PartKey, PartKind, Ship, load_catalog, load_ship,
};
use std::collections::BTreeSet;

#[derive(Resource)]
pub struct EditorDocument {
    /// 自由几何与原有连接保留策略；不切换吸附。
    pub free_mode: bool,
    /// 对齐与新自动连接；关闭时保留已有连接。
    pub snap_enabled: bool,
    pub revision: u64,
    pub ship: Ship,
    pub catalog: PartCatalog,
    pub selected: Option<PartKey>,
    pub selection: BTreeSet<PartKey>,
    pub dirty: bool,
    pub history: EditorHistory,
    pub saved_ship: Ship,
    pub status: String,
    /// 事务结果待播音效；SfxPlugin 每帧取走，文档本身不依赖播放后端。
    pub sfx: Vec<audio::SfxEvent>,
}

impl EditorDocument {
    pub fn selected_keys(&self) -> Vec<PartKey> {
        self.selection
            .iter()
            .copied()
            .chain(self.selected)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn is_selected(&self, key: PartKey) -> bool {
        self.selected == Some(key) || self.selection.contains(&key)
    }
    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.selection.clear();
    }
    pub fn select_only(&mut self, key: Option<PartKey>) {
        self.clear_selection();
        self.selected = key;
    }
    pub fn refresh(&mut self) {
        self.revision = self.revision.wrapping_add(1);
        self.dirty = self.ship != self.saved_ship;
        if self
            .selected
            .is_some_and(|id| self.ship.part_at(id).is_none())
        {
            self.selected = None;
        }
    }
    pub fn undo(&mut self) -> bool {
        let changed = self.history.undo(&mut self.ship);
        if changed {
            self.sfx.push(audio::SfxEvent::Undo);
            self.clear_selection();
        }
        self.refresh();
        changed
    }
    pub fn redo(&mut self) -> bool {
        let changed = self.history.redo(&mut self.ship);
        if changed {
            self.sfx.push(audio::SfxEvent::Redo);
            self.clear_selection();
        }
        self.refresh();
        changed
    }
    pub fn execute(&mut self, command: EditorCommand) -> bool {
        let command = self.edit_command(command);
        let sfx = audio::command_sfx(&command);
        let before: Vec<_> = self.ship.keyed_parts().map(|(key, _)| key).collect();
        match self
            .history
            .execute_with_catalog(&mut self.ship, &self.catalog, command)
        {
            Ok(()) => {
                if let Some(sfx) = sfx {
                    self.sfx.push(sfx);
                }
                if !before
                    .into_iter()
                    .eq(self.ship.keyed_parts().map(|(key, _)| key))
                {
                    self.clear_selection();
                }
                self.refresh();
                self.status.clear();
                true
            }
            Err(error) => {
                self.sfx.push(audio::SfxEvent::Reject);
                self.status = error.to_string();
                false
            }
        }
    }

    pub fn edit_command(&self, command: EditorCommand) -> EditorCommand {
        if self.free_mode {
            EditorCommand::FreeEdit(Box::new(command))
        } else {
            command
        }
    }
}

pub fn load_document(
    ship_path: Option<&str>,
    catalog_path: &str,
) -> anyhow::Result<EditorDocument> {
    let catalog = load_catalog(catalog_path)?;
    let ship = ship_path
        .map(load_ship)
        .transpose()?
        .unwrap_or_else(|| new_ship(&catalog));
    Ok(EditorDocument {
        free_mode: false,
        snap_enabled: true,
        revision: 0,
        saved_ship: ship.clone(),
        status: String::new(),
        ship,
        catalog,
        selected: None,
        selection: default(),
        dirty: false,
        history: EditorHistory::with_limit(256),
        sfx: Vec::new(),
    })
}

/// 与原版 assets/builtin/none_ship.xml 一致，新船体自带隐藏的驾驶舱。
pub fn new_ship(catalog: &PartCatalog) -> Ship {
    let Some(pod) = catalog.types.iter().find(|kind| kind.kind == PartKind::Pod) else {
        return Ship::default();
    };
    Ship {
        touching_ground: false,
        parts: vec![pod.instantiate(1, (0.0, pod.half_extents().1))],
        ..Ship::default()
    }
}

#[cfg(test)]
mod tests;
