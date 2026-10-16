use super::{Draft, PartKey, StagingState};

/// 只记录可提交的属性；目标选择、预览与展开状态属于界面导航。
#[derive(Clone, PartialEq)]
struct Snapshot {
    name: String,
    fuel: String,
    throttle: String,
    current_stage: String,
    active: bool,
    staging: Option<StagingState>,
}

impl Snapshot {
    fn new(draft: &Draft) -> Self {
        Self {
            name: draft.name.clone(),
            fuel: draft.fuel.clone(),
            throttle: draft.throttle.clone(),
            current_stage: draft.current_stage.clone(),
            active: draft.active,
            staging: draft.staging.clone(),
        }
    }

    fn matches(&self, draft: &Draft) -> bool {
        self.name == draft.name
            && self.fuel == draft.fuel
            && self.throttle == draft.throttle
            && self.current_stage == draft.current_stage
            && self.active == draft.active
            && self.staging == draft.staging
    }

    fn restore(&self, draft: &mut Draft) {
        draft.name.clone_from(&self.name);
        draft.fuel.clone_from(&self.fuel);
        draft.throttle.clone_from(&self.throttle);
        draft.current_stage.clone_from(&self.current_stage);
        draft.active = self.active;
        draft.staging.clone_from(&self.staging);
    }
}

#[derive(Default)]
pub(super) struct DraftHistory {
    current: Option<(PartKey, Snapshot)>,
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
}

impl DraftHistory {
    pub(super) fn changed(draft: &Draft) -> bool {
        !Snapshot::new(&Draft::new(draft.key, &draft.original)).matches(draft)
    }

    fn pending(&self, draft: &Draft) -> bool {
        match &self.current {
            Some((key, snapshot)) if *key == draft.key => !snapshot.matches(draft),
            _ => Self::changed(draft),
        }
    }

    pub(super) fn can_undo(&self, draft: &Draft) -> bool {
        self.pending(draft)
            || (self
                .current
                .as_ref()
                .is_some_and(|(key, _)| *key == draft.key)
                && !self.undo.is_empty())
    }

    pub(super) fn can_redo(&self, draft: &Draft) -> bool {
        !self.pending(draft)
            && self
                .current
                .as_ref()
                .is_some_and(|(key, _)| *key == draft.key)
            && !self.redo.is_empty()
    }

    pub(super) fn observe(&mut self, draft: Option<&Draft>) {
        let Some(draft) = draft else {
            *self = Self::default();
            return;
        };
        if self
            .current
            .as_ref()
            .is_none_or(|(key, _)| *key != draft.key)
        {
            *self = Self {
                current: Some((
                    draft.key,
                    Snapshot::new(&Draft::new(draft.key, &draft.original)),
                )),
                ..Self::default()
            };
        }
        if self.pending(draft) {
            let (_, previous) = self
                .current
                .replace((draft.key, Snapshot::new(draft)))
                .unwrap();
            self.undo.push(previous);
            self.redo.clear();
        }
    }

    pub(super) fn undo(&mut self, draft: &mut Draft) -> bool {
        self.observe(Some(draft));
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(Snapshot::new(draft));
        previous.restore(draft);
        self.current = Some((draft.key, previous));
        true
    }

    pub(super) fn redo(&mut self, draft: &mut Draft) -> bool {
        self.observe(Some(draft));
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(Snapshot::new(draft));
        next.restore(draft);
        self.current = Some((draft.key, next));
        true
    }
}
