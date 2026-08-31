use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::PathBuf,
    process::ExitCode,
    time::Duration,
};

use clap::{Parser, Subcommand, ValueEnum};
use gui2tui::modality::{
    ArtifactDescriptor, ArtifactHash, ArtifactId, ArtifactLifetime, ArtifactTransport,
    AuthorizationDecision, CancellationToken, HandlerRegistry, LocalModalityBroker,
    LocalModalityCapabilities, ModalityKind, PathMapping, ProcessHandler, RecordingHandler,
    ReferenceProvenance, ReferencedResource, ResourceReference,
};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "User-controlled local broker for GUI2TUI modality handoff"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Capabilities,
    /// Receive one authorized operation at a time on a private local socket.
    Serve {
        #[arg(long)]
        socket: PathBuf,
        /// MIME allowlist/registry entries owned exclusively by the local user.
        #[arg(long, required = true)]
        mime: Vec<String>,
        #[arg(long, conflicts_with = "recording_handler")]
        handler_program: Option<PathBuf>,
        /// Explicit test mode: records invocation, does not start a viewer.
        #[arg(long)]
        recording_handler: bool,
        /// Omit to prompt on the local controlling terminal for every new grant.
        #[arg(long, value_enum)]
        authorization: Option<Authorization>,
        /// Explicit local source-prefix=destination-prefix mappings.
        #[arg(long)]
        map: Vec<String>,
        #[arg(long, default_value_t = 0)]
        max_requests: usize,
        #[arg(long, default_value_t = 512 * 1024 * 1024)]
        max_bytes: u64,
        #[arg(long, default_value_t = 300)]
        timeout_secs: u64,
    },
    /// Diagnostic producer: send only this explicit portable artifact after approval.
    SendArtifact {
        #[arg(long)]
        socket: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        mime: String,
        #[arg(long, value_enum)]
        kind: Kind,
        #[arg(long)]
        cancel_before_transfer: bool,
    },
    Reference {
        #[arg(long)]
        uri: Option<String>,
        #[arg(long)]
        path: Option<String>,
        #[arg(long)]
        mapped_path: Option<String>,
        #[arg(long, requires = "mapped_path")]
        map_source: Option<String>,
        #[arg(long, requires = "mapped_path")]
        map_destination: Option<PathBuf>,
        #[arg(long)]
        mime: String,
        #[arg(long, value_enum)]
        kind: Kind,
        #[arg(long, value_enum, default_value_t = Authorization::Deny)]
        authorization: Authorization,
        /// Locally configured handler. This is never accepted from a server descriptor.
        #[arg(long)]
        handler_program: Option<PathBuf>,
    },
    Artifact {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        mime: String,
        #[arg(long, value_enum)]
        kind: Kind,
        #[arg(long, value_enum, default_value_t = Authorization::Deny)]
        authorization: Authorization,
        /// Locally configured handler. This is never part of the artifact descriptor.
        #[arg(long)]
        handler_program: Option<PathBuf>,
        #[arg(long, default_value_t = 512 * 1024 * 1024)]
        max_bytes: u64,
        #[arg(long)]
        cancel_before_transfer: bool,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Authorization {
    Once,
    Session,
    Deny,
}

impl From<Authorization> for AuthorizationDecision {
    fn from(value: Authorization) -> Self {
        match value {
            Authorization::Once => Self::Once,
            Authorization::Session => Self::Session,
            Authorization::Deny => Self::Deny,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum Kind {
    Image,
    Document,
    Video,
    Audio,
    VectorGraphic,
    PortableModel,
}

impl From<Kind> for ModalityKind {
    fn from(value: Kind) -> Self {
        match value {
            Kind::Image => Self::Image,
            Kind::Document => Self::Document,
            Kind::Video => Self::Video,
            Kind::Audio => Self::Audio,
            Kind::VectorGraphic => Self::VectorGraphic,
            Kind::PortableModel => Self::PortableModel,
        }
    }
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let recovered = gui2tui::runtime::artifacts::recover_abandoned()?;
    if recovered != 0 {
        eprintln!("recovered_artifact_namespaces={recovered}");
    }
    if matches!(cli.command, Command::Capabilities) {
        println!("No connected broker; handlers must be explicitly configured with serve.");
        println!("reference_schemes=[] mime_patterns=[] artifact_receive=false");
        println!("executable_paths_disclosed=false");
        return Ok(());
    }

    let temp_root = gui2tui::product::paths::runtime_dir()?;
    match cli.command {
        Command::Capabilities => unreachable!(),
        Command::Serve {
            socket,
            mime,
            handler_program,
            recording_handler,
            authorization,
            map,
            max_requests,
            max_bytes,
            timeout_secs,
        } => {
            use gui2tui::modality::wire::{self, LocalSocket, Request};
            let mut registry = HandlerRegistry::default();
            let recorder = RecordingHandler::default();
            for pattern in &mime {
                if recording_handler {
                    registry.register(pattern, Box::new(recorder.clone()));
                } else if let Some(program) = &handler_program {
                    registry.register(
                        pattern,
                        Box::new(ProcessHandler::configured_locally(program)),
                    );
                }
            }
            let caps = LocalModalityCapabilities {
                reference_schemes: HashSet::from([
                    "http".to_owned(),
                    "https".to_owned(),
                    "file".to_owned(),
                    "mapped-path".to_owned(),
                ]),
                mime_patterns: registry.mime_patterns(),
                artifact_receive: true,
            };
            let mut broker = LocalModalityBroker::new(caps, registry, temp_root)?;
            for mapping in map {
                let (source, destination) = mapping
                    .split_once('=')
                    .ok_or("mapping requires SOURCE=DESTINATION")?;
                broker.add_mapping(PathMapping::new(source, destination)?);
            }
            let listener = LocalSocket::bind(&socket)?;
            println!("broker ready; view-only; executable configuration stays local");
            let stop = CancellationToken::default();
            let signal = stop.clone();
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.spawn(async move {
                if let Ok(mut signals) = gui2tui::runtime::signals::RuntimeSignals::new() {
                    loop {
                        if matches!(
                            signals.recv().await,
                            gui2tui::runtime::signals::RuntimeSignal::Stop
                        ) {
                            signal.cancel();
                            break;
                        }
                    }
                }
            });
            let transport = ArtifactTransport {
                max_size: max_bytes,
                timeout: Duration::from_secs(timeout_secs.max(1)),
                ..Default::default()
            };
            listener.run(
                &mut broker,
                &transport,
                max_requests,
                &stop,
                |request, session_grant| {
                    if let Some(decision) = authorization {
                        return decision.into();
                    }
                    if session_grant {
                        return AuthorizationDecision::Once;
                    }
                    let summary = match request {
                        Request::Reference { kind, resource } => format!(
                            "{kind:?} MIME={:?} label={:?} reference={:?}",
                            resource.mime, resource.display_name, resource.reference
                        ),
                        Request::Artifact { descriptor } => format!(
                            "{:?} origin={:?} MIME={:?} bytes={}",
                            descriptor.kind, descriptor.origin, descriptor.mime, descriptor.size
                        ),
                        _ => return AuthorizationDecision::Deny,
                    };
                    prompt_authorization(&summary, &stop)
                },
            )?;
            wire::print_metrics(broker.metrics());
            if recording_handler {
                println!("recorded_invocations={}", recorder.invocations().len());
            }
            return Ok(());
        }
        Command::SendArtifact {
            socket,
            input,
            mime,
            kind,
            cancel_before_transfer,
        } => {
            let (descriptor, mut file) = describe_file(&input, mime, kind.into())?;
            let cancellation = CancellationToken::default();
            if cancel_before_transfer {
                cancellation.cancel();
            }
            let (result, bytes) = gui2tui::modality::wire::send_artifact(
                &socket,
                descriptor,
                &mut file,
                &cancellation,
            )?;
            println!("{result:?} payload_sent={bytes}");
            if matches!(result, gui2tui::modality::wire::Response::Failed { .. }) {
                return Err("handoff failed".into());
            }
            return Ok(());
        }
        Command::Reference {
            uri,
            path,
            mapped_path,
            map_source,
            map_destination,
            mime,
            kind,
            authorization,
            handler_program,
        } => {
            let (mut broker, recorder) = broker(&temp_root, &mime, handler_program)?;
            let reference = if let Some(uri) = uri {
                ResourceReference::NetworkUri(uri)
            } else if let Some(path) = path {
                ResourceReference::LocalPath(path)
            } else if let Some(remote) = mapped_path {
                let source = map_source.as_deref().ok_or("--map-source is required")?;
                let destination = map_destination.ok_or("--map-destination is required")?;
                broker.add_mapping(PathMapping::new(source, &destination)?);
                ResourceReference::MappedPath { remote }
            } else {
                return Err("one of --uri, --path, or --mapped-path is required".into());
            };
            let resource = ReferencedResource {
                reference,
                mime: Some(mime.clone()),
                display_name: Some("remote resource".to_owned()),
                provenance: if map_source.is_some() {
                    ReferenceProvenance::UserConfiguredMapping
                } else {
                    ReferenceProvenance::HyperlinkUri
                },
            };
            broker.handoff_reference(kind.into(), &resource, authorization.into())?;
            let metrics = broker.metrics();
            println!(
                "status=opened reference_only={} artifact_bytes={}",
                metrics.reference_only_handoffs, metrics.artifact_bytes
            );
            if let Some(recorder) = recorder {
                println!("handler_invocations={}", recorder.invocations().len());
            }
        }
        Command::Artifact {
            input,
            mime,
            kind,
            authorization,
            handler_program,
            max_bytes,
            cancel_before_transfer,
        } => {
            let (descriptor, file) = describe_file(&input, mime.clone(), kind.into())?;
            println!(
                "descriptor mime={} size={} sha256={} authorized_payload=false",
                descriptor.mime,
                descriptor.size,
                descriptor.hash.hex()
            );
            let (mut broker, recorder) = broker(&temp_root, &mime, handler_program)?;
            let cancellation = CancellationToken::default();
            if cancel_before_transfer {
                cancellation.cancel();
            }
            let transport = ArtifactTransport {
                max_size: max_bytes,
                ..Default::default()
            };
            let transferred = transport.transfer(
                &mut broker,
                &descriptor,
                file,
                authorization.into(),
                &cancellation,
            )?;
            println!("status=opened reference_only=0 artifact_bytes={transferred}");
            if let Some(recorder) = recorder {
                println!("handler_invocations={}", recorder.invocations().len());
            }
        }
    }
    // OwnedArtifactDirectory cleans its operation namespace only. The runtime
    // root can be shared by live sessions and must never be recursively removed.
    Ok(())
}

fn broker(
    root: &PathBuf,
    mime: &str,
    handler_program: Option<PathBuf>,
) -> Result<(LocalModalityBroker, Option<RecordingHandler>), Box<dyn std::error::Error>> {
    let mut registry = HandlerRegistry::default();
    let recorder = if let Some(program) = handler_program {
        registry.register(mime, Box::new(ProcessHandler::configured_locally(program)));
        None
    } else {
        let recorder = RecordingHandler::default();
        registry.register(mime, Box::new(recorder.clone()));
        Some(recorder)
    };
    let capabilities = LocalModalityCapabilities {
        reference_schemes: HashSet::from([
            "http".to_owned(),
            "https".to_owned(),
            "file".to_owned(),
            "mapped-path".to_owned(),
        ]),
        mime_patterns: HashSet::from([mime.to_owned()]),
        artifact_receive: true,
    };
    Ok((
        LocalModalityBroker::new(capabilities, registry, root)?,
        recorder,
    ))
}

fn describe_file(
    input: &PathBuf,
    mime: String,
    kind: ModalityKind,
) -> Result<(ArtifactDescriptor, fs::File), Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};
    use std::io::{Seek, SeekFrom};
    let mut file = fs::File::open(input)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err("artifact source must be a regular file".into());
    }
    if metadata.len() > 512 * 1024 * 1024 {
        return Err("artifact exceeds 512 MiB producer limit".into());
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0; 65536];
    let mut size = 0;
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size += count as u64;
        if size > 512 * 1024 * 1024 {
            return Err("artifact grew beyond limit".into());
        }
        hasher.update(&buffer[..count]);
    }
    file.seek(SeekFrom::Start(0))?;
    Ok((
        ArtifactDescriptor {
            origin: Default::default(),
            id: ArtifactId::new(1),
            kind,
            mime,
            size,
            hash: ArtifactHash(hasher.finalize().into()),
            display_name: input.file_name().map(|s| s.to_string_lossy().into_owned()),
            lifetime: ArtifactLifetime::Session,
        },
        file,
    ))
}

fn prompt_authorization(summary: &str, stop: &CancellationToken) -> AuthorizationDecision {
    use std::io::BufRead;
    let Ok(mut tty) = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
    else {
        return AuthorizationDecision::Deny;
    };
    let _ = writeln!(
        tty,
        "View locally: {summary}\n[o] Once / [s] Session / [d] Deny (default):"
    );
    let (send, receive) = std::sync::mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let mut answer = String::new();
        let result = std::io::BufReader::new(tty).take(16).read_line(&mut answer);
        let _ = send.send(result.map(|_| answer));
    });
    let answer = loop {
        if stop.is_cancelled() {
            return AuthorizationDecision::Deny;
        }
        match receive.recv_timeout(Duration::from_millis(100)) {
            Ok(Ok(answer)) => break answer,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            _ => return AuthorizationDecision::Deny,
        }
    };
    match answer.trim() {
        "o" => AuthorizationDecision::Once,
        "s" => AuthorizationDecision::Session,
        _ => AuthorizationDecision::Deny,
    }
}
