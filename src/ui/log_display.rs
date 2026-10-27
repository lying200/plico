//! Accepted Log page. Identity, scope and pagination are published together.
//! Rebased 4aec352: CommitId binds hash/root; VisiblePack binds graph/filter/more.
use plico::{
    git::{Commit, Repository, Snapshot},
    graph::{Layout, Row},
    graph_print::Model,
    history,
    project_log::{self, CommitId, Entry, Query, Scope, navigation},
};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Display {
    commits: Arc<Vec<Commit>>,
    identities: Arc<Vec<CommitId>>,
    scope: Option<Arc<Scope>>,
    project: Option<Box<navigation::Window>>,
    project_rows: bool,
    rows: Arc<Vec<Row>>,
    model: Option<Arc<Model>>,
    offset: usize,
    start: Layout,
    end: Layout,
    more: bool,
}
impl Display {
    fn entries(&mut self, entries: Vec<Entry>) {
        let (ids, commits) = entries
            .into_iter()
            .map(|e| (e.identity(), e.commit))
            .unzip();
        self.identities = Arc::new(ids);
        self.commits = Arc::new(commits);
    }
    pub fn single(
        query: Query,
        mut window: history::Window,
        offset: usize,
        model: Option<Arc<Model>>,
    ) -> (Self, Snapshot) {
        let snapshot = &mut window.snapshot;
        let root = Arc::new(query.repository.path.clone());
        let scope = Arc::new(Scope {
            roots: vec![project_log::LoadedRoot {
                query,
                tips: snapshot.tips.clone(),
                current_branch: snapshot.current_branch.clone(),
                ref_titles: snapshot.ref_titles.clone(),
            }],
            errors: vec![],
        });
        let mut display = Self {
            scope: Some(scope),
            rows: Arc::new(window.rows),
            model,
            offset,
            start: window.start,
            end: window.end,
            more: snapshot.more,
            ..Default::default()
        };
        display.entries(
            std::mem::take(&mut snapshot.commits)
                .into_iter()
                .map(|commit| Entry {
                    root: root.clone(),
                    commit,
                    display_parents: None,
                })
                .collect(),
        );
        (display, window.snapshot)
    }
    pub fn project(mut page: navigation::Window, model: Option<Arc<Model>>) -> Self {
        let mut display = Self {
            scope: Some(page.read.scope.clone()),
            rows: Arc::new(std::mem::take(&mut page.rows)),
            model,
            offset: page.offset,
            project_rows: true,
            more: page.read.page.more,
            ..Default::default()
        };
        display.entries(std::mem::take(&mut page.read.page.entries));
        display.project = Some(Box::new(page));
        display
    }
    pub fn commits(&self) -> &Arc<Vec<Commit>> {
        &self.commits
    }
    pub fn ids(&self) -> &Arc<Vec<CommitId>> {
        &self.identities
    }
    pub fn sources(&self) -> Option<&Arc<Vec<CommitId>>> {
        self.project_rows.then_some(&self.identities)
    }
    pub fn repository(&self, index: usize) -> Option<Repository> {
        self.identities
            .get(index)
            .map(|id| Repository::new(id.root.as_ref().clone()))
    }
    pub fn scope(&self) -> Option<Arc<Scope>> {
        self.scope.clone()
    }
    pub fn project_window(&self) -> Option<&navigation::Window> {
        self.project.as_deref()
    }
    pub fn rows(&self) -> &Arc<Vec<Row>> {
        &self.rows
    }
    pub fn model(&self) -> &Option<Arc<Model>> {
        &self.model
    }
    pub fn offset(&self) -> usize {
        self.offset
    }
    pub fn start(&self) -> &Layout {
        &self.start
    }
    pub fn end(&self) -> &Layout {
        &self.end
    }
    pub fn more(&self) -> bool {
        self.more
    }
    /// Only geometry changes; retain the accepted page's identities and query.
    /// A projection from a replaced page must not overwrite current geometry.
    pub fn reproject(&mut self, expected: &Arc<Model>, rows: Vec<Row>) -> bool {
        if !self
            .model
            .as_ref()
            .is_some_and(|m| Arc::ptr_eq(m, expected))
        {
            return false;
        }
        self.rows = Arc::new(rows);
        true
    }
    #[cfg(all(test, feature = "desktop-tests"))]
    pub fn fixture(entries: Vec<Entry>, scope: Option<Arc<Scope>>) -> Self {
        let mut value = Self {
            scope,
            project_rows: true,
            ..Default::default()
        };
        value.rows = Arc::new(
            project_log::Page {
                entries: entries
                    .iter()
                    .map(|e| Entry {
                        root: e.root.clone(),
                        commit: e.commit.clone(),
                        display_parents: e.display_parents.clone(),
                    })
                    .collect(),
                more: false,
            }
            .layout(&Default::default())
            .unwrap(),
        );
        value.entries(entries);
        value
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
impl Display {
    pub fn fixture_single(root: &std::path::Path, commits: Vec<Commit>) -> Self {
        Self::single(
            Query {
                repository: Repository::new(root),
                branches: vec![],
                filter: None,
            },
            fixture_window(commits),
            0,
            None,
        )
        .0
    }
    pub fn fixture_identified(&self, ids: Vec<CommitId>) -> Self {
        Self::fixture(
            ids.into_iter()
                .filter_map(|id| {
                    self.commits
                        .iter()
                        .find(|c| c.id == id.hash)
                        .cloned()
                        .map(|commit| Entry {
                            root: id.root,
                            commit,
                            display_parents: None,
                        })
                })
                .collect(),
            self.scope(),
        )
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
fn fixture_window(commits: Vec<Commit>) -> history::Window {
    let snapshot = Snapshot {
        head_id: None,
        included_in_head: Arc::default(),
        current_branch: None,
        ref_titles: Arc::default(),
        tracked_branches: Arc::default(),
        commits,
        branches: vec![],
        tags: vec![],
        tips: vec![],
        more: false,
        shallow: false,
    };
    let rows = plico::graph::layout(&snapshot.commits);
    history::Window {
        snapshot,
        rows,
        projected_parents: None,
        start: Layout::default(),
        end: Layout::default(),
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    use super::*;
    fn commit(id: &str) -> Commit {
        Commit {
            id: id.into(),
            parents: vec![],
            shallow_boundary: false,
            committer_timestamp: 0,
            author: String::new(),
            date: String::new(),
            subject: id.into(),
            refs: vec![],
        }
    }
    #[test]
    fn accepted_page_binds_identity_scope_and_projection_without_old_metadata() {
        let mut query = Query {
            repository: Repository::new("parent"),
            branches: vec!["topic".into()],
            filter: Some(plico::git::LogFilter {
                text: "filter".into(),
                ..Default::default()
            }),
        };
        let mut window = fixture_window(vec![commit("same-hash")]);
        window.snapshot.tips = vec!["fixed-tip".into()];
        window.snapshot.more = true;
        let old_model = Arc::new(Model::new(&window.rows, &Default::default()).unwrap());
        let (first, primary) = Display::single(query.clone(), window, 100, Some(old_model.clone()));
        assert!(
            primary.commits.is_empty(),
            "do not retain duplicate metadata in primary snapshot"
        );
        assert_eq!(first.offset(), 100);
        assert!(first.more());
        assert_eq!(
            first.repository(0).unwrap().path,
            std::path::PathBuf::from("parent")
        );
        let first_ids = first.ids().clone();
        let first_scope = first.scope().unwrap();
        query.repository = Repository::new("child");
        query.filter = None;
        let window = fixture_window(vec![commit("same-hash")]);
        let new_model = Arc::new(Model::new(&window.rows, &Default::default()).unwrap());
        let (mut second, _) = Display::single(query, window, 0, Some(new_model.clone()));
        assert_ne!(first_ids[0], second.ids()[0]);
        assert_eq!(first_scope.roots[0].tips, ["fixed-tip"]);
        assert_eq!(
            first_scope.roots[0].query.filter.as_ref().unwrap().text,
            "filter"
        );
        assert!(second.scope().unwrap().roots[0].query.filter.is_none());
        assert!(!second.more());
        assert!(!second.reproject(&old_model, vec![]));
        assert_eq!(second.rows().len(), 1);
        assert!(second.reproject(
            &new_model,
            new_model.project(true, &Default::default()).unwrap()
        ));
        assert_eq!(second.ids()[0].hash, "same-hash");
        assert_eq!(
            second.repository(0).unwrap().path,
            std::path::PathBuf::from("child")
        );
        assert!(second.repository(1).is_none());
    }
}
