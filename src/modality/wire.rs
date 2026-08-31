//! One operation per private Unix socket connection. Control frames are
//! length-prefixed JSON (64 KiB max). Artifact bytes follow only Approved;
//! the sender half-closes its write side to delimit exactly one payload.
//! No remote executable, directory operation, download, or resume protocol.

use super::{
    ArtifactDescriptor, ArtifactTransport, AuthorizationDecision, CancellationToken,
    HandoffMetrics, LocalModalityBroker, LocalModalityCapabilities, ModalityKind,
    ReferencedResource,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAX_CONTROL: usize = 64 * 1024;
const IO_POLL: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "request", deny_unknown_fields)]
pub enum Request {
    Capabilities {},
    Reference {
        kind: ModalityKind,
        resource: ReferencedResource,
    },
    Artifact {
        descriptor: ArtifactDescriptor,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", deny_unknown_fields)]
pub enum Response {
    Capabilities {
        capabilities: LocalModalityCapabilities,
    },
    Approved,
    Opened {
        reference_only: bool,
        artifact_bytes: u64,
    },
    Failed {
        reason: String,
        artifact_bytes: u64,
    },
}

pub fn write_control<T: Serialize>(stream: &mut impl Write, value: &T) -> io::Result<()> {
    let bytes = serde_json::to_vec(value).map_err(io::Error::other)?;
    if bytes.len() > MAX_CONTROL {
        return Err(io::Error::other("control frame too large"));
    }
    stream.write_all(&(bytes.len() as u32).to_be_bytes())?;
    stream.write_all(&bytes)?;
    stream.flush()
}

pub fn read_control<T: DeserializeOwned>(stream: &mut impl Read) -> io::Result<T> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = u32::from_be_bytes(length) as usize;
    if length > MAX_CONTROL {
        return Err(io::Error::other("control frame too large"));
    }
    let mut bytes = vec![0; length];
    stream.read_exact(&mut bytes)?;
    // Never include serde's value-containing error in a remote/log diagnostic.
    serde_json::from_slice(&bytes).map_err(|_| io::Error::other("invalid control descriptor"))
}

pub struct LocalSocket {
    listener: UnixListener,
    path: PathBuf,
}

impl LocalSocket {
    /// The caller creates/owns a private (0700) directory. Never unlink an
    /// existing socket; bind failure must not replace another broker.
    pub fn bind(path: &Path) -> io::Result<Self> {
        let parent = path
            .parent()
            .ok_or_else(|| io::Error::other("socket needs a parent directory"))?;
        let metadata = std::fs::metadata(parent)?;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(io::Error::other(
                "broker socket directory must be private (0700)",
            ));
        }
        let listener = UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(Self {
            listener,
            path: path.to_owned(),
        })
    }

