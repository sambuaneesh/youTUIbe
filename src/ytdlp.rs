use crate::model::{DownloadOptions, Job, MediaMode, Quality, Settings};
use serde::Deserialize;
use std::{collections::BTreeMap, path::Path, process::Stdio};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixStream,
    process::Command,
    sync::mpsc,
};
use uuid::Uuid;

const PROGRESS_PREFIX: &str = "__YOUTUIBE_PROGRESS__\t";
const DONE_PREFIX: &str = "__YOUTUIBE_DONE__\t";

#[derive(Debug, Clone)]
pub enum Control {
    Pause,
    Resume,
    Cancel,
}

#[derive(Debug, Clone)]
pub enum PlayerControl {
    Pause,
    Resume,
    Stop,
    SeekRelative(f64),
    SeekPercent(f64),
    SetVolume(f64),
    ToggleMute,
    SetSpeed(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerMode {
    Audio,
    Video,
}

#[derive(Debug, Clone)]
pub struct PlayerStatus {
    pub position: f64,
    pub duration: f64,
    pub volume: f64,
    pub speed: f64,
    pub paused: bool,
    pub muted: bool,
}

impl Default for PlayerStatus {
    fn default() -> Self {
        Self {
            position: 0.0,
            duration: 0.0,
            volume: 100.0,
            speed: 1.0,
            paused: false,
            muted: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum EngineEvent {
    Started(Uuid),
    Progress {
        id: Uuid,
        downloaded: u64,
        total: u64,
        speed: f64,
        eta: Option<u64>,
        title: String,
    },
    Processing(Uuid),
    Output {
        id: Uuid,
        path: String,
    },
    Log {
        id: Uuid,
        line: String,
    },
    Paused(Uuid),
    Resumed(Uuid),
    Finished {
        id: Uuid,
        success: bool,
        error: String,
    },
    Inspected {
        token: u64,
        result: Result<VideoInfo, String>,
    },
    SearchFinished {
        token: u64,
        result: Result<Vec<SearchResult>, String>,
    },
    SearchInspected {
        token: u64,
        result: Result<VideoInfo, String>,
    },
    ThumbnailReady {
        token: u64,
        result: Result<ThumbnailData, String>,
    },
    PlayerStarted(u64),
    PlayerStatus {
        token: u64,
        status: PlayerStatus,
    },
    PlayerFinished {
        token: u64,
        error: String,
    },
}

#[derive(Debug, Clone)]
pub struct ThumbnailData {
    pub image: image::DynamicImage,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub uploader: String,
    pub duration: Option<f64>,
    pub url: String,
    pub thumbnail_url: String,
    pub view_count: Option<u64>,
    pub live_status: String,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    entries: Vec<SearchEntry>,
}

#[derive(Debug, Deserialize)]
struct SearchEntry {
    #[serde(default, deserialize_with = "string_or_default")]
    id: String,
    #[serde(default, deserialize_with = "string_or_default")]
    title: String,
    #[serde(default, deserialize_with = "string_or_default")]
    uploader: String,
    #[serde(default, deserialize_with = "string_or_default")]
    channel: String,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default, deserialize_with = "string_or_default")]
    url: String,
    #[serde(default, deserialize_with = "string_or_default")]
    webpage_url: String,
    #[serde(default)]
    thumbnails: Vec<SearchThumbnail>,
    #[serde(default)]
    view_count: Option<u64>,
    #[serde(default, deserialize_with = "string_or_default")]
    live_status: String,
}

#[derive(Debug, Deserialize)]
struct SearchThumbnail {
    #[serde(default, deserialize_with = "string_or_default")]
    url: String,
    #[serde(default)]
    width: Option<u64>,
    #[serde(default)]
    height: Option<u64>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct VideoInfo {
    #[serde(default, deserialize_with = "string_or_default")]
    pub id: String,
    #[serde(default, deserialize_with = "string_or_default")]
    pub title: String,
    #[serde(default, deserialize_with = "string_or_default")]
    pub uploader: String,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default, deserialize_with = "string_or_default")]
    pub webpage_url: String,
    #[serde(default, deserialize_with = "string_or_default")]
    pub live_status: String,
    #[serde(default)]
    pub formats: Vec<FormatInfo>,
    #[serde(default, deserialize_with = "map_or_default")]
    pub subtitles: BTreeMap<String, serde_json::Value>,
    #[serde(default, deserialize_with = "map_or_default")]
    pub automatic_captions: BTreeMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct FormatInfo {
    #[serde(default, deserialize_with = "string_or_default")]
    pub format_id: String,
    #[serde(default, deserialize_with = "string_or_default")]
    pub ext: String,
    #[serde(default, deserialize_with = "string_or_default")]
    pub resolution: String,
    #[serde(default, deserialize_with = "string_or_default")]
    pub vcodec: String,
    #[serde(default, deserialize_with = "string_or_default")]
    pub acodec: String,
    #[serde(default)]
    pub filesize: Option<u64>,
    #[serde(default)]
    pub filesize_approx: Option<u64>,
    #[serde(default)]
    pub width: Option<u32>,
    #[serde(default)]
    pub height: Option<u32>,
    #[serde(default)]
    pub fps: Option<f64>,
    #[serde(default)]
    pub abr: Option<f64>,
    #[serde(default)]
    pub tbr: Option<f64>,
    #[serde(default)]
    pub audio_channels: Option<u32>,
    #[serde(default, deserialize_with = "string_or_default")]
    pub language: String,
    #[serde(
        default,
        rename = "format_note",
        deserialize_with = "string_or_default"
    )]
    pub note: String,
}

fn string_or_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Option::unwrap_or_default)
}

