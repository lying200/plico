//! Operation-scoped AskPass/UI handoff. Rebased GitHttpAuthService dispatches
//! helper requests to an operation handler; its authentication gate serializes UI
//! interaction and propagates cancellation (reference 4aec35256d765c217aa0d662b82ac4e860c396fd).
//! HTTP credentials may be reused within this operation; transport, SSH prompt
//! classification and persistence are separate from this bounded handoff.
use crate::git::Cancel;
use std::{
    sync::{Arc, Mutex, TryLockError, mpsc},
    thread,
    time::Duration,
};
const MAX_TEXT: usize = 64 * 1024;
const POLL: Duration = Duration::from_millis(50);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    HttpUsername,
    HttpPassword,
    Ssh,
    CommitEditor,
    SequenceEditor,
}
struct Credentials {
    target: String,
    username: String,
    password: String,
}
type Cache = Arc<Mutex<Option<Credentials>>>;
fn http_url(prompt: &str) -> Option<url::Url> {
    // GitHttpAuthService.AuthAppHandler: third space-delimited token, unquoted.
    let text = prompt
        .split(' ')
        .nth(2)?
        .trim_start_matches('\'')
        .trim_end_matches(['\'', ':']);
    let url = url::Url::parse(text).ok()?;
    (matches!(url.scheme(), "http" | "https") && url.host_str().is_some()).then_some(url)
}
fn target(mut url: url::Url) -> String {
    let _ = url.set_username("");
    let _ = url.set_password(None);
    url.into()
}
// No Debug: prompts can contain URLs/user identities; responses contain secrets.
pub struct Pending {
    pub kind: Kind,
    pub prompt: String,
    reply: Option<mpsc::SyncSender<String>>,
    cancel: Cancel,
    stop: Cancel,
    credentials: Cache,
}
impl Pending {
    pub fn is_cancelled(&self) -> bool {
        self.cancel.cancelled() || self.stop.cancelled()
    }
    pub fn http_target(&self) -> Option<String> {
        let mut url = http_url(&self.prompt)?;
        url.set_query(None);
        url.set_fragment(None);
        Some(target(url))
    }
    /// One HTTP login form supplies both fields, as in GitHttpGuiAuthenticator.
    /// Only a subsequent password request for the exact target/login may reuse it.
    pub fn answer_credentials(self, username: String, password: String) -> Result<(), Error> {
        if self.kind != Kind::HttpUsername || !valid_reply(&username) || !valid_reply(&password) {
            return Err(Error::InvalidResponse);
        }
        let mut url = http_url(&self.prompt).ok_or(Error::InvalidPrompt)?;
        url.set_username(&username)
            .map_err(|_| Error::InvalidResponse)?;
        let encoded_username = url.username().to_owned();
        let credentials = self.credentials.clone();
        *credentials.lock().unwrap_or_else(|e| e.into_inner()) = Some(Credentials {
            target: target(url),
            username: encoded_username,
            password,
        });
        let result = self.answer(username);
        if result.is_err() {
            credentials.lock().unwrap_or_else(|e| e.into_inner()).take();
        }
        result
    }
    /// Consumes the UI request. Drop without answering means dismissal/cancellation.
    /// A stale UI callback cannot revive a cancelled operation.
    pub fn answer(mut self, response: String) -> Result<(), Error> {
        if self.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if !valid_reply(&response) {
            return Err(Error::InvalidResponse);
        }
        let reply = self.reply.take().unwrap();
        reply.send(response).map_err(|_| Error::Cancelled)
    }
}
fn valid_reply(response: &str) -> bool {
    response.len() <= MAX_TEXT && !response.contains(['\0', '\r', '\n'])
}
impl Drop for Pending {
    fn drop(&mut self) {
        if self.reply.is_some() {
            self.cancel.cancel();
            self.credentials
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    Cancelled,
    InvalidPrompt,
    InvalidResponse,
}
struct Shared {
    pending: Mutex<Option<Pending>>,
    gate: Mutex<()>,
    cancel: Cancel,
    credentials: Cache,
}
#[derive(Clone)]
pub struct Requests(Arc<Shared>);
impl Requests {
    pub fn new(cancel: Cancel) -> Self {
        Self(Arc::new(Shared {
            pending: Mutex::new(None),
            gate: Mutex::new(()),
            cancel,
            credentials: Arc::new(Mutex::new(None)),
        }))
    }
    pub fn clear_credentials(&self) {
        self.0
            .credentials
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
    }
    /// UI polling consumes the single active prompt; its Pending handle must be
    /// retained until submit/dismiss, never dropped as a temporary render value.
    pub fn take_pending(&self) -> Option<Pending> {
        self.0
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }
    /// Blocking helper worker API. Queued helpers wait without growing a UI queue.
    pub fn ask(&self, kind: Kind, prompt: String) -> Result<String, Error> {
        self.ask_until(kind, prompt, &self.0.cancel)
    }
    pub(crate) fn ask_until(
        &self,
        kind: Kind,
        prompt: String,
        stop: &Cancel,
    ) -> Result<String, Error> {
        if prompt.len() > MAX_TEXT || prompt.contains('\0') {
            return Err(Error::InvalidPrompt);
        }
        let _gate = loop {
            if self.0.cancel.cancelled() || stop.cancelled() {
                return Err(Error::Cancelled);
            }
            match self.0.gate.try_lock() {
                Ok(guard) => break guard,
                Err(TryLockError::Poisoned(error)) => break error.into_inner(),
                Err(TryLockError::WouldBlock) => thread::sleep(POLL),
            }
        };
        if self.0.cancel.cancelled() || stop.cancelled() {
            return Err(Error::Cancelled);
        }
        if kind == Kind::HttpPassword {
            if let Some(url) = http_url(&prompt) {
                let cache = self.0.credentials.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(credentials) = cache.as_ref() {
                    if credentials.username == url.username() && credentials.target == target(url) {
                        return Ok(credentials.password.clone());
                    }
                }
            }
            // Changed/unrecognized target must ask again, never reuse credentials.
            self.clear_credentials();
        } else if kind == Kind::HttpUsername {
            self.clear_credentials();
        }
        let (tx, rx) = mpsc::sync_channel(1);
        *self.0.pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(Pending {
            kind,
            prompt,
            reply: Some(tx),
            cancel: self.0.cancel.clone(),
            stop: stop.clone(),
            credentials: self.0.credentials.clone(),
        });
        let result = loop {
            if self.0.cancel.cancelled() || stop.cancelled() {
                break Err(Error::Cancelled);
            }
            match rx.recv_timeout(POLL) {
                Ok(response) => {
                    break if self.0.cancel.cancelled() {
                        Err(Error::Cancelled)
                    } else {
                        Ok(response)
                    };
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break Err(Error::Cancelled),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
        };
        // Remove an unconsumed prompt on cancellation so later UI polling cannot
        // surface a request from a finished operation.
        self.0
            .pending
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pending(requests: &Requests) -> Pending {
        let start = std::time::Instant::now();
        loop {
            if let Some(request) = requests.take_pending() {
                return request;
            }
            assert!(start.elapsed() < Duration::from_secs(3));
            thread::sleep(Duration::from_millis(5));
        }
    }
    fn ask(requests: &Requests) -> thread::JoinHandle<Result<String, Error>> {
        let requests = requests.clone();
        thread::spawn(move || requests.ask(Kind::HttpPassword, "Password:".into()))
    }
    #[test]
    fn response_is_returned_only_to_requesting_operation() {
        let first = Requests::new(Cancel::default());
        let second = Requests::new(Cancel::default());
        let a = ask(&first);
        let b = ask(&second);
        pending(&first).answer("one".into()).unwrap();
        pending(&second).answer("two".into()).unwrap();
        assert_eq!(a.join().unwrap().unwrap(), "one");
        assert_eq!(b.join().unwrap().unwrap(), "two");
    }
    #[test]
    fn dismissal_cancels_active_and_queued_helpers_without_reprompting() {
        let cancel = Cancel::default();
        let requests = Requests::new(cancel.clone());
        let first = ask(&requests);
        let prompt = pending(&requests);
        let second = ask(&requests);
        drop(prompt);
        assert_eq!(first.join().unwrap(), Err(Error::Cancelled));
        assert_eq!(second.join().unwrap(), Err(Error::Cancelled));
        assert!(cancel.cancelled());
        assert!(requests.take_pending().is_none());
    }
    #[test]
    fn operation_cancel_rejects_late_ui_reply() {
        let cancel = Cancel::default();
        let requests = Requests::new(cancel.clone());
        let worker = ask(&requests);
        let prompt = pending(&requests);
        cancel.cancel();
        assert_eq!(worker.join().unwrap(), Err(Error::Cancelled));
        assert_eq!(prompt.answer("late".into()), Err(Error::Cancelled));
        assert!(requests.take_pending().is_none());
    }
    #[test]
    fn invalid_response_cannot_inject_another_protocol_line() {
        let requests = Requests::new(Cancel::default());
        let worker = ask(&requests);
        assert_eq!(
            pending(&requests).answer("password\nextra".into()),
            Err(Error::InvalidResponse)
        );
        assert_eq!(worker.join().unwrap(), Err(Error::Cancelled));
        assert_eq!(
            requests.ask(Kind::Ssh, "x".repeat(MAX_TEXT + 1)),
            Err(Error::InvalidPrompt)
        );
    }
    fn login(requests: &Requests) {
        let worker_requests = requests.clone();
        let worker = thread::spawn(move || {
            worker_requests.ask(
                Kind::HttpUsername,
                "Username for 'https://example.invalid/repo':".into(),
            )
        });
        pending(requests)
            .answer_credentials("user".into(), "secret".into())
            .unwrap();
        assert_eq!(worker.join().unwrap().unwrap(), "user");
    }
    #[test]
    fn http_login_reuses_password_only_for_same_target_and_username() {
        let requests = Requests::new(Cancel::default());
        login(&requests);
        assert_eq!(
            requests
                .ask(
                    Kind::HttpPassword,
                    "Password for 'https://user@example.invalid/repo':".into()
                )
                .unwrap(),
            "secret"
        );
        for changed in [
            "https://user@other.invalid/repo",
            "https://other@example.invalid/repo",
            "http://user@example.invalid/repo",
            "https://user@example.invalid/other",
        ] {
            login(&requests);
            let worker_requests = requests.clone();
            let worker = thread::spawn(move || {
                worker_requests.ask(Kind::HttpPassword, format!("Password for '{changed}':"))
            });
            pending(&requests).answer("different".into()).unwrap();
            assert_eq!(worker.join().unwrap().unwrap(), "different");
        }
    }
    #[test]
    fn clearing_operation_credentials_requires_new_answer() {
        let requests = Requests::new(Cancel::default());
        login(&requests);
        requests.clear_credentials();
        let worker_requests = requests.clone();
        let worker = thread::spawn(move || {
            worker_requests.ask(
                Kind::HttpPassword,
                "Password for 'https://user@example.invalid/repo':".into(),
            )
        });
        pending(&requests).answer("fresh".into()).unwrap();
        assert_eq!(worker.join().unwrap().unwrap(), "fresh");
    }
}
