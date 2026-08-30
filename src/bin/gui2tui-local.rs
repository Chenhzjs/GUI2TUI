use std::{collections::HashSet, fs, path::PathBuf, process::ExitCode, time::Duration};

use clap::{Parser, Subcommand, ValueEnum};
use gui2tui::modality::{
    ArtifactDescriptor, ArtifactHash, ArtifactId, ArtifactLifetime, ArtifactTransport,
    AuthorizationDecision, CancellationToken, HandlerRegistry, LocalModalityBroker,
    LocalModalityCapabilities, ModalityKind, PathMapping, ProcessHandler, RecordingHandler,
    ReferenceProvenance, ReferencedResource, ResourceReference,
};

#[derive(Debug, Parser)]
#[command(about = "User-controlled local broker for GUI2TUI modality handoff")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Capabilities,
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
        #[arg(long, value_enum, default_value_t = Authorization::Once)]
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
        #[arg(long, value_enum, default_value_t = Authorization::Once)]
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
    if matches!(cli.command, Command::Capabilities) {
        println!("reference_schemes=http,https,file,mapped-path");
        println!("mime_patterns=image/*,application/pdf,video/*,audio/*,model/*");
        println!("artifact_receive=true");
        println!("executable_paths_disclosed=false");
        return Ok(());
    }

    let temp_root = std::env::temp_dir().join(format!("gui2tui-local-{}", std::process::id()));
    match cli.command {
        Command::Capabilities => unreachable!(),
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
                ResourceReference::MappedPath {
                    remote,
                    local: destination.display().to_string(),
                }
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
            let bytes = fs::read(&input)?;
            let descriptor = ArtifactDescriptor {
                id: ArtifactId::new(1),
                kind: kind.into(),
                mime: mime.clone(),
                size: bytes.len() as u64,
                hash: ArtifactHash::sha256(&bytes),
                display_name: input
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned()),
                lifetime: ArtifactLifetime::Temporary {
                    ttl: Duration::from_secs(300),
                },
            };
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
                bytes.as_slice(),
                authorization.into(),
                &cancellation,
            )?;
            println!("status=opened reference_only=0 artifact_bytes={transferred}");
            if let Some(recorder) = recorder {
                println!("handler_invocations={}", recorder.invocations().len());
            }
        }
    }
    let _ = fs::remove_dir_all(temp_root);
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