fn map_or_default<'de, D>(deserializer: D) -> Result<BTreeMap<String, serde_json::Value>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<BTreeMap<String, serde_json::Value>>::deserialize(deserializer)
        .map(Option::unwrap_or_default)
}

pub fn build_args(
    options: &DownloadOptions,
    settings: &Settings,
    url: &str,
) -> Result<Vec<String>, String> {
    if settings.output_dir.as_os_str().is_empty() {
        return Err("Output directory cannot be empty".into());
    }
    if settings.output_template.trim().is_empty() {
        return Err("Filename template cannot be empty".into());
    }
    if settings.archive && settings.archive_path.as_os_str().is_empty() {
        return Err("Archive is enabled but its file path is empty".into());
    }
    if settings.use_aria2 && !executable_exists("aria2c") {
        return Err("aria2 was enabled, but aria2c is not installed or not on PATH".into());
    }
    if !settings.cookies_file.trim().is_empty() && !Path::new(&settings.cookies_file).is_file() {
        return Err(format!(
            "Cookies file does not exist: {}",
            settings.cookies_file
        ));
    }
    let mut a = vec![
        "--ignore-config".into(),
        "--newline".into(),
        "--progress".into(),
        "--progress-delta".into(),
        "0.2".into(),
        "--progress-template".into(),
        format!(
            "download:{PROGRESS_PREFIX}%(progress.status)s\t%(progress.downloaded_bytes)s\t%(progress.total_bytes,progress.total_bytes_estimate)s\t%(progress.speed)s\t%(progress.eta)s\t%(info.title)s"
        ),
        "--print".into(),
        format!("after_move:{DONE_PREFIX}%(filepath)s"),
        "--paths".into(),
        settings.output_dir.to_string_lossy().into_owned(),
        "--output".into(),
        settings.output_template.clone(),
        "--concurrent-fragments".into(),
        settings.concurrent_fragments.clamp(1, 32).to_string(),
        "--retries".into(),
        settings.retries.clone(),
        "--fragment-retries".into(),
        settings.fragment_retries.clone(),
        "--socket-timeout".into(),
        settings.socket_timeout.to_string(),
    ];

    flag(
        &mut a,
        settings.continue_downloads,
        "--continue",
        "--no-continue",
    );
    flag(
        &mut a,
        settings.no_overwrites,
        "--no-overwrites",
        "--force-overwrites",
    );
    // .part is deliberately retained even when aria2 is selected; it is the basis for recovery.
    a.push("--part".into());
    if !settings.retry_sleep.trim().is_empty() {
        pair(&mut a, "--retry-sleep", &settings.retry_sleep);
    }
    if !settings.rate_limit.trim().is_empty() {
        pair(&mut a, "--limit-rate", &settings.rate_limit);
    }
    if !settings.throttled_rate.trim().is_empty() {
        pair(&mut a, "--throttled-rate", &settings.throttled_rate);
    }
    if settings.sleep_requests > 0.0 {
        pair(
            &mut a,
            "--sleep-requests",
            &settings.sleep_requests.to_string(),
        );
    }
    if settings.archive {
        pair(
            &mut a,
            "--download-archive",
            &settings.archive_path.to_string_lossy(),
        );
    }
    if settings.restrict_filenames {
        a.push("--restrict-filenames".into());
    }
    if settings.write_playlist_metafiles {
        a.push("--write-playlist-metafiles".into());
    }
    if !settings.cookies_browser.trim().is_empty() {
        pair(&mut a, "--cookies-from-browser", &settings.cookies_browser);
    }
    if !settings.cookies_file.trim().is_empty() {
        pair(&mut a, "--cookies", &settings.cookies_file);
    }
    if !settings.proxy.trim().is_empty() {
        pair(&mut a, "--proxy", &settings.proxy);
    }
    if !settings.impersonate.trim().is_empty() {
        pair(&mut a, "--impersonate", &settings.impersonate);
    }
    if !settings.geo_bypass_country.trim().is_empty() {
        pair(&mut a, "--geo-bypass-country", &settings.geo_bypass_country);
    }
    if !settings.user_agent.trim().is_empty() {
        pair(&mut a, "--user-agent", &settings.user_agent);
    }
    if !settings.ffmpeg_location.trim().is_empty() {
        pair(&mut a, "--ffmpeg-location", &settings.ffmpeg_location);
    }
    if settings.use_aria2 {
        a.extend([
            "--downloader".into(),
            "aria2c".into(),
            "--downloader-args".into(),
        ]);
        let n = settings.aria2_connections.clamp(1, 16);
        a.push(format!(
            "aria2c:-x {n} -s {n} -k 1M --continue=true --file-allocation=none --summary-interval=1"
        ));
    }

    match options.mode {
        MediaMode::Video => {
            pair(
                &mut a,
                "--format",
                &format_selector(&options.quality, &options.video_container),
            );
            if options.video_container != "auto" {
                pair(&mut a, "--merge-output-format", &options.video_container);
            }
        }
        MediaMode::VideoOnly => pair(&mut a, "--format", &video_only_selector(&options.quality)),
        MediaMode::Audio => {
            if let Quality::Custom(format) = &options.quality {
                pair(&mut a, "--format", format);
            }
            a.push("--extract-audio".into());
            pair(&mut a, "--audio-format", &options.audio_format);
            if options.audio_format != "best" {
                pair(&mut a, "--audio-quality", &options.audio_quality);
            }
        }
        MediaMode::MetadataOnly => a.push("--skip-download".into()),
    }

    flag(&mut a, options.playlist, "--yes-playlist", "--no-playlist");
    if !options.playlist_items.trim().is_empty() {
        pair(&mut a, "--playlist-items", &options.playlist_items);
    }
    if options.subtitles {
        a.push("--write-subs".into());
    }
    if options.auto_subtitles {
        a.push("--write-auto-subs".into());
    }
    if options.subtitles || options.auto_subtitles {
        pair(&mut a, "--sub-langs", &options.subtitle_languages);
    }
    if options.embed_subtitles {
        a.push("--embed-subs".into());
    }
    if options.thumbnail {
        a.push("--write-thumbnail".into());
    }
    let automatic_audio_cover = options.mode == MediaMode::Audio;
    if options.embed_thumbnail || automatic_audio_cover {
        a.push("--embed-thumbnail".into());
        if automatic_audio_cover {
            // JPEG cover art is broadly understood by desktop/mobile music
            // libraries. yt-dlp selects its highest-quality thumbnail first.
            pair(&mut a, "--convert-thumbnails", "jpg");
        }
    }
    if options.metadata || automatic_audio_cover {
        a.push("--embed-metadata".into());
    }
    if options.chapters {
        a.push("--embed-chapters".into());
    }
    if options.sponsorblock {
        pair(&mut a, "--sponsorblock-remove", &options.sponsor_categories);
    }
    if options.comments {
        a.push("--write-comments".into());
    }
    if options.info_json {
        a.push("--write-info-json".into());
    }
    if options.description {
        a.push("--write-description".into());
    }
    if options.live_from_start {
        a.push("--live-from-start".into());
    }
    if !options.date_after.trim().is_empty() {
        pair(&mut a, "--dateafter", &options.date_after);
    }
    if !options.date_before.trim().is_empty() {
        pair(&mut a, "--datebefore", &options.date_before);
    }
    if !options.match_filter.trim().is_empty() {
        pair(&mut a, "--match-filter", &options.match_filter);
    }
    if !options.raw_args.trim().is_empty() {
        let raw = shell_words::split(&options.raw_args)
            .map_err(|e| format!("invalid advanced arguments: {e}"))?;
        // URL is appended last so raw arguments cannot accidentally consume it.
        a.extend(raw);
    }
    a.push("--".into());
    a.push(url.into());
    Ok(a)
}

