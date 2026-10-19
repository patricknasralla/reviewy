mod app;
mod cli;
mod comments;
mod config;
mod diff;
mod git;
mod highlight;
mod ui;
mod vim;

use std::fs::{self, OpenOptions};
use std::io::{self, Stdout, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use clap::Parser;
use color_eyre::Result;
use color_eyre::eyre::bail;
use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind, KeyModifiers,
};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{cursor, execute};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use tracing_subscriber::EnvFilter;

use app::App;
use cli::Cli;

type Tui = Terminal<CrosstermBackend<Stdout>>;

fn main() -> Result<()> {
    let (panic_hook, eyre_hook) = color_eyre::config::HookBuilder::default().into_hooks();
    eyre_hook.install()?;
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal();
        eprintln!("{}", panic_hook.panic_report(info));
    }));

    let cli = Cli::parse();
    if !cli.diff_args.is_empty() {
        bail!("diff arguments are not supported in v1");
    }

    let terminate = Arc::new(AtomicBool::new(false));
    for signal in [SIGTERM, SIGHUP, SIGINT] {
        signal_hook::flag::register(signal, Arc::clone(&terminate))?;
    }

    let root = git::repo_root()?;
    // task 02: base commit
    // task 03: difft version check
    let _config = config::load(&root)?;
    if let Ok(directive) = std::env::var("REVIEWY_LOG") {
        init_logging(&ensure_reviewy_dir(&root)?, &directive)?;
    }
    // task 02: collect changed files; none → print "No changes" and exit 0
    ensure_reviewy_dir(&root)?;

    let mut terminal = enter_terminal()?;
    let result = run(&mut terminal, &terminate);
    restore_terminal()?;
    result
}

fn run(terminal: &mut Tui, terminate: &AtomicBool) -> Result<()> {
    let mut app = App {
        dirty: true,
        ..App::default()
    };
    loop {
        if terminate.load(Ordering::Relaxed) {
            // task 10: save comments
            break;
        }
        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key)
                    if key.kind == KeyEventKind::Press
                        && key.code == KeyCode::Char('q')
                        && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    app.should_quit = true;
                }
                Event::Resize(..) => app.dirty = true,
                _ => {}
            }
        }
        if app.dirty {
            terminal.draw(|frame| ui::draw(frame, &app))?;
            app.dirty = false;
        }
        if app.should_quit {
            // task 10: save comments
            break;
        }
    }
    Ok(())
}

/// Creates `<root>/.reviewy/` with a `*` `.gitignore`; returns the directory.
fn ensure_reviewy_dir(root: &Path) -> io::Result<PathBuf> {
    let dir = root.join(".reviewy");
    fs::create_dir_all(&dir)?;
    match OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dir.join(".gitignore"))
    {
        Ok(mut file) => file.write_all(b"*\n")?,
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e),
    }
    Ok(dir)
}

fn init_logging(dir: &Path, directive: &str) -> Result<()> {
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("reviewy.log"))?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(directive)?)
        .with_writer(Mutex::new(file))
        .with_ansi(false)
        .init();
    Ok(())
}

fn enter_terminal() -> Result<Tui> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
    Ok(Terminal::new(CrosstermBackend::new(io::stdout()))?)
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        io::stdout(),
        DisableBracketedPaste,
        LeaveAlternateScreen,
        cursor::Show
    )
}
