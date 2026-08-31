use std::{error::Error, io, process::ExitCode, time::Duration};

use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{DisableMouseCapture, EnableMouseCapture, Event, EventStream},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_lite::StreamExt;
use gui2tui::{
    backend::{
        AtspiBackend, BackendError, BootstrapStrategy, DEFAULT_EVENT_BUFFER_CAPACITY,
        InspectOptions,
    },
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
    /// Accessible application name or an unambiguous substring.
    #[arg(long, value_name = "NAME")]
    app: Option<String>,

    /// Maximum accessibility-tree depth per snapshot.
    #[arg(long, default_value_t = 64)]
    max_depth: usize,

    /// Maximum accessibility objects per snapshot.
    #[arg(long, default_value_t = 10_000)]
    max_nodes: usize,

    /// Per-operation D-Bus/AT-SPI timeout in milliseconds.
    #[arg(long, default_value_t = 5_000, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,

    /// Maximum wait for a related AT-SPI event before fallback refresh.
    #[arg(long, default_value_t = 500, value_parser = clap::value_parser!(u64).range(1..))]
    settle_ms: u64,

    /// Initial semantic bootstrap strategy.
    #[arg(long, value_enum, default_value_t = BootstrapStrategy::Auto)]
    bootstrap: BootstrapStrategy,

    /// Maximum buffered AT-SPI events before a correctness resync.
    #[arg(long, default_value_t = DEFAULT_EVENT_BUFFER_CAPACITY)]
    event_buffer_capacity: usize,

    /// Frontend projection pipeline; legacy preserves the Phase 3B flat mapping.
    #[arg(long, value_enum, default_value_t = PresentationMode::Transcompiled)]
    presentation: PresentationMode,

    /// Private local modality broker socket; absent means safe read-only fallback.
    #[arg(long)]
    modality_socket: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() -> ExitCode {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(io::stderr)
        .try_init();
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    let recovered = gui2tui::runtime::artifacts::recover_abandoned()?;
    tracing::debug!(
        recovered_artifact_namespaces = recovered,
        "runtime startup recovery"
    );
    let mut signals = RuntimeSignals::new()?;
    let backend = AtspiBackend::connect(Duration::from_millis(cli.timeout_ms)).await?;
    let mut guard = TerminalGuard::attach()?;
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        previous_hook(info);
    }));
    let stdout = io::stdout();
    let terminal_backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(terminal_backend)?;
    let mut terminal_events = EventStream::new();

    let app_selector = match cli.app {
        Some(name) => name,
        None => {
            let applications = backend.applications().await?;
            if applications.is_empty() {
                return Err(BackendError::NoApplications.into());
            }
            let names = applications.into_iter().map(|app| app.name).collect();
            let Some(name) = run_selector(
                &mut terminal,
                ApplicationSelector::new(names),
                &mut signals,
                &mut terminal_events,
            )
            .await?
            else {
                return Ok(());
            };
            name
        }
    };

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
        cli.event_buffer_capacity,
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
                            guard = TerminalGuard::attach()?;
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
                            if let Ok(backend) = AtspiBackend::connect(Duration::from_millis(cli.timeout_ms)).await
                                && let Ok(applications) = backend.applications().await {
                                let selector = ApplicationSelector::new(applications.into_iter().map(|a| a.name).collect());
                                if let Some(name) = run_selector(&mut terminal, selector, &mut signals, &mut terminal_events).await? {
                                    app.select_fresh_application(name).await;
                                }
                            }
                            continue;
                        }
                        if app.handle_key_event(key).await {
                            break;
                        }
                    }
                    Event::Mouse(mouse) => {
                        if let Some(intent) = mouse_to_intent(mouse) {
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
    mut selector: ApplicationSelector,
    signals: &mut RuntimeSignals,
    events: &mut EventStream,
) -> Result<Option<String>, io::Error> {
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
                if let Some((x, y)) = mouse_click(mouse)
                    && let Some(name) = selector.click(x, y)
                {
                    return Ok(Some(name));
                }
            }
            Event::Resize(_, _) | Event::FocusGained | Event::FocusLost | Event::Paste(_) => {}
        }
    }
}

struct TerminalGuard {
    attached: bool,
}

impl TerminalGuard {
    fn attach() -> io::Result<Self> {
        enable_raw_mode()?;
        let guard = Self { attached: true };
        execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture, Hide)?;
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