fn pair(a: &mut Vec<String>, key: &str, value: &str) {
    a.push(key.into());
    a.push(value.into());
}
fn flag(a: &mut Vec<String>, yes: bool, y: &str, n: &str) {
    a.push(if yes { y } else { n }.into());
}

fn dimension_filter(q: &Quality) -> String {
    match q {
        Quality::Screen { width, height } if *width > 0 && *height > 0 => {
            format!("[width<={width}][height<={height}]")
        }
        Quality::P2160 => "[height<=2160]".into(),
        Quality::P1440 => "[height<=1440]".into(),
        Quality::P1080 => "[height<=1080]".into(),
        Quality::P720 => "[height<=720]".into(),
        Quality::P480 => "[height<=480]".into(),
        Quality::P360 => "[height<=360]".into(),
        _ => String::new(),
    }
}

fn format_selector(q: &Quality, container: &str) -> String {
    if let Quality::Custom(s) = q {
        return s.clone();
    }
    if matches!(q, Quality::Smallest) {
        return "worstvideo*+worstaudio/worst".into();
    }
    let cap = dimension_filter(q);
    if container == "mp4" {
        format!("bv*{cap}[vcodec^=avc]+ba[acodec^=mp4a]/b{cap}[ext=mp4]/bv*{cap}+ba/b{cap}")
    } else {
        format!("bv*{cap}+ba/b{cap}")
    }
}

fn video_only_selector(q: &Quality) -> String {
    if let Quality::Custom(s) = q {
        return s.clone();
    }
    let cap = dimension_filter(q);
    if matches!(q, Quality::Smallest) {
        "wv".into()
    } else {
        format!("bv{cap}")
    }
}

pub fn command_preview(job: &Job, settings: &Settings) -> String {
    match build_args(&job.options, settings, &job.url) {
        Ok(args) => {
            let mut redact_next = false;
            let shown: Vec<String> = args
                .iter()
                .map(|arg| {
                    if redact_next {
                        redact_next = false;
                        return "<redacted>".into();
                    }
                    if matches!(
                        arg.as_str(),
                        "--proxy"
                            | "--username"
                            | "--password"
                            | "--video-password"
                            | "--ap-password"
                    ) {
                        redact_next = true;
                    }
                    arg.clone()
                })
                .collect();
            std::iter::once(settings.yt_dlp_path.as_str())
                .chain(shown.iter().map(String::as_str))
                .map(shell_words::quote)
                .map(|s| s.into_owned())
                .collect::<Vec<_>>()
                .join(" ")
        }
        Err(e) => format!("Error: {e}"),
    }
}

pub async fn inspect(
    url: String,
    settings: Settings,
    token: u64,
    tx: mpsc::UnboundedSender<EngineEvent>,
) {
    let result = fetch_video_info(url, settings).await;
    let _ = tx.send(EngineEvent::Inspected { token, result });
}

pub async fn inspect_search(
    url: String,
    settings: Settings,
    token: u64,
    tx: mpsc::UnboundedSender<EngineEvent>,
) {
    let result = fetch_video_info(url, settings).await;
    let _ = tx.send(EngineEvent::SearchInspected { token, result });
}

