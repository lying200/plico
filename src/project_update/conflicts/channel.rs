//! A single updater's worker/UI conversation. Commands run on the worker that
//! still owns the group reservation; no guard or lock exemption crosses to UI.
use super::{Decision, Interaction};
use crate::{
    conflicts::{File, Resolution, Snapshot},
    git::{Cancel, Repository},
    merge_result::Prepared,
};
use std::{
    sync::{
        Arc,
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    },
    time::Duration,
};

const POLL: Duration = Duration::from_millis(25);
type Reply<T> = SyncSender<Result<T, String>>;

enum Command {
    Snapshot(Reply<Snapshot>),
    Prepare(Arc<Snapshot>, File, Reply<Arc<Prepared>>),
    Resolve(Arc<Snapshot>, File, Resolution, Reply<Snapshot>),
    Apply(Arc<Prepared>, Reply<()>),
    Stage(Arc<Prepared>, Reply<Snapshot>),
    SavedText(Arc<Prepared>, Reply<String>),
    Automatic(
        Arc<Prepared>,
        Option<crate::text_merge::State>,
        Reply<Option<crate::merge_result::AutomaticResult>>,
    ),
    StageResolved(Vec<Arc<Prepared>>, Reply<crate::merge_result::StageResult>),
    Finish(Decision),
}

/// Cloneable only within one resolver. A new conflict (even in the same root)
/// gets a new conversation, so an old dialog cannot affect the next commit.
#[derive(Clone)]
pub struct Client {
    repository: Repository,
    identity: Arc<()>,
    outgoing: SyncSender<Command>,
    cancelled: Cancel,
    ended: Cancel,
}
impl Client {
    pub fn active(&self) -> bool {
        !self.cancelled.cancelled() && !self.ended.cancelled()
    }
    pub fn cancel_token(&self) -> Cancel {
        self.cancelled.clone()
    }
    pub fn same_interaction(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
    pub fn repository(&self) -> &Repository {
        &self.repository
    }
    fn call<T>(&self, command: impl FnOnce(Reply<T>) -> Command) -> Result<T, String> {
        if !self.active() {
            return Err("Update conflict interaction has ended".into());
        }
        let (reply, incoming) = mpsc::sync_channel(1);
        let mut pending = command(reply);
        loop {
            if !self.active() {
                return Err("Update conflict interaction has ended".into());
            }
            match self.outgoing.try_send(pending) {
                Ok(()) => break,
                Err(TrySendError::Full(command)) => {
                    pending = command;
                    std::thread::sleep(POLL);
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err("Update conflict worker has ended".into());
                }
            }
        }
        loop {
            // Prefer a completed write's result over cancellation. The caller
            // still checks its operation identity before accepting UI updates.
            match incoming.recv_timeout(POLL) {
                Ok(result) => return result,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err("Update conflict worker has ended".into());
                }
                Err(RecvTimeoutError::Timeout) if !self.active() => {
                    return Err("Update conflict interaction has ended".into());
                }
                Err(RecvTimeoutError::Timeout) => {}
            }
        }
    }
    pub fn snapshot(&self) -> Result<Snapshot, String> {
        self.call(Command::Snapshot)
    }
    pub fn prepare(&self, snapshot: Arc<Snapshot>, file: File) -> Result<Arc<Prepared>, String> {
        self.call(|reply| Command::Prepare(snapshot, file, reply))
    }
    pub fn resolve(
        &self,
        snapshot: Arc<Snapshot>,
        file: File,
        resolution: Resolution,
    ) -> Result<Snapshot, String> {
        self.call(|reply| Command::Resolve(snapshot, file, resolution, reply))
    }
    pub fn apply(&self, prepared: Arc<Prepared>) -> Result<(), String> {
        self.call(|reply| Command::Apply(prepared, reply))
    }
    pub fn stage(&self, prepared: Arc<Prepared>) -> Result<Snapshot, String> {
        self.call(|reply| Command::Stage(prepared, reply))
    }
    pub fn saved_text(&self, prepared: Arc<Prepared>) -> Result<String, String> {
        self.call(|reply| Command::SavedText(prepared, reply))
    }
    pub fn resolve_automatically(
        &self,
        prepared: Arc<Prepared>,
        saved: Option<crate::text_merge::State>,
    ) -> Result<Option<crate::merge_result::AutomaticResult>, String> {
        self.call(|reply| Command::Automatic(prepared, saved, reply))
    }
    pub fn stage_resolved(&self, prepared: Vec<Arc<Prepared>>) -> crate::merge_result::StageResult {
        self.call(|reply| Command::StageResolved(prepared, reply))
            .unwrap_or_else(|error| crate::merge_result::StageResult {
                error: Some(error),
                ..Default::default()
            })
    }
    /// Non-blocking UI completion. A busy resolver must finish its pending write
    /// first. Failed enqueue does not silently dismiss the resolver.
    pub fn finish(&self, decision: Decision) -> bool {
        self.active() && self.outgoing.try_send(Command::Finish(decision)).is_ok()
    }
}

