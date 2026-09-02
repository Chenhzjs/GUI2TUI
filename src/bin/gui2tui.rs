use std::{
    error::Error,
    io::{self, IsTerminal, Write},
    path::Path,
    process::{Command as ProcessCommand, ExitCode},
    time::Duration,
};

use clap::{Parser, Subcommand, ValueEnum};
use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_lite::StreamExt;
use gui2tui::{
    backend::{AtspiBackend, BootstrapStrategy, InspectOptions},
    runtime::signals::{RuntimeSignal, RuntimeSignals},
    transcompile::PresentationMode,
    tui::{
        app::TuiApplication,
        input::mouse_to_intent,
        selector::{
            ApplicationSelector, SelectorIntent, SelectorTarget, key_to_selector_intent,
            mouse_click,
        },
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(
    name = "gui2tui",
    version,
    about = "Interact with a GUI application through a terminal-native semantic UI"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Debug-build-only terminal restoration failpoint used by the lifecycle harness.
    #[cfg(debug_assertions)]
    #[arg(long, hide = true)]
    test_panic_after_attach: bool,

    /// Accessible application name or an unambiguous substring.
    #[arg(long, value_name = "NAME", global = true)]
    app: Option<String>,

    /// Maximum accessibility-tree depth per snapshot.
    #[arg(long, default_value_t = 64, hide = true)]
    max_depth: usize,

    /// Maximum accessibility objects per snapshot.
    #[arg(long, default_value_t = 10_000, hide = true)]
    max_nodes: usize,

    /// Per-operation D-Bus/AT-SPI timeout in milliseconds.
    #[arg(long, hide = true)]
    timeout_ms: Option<u64>,

    /// Maximum wait for a related AT-SPI event before fallback refresh.
    #[arg(long, default_value_t = 500, hide = true, value_parser = clap::value_parser!(u64).range(1..))]
    settle_ms: u64,

    /// Initial semantic bootstrap strategy.
    #[arg(long, value_enum, default_value_t = BootstrapStrategy::Auto, hide = true)]
    bootstrap: BootstrapStrategy,

    /// Maximum buffered AT-SPI events before a correctness resync.
    #[arg(long, hide = true)]
    event_buffer_capacity: Option<usize>,

    /// Frontend projection pipeline; legacy preserves the Phase 3B flat mapping.
    #[arg(long, value_enum, default_value_t = PresentationMode::Transcompiled, hide = true)]
    presentation: PresentationMode,

    /// Experimental terminal-native spatial reconstruction and responsive composition (v0.2).
    #[arg(long, value_enum, default_value_t = LayoutMode::Flat)]
    layout: LayoutMode,

    /// Private local modality broker socket; absent means safe read-only fallback.
    #[arg(long, global = true)]
    modality_socket: Option<std::path::PathBuf>,

    /// Disable terminal mouse capture (keyboard remains available).
    #[arg(long, global = true)]
    no_mouse: bool,

    /// Write contents-free product lifecycle diagnostics to a private runtime log.
    #[arg(long, value_enum, default_value_t = LogLevel::Off, global = true)]
    log_level: LogLevel,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum LayoutMode {
    /// Keep the stable v0.1 linear scene renderer.
    Flat,
    /// Render the experimental SpatialEvidence/TuiLayoutPlan composition.
    Spatial,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Open the application selector (also the default with no command).
    Run,
    /// Check dependencies and session access without reading GUI contents.
    Doctor {
        #[arg(long)]
        verbose: bool,
        #[arg(long)]
        json: bool,
        /// Save a contents-free JSON report to a new private file (never overwrite).
        #[arg(long, value_name = "FILE")]
        report: Option<std::path::PathBuf>,
    },
    /// Inspect or explicitly initialize the optional XDG configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// Manage explicit GUI launchers stored in the user configuration.
    App {
        #[command(subcommand)]
        command: AppCommand,
    },
    /// Start a registered GUI application, wait for AT-SPI, and open it.
    Launch {
        /// Registered launcher id from `gui2tui app list`.
        id: String,
    },
    /// Configure a managed headless accessibility session.
    Setup {
        #[command(subcommand)]
        command: SetupCommand,
    },
    /// Run the low-level semantic inspector through the unified CLI.
    Inspect {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Manage the optional same-host modality endpoint.
    Endpoint {
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}
#[derive(Debug, Subcommand)]
enum SetupCommand {
    /// Start (or reuse) a persistent session used automatically by future terminals.
    Persistent {
        #[arg(long, default_value = "1440x900x24")]
        screen: String,
    },
    /// Open a shell or run one command in an isolated session that ends on exit.
    Temporary {
        #[arg(last = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Report whether the persistent managed session is running.
    Status,
    /// Stop the persistent managed session and disable automatic attachment.
    Stop,
    /// Restart the persistent managed session.
    Restart {
        #[arg(long, default_value = "1440x900x24")]
        screen: String,
    },
}
#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Path,
    Check,
    Show,
    Init,
}
#[derive(Debug, Subcommand)]
enum AppCommand {
    /// Add a launcher without invoking a shell.
    Add {
        /// Executable name/path. Omit it to use the interactive setup wizard.
        program: Option<String>,
        /// Short launcher id shown in the selector; defaults to executable name.
        #[arg(long)]
        id: Option<String>,
        /// Expected AT-SPI application name; defaults to ID.
        #[arg(long = "match", value_name = "AT_SPI_NAME")]
        match_name: Option<String>,
        /// Maximum time to wait for AT-SPI registration.
        #[arg(long, default_value_t = 15_000, value_parser = clap::value_parser!(u64).range(100..=120_000))]
        wait_ms: u64,
        /// Replace an existing launcher with the same id.
        #[arg(long)]
        replace: bool,
        /// Arguments passed directly to the program. Put them after `--`.
        #[arg(last = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// List registered launchers and their direct argv (no shell).
    List,
    /// Remove a registered launcher.
    Remove { id: String },
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum LogLevel {
    Off,
    Info,
    Debug,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let setup_command = matches!(&cli.command, Some(Command::Setup { .. }));
    if !setup_command {
        if let Err(error) = gui2tui::product::headless::apply_at_process_start() {
            eprintln!("warning: {error}");
        }
    }
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("error: cannot start async runtime: {error}");
            return ExitCode::FAILURE;
        }
    };
    match runtime.block_on(dispatch(cli)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn dispatch(mut cli: Cli) -> Result<(), Box<dyn Error>> {
    use gui2tui::product::{
        config::{Config, LauncherConfig},
        doctor, launcher, paths,
    };
    let mut launch_id = None;
    match cli.command.take() {
        Some(Command::Setup { command }) => {
            return run_setup(command);
        }
        Some(Command::Inspect { args }) => {
            return run_companion("gui2tui-inspect", args);
        }
        Some(Command::Endpoint { args }) => {
            return run_companion("gui2tui-local", args);
        }
        Some(Command::Doctor {
            verbose,
            json,
            report,
        }) => {
            let result = doctor::run(cli.modality_socket.as_deref()).await;
            if let Some(path) = report {
                result.write_private(&path)?;
                eprintln!(
                    "Contents-free report saved; GUI text, input, queries, payloads and URIs excluded."
                );
            }
            if json {
                println!("{}", serde_json::to_string_pretty(&result)?);
            } else {
                print!("{}", result.text(verbose));
            }
            if !result.healthy() {
                return Err("Diagnostics found a blocking issue; see FAIL guidance above".into());
            }
            return Ok(());
        }
        Some(Command::Config { command }) => {
            let path = paths::config_path()?;
            match command {
                ConfigCommand::Path => println!("{}", path.display()),
                ConfigCommand::Init => {
                    Config::init(&path)?;
                    println!("Created {}", path.display());
                }
                ConfigCommand::Check => {
                    Config::load(&path)?;
                    println!("PASS: {} (defaults when absent)", path.display());
                }
                ConfigCommand::Show => {
                    let mut config = Config::load(&path)?;
                    config.apply_overrides(
                        cli.timeout_ms,
                        cli.event_buffer_capacity,
                        cli.no_mouse,
                    )?;
                    print!("{}", toml::to_string_pretty(&config)?);
                }
            }
            return Ok(());
        }
        Some(Command::App { command }) => {
            let path = paths::config_path()?;
            let mut config = Config::load(&path)?;
            match command {
                AppCommand::Add {
                    program,
                    id,
                    match_name,
                    wait_ms,
                    replace,
                    args,
                } => {
                    let (id, launcher) = complete_launcher_fields(program, id, match_name, args)?;
                    gui2tui::product::launcher::validate_program(&launcher.program)?;
                    if config.launchers.contains_key(&id) && !replace {
                        return Err(format!(
                            "launcher '{id}' already exists; pass --replace to update it"
                        )
                        .into());
                    }
                    config.launchers.insert(
                        id.clone(),
                        LauncherConfig {
                            wait_ms,
                            ..launcher
                        },
                    );
                    config.save(&path)?;
                    println!("Registered launcher '{id}' in {}", path.display());
                    println!(
                        "The real AT-SPI application name will be discovered and saved on the first successful launch."
                    );
                    if let Err(warning) = launcher::validate_launch_environment(
                        &config.launchers.get(&id).unwrap().program,
                    ) {
                        eprintln!("warning: {warning}");
                    }
                    println!("Run it with: gui2tui launch {id}");
                }
                AppCommand::List => {
                    if config.launchers.is_empty() {
                        println!("No launchers registered. Add one with `gui2tui app add`.");
                    }
                    for (id, entry) in &config.launchers {
                        println!(
                            "{id}\tstatus={}\tprogram={}\tmatch={}\targs={}",
                            if entry.verified {
                                "verified"
                            } else {
                                "unverified"
                            },
                            entry.program,
                            entry.match_name,
                            entry.args.len()
                        );
                    }
                }
                AppCommand::Remove { id } => {
                    if config.launchers.remove(&id).is_none() {
                        return Err(format!("launcher '{id}' is not registered").into());
                    }
                    config.save(&path)?;
                    println!("Removed launcher '{id}'");
                }
            }
            return Ok(());
        }
        Some(Command::Launch { id }) => launch_id = Some(id),
        Some(Command::Run) | None => {}
    }
    let mut config = Config::load(&paths::config_path()?)?;
    config.apply_overrides(cli.timeout_ms, cli.event_buffer_capacity, cli.no_mouse)?;
    if let Some(id) = launch_id {
        let registered =
            config.launchers.get(&id).cloned().ok_or_else(|| {
                format!("launcher '{id}' is not registered; run `gui2tui app list`")
            })?;
        eprintln!(
            "Starting launcher '{id}' and waiting up to {} ms for AT-SPI registration...",
            registered.wait_ms
        );
        let outcome = launcher::ensure_running(
            &id,
            &registered,
            Duration::from_millis(config.runtime.backend_timeout_ms),
        )
        .await?;
        if let Some(entry) = config.launchers.get_mut(&id) {
            entry.verified = true;
            if outcome.discovered_name {
                entry.match_name = outcome.application_name.clone();
            }
            config.save(&paths::config_path()?)?;
        }
        if outcome.discovered_name {
            eprintln!(
                "Discovered and saved AT-SPI application name '{}'.",
                outcome.application_name
            );
        }
        cli.app = Some(outcome.application_name);
    }
    if !matches!(cli.log_level, LogLevel::Off) {
        use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
        let path = paths::runtime_dir()?.join("product.log");
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.nlink() != 1
            || metadata.uid() != rustix::process::geteuid().as_raw()
            || metadata.mode() & 0o077 != 0
        {
            return Err("Unsafe diagnostic log path".into());
        }
        file.set_len(0)?;
        // General AT-SPI tracing may include payloads. Deliberately allow only
        // this contents-free product target, never arbitrary RUST_LOG filters.
        let filter = match cli.log_level {
            LogLevel::Debug => "off,gui2tui::product=debug",
            _ => "off,gui2tui::product=info",
        };
        let _ = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_env_filter(EnvFilter::new(filter))
            .with_writer(file)
            .try_init();
    }
    tracing::info!(target: "gui2tui::product", version=env!("CARGO_PKG_VERSION"), "session starting");
    let result = run(cli, config).await;
    tracing::info!(target: "gui2tui::product", success=result.is_ok(), "session stopped");
    result
}

fn companion_path(name: &str) -> Result<std::path::PathBuf, Box<dyn Error>> {
    let current = std::env::current_exe()?;
    let bin = current.parent().ok_or("Current executable has no parent")?;
    let installed = bin.join("../libexec/gui2tui").join(name);
    if installed.is_file() {
        return Ok(installed);
    }
    let sibling = bin.join(name);
    if sibling.is_file() {
        return Ok(sibling);
    }
    // A release binary built directly from a checkout is also a supported
    // developer install (`cargo build --release`). Locate the repository by
    // walking from target/{debug,release}; packaged archives are handled by
    // the libexec path above and never contain this layout.
    if let Some(target_dir) = bin.parent()
        && let Some(project_dir) = target_dir.parent()
    {
        let source = project_dir.join("scripts").join(name);
        if source.is_file() {
            return Ok(source);
        }
    }
    Err(format!("Required internal component '{name}' is missing; reinstall GUI2TUI").into())
}

fn run_companion(name: &str, args: Vec<String>) -> Result<(), Box<dyn Error>> {
    let status = ProcessCommand::new(companion_path(name)?)
        .args(args)
        .status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{name} exited with {status}").into())
    }
}

fn run_setup(command: SetupCommand) -> Result<(), Box<dyn Error>> {
    let helper = companion_path("headless-session")?;
    let current = std::env::current_exe()?;
    let mut args = Vec::new();
    let run_doctor = match command {
        SetupCommand::Persistent { screen } => {
            args.extend(["persistent-start".to_owned(), "--screen".to_owned(), screen]);
            true
        }
        SetupCommand::Temporary { command } => {
            args.push("temporary".to_owned());
            if !command.is_empty() {
                args.push("--".to_owned());
                args.extend(command);
            }
            false
        }
        SetupCommand::Status => {
            args.push("status".to_owned());
            false
        }
        SetupCommand::Stop => {
            args.push("stop".to_owned());
            false
        }
        SetupCommand::Restart { screen } => {
            args.extend(["restart".to_owned(), "--screen".to_owned(), screen]);
            true
        }
    };
    let status = ProcessCommand::new(helper)
        .env("GUI2TUI_SETUP_BINARY", &current)
        .args(args)
        .status()?;
    if !status.success() {
        return Err(format!("headless environment setup exited with {status}").into());
    }
    if run_doctor {
        println!("\nVerifying the managed session with a fresh GUI2TUI process...");
        let status = ProcessCommand::new(current).arg("doctor").status()?;
        if !status.success() {
            return Err("Managed session started, but diagnostics did not pass".into());
        }
    }
    Ok(())
}

fn complete_launcher_fields(
    program: Option<String>,
    id: Option<String>,
    match_name: Option<String>,
    mut args: Vec<String>,
) -> Result<(String, gui2tui::product::config::LauncherConfig), Box<dyn Error>> {
    let interactive = program.is_none();
    if interactive && !io::stdin().is_terminal() {
        return Err(
            "Executable is required when stdin is not a terminal; try `gui2tui app add PROGRAM`"
                .into(),
        );
    }
    let program = match program {
        Some(program) => program,
        None => prompt_required("Executable")?,
    };
    let inferred_id = Path::new(&program)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(program.as_str())
        .to_owned();
    let id = id.unwrap_or(inferred_id);
    let match_name = match_name.unwrap_or_else(|| id.clone());
    if interactive && args.is_empty() {
        loop {
            let argument = prompt("Extra argument (blank to finish)")?;
            if argument.is_empty() {
                break;
            }
            args.push(argument);
        }
    }
    Ok((
        id,
        gui2tui::product::config::LauncherConfig {
            program,
            args,
            match_name,
            ..Default::default()
        },
    ))
}

fn prompt(label: &str) -> io::Result<String> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut value = String::new();
    io::stdin().read_line(&mut value)?;
    Ok(value.trim().to_owned())
}

fn prompt_required(label: &str) -> Result<String, Box<dyn Error>> {
    let value = prompt(label)?;
    if value.is_empty() {
        return Err(format!("{label} cannot be empty").into());
    }
    Ok(value)
}

async fn run(cli: Cli, mut config: gui2tui::product::config::Config) -> Result<(), Box<dyn Error>> {
    let timeout = Duration::from_millis(config.runtime.backend_timeout_ms);
    let recovered = gui2tui::runtime::artifacts::recover_abandoned()?;
    tracing::debug!(
        recovered_artifact_namespaces = recovered,
        "runtime startup recovery"
    );
    let mut signals = RuntimeSignals::new()?;
    let mut guard = TerminalGuard::attach(config.terminal.mouse)?;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous_hook(info);
    }));
    let stdout = io::stdout();
    let terminal_backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(terminal_backend)?;
    let mut terminal_events = EventStream::new();

    #[cfg(debug_assertions)]
    if cli.test_panic_after_attach {
        panic!("controlled Phase 4A terminal restoration failpoint");
    }

    let app_selector = match cli.app {
        Some(name) => name,
        None => {
            let Some(name) = run_selector(
                &mut terminal,
                &mut signals,
                &mut terminal_events,
                timeout,
                config.terminal.mouse,
                &mut config,
            )
            .await?
            else {
                return Ok(());
            };
            name
        }
    };

    let backend = AtspiBackend::connect(timeout).await.map_err(|_| "Desktop accessibility service unavailable. Run gui2tui doctor; use the same desktop session/user.")?;

    terminal.draw(|frame| frame.render_widget(ratatui::widgets::Paragraph::new("Loading the application's accessible interface... Large applications may take a few seconds."), frame.area()))?;
    let initial_terminal = terminal.size()?;
    let mut app = TuiApplication::new(
        backend,
        app_selector,
        InspectOptions {
            verbose: false,
            max_depth: cli.max_depth,
            max_nodes: cli.max_nodes,
        },
        Duration::from_millis(cli.settle_ms),
        cli.bootstrap,
        config.runtime.event_queue_capacity,
        cli.presentation,
        matches!(cli.layout, LayoutMode::Spatial),
        (initial_terminal.width, initial_terminal.height),
    )
    .await?;

    app.configure_modality_client(cli.modality_socket);

    // Keep one Crossterm reader for the lifetime of this terminal. Recreating
    // EventStream while its old poll worker retires can contend on Crossterm's
    // process-global reader lock and block reattachment.
    let mut input_available = true;
    let mut liveness =
        tokio::time::interval(gui2tui::runtime::RuntimeLimits::default().lifecycle_probe);
    liveness.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut content_tick = tokio::time::interval(Duration::from_millis(25));
    content_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut redraw = true;
    loop {
        if redraw && guard.attached {
            terminal.draw(|frame| app.render(frame))?;
        }
        redraw = false;
        tokio::select! {
            signal = signals.recv() => {
                match signal {
                    RuntimeSignal::Stop => break,
                    RuntimeSignal::Detach => {
                        guard.detach();
                        app.set_terminal_attached(false);
                    }
                    RuntimeSignal::Reattach => {
                        tracing::debug!("reattachment signal received");
                        if !guard.attached {
                            app.begin_terminal_reattach();
                            guard = TerminalGuard::attach(config.terminal.mouse)?;
                            tracing::debug!("terminal modes restored for reattachment");
                            // Terminal::clear queries remote cursor position
                            // (DSR), which may block on a PTY without replies.
                            // Fullscreen resize invalidates buffers without DSR.
                            let (width, height) = crossterm::terminal::size()?;
                            terminal.resize(ratatui::layout::Rect::new(0, 0, width, height))?;
                            tracing::debug!("terminal frame invalidated for reattachment");
                            // Discard buffered terminal input, never replay it
                            // as semantic operations after reattachment.
                            for _ in 0..128 {
                                if futures_lite::future::poll_once(terminal_events.next()).await.is_none() { break; }
                            }
                            app.set_terminal_attached(true);
                            redraw = true;
                        }
                    }
                }
            },
            terminal_event = async {
                if guard.attached && input_available {
                    terminal_events.next().await
                } else {
                    std::future::pending().await
                }
            } => {
                let Some(terminal_event) = terminal_event else {
                    input_available = false;
                    guard.detach();
                    app.set_terminal_attached(false);
                    continue;
                };
                redraw = true;
                match terminal_event? {
                    Event::Key(key) => {
                        if !app.is_available() && key.code == crossterm::event::KeyCode::Char('b') {
                                if let Some(name) = run_selector(&mut terminal, &mut signals, &mut terminal_events, timeout, config.terminal.mouse, &mut config).await? {
                                    app.select_fresh_application(name).await;
                                }
                            continue;
                        }
                        if !app.is_available() && key.code == crossterm::event::KeyCode::Char('d') {
                            show_diagnostics(&mut terminal, &mut terminal_events).await?;
                            continue;
                        }
                        if app.handle_key_event(key).await {
                            break;
                        }
                    }
                    Event::Mouse(mouse) => {
                        if config.terminal.mouse && let Some(intent) = mouse_to_intent(mouse) {
                            app.handle_mouse(intent).await;
                        }
                    }
                    Event::Resize(_, _) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
                }
            },
            event = app.next_event() => {
                redraw = true;
                if let Some(event) = event {
                    app.apply_external_delivery(event).await;
                } else {
                    app.handle_event_stream_closed().await;
                }
            },
            _ = liveness.tick() => {
                let was_available = app.is_available();
                app.check_application_available().await;
                redraw |= was_available != app.is_available();
            },
            _ = content_tick.tick() => {
                redraw |= app.has_pending_work();
                app.progress_content_operations().await;
            }
        }
    }
    Ok(())
}

async fn run_selector(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    signals: &mut RuntimeSignals,
    events: &mut EventStream,
    timeout: Duration,
    mouse_enabled: bool,
    config: &mut gui2tui::product::config::Config,
) -> Result<Option<String>, io::Error> {
    let launchers = config.launchers.keys().cloned().collect::<Vec<_>>();
    let mut selector = ApplicationSelector::with_launchers(Vec::new(), launchers.clone());
    refresh_selector(&mut selector, timeout, &launchers).await;
    loop {
        terminal.draw(|frame| selector.render(frame))?;
        let event = tokio::select! {
            signal = signals.recv() => {
                if matches!(signal, RuntimeSignal::Stop) { return Ok(None); }
                continue;
            },
            event = events.next() => match event { Some(event) => event?, None => return Ok(None) },
        };
        match event {
            Event::Key(key) => {
                if selector.filter_key(key) {
                    continue;
                }
                if matches!(
                    key.code,
                    crossterm::event::KeyCode::Char('r') | crossterm::event::KeyCode::F(5)
                ) {
                    refresh_selector(&mut selector, timeout, &launchers).await;
                    continue;
                }
                if key.code == crossterm::event::KeyCode::Char('d') {
                    show_diagnostics(terminal, events).await?;
                    continue;
                }
                if let Some(intent) = key_to_selector_intent(key) {
                    if intent == SelectorIntent::Quit {
                        return Ok(None);
                    }
                    if let Some(target) = selector.handle(intent) {
                        if let Some(name) = resolve_selector_target(
                            terminal,
                            events,
                            &mut selector,
                            target,
                            config,
                            timeout,
                        )
                        .await?
                        {
                            return Ok(Some(name));
                        }
                    }
                }
            }
            Event::Mouse(mouse) => {
                if mouse_enabled
                    && let Some((x, y)) = mouse_click(mouse)
                    && let Some(target) = selector.click(x, y)
                {
                    if let Some(name) = resolve_selector_target(
                        terminal,
                        events,
                        &mut selector,
                        target,
                        config,
                        timeout,
                    )
                    .await?
                    {
                        return Ok(Some(name));
                    }
                }
            }
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
        }
    }
}

async fn resolve_selector_target(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    events: &mut EventStream,
    selector: &mut ApplicationSelector,
    target: SelectorTarget,
    config: &mut gui2tui::product::config::Config,
    timeout: Duration,
) -> io::Result<Option<String>> {
    let SelectorTarget::Launcher(id) = target else {
        let SelectorTarget::Running(name) = target else {
            unreachable!()
        };
        return Ok(Some(name));
    };
    terminal.draw(|frame| {
        frame.render_widget(
            ratatui::widgets::Paragraph::new(format!(
                "Starting '{id}' and waiting for its accessible interface..."
            )),
            frame.area(),
        )
    })?;
    let result = match config.launchers.get(&id).cloned() {
        Some(launcher) => wait_for_launcher(terminal, events, &id, &launcher, timeout).await,
        None => Err(format!(
            "launcher '{id}' disappeared from configuration; restart GUI2TUI"
        )),
    };
    match result {
        Ok(outcome) => {
            if let Some(entry) = config.launchers.get_mut(&id) {
                entry.verified = true;
                if outcome.discovered_name {
                    entry.match_name = outcome.application_name.clone();
                }
                let path = gui2tui::product::paths::config_path().map_err(io::Error::other)?;
                if let Err(error) = config.save(&path) {
                    selector.set_message(format!(
                        "Application '{}' started, but launcher verification could not be saved: {error}",
                        outcome.application_name
                    ));
                }
            }
            Ok(Some(outcome.application_name))
        }
        Err(error) => {
            selector.set_message(error);
            Ok(None)
        }
    }
}

async fn wait_for_launcher(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    events: &mut EventStream,
    id: &str,
    launcher: &gui2tui::product::config::LauncherConfig,
    timeout: Duration,
) -> Result<gui2tui::product::launcher::LaunchOutcome, String> {
    let started = tokio::time::Instant::now();
    let mut launch = Box::pin(gui2tui::product::launcher::ensure_running(
        id, launcher, timeout,
    ));
    let mut redraw = tokio::time::interval(Duration::from_millis(100));
    redraw.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            result = &mut launch => return result,
            event = events.next() => {
                match event {
                    Some(Ok(Event::Key(key))) if matches!(key.code, crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('q')) => {
                        return Err(format!("Launch wait for '{id}' cancelled; the program may still be starting"));
                    }
                    Some(Err(error)) => return Err(format!("Terminal input failed while launching: {error}")),
                    None => return Err("Terminal input ended while launching".into()),
                    _ => {}
                }
            }
            _ = redraw.tick() => {
                let elapsed = started.elapsed().as_millis() as u64;
                let remaining = launcher.wait_ms.saturating_sub(elapsed);
                terminal.draw(|frame| {
                    frame.render_widget(
                        ratatui::widgets::Paragraph::new(format!(
                            "Starting '{id}' and waiting for AT-SPI...\n\n{}.{:01}s remaining\n\nEsc/q: cancel wait",
                            remaining / 1000,
                            (remaining % 1000) / 100,
                        )),
                        frame.area(),
                    )
                }).map_err(|error| format!("Cannot redraw launch status: {error}"))?;
            }
        }
    }
}

