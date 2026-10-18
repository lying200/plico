//! Rebased continuation editor routing: GitRebaser ->
//! GitInteractiveRebaseEditorHandler -> GitRewordedCommitMessageProvider.
//! Reference 4aec352 (JetBrains, Apache-2.0).
use crate::{askpass::Server, auth_request::{Kind, Requests}, editor::Document, git::Cancel, squash_message};
use std::{io, path::Path, process::Command, thread, time::Duration};

/// Operation-scoped bridge. Persisted matching messages bypass the ordinary UI;
/// every unmatched request retains the ordinary editor and cancellation path.
pub(crate) struct Router {
    server: Option<Server>,
    stop: Cancel,
    worker: Option<thread::JoinHandle<Option<String>>>,
}
impl Router {
    pub(crate) fn start(directory: &Path, fallback: Requests, token: Cancel) -> io::Result<Self> {
        let requests = Requests::new(token.clone());
        let server = Server::start(requests.clone())?;
        let stop = Cancel::default(); let worker_stop = stop.clone(); let directory = directory.to_owned();
        let worker = thread::Builder::new().name("rebase-editor".into()).spawn(move || {
            let mut error = None;
            while !worker_stop.cancelled() && !token.cancelled() {
                let Some(pending) = requests.take_pending() else { thread::sleep(Duration::from_millis(10)); continue; };
                let result = (|| -> Result<(), String> {
                    if matches!(pending.kind, Kind::CommitEditor | Kind::SequenceEditor) {
                        let mut document = Document::open(pending, &directory).map_err(|e| e.to_string())?;
                        if squash_message::apply(&mut document).map_err(|e| e.to_string())? { return Ok(()); }
                        match document.forward(&fallback, &worker_stop) {
                            // Dismissing the ordinary editor answers Git with exit
                            // 1. Preserve that command outcome and retry semantics.
                            Err(e) if e.kind() == io::ErrorKind::Interrupted => Ok(()),
                            other => other.map_err(|e| e.to_string()),
                        }
                    } else {
                        let reply = fallback.ask_until(pending.kind, pending.prompt.clone(), &worker_stop)
                            .map_err(|_| "Rebase prompt cancelled".to_owned())?;
                        pending.answer(reply).map_err(|_| "Rebase prompt ended".to_owned())
                    }
                })();
                if let Err(message) = result { if error.is_none() { error = Some(message); } }
            }
            error
        })?;
        Ok(Self { server: Some(server), stop, worker: Some(worker) })
    }
    pub(crate) fn configure(&self, command: &mut Command) { self.server.as_ref().unwrap().configure(command); }
    pub(crate) fn finish(mut self) -> Option<String> { self.close() }
    fn close(&mut self) -> Option<String> {
        self.stop.cancel(); self.server.take();
        self.worker.take().and_then(|worker| worker.join().unwrap_or_else(|_| Some("Rebase editor worker failed".into())))
    }
}
impl Drop for Router { fn drop(&mut self) { self.close(); } }
