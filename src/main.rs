use std::{
    process::ExitCode,
    time::{Duration, Instant},
};

use clap::Parser;
use gui2tui::{
    backend::{
        AtspiBackend, BackendError, BootstrapStrategy, DEFAULT_EVENT_BUFFER_CAPACITY,
        InspectOptions,
    },
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
    #[arg(long, conflicts_with_all = ["app", "app_id", "actions", "activate", "action", "action_name", "select_child", "watch_events", "probe_cache", "probe_collection"])]
    list: bool,

    /// Inspect an application by exact name or an unambiguous substring.
    #[arg(long, value_name = "NAME", conflicts_with_all = ["list", "app_id", "actions", "activate", "action", "action_name", "select_child"])]
    app: Option<String>,

    /// Inspect an application by the one-based index printed by --list.
    #[arg(long, value_name = "INDEX", conflicts_with_all = ["list", "app", "actions", "activate", "action", "action_name", "select_child"])]
    app_id: Option<usize>,

    /// List actions exposed by NODE_ID.
    #[arg(long, value_name = "NODE_ID", conflicts_with_all = ["list", "app", "app_id", "activate", "action", "action_name", "select_child"])]
    actions: Option<String>,

    /// Invoke a safe convenience click/press/activate action on NODE_ID.
    #[arg(long, value_name = "NODE_ID", conflicts_with_all = ["list", "app", "app_id", "actions", "action", "action_name", "select_child"])]
    activate: Option<String>,

    /// Invoke an action on NODE_ID; requires --index.
    #[arg(long, value_name = "NODE_ID", requires = "index", conflicts_with_all = ["list", "app", "app_id", "actions", "activate", "action_name", "select_child"])]
    action: Option<String>,

    /// Zero-based action index used with --action.
    #[arg(long, value_name = "INDEX", requires = "action")]
    index: Option<i32>,

    /// Invoke an AT-SPI action by its exposed name (case-insensitive fallback).
    #[arg(long, num_args = 2, value_names = ["NODE_ID", "NAME"], conflicts_with_all = ["list", "app", "app_id", "actions", "activate", "action", "index", "select_child"])]
    action_name: Option<Vec<String>>,

    /// Select a direct child through the container's AT-SPI Selection interface.
    #[arg(long, value_name = "PARENT_NODE_ID", requires = "child_index", conflicts_with_all = ["list", "app", "app_id", "actions", "activate", "action", "index", "action_name"])]
    select_child: Option<String>,

    /// Zero-based direct-child index used with --select-child.
    #[arg(long, value_name = "INDEX", requires = "select_child")]
    child_index: Option<usize>,

    /// Continuously print AT-SPI events emitted by the selected --app.
    #[arg(long, requires = "app", conflicts_with_all = ["list", "app_id", "actions", "activate", "action", "action_name", "select_child"])]
    watch_events: bool,

    /// Probe the selected application's bulk AT-SPI Cache.GetItems support.
    #[arg(long, requires = "app", conflicts_with_all = ["list", "app_id", "actions", "activate", "action", "action_name", "select_child", "watch_events"])]
    probe_cache: bool,

    /// Probe the selected application's AT-SPI Collection interface.
    #[arg(long, requires = "app", conflicts_with_all = ["list", "app_id", "actions", "activate", "action", "action_name", "select_child", "watch_events", "probe_cache"])]
    probe_collection: bool,

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

    /// Initial semantic bootstrap strategy.
    #[arg(long, value_enum, default_value_t = BootstrapStrategy::Auto)]
    bootstrap: BootstrapStrategy,

    /// Maximum buffered events used by --watch-events.
    #[arg(long, default_value_t = DEFAULT_EVENT_BUFFER_CAPACITY)]
    event_buffer_capacity: usize,

    /// Print the toolkit-independent SemanticRegion analysis instead of the raw tree.
    #[arg(long, requires = "app")]
    dump_regions: bool,

    /// Print the planned terminal TuiScene instead of the raw tree.
    #[arg(long, requires = "app")]
    dump_scene: bool,
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

    if let Some(parent_id) = cli.select_child {
        let child_index = cli
            .child_index
            .ok_or(BackendError::MissingSelectionChildIndex)?;
        let parent = gui2tui::semantic::BackendLocator::decode(&parent_id)?;
        backend.select_child(&parent, child_index).await?;
        println!("Selected child {child_index} through container {parent_id}");
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
    if cli.watch_events {
        println!(
            "Watching AT-SPI events for {} ({})",
            application.name, application.backend_locator
        );
        return backend
            .watch_events_with_capacity(application, cli.event_buffer_capacity)
            .await;
    }
    if cli.probe_cache {
        let result = backend.probe_cache(application).await?;
        println!("Cache available: yes");
        println!("signature: {}", result.format);
        println!("items: {}", result.records.len());
        println!(
            "named items: {}",
            result
                .records
                .iter()
                .filter(|record| record.name.is_some())
                .count()
        );
        println!(
            "RPC duration: {:.3} ms",
            result.rpc_duration.as_secs_f64() * 1000.0
        );
        if let Some(error) = result.modern_error {
            println!("modern decode failed before legacy fallback: {error}");
        }
        for record in result.records.iter().take(5) {
            println!(
                "sample role={} name={:?} description={:?} path={} parent={:?} index={:?} children={:?}",
                record.role.name(),
                record.name,
                record.description,
                record.locator.object_path(),
                record.parent.as_ref().map(|parent| parent.object_path()),
                record.index_in_parent,
                record.child_count,
            );
        }
        return Ok(());
    }
    if cli.probe_collection {
        let result = backend.probe_collection(application).await?;
        println!("Collection nodes: {}", result.collection_nodes);
        match result.source {
            Some(source) => println!("query source: {source}"),
            None => println!("query source: none"),
        }
        for query in result.queries {
            match (query.count, query.error) {
                (Some(count), _) => println!(
                    "{}: {} matches in {:.3} ms",
                    query.query,
                    count,
                    query.duration.as_secs_f64() * 1000.0
                ),
                (_, Some(error)) => println!(
                    "{}: unavailable after {:.3} ms: {}",
                    query.query,
                    query.duration.as_secs_f64() * 1000.0,
                    error
                ),
                _ => println!("{}: unavailable", query.query),
            }
        }
        return Ok(());
    }
    let bootstrap = backend
        .bootstrap_application(
            application,
            InspectOptions {
                verbose: cli.verbose,
                max_depth: cli.max_depth,
                max_nodes: cli.max_nodes,
            },
            cli.bootstrap,
        )
        .await?;
    eprintln!(
        "Bootstrap: {} nodes via {} in {:.3} ms (cache_rpc={:.3} ms enrichment={:.3} ms reconstruction={:.3} ms enrichment_rpcs={} orphans={}){}",
        bootstrap.metrics.node_count,
        bootstrap.metrics.strategy,
        bootstrap.metrics.total.as_secs_f64() * 1000.0,
        bootstrap.metrics.cache_rpc.as_secs_f64() * 1000.0,
        bootstrap.metrics.enrichment.as_secs_f64() * 1000.0,
        bootstrap.metrics.reconstruction.as_secs_f64() * 1000.0,
        bootstrap.metrics.enrichment_rpc_count,
        bootstrap.metrics.orphans_ignored,
        bootstrap
            .metrics
            .fallback_reason
            .as_ref()
            .map(|reason| format!(" fallback={reason:?}"))
            .unwrap_or_default(),
    );
    if cli.dump_regions || cli.dump_scene {
        let analysis_started = Instant::now();
        let analysis = gui2tui::transcompile::analyze_regions(&bootstrap.root);
        let analysis_elapsed = analysis_started.elapsed();
        if cli.dump_regions {
            print!("{}", gui2tui::transcompile::format_regions(&analysis));
        }
        if cli.dump_scene {
            let scene_started = Instant::now();
            let scene = gui2tui::transcompile::compile_scene(&bootstrap.root, &analysis);
            let scene_elapsed = scene_started.elapsed();
            print!("{}", gui2tui::transcompile::format_scene(&scene));
            eprintln!(
                "Transcompiler: region analysis {:.3} ms; scene compile {:.3} ms",
                analysis_elapsed.as_secs_f64() * 1000.0,
                scene_elapsed.as_secs_f64() * 1000.0,
            );
        } else {
            eprintln!(
                "Transcompiler: region analysis {:.3} ms",
                analysis_elapsed.as_secs_f64() * 1000.0,
            );
        }
        return Ok(());
    }
    print!(
        "{}",
        format_tree(
            &bootstrap.root,
            FormatOptions {
                verbose: cli.verbose
            }
        )
    );
    Ok(())
}
