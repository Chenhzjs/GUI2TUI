use std::{error::Error, io, process::ExitCode, time::Duration};

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
        selector::{ApplicationSelector, SelectorIntent, key_to_selector_intent, mouse_click},
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
}
#[derive(Debug, Subcommand)]
enum ConfigCommand {
    Path,
    Check,
    Show,
    Init,
}
#[derive(Clone, Copy, Debug, ValueEnum)]
enum LogLevel {
    Off,
    Info,
    Debug,
}

#[tokio::main]
async fn main() -> ExitCode {
    match dispatch(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn dispatch(mut cli: Cli) -> Result<(), Box<dyn Error>> {
    use gui2tui::product::{config::Config, doctor, paths};
    match cli.command.take() {
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
        Some(Command::Run) | None => {}
    }
    let mut config = Config::load(&paths::config_path()?)?;
    config.apply_overrides(cli.timeout_ms, cli.event_buffer_capacity, cli.no_mouse)?;
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

async fn run(cli: Cli, config: gui2tui::product::config::Config) -> Result<(), Box<dyn Error>> {
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
                                if let Some(name) = run_selector(&mut terminal, &mut signals, &mut terminal_events, timeout, config.terminal.mouse).await? {
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
) -> Result<Option<String>, io::Error> {
    let mut selector = ApplicationSelector::new(Vec::new());
    refresh_selector(&mut selector, timeout).await;
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
                    refresh_selector(&mut selector, timeout).await;
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
                    if let Some(name) = selector.handle(intent) {
                        return Ok(Some(name));
                    }
                }
            }
            Event::Mouse(mouse) => {
                if mouse_enabled
                    && let Some((x, y)) = mouse_click(mouse)
                    && let Some(name) = selector.click(x, y)
                {
                    return Ok(Some(name));
                }
            }
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
        }
    }
}

async fn refresh_selector(selector: &mut ApplicationSelector, timeout: Duration) {
    let result = tokio::time::timeout(timeout, async {
        AtspiBackend::connect(timeout).await?.applications().await
    })
    .await;
    match result {
        Ok(Ok(apps)) => selector.replace(apps.into_iter().map(|app| app.name).collect(), None),
        _ => selector.replace(Vec::new(), Some("Desktop accessibility service unavailable. Use the same desktop session/user; press d for diagnostics.".into())),
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