    pub fn run(
        &self,
        broker: &mut LocalModalityBroker,
        transport: &ArtifactTransport,
        max_requests: usize,
        stop: &CancellationToken,
        mut authorize: impl FnMut(&Request, bool) -> AuthorizationDecision,
    ) -> io::Result<()> {
        self.listener.set_nonblocking(true)?;
        let mut count = 0;
        while !stop.is_cancelled() && (max_requests == 0 || count < max_requests) {
            broker.cleanup_expired();
            match self.listener.accept() {
                Ok((stream, _)) => {
                    count += 1;
                    if let Err(error) =
                        serve_connection(stream, broker, transport, stop, &mut authorize)
                    {
                        // Error kind only: descriptors/URIs never enter ordinary logs.
                        eprintln!("modality connection failed: {:?}", error.kind());
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(20))
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }
}

impl Drop for LocalSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub fn serve_connection(
    mut stream: UnixStream,
    broker: &mut LocalModalityBroker,
    transport: &ArtifactTransport,
    stop: &CancellationToken,
    authorize: &mut impl FnMut(&Request, bool) -> AuthorizationDecision,
) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_POLL))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    let request: Request = read_control(&mut DeadlineReader::new(
        &stream,
        Duration::from_secs(5),
        stop,
    ))?;
    if matches!(request, Request::Capabilities {}) {
        return write_control(
            &mut stream,
            &Response::Capabilities {
                capabilities: broker.capabilities().clone(),
            },
        );
    }
    let (kind, mime) = match &request {
        Request::Reference { kind, resource } => (*kind, resource.mime.as_deref().unwrap_or("")),
        Request::Artifact { descriptor } => (descriptor.kind, descriptor.mime.as_str()),
        Request::Capabilities {} => unreachable!(),
    };
    let decision = authorize(&request, broker.session_allows(kind, mime));
    let before = broker.metrics().artifact_bytes;
    let result = match request {
        Request::Reference { kind, resource } => broker
            .handoff_reference(kind, &resource, decision)
            .map(|_| true)
            .map_err(|_| "Reference denied, unsupported, or local handler failed"),
        Request::Artifact { descriptor } => {
            if descriptor.size > transport.max_size
                || broker.authorize_artifact(&descriptor, decision).is_err()
            {
                Err("Artifact denied or unavailable; payload not requested")
            } else {
                write_control(&mut stream, &Response::Approved)?;
                let reader = DeadlineReader::new(&stream, transport.timeout, stop);
                transport.transfer(broker, &descriptor, reader, AuthorizationDecision::Once, stop)
                    .map(|_| false).map_err(|_| "Artifact failed integrity, length, timeout, cancellation, or local handling")
            }
        }
        Request::Capabilities {} => unreachable!(),
    };
    let bytes = broker.metrics().artifact_bytes - before;
    let response = match result {
        Ok(reference_only) => Response::Opened {
            reference_only,
            artifact_bytes: bytes,
        },
        Err(reason) => Response::Failed {
            reason: reason.to_owned(),
            artifact_bytes: bytes,
        },
    };
    write_control(&mut stream, &response)
}

struct DeadlineReader<'a> {
    stream: &'a UnixStream,
    deadline: Instant,
    stop: &'a CancellationToken,
}

impl<'a> DeadlineReader<'a> {
    fn new(stream: &'a UnixStream, timeout: Duration, stop: &'a CancellationToken) -> Self {
        Self {
            stream,
            deadline: Instant::now() + timeout,
            stop,
        }
    }
}

impl Read for DeadlineReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            if self.stop.is_cancelled() {
                return Err(io::Error::other("transfer cancelled"));
            }
            if Instant::now() >= self.deadline {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "transfer deadline"));
            }
            match self.stream.read(buffer) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    continue;
                }
                result => return result,
            }
        }
    }
}

fn connect(socket: &Path) -> io::Result<UnixStream> {
    let stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(60)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    Ok(stream)
}

pub fn capabilities(socket: &Path) -> io::Result<LocalModalityCapabilities> {
    let mut stream = connect(socket)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    write_control(&mut stream, &Request::Capabilities {})?;
    match read_control(&mut stream)? {
        Response::Capabilities { capabilities } => Ok(capabilities),
        _ => Err(io::Error::other("unexpected capability response")),
    }
}

pub fn send_reference(
    socket: &Path,
    kind: ModalityKind,
    resource: ReferencedResource,
) -> io::Result<Response> {
    send_reference_cancellable(socket, kind, resource, &CancellationToken::default())
}

pub fn send_reference_cancellable(
    socket: &Path,
    kind: ModalityKind,
    resource: ReferencedResource,
    cancel: &CancellationToken,
) -> io::Result<Response> {
    let mut stream = connect(socket)?;
    stream.set_read_timeout(Some(IO_POLL))?;
    write_control(&mut stream, &Request::Reference { kind, resource })?;
    read_control(&mut DeadlineReader::new(
        &stream,
        Duration::from_secs(60),
        cancel,
    ))
}

