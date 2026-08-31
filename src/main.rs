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

    /// Print toolkit-independent choice discovery and safe selection strategies.
    #[arg(long, requires = "app")]
    dump_choices: bool,

    /// Lazily enrich and print relations for scene-relevant nodes.
    #[arg(long, requires = "app")]
    dump_relations: bool,

    /// Print relations for one NODE_ID within the selected application.
    #[arg(long, value_name = "NODE_ID", requires = "app")]
    relations: Option<String>,

    /// Print the application/window/dialog/popup interaction-scope hierarchy.
    #[arg(long, requires = "app")]
    dump_scopes: bool,

    /// Print scope-filtered hierarchical commands and ranking explanations.
    #[arg(long, requires = "app")]
    dump_commands: bool,

    /// Search string used with --dump-commands.
    #[arg(long, requires = "dump_commands", default_value = "")]
    command_query: String,

    /// Expand command search beyond the active interaction scope.
    #[arg(long, requires = "dump_commands")]
    all_scopes: bool,

    /// Audit reachability of safely invokable semantic leaf operations.
    #[arg(long, requires = "app")]
    audit_scene_reachability: bool,

    /// Audit Reader navigation and document control reachability after compression.
    #[arg(long, requires = "app")]
    audit_content_reachability: bool,

    /// Print semantic content blocks without eagerly reading body text.
    #[arg(long, requires = "app")]
    dump_content: bool,

    /// Include non-secret content text already available to --dump-content.
    #[arg(long, requires = "dump_content")]
    with_text: bool,

    /// Print the semantic heading outline.
    #[arg(long, requires = "app")]
    dump_outline: bool,

    /// Probe generic AT-SPI Document/Text/Hypertext metadata for content roots.
    #[arg(long, requires = "app")]
    probe_document: bool,

    /// Print progressively realized List/Tree/Table models.
    #[arg(long, requires = "app")]
    dump_virtual_collections: bool,

    /// Probe generic AT-SPI Table metadata and sample realized cells.
    #[arg(long, requires = "app")]
    probe_tables: bool,

    /// Discover and resolve external modality resources from generic AT-SPI metadata.
    #[arg(long, requires = "app")]
    dump_modalities: bool,

    /// Resolve one modality owner identified by its backend NODE_ID.
    #[arg(long, value_name = "NODE_ID", requires = "app")]
    resolve_modality: Option<String>,

    /// Print the redacted resource reference for one modality owner.
    #[arg(long, value_name = "NODE_ID", requires = "app")]
    dump_resource_reference: Option<String>,

    /// Print the server-side modality protocol capabilities (never local executable paths).
    #[arg(long)]
    modality_capabilities: bool,

    /// Request a resolved reference handoff; approval and handler choice stay local.
    #[arg(long, value_name = "NODE_ID", requires_all = ["app", "modality_socket"])]
    handoff_modality: Option<String>,

    /// Private local broker socket (or a user-managed socket forwarding endpoint).
    #[arg(long)]
    modality_socket: Option<std::path::PathBuf>,
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
    if cli.modality_capabilities {
        println!("resolution=reference-first,portable-artifact,live-visual-fallback");
        println!("payload_in_semantic_cache=false");
        println!("server_executable_selection=false");
        println!("static_visual_artifact=false");
        println!("continuous_streaming=false");
        return Ok(());
    }

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
        let sample_limit = if cli.verbose { result.records.len() } else { 5 };
        for record in result.records.iter().take(sample_limit) {
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
    if cli.dump_regions
        || cli.dump_scene
        || cli.dump_choices
        || cli.dump_relations
        || cli.relations.is_some()
        || cli.dump_scopes
        || cli.dump_commands
        || cli.audit_scene_reachability
        || cli.audit_content_reachability
        || cli.dump_content
        || cli.dump_outline
        || cli.probe_document
        || cli.dump_virtual_collections
        || cli.probe_tables
        || cli.dump_modalities
        || cli.resolve_modality.is_some()
        || cli.dump_resource_reference.is_some()
        || cli.handoff_modality.is_some()
    {
        let mut cache = gui2tui::semantic::SemanticCache::from_snapshot(bootstrap.root)
            .map_err(|error| BackendError::SemanticCache(error.to_string()))?;
        let content = gui2tui::content::ContentCatalog::analyze(&cache);
        if cli.dump_modalities
            || cli.resolve_modality.is_some()
            || cli.dump_resource_reference.is_some()
            || cli.handoff_modality.is_some()
        {
            let requested = cli
                .resolve_modality
                .as_deref()
                .or(cli.dump_resource_reference.as_deref())
                .or(cli.handoff_modality.as_deref())
                .map(gui2tui::semantic::BackendLocator::decode)
                .transpose()?;
            let candidates = gui2tui::modality::ModalityResolver::discover(&cache);
            let mut matched = 0_usize;
            for candidate in candidates {
                if requested
                    .as_ref()
                    .is_some_and(|locator| locator != &candidate.locator)
                {
                    continue;
                }
                matched += 1;
                let mut metadata = Vec::new();
                for locator in &candidate.evidence_locators {
                    if let Ok(probe) = backend.probe_modality_metadata(locator).await {
                        metadata.push(gui2tui::modality::ModalityMetadata {
                            accessible_attributes: probe.accessible_attributes,
                            document_attributes: probe.document_attributes,
                            hyperlink_uris: probe.hyperlink_uris,
                        });
                    }
                }
                let modality =
                    gui2tui::modality::ModalityResolver::default().resolve(&candidate, &metadata);
                if cli.dump_resource_reference.is_some() {
                    match &modality.resolution {
                        gui2tui::modality::ModalityResolution::ReferencedResource(resource) => {
                            println!(
                                "{:?} provenance={:?}",
                                gui2tui::modality::redact_reference(&resource.reference),
                                resource.provenance
                            );
                        }
                        _ => println!("UNRESOLVED"),
                    }
                } else {
                    println!("{}", gui2tui::modality::format_external_modality(&modality));
                }
                if cli.handoff_modality.is_some() {
                    let socket = cli.modality_socket.clone().ok_or_else(|| {
                        BackendError::SemanticCache("local client unavailable".to_owned())
                    })?;
                    let result = tokio::task::spawn_blocking(move || {
                        let capabilities = gui2tui::modality::wire::capabilities(&socket)?;
                        let mut modality = modality;
                        modality.negotiate(Some(&capabilities));
                        if !modality.capabilities.reference_handoff {
                            return Err(std::io::Error::other(
                                "no resolved resource with an available local handler",
                            ));
                        }
                        let gui2tui::modality::ModalityResolution::ReferencedResource(resource) =
                            modality.resolution
                        else {
                            return Err(std::io::Error::other("resource unresolved"));
                        };
                        gui2tui::modality::wire::send_reference(&socket, modality.kind, resource)
                    })
                    .await
                    .map_err(|_| {
                        BackendError::SemanticCache("local handoff task failed".to_owned())
                    })?
                    .map_err(|_| {
                        BackendError::SemanticCache(
                            "local modality client unavailable or unsupported".to_owned(),
                        )
                    })?;
                    println!("Handoff {result:?}");
                    if matches!(result, gui2tui::modality::wire::Response::Failed { .. }) {
                        return Err(BackendError::SemanticCache(
                            "local handoff denied or failed".to_owned(),
                        ));
                    }
                }
            }
            if requested.is_some() && matched == 0 {
                return Err(BackendError::SemanticCache(
                    "requested node is not an external modality owner in this application"
                        .to_owned(),
                ));
            }
        }
        if cli.dump_content {
            for model in content.models() {
                print!(
                    "{}",
                    gui2tui::content::format_content_model(model, cli.with_text)
                );
            }
        }
        if cli.dump_outline {
            for model in content.models() {
                print!("{}", gui2tui::content::format_outline(model));
            }
        }
        if cli.probe_document {
            for model in content.models() {
                let Some(node) = cache.node(model.root) else {
                    continue;
                };
                match backend.probe_document(&node.backend_locator).await {
                    Ok(probe) => println!(
                        "Document root={} locator={} document={} text={} hypertext={} locale={:?} page={:?}/{:?} chars={:?} links={:?} attributes={:?}",
                        model.root,
                        node.backend_locator,
                        probe.document_interface,
                        probe.text_interface,
                        probe.hypertext_interface,
                        probe.locale,
                        probe.current_page,
                        probe.page_count,
                        probe.character_count,
                        probe.hyperlink_count,
                        probe.attributes,
                    ),
                    Err(error) => println!("Document root={} unavailable: {error}", model.root),
                }
            }
        }
        if cli.dump_virtual_collections {
            let models = gui2tui::content::analyze_virtual_collections(&cache);
            print!(
                "{}",
                gui2tui::content::format_virtual_collections(&cache, &models)
            );
        }
        if cli.probe_tables {
            let probes = backend.probe_tables(&cache).await;
            if probes.is_empty() {
                println!("Table interface nodes: 0");
            }
            for probe in probes {
                println!(
                    "Table locator={} rows={:?} columns={:?} sampled_cells={} duration={:.3} ms error={:?}",
                    probe.source,
                    probe.rows,
                    probe.columns,
                    probe.sampled_cells,
                    probe.duration.as_secs_f64() * 1000.0,
                    probe.error,
                );
            }
        }
        let only_content = !cli.dump_regions
            && !cli.dump_scene
            && !cli.dump_choices
            && !cli.dump_relations
            && cli.relations.is_none()
            && !cli.dump_scopes
            && !cli.dump_commands
            && !cli.audit_scene_reachability
            && !cli.audit_content_reachability;
        if only_content {
            return Ok(());
        }
        let initial_tree = cache
            .materialize_tree()
            .map_err(|error| BackendError::SemanticCache(error.to_string()))?;
        let initial_analysis = gui2tui::transcompile::analyze_regions(&initial_tree);
        let mut initial_scene =
            gui2tui::transcompile::compile_scene(&initial_tree, &initial_analysis);
        gui2tui::transcompile::compress_content_scene(&mut initial_scene, &cache, &content);
        let requested_locator = cli
            .relations
            .as_deref()
            .map(gui2tui::semantic::BackendLocator::decode)
            .transpose()?;
        let visible_scene = initial_scene
            .elements
            .iter()
            .flat_map(|element| element.sources.iter().copied())
            .collect();
        let budget = if cache.node_count() <= 512 {
            cache.node_count()
        } else {
            gui2tui::semantic::LARGE_TREE_RELATION_CANDIDATE_LIMIT
        };
        let schedule = gui2tui::semantic::schedule_relation_candidates(
            &cache,
            &gui2tui::semantic::RelationPriorityContext {
                visible_scene,
                ..Default::default()
            },
            budget,
        );
        let mut candidates: Vec<_> = schedule
            .candidates
            .iter()
            .map(|candidate| candidate.runtime_id)
            .collect();
        if let Some(locator) = &requested_locator {
            let runtime_id = cache.runtime_id(locator).ok_or_else(|| {
                BackendError::SemanticCache(format!(
                    "relation node {locator} does not belong to selected application"
                ))
            })?;
            if !candidates.contains(&runtime_id) {
                candidates.push(runtime_id);
            }
        }
        let relation_metrics = backend.enrich_relations(&mut cache, &candidates).await;
        eprintln!(
            "Relations: budget={} deferred={} visible={} relation-sensitive={} background={} candidates={} rpcs={} found={} unresolved={} unavailable={} latency={:.3} ms",
            schedule.budget,
            schedule.deferred,
            schedule
                .candidates
                .iter()
                .filter(|candidate| candidate.reason
                    == gui2tui::semantic::RelationPriorityReason::VisibleScene)
                .count(),
            schedule
                .candidates
                .iter()
                .filter(|candidate| candidate.reason
                    == gui2tui::semantic::RelationPriorityReason::RelationSensitiveRole)
                .count(),
            schedule
                .candidates
                .iter()
                .filter(|candidate| candidate.reason
                    == gui2tui::semantic::RelationPriorityReason::Background)
                .count(),
            relation_metrics.candidate_nodes,
            relation_metrics.rpc_count,
            relation_metrics.relations_found,
            relation_metrics.unresolved_targets,
            relation_metrics.unavailable_nodes,
            relation_metrics.duration.as_secs_f64() * 1000.0,
        );
        let tree = cache
            .materialize_tree()
            .map_err(|error| BackendError::SemanticCache(error.to_string()))?;
        let graph = gui2tui::semantic::RelationalSemanticGraph::new(&cache);
        let scopes = gui2tui::transcompile::InteractionScopes::analyze(&cache, &graph);
        let analysis_started = Instant::now();
        let analysis = gui2tui::transcompile::analyze_regions_with_graph(&tree, &graph);
        let analysis_elapsed = analysis_started.elapsed();
        let scene_started = Instant::now();
        let mut scene = gui2tui::transcompile::compile_scene(&tree, &analysis);
        let content_compression =
            gui2tui::transcompile::compress_content_scene(&mut scene, &cache, &content);
        let scene_elapsed = scene_started.elapsed();
        let commands = gui2tui::transcompile::CommandHierarchy::build(&cache, &scopes);
        if cli.dump_choices {
            let choices = gui2tui::transcompile::ChoiceCatalog::discover(&cache);
            print!(
                "{}",
                gui2tui::transcompile::format_choices(&cache, &choices)
            );
        }
        if cli.dump_regions {
            print!("{}", gui2tui::transcompile::format_regions(&analysis));
        }
        if cli.dump_scene {
            print!("{}", gui2tui::transcompile::format_scene(&scene));
            eprintln!(
                "Transcompiler: region analysis {:.3} ms; scene compile {:.3} ms; content compression {} -> {} elements (summaries={} preserved_bindings={})",
                analysis_elapsed.as_secs_f64() * 1000.0,
                scene_elapsed.as_secs_f64() * 1000.0,
                content_compression.before_elements,
                content_compression.after_elements,
                content_compression.summaries,
                content_compression.preserved_bound_elements,
            );
        } else if cli.dump_regions {
            eprintln!(
                "Transcompiler: region analysis {:.3} ms",
                analysis_elapsed.as_secs_f64() * 1000.0,
            );
        }
        if cli.dump_relations || cli.relations.is_some() {
            let only = requested_locator
                .as_ref()
                .and_then(|locator| cache.runtime_id(locator));
            print!("{}", gui2tui::semantic::format_relations(&cache, only));
        }
        if cli.dump_scopes {
            print!("{}", gui2tui::transcompile::format_scopes(&scopes));
        }
        if cli.dump_commands {
            print!(
                "{}",
                gui2tui::transcompile::format_commands(
                    &commands,
                    &scopes,
                    &cli.command_query,
                    cli.all_scopes,
                )
            );
        }
        if cli.audit_scene_reachability {
            let audit = commands.audit(&cache);
            println!("safe leaves: {}", audit.safe_leaves);
            println!("reachable: {}", audit.reachable);
            println!(
                "structural reveal omitted: {}",
                audit.structural_reveal_omitted
            );
            println!("unsafe/unresolved: {}", audit.unsafe_or_unresolved);
            println!("unreachable: {}", audit.unreachable);
            for command in &audit.unreachable_commands {
                println!(
                    "  RuntimeNodeId={} role={} name={:?} operation={:?} reason={}",
                    command.source, command.role, command.name, command.intent, command.reason
                );
            }
        }
        if cli.audit_content_reachability {
            let audit = gui2tui::transcompile::audit_content_reachability(&scene, &content);
            print!(
                "{}",
                gui2tui::transcompile::format_content_reachability(&audit)
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
