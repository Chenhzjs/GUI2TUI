use std::{error::Error, io, process::ExitCode, time::Duration};

use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, DisableMouseCapture, EnableMouseCapture, Event},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use gui2tui::{
    backend::{AtspiBackend, InspectOptions},
    tui::{
        app::TuiApplication,
        input::{key_to_intent, mouse_to_intent},
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
    app: String,

    /// Maximum accessibility-tree depth per snapshot.
    #[arg(long, default_value_t = 64)]
    max_depth: usize,

    /// Maximum accessibility objects per snapshot.
    #[arg(long, default_value_t = 10_000)]
    max_nodes: usize,

    /// Per-operation D-Bus/AT-SPI timeout in milliseconds.
    #[arg(long, default_value_t = 5_000, value_parser = clap::value_parser!(u64).range(1..))]
    timeout_ms: u64,

    /// Delay after an action before refreshing the semantic snapshot.
    #[arg(long, default_value_t = 500, value_parser = clap::value_parser!(u64).range(1..))]
    settle_ms: u64,
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
    let mut app = TuiApplication::new(
        backend,
        cli.app,
        InspectOptions {
            verbose: false,
            max_depth: cli.max_depth,
            max_nodes: cli.max_nodes,
        },
        Duration::from_millis(cli.settle_ms),
    )
    .await?;

    enable_raw_mode()?;
    let _guard = TerminalGuard;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    loop {
        terminal.draw(|frame| app.render(frame))?;
        match event::read()? {
            Event::Key(key) => {
                if let Some(intent) = key_to_intent(key)
                    && app.handle_intent(intent).await
                {
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
    }
    Ok(())
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
