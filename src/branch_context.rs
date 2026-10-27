//! Lightweight current-branch presentation, independent of history queries.
//! Rebased 4aec352 GitBranchStateProvider / BranchPresentation (Apache-2.0).
use crate::git::{Cancel, Repository};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Context {
    pub branch: Option<String>,
    pub upstream: Option<String>,
}
impl Context {
    pub fn label(&self) -> &str {
        self.branch.as_deref().unwrap_or("!")
    }
}
pub fn read(repo: &Repository, token: &Cancel) -> Result<Context, String> {
    let reference = symbolic_head(repo, token)?;
    let Some(reference) = reference else {
        return Ok(Context {
            branch: None,
            upstream: None,
        });
    };
    let rows = repo.text(
        &[
            "for-each-ref",
            "--format=%(refname)%09%(upstream:short)",
            &reference,
        ],
        token,
    )?;
    let upstream = rows
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .find(|(name, _)| *name == reference)
        .and_then(|(_, upstream)| (!upstream.is_empty()).then(|| upstream.to_owned()));
    Ok(Context {
        branch: Some(reference["refs/heads/".len()..].into()),
        upstream,
    })
}

pub(crate) fn symbolic_head(repo: &Repository, token: &Cancel) -> Result<Option<String>, String> {
    use crate::operation_process::{self, Progress};
    let out = operation_process::run(
        repo.command().args(["symbolic-ref", "--quiet", "HEAD"]),
        token,
        &Progress::default(),
        Some(std::time::Duration::from_secs(30)),
    )
    .map_err(|e| e.to_string())?;
    if out.stopped.is_some() || out.stdout.truncated {
        return Err("HEAD branch query interrupted".into());
    }
    if out.status.code() == Some(1) {
        return Ok(None);
    }
    if !out.status.success() {
        return Err("Cannot determine HEAD branch".into());
    }
    let bytes = out.stdout.bytes();
    let name = std::str::from_utf8(&bytes)
        .map_err(|_| "Invalid HEAD branch")?
        .trim_end_matches(['\r', '\n']);
    if !name.starts_with("refs/heads/") {
        return Err("Unexpected HEAD symbolic reference".into());
    }
    Ok(Some(name.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_handles_unborn_tracking_and_detached_without_history() {
        struct Fixture(std::path::PathBuf);
        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let fixture = Fixture(std::env::temp_dir().join(format!(
                "plico-branch-context-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            )));
        std::fs::create_dir_all(&fixture.0).unwrap();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .current_dir(&fixture.0)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-q", "-b", "main"]);
        let repo = Repository::new(&fixture.0);
        let token = Cancel::default();
        assert_eq!(
            read(&repo, &token).unwrap(),
            Context {
                branch: Some("main".into()),
                upstream: None
            }
        );
        git(&[
            "-c",
            "user.name=Branch",
            "-c",
            "user.email=branch@example.invalid",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-qm",
            "base",
        ]);
        git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
        git(&["config", "remote.origin.url", "./unused-local-remote"]);
        git(&[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ]);
        git(&["config", "branch.main.remote", "origin"]);
        git(&["config", "branch.main.merge", "refs/heads/main"]);
        assert_eq!(
            read(&repo, &token).unwrap().upstream.as_deref(),
            Some("origin/main")
        );
        git(&["checkout", "--detach", "-q"]);
        let detached = read(&repo, &token).unwrap();
        assert_eq!(detached.label(), "!");
        assert_eq!(detached.upstream, None);
        let cancelled = Cancel::default();
        cancelled.cancel();
        assert!(read(&repo, &cancelled).is_err());
        assert!(read(&Repository::new(fixture.0.join("missing")), &token).is_err());
    }
}