async fn fetch_video_info(url: String, settings: Settings) -> Result<VideoInfo, String> {
    let inspect_timeout =
        std::time::Duration::from_secs(settings.socket_timeout.saturating_mul(2).clamp(20, 60));
    let mut cmd = Command::new(&settings.yt_dlp_path);
    cmd.kill_on_drop(true);
    add_access_args(&mut cmd, &settings);
    cmd.args([
        "--ignore-config",
        "--dump-single-json",
        "--no-warnings",
        "--playlist-items",
        "1",
        "--",
        &url,
    ]);
    match tokio::time::timeout(inspect_timeout, cmd.output()).await {
        Ok(Ok(out)) if out.status.success() => serde_json::from_slice::<VideoInfo>(&out.stdout)
            .map_err(|e| format!("Could not parse yt-dlp metadata: {e}")),
        Ok(Ok(out)) => {
            let msg = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if msg.is_empty() {
                format!("yt-dlp exited with {}", out.status)
            } else {
                msg
            })
        }
        Ok(Err(e)) => Err(format!("Could not run {}: {e}", settings.yt_dlp_path)),
        Err(_) => Err(format!(
            "Inspection exceeded the {} second safety timeout",
            inspect_timeout.as_secs()
        )),
    }
}

pub fn start_player(
    url: String,
    format: String,
    mode: PlayerMode,
    token: u64,
    tx: mpsc::UnboundedSender<EngineEvent>,
) -> mpsc::UnboundedSender<PlayerControl> {
    let (control_tx, control_rx) = mpsc::unbounded_channel();
    tokio::spawn(run_player(url, format, mode, token, tx, control_rx));
    control_tx
}

async fn run_player(
    url: String,
    format: String,
    mode: PlayerMode,
    token: u64,
    tx: mpsc::UnboundedSender<EngineEvent>,
    mut controls: mpsc::UnboundedReceiver<PlayerControl>,
) {
    let socket_path =
        std::env::temp_dir().join(format!("youtuibe-mpv-{}-{token}.sock", std::process::id()));
    if socket_path.exists() {
        let _ = std::fs::remove_file(&socket_path);
    }
    let socket_arg = format!("--input-ipc-server={}", socket_path.display());
    let mut command = Command::new("mpv");
    command.args(player_args(&url, &format, mode, &socket_arg));
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let _ = tx.send(EngineEvent::PlayerFinished {
                token,
                error: format!("Could not start mpv: {error}"),
            });
            return;
        }
    };
    let pid = child.id();
    let mut connect_attempts = 0_u16;
    let stream = loop {
        match UnixStream::connect(&socket_path).await {
            Ok(stream) => break stream,
            Err(error) => {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let _ = std::fs::remove_file(&socket_path);
                        let _ = tx.send(EngineEvent::PlayerFinished {
                            token,
                            error: format!("mpv exited with {status}: {error}"),
                        });
                        return;
                    }
                    Err(wait_error) => {
                        let _ = tx.send(EngineEvent::PlayerFinished {
                            token,
                            error: format!("Could not connect to mpv: {wait_error}"),
                        });
                        return;
                    }
                    Ok(None) => {}
                }
                match controls.try_recv() {
                    Ok(PlayerControl::Stop)
                    | Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                        kill_player_group(pid);
                        let _ = child.wait().await;
                        let _ = std::fs::remove_file(&socket_path);
                        let _ = tx.send(EngineEvent::PlayerFinished {
                            token,
                            error: String::new(),
                        });
                        return;
                    }
                    Ok(_) | Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {}
                }
                connect_attempts += 1;
                if connect_attempts >= 100 {
                    kill_player_group(pid);
                    let _ = child.wait().await;
                    let _ = std::fs::remove_file(&socket_path);
                    let _ = tx.send(EngineEvent::PlayerFinished {
                        token,
                        error: format!("mpv control socket did not become ready: {error}"),
                    });
                    return;
                }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }
    };
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    for (id, property) in ["time-pos", "duration", "volume", "speed", "pause", "mute"]
        .iter()
        .enumerate()
    {
        if send_player_command(
            &mut writer,
            serde_json::json!({
                "command": ["observe_property", id + 1, property]
            }),
        )
        .await
        .is_err()
        {
            kill_player_group(pid);
            let _ = child.wait().await;
            let _ = std::fs::remove_file(&socket_path);
            let _ = tx.send(EngineEvent::PlayerFinished {
                token,
                error: "Could not initialize mpv controls".into(),
            });
            return;
        }
    }
    let _ = tx.send(EngineEvent::PlayerStarted(token));
    let mut player_status = PlayerStatus::default();
    let mut stopped = false;
    let mut ipc_open = true;
    loop {
        tokio::select! {
            status = child.wait() => {
                let error = match status {
                    Ok(status) if status.success() || stopped => String::new(),
                    Ok(status) => format!("mpv exited with {status}"),
                    Err(error) => format!("Could not wait for mpv: {error}"),
                };
                let _ = std::fs::remove_file(&socket_path);
                let _ = tx.send(EngineEvent::PlayerFinished { token, error });
                return;
            }
            control = controls.recv() => match control {
                Some(PlayerControl::Pause) => {
                    let _ = send_player_command(&mut writer, serde_json::json!({"command": ["set_property", "pause", true]})).await;
                }
                Some(PlayerControl::Resume) => {
                    let _ = send_player_command(&mut writer, serde_json::json!({"command": ["set_property", "pause", false]})).await;
                }
                Some(PlayerControl::Stop) | None => {
                    stopped = true;
                    kill_player_group(pid);
                }
                Some(PlayerControl::SeekRelative(seconds)) => {
                    let _ = send_player_command(&mut writer, serde_json::json!({"command": ["seek", seconds, "relative+exact"]})).await;
                }
                Some(PlayerControl::SeekPercent(percent)) => {
                    let _ = send_player_command(&mut writer, serde_json::json!({"command": ["seek", percent.clamp(0.0, 100.0), "absolute-percent+exact"]})).await;
                }
                Some(PlayerControl::SetVolume(volume)) => {
                    let _ = send_player_command(&mut writer, serde_json::json!({"command": ["set_property", "volume", volume.clamp(0.0, 100.0)]})).await;
                }
                Some(PlayerControl::ToggleMute) => {
                    let _ = send_player_command(&mut writer, serde_json::json!({"command": ["cycle", "mute"]})).await;
                }
                Some(PlayerControl::SetSpeed(speed)) => {
                    let _ = send_player_command(&mut writer, serde_json::json!({"command": ["set_property", "speed", speed.clamp(0.25, 3.0)]})).await;
                }
            },
            line = lines.next_line(), if ipc_open => match line {
                Ok(Some(line)) => {
                    if update_player_status(&line, &mut player_status) {
                        let _ = tx.send(EngineEvent::PlayerStatus {
                            token,
                            status: player_status.clone(),
                        });
                    }
                }
                Ok(None) | Err(_) => ipc_open = false,
            }
        }
    }
}

