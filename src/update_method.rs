//! Rebased GitUpdater.resolveUpdateMethod + GitConfigUtil.getBooleanValue.
//! Reference 4aec35256d765c217aa0d662b82ac4e860c396fd, plugins/git4idea/backend/src/.
//! This resolves the updater's default, not the Pull dialog's explicit options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method { Merge, Rebase }
fn boolean(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" | "" => Some(false),
        _ => None,
    }
}
fn rebase(value: &str) -> bool {
    boolean(value) == Some(true) || value.eq_ignore_ascii_case("interactive") || value.eq_ignore_ascii_case("preserve")
}
pub fn resolve(branch_rebase: Option<&str>, pull_rebase: Option<&str>) -> Method {
    if let Some(value) = branch_rebase {
        if rebase(value) { return Method::Rebase; }
        if boolean(value) == Some(false) { return Method::Merge; }
        // Unknown branch values fall through to pull.rebase, as upstream does.
    }
    if pull_rebase.is_some_and(rebase) { Method::Rebase } else { Method::Merge }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebased_update_method_test_scenarios() {
        assert_eq!(resolve(None, None), Method::Merge);
        for value in ["interactive", "preserve"] {
            assert_eq!(resolve(Some(value), None), Method::Rebase);
        }
        for value in ["true", "interactive", "preserve"] {
            assert_eq!(resolve(None, Some(value)), Method::Rebase);
        }
        assert_eq!(resolve(Some("false"), Some("true")), Method::Merge);
    }
    #[test]
    fn explicit_false_overrides_global_and_unknown_branch_falls_through() {
        for value in ["false", "NO", "off", "0", ""] {
            assert_eq!(resolve(Some(value), Some("true")), Method::Merge);
        }
        for value in ["TRUE", "yes", "ON", "1", "INTERACTIVE", "Preserve"] {
            assert_eq!(resolve(Some(value), Some("false")), Method::Rebase);
        }
        assert_eq!(resolve(Some("unknown"), Some("true")), Method::Rebase);
        assert_eq!(resolve(None, Some("unknown")), Method::Merge);
        // Match this reference version exactly, not all values accepted by Git.
        assert_eq!(resolve(None, Some("merges")), Method::Merge);
    }
}
