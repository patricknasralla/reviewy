mod app;
mod cli;
mod comments;
mod config;
mod defaults;
mod diff;
mod editor;
mod git;
mod highlight;
mod ui;
mod vim;

use std::fs::{self, OpenOptions};
use std::io::{self, Stdout, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use clap::Parser;
use color_eyre::Result;
use color_eyre::eyre::{WrapErr, bail};
use crossterm::cursor::SetCursorStyle;
use crossterm::event::{self, DisableBracketedPaste, EnableBracketedPaste, Event};
use crossterm::style::Print;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::{cursor, execute};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::style::Color;
use signal_hook::consts::{SIGHUP, SIGINT, SIGTERM};
use tracing_subscriber::EnvFilter;

use app::App;
use cli::Cli;
use git::DiffTarget;
use highlight::Theme;

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
    let base_commit = git::base_commit(&root)?;
    let config = config::load(&root)?;
    if let Ok(directive) = std::env::var("REVIEWY_LOG") {
        init_logging(&ensure_reviewy_dir(&root)?, &directive)?;
    }
    let variant = config.ui.theme_variant.as_deref();
    let (theme, status) = match config
        .ui
        .theme
        .as_deref()
        .map(|t| Theme::resolve(t, variant, config.ui.tint))
    {
        None => (Theme::bundled_default(config.ui.tint), None),
        Some(Ok(theme)) => (theme, None),
        Some(Err(e)) => {
            let theme = Theme::bundled_default(config.ui.tint);
            tracing::warn!(%e, "using {}", theme.name);
            let status = format!("{e}; using {}", theme.name);
            (theme, Some(status))
        }
    };
    let difft_version = diff::difft::check_version(&config.difft.binary)?;
    tracing::debug!(%difft_version, "difftastic");
    let ignore = git::build_ignore(&config.ignore.paths)?;
    let files = git::changed_files(&root, DiffTarget::WorkingTreeVsHead, &ignore)?;
    if files.is_empty() {
        println!("No changes");
        return Ok(());
    }
    tracing::debug!(?files, "changed files");
    let dir = ensure_reviewy_dir(&root)?;

    let mut app = App::new(config, root, base_commit, files, theme, status);
    app.load_existing(comments::store::load(&dir));
    let mut terminal = enter_terminal(&app.theme, app.config.ui.cursor_blink)?;
    let size = terminal.size()?;
    app.resize(size.width, size.height);
    let result = run(&mut terminal, &terminate, app);
    restore_terminal()?;
    result
}

fn run(terminal: &mut Tui, terminate: &AtomicBool, mut app: App) -> Result<()> {
    loop {
        if terminate.load(Ordering::Relaxed) {
            app.save().wrap_err("saving comments")?;
            break;
        }
        let mut timeout = Duration::from_millis(250);
        if let Some(at) = app.next_blink() {
            timeout = timeout.min(at.saturating_duration_since(Instant::now()));
        }
        if event::poll(timeout)? {
            match event::read()? {
                Event::Key(key) => app.handle_key(key),
                Event::Paste(text) => app.paste(&text),
                Event::Resize(width, height) => app.resize(width, height),
                _ => {}
            }
        }
        app.blink(Instant::now());
        if app.dirty {
            terminal.draw(|frame| ui::draw(frame, &app))?;
            app.dirty = false;
        }
        // "Diffing…" is on screen now; difft blocks for up to ~1 s on big files.
        if app.pending.is_some() {
            app.finish_open();
            terminal.draw(|frame| ui::draw(frame, &app))?;
            app.dirty = false;
        }
        if app.should_quit {
            app.save().wrap_err("saving comments")?;
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

/// The terminal cursor is only ever the insert bar; block cursors are drawn.
fn enter_terminal(theme: &Theme, blink: bool) -> Result<Tui> {
    enable_raw_mode()?;
    let bar = if blink {
        SetCursorStyle::BlinkingBar
    } else {
        SetCursorStyle::SteadyBar
    };
    // OSC 12: cursor color.
    let color = match theme.cursor {
        Color::Rgb(r, g, b) => format!("\x1b]12;#{r:02x}{g:02x}{b:02x}\x07"),
        _ => String::new(),
    };
    execute!(
        io::stdout(),
        EnterAlternateScreen,
        EnableBracketedPaste,
        bar,
        Print(color)
    )?;
    Ok(Terminal::new(CrosstermBackend::new(io::stdout()))?)
}

fn restore_terminal() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(
        io::stdout(),
        DisableBracketedPaste,
        LeaveAlternateScreen,
        cursor::Show,
        SetCursorStyle::DefaultUserShape,
        // OSC 112: reset the cursor color.
        Print("\x1b]112\x07")
    )
}