fn player_args(url: &str, format: &str, mode: PlayerMode, socket_arg: &str) -> Vec<String> {
    let mut args = vec![
        "--no-config".into(),
        "--really-quiet".into(),
        "--no-terminal".into(),
        "--ytdl=yes".into(),
        format!("--ytdl-format={format}"),
        socket_arg.into(),
    ];
    match mode {
        PlayerMode::Audio => {
            args.push("--no-video".into());
            args.push("--force-window=no".into());
        }
        PlayerMode::Video => {
            args.push("--force-window=yes".into());
        }
    }
    args.extend(["--".into(), url.into()]);
    args
}

async fn send_player_command(
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    command: serde_json::Value,
) -> std::io::Result<()> {
    writer.write_all(command.to_string().as_bytes()).await?;
    writer.write_all(b"\n").await
}

fn update_player_status(line: &str, status: &mut PlayerStatus) -> bool {
    let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
        return false;
    };
    if message.get("event").and_then(|value| value.as_str()) != Some("property-change") {
        return false;
    }
    let Some(name) = message.get("name").and_then(|value| value.as_str()) else {
        return false;
    };
    let data = &message["data"];
    match name {
        "time-pos" => status.position = data.as_f64().unwrap_or(status.position),
        "duration" => status.duration = data.as_f64().unwrap_or(status.duration),
        "volume" => status.volume = data.as_f64().unwrap_or(status.volume),
        "speed" => status.speed = data.as_f64().unwrap_or(status.speed),
        "pause" => status.paused = data.as_bool().unwrap_or(status.paused),
        "mute" => status.muted = data.as_bool().unwrap_or(status.muted),
        _ => return false,
    }
    true
}

fn kill_player_group(pid: Option<u32>) {
    #[cfg(unix)]
    if let Some(pid) = pid {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
        }
    }
}

pub async fn search(
    query: String,
    limit: usize,
    settings: Settings,
    token: u64,
    tx: mpsc::UnboundedSender<EngineEvent>,
) {
    let target = format!("ytsearch{}:{}", limit.clamp(1, 50), query.trim());
    let socket_timeout = settings.socket_timeout.to_string();
    let search_timeout =
        std::time::Duration::from_secs(settings.socket_timeout.saturating_mul(2).clamp(20, 60));
    let mut cmd = Command::new(&settings.yt_dlp_path);
    cmd.kill_on_drop(true);
    add_access_args(&mut cmd, &settings);
    cmd.args([
        "--ignore-config",
        "--flat-playlist",
        "--dump-single-json",
        "--no-warnings",
        "--socket-timeout",
        &socket_timeout,
        "--",
        &target,
    ]);
    let result = match tokio::time::timeout(search_timeout, cmd.output()).await {
        Ok(Ok(out)) if out.status.success() => {
            serde_json::from_slice::<SearchResponse>(&out.stdout)
                .map(|response| {
                    response
                        .entries
                        .into_iter()
                        .map(|entry| {
                            let thumbnail = entry
                                .thumbnails
                                .iter()
                                .filter(|thumb| !thumb.url.is_empty())
                                .max_by_key(|thumb| {
                                    thumb.width.unwrap_or(0) * thumb.height.unwrap_or(0)
                                })
                                .map(|thumb| thumb.url.clone())
                                .unwrap_or_else(|| {
                                    format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", entry.id)
                                });
                            SearchResult {
                                id: entry.id.clone(),
                                title: if entry.title.is_empty() {
                                    format!("Video {}", entry.id)
                                } else {
                                    entry.title
                                },
                                uploader: if entry.uploader.is_empty() {
                                    entry.channel
                                } else {
                                    entry.uploader
                                },
                                duration: entry.duration,
                                url: if entry.webpage_url.is_empty() {
                                    if entry.url.starts_with("http") {
                                        entry.url
                                    } else {
                                        format!("https://www.youtube.com/watch?v={}", entry.id)
                                    }
                                } else {
                                    entry.webpage_url
                                },
                                thumbnail_url: thumbnail,
                                view_count: entry.view_count,
                                live_status: entry.live_status,
                            }
                        })
                        .collect()
                })
                .map_err(|e| format!("Could not parse search results: {e}"))
        }
        Ok(Ok(out)) => {
            let message = String::from_utf8_lossy(&out.stderr).trim().to_string();
            Err(if message.is_empty() {
                format!("yt-dlp search exited with {}", out.status)
            } else {
                message
            })
        }
        Ok(Err(e)) => Err(format!("Could not run {}: {e}", settings.yt_dlp_path)),
        Err(_) => Err(format!(
            "Search exceeded the {} second safety timeout",
            search_timeout.as_secs()
        )),
    };
    let _ = tx.send(EngineEvent::SearchFinished { token, result });
}