pub fn send_artifact(
    socket: &Path,
    descriptor: ArtifactDescriptor,
    payload: &mut impl Read,
    cancel: &CancellationToken,
) -> io::Result<(Response, u64)> {
    let mut stream = connect(socket)?;
    write_control(
        &mut stream,
        &Request::Artifact {
            descriptor: descriptor.clone(),
        },
    )?;
    let response: Response = read_control(&mut stream)?;
    if !matches!(response, Response::Approved) {
        return Ok((response, 0));
    }
    let mut bytes = 0;
    let mut buffer = [0; 64 * 1024];
    while bytes < descriptor.size {
        if cancel.is_cancelled() {
            break;
        }
        let limit = buffer.len().min((descriptor.size - bytes) as usize);
        let count = payload.read(&mut buffer[..limit])?;
        if count == 0 {
            break;
        }
        stream.write_all(&buffer[..count])?;
        bytes += count as u64;
    }
    stream.shutdown(Shutdown::Write)?;
    Ok((read_control(&mut stream)?, bytes))
}

pub fn print_metrics(metrics: HandoffMetrics) {
    println!(
        "reference_only={} artifact_bytes={} denied={} cancelled={} handler_unavailable={}",
        metrics.reference_only_handoffs,
        metrics.artifact_bytes,
        metrics.authorization_denied,
        metrics.transfer_cancelled,
        metrics.handler_unavailable
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modality::{
        ArtifactHash, ArtifactId, ArtifactLifetime, HandlerRegistry, LocalResource,
        RecordingHandler,
    };
    use std::{collections::HashSet, thread};

    fn setup() -> (
        tempfile::TempDir,
        LocalModalityBroker,
        RecordingHandler,
        ArtifactDescriptor,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let handler = RecordingHandler::default();
        let mut registry = HandlerRegistry::default();
        registry.register("image/*", Box::new(handler.clone()));
        let broker = LocalModalityBroker::new(
            LocalModalityCapabilities {
                reference_schemes: HashSet::from(["https".into()]),
                mime_patterns: HashSet::from(["image/*".into(), "video/*".into()]),
                artifact_receive: true,
            },
            registry,
            dir.path(),
        )
        .unwrap();
        let descriptor = ArtifactDescriptor {
            id: ArtifactId::new(1),
            kind: ModalityKind::Image,
            mime: "image/svg+xml".into(),
            size: 6,
            hash: ArtifactHash::sha256(b"<svg/>"),
            display_name: Some("../../unsafe.sh".into()),
            lifetime: ArtifactLifetime::Session,
        };
        (dir, broker, handler, descriptor)
    }

    #[test]
    fn socket_capabilities_advertise_only_configured_handlers() {
        let (_dir, mut broker, _, _) = setup();
        let (mut client, server) = UnixStream::pair().unwrap();
        let task = thread::spawn(move || {
            serve_connection(
                server,
                &mut broker,
                &ArtifactTransport::default(),
                &CancellationToken::default(),
                &mut |_, _| panic!("no authorization for capability query"),
            )
        });
        write_control(&mut client, &Request::Capabilities {}).unwrap();
        let Response::Capabilities { capabilities } = read_control(&mut client).unwrap() else {
            panic!()
        };
        assert!(capabilities.supports_mime("image/png"));
        assert!(!capabilities.supports_mime("video/mp4"));
        let json = serde_json::to_string(&capabilities).unwrap();
        assert!(!json.contains("executable"));
        task.join().unwrap().unwrap();
    }

    #[test]
    fn denied_socket_descriptor_never_requests_or_reads_payload() {
        let (_dir, mut broker, handler, descriptor) = setup();
        let (mut client, server) = UnixStream::pair().unwrap();
        let task = thread::spawn(move || {
            serve_connection(
                server,
                &mut broker,
                &ArtifactTransport::default(),
                &CancellationToken::default(),
                &mut |_, _| AuthorizationDecision::Deny,
            )
            .unwrap();
            assert_eq!(broker.metrics().artifact_bytes, 0);
        });
        write_control(&mut client, &Request::Artifact { descriptor }).unwrap();
        assert!(matches!(
            read_control::<Response>(&mut client).unwrap(),
            Response::Failed {
                artifact_bytes: 0,
                ..
            }
        ));
        task.join().unwrap();
        assert!(handler.invocations().is_empty());
    }

    #[test]
    fn socket_artifact_integrity_length_and_unique_temporary_lifecycle() {
        let (_dir, mut broker, handler, descriptor) = setup();
        for bytes in [
            b"<svg/>".as_slice(),
            b"<svg/>".as_slice(),
            b"broken".as_slice(),
            b"short".as_slice(),
        ] {
            let (mut client, server) = UnixStream::pair().unwrap();
            thread::scope(|scope| {
                let task = scope.spawn(|| {
                    serve_connection(
                        server,
                        &mut broker,
                        &ArtifactTransport::default(),
                        &CancellationToken::default(),
                        &mut |_, _| AuthorizationDecision::Once,
                    )
                });
                write_control(
                    &mut client,
                    &Request::Artifact {
                        descriptor: descriptor.clone(),
                    },
                )
                .unwrap();
                assert!(matches!(
                    read_control::<Response>(&mut client).unwrap(),
                    Response::Approved
                ));
                client.write_all(bytes).unwrap();
                client.shutdown(Shutdown::Write).unwrap();
                let response: Response = read_control(&mut client).unwrap();
                assert_eq!(
                    matches!(response, Response::Opened { .. }),
                    bytes == b"<svg/>"
                );
                task.join().unwrap().unwrap();
            });
        }
        let calls = handler.invocations();
        assert_eq!(calls.len(), 2);
        assert_ne!(calls[0].0, calls[1].0);
        for (resource, _) in &calls {
            let LocalResource::Path(path) = resource else {
                panic!()
            };
            assert_eq!(std::fs::read(path).unwrap(), b"<svg/>");
        }
        broker.cleanup();
        for (resource, _) in calls {
            let LocalResource::Path(path) = resource else {
                panic!()
            };
            assert!(!path.exists());
        }
    }

    #[test]
    fn stalled_socket_payload_has_real_deadline_and_no_handler_call() {
        let (_dir, mut broker, handler, descriptor) = setup();
        let (mut client, server) = UnixStream::pair().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let task = thread::spawn(move || {
            serve_connection(
                server,
                &mut broker,
                &ArtifactTransport {
                    timeout: Duration::from_millis(150),
                    ..Default::default()
                },
                &CancellationToken::default(),
                &mut |_, _| AuthorizationDecision::Once,
            )
        });
        write_control(&mut client, &Request::Artifact { descriptor }).unwrap();
        assert!(matches!(
            read_control::<Response>(&mut client).unwrap(),
            Response::Approved
        ));
        let start = Instant::now();
        assert!(matches!(
            read_control::<Response>(&mut client).unwrap(),
            Response::Failed {
                artifact_bytes: 0,
                ..
            }
        ));
        assert!(start.elapsed() < Duration::from_secs(2));
        task.join().unwrap().unwrap();
        assert!(handler.invocations().is_empty());
    }

    #[test]
    fn oversized_control_and_remote_command_are_rejected() {
        assert!(
            read_control::<Request>(&mut (MAX_CONTROL as u32 + 1).to_be_bytes().as_slice())
                .is_err()
        );
        let bytes = br#"{"request":"Capabilities","command":"/bin/sh"}"#;
        let mut framed = (bytes.len() as u32).to_be_bytes().to_vec();
        framed.extend_from_slice(bytes);
        assert!(read_control::<Request>(&mut framed.as_slice()).is_err());
    }

    #[test]
    fn socket_cancellation_interrupts_blocked_payload_read() {
        let (_dir, mut broker, handler, descriptor) = setup();
        let (mut client, server) = UnixStream::pair().unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let stop = CancellationToken::default();
        let server_stop = stop.clone();
        let task = thread::spawn(move || {
            serve_connection(
                server,
                &mut broker,
                &ArtifactTransport::default(),
                &server_stop,
                &mut |_, _| AuthorizationDecision::Once,
            )
        });
        write_control(&mut client, &Request::Artifact { descriptor }).unwrap();
        assert!(matches!(
            read_control::<Response>(&mut client).unwrap(),
            Response::Approved
        ));
        stop.cancel();
        assert!(matches!(
            read_control::<Response>(&mut client).unwrap(),
            Response::Failed { .. }
        ));
        task.join().unwrap().unwrap();
        assert!(handler.invocations().is_empty());
    }
}
