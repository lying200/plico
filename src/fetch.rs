//! Fetch request semantics translated from Rebased GitFetchSpec/GitImpl.
//! Reference: detachhead/rebased 4aec35256d765c217aa0d662b82ac4e860c396fd,
//! plugins/git4idea/backend/src/fetch/GitFetchSpec.kt and commands/GitImpl.java.
//! Execution, authentication and UI lifecycle are separate from this request model.
use std::ffi::OsString;

#[derive(Debug, PartialEq, Eq)]
pub enum RemoteState { Succeeded, Failed, Cancelled, NotStarted }
#[derive(Debug)]
pub struct RemoteResult {
    pub remote: String,
    pub state: RemoteState,
    /// Raw diagnostic bytes, not suitable for logging/display without redaction.
    pub diagnostic: Vec<u8>,
    pub diagnostic_truncated: bool,
}
#[derive(Debug)]
pub struct BatchResult { pub remotes: Vec<RemoteResult> }
impl BatchResult {
    pub fn successful(&self) -> bool {
        self.remotes.iter().all(|r| r.state == RemoteState::Succeeded)
    }
}

/// GitFetchSupportImpl's per-remote results and all-success aggregation.
/// Serial execution is the low-memory adaptation; an individual failure does not
/// skip subsequent remotes. Cancellation preserves completed results and records
/// untouched targets explicitly. Always refresh repository state after returning.
pub fn execute(repo: &crate::git::Repository, specs: &[Spec], token: &crate::git::Cancel,
    progress: &crate::operation_process::Progress) -> Result<BatchResult, String> {
    execute_inner(repo, specs, token, progress, None)
}
pub fn execute_authenticated(repo: &crate::git::Repository, specs: &[Spec], token: &crate::git::Cancel,
    progress: &crate::operation_process::Progress, requests: &crate::auth_request::Requests,
    helper: &std::path::Path) -> Result<BatchResult, String> {
    execute_inner(repo, specs, token, progress, Some((requests, helper)))
}
fn execute_inner(repo: &crate::git::Repository, specs: &[Spec], token: &crate::git::Cancel,
    progress: &crate::operation_process::Progress,
    auth: Option<(&crate::auth_request::Requests, &std::path::Path)>) -> Result<BatchResult, String> {
    if specs.len() > 4096 { return Err("Remote count exceeds the 4096 limit".into()); }
    let mut remaining_diagnostics = 64 * 1024;
    let mut remotes = Vec::with_capacity(specs.len());
    for spec in specs {
        if token.cancelled() {
            remotes.push(RemoteResult { remote: spec.remote.clone(), state: RemoteState::NotStarted,
                diagnostic: Vec::new(), diagnostic_truncated: false });
            continue;
        }
        let result = match auth {
            Some((requests, helper)) => repo.fetch_authenticated(spec, token, progress, requests, helper),
            None => repo.fetch(spec, token, progress),
        };
        let (state, mut diagnostic, truncated) = match result {
            Ok(outcome) => {
                let state = if outcome.stopped == Some(crate::operation_process::StopReason::Cancelled) {
                    RemoteState::Cancelled
                } else if outcome.stopped.is_some() || !outcome.status.success() {
                    RemoteState::Failed
                } else { RemoteState::Succeeded };
                let diagnostic = if state == RemoteState::Succeeded { Vec::new() } else { outcome.stderr.bytes() };
                let truncated = state != RemoteState::Succeeded && outcome.stderr.truncated;
                (state, diagnostic, truncated)
            }
            Err(error) => (if token.cancelled() { RemoteState::Cancelled } else { RemoteState::Failed }, error.into_bytes(), false),
        };
        // Total diagnostic retention stays bounded even for thousands of remotes.
        let keep = diagnostic.len().min(8 * 1024).min(remaining_diagnostics);
        let diagnostic_truncated = truncated || keep < diagnostic.len();
        if keep < diagnostic.len() { diagnostic = diagnostic.split_off(diagnostic.len() - keep); }
        remaining_diagnostics -= keep;
        remotes.push(RemoteResult { remote: spec.remote.clone(), state, diagnostic, diagnostic_truncated });
    }
    Ok(BatchResult { remotes })
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tags { #[default] Default, Prune, All, None }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    remote: String,
    pub tags: Tags,
    pub unshallow: bool,
    pub refspec: Option<String>,
}
impl Spec {
    pub fn new(remote: String) -> Result<Self, String> {
        if remote.is_empty() || remote.contains(['\0', '\r', '\n']) {
            return Err("Invalid remote name".into());
        }
        Ok(Self { remote, tags: Tags::Default, unshallow: false, refspec: None })
    }
    pub fn remote(&self) -> &str { &self.remote }
    /// Modern Git equivalent of GitFetchSpec.asParams + GitImpl.fetch.
    /// Options precede `--` so remote/refspec values can never become options.
    pub fn args(&self) -> Result<Vec<OsString>, String> {
        if self.refspec.as_ref().is_some_and(|s| s.is_empty() || s.contains(['\0', '\r', '\n'])) {
            return Err("Invalid fetch refspec".into());
        }
        let mut args: Vec<OsString> = ["fetch", "--recurse-submodules=no", "--progress", "--prune"]
            .into_iter().map(OsString::from).collect();
        match self.tags {
            Tags::Default => {},
            Tags::Prune => args.push("--prune-tags".into()),
            Tags::All => args.push("--tags".into()),
            Tags::None => args.push("--no-tags".into()),
        }
        if self.unshallow { args.push("--unshallow".into()); }
        args.push("--".into());
        args.push(self.remote.clone().into());
        if let Some(refspec) = &self.refspec { args.push(refspec.clone().into()); }
        Ok(args)
    }
}

/// GitFetchSupportImpl.getDefaultRemoteToFetch; the toolbar Fetch uses all
/// remotes instead. A missing configured target is not a guessed remote.
pub fn default_remote<'a>(remotes: &'a [String], tracked_remote: Option<&str>) -> Option<&'a str> {
    if remotes.len() == 1 { return Some(&remotes[0]); }
    tracked_remote.and_then(|name| remotes.iter().find(|remote| remote.as_str() == name))
        .or_else(|| remotes.iter().find(|remote| remote.as_str() == "origin"))
        .map(String::as_str)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_follow_tracking_then_origin_without_guessing() {
        assert_eq!(default_remote(&[], None), None);
        let one = vec!["company".into()];
        assert_eq!(default_remote(&one, Some("gone")), Some("company"));
        let remotes = vec!["company".into(), "origin".into()];
        assert_eq!(default_remote(&remotes, Some("company")), Some("company"));
        assert_eq!(default_remote(&remotes, Some("gone")), Some("origin"));
        assert_eq!(default_remote(&["a".into(), "b".into()], None), None);
    }
    #[test]
    fn argument_values_cannot_enable_unrequested_options() {
        let mut spec = Spec::new("--upload-pack=other".into()).unwrap();
        spec.refspec = Some("--force".into());
        let args = spec.args().unwrap();
        let end = args.iter().position(|arg| arg == "--").unwrap();
        assert_eq!(args[end + 1], "--upload-pack=other");
        assert_eq!(args[end + 2], "--force");
        assert!(!args[..end].iter().any(|arg| arg == "--tags" || arg == "--prune-tags" || arg == "--unshallow"));
        spec.refspec = Some("bad\0ref".into());
        assert!(spec.args().is_err());
        assert!(Spec::new("".into()).is_err());
    }
}
