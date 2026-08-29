use std::{error::Error, io, process::ExitCode, time::Duration};

use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, EventStream},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_lite::StreamExt;
use gui2tui::{
    backend::{
        AtspiBackend, BackendError, BootstrapStrategy, DEFAULT_EVENT_BUFFER_CAPACITY,
        InspectOptions,
    },
    transcompile::PresentationMode,
    tui::{
        app::TuiApplication,
        input::mouse_to_intent,
        selector::{ApplicationSelector, SelectorIntent, key_to_selector_intent, mouse_click},
    },
};
use ratatui::{Terminal, backend::CrosstermBackend};

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
}

#[tokio::main]
async fn main() -> ExitCode {
    match run(Cli::parse()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<(), Box<dyn Error>> {
    let backend = AtspiBackend::connect(Duration::from_millis(cli.timeout_ms)).await?;
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
    let terminal_backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(terminal_backend)?;

    let app_selector = match cli.app {
        Some(name) => name,
        None => {
            let applications = backend.applications().await?;
            if applications.is_empty() {
                return Err(BackendError::NoApplications.into());
            }
            let names = applications.into_iter().map(|app| app.name).collect();
            let Some(name) = run_selector(&mut terminal, ApplicationSelector::new(names))? else {
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

    let terminal_events = EventStream::new();
    futures_lite::pin!(terminal_events);
    let mut liveness = tokio::time::interval(Duration::from_millis(500));
    liveness.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        terminal.draw(|frame| app.render(frame))?;
        tokio::select! {
            terminal_event = terminal_events.next() => {
                let Some(terminal_event) = terminal_event else { break };
                match terminal_event? {
                    Event::Key(key) => {
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
                if let Some(event) = event {
                    app.apply_external_delivery(event).await;
                } else {
                    app.handle_event_stream_closed().await;
                }
            },
            _ = liveness.tick() => {
                app.check_application_available().await;
            }
        }
    }
    Ok(())
}

fn run_selector(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut selector: ApplicationSelector,
) -> Result<Option<String>, io::Error> {
    loop {
        terminal.draw(|frame| selector.render(frame))?;
        match event::read()? {
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

struct TerminalGuard;

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            Show,
            DisableMouseCapture,
            LeaveAlternateScreen
        );
    }
}
