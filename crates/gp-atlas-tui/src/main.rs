//! `gp-atlas-tui`: clickable terminal UI for GP Atlas (read-only).

mod app;
mod clip;
mod ui;
mod worker;

use std::io::stdout;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc;
use std::time::Duration;

use clap::Parser;
use gp_atlas_core::manifest::Manifest;
use gp_atlas_core::runner::{RunnerConfig, SfRunner, resolve_sf_bin};
use ratatui::crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use ratatui::crossterm::execute;

use crate::app::App;
use crate::worker::Pool;

#[derive(Parser)]
#[command(
    name = "gp-atlas-tui",
    version,
    about = "GP Atlas terminal UI: browse 1GP/2GP package versions through your installed sf (read-only).",
    long_about = "GP Atlas terminal UI. Read-only companion to the Salesforce CLI (sf 2.150.6 or newer).\n\
                  Not affiliated with or endorsed by Salesforce.\n\
                  Click tabs and rows, or use the keyboard; press ? inside for help."
)]
struct Cli {
    /// Path to the sf binary (default: $GP_ATLAS_SF_BIN, then `sf` on PATH).
    #[arg(long)]
    sf: Option<PathBuf>,
    /// Per-command timeout in seconds.
    #[arg(long, default_value_t = 120)]
    timeout: u64,
    /// Developer override: allow this older CLI version. Results may be wrong.
    #[arg(long, env = "GP_ATLAS_ALLOW_CLI_VERSION")]
    allow_cli_version: Option<String>,
    /// Disable mouse capture (lets the terminal select text normally).
    #[arg(long)]
    no_mouse: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let (bin, _) = match resolve_sf_bin(cli.sf.as_deref()) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("✗ {e}");
            return ExitCode::from(3);
        }
    };
    let manifest = match Manifest::embedded() {
        Ok(m) => m,
        Err(e) => {
            eprintln!("✗ embedded manifest is invalid: {e}");
            return ExitCode::from(4);
        }
    };
    let mut cfg = RunnerConfig::new(bin);
    cfg.timeout = Duration::from_secs(cli.timeout.max(1));

    let (tx, rx) = mpsc::channel();
    let pool = Pool::start(SfRunner::new(cfg), manifest.clone(), tx);
    let mut app = App::new(pool, manifest, cli.allow_cli_version);
    app.start();

    // ratatui::init enables raw mode + alternate screen and restores on panic.
    let mut terminal = ratatui::init();
    if !cli.no_mouse {
        let _ = execute!(stdout(), EnableMouseCapture);
    }
    let result = run(&mut terminal, &mut app, &rx);
    if !cli.no_mouse {
        let _ = execute!(stdout(), DisableMouseCapture);
    }
    ratatui::restore();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(
    terminal: &mut ratatui::DefaultTerminal,
    app: &mut App,
    rx: &mpsc::Receiver<worker::Done>,
) -> std::io::Result<()> {
    while !app.quit {
        terminal.draw(|f| ui::draw(f, app))?;
        // Short poll keeps spinners moving and picks up background results.
        if event::poll(Duration::from_millis(120))? {
            match event::read()? {
                Event::Key(k) => app.on_key(k),
                Event::Mouse(m) => app.on_mouse(m),
                _ => {}
            }
        }
        while let Ok(done) = rx.try_recv() {
            app.on_done(done);
        }
        app.tick();
    }
    Ok(())
}