async fn refresh_selector(
    selector: &mut ApplicationSelector,
    timeout: Duration,
    launchers: &[String],
) {
    let result = tokio::time::timeout(timeout, async {
        AtspiBackend::connect(timeout).await?.applications().await
    })
    .await;
    match result {
        Ok(Ok(apps)) => selector.replace(
            apps.into_iter().map(|app| app.name).collect(),
            launchers.to_vec(),
            None,
        ),
        _ => selector.replace(Vec::new(), launchers.to_vec(), Some("Desktop accessibility service unavailable. Registered launchers still require a working AT-SPI session; press d for diagnostics.".into())),
    }
}

async fn show_diagnostics(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    events: &mut EventStream,
) -> io::Result<()> {
    use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
    terminal.draw(|frame| {
        frame.render_widget(
            Paragraph::new("Checking session access (bounded probes)..."),
            frame.area(),
        )
    })?;
    let report = gui2tui::product::doctor::run(None).await;
    let text = report.text(false);
    let mut scroll = 0;
    loop {
        terminal.draw(|frame| {
            frame.render_widget(
                Paragraph::new(text.as_str())
                    .wrap(Wrap { trim: true })
                    .scroll((scroll, 0))
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title("Diagnostics — ↑/↓ scroll | Esc return"),
                    ),
                frame.area(),
            )
        })?;
        match events.next().await {
            Some(Ok(Event::Key(key))) => match key.code {
                crossterm::event::KeyCode::Esc | crossterm::event::KeyCode::Char('q') => {
                    return Ok(());
                }
                crossterm::event::KeyCode::Down => scroll = scroll.saturating_add(1),
                crossterm::event::KeyCode::Up => scroll = scroll.saturating_sub(1),
                _ => {}
            },
            None => return Ok(()),
            Some(Err(error)) => return Err(error),
            _ => {}
        }
    }
}

struct TerminalGuard {
    attached: bool,
}

impl TerminalGuard {
    fn attach(mouse: bool) -> io::Result<Self> {
        enable_raw_mode()?;
        let guard = Self { attached: true };
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        if mouse {
            execute!(io::stdout(), EnableMouseCapture)?;
        }
        Ok(guard)
    }
    fn detach(&mut self) {
        if self.attached {
            restore_terminal();
            self.attached = false;
        }
    }
}

fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(
        io::stdout(),
        Show,
        DisableMouseCapture,
        LeaveAlternateScreen
    );
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        self.detach();
    }
}