pub async fn fetch_thumbnail(url: String, token: u64, tx: mpsc::UnboundedSender<EngineEvent>) {
    const MAX_THUMBNAIL_BYTES: usize = 8 * 1024 * 1024;
    let result = async {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(12))
            .user_agent("youTUIbe/0.1 thumbnail preview")
            .build()
            .map_err(|e| format!("thumbnail client: {e}"))?;
        let response = client
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|e| format!("thumbnail request: {e}"))?;
        if response
            .content_length()
            .is_some_and(|n| n > MAX_THUMBNAIL_BYTES as u64)
        {
            return Err("thumbnail is larger than the 8 MiB safety limit".into());
        }
        let bytes = response
            .bytes()
            .await
            .map_err(|e| format!("thumbnail body: {e}"))?;
        if bytes.len() > MAX_THUMBNAIL_BYTES {
            return Err("thumbnail is larger than the 8 MiB safety limit".into());
        }
        let source = bytes.to_vec();
        let image = tokio::task::spawn_blocking(move || image::load_from_memory(&bytes))
            .await
            .map_err(|e| format!("thumbnail decoder task: {e}"))?
            .map_err(|e| format!("thumbnail decode: {e}"))?;
        Ok(ThumbnailData {
            image,
            bytes: source,
        })
    }
    .await;
    let _ = tx.send(EngineEvent::ThumbnailReady { token, result });
}

fn add_access_args(cmd: &mut Command, settings: &Settings) {
    if !settings.cookies_browser.trim().is_empty() {
        cmd.args(["--cookies-from-browser", &settings.cookies_browser]);
    }
    if !settings.cookies_file.trim().is_empty() {
        cmd.args(["--cookies", &settings.cookies_file]);
    }
    if !settings.proxy.trim().is_empty() {
        cmd.args(["--proxy", &settings.proxy]);
    }
    if !settings.impersonate.trim().is_empty() {
        cmd.args(["--impersonate", &settings.impersonate]);
    }
}

pub fn start(
    job: Job,
    settings: Settings,
    tx: mpsc::UnboundedSender<EngineEvent>,
) -> mpsc::UnboundedSender<Control> {
    let (control_tx, control_rx) = mpsc::unbounded_channel();
    tokio::spawn(run(job, settings, tx, control_rx));
    control_tx
}

async fn run(
    job: Job,
    settings: Settings,
    tx: mpsc::UnboundedSender<EngineEvent>,
    mut controls: mpsc::UnboundedReceiver<Control>,
) {
    let id = job.id;
    if let Err(e) = tokio::fs::create_dir_all(&settings.output_dir).await {
        let _ = tx.send(EngineEvent::Finished {
            id,
            success: false,
            error: format!("Cannot create output directory: {e}"),
        });
        return;
    }
    if settings.archive
        && let Some(parent) = settings
            .archive_path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
        && let Err(e) = tokio::fs::create_dir_all(parent).await
    {
        let _ = tx.send(EngineEvent::Finished {
            id,
            success: false,
            error: format!("Cannot create archive directory: {e}"),
        });
        return;
    }
    let args = match build_args(&job.options, &settings, &job.url) {
        Ok(a) => a,
        Err(e) => {
            let _ = tx.send(EngineEvent::Finished {
                id,
                success: false,
                error: e,
            });
            return;
        }
    };
    let mut cmd = Command::new(&settings.yt_dlp_path);
    cmd.args(&args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.as_std_mut().process_group(0);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(EngineEvent::Finished {
                id,
                success: false,
                error: format!("Could not launch {}: {e}", settings.yt_dlp_path),
            });
            return;
        }
    };
    let _ = tx.send(EngineEvent::Started(id));
    let stdout_task = child
        .stdout
        .take()
        .map(|stdout| tokio::spawn(read_output(id, BufReader::new(stdout), tx.clone())));
    let stderr_task = child
        .stderr
        .take()
        .map(|stderr| tokio::spawn(read_output(id, BufReader::new(stderr), tx.clone())));
    let pid = child.id();
    let mut cancelled = false;
    let status = loop {
        tokio::select! {
            result = child.wait() => break result,
            control = controls.recv() => match control {
                Some(Control::Cancel) => {
                    cancelled = true;
                    #[cfg(unix)]
                    if let Some(pid) = pid {
                        // yt-dlp may have ffmpeg/aria2 children. Killing the
                        // dedicated process group prevents orphan workers.
                        unsafe { libc::kill(-(pid as i32), libc::SIGKILL); }
                    }
                    #[cfg(not(unix))]
                    let _ = child.kill().await;
                }
                Some(Control::Pause) => {
                    #[cfg(unix)] if let Some(pid) = pid { unsafe { libc::kill(-(pid as i32), libc::SIGSTOP); } }
                    let _ = tx.send(EngineEvent::Paused(id));
                }
                Some(Control::Resume) => {
                    #[cfg(unix)] if let Some(pid) = pid { unsafe { libc::kill(-(pid as i32), libc::SIGCONT); } }
                    let _ = tx.send(EngineEvent::Resumed(id));
                }
                None => {}
            }
        }
    };
    let (success, error) = match status {
        Ok(s) if s.success() => (true, String::new()),
        Ok(_s) if cancelled => (
            false,
            "Cancelled by user; partial data was kept for resuming".into(),
        ),
        Ok(s) => (false, format!("yt-dlp exited with {s}")),
        Err(e) => (false, format!("Could not wait for yt-dlp: {e}")),
    };
    // Drain both streams before Finished so a late progress line cannot move a
    // completed job back into Downloading state.
    if let Some(task) = stdout_task {
        let _ = task.await;
    }
    if let Some(task) = stderr_task {
        let _ = task.await;
    }
    let _ = tx.send(EngineEvent::Finished { id, success, error });
}

