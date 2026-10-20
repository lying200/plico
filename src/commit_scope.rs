//! Shared Commit-tree and local-Diff inclusion. Never inferred from review focus.
use gpui_kit::*;
use plico::{
    commit_inclusion::{Identity, Inclusion},
    partial_selection::{File, Selection},
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
#[derive(Default)]
pub struct Scope {
    pub inclusion: Inclusion,
    head: Option<String>,
    pub partial: HashMap<PathBuf, File>,
}
impl Scope {
    pub fn replace(&mut self, snapshot: &plico::commit_changes::Snapshot, cx: &mut Context<Self>) {
        if self.head != snapshot.head { self.partial.clear(); }
        self.head=snapshot.head.clone();
        self.inclusion.replace(snapshot);
        self.partial.retain(|path, _| snapshot.files.iter().any(|f| Identity::of(f).path == *path));
        cx.notify();
    }
    pub fn set_scope(&mut self, ids: &HashSet<Identity>, checked: bool, cx: &mut Context<Self>) {
        self.inclusion.set_scope(ids, checked);
        for id in ids {
            self.partial.remove(&id.path);
        }
        cx.notify();
    }
    pub fn counts(&self, ids: &HashSet<Identity>) -> (usize, usize) {
        let (n, t) = self.inclusion.scope_counts(ids);
        // A partially selected file contributes both included and excluded state.
        let mixed = ids
            .iter()
            .filter(|id| {
                self.partial.get(&id.path).is_some_and(|f| {
                    let (a, b) = f.selection.counts();
                    a > 0 && a < b
                })
            })
            .count();
        (n, t + mixed)
    }
    pub fn selection(&self, id: &Identity, template: &Selection) -> Selection {
        self.partial
            .get(&id.path)
            .filter(|f| {
                f.selection.original == template.original && f.selection.current == template.current
            })
            .map(|f| f.selection.clone())
            .unwrap_or_else(|| {
                let mut s = template.clone();
                s.set_all(self.inclusion.included(id));
                s
            })
    }
    pub fn change(&mut self, id: &Identity, selection: Selection, cx: &mut Context<Self>) {
        let (n, t) = selection.counts();
        self.inclusion.set(id, n > 0);
        let split=selection.included.iter().any(|s|matches!(s,plico::partial_selection::Included::Lines{..}));
        if !split && (n == 0 || n == t) {
            self.partial.remove(&id.path);
        } else {
            self.partial.insert(
                id.path.clone(),
                File {
                    path: id.path.clone(),
                    selection,
                },
            );
        }
        cx.notify();
    }
    pub fn invalidate(
        &mut self,
        path: &Path,
        original: &str,
        current: &str,
        cx: &mut Context<Self>,
    ) {
        if self.partial.get(path).is_some_and(|f| {
            f.selection.original.as_ref() != original || f.selection.current.as_ref() != current
        }) {
            // Never silently turn a stale partial selection into whole-file inclusion.
            if let Some(file) = self.partial.remove(path) {
                let id = Identity {
                    kind: plico::commit_inclusion::Kind::Tracked,
                    path: file.path.clone(),
                    before: Some(file.path.clone()),
                    after: Some(file.path),
                };
                self.inclusion.set(&id, false);
            }
            cx.notify();
        }
    }
}
#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::{Identity, Scope, Selection};
    use gpui_kit::AppContext;
    #[gpui::test]
    fn partial_tree_state_shared_and_stale_text_is_excluded(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            let file = plico::commit_changes::File::Tracked(plico::git::WorkingTreeChange {
                path: "f".into(),
                old_path: None,
                status: "M".into(),
            });
            let id = Identity::of(&file);
            let scope = cx.new(|_| Scope::default());
            scope.update(cx, |s, cx| {
                s.replace(
                    &plico::commit_changes::Snapshot {
                        head: Some("a".repeat(40)),
                        files: vec![file],
                        review_versions: vec![],
                    },
                    cx,
                )
            });
            let mut selected = Selection::new("a\nkeep\nb\n".into(), "A\nkeep\nB\n".into(), false);
            selected.toggle_block(0);
            scope.update(cx, |s, cx| s.change(&id, selected, cx));
            assert_eq!(scope.read(cx).counts(&[id.clone()].into()), (1, 2));
            assert_eq!(scope.read(cx).inclusion.count(), 1);
            let mut none=scope.read(cx).partial[&id.path].selection.clone();
            none.set_lines(true,0..1,false);
            scope.update(cx,|s,cx|s.change(&id,none,cx));
            assert_eq!(scope.read(cx).inclusion.count(),0);
            assert!(scope.read(cx).partial.contains_key(&id.path),"unchecking the last line must keep split-line controls");

            scope.update(cx, |s, cx| {
                s.invalidate(&id.path, "a\nkeep\nb\n", "new text", cx)
            });
            assert_eq!(scope.read(cx).inclusion.count(), 0);
            assert!(scope.read(cx).partial.is_empty());
        });
    }
}