pub struct Pending {
    pub snapshot: Arc<Snapshot>,
    pub client: Client,
}
pub struct Ui {
    incoming: Receiver<Pending>,
}
impl Ui {
    pub fn take_pending(&self) -> Option<Pending> {
        self.incoming.try_recv().ok().filter(|p| p.client.active())
    }
}
pub struct Worker {
    outgoing: SyncSender<Pending>,
}
pub fn channel() -> (Worker, Ui) {
    let (outgoing, incoming) = mpsc::sync_channel(1);
    (Worker { outgoing }, Ui { incoming })
}
struct End(Cancel);
impl Drop for End {
    fn drop(&mut self) {
        self.0.cancel();
    }
}
impl Worker {
    pub fn resolve(&self, interaction: &Interaction<'_>) -> Result<Decision, String> {
        let snapshot = Arc::new(interaction.snapshot()?);
        // Mirrors proceedIfNothingToMerge: no empty dialog while an updater is
        // ready to complete. Session continuation still revalidates Git state.
        if snapshot.files.is_empty() {
            return Ok(Decision::Continue);
        }
        let (outgoing, incoming) = mpsc::sync_channel(1);
        let ended = Cancel::default();
        let _end = End(ended.clone());
        let client = Client {
            repository: interaction.repository().clone(),
            identity: Arc::new(()),
            outgoing,
            cancelled: interaction.token.clone(),
            ended,
        };
        let mut pending = Pending { snapshot, client };
        loop {
            if interaction.token.cancelled() {
                return Err("Cancelled".into());
            }
            match self.outgoing.try_send(pending) {
                Ok(()) => break,
                Err(TrySendError::Full(value)) => {
                    pending = value;
                    std::thread::sleep(POLL);
                }
                Err(TrySendError::Disconnected(_)) => return Ok(Decision::Close),
            }
        }
        loop {
            if interaction.token.cancelled() {
                return Err("Cancelled".into());
            }
            let command = match incoming.recv_timeout(POLL) {
                Ok(command) => command,
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return Ok(Decision::Close),
            };
            // Check again after receive: queued commands must not authorize
            // writes once the outer update/window has been cancelled.
            if interaction.token.cancelled() {
                return Err("Cancelled".into());
            }
            match command {
                Command::Snapshot(reply) => {
                    let _ = reply.send(interaction.snapshot());
                }
                Command::Prepare(snapshot, file, reply) => {
                    let _ = reply.send(interaction.prepare(&snapshot, &file).map(Arc::new));
                }
                Command::Resolve(snapshot, file, resolution, reply) => {
                    let _ = reply.send(interaction.resolve(&snapshot, &file, resolution));
                }
                Command::Apply(prepared, reply) => {
                    let _ = reply.send(interaction.apply(&prepared));
                }
                Command::Stage(prepared, reply) => {
                    let _ = reply.send(interaction.stage(&prepared));
                }
                Command::SavedText(prepared, reply) => {
                    let _ = reply.send(interaction.saved_text(&prepared));
                }
                Command::Automatic(prepared, saved, reply) => {
                    let _ =
                        reply.send(interaction.resolve_automatically(&prepared, saved.as_ref()));
                }
                Command::StageResolved(prepared, reply) => {
                    let _ = reply.send(Ok(interaction.stage_resolved(&prepared)));
                }
                Command::Finish(decision) => return Ok(decision),
            }
        }
    }
}