async fn read_output<R: tokio::io::AsyncRead + Unpin>(
    id: Uuid,
    reader: BufReader<R>,
    tx: mpsc::UnboundedSender<EngineEvent>,
) {
    let mut lines = reader.lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Some(rest) = line.strip_prefix(PROGRESS_PREFIX) {
            let f: Vec<&str> = rest.splitn(7, '\t').collect();
            if f.len() >= 6 {
                let downloaded = numeric(f[1]) as u64;
                let total = numeric(f[2]) as u64;
                let speed = numeric(f[3]);
                let eta = f[4].trim().parse().ok();
                let title = f.get(5).unwrap_or(&"").to_string();
                let _ = tx.send(EngineEvent::Progress {
                    id,
                    downloaded,
                    total,
                    speed,
                    eta,
                    title,
                });
            }
        } else if let Some(path) = line.strip_prefix(DONE_PREFIX) {
            let _ = tx.send(EngineEvent::Output {
                id,
                path: path.into(),
            });
        } else {
            if line.contains("Post-process")
                || line.contains("Merger")
                || line.contains("ExtractAudio")
            {
                let _ = tx.send(EngineEvent::Processing(id));
            }
            let _ = tx.send(EngineEvent::Log { id, line });
        }
    }
}

fn numeric(s: &str) -> f64 {
    s.trim().parse().unwrap_or(0.0)
}

