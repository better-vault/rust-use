use super::{Client, Endpoint, IpcError, frame};
use crate::Agent;
use interprocess::local_socket::{Stream, prelude::*};
use rust_use_core::{ActionRequest, Ui};
use std::{
    collections::HashMap,
    io,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

/// One answered request, observed on its connection thread.
#[derive(Debug)]
pub struct RequestLog<'a> {
    /// Original request payload.
    pub request: &'a str,
    /// Reply payload sent to the client.
    pub reply: &'a str,
    /// Time spent dispatching the complete request and writing its reply.
    pub elapsed: Duration,
    /// Connection identifier unique within this server.
    pub connection: u64,
}

type Handler = Box<dyn Fn(&str) -> String + Send + Sync>;
type Observer = Box<dyn Fn(&RequestLog<'_>) + Send + Sync>;
type Predicate = Box<dyn Fn(&ActionRequest<'_>) -> bool + Send + Sync>;

/// Builder for a background local IPC server.
#[must_use]
pub struct Server {
    ui: Ui,
    endpoint: Endpoint,
    commands: HashMap<&'static str, Handler>,
    predicates: Vec<Predicate>,
    observer: Option<Observer>,
}

impl Server {
    /// Serve the registry at the application endpoint selected by the environment.
    pub fn new(ui: &Ui) -> Self {
        Self {
            ui: ui.clone(),
            endpoint: Endpoint::from_env(),
            commands: HashMap::new(),
            predicates: Vec::new(),
            observer: None,
        }
    }
    /// Select an explicit endpoint.
    pub fn endpoint(mut self, endpoint: Endpoint) -> Self {
        self.endpoint = endpoint;
        self
    }
    /// Register a first-word command. The handler receives the remaining text.
    pub fn command(
        mut self,
        name: &'static str,
        handler: impl Fn(&str) -> String + Send + Sync + 'static,
    ) -> Self {
        self.commands.insert(name, Box::new(handler));
        self
    }
    /// Require the predicate for agent-originated actions on the registry while serving.
    pub fn allow(mut self, f: impl Fn(&ActionRequest<'_>) -> bool + Send + Sync + 'static) -> Self {
        self.predicates.push(Box::new(f));
        self
    }
    /// Observe each reply after writing it to the connection.
    pub fn observe(mut self, f: impl Fn(&RequestLog<'_>) + Send + Sync + 'static) -> Self {
        self.observer = Some(Box::new(f));
        self
    }
    /// Bind the endpoint and begin accepting connections on a background thread.
    ///
    /// # Errors
    /// Returns an error if binding or starting the thread fails.
    pub fn spawn(self) -> Result<ServerGuard, IpcError> {
        let listener = self.endpoint.bind()?;
        let mut policy = Agent::new(&self.ui);
        let predicates = self.predicates;
        if !predicates.is_empty() {
            policy = policy.allow(move |req| predicates.iter().all(|p| p(req)));
        }
        let state = Arc::new(State {
            ui: self.ui,
            commands: self.commands,
            observer: self.observer,
            _policy: policy,
            stopped: AtomicBool::new(false),
        });
        let shared = Arc::clone(&state);
        let accept = thread::Builder::new().spawn(move || {
            let mut id = 0;
            loop {
                let stream = match listener.accept() {
                    Ok(stream) => stream,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                };
                if shared.stopped.load(Ordering::Acquire) {
                    break;
                }
                id += 1;
                let state = Arc::downgrade(&shared);
                let agent = Agent::new(&shared.ui);
                let _ = thread::Builder::new().spawn(move || serve(stream, id, &agent, &state));
            }
        })?;
        Ok(ServerGuard {
            endpoint: self.endpoint,
            state,
            accept: Some(accept),
        })
    }
}

struct State {
    ui: Ui,
    commands: HashMap<&'static str, Handler>,
    observer: Option<Observer>,
    // An in-flight request retains its authorization through dispatch.
    _policy: Agent,
    stopped: AtomicBool,
}

fn serve(mut stream: Stream, id: u64, agent: &Agent, state: &Weak<State>) {
    loop {
        let request = frame::read(&mut stream);
        let Some(state) = state.upgrade() else { return };
        if state.stopped.load(Ordering::Acquire) {
            return;
        }
        let start = Instant::now();
        let (request, mut reply, mut close) = match request {
            Ok(request) => {
                let trimmed = request.trim();
                let (name, rest) = trimmed
                    .split_once(char::is_whitespace)
                    .unwrap_or((trimmed, ""));
                let reply = state
                    .commands
                    .get(name)
                    .map_or_else(|| agent.handle(&request), |f| f(rest.trim()));
                (request, reply, false)
            }
            Err(e @ IpcError::FrameTooLarge(_)) => (String::new(), format!("err {e}"), true),
            Err(_) => return,
        };
        if reply.len() > frame::MAX {
            reply = format!("err {}", IpcError::FrameTooLarge(reply.len()));
            close = true;
        }
        if frame::write(&mut stream, &reply).is_err() {
            return;
        }
        if let Some(observer) = &state.observer {
            observer(&RequestLog {
                request: &request,
                reply: &reply,
                elapsed: start.elapsed(),
                connection: id,
            });
        }
        if close {
            return;
        }
    }
}

/// Owns the listener and its action policy. Drop to stop serving and reclaim the endpoint.
pub struct ServerGuard {
    endpoint: Endpoint,
    state: Arc<State>,
    accept: Option<thread::JoinHandle<()>>,
}
impl ServerGuard {
    /// Endpoint bound by the listener.
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }
}
impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.state.stopped.store(true, Ordering::Release);
        if let Some(thread) = self.accept.take() {
            // Keep the wake connection open until the listener accepts it on Windows.
            let _wake = Client::connect(&self.endpoint);
            let _ = thread.join();
        }
    }
}

#[cfg(all(unix, test))]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        os::unix::net::UnixStream,
    };

    #[test]
    fn consecutive_frames_then_oversized_header() -> Result<(), IpcError> {
        let (mut client, server) = UnixStream::pair()?;
        let ui = Ui::new();
        let state = Arc::new(State {
            _policy: Agent::new(&ui),
            ui,
            commands: HashMap::new(),
            observer: None,
            stopped: AtomicBool::new(false),
        });
        let worker_state = Arc::downgrade(&state);
        let session = Agent::new(&state.ui);
        let worker = thread::spawn(move || {
            serve(Stream::UdSocket(server.into()), 1, &session, &worker_state);
        });
        // Pipeline complete frames and an invalid header without waiting for replies.
        frame::write(&mut client, "snapshot")?;
        frame::write(&mut client, "diff")?;
        client.write_all(&u32::MAX.to_le_bytes())?;
        assert_eq!(frame::read(&mut client)?, "rev 0");
        assert_eq!(frame::read(&mut client)?, "rev 0");
        assert!(frame::read(&mut client)?.starts_with("err frame too large"));
        assert_eq!(client.read(&mut [0])?, 0);
        assert!(worker.join().is_ok());
        Ok(())
    }
}
