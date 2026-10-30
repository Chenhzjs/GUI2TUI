//! Development JSON-lines interface. One persistent runtime owns all authority.
use clap::Parser;
use gui2tui::{
    backend::{AtspiBackend, BootstrapStrategy, InspectOptions},
    product::headless::{SessionChoice, select_at_process_start},
    semantic::BackendLocator,
    transcompile::PresentationMode,
    tui::app::{AgentObservation, TuiApplication},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    error::Error,
    io::{self, BufRead, Write},
    time::Duration,
};

#[derive(Parser)]
#[command(about = "Development Accessibility observation and exact-operation JSONL bridge")]
struct Cli {
    #[arg(long, required_unless_present = "application_id")]
    app: Option<String>,
    /// Exact public application identity; never rebind by application name.
    #[arg(long, conflicts_with = "app", required_unless_present = "app", value_parser = parse_application_id)]
    application_id: Option<BackendLocator>,
    /// Explicit experimental Human spatial TUI projection; full exports remain available.
    #[arg(long)]
    spatial_tui: bool,
    /// Independent sibling metadata reads; never limits exported nodes.
    #[arg(long, default_value_t = 4, value_parser = clap::value_parser!(u16).range(1..=32))]
    ax_read_concurrency: u16,
    /// Optional shared research organization; does not change facts or authority.
    #[arg(long, value_enum, default_value = "off")]
    decision_annotations: gui2tui::transcompile::document::annotations::AnnotationMode,
    #[arg(long, value_enum)]
    session: Option<SessionChoice>,
    #[arg(long, default_value_t = 10000)]
    max_nodes: usize,
    #[arg(long, default_value_t = 64)]
    max_depth: usize,
    #[arg(long, default_value_t = 5000)]
    timeout_ms: u64,
    #[arg(long, default_value_t = 262144)]
    max_edit_bytes: usize,
}
fn parse_application_id(value: &str) -> Result<BackendLocator, String> {
    BackendLocator::decode(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod exact_application_tests {
    use super::*;

    #[test]
    fn exact_locator_and_name_are_explicit_alternatives() {
        let locator = BackendLocator::new(":1.42", "/org/a11y/atspi/accessible/root");
        let encoded = locator.encode();
        let exact = Cli::try_parse_from(["gui2tui-agent", "--application-id", &encoded]).unwrap();
        assert_eq!(exact.application_id, Some(locator));
        assert!(
            Cli::try_parse_from([
                "gui2tui-agent",
                "--app",
                "old name",
                "--application-id",
                &encoded
            ])
            .is_err()
        );
        assert!(Cli::try_parse_from(["gui2tui-agent", "--application-id", "invalid"]).is_err());
        assert!(Cli::try_parse_from(["gui2tui-agent"]).is_err());
        assert!(Cli::try_parse_from(["gui2tui-agent", "--app", "current public name"]).is_ok());
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BatchOperation {
    operation_id: String,
    parameter: Option<Value>,
}
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Observe,
    ObserveDocument,
    ObserveTuiText {
        width: u16,
        height: u16,
    },
    /// Local TUI navigation only; GUI activation still uses Execute.
    TuiNavigate {
        observation_id: u64,
        action: String,
        query: Option<String>,
        context: Option<String>,
        #[serde(default)]
        exact: bool,
        #[serde(default = "default_steps")]
        max_steps: u16,
        #[serde(default = "default_width")]
        width: u16,
        #[serde(default = "default_height")]
        height: u16,
    },
    ExecuteBatchV1 {
        observation_id: u64,
        operations: Vec<BatchOperation>,
        #[serde(default = "default_width")]
        width: u16,
        #[serde(default = "default_height")]
        height: u16,
    },
    Execute {
        observation_id: u64,
        operation_id: String,
        parameter: Option<Value>,
    },
    Quit,
}
fn default_steps() -> u16 {
    64
}
fn default_width() -> u16 {
    120
}
fn default_height() -> u16 {
    40
}
fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    select_at_process_start(cli.session).map_err(io::Error::other)?;
    gui2tui::backend::set_external_text_limit(cli.max_edit_bytes);
    tokio::runtime::Runtime::new()?.block_on(run(cli))
}
async fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    let backend = AtspiBackend::connect(Duration::from_millis(cli.timeout_ms))
        .await?
        .with_tree_read_concurrency(usize::from(cli.ax_read_concurrency));
    let options = InspectOptions {
        verbose: false,
        max_depth: cli.max_depth,
        max_nodes: cli.max_nodes,
    };
    let mut app = if let Some(locator) = cli.application_id {
        let selected = backend
            .applications()
            .await?
            .into_iter()
            .find(|application| application.backend_locator == locator)
            .ok_or_else(|| {
                io::Error::other("Exact application is no longer present; no name rebinding")
            })?;
        TuiApplication::new_selected(
            backend,
            selected,
            options,
            Duration::from_millis(500),
            BootstrapStrategy::Walk,
            2048,
            PresentationMode::Transcompiled,
            cli.spatial_tui,
            (120, 40),
            false,
        )
        .await?
    } else {
        TuiApplication::new(
            backend,
            cli.app.expect("validated application selector"),
            options,
            Duration::from_millis(500),
            BootstrapStrategy::Walk,
            2048,
            PresentationMode::Transcompiled,
            cli.spatial_tui,
            (120, 40),
            false,
        )
        .await?
    };
    app.configure_decision_annotations(cli.decision_annotations);
    let mut observation: Option<AgentObservation> = None;
    let mut serial = 0u64;
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let request = serde_json::from_str::<Request>(&line?);
        let response = match request {
            Ok(Request::Quit) => break,
            Ok(
                request @ (Request::Observe
                | Request::ObserveDocument
                | Request::ObserveTuiText { .. }),
            ) => {
                observation = None;
                serial += 1;
                let view_size = match &request {
                    Request::ObserveTuiText { width, height } => Some((*width, *height)),
                    _ => None,
                };
                let fresh = match request {
                    Request::ObserveTuiText { .. } if !cli.spatial_tui => {
                        Err("Experimental TUI text requires --spatial-tui".into())
                    }
                    Request::ObserveDocument | Request::ObserveTuiText { .. } => {
                        app.agent_observe_document(serial).await
                    }
                    _ => app.agent_observe(serial).await,
                };
                match fresh {
                    Ok(fresh) => {
                        let data = fresh.data.clone();
                        match view_size
                            .map(|(width, height)| app.agent_render_tui_text(&fresh, width, height))
                            .transpose()
                        {
                            Ok(view) => {
                                observation = Some(fresh);
                                if let Some(view) = view {
                                    json!({"ok":true,"observation":data,"tui_view":view})
                                } else {
                                    json!({"ok":true,"observation":data})
                                }
                            }
                            Err(error) => {
                                json!({"ok":false,"error":error,"delivery":"not_attempted"})
                            }
                        }
                    }
                    Err(error) => json!({"ok": false, "error": error, "delivery": "not_attempted"}),
                }
            }
            Ok(Request::TuiNavigate {
                observation_id,
                action,
                query,
                context,
                exact,
                max_steps,
                width,
                height,
            }) => {
                let prior = observation
                    .take()
                    .filter(|o| o.data["observation_id"] == observation_id);
                if prior.is_none() || !cli.spatial_tui {
                    json!({"ok":false,"delivery":"not_attempted",
                        "error":"Local TUI navigation requires --spatial-tui and a current unconsumed observation"})
                } else {
                    // Query navigation is re-evaluated on a fresh observation;
                    // no old object locator is silently rebound or activated.
                    serial += 1;
                    match app.agent_observe_document(serial).await {
                        Ok(fresh) => match app
                            .agent_tui_navigate(
                                &fresh,
                                &action,
                                query.as_deref(),
                                context.as_deref(),
                                exact,
                                max_steps,
                                width,
                                height,
                            )
                            .await
                        {
                            Ok(mut result) => {
                                let data = fresh.data.clone();
                                observation = Some(fresh);
                                result["navigation"]["requested_observation_id"] =
                                    json!(observation_id);
                                json!({"ok":true,"observation":data,
                                    "tui_view":result["tui_view"].take(),"navigation":result["navigation"].take()})
                            }
                            Err(error) => {
                                json!({"ok":false,"error":error,"delivery":"not_attempted"})
                            }
                        },
                        Err(error) => json!({"ok":false,"error":error,"delivery":"not_attempted"}),
                    }
                }
            }
            Ok(Request::ExecuteBatchV1 {
                observation_id,
                operations,
                width,
                height,
            }) => {
                let prior = observation
                    .take()
                    .filter(|o| o.data["observation_id"] == observation_id);
                if !cli.spatial_tui || prior.is_none() {
                    json!({"ok":false,"delivery":"not_attempted",
                        "error":"Batch requires --spatial-tui and a current unconsumed observation"})
                } else {
                    let requests = operations
                        .into_iter()
                        .map(|operation| (operation.operation_id, operation.parameter))
                        .collect();
                    match app
                        .agent_execute_batch_v1(
                            prior.unwrap(),
                            requests,
                            &mut serial,
                            width,
                            height,
                        )
                        .await
                    {
                        Ok((mut response, fresh)) => {
                            observation = fresh;
                            response["ok"] = json!(true);
                            response
                        }
                        Err(error) => json!({"ok":false,"delivery":"not_attempted","error":error}),
                    }
                }
            }
            Ok(Request::Execute {
                observation_id,
                operation_id,
                parameter,
            }) => {
                match observation
                    .take()
                    .filter(|o| o.data["observation_id"] == observation_id)
                {
                    None => {
                        json!({"ok": false, "error": "Observe first; missing or consumed observation", "delivery": "not_attempted"})
                    }
                    Some(observed) => {
                        if cli.spatial_tui {
                            app.agent_tui_prepare_execution();
                        }
                        match app.agent_execute(observed, &operation_id, parameter).await {
                            Ok(receipt) => json!({"ok": true, "receipt": receipt}),
                            // An error can follow a backend call. Never interpret it as permission to retry.
                            Err(error) => {
                                json!({"ok": false, "error": error.to_string(), "delivery": error.delivery()})
                            }
                        }
                    }
                }
            }
            Err(error) => {
                json!({"ok": false, "error": error.to_string(), "delivery": "not_attempted"})
            }
        };
        let encoding_started = std::time::Instant::now();
        let encoded = serde_json::to_string(&response)?;
        let encoding_ms = encoding_started.elapsed().as_secs_f64() * 1000.0;
        let write_started = std::time::Instant::now();
        writeln!(stdout, "{encoded}")?;
        stdout.flush()?;
        if response["observation"]["version"]
            == gui2tui::transcompile::document::wire::DOCUMENT_VERSION
        {
            eprintln!(
                "{}",
                json!({"stage":"agent_jsonl_transport","observation_id":serial,"bytes":encoded.len(),
                "utf8_encoding_ms":encoding_ms,"write_flush_ms":write_started.elapsed().as_secs_f64()*1000.0})
            );
        }
    }
    app.shutdown().await;
    Ok(())
}
