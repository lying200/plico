//! Compact label titles based on Rebased GitRefManager.groupForTable.
use crate::git::{RefKind, RefLabel};
use std::collections::HashMap;

pub fn rank(r: &RefLabel, current: Option<&str>) -> u8 {
    match r.kind {
        RefKind::Head => 0,
        RefKind::Local if Some(r.name.as_str()) == current => 1,
        RefKind::Local if matches!(r.name.as_str(), "main" | "master") => 2,
        RefKind::Remote if matches!(r.name.as_str(), "origin/main" | "origin/master") => 3,
        RefKind::Local => 4,
        RefKind::Remote => 5,
        RefKind::Tag => 6,
    }
}


pub fn compact_title(
    refs: &[RefLabel],
    current: Option<&str>,
    upstreams: &HashMap<String, (String, String)>,
    remotes: &[&str],
) -> String {
    if current.is_none() && refs.iter().any(|r| r.kind == RefKind::Head) {
        return "HEAD (detached)".into();
    }
    let mut sorted: Vec<_> = refs.iter().filter(|r| r.kind != RefKind::Head).collect();
    sorted.sort_by(|a, b| rank(a, current).cmp(&rank(b, current)).then_with(|| crate::natural_order::compare(&a.name, &b.name)));
    let has_remote = |name: &str| {
        refs.iter()
            .any(|r| r.kind == RefKind::Remote && r.name == name)
    };
    let tracked: Vec<_> = sorted
        .iter()
        .filter(|r| r.kind == RefKind::Local)
        .filter_map(|r| {
            let remote = upstreams
                .get(&r.name)
                .filter(|(name, _)| has_remote(name))
                .map(|(_, remote)| remote.as_str())
                .or_else(|| {
                    remotes
                        .iter()
                        .copied()
                        .find(|remote| has_remote(&format!("{remote}/{}", r.name)))
                })?;
            Some((r.name.as_str(), format!("{remote} & {}", r.name)))
        })
        .collect();
    // A current branch outside a tracked group comes before all tracked groups.
    if let Some(branch) = sorted
        .iter()
        .find(|r| r.kind == RefKind::Local && Some(r.name.as_str()) == current)
    {
        if !tracked.iter().any(|(name, _)| *name == branch.name) {
            return branch.name.clone();
        }
    }
    tracked
        .first()
        .map(|(_, title)| title.clone())
        .or_else(|| sorted.first().map(|r| r.name.clone()))
        .unwrap_or_else(|| "HEAD".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn upstream_pair_requires_same_commit_and_uses_configured_remote() {
        let refs = vec![
            RefLabel {
                kind: RefKind::Local,
                name: "review".into(),
            },
            RefLabel {
                kind: RefKind::Remote,
                name: "team/trunk".into(),
            },
            RefLabel {
                kind: RefKind::Head,
                name: "HEAD".into(),
            },
        ];
        let upstream = HashMap::from([("review".into(), ("team/trunk".into(), "team".into()))]);
        assert_eq!(
            compact_title(&refs, Some("review"), &upstream, &["team"]),
            "team & review"
        );
        assert_eq!(
            compact_title(&refs[..1], Some("review"), &upstream, &["team"]),
            "review"
        );
        assert_eq!(
            compact_title(&refs, None, &upstream, &["team"]),
            "HEAD (detached)"
        );
    }
    #[test]
    fn same_name_fallback_and_current_untracked_precedence() {
        let mut refs = vec![
            RefLabel {
                kind: RefKind::Local,
                name: "main".into(),
            },
            RefLabel {
                kind: RefKind::Remote,
                name: "origin/main".into(),
            },
        ];
        assert_eq!(
            compact_title(&refs, Some("main"), &HashMap::new(), &["origin"]),
            "origin & main"
        );
        refs.push(RefLabel {
            kind: RefKind::Local,
            name: "review".into(),
        });
        assert_eq!(
            compact_title(&refs, Some("review"), &HashMap::new(), &["origin"]),
            "review"
        );
        assert_eq!(
            compact_title(&refs, Some("elsewhere"), &HashMap::new(), &[]),
            "main"
        );
    }
}
