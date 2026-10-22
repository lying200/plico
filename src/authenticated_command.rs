//! Remote-command authentication boundary. Never retries arbitrary Git writes.
use crate::{
    auth_request::Requests,
    git::Cancel,
    http_auth::Mode,
    operation_process::{self, Outcome, Progress},
};
use std::{path::Path, process::Command, time::Duration};
/// Caller supplies the side-effect guard: Fetch/ls-remote, an empty clone target,
/// or an unchanged update state. Command argv, force leases and editors stay fixed.
pub(crate) fn run(
    command: &mut Command,
    auth: Option<(&Requests, &Path)>,
    token: &Cancel,
    progress: &Progress,
    timeout: Option<Duration>,
    can_retry: impl Fn(&Outcome) -> bool,
) -> Result<Outcome, String> {
    let Some((requests, helper)) = auth else {
        return operation_process::run(command, token, progress, timeout)
            .map_err(|e| e.to_string());
    };
    if !helper.is_absolute() || !helper.is_file() {
        return Err("Authentication helper is unavailable".into());
    }
    let helper_available = has_helper(command, token);
    // Preserve inherited command-level config instead of replacing the caller's
    // keys (e.g. an explicit test proxy); our overrides are appended once.
    let inherited_count = match command.get_envs().find(|(k, _)| *k == "GIT_CONFIG_COUNT") {
        Some((_, value)) => value.map(|s| s.to_string_lossy().into_owned()),
        None => std::env::var("GIT_CONFIG_COUNT").ok(),
    };
    let count = inherited_count
        .unwrap_or_else(|| "0".into())
        .parse::<usize>()
        .map_err(|_| "Invalid Git configuration count")?;
    if count > 1024 {
        return Err("Invalid Git configuration count".into());
    }
    let mut attempts = 0;
    loop {
        if token.cancelled() {
            return Err("Cancelled".into());
        }
        requests.clear_credentials();
        requests.auth().begin(helper_available);
        let use_helper = requests.auth().helper();
        let mut settings = if use_helper {
            vec![("credential.useHttpPath", "true")]
        } else {
            vec![
                ("credential.useHttpPath", "true"),
                ("credential.helper", ""),
            ]
        };
        // Silent app authentication still needs Git to invoke AskPass; silence
        // is enforced by Requests, not by disabling the bridge itself.
        settings.push((
            "credential.interactive",
            if use_helper && requests.auth().mode != Mode::Full {
                "false"
            } else {
                "true"
            },
        ));
        command.env("GIT_CONFIG_COUNT", (count + settings.len()).to_string());
        for (i, (key, value)) in settings.iter().enumerate() {
            command
                .env(format!("GIT_CONFIG_KEY_{}", count + i), key)
                .env(format!("GIT_CONFIG_VALUE_{}", count + i), value);
        }
        if requests.auth().mode != Mode::Full {
            command.env("GCM_INTERACTIVE", "Never");
        }
        let server = crate::askpass::Server::start(requests.clone()).map_err(|e| e.to_string())?;
        server.configure(command);
        command
            .env("GIT_ASKPASS", helper)
            .env_remove("GITUI_ASKPASS_SSH");
        let outcome =
            operation_process::run(command, token, progress, timeout).map_err(|e| e.to_string())?;
        drop(server);
        if token.cancelled() || outcome.stopped.is_some() || outcome.status.success() {
            return Ok(outcome);
        }
        let switch = requests.auth().switched();
        let failed = requests.auth().requested()
            && crate::http_auth::authentication_failed(&outcome.stderr.bytes());
        if failed && !switch {
            requests.auth().failed();
        }
        if !switch {
            attempts += 1;
        }
        if requests.auth().mode != Mode::Full
            || (!switch && (!failed || attempts >= 2))
            || !can_retry(&outcome)
        {
            return Ok(outcome);
        }
    }
}
fn has_helper(command: &Command, token: &Cancel) -> bool {
    let mut query = Command::new(command.get_program());
    // Only take the leading Git global arguments, before the subcommand.
    let args: Vec<_> = command.get_args().collect();
    let mut i = 0;
    while i < args.len() {
        let arg = args[i];
        if arg == "-c" || arg == "-C" || arg == "--git-dir" || arg == "--work-tree" {
            if i + 1 >= args.len() {
                return false;
            }
            query.arg(arg).arg(args[i + 1]);
            i += 2;
        } else if arg.to_string_lossy().starts_with('-') {
            query.arg(arg);
            i += 1;
        } else {
            break;
        }
    }
    for (k, v) in command.get_envs() {
        if let Some(v) = v {
            query.env(k, v);
        } else {
            query.env_remove(k);
        }
    }
    if let Some(dir) = command.get_current_dir() {
        query.current_dir(dir);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        query.creation_flags(0x08000000);
    }
    query.args(["config", "--get-all", "credential.helper"]);
    operation_process::run(
        &mut query,
        token,
        &Progress::default(),
        Some(Duration::from_secs(5)),
    )
    .is_ok_and(|r| {
        r.status.success()
            && String::from_utf8_lossy(&r.stdout.bytes())
                .lines()
                .last()
                .is_some_and(|s| !s.trim().is_empty())
    })
}
/// Do not replay a push that already succeeded for any ref / push URL.
pub(crate) fn push_can_retry(outcome: &Outcome) -> bool {
    !outcome.stdout.truncated
        && !String::from_utf8_lossy(&outcome.stdout.bytes())
            .lines()
            .any(|line| {
                matches!(line.as_bytes().first(), Some(b' ' | b'+' | b'-' | b'*'))
                    && line.as_bytes().get(1) == Some(&b'\t')
            })
}
