//! Behavioral translation of Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd:
//! GitPushSpecParser.kt, GitPushTarget.java, GitPushSupport.java.
//! Copyright JetBrains s.r.o. and contributors, Apache-2.0.

/// First exact match wins over any wildcard; otherwise first wildcard wins.
/// `branch` is a short local branch name. A leading '+' does not authorize force.
pub fn target_ref(branch: &str, specs: &[String]) -> Option<String> {
    let full = format!("refs/heads/{branch}");
    let mut wildcard = None;
    for spec in specs {
        let Some((source, target)) = spec.split_once(':') else {
            continue;
        };
        if target.contains(':') {
            continue;
        }
        let source = source.trim().strip_prefix('+').unwrap_or(source.trim());
        let target = target.trim();
        match (source.find('*'), target.find('*')) {
            (None, None) => {
                if source == "HEAD"
                    || source.strip_prefix("refs/heads/").unwrap_or(source) == branch
                {
                    return Some(target.into());
                }
            }
            (Some(a), Some(b)) if a + 1 == source.len() && b + 1 == target.len() => {
                if let Some(suffix) = full.strip_prefix(&source[..a]) {
                    if wildcard.is_none() {
                        wildcard = Some(target.replace('*', suffix));
                    }
                }
            }
            _ => {}
        }
    }
    wildcard
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Origin {
    PushSpec,
    TrackingBranch,
    Custom,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub remote: String,
    /// Short name for ordinary branches, full reference for special targets.
    pub branch: String,
    pub special: bool,
    pub new_branch: bool,
    pub origin: Origin,
}
impl Target {
    pub fn destination(&self) -> String {
        if self.special {
            self.branch.clone()
        } else {
            format!("refs/heads/{}", self.branch)
        }
    }
    /// GitPushTarget.shouldSetUpstream: commit/tag sources never acquire tracking.
    pub fn should_set_upstream(
        &self,
        branch_source: bool,
        has_tracking: bool,
        requested: bool,
    ) -> bool {
        branch_source && ((self.new_branch && !has_tracking) || requested)
    }
}

/// Pure selection over a repository snapshot. Config reading and branch validation
/// are caller responsibilities; this does not run Git or silently create a ref.
/// Mirrors the attached-branch overload of GitPushSupport.getDefaultTarget.
pub fn default_for_branch(
    branch: &str,
    targets: &crate::pull::Targets,
    specs: &[(String, Vec<String>)],
) -> Option<Target> {
    let remote = targets.default_remote()?;
    let push_ref = specs
        .iter()
        .find(|(name, _)| name == remote)
        .and_then(|(_, specs)| target_ref(branch, specs));
    if let Some(reference) = push_ref {
        let remote_prefix = format!("refs/remotes/{remote}/");
        let short = reference
            .strip_prefix("refs/heads/")
            .or_else(|| reference.strip_prefix(&remote_prefix));
        let special = short.is_none();
        let name = short.unwrap_or(&reference);
        return Some(Target {
            remote: remote.into(),
            branch: name.into(),
            special,
            new_branch: special || !targets.contains(remote, name),
            origin: Origin::PushSpec,
        });
    }
    if let Some(tracked) = &targets.tracked {
        return Some(Target {
            remote: tracked.remote.clone(),
            branch: tracked.branch.clone(),
            special: false,
            new_branch: false,
            origin: Origin::TrackingBranch,
        });
    }
    Some(Target {
        remote: remote.into(),
        branch: branch.into(),
        special: false,
        new_branch: !targets.contains(remote, branch),
        origin: Origin::TrackingBranch,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(branch: &str, specs: &[&str]) -> Option<String> {
        target_ref(
            branch,
            &specs.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        )
    }
    #[test]
    fn upstream_refspec_cases_and_match_precedence() {
        for spec in [
            "HEAD:refs/heads/*",
            "refs/heads/*:refs/for/master",
            "refs/for/*/branch:refs/for/*",
            "refs/heads/*:refs/*/for",
            "refs/for/*:refs/remotes/origin/*",
        ] {
            assert!(parse("master", &[spec]).is_none());
        }
        for spec in [
            "HEAD:refs/for/master",
            "+refs/heads/*:refs/remotes/origin/*",
            "*:refs/remotes/origin/*",
            "*:*",
        ] {
            assert!(parse("master", &[spec]).is_some());
        }
        assert_eq!(
            parse("master", &["+refs/heads/*:refs/remotes/origin/*"]).as_deref(),
            Some("refs/remotes/origin/master")
        );
        let specs = [
            "refs/heads/qa/*:refs/remotes/origin/qa/*",
            "+refs/heads/master:refs/remotes/origin/master",
        ];
        assert_eq!(
            parse("qa/ticket1", &specs).as_deref(),
            Some("refs/remotes/origin/qa/ticket1")
        );
        assert_eq!(
            parse("master", &specs).as_deref(),
            Some("refs/remotes/origin/master")
        );
        assert!(parse("feature", &specs).is_none());
        let specs = [
            "refs/heads/*:refs/heads/pr/*",
            "refs/heads/master:refs/heads/qa/master",
            "refs/tags/*:refs/tags/*",
        ];
        assert_eq!(
            parse("master", &specs).as_deref(),
            Some("refs/heads/qa/master")
        );
        assert_eq!(
            parse("qa/ticket1", &specs).as_deref(),
            Some("refs/heads/pr/qa/ticket1")
        );
        assert!(parse("master", &["ma*:refs/heads/ma*"]).is_none());
        assert!(
            parse(
                "feature",
                &[
                    "+refs/heads/master:refs/remotes/origin/master",
                    "refs/tags/*:refs/tags/*"
                ]
            )
            .is_none()
        );
        assert_eq!(
            parse(
                "main",
                &[
                    "refs/heads/*:refs/heads/first/*",
                    "refs/heads/*:refs/heads/second/*"
                ]
            )
            .as_deref(),
            Some("refs/heads/first/main")
        );
        assert_eq!(
            parse("main", &[" +HEAD : refs/for/review "]).as_deref(),
            Some("refs/for/review")
        );
        assert!(parse("main", &["main", "main:x:y", "refs/heads/**:refs/heads/**"]).is_none());
    }
    #[test]
    fn upstream_target_cases_and_tracking_rules() {
        use crate::pull::{Target as Branch, Targets};
        let mut input = Targets {
            remotes: vec!["origin".into(), "my/remote".into()],
            branches: vec![Branch {
                remote: "origin".into(),
                branch: "master".into(),
            }],
            current_branch: Some("master".into()),
            tracked: None,
        };
        let select = |input: &Targets, specs: &[(&str, &str)]| {
            default_for_branch(
                "master",
                input,
                &specs
                    .iter()
                    .map(|(r, s)| (r.to_string(), vec![s.to_string()]))
                    .collect::<Vec<_>>(),
            )
            .unwrap()
        };
        let target = select(&input, &[]);
        assert!(!target.new_branch);
        assert_eq!(target.destination(), "refs/heads/master");
        for spec in ["HEAD:refs/for/master", "refs/heads/*:refs/for/*"] {
            let target = select(&input, &[("origin", spec)]);
            assert!(target.special);
            assert_eq!(target.destination(), "refs/for/master");
        }
        let target = select(&input, &[("origin", "refs/heads/*:refs/remotes/origin/*")]);
        assert!(!target.special);
        assert!(!target.new_branch);
        assert_eq!(target.branch, "master");
        input.tracked = Some(Branch {
            remote: "my/remote".into(),
            branch: "master".into(),
        });
        let target = select(
            &input,
            &[
                ("origin", "HEAD:refs/for/origin"),
                ("my/remote", "HEAD:refs/for/master"),
            ],
        );
        assert_eq!(target.remote, "my/remote");
        assert!(target.special);
        assert_eq!(target.branch, "refs/for/master");
        assert!(!target.should_set_upstream(true, true, false));
        assert!(target.should_set_upstream(true, true, true));
        assert!(!target.should_set_upstream(false, false, true));
        input.tracked = None;
        input.branches.clear();
        let target = select(&input, &[]);
        assert!(target.new_branch);
        assert!(target.should_set_upstream(true, false, false));
        input.remotes.clear();
        assert!(default_for_branch("master", &input, &[]).is_none());
    }
}

/// Attached-branch Push dialog snapshot. No network access or repository writes.
/// Detached/commit-source selection is a separate path, not an invented branch.
pub struct Snapshot {
    pub source_branch: String,
    pub source_oid: String,
    pub targets: crate::pull::Targets,
    pub default: Option<Target>,
}
impl crate::git::Repository {
    pub fn push_snapshot(&self, token: &crate::git::Cancel) -> Result<Option<Snapshot>, String> {
        let targets = self.pull_targets(token)?;
        let Some(branch) = targets.current_branch.as_deref() else {
            return Ok(None);
        };
        let reference = format!("refs/heads/{branch}");
        let refs = self.text(
            &[
                "for-each-ref",
                "--format=%(refname)%09%(objectname)",
                &reference,
            ],
            token,
        )?;
        let Some(oid) = refs
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .find_map(|(name, oid)| (name == reference).then_some(oid))
        else {
            return Ok(None);
        };
        let mut specs = Vec::new();
        // Only the selected remote's specs influence the upstream default policy.
        if let Some(remote) = targets.default_remote() {
            let key = format!("remote.{remote}.push");
            let values = read_config_values(self, &key, token)?;
            specs.push((remote.to_owned(), values));
        }
        let default = default_for_branch(branch, &targets, &specs);
        let source_branch = branch.to_owned();
        let source_oid = oid.to_owned();
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(Some(Snapshot {
            source_branch,
            source_oid,
            targets,
            default,
        }))
    }
}

fn read_config_values(
    repo: &crate::git::Repository,
    key: &str,
    token: &crate::git::Cancel,
) -> Result<Vec<String>, String> {
    use crate::operation_process::{self, Progress};
    let outcome = operation_process::run(
        repo.command().args(["config", "--null", "--get-all", key]),
        token,
        &Progress::default(),
        Some(std::time::Duration::from_secs(30)),
    )
    .map_err(|e| e.to_string())?;
    if outcome.stopped.is_some() || token.cancelled() {
        return Err("Push configuration read interrupted".into());
    }
    if outcome.stdout.truncated {
        return Err("Push configuration exceeds the 64 KiB read limit".into());
    }
    let bytes = outcome.stdout.bytes();
    if outcome.status.code() == Some(1) && bytes.is_empty() {
        return Ok(Vec::new());
    }
    if !outcome.status.success() {
        return Err("Could not read push configuration".into());
    }
    let text = String::from_utf8(bytes).map_err(|_| "Push configuration is not UTF-8")?;
    if !text.ends_with('\0') {
        return Err("Incomplete push configuration record".into());
    }
    Ok(text[..text.len() - 1]
        .split('\0')
        .map(str::to_owned)
        .collect())
}

/// `capped` means the upstream new-target ceiling was reached, not proof that
/// additional commits exist. `more` requests another page within that ceiling.
pub struct Outgoing {
    pub references: std::sync::Arc<str>,
    pub commits: Vec<crate::git::Commit>,
    pub more: bool,
    pub capped: bool,
}

impl crate::git::Repository {
    /// GitPushTarget.parse: editing selects an ordinary remote branch.
    pub fn custom_push_target(
        &self,
        remote: &str,
        branch: &str,
        token: &crate::git::Cancel,
    ) -> Result<Target, String> {
        let targets = self.pull_targets(token)?;
        if !targets.remotes.iter().any(|name| name == remote) {
            return Err("Invalid push remote".into());
        }
        self.text(
            &["check-ref-format", &format!("refs/heads/{branch}")],
            token,
        )?;
        // GitRefNameValidator additionally rejects quotes and dot-before-slash.
        if branch.is_empty()
            || branch.starts_with('-')
            || branch.contains('"')
            || branch.contains("./")
        {
            return Err("Invalid destination branch".into());
        }
        Ok(Target {
            remote: remote.into(),
            branch: branch.into(),
            special: false,
            new_branch: !targets.contains(remote, branch),
            origin: Origin::Custom,
        })
    }
}

/// GitRefNameValidator.checkInput (without on-typing cleanup).
pub fn valid_branch_input(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(['.', '-', '/'])
        && !name.ends_with(['.', '/'])
        && !name.ends_with(".lock")
        && !name.contains([' ', '~', ':', '^', '?', '*', '\"', '[', '\\'])
        && !name.chars().any(|c| (c as u32) < 32 || c == '\u{7f}')
        && !["@{", "//", "..", "./", "/."]
            .iter()
            .any(|value| name.contains(value))
}
/// Checkbox exists for an existing default tracking-type target and is shown
/// only for a different, valid destination. It starts unchecked.
pub fn show_upstream_checkbox(default: Option<&Target>, remote: &str, branch: &str) -> bool {
    default.is_some_and(|target| {
        target.origin == Origin::TrackingBranch
            && !target.new_branch
            && (target.remote != remote || target.branch != branch)
            && valid_branch_input(branch)
    })
}

#[cfg(test)]
mod upstream_tests {
    use super::*;
    #[test]
    fn checkbox_tracks_default_kind_existing_target_and_valid_changed_destination() {
        let mut target = Target {
            remote: "origin".into(),
            branch: "main".into(),
            special: false,
            new_branch: false,
            origin: Origin::TrackingBranch,
        };
        assert!(!show_upstream_checkbox(Some(&target), "origin", "main"));
        assert!(show_upstream_checkbox(Some(&target), "origin", "review"));
        assert!(show_upstream_checkbox(Some(&target), "other", "main"));
        for name in [
            "", "a b", "a..b", "a./b", "a/.b", "a.lock", "a\\b", "a\"b", "-a", "a\n", "a@{b",
        ] {
            assert!(
                !show_upstream_checkbox(Some(&target), "origin", name),
                "{name:?}"
            );
        }
        assert!(show_upstream_checkbox(Some(&target), "origin", "审阅/修改"));
        target.new_branch = true;
        assert!(!show_upstream_checkbox(Some(&target), "origin", "review"));
        target.new_branch = false;
        target.origin = Origin::PushSpec;
        assert!(!show_upstream_checkbox(Some(&target), "origin", "review"));
        assert!(!show_upstream_checkbox(None, "origin", "review"));
    }
}
