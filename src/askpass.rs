//! Lightweight alternative to Rebased's operation handler + IDE REST AskPass
//! transport. Only loopback is used; each operation gets an OS-random capability.
//! No prompts, tokens or replies are logged or written to temporary files.
use crate::{
    auth_request::{Kind, Requests},
    git::Cancel,
};
use std::{
    io::{self, Read, Write},
    net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream},
    process::Command,
    thread,
    time::{Duration, Instant},
};
const LIMIT: usize = 64 * 1024;
const PORT_ENV: &str = "GITUI_ASKPASS_PORT";
const TOKEN_ENV: &str = "GITUI_ASKPASS_TOKEN";

pub struct Server {
    port: u16,
    token: String,
    stop: Cancel,
    worker: Option<thread::JoinHandle<()>>,
    requests: Requests,
}
impl Server {
    pub fn start(requests: Requests) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let mut random = [0u8; 32];
        getrandom::fill(&mut random)
            .map_err(|_| io::Error::other("Cannot create authentication session"))?;
        let token: String = random.iter().map(|b| format!("{b:02x}")).collect();
        let expected = token.clone();
        let stop = Cancel::default();
        let worker_stop = stop.clone();
        let owner_requests = requests.clone();
        let worker = thread::Builder::new()
            .name("askpass".into())
            .spawn(move || {
                while !worker_stop.cancelled() {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = serve(stream, &expected, &requests, &worker_stop);
                        }
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(10))
                        }
                        Err(_) => break,
                    }
                }
            })?;
        Ok(Self {
            port,
            token,
            stop,
            worker: Some(worker),
            requests: owner_requests,
        })
    }
    /// Sets session routing only. Caller selects GIT_ASKPASS/SSH_ASKPASS helper path.
    pub fn configure(&self, command: &mut Command) {
        command
            .env(PORT_ENV, self.port.to_string())
            .env(TOKEN_ENV, &self.token);
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.requests.clear_credentials();
    }
}
fn interrupted() -> io::Error {
    io::Error::new(io::ErrorKind::Interrupted, "Authentication request ended")
}
fn read_until(
    stream: &mut TcpStream,
    mut bytes: &mut [u8],
    stop: &Cancel,
    deadline: Instant,
) -> io::Result<()> {
    while !bytes.is_empty() {
        if stop.cancelled() || Instant::now() >= deadline {
            return Err(interrupted());
        }
        match stream.read(bytes) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(n) => bytes = &mut bytes[n..],
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
fn serve(mut stream: TcpStream, token: &str, requests: &Requests, stop: &Cancel) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(100)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut received = [0; 64];
    read_until(&mut stream, &mut received, stop, deadline)?;
    if received
        .iter()
        .zip(token.bytes())
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        != 0
    {
        return Err(io::ErrorKind::PermissionDenied.into());
    }
    let mut header = [0; 5];
    read_until(&mut stream, &mut header, stop, deadline)?;
    let kind = match header[0] {
        0 => Kind::HttpUsername,
        1 => Kind::HttpPassword,
        2 => Kind::Ssh,
        3 => Kind::CommitEditor,
        4 => Kind::SequenceEditor,
        _ => return Err(io::ErrorKind::InvalidData.into()),
    };
    let length = u32::from_be_bytes(header[1..].try_into().unwrap()) as usize;
    if length > LIMIT {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut prompt = vec![0; length];
    read_until(&mut stream, &mut prompt, stop, deadline)?;
    let prompt = String::from_utf8(prompt).map_err(|_| io::ErrorKind::InvalidData)?;
    match requests.ask_until(kind, prompt, stop) {
        Ok(answer) => {
            stream.write_all(&[0])?;
            stream.write_all(&(answer.len() as u32).to_be_bytes())?;
            stream.write_all(answer.as_bytes())
        }
        Err(_) => stream.write_all(&[1]),
    }
}
/// Entry used by the tiny helper binary. Git passes the prompt as a single arg.
/// HTTP classification matches GitHttpAuthService; SSH uses an explicit env mode.
pub fn helper(prompt: &str) -> io::Result<Option<String>> {
    let kind = if std::env::var_os("GITUI_ASKPASS_SSH").is_some() {
        2
    } else if prompt
        .get(..8)
        .is_some_and(|s| s.eq_ignore_ascii_case("username"))
    {
        0
    } else {
        1
    };
    request(kind, prompt)
}
/// Shared capability transport; editor requests carry a path and return an exit code.
pub(crate) fn request(kind: u8, prompt: &str) -> io::Result<Option<String>> {
    if prompt.len() > LIMIT || prompt.contains('\0') {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let port: u16 = std::env::var(PORT_ENV)
        .ok()
        .and_then(|v| v.parse().ok())
        .ok_or(io::ErrorKind::InvalidInput)?;
    let token = std::env::var(TOKEN_ENV).map_err(|_| io::ErrorKind::InvalidInput)?;
    if token.len() != 64 || !token.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(3))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    stream.write_all(token.as_bytes())?;
    stream.write_all(&[kind])?;
    stream.write_all(&(prompt.len() as u32).to_be_bytes())?;
    stream.write_all(prompt.as_bytes())?;
    // The owning operation/server controls cancellation while the user responds.
    let mut status = [0];
    stream.read_exact(&mut status)?;
    if status[0] == 1 {
        return Ok(None);
    }
    if status[0] != 0 {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > LIMIT {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let mut answer = vec![0; length];
    stream.read_exact(&mut answer)?;
    let answer = String::from_utf8(answer).map_err(|_| io::ErrorKind::InvalidData)?;
    if answer.contains(['\0', '\r', '\n']) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(Some(answer))
}
