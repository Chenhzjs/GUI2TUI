use std::{process::ExitCode, time::Duration};

use clap::Parser;
use gui2tui::{
    backend::{AtspiBackend, BackendError, InspectOptions},
    inspect::{FormatOptions, format_tree},
};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "gui2tui-inspect",
    version,
    about = "Inspect and activate semantic GUI controls exposed through Linux AT-SPI"
)]
struct Cli {
    /// List applications currently exposed by the AT-SPI desktop.
    #[arg(long, conflicts_with_all = ["app", "app_id", "actions", "activate", "action", "action_name"])]
    list: bool,

    /// Inspect an application by exact name or an unambiguous substring.
    #[arg(long, value_name = "NAME", conflicts_with_all = ["list", "app_id", "actions", "activate", "action", "action_name"])]
    app: Option<String>,

    /// Inspect an application by the one-based index printed by --list.
    #[arg(long, value_name = "INDEX", conflicts_with_all = ["list", "app", "actions", "activate", "action", "action_name"])]
    app_id: Option<usize>,

    /// List actions exposed by NODE_ID.
    #[arg(long, value_name = "NODE_ID", conflicts_with_all = ["list", "app", "app_id", "activate", "action", "action_name"])]
    actions: Option<String>,

    /// Invoke press/click/activate/open (or the first available action) on NODE_ID.
    #[arg(long, value_name = "NODE_ID", conflicts_with_all = ["list", "app", "app_id", "actions", "action", "action_name"])]
    activate: Option<String>,

    /// Invoke an action on NODE_ID; requires --index.
    #[arg(long, value_name = "NODE_ID", requires = "index", conflicts_with_all = ["list", "app", "app_id", "actions", "activate", "action_name"])]
    action: Option<String>,

    /// Zero-based action index used with --action.
    #[arg(long, value_name = "INDEX", requires = "action")]
    index: Option<i32>,

    /// Invoke an AT-SPI action by its exposed name (case-insensitive fallback).
    #[arg(long, num_args = 2, value_names = ["NODE_ID", "NAME"], conflicts_with_all = ["list", "app", "app_id", "actions", "activate", "action", "index"])]
    action_name: Option<Vec<String>>,

    /// Include AT-SPI role, full states, interfaces, object identity and geometry.
    #[arg(short, long)]
    verbose: bool,

    /// Maximum accessibility-tree depth to traverse.
    #[arg(long, default_value_t = 64)]
    max_depth: usize,

    /// Maximum number of accessibility objects to traverse.
    #[arg(long, default_value_t = 10_000)]
    max_nodes: usize,

    /// Per-operation D-Bus/AT-SPI timeout in milliseconds.
    #[arg(long, default_value_t = 5_000, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,
}

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            if matches!(
                error,
                BackendError::NoDesktopSession { .. } | BackendError::AtspiUnavailable { .. }
            ) {
                eprintln!(
                    "hint: run inside the target Linux graphical user's session and ensure AT-SPI accessibility is enabled; a plain SSH shell usually lacks the required session D-Bus environment"
                );
            }
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), BackendError> {
    let backend = AtspiBackend::connect(Duration::from_millis(cli.timeout_ms)).await?;

    if let Some(node_id) = cli.actions {
        let actions = backend.actions(&node_id).await?;
        if actions.is_empty() {
            println!("No actions exposed.");
        } else {
            for action in actions {
                print!("{} {}", action.index, action.name);
                if let Some(description) = action.description {
                    print!(" description={description:?}");
                }
                if let Some(keybinding) = action.keybinding {
                    print!(" keybinding={keybinding:?}");
                }
                println!();
            }
        }
        return Ok(());
    }

    if let Some(node_id) = cli.activate {
        let action = backend.activate(&node_id).await?;
        println!(
            "Activated action {} ({}) on {node_id}",
            action.index, action.name
        );
        return Ok(());
    }

    if let Some(node_id) = cli.action {
        let index = cli.index.ok_or(BackendError::MissingActionIndex)?;
        let action = backend.do_action(&node_id, index).await?;
        println!(
            "Invoked action {} ({}) on {node_id}",
            action.index, action.name
        );
        return Ok(());
    }

    if let Some(action_name) = cli.action_name {
        let [node_id, name] = action_name.as_slice() else {
            return Err(BackendError::MissingActionNameArguments);
        };
        let action = backend.do_action_by_name(node_id, name).await?;
        println!(
            "Invoked action {} ({}) on {node_id}",
            action.index, action.name
        );
        return Ok(());
    }

    let applications = backend.applications().await?;
    if cli.app.is_none() && cli.app_id.is_none() {
        if applications.is_empty() {
            return Err(BackendError::NoApplications);
        }
        for application in applications {
            println!("{}  {}", application.index, application.name);
        }
        return Ok(());
    }

    let application =
        AtspiBackend::select_application(&applications, cli.app.as_deref(), cli.app_id)?;
    let tree = backend
        .inspect_application(
            application,
            InspectOptions {
                verbose: cli.verbose,
                max_depth: cli.max_depth,
                max_nodes: cli.max_nodes,
            },
        )
        .await?;
    print!(
        "{}",
        format_tree(
            &tree,
            FormatOptions {
                verbose: cli.verbose
            }
        )
    );
    Ok(())
}
