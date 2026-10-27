//! Rebased 4aec352 DvcsUtil.guessRepositoryForOperation: current context,
//! last active editor, then project base directory / first mapped root.
use super::Repositories;
use crate::{git::Repository, recent_projects::identity};
use std::path::Path;

impl Repositories {
    /// Resolve only among validated project members. This is a lexical lookup:
    /// no disk/network queries on the UI thread, and no implicit root discovery.
    /// Callers supply a single focused file/root, not an arbitrary first member
    /// of a multi-root selection. The returned repository retains catalog spelling.
    pub fn operation_repository(
        &self,
        current: Option<&Path>,
        last_editor: Option<&Path>,
        project_base: &Path,
    ) -> Option<&Repository> {
        let containing = |path: &Path| {
            let path = identity(path);
            self.roots
                .iter()
                .filter_map(|repo| {
                    let root = identity(&repo.path);
                    let prefix = root.trim_end_matches('/');
                    (path == root
                        || path
                            .strip_prefix(prefix)
                            .is_some_and(|tail| tail.starts_with('/')))
                    .then_some((root.len(), repo))
                })
                .max_by_key(|(length, _)| *length)
                .map(|(_, repo)| repo)
        };
        current
            .and_then(containing)
            .or_else(|| last_editor.and_then(containing))
            .or_else(|| containing(project_base))
            .or_else(|| self.roots.first())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_context_wins_then_editor_then_project_without_prefix_collisions() {
        let roots = Repositories {
            roots: ["/workspace/project", "/workspace/project/child", "/other"]
                .into_iter()
                .map(Repository::new)
                .collect(),
            ..Default::default()
        };
        let base = Path::new("/workspace/project");
        let child = Path::new("/workspace/project/child/file.txt");
        let other = Path::new("/other/file.txt");
        let pick = |current, editor| {
            roots
                .operation_repository(current, editor, base)
                .unwrap()
                .path
                .clone()
        };
        assert_eq!(pick(Some(child), Some(other)), roots.roots[1].path);
        assert_eq!(pick(Some(base), Some(child)), roots.roots[0].path);
        assert_eq!(pick(None, Some(child)), roots.roots[1].path);
        assert_eq!(
            pick(
                Some(Path::new("/workspace/project-other/file")),
                Some(other)
            ),
            roots.roots[2].path
        );
        assert_eq!(pick(None, None), roots.roots[0].path);
        assert!(
            Repositories::default()
                .operation_repository(Some(child), None, base)
                .is_none()
        );
    }

    #[cfg(windows)]
    #[test]
    fn windows_identity_preserves_the_registered_repository_path() {
        let roots = Repositories {
            roots: vec![
                Repository::new(r"C:\Project"),
                Repository::new(r"C:\Project\Child"),
            ],
            ..Default::default()
        };
        let selected = roots
            .operation_repository(
                Some(Path::new(r"\\?\c:\PROJECT\child\file.txt")),
                None,
                Path::new(r"C:\Project"),
            )
            .unwrap();
        assert_eq!(selected.path, roots.roots[1].path);
    }
}
