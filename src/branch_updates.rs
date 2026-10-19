//! Bounded reference observation and incoming/outgoing state. Remote probing
//! never fetches objects or changes refs; unknown remote state is not “up to date”.
use crate::{
    git::{Cancel, Repository, Result},
    operation_process::{self, Progress},
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
    pub fn read(repo: &Repository, token: &Cancel) -> Result<Self> {
        let refs=repo.text(&["for-each-ref", "--format=%(refname)%09%(objectname)%09%(HEAD)%09%(upstream)%09%(upstream:remotename)%09%(upstream:remoteref)%09%(push)","refs/heads","refs/remotes","refs/tags"],token)?;
        let head = repo
            .text(&["rev-parse", "--verify", "HEAD"], token)
            .ok()
            .map(|s| s.trim().to_owned());
        if token.cancelled() {
            return Err("Cancelled".into());
        }
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
        Ok(Self {
            fingerprint: refs,
            head,
            branch,
            upstream,
            push_oid,
        })
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
        let mut command = repo.command();
        command
            .args([
                "-c",
                "credential.interactive=false",
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
        let result = operation_process::run(
            &mut command,
            token,
            &Progress::default(),
            Some(Duration::from_secs(30)),
        )
        .map_err(|e| e.to_string())?;
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
