//! Development JSON-lines interface. One persistent runtime owns all authority.
use clap::Parser;
use gui2tui::{
    backend::{AtspiBackend, BootstrapStrategy, InspectOptions},
    product::headless::{SessionChoice, select_at_process_start},
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
    #[arg(long)]
    app: String,
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
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Observe,
    Execute {
        observation_id: u64,
        operation_id: String,
        parameter: Option<Value>,
    },
    Quit,
}
fn main() -> Result<(), Box<dyn Error>> {
    let cli = Cli::parse();
    select_at_process_start(cli.session).map_err(io::Error::other)?;
    gui2tui::backend::set_external_text_limit(cli.max_edit_bytes);
    tokio::runtime::Runtime::new()?.block_on(run(cli))
}
async fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    let backend = AtspiBackend::connect(Duration::from_millis(cli.timeout_ms)).await?;
    let mut app = TuiApplication::new(
        backend,
        cli.app,
        InspectOptions {
            verbose: false,
            max_depth: cli.max_depth,
            max_nodes: cli.max_nodes,
        },
        Duration::from_millis(500),
        BootstrapStrategy::Walk,
        2048,
        PresentationMode::Transcompiled,
        false,
        (120, 40),
        false,
    )
    .await?;
    let mut observation: Option<AgentObservation> = None;
    let mut serial = 0u64;
    let mut stdout = io::stdout().lock();
    for line in io::stdin().lock().lines() {
        let request = serde_json::from_str::<Request>(&line?);
        let response = match request {
            Ok(Request::Quit) => break,
            Ok(Request::Observe) => {
                observation = None;
                serial += 1;
                match app.agent_observe(serial).await {
                    Ok(fresh) => {
                        let data = fresh.data.clone();
                        observation = Some(fresh);
                        json!({"ok": true, "observation": data})
                    }
                    Err(error) => json!({"ok": false, "error": error, "delivery": "not_attempted"}),
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
        writeln!(stdout, "{}", serde_json::to_string(&response)?)?;
        stdout.flush()?;
    }
    app.shutdown().await;
    Ok(())
}
