mod app;
mod display;
mod model;
mod storage;
mod ui;
mod ytdlp;

use anyhow::{Context, Result};
use app::App;
use clap::Parser;
use crossterm::{
    cursor::MoveTo,
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
        Event, EventStream,
    },
    execute,
    style::Print,
    terminal::{
        Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use futures::StreamExt;
use ratatui::{Terminal, backend::CrosstermBackend};
use ratatui_image::picker::Picker;
use std::{
    io::{self, IsTerminal},
    path::PathBuf,
    process::Stdio,
    time::Duration,
};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Parser, Debug)]
#[command(
    name = "tide",
    version,
    about = "Resilient Ratatui command center for yt-dlp"
)]
struct Cli {
    /// Use a custom state file (useful for portable installs)
    #[arg(long)]
    state: Option<PathBuf>,
    /// Override the download directory for this and future sessions
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Disable mouse capture
    #[arg(long)]
    no_mouse: bool,
    /// URL(s) to place in the input field
    urls: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!("Tide needs an interactive terminal (TTY)");
    }
    let state_path = cli.state.unwrap_or_else(storage::state_path);
    let mut state = storage::load(&state_path).unwrap_or_else(|e| {
        eprintln!("warning: could not load saved state: {e:#}");
        model::PersistedState::default()
    });
    if let Some(path) = cli.output {
        let old_default_archive = state.settings.output_dir.join(".tide-archive.txt");
        if state.settings.archive_path == old_default_archive {
            state.settings.archive_path = path.join(".tide-archive.txt");
        }
        state.settings.output_dir = path;
    }
    let help_text = load_help(&state.settings.yt_dlp_path).await;
    let mut app = App::new(state_path, state, help_text);
    app.url = cli.urls.join(" ");

    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = restore_terminal(true);
        old_hook(info);
    }));
    enable_raw_mode().context("enable terminal raw mode")?;
    execute!(io::stdout(), EnterAlternateScreen, EnableBracketedPaste)?;
    if !cli.no_mouse {
        execute!(io::stdout(), EnableMouseCapture)?;
    }
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());
    app.set_image_picker(picker);

    let result = run_loop(&mut terminal, &mut app).await;
    app.save();
    let _ = restore_terminal(!cli.no_mouse);
    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    let mut events = EventStream::new();
    let mut ticker = tokio::time::interval(Duration::from_millis(100));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        terminal.draw(|f| ui::render(f, app))?;
        tokio::select! {
            _ = ticker.tick() => app.on_tick(),
            maybe_event = events.next() => match maybe_event {
                Some(Ok(Event::Key(key))) if key.kind == crossterm::event::KeyEventKind::Press => app.handle_key(key),
                Some(Ok(Event::Mouse(mouse))) => app.click(mouse),
                Some(Ok(Event::Paste(text))) => app.handle_paste(text),
                Some(Ok(Event::Resize(_, _))) => {},
                Some(Ok(_)) => {},
                Some(Err(e)) => return Err(e.into()),
                None => break,
            }
        }
        if app.should_quit {
            break;
        }
        if let Some(bytes) = app.viu_request.take() {
            match show_viu_preview(terminal, bytes).await {
                Ok(()) => {
                    loop {
                        match events.next().await {
                            Some(Ok(Event::Key(key)))
                                if key.kind == crossterm::event::KeyEventKind::Press =>
                            {
                                break;
                            }
                            Some(Ok(Event::Mouse(mouse)))
                                if matches!(
                                    mouse.kind,
                                    crossterm::event::MouseEventKind::Down(_)
                                ) =>
                            {
                                break;
                            }
                            Some(Ok(_)) => {}
                            Some(Err(error)) => {
                                app.notice = format!("viu preview input failed: {error}");
                                break;
                            }
                            None => break,
                        }
                    }
                    app.notice = "Closed full-screen thumbnail preview".into();
                }
                Err(error) => {
                    app.notice = format!("viu preview unavailable: {error}");
                }
            }
            terminal.clear()?;
        }
    }
    Ok(())
}

async fn show_viu_preview(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    bytes: Vec<u8>,
) -> Result<(), String> {
    let area = terminal
        .size()
        .map_err(|error| format!("terminal size: {error}"))?;
    let width = area.width.saturating_sub(4).max(20).to_string();
    terminal
        .clear()
        .map_err(|error| format!("clear terminal: {error}"))?;
    execute!(io::stdout(), MoveTo(0, 0)).map_err(|error| format!("position cursor: {error}"))?;

    let mut child = Command::new("viu")
        .args(["--blocks", "--static", "--width", &width, "-"])
        .stdin(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|error| format!("start viu: {error}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| "viu input pipe was unavailable".to_string())?;
    stdin
        .write_all(&bytes)
        .await
        .map_err(|error| format!("send image to viu: {error}"))?;
    stdin
        .shutdown()
        .await
        .map_err(|error| format!("close viu input: {error}"))?;
    drop(stdin);
    let status = child
        .wait()
        .await
        .map_err(|error| format!("wait for viu: {error}"))?;
    if !status.success() {
        return Err(format!("viu exited with {status}"));
    }
    execute!(
        io::stdout(),
        MoveTo(0, area.height.saturating_sub(1)),
        Clear(ClearType::CurrentLine),
        Print("Press any key or click to return to Tide")
    )
    .map_err(|error| format!("draw preview prompt: {error}"))?;
    Ok(())
}

fn restore_terminal(mouse: bool) -> Result<()> {
    disable_raw_mode()?;
    if mouse {
        execute!(
            io::stdout(),
            DisableMouseCapture,
            DisableBracketedPaste,
            LeaveAlternateScreen
        )?;
    } else {
        execute!(io::stdout(), DisableBracketedPaste, LeaveAlternateScreen)?;
    }
    Ok(())
}

async fn load_help(path: &str) -> String {
    match Command::new(path)
        .arg("--help")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .output()
        .await
    {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).into_owned(),
        _ => "yt-dlp help is unavailable. Check Settings → yt-dlp executable.".into(),
    }
}