pub fn executable_exists(name: &str) -> bool {
    let p = Path::new(name);
    if p.components().count() > 1 {
        return p.is_file();
    }
    std::env::var_os("PATH")
        .is_some_and(|paths| std::env::split_paths(&paths).any(|d| d.join(name).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_are_individual_and_url_is_last() {
        let options = DownloadOptions {
            raw_args: "--age-limit 18 --format-sort 'res:720'".into(),
            ..Default::default()
        };
        let settings = Settings::default();
        let args = build_args(&options, &settings, "https://example.com/watch?v=a&x=b").unwrap();
        assert_eq!(args[args.len() - 2], "--");
        assert_eq!(args.last().unwrap(), "https://example.com/watch?v=a&x=b");
        assert!(args.windows(2).any(|w| w == ["--age-limit", "18"]));
    }

    #[test]
    fn audio_preset_extracts_audio() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            quality: Quality::Custom("251".into()),
            audio_format: "opus".into(),
            ..Default::default()
        };
        let args = build_args(&options, &Settings::default(), "https://x.test").unwrap();
        assert!(args.contains(&"--extract-audio".into()));
        assert!(args.windows(2).any(|w| w == ["--audio-format", "opus"]));
        assert!(args.windows(2).any(|w| w == ["--format", "251"]));
        assert!(args.contains(&"--embed-thumbnail".into()));
        assert!(args.contains(&"--embed-metadata".into()));
        assert!(
            args.windows(2)
                .any(|w| w == ["--convert-thumbnails", "jpg"])
        );
    }

    #[test]
    fn mpv_format_is_passed_as_a_single_option() {
        let args = player_args(
            "https://example.test/video",
            "251",
            PlayerMode::Audio,
            "--input-ipc-server=/tmp/test.sock",
        );
        assert!(args.contains(&"--ytdl-format=251".into()));
        assert!(!args.contains(&"--ytdl-format".into()));
        assert!(args.contains(&"--no-video".into()));
        assert!(args.contains(&"--input-ipc-server=/tmp/test.sock".into()));
        assert_eq!(args[args.len() - 2], "--");
        assert_eq!(args.last().unwrap(), "https://example.test/video");
    }

    #[test]
    fn video_player_keeps_video_and_uses_ipc() {
        let args = player_args(
            "https://example.test/video",
            "137+251/137",
            PlayerMode::Video,
            "--input-ipc-server=/tmp/test.sock",
        );
        assert!(!args.contains(&"--no-video".into()));
        assert!(args.contains(&"--force-window=yes".into()));
        assert!(args.contains(&"--ytdl-format=137+251/137".into()));
    }

    #[test]
    fn player_property_events_update_status() {
        let mut status = PlayerStatus::default();
        assert!(update_player_status(
            r#"{"event":"property-change","name":"time-pos","data":42.5}"#,
            &mut status
        ));
        assert!(update_player_status(
            r#"{"event":"property-change","name":"duration","data":180.0}"#,
            &mut status
        ));
        assert!(update_player_status(
            r#"{"event":"property-change","name":"pause","data":true}"#,
            &mut status
        ));
        assert_eq!(status.position, 42.5);
        assert_eq!(status.duration, 180.0);
        assert!(status.paused);
    }

    #[test]
    fn flac_audio_keeps_cover_art_flags() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            quality: Quality::Custom("251".into()),
            audio_format: "flac".into(),
            ..Default::default()
        };
        let args = build_args(&options, &Settings::default(), "https://x.test").unwrap();
        assert!(args.windows(2).any(|w| w == ["--audio-format", "flac"]));
        assert!(args.contains(&"--embed-thumbnail".into()));
        assert!(
            args.windows(2)
                .any(|w| w == ["--convert-thumbnails", "jpg"])
        );
    }

    #[test]
    fn best_audio_is_not_inflated_to_flac() {
        let options = DownloadOptions {
            mode: MediaMode::Audio,
            audio_format: "best".into(),
            ..Default::default()
        };
        let args = build_args(&options, &Settings::default(), "https://x.test").unwrap();
        assert!(args.windows(2).any(|w| w == ["--audio-format", "best"]));
        assert!(!args.windows(2).any(|w| w == ["--audio-format", "flac"]));
        assert!(args.contains(&"--embed-thumbnail".into()));
    }

    #[test]
    fn screen_quality_caps_both_dimensions_and_keeps_best_audio() {
        let options = DownloadOptions {
            quality: Quality::Screen {
                width: 1920,
                height: 1080,
            },
            ..DownloadOptions::default()
        };
        let args = build_args(&options, &Settings::default(), "https://x.test").unwrap();
        let selector = args
            .windows(2)
            .find(|pair| pair[0] == "--format")
            .map(|pair| pair[1].as_str())
            .unwrap();
        assert_eq!(
            selector,
            "bv*[width<=1920][height<=1080]+ba/b[width<=1920][height<=1080]"
        );
    }

    #[test]
    fn invalid_paths_fail_before_process_launch() {
        let settings = Settings {
            output_template: String::new(),
            ..Settings::default()
        };
        assert!(
            build_args(&DownloadOptions::default(), &settings, "https://x.test")
                .unwrap_err()
                .contains("template")
        );

        let settings = Settings {
            cookies_file: "/definitely/not/a/real/cookies.txt".into(),
            ..Settings::default()
        };
        assert!(
            build_args(&DownloadOptions::default(), &settings, "https://x.test")
                .unwrap_err()
                .contains("Cookies file")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn worker_streams_progress_and_completion() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::Duration;

        let temp = tempfile::tempdir().unwrap();
        let fake = temp.path().join("fake-yt-dlp");
        std::fs::write(
            &fake,
            "#!/bin/sh\nprintf '__YOUTUIBE_PROGRESS__\\tdownloading\\t50\\t100\\t25\\t2\\tFixture\\n'\nprintf '__YOUTUIBE_DONE__\\t/tmp/fixture.mp4\\n'\n",
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let settings = Settings {
            yt_dlp_path: fake.to_string_lossy().into_owned(),
            output_dir: temp.path().join("out"),
            archive: false,
            ..Settings::default()
        };
        let job = Job::new(
            "https://example.test/video".into(),
            "test".into(),
            DownloadOptions::default(),
        );
        let id = job.id;
        let (tx, mut rx) = mpsc::unbounded_channel();
        let _control = start(job, settings, tx);
        let mut progress = false;
        let mut output = false;
        let mut finished = false;
        tokio::time::timeout(Duration::from_secs(3), async {
            while let Some(event) = rx.recv().await {
                match event {
                    EngineEvent::Progress {
                        id: got,
                        downloaded: 50,
                        total: 100,
                        ..
                    } if got == id => progress = true,
                    EngineEvent::Output { id: got, path }
                        if got == id && path == "/tmp/fixture.mp4" =>
                    {
                        output = true
                    }
                    EngineEvent::Finished {
                        id: got,
                        success: true,
                        ..
                    } if got == id => {
                        finished = true;
                        break;
                    }
                    _ => {}
                }
            }
        })
        .await
        .unwrap();
        assert!(progress && output && finished);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn search_maps_flat_playlist_results() {
        use std::os::unix::fs::PermissionsExt;
        use std::time::Duration;

        let temp = tempfile::tempdir().unwrap();
        let fake = temp.path().join("fake-search-yt-dlp");
        std::fs::write(
            &fake,
            r#"#!/bin/sh
printf '%s\n' '{"entries":[{"id":"abc123","title":"A result","channel":"Channel","duration":91.0,"url":"abc123","view_count":1200,"live_status":null,"thumbnails":[{"url":"https://img.test/s.jpg","width":120,"height":90},{"url":"https://img.test/l.jpg","width":480,"height":360}]}]}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();

        let settings = Settings {
            yt_dlp_path: fake.to_string_lossy().into_owned(),
            ..Settings::default()
        };
        let (tx, mut rx) = mpsc::unbounded_channel();
        search("rust tui".into(), 20, settings, 7, tx).await;
        let event = tokio::time::timeout(Duration::from_secs(1), rx.recv())
            .await
            .unwrap()
            .unwrap();
        let EngineEvent::SearchFinished {
            token: 7,
            result: Ok(results),
        } = event
        else {
            panic!("unexpected search event")
        };
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].url, "https://www.youtube.com/watch?v=abc123");
        assert_eq!(results[0].uploader, "Channel");
        assert_eq!(results[0].thumbnail_url, "https://img.test/l.jpg");
    }
}
