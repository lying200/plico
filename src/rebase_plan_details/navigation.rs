//! Visible-tree keyboard navigation; identities survive expansion and grouping.
use super::*;
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Key {
    Folder(PathBuf),
    File(usize),
}
impl Key {
    pub(super) fn matches(&self, row: &Row) -> bool {
        match (self, row) {
            (Self::Folder(a), Row::Folder { path: b, .. }) => a == b,
            (Self::File(a), Row::File { index: b, .. }) => a == b,
            _ => false,
        }
    }
    pub(super) fn of(row: &Row) -> Self {
        match row {
            Row::Folder { path, .. } => Self::Folder(path.clone()),
            Row::File { index, .. } => Self::File(*index),
        }
    }
}
fn depth(row: &Row) -> usize {
    match row {
        Row::Folder { depth, .. } | Row::File { depth, .. } => *depth,
    }
}
impl Details {
    pub(super) fn focused_row(&self) -> Option<usize> {
        self.cursor
            .as_ref()
            .and_then(|key| self.rows.iter().position(|row| key.matches(row)))
    }
    pub(super) fn choose_row(&mut self, index: usize) {
        self.select_row(index, false, false);
    }
    pub(super) fn select_row(&mut self, index: usize, control: bool, shift: bool) {
        let Some(row) = self.rows.get(index) else {
            return;
        };
        let key = Key::of(row);
        let anchor = self
            .anchor
            .as_ref()
            .and_then(|key| self.rows.iter().position(|row| key.matches(row)));
        if let Some(anchor) = anchor.filter(|_| shift) {
            self.selection = self.rows[index.min(anchor)..=index.max(anchor)]
                .iter()
                .map(Key::of)
                .collect();
        } else {
            if !control {
                self.selection.clear();
            }
            if !self.selection.remove(&key) {
                self.selection.insert(key.clone());
            }
            self.anchor = Some(key.clone());
        }
        self.cursor = Some(key);
        self.selected = match row {
            Row::File { index, .. } => Some(*index),
            _ => None,
        };
        self.scroll.scroll_to_item(index, ScrollStrategy::Top);
    }
    pub(super) fn selected_files(&self) -> Vec<usize> {
        let Some(data) = &self.data else {
            return vec![];
        };
        data.files
            .iter()
            .enumerate()
            .filter_map(|(index, file)| {
                let included = self.selection.contains(&Key::File(index))
                    || file
                        .after
                        .as_ref()
                        .or(file.before.as_ref())
                        .is_some_and(|r| {
                            r.path
                                .ancestors()
                                .skip(1)
                                .any(|p| self.selection.contains(&Key::Folder(p.to_path_buf())))
                        });
                included.then_some(index)
            })
            .collect()
    }
    pub(super) fn set_grouped(&mut self, grouped: bool) {
        let files = self.selected_files();
        self.selection = files.iter().copied().map(Key::File).collect();
        if self
            .cursor
            .as_ref()
            .is_some_and(|key| matches!(key, Key::Folder(_)))
        {
            self.cursor = files.first().copied().map(Key::File);
            self.anchor = self.cursor.clone();
        }
        if grouped {
            if let Some(data) = &self.data {
                for index in files {
                    if let Some(r) = data.files[index]
                        .after
                        .as_ref()
                        .or(data.files[index].before.as_ref())
                    {
                        for p in r.path.ancestors().skip(1) {
                            self.collapsed.remove(p);
                        }
                    }
                }
            }
        }
        self.grouped = grouped;
        self.rebuild();
    }
    pub(super) fn restore_cursor(&mut self) {
        let visible: HashSet<_> = self.rows.iter().map(Key::of).collect();
        self.selection.retain(|key| visible.contains(key));
        if let Some(index) = self.focused_row() {
            self.selected = match &self.rows[index] {
                Row::File { index, .. } => Some(*index),
                _ => None,
            };
            self.scroll.scroll_to_item(index, ScrollStrategy::Top);
        } else {
            self.cursor = None;
            self.selected = None;
        }
        if self
            .anchor
            .as_ref()
            .is_some_and(|key| !visible.contains(key))
        {
            self.anchor = self.cursor.clone();
        }
    }
    pub(super) fn extend_selection(&mut self, key: &str) -> bool {
        if self.rows.is_empty() {
            return false;
        }
        let current = self.focused_row();
        let target = match key {
            "home" => 0,
            "end" => self.rows.len() - 1,
            "up" => current.unwrap_or(0).saturating_sub(1),
            "down" => current.map_or(0, |i| (i + 1).min(self.rows.len() - 1)),
            _ => return false,
        };
        self.select_row(target, false, true);
        true
    }
    pub(super) fn navigate(&mut self, key: &str) -> bool {
        if !matches!(key, "up" | "down" | "left" | "right" | "home" | "end") {
            return false;
        }
        if self.rows.is_empty() {
            return true;
        }
        let current = self.focused_row();
        let i = current.unwrap_or(0);
        if current.is_none() && matches!(key, "left" | "right") {
            self.choose_row(0);
            return true;
        }
        let target = match key {
            "home" => 0,
            "end" => self.rows.len() - 1,
            "up" => i.saturating_sub(1),
            "down" => current.map_or(0, |i| (i + 1).min(self.rows.len() - 1)),
            "left" => {
                if let Row::Folder {
                    path,
                    expanded: true,
                    ..
                } = &self.rows[i]
                {
                    self.collapsed.insert(path.clone());
                    self.rebuild();
                    return true;
                }
                (0..i)
                    .rev()
                    .find(|&p| depth(&self.rows[p]) < depth(&self.rows[i]))
                    .unwrap_or(i)
            }
            "right" => {
                if let Row::Folder { path, expanded, .. } = &self.rows[i] {
                    if !expanded {
                        self.collapsed.remove(path);
                        self.rebuild();
                        return true;
                    }
                }
                (i + 1).min(self.rows.len() - 1)
            }
            _ => unreachable!(),
        };
        self.choose_row(target);
        true
    }
}
