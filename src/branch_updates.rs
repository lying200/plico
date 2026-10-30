//! Bounded reference observation and incoming/outgoing state. Remote probing
//! never fetches objects or changes refs; unknown remote state is not “up to date”.
use crate::{
    git::{Cancel, Repository, Result},
    operation_process::Progress,
};
use std::{collections::HashMap, time::Duration};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upstream {
    pub remote: String,
    pub reference: String,
    pub oid: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct References {
    fingerprint: String,
    pub head: Option<String>,
    pub branch: Option<String>,
    pub upstream: Option<Upstream>,
    push_oid: Option<String>,
}

/// Inputs which can invalidate a rendered history even when its visible HEAD
/// stays the same. Capture before reading history; never publish a later sample
/// as evidence that an earlier history result is current.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryObservation {
    pub references: References,
    shallow: (bool, std::collections::HashSet<String>),
}
impl HistoryObservation {
    pub(crate) fn from_parts(
        references: References,
        shallow: (bool, std::collections::HashSet<String>),
    ) -> Self {
        Self {
            references,
            shallow,
        }
    }
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Self> {
        Ok(Self {
            references: References::read(repo, token)?,
            shallow: repo.shallow_boundaries(token)?,
        })
    }
    pub fn is_shallow(&self) -> bool {
        self.shallow.0
    }
    /// Failed/cancelled reads are not evidence that history can be retained.
    pub fn unchanged(&self, repo: &Repository, token: &Cancel) -> bool {
        Self::read(repo, token).as_ref() == Ok(self)
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub incoming: usize,
    pub outgoing: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemoteState {
    Unknown,
    Current,
    Unfetched,
}
impl References {
    pub fn same_replacements(&self, other: &Self) -> bool {
        self.fingerprint
            .lines()
            .filter(|line| line.starts_with("refs/replace/"))
            .eq(other
                .fingerprint
                .lines()
                .filter(|line| line.starts_with("refs/replace/")))
    }
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Self> {
        // Replacement objects can change history and details without moving
        // HEAD or ordinary branch/tag refs. They invalidate the observation,
        // but never become displayed branches or incoming/outgoing targets.
        let refs=repo.text(&["for-each-ref", "--format=%(refname)%09%(objectname)%09%(HEAD)%09%(upstream)%09%(upstream:remotename)%09%(upstream:remoteref)%09%(push)","refs/heads","refs/remotes","refs/tags","refs/replace"],token)?;
        let head = repo
            .text(&["rev-parse", "--verify", "HEAD"], token)
            .ok()
            .map(|s| s.trim().to_owned());
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        Ok(Self::from_records(refs, head))
    }
    pub(crate) fn from_records(refs: String, head: Option<String>) -> Self {
        let rows = refs
            .lines()
            .map(|s| s.split('\t').collect::<Vec<_>>())
            .collect::<Vec<_>>();
        let oids: HashMap<_, _> = rows
            .iter()
            .filter(|r| r.len() == 7)
            .map(|r| (r[0], r[1]))
            .collect();
        let current = rows.iter().find(|r| r.len() == 7 && r[2] == "*");
        let branch = current.map(|r| r[0].trim_start_matches("refs/heads/").to_owned());
        let upstream = current
            .filter(|r| !r[4].is_empty() && !r[5].is_empty())
            .map(|r| Upstream {
                remote: r[4].into(),
                reference: r[5].into(),
                oid: oids.get(r[3]).map(|s| s.to_string()),
            });
        let push_oid = current.and_then(|r| oids.get(r[6])).map(|s| s.to_string());
        Self {
            fingerprint: refs,
            head,
            branch,
            upstream,
            push_oid,
        }
    }
    pub fn counts(&self, repo: &Repository, token: &Cancel) -> Result<Counts> {
        let Some(head) = &self.head else {
            return Ok(Counts::default());
        };
        let count = |from: &str, to: &str| -> Result<usize> {
            if from == to {
                return Ok(0);
            }
            repo.text(
                &["rev-list", "--count", &format!("{from}..{to}"), "--"],
                token,
            )?
            .trim()
            .parse()
            .map_err(|_| "Invalid commit count".into())
        };
        Ok(Counts {
            incoming: match self.upstream.as_ref().and_then(|u| u.oid.as_ref()) {
                Some(oid) => count(head, oid)?,
                None => 0,
            },
            outgoing: match &self.push_oid {
                Some(oid) if self.branch.is_some() => count(oid, head)?,
                _ => 0,
            },
        })
    }
    pub fn check_remote(&self, repo: &Repository, token: &Cancel) -> Result<RemoteState> {
        let Some(upstream) = &self.upstream else {
            return Ok(RemoteState::Unknown);
        };
        if upstream.remote == "." {
            return Ok(RemoteState::Current);
        }
        // A custom transport may require its own non-interactive switches;
        // don't silently replace it or launch an authentication UI in a timer.
        if std::env::var_os("GIT_SSH_COMMAND").is_some()
            || std::env::var_os("GIT_SSH").is_some()
            || repo
                .text(&["config", "--get", "core.sshCommand"], token)
                .is_ok_and(|s| !s.trim().is_empty())
        {
            return Err("Background check requires explicit custom transport support".into());
        }
        let helper = std::env::current_exe().ok().map(|p| {
            p.with_file_name(if cfg!(windows) {
                "plico-askpass.exe"
            } else {
                "plico-askpass"
            })
        });
        let requests = crate::auth_request::Requests::with_store(
            token.clone(),
            crate::http_auth::Mode::Silent,
            crate::credential_store::Store::system(),
        );
        requests.set_project(&repo.path);
        let auth = helper
            .as_deref()
            .filter(|p| p.is_file())
            .map(|p| (&requests, p));
        let mut command = repo.command();
        if auth.is_none() {
            command.args(["-c", "credential.interactive=false"]);
        }
        command
            .args([
                "ls-remote",
                "--heads",
                "--",
                &upstream.remote,
                &upstream.reference,
            ])
            .env("GCM_INTERACTIVE", "Never")
            .env("GIT_ASKPASS", "")
            .env("SSH_ASKPASS", "")
            .env("GIT_SSH_COMMAND", "ssh -oBatchMode=yes -oConnectTimeout=10");
        let result = crate::authenticated_command::run(
            &mut command,
            auth,
            token,
            &Progress::default(),
            Some(Duration::from_secs(30)),
            |_| false,
        )?;
        if !result.status.success() || result.stopped.is_some() || result.stdout.truncated {
            return Err("Background remote check unavailable".into());
        }
        let output =
            String::from_utf8(result.stdout.bytes()).map_err(|_| "Invalid remote references")?;
        let oid = output
            .lines()
            .filter_map(|s| s.split_once('\t'))
            .find(|(_, r)| *r == upstream.reference)
            .map(|(id, _)| id);
        Ok(match oid {
            Some(oid) if Some(oid) == upstream.oid.as_deref() => RemoteState::Current,
            Some(_) => RemoteState::Unfetched,
            None => RemoteState::Unknown,
        })
    }
}
