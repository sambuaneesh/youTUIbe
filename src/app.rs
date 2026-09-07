use crate::{
    display::{self, DisplayInfo},
    model::{
        DownloadOptions, DownloadStatus, Job, MediaMode, PersistedState, Preset, Quality, Settings,
        presets,
    },
    storage,
    ytdlp::{self, Control, EngineEvent, PlayerControl, SearchResult, VideoInfo},
};
use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::{
    collections::{HashMap, VecDeque},
    path::PathBuf,
};
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Download,
    Search,
    Queue,
    History,
    Settings,
    Logs,
    Help,
}

impl Tab {
    pub const ALL: [Tab; 7] = [
        Tab::Download,
        Tab::Search,
        Tab::Queue,
        Tab::History,
        Tab::Settings,
        Tab::Logs,
        Tab::Help,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

#[derive(Debug, Clone)]
pub enum EditTarget {
    Url,
    SearchQuery,
    RawArgs,
    SubtitleLanguages,
    PlaylistItems,
    AudioQuality,
    SponsorCategories,
    DateAfter,
    DateBefore,
    MatchFilter,
    Setting(usize),
}

#[derive(Debug, Clone)]
pub enum Modal {
    Inspect,
    Explorer {
        query: String,
        scroll: usize,
    },
    Edit {
        title: String,
        value: String,
        cursor: usize,
        target: EditTarget,
        secret: bool,
    },
    Command(String),
    ConfirmCancel(Uuid),
    Error(String),
}

#[derive(Debug, Clone)]
pub enum ClickAction {
    Tab(Tab),
    SearchInput,
    RunSearch,
    SearchResult(usize),
    UseSearchResult,
    QueueSearchResult,
    ViewSearchThumbnail,
    SearchVideoQuality(usize),
    SearchAudioQuality(usize),
    ToggleSearchSubtitles,
    TogglePlayback,
    StopPlayback,
    Preset(usize),
    Builder(usize),
    Queue(usize),
    History(usize),
    Setting(usize),
    Add,
    AddStart,
    Inspect,
    Url,
    PauseResume,
    Retry,
    Cancel,
    CloseModal,
}

#[derive(Debug, Clone)]
pub struct SearchFormatChoice {
    pub format_id: String,
    pub label: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Loading,
    Playing,
    Paused,
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub rect: Rect,
    pub action: ClickAction,
}

pub struct App {
    pub tab: Tab,
    pub url: String,
    pub preset_index: usize,
    pub presets: Vec<Preset>,
    pub display: Option<DisplayInfo>,
    pub options: DownloadOptions,
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub search_index: usize,
    pub searching: bool,
    pub search_error: String,
    pub thumbnail: Option<StatefulProtocol>,
    pub thumbnail_status: String,
    pub thumbnail_renderer: String,
    pub thumbnail_bytes: Option<Vec<u8>>,
    pub viu_request: Option<Vec<u8>>,
    pub search_details: Option<VideoInfo>,
    pub search_details_loading: bool,
    pub search_details_error: String,
    pub search_video_choices: Vec<SearchFormatChoice>,
    pub search_audio_choices: Vec<SearchFormatChoice>,
    pub search_video_index: usize,
    pub search_audio_index: usize,
    pub search_subtitles: bool,
    pub playback_state: PlaybackState,
    pub playback_title: String,
    pub builder_index: usize,
    pub queue_index: usize,
    pub history_index: usize,
    pub settings_index: usize,
    pub jobs: Vec<Job>,
    pub settings: Settings,
    pub logs: VecDeque<String>,
    pub log_scroll: usize,
    pub notice: String,
    pub modal: Option<Modal>,
    pub metadata: Option<VideoInfo>,
    pub format_index: usize,
    pub inspecting: bool,
    pub help_text: String,
    pub dependencies: Vec<(String, bool)>,
    pub hits: Vec<Hit>,
    pub should_quit: bool,
    pub dirty: bool,
    pub tick: u64,
    pub state_path: PathBuf,
    pub event_tx: mpsc::UnboundedSender<EngineEvent>,
    pub event_rx: mpsc::UnboundedReceiver<EngineEvent>,
    controllers: HashMap<Uuid, mpsc::UnboundedSender<Control>>,
    inspect_token: u64,
    search_token: u64,
    thumbnail_token: u64,
    image_picker: Option<Picker>,
    inspect_abort: Option<tokio::task::AbortHandle>,
    search_abort: Option<tokio::task::AbortHandle>,
    thumbnail_abort: Option<tokio::task::AbortHandle>,
    details_token: u64,
    details_abort: Option<tokio::task::AbortHandle>,
    player_token: u64,
    player_controller: Option<mpsc::UnboundedSender<PlayerControl>>,
}

impl App {
    pub fn new(state_path: PathBuf, state: PersistedState, help_text: String) -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let display = display::detect();
        let mut ps = presets();
        if let Some(info) = &display {
            ps[0].options.quality = Quality::Screen {
                width: info.width,
                height: info.height,
            };
        } else {
            ps[0].options.quality = Quality::Best;
        }
        let settings = state.settings;
        let dependencies = dependency_status(&settings);
        let options = ps[0].options.clone();
        let notice = display.as_ref().map_or_else(
            || {
                "Display detection unavailable; screen preset falls back to uncapped best video"
                    .into()
            },
            |d| {
                format!(
                    "Detected {}×{} on {}; the screen preset is ready",
                    d.width, d.height, d.name
                )
            },
        );
        Self {
            tab: Tab::Download,
            url: String::new(),
            preset_index: 0,
            presets: ps,
            display,
            options,
            search_query: String::new(),
            search_results: vec![],
            search_index: 0,
            searching: false,
            search_error: String::new(),
            thumbnail: None,
            thumbnail_status: "No thumbnail loaded".into(),
            thumbnail_renderer: "initializing".into(),
            thumbnail_bytes: None,
            viu_request: None,
            search_details: None,
            search_details_loading: false,
            search_details_error: String::new(),
            search_video_choices: none_choice(),
            search_audio_choices: none_choice(),
            search_video_index: 0,
            search_audio_index: 0,
            search_subtitles: false,
            playback_state: PlaybackState::Stopped,
            playback_title: String::new(),
            builder_index: 0,
            queue_index: 0,
            history_index: 0,
            settings_index: 0,
            jobs: state.jobs,
            settings,
            logs: VecDeque::with_capacity(2000),
            log_scroll: 0,
            notice,
            modal: None,
            metadata: None,
            format_index: 0,
            inspecting: false,
            help_text,
            dependencies,
            hits: vec![],
            should_quit: false,
            dirty: false,
            tick: 0,
            state_path,
            event_tx,
            event_rx,
            controllers: HashMap::new(),
            inspect_token: 0,
            search_token: 0,
            thumbnail_token: 0,
            image_picker: None,
            inspect_abort: None,
            search_abort: None,
            thumbnail_abort: None,
            details_token: 0,
            details_abort: None,
            player_token: 0,
            player_controller: None,
        }
    }

    pub fn set_image_picker(&mut self, picker: Picker) {
        self.thumbnail_renderer = format!("{:?}", picker.protocol_type());
        self.image_picker = Some(picker);
    }

    pub fn persisted(&self) -> PersistedState {
        PersistedState {
            settings: self.settings.clone(),
            jobs: self.jobs.clone(),
        }
    }

    pub fn save(&mut self) {
        match storage::save_atomic(&self.state_path, &self.persisted()) {
            Ok(()) => self.dirty = false,
            Err(e) => self.notice = format!("Could not save state: {e:#}"),
        }
    }

    pub fn on_tick(&mut self) {
        self.tick = self.tick.wrapping_add(1);
        while let Ok(event) = self.event_rx.try_recv() {
            self.apply_engine_event(event);
        }
        self.start_available();
        if self.dirty && self.tick.is_multiple_of(10) {
            self.save();
        }
    }

    fn apply_engine_event(&mut self, event: EngineEvent) {
        match event {
            EngineEvent::Started(id) => {
                if let Some(j) = self.job_mut(id) {
                    j.status = DownloadStatus::Downloading;
                    j.started_at = Some(Utc::now());
                }
            }
            EngineEvent::Progress {
                id,
                downloaded,
                total,
                speed,
                eta,
                title,
            } => {
                if let Some(j) = self.job_mut(id) {
                    j.status = DownloadStatus::Downloading;
                    j.progress.downloaded = downloaded;
                    j.progress.total = total;
                    j.progress.speed = speed;
                    j.progress.eta = eta;
                    j.progress.percent = if total > 0 {
                        downloaded as f64 * 100.0 / total as f64
                    } else {
                        0.0
                    };
                    if !title.is_empty() {
                        j.title = title;
                    }
                }
            }
            EngineEvent::Processing(id) => {
                if let Some(j) = self.job_mut(id) {
                    j.status = DownloadStatus::Processing;
                }
            }
            EngineEvent::Output { id, path } => {
                if let Some(j) = self.job_mut(id) {
                    j.output_path = path;
                }
            }
            EngineEvent::Log { id, line } => {
                let short = id.to_string();
                self.logs.push_back(format!("[{}] {line}", &short[..8]));
                while self.logs.len() > 2000 {
                    self.logs.pop_front();
                }
            }
            EngineEvent::Paused(id) => {
                if let Some(j) = self.job_mut(id) {
                    j.status = DownloadStatus::Paused;
                }
            }
            EngineEvent::Resumed(id) => {
                if let Some(j) = self.job_mut(id) {
                    j.status = DownloadStatus::Downloading;
                }
            }
            EngineEvent::Finished { id, success, error } => {
                self.controllers.remove(&id);
                let mut finished_title = "Download".to_string();
                if let Some(j) = self.job_mut(id) {
                    finished_title = j.title.clone();
                    j.status = if success {
                        DownloadStatus::Completed
                    } else if error.starts_with("Cancelled") {
                        DownloadStatus::Cancelled
                    } else {
                        DownloadStatus::Failed
                    };
                    j.error = error;
                    j.finished_at = Some(Utc::now());
                    if success {
                        j.progress.percent = 100.0;
                    }
                }
                self.notice = if success {
                    "Download completed".into()
                } else {
                    "A download stopped; open Logs for details".into()
                };
                if self.settings.notifications && ytdlp::executable_exists("notify-send") {
                    let message = if success {
                        "Download completed"
                    } else {
                        "Download stopped"
                    };
                    tokio::spawn(async move {
                        let _ = tokio::process::Command::new("notify-send")
                            .args(["Tide", &format!("{message}: {finished_title}")])
                            .status()
                            .await;
                    });
                }
                self.dirty = true;
            }
            EngineEvent::Inspected { token, result } if token == self.inspect_token => {
                self.inspecting = false;
                self.inspect_abort = None;
                match result {
                    Ok(info) => {
                        self.metadata = Some(info);
                        self.format_index = 0;
                        self.notice = "Inspection complete; choose an exact format with f or keep the smart selector".into();
                        self.modal = Some(Modal::Inspect);
                    }
                    Err(e) => self.modal = Some(Modal::Error(e)),
                }
            }
            EngineEvent::Inspected { .. } => {}
            EngineEvent::SearchFinished { token, result } if token == self.search_token => {
                self.searching = false;
                self.search_abort = None;
                match result {
                    Ok(results) => {
                        self.search_results = results;
                        self.search_index = 0;
                        self.search_error.clear();
                        self.notice = if self.search_results.is_empty() {
                            "Search completed with no results; downloads remain available".into()
                        } else {
                            format!("Found {} videos", self.search_results.len())
                        };
                        self.load_selected_thumbnail();
                        self.load_selected_details();
                    }
                    Err(error) => {
                        self.search_error = compact_error(&error);
                        self.notice =
                            "Search is unavailable; direct URL downloads still work normally"
                                .into();
                    }
                }
            }
            EngineEvent::SearchFinished { .. } => {}
            EngineEvent::SearchInspected { token, result } if token == self.details_token => {
                self.search_details_loading = false;
                self.details_abort = None;
                match result {
                    Ok(info) => {
                        let (video, audio) = format_choices(&info);
                        self.search_video_choices = video;
                        self.search_audio_choices = audio;
                        self.search_video_index = self.search_video_choices.len().saturating_sub(1);
                        self.search_audio_index = self.search_audio_choices.len().saturating_sub(1);
                        self.search_subtitles = false;
                        self.search_details_error.clear();
                        self.search_details = Some(info);
                        self.notice = format!(
                            "Ready: {} video levels · {} audio levels",
                            self.search_video_choices.len().saturating_sub(1),
                            self.search_audio_choices.len().saturating_sub(1)
                        );
                    }
                    Err(error) => {
                        self.search_details = None;
                        self.search_video_choices = none_choice();
                        self.search_audio_choices = none_choice();
                        self.search_video_index = 0;
                        self.search_audio_index = 0;
                        self.search_details_error = compact_error(&error);
                        self.notice =
                            "Format details unavailable; normal presets and queue still work"
                                .into();
                    }
                }
            }
            EngineEvent::SearchInspected { .. } => {}
            EngineEvent::ThumbnailReady { token, result } if token == self.thumbnail_token => {
                self.thumbnail_abort = None;
                match result {
                    Ok(data) => {
                        self.thumbnail_bytes = Some(data.bytes);
                        if let Some(picker) = &self.image_picker {
                            self.thumbnail = Some(picker.new_resize_protocol(data.image));
                            self.thumbnail_status =
                                format!("Thumbnail via {}", self.thumbnail_renderer);
                        } else {
                            self.thumbnail = None;
                            self.thumbnail_status =
                                "Image renderer unavailable; video details are still usable".into();
                        }
                    }
                    Err(error) => {
                        self.thumbnail = None;
                        self.thumbnail_status = format!(
                            "Thumbnail unavailable: {} (search and downloads still work)",
                            compact_error(&error)
                        );
                    }
                }
            }
            EngineEvent::ThumbnailReady { .. } => {}
            EngineEvent::PlayerStarted(token) if token == self.player_token => {
                self.playback_state = PlaybackState::Playing;
                self.notice = format!("Now playing audio: {}", self.playback_title);
            }
            EngineEvent::PlayerStarted(_) => {}
            EngineEvent::PlayerFinished { token, error } if token == self.player_token => {
                self.player_controller = None;
                self.playback_state = PlaybackState::Stopped;
                self.notice = if error.is_empty() {
                    "Audio preview stopped".into()
                } else {
                    format!("Audio preview unavailable: {}", compact_error(&error))
                };
            }
            EngineEvent::PlayerFinished { .. } => {}
        }
        self.dirty = true;
    }

    fn job_mut(&mut self, id: Uuid) -> Option<&mut Job> {
        self.jobs.iter_mut().find(|j| j.id == id)
    }

    fn start_available(&mut self) {
        let active = self.controllers.len();
        let slots = self.settings.max_parallel_downloads.saturating_sub(active);
        if slots == 0 {
            return;
        }
        let ids: Vec<Uuid> = self
            .jobs
            .iter()
            .filter(|j| j.status == DownloadStatus::Queued)
            .take(slots)
            .map(|j| j.id)
            .collect();
        for id in ids {
            if let Some(job) = self.jobs.iter().find(|j| j.id == id).cloned() {
                let control = ytdlp::start(job, self.settings.clone(), self.event_tx.clone());
                self.controllers.insert(id, control);
                if let Some(j) = self.job_mut(id) {
                    j.status = DownloadStatus::Inspecting;
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        if let Some(modal) = self.modal.take() {
            self.handle_modal_key(key, modal);
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match key.code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.tab = Tab::Help,
            KeyCode::Char('1'..='7') => {
                if let KeyCode::Char(c) = key.code {
                    self.tab = Tab::ALL[(c as usize - '1' as usize).min(Tab::ALL.len() - 1)];
                }
            }
            KeyCode::Tab | KeyCode::Right if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.tab = Tab::ALL[(self.tab.index() + 1) % Tab::ALL.len()]
            }
            KeyCode::BackTab | KeyCode::Left if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.tab = Tab::ALL[(self.tab.index() + Tab::ALL.len() - 1) % Tab::ALL.len()]
            }
            _ => match self.tab {
                Tab::Download => self.download_key(key),
                Tab::Search => self.search_key(key),
                Tab::Queue => self.queue_key(key),
                Tab::History => self.history_key(key),
                Tab::Settings => self.settings_key(key),
                Tab::Logs => self.logs_key(key),
                Tab::Help => self.help_key(key),
            },
        }
    }

    pub fn handle_paste(&mut self, text: String) {
        match self.modal.take() {
            Some(Modal::Edit {
                title,
                mut value,
                mut cursor,
                target,
                secret,
            }) => {
                let pasted = text.replace(['\r', '\n'], " ");
                insert_at_char(&mut value, cursor, &pasted);
                cursor += pasted.chars().count();
                self.modal = Some(Modal::Edit {
                    title,
                    value,
                    cursor,
                    target,
                    secret,
                });
            }
            Some(modal) => self.modal = Some(modal),
            None if self.tab == Tab::Download => {
                if !self.url.is_empty() {
                    self.url.push(' ');
                }
                self.url.push_str(&text.replace(['\r', '\n'], " "));
            }
            None if self.tab == Tab::Search => {
                if !self.search_query.is_empty() {
                    self.search_query.push(' ');
                }
                self.search_query.push_str(&text.replace(['\r', '\n'], " "));
            }
            None => {}
        }
    }

    fn handle_modal_key(&mut self, key: KeyEvent, mut modal: Modal) {
        match &mut modal {
            Modal::Edit {
                value,
                cursor,
                target,
                ..
            } => match key.code {
                KeyCode::Esc => return,
                KeyCode::Enter => {
                    let target = target.clone();
                    let value = value.clone();
                    self.commit_edit(target, value);
                    return;
                }
                KeyCode::Backspace if *cursor > 0 => {
                    remove_char(value, *cursor - 1);
                    *cursor -= 1;
                }
                KeyCode::Delete if *cursor < value.chars().count() => remove_char(value, *cursor),
                KeyCode::Left => *cursor = cursor.saturating_sub(1),
                KeyCode::Right => *cursor = (*cursor + 1).min(value.chars().count()),
                KeyCode::Home => *cursor = 0,
                KeyCode::End => *cursor = value.chars().count(),
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    value.clear();
                    *cursor = 0;
                }
                KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    delete_previous_word(value, cursor)
                }
                KeyCode::Char(c) => {
                    insert_at_char(value, *cursor, &c.to_string());
                    *cursor += 1;
                }
                _ => {}
            },
            Modal::Explorer { query, scroll } => match key.code {
                KeyCode::Esc | KeyCode::Enter => return,
                KeyCode::Backspace => {
                    query.pop();
                    *scroll = 0;
                }
                KeyCode::Char(c) => {
                    query.push(c);
                    *scroll = 0;
                }
                KeyCode::Down | KeyCode::PageDown => {
                    *scroll =
                        scroll.saturating_add(if key.code == KeyCode::PageDown { 15 } else { 1 })
                }
                KeyCode::Up => *scroll = scroll.saturating_sub(1),
                KeyCode::PageUp => *scroll = scroll.saturating_sub(15),
                _ => {}
            },
            Modal::ConfirmCancel(id) => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.cancel_id(*id);
                    return;
                }
                KeyCode::Char('n') | KeyCode::Esc => return,
                _ => {}
            },
            Modal::Inspect => match key.code {
                KeyCode::Esc | KeyCode::Enter => return,
                KeyCode::Down | KeyCode::Char('j') => {
                    let len = self.metadata.as_ref().map_or(0, |m| m.formats.len());
                    if len > 0 {
                        self.format_index = (self.format_index + 1).min(len - 1);
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.format_index = self.format_index.saturating_sub(1)
                }
                KeyCode::Char('f') => {
                    if let Some(format) = self
                        .metadata
                        .as_ref()
                        .and_then(|m| m.formats.iter().rev().nth(self.format_index))
                    {
                        self.options.quality = Quality::Custom(format.format_id.clone());
                        self.notice =
                            format!("Using exact format {} for the next job", format.format_id);
                    }
                    return;
                }
                _ => {}
            },
            _ => {
                if matches!(key.code, KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q')) {
                    return;
                }
            }
        }
        self.modal = Some(modal);
    }

    fn download_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('/') => self.edit("URL", self.url.clone(), EditTarget::Url, false),
            KeyCode::Char('a') => self.add_urls(false),
            KeyCode::Char('d') => self.add_urls(true),
            KeyCode::Char('i') => self.inspect(),
            KeyCode::Char('c') => self.preview_current(),
            KeyCode::Char('o') => {
                self.modal = Some(Modal::Explorer {
                    query: String::new(),
                    scroll: 0,
                })
            }
            KeyCode::Char('e') => self.edit_builder_value(),
            KeyCode::Up => self.builder_index = self.builder_index.saturating_sub(1),
            KeyCode::Down => {
                self.builder_index = (self.builder_index + 1).min(builder_rows().len() - 1)
            }
            KeyCode::Left => self.adjust_builder(false),
            KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => self.adjust_builder(true),
            KeyCode::Char('j') => {
                self.preset_index = (self.preset_index + 1).min(self.presets.len() - 1);
                self.apply_preset();
            }
            KeyCode::Char('k') => {
                self.preset_index = self.preset_index.saturating_sub(1);
                self.apply_preset();
            }
            _ => {}
        }
    }

    fn search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('/') | KeyCode::Char('e') => self.edit(
                "YouTube search",
                self.search_query.clone(),
                EditTarget::SearchQuery,
                false,
            ),
            KeyCode::Char('s') | KeyCode::Char('r') => self.run_search(),
            KeyCode::Enter if self.search_results.is_empty() => self.run_search(),
            KeyCode::Enter => self.use_search_selected(),
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.search_results.is_empty() {
                    self.select_search((self.search_index + 1).min(self.search_results.len() - 1));
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.select_search(self.search_index.saturating_sub(1));
            }
            KeyCode::Char('a') => self.queue_search_selected(false),
            KeyCode::Char('d') => self.queue_search_selected(true),
            KeyCode::Char('i') => self.inspect_search_selected(),
            KeyCode::Char('v') => self.view_search_thumbnail(),
            KeyCode::Char('[') => self.adjust_search_video(false),
            KeyCode::Char(']') => self.adjust_search_video(true),
            KeyCode::Char('{') => self.adjust_search_audio(false),
            KeyCode::Char('}') => self.adjust_search_audio(true),
            KeyCode::Char('t') => self.toggle_search_subtitles(),
            KeyCode::Char('p') | KeyCode::Char(' ') => self.toggle_audio_preview(),
            KeyCode::Char('x') => self.stop_audio_preview(),
            _ => {}
        }
    }

    fn queue_key(&mut self, key: KeyEvent) {
        let len = self.queue_jobs().len();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if len > 0 {
                    self.queue_index = (self.queue_index + 1).min(len - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.queue_index = self.queue_index.saturating_sub(1)
            }
            KeyCode::Char('p') | KeyCode::Char(' ') => self.pause_resume_selected(),
            KeyCode::Char('x') | KeyCode::Delete => self.confirm_cancel_selected(),
            KeyCode::Char('r') => self.retry_selected(false),
            KeyCode::Char('R') => self.retry_selected(true),
            KeyCode::Enter | KeyCode::Char('c') => self.preview_selected_queue(),
            _ => {}
        }
    }

    fn history_key(&mut self, key: KeyEvent) {
        let len = self.history_jobs().len();
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if len > 0 {
                    self.history_index = (self.history_index + 1).min(len - 1);
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.history_index = self.history_index.saturating_sub(1)
            }
            KeyCode::Char('r') => self.retry_selected(false),
            KeyCode::Char('R') => self.retry_selected(true),
            KeyCode::Enter | KeyCode::Char('c') => self.preview_selected_history(),
            _ => {}
        }
    }

    fn settings_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                self.settings_index = (self.settings_index + 1).min(setting_rows().len() - 1)
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.settings_index = self.settings_index.saturating_sub(1)
            }
            KeyCode::Left => self.adjust_setting(false),
            KeyCode::Right | KeyCode::Enter | KeyCode::Char(' ') => self.adjust_setting(true),
            KeyCode::Char('e') => self.edit_setting(),
            KeyCode::Char('s') => {
                self.dirty = true;
                self.save();
                self.notice = "Settings saved".into();
            }
            _ => {}
        }
    }

    fn logs_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.log_scroll = self.log_scroll.saturating_add(1),
            KeyCode::Down | KeyCode::Char('j') => {
                self.log_scroll = self.log_scroll.saturating_sub(1)
            }
            KeyCode::PageUp => self.log_scroll = self.log_scroll.saturating_add(20),
            KeyCode::PageDown => self.log_scroll = self.log_scroll.saturating_sub(20),
            KeyCode::Char('g') => self.log_scroll = self.logs.len(),
            KeyCode::Char('G') => self.log_scroll = 0,
            KeyCode::Char('c') => self.logs.clear(),
            _ => {}
        }
    }

    fn help_key(&mut self, key: KeyEvent) {
        if key.code == KeyCode::Char('o') {
            self.modal = Some(Modal::Explorer {
                query: String::new(),
                scroll: 0,
            });
        }
    }

    fn edit(&mut self, title: &str, value: String, target: EditTarget, secret: bool) {
        let cursor = value.chars().count();
        self.modal = Some(Modal::Edit {
            title: title.into(),
            value,
            cursor,
            target,
            secret,
        });
    }

    fn commit_edit(&mut self, target: EditTarget, value: String) {
        match target {
            EditTarget::Url => self.url = value,
            EditTarget::SearchQuery => {
                self.search_query = value;
                self.run_search();
            }
            EditTarget::RawArgs => self.options.raw_args = value,
            EditTarget::SubtitleLanguages => self.options.subtitle_languages = value,
            EditTarget::PlaylistItems => self.options.playlist_items = value,
            EditTarget::AudioQuality => self.options.audio_quality = value,
            EditTarget::SponsorCategories => self.options.sponsor_categories = value,
            EditTarget::DateAfter => self.options.date_after = value,
            EditTarget::DateBefore => self.options.date_before = value,
            EditTarget::MatchFilter => self.options.match_filter = value,
            EditTarget::Setting(i) => self.set_setting_text(i, value),
        }
        self.dirty = true;
    }

    pub fn run_search(&mut self) {
        let query = self.search_query.trim().to_string();
        if query.is_empty() {
            self.search_error = "Enter something to search for".into();
            self.notice = "Search needs a query".into();
            return;
        }
        self.search_token = self.search_token.wrapping_add(1);
        if let Some(task) = self.search_abort.take() {
            task.abort();
        }
        self.searching = true;
        self.search_error.clear();
        self.notice = format!("Searching YouTube for “{query}”…");
        let task = tokio::spawn(ytdlp::search(
            query,
            20,
            self.settings.clone(),
            self.search_token,
            self.event_tx.clone(),
        ));
        self.search_abort = Some(task.abort_handle());
    }

    pub fn select_search(&mut self, index: usize) {
        if index < self.search_results.len() {
            self.search_index = index;
            self.stop_audio_preview();
            self.load_selected_thumbnail();
            self.load_selected_details();
        }
    }

    fn load_selected_thumbnail(&mut self) {
        self.thumbnail_token = self.thumbnail_token.wrapping_add(1);
        if let Some(task) = self.thumbnail_abort.take() {
            task.abort();
        }
        self.thumbnail = None;
        self.thumbnail_bytes = None;
        let Some(url) = self
            .search_results
            .get(self.search_index)
            .map(|result| result.thumbnail_url.clone())
            .filter(|url| !url.is_empty())
        else {
            self.thumbnail_status = "No thumbnail supplied; video details are still usable".into();
            return;
        };
        self.thumbnail_status = "Loading thumbnail…".into();
        let task = tokio::spawn(ytdlp::fetch_thumbnail(
            url,
            self.thumbnail_token,
            self.event_tx.clone(),
        ));
        self.thumbnail_abort = Some(task.abort_handle());
    }

    fn load_selected_details(&mut self) {
        self.details_token = self.details_token.wrapping_add(1);
        if let Some(task) = self.details_abort.take() {
            task.abort();
        }
        self.search_details = None;
        self.search_details_error.clear();
        self.search_details_loading = false;
        self.search_video_choices = none_choice();
        self.search_audio_choices = none_choice();
        self.search_video_index = 0;
        self.search_audio_index = 0;
        self.search_subtitles = false;
        let Some(url) = self
            .search_results
            .get(self.search_index)
            .map(|result| result.url.clone())
        else {
            return;
        };
        self.search_details_loading = true;
        let task = tokio::spawn(ytdlp::inspect_search(
            url,
            self.settings.clone(),
            self.details_token,
            self.event_tx.clone(),
        ));
        self.details_abort = Some(task.abort_handle());
    }

    pub fn adjust_search_video(&mut self, next: bool) {
        self.search_video_index = slider_step(
            self.search_video_index,
            self.search_video_choices.len(),
            next,
        );
        self.describe_search_mix();
    }

    pub fn set_search_video(&mut self, index: usize) {
        if index < self.search_video_choices.len() {
            self.search_video_index = index;
            self.describe_search_mix();
        }
    }

    pub fn adjust_search_audio(&mut self, next: bool) {
        self.search_audio_index = slider_step(
            self.search_audio_index,
            self.search_audio_choices.len(),
            next,
        );
        self.describe_search_mix();
    }

    pub fn set_search_audio(&mut self, index: usize) {
        if index < self.search_audio_choices.len() {
            self.search_audio_index = index;
            self.describe_search_mix();
        }
    }

    fn describe_search_mix(&mut self) {
        let video = &self.search_video_choices[self.search_video_index].label;
        let audio = &self.search_audio_choices[self.search_audio_index].label;
        self.notice = format!("Download mix: video {video} · audio {audio}");
    }

    pub fn toggle_search_subtitles(&mut self) {
        if !self.search_has_subtitles() {
            self.notice = "No subtitles were reported for this video".into();
            self.search_subtitles = false;
            return;
        }
        self.search_subtitles = !self.search_subtitles;
        self.notice = if self.search_subtitles {
            format!("Subtitles enabled: {}", self.search_subtitle_label())
        } else {
            "Subtitles disabled".into()
        };
    }

    pub fn search_has_subtitles(&self) -> bool {
        self.search_details.as_ref().is_some_and(|info| {
            info.subtitles.keys().any(|key| key != "live_chat")
                || info.automatic_captions.keys().any(|key| key != "live_chat")
        })
    }

    pub fn search_subtitle_label(&self) -> String {
        let Some(info) = &self.search_details else {
            return "unavailable".into();
        };
        let manual = info
            .subtitles
            .keys()
            .filter(|key| key.as_str() != "live_chat")
            .count();
        let automatic = info
            .automatic_captions
            .keys()
            .filter(|key| key.as_str() != "live_chat")
            .count();
        match (manual, automatic) {
            (0, 0) => "unavailable".into(),
            (manual, 0) => format!("{manual} manual languages"),
            (0, automatic) => format!("{automatic} auto languages"),
            (manual, automatic) => format!("{manual} manual + {automatic} auto languages"),
        }
    }

    pub fn use_search_selected(&mut self) {
        let Some(result) = self.search_results.get(self.search_index).cloned() else {
            return;
        };
        let options = match self.configured_search_options() {
            Ok(options) => options,
            Err(error) => {
                self.notice = error;
                return;
            }
        };
        self.options = options;
        self.url = result.url;
        self.metadata = None;
        self.tab = Tab::Download;
        self.notice = format!("Ready to configure: {}", result.title);
    }

    pub fn queue_search_selected(&mut self, go_queue: bool) {
        let Some(result) = self.search_results.get(self.search_index).cloned() else {
            return;
        };
        let options = match self.configured_search_options() {
            Ok(options) => options,
            Err(error) => {
                self.notice = error;
                return;
            }
        };
        let video = &self.search_video_choices[self.search_video_index].label;
        let audio = &self.search_audio_choices[self.search_audio_index].label;
        let preset = if self.search_details.is_some() {
            format!("Search · {video} + {audio}")
        } else {
            self.presets[self.preset_index].name.into()
        };
        let mut job = Job::new(result.url, preset, options);
        job.title = result.title;
        job.command_preview = ytdlp::command_preview(&job, &self.settings);
        self.jobs.push(job);
        self.notice = "Search result queued; it will start when a slot is free".into();
        self.dirty = true;
        self.save();
        if go_queue {
            self.tab = Tab::Queue;
        }
    }

    fn configured_search_options(&self) -> Result<DownloadOptions, String> {
        if self.search_details.is_none() {
            return Ok(self.options.clone());
        }
        let video = self.search_video_choices.get(self.search_video_index);
        let audio = self.search_audio_choices.get(self.search_audio_index);
        let video_id = video.map_or("", |choice| choice.format_id.as_str());
        let audio_id = audio.map_or("", |choice| choice.format_id.as_str());
        if video_id.is_empty() && audio_id.is_empty() {
            return Err("Choose at least video or audio before downloading".into());
        }
        let mut options = self.options.clone();
        match (video_id.is_empty(), audio_id.is_empty()) {
            (false, false) => {
                options.mode = MediaMode::Video;
                options.quality = Quality::Custom(format!("{video_id}+{audio_id}/{video_id}"));
            }
            (false, true) => {
                options.mode = MediaMode::VideoOnly;
                options.quality = Quality::Custom(video_id.into());
            }
            (true, false) => {
                options.mode = MediaMode::Audio;
                options.quality = Quality::Custom(audio_id.into());
                options.audio_format = "best".into();
                options.embed_subtitles = false;
            }
            (true, true) => unreachable!(),
        }
        if self.search_subtitles {
            let (manual, automatic, language) = self.preferred_search_subtitle();
            options.subtitles = manual;
            options.auto_subtitles = automatic;
            options.subtitle_languages = language;
        } else {
            options.subtitles = false;
            options.auto_subtitles = false;
        }
        Ok(options)
    }

    fn preferred_search_subtitle(&self) -> (bool, bool, String) {
        let Some(info) = &self.search_details else {
            return (false, false, String::new());
        };
        let preferred = |keys: Vec<&String>| {
            keys.iter()
                .find(|key| key.starts_with("en"))
                .or_else(|| keys.first())
                .map(|key| (*key).clone())
        };
        let manual: Vec<&String> = info
            .subtitles
            .keys()
            .filter(|key| key.as_str() != "live_chat")
            .collect();
        if let Some(language) = preferred(manual) {
            return (true, false, language);
        }
        let automatic: Vec<&String> = info
            .automatic_captions
            .keys()
            .filter(|key| key.as_str() != "live_chat")
            .collect();
        preferred(automatic).map_or((false, false, String::new()), |language| {
            (false, true, language)
        })
    }

    fn inspect_search_selected(&mut self) {
        let Some(result) = self.search_results.get(self.search_index) else {
            return;
        };
        self.url = result.url.clone();
        self.inspect();
    }

    pub fn view_search_thumbnail(&mut self) {
        if !ytdlp::executable_exists("viu") {
            self.notice = "Full-screen preview needs the optional viu executable".into();
        } else if let Some(bytes) = &self.thumbnail_bytes {
            self.viu_request = Some(bytes.clone());
            self.notice = "Opening full-screen thumbnail with viu…".into();
        } else {
            self.notice = "Wait for the selected thumbnail to finish loading".into();
        }
    }

    pub fn toggle_audio_preview(&mut self) {
        match self.playback_state {
            PlaybackState::Playing => {
                if let Some(controller) = &self.player_controller {
                    let _ = controller.send(PlayerControl::Pause);
                    self.playback_state = PlaybackState::Paused;
                    self.notice = "Audio preview paused".into();
                }
            }
            PlaybackState::Paused => {
                if let Some(controller) = &self.player_controller {
                    let _ = controller.send(PlayerControl::Resume);
                    self.playback_state = PlaybackState::Playing;
                    self.notice = format!("Now playing audio: {}", self.playback_title);
                }
            }
            PlaybackState::Loading => {
                self.notice = "Audio preview is still connecting…".into();
            }
            PlaybackState::Stopped => {
                if !ytdlp::executable_exists("mpv") {
                    self.notice = "Audio preview needs the optional mpv executable".into();
                    return;
                }
                let Some(result) = self.search_results.get(self.search_index) else {
                    return;
                };
                let format = self
                    .search_audio_choices
                    .get(self.search_audio_index)
                    .map(|choice| choice.format_id.clone())
                    .filter(|id| !id.is_empty())
                    .unwrap_or_else(|| "bestaudio/best".into());
                self.player_token = self.player_token.wrapping_add(1);
                self.playback_title = result.title.clone();
                self.playback_state = PlaybackState::Loading;
                self.player_controller = Some(ytdlp::start_player(
                    result.url.clone(),
                    format,
                    self.player_token,
                    self.event_tx.clone(),
                ));
                self.notice = format!("Connecting audio preview: {}", result.title);
            }
        }
    }

    pub fn stop_audio_preview(&mut self) {
        if let Some(controller) = self.player_controller.take() {
            let _ = controller.send(PlayerControl::Stop);
        }
        self.playback_state = PlaybackState::Stopped;
    }

    pub fn add_urls(&mut self, go_queue: bool) {
        let urls: Vec<String> = self
            .url
            .split_whitespace()
            .filter_map(normalize_url)
            .collect();
        if urls.is_empty() {
            self.modal = Some(Modal::Error("Enter at least one valid http(s) URL. Multiple whitespace-separated URLs are supported.".into()));
            return;
        }
        for url in urls {
            let mut job = Job::new(
                url,
                self.presets[self.preset_index].name.into(),
                self.options.clone(),
            );
            job.command_preview = ytdlp::command_preview(&job, &self.settings);
            self.jobs.push(job);
        }
        self.notice = "Added to queue; downloads start automatically when a slot is free".into();
        self.url.clear();
        self.metadata = None;
        self.dirty = true;
        self.save();
        if go_queue {
            self.tab = Tab::Queue;
        }
    }

    pub fn inspect(&mut self) {
        let Some(url) = self.url.split_whitespace().next().and_then(normalize_url) else {
            self.modal = Some(Modal::Error("Enter a valid URL before inspecting.".into()));
            return;
        };
        self.inspect_token = self.inspect_token.wrapping_add(1);
        if let Some(task) = self.inspect_abort.take() {
            task.abort();
        }
        self.inspecting = true;
        self.notice = "Inspecting media without downloading…".into();
        let task = tokio::spawn(ytdlp::inspect(
            url,
            self.settings.clone(),
            self.inspect_token,
            self.event_tx.clone(),
        ));
        self.inspect_abort = Some(task.abort_handle());
    }

    fn preview_current(&mut self) {
        let url = self
            .url
            .split_whitespace()
            .next()
            .and_then(normalize_url)
            .unwrap_or_else(|| "URL".into());
        let job = Job::new(
            url,
            self.presets[self.preset_index].name.into(),
            self.options.clone(),
        );
        self.modal = Some(Modal::Command(ytdlp::command_preview(&job, &self.settings)));
    }

    fn apply_preset(&mut self) {
        self.options = self.presets[self.preset_index].options.clone();
        self.notice = format!("Preset: {}", self.presets[self.preset_index].name);
    }

    pub fn queue_jobs(&self) -> Vec<&Job> {
        self.jobs
            .iter()
            .filter(|j| !j.status.is_terminal())
            .collect()
    }
    pub fn history_jobs(&self) -> Vec<&Job> {
        self.jobs
            .iter()
            .filter(|j| j.status.is_terminal())
            .rev()
            .collect()
    }
    fn selected_queue_id(&self) -> Option<Uuid> {
        self.queue_jobs().get(self.queue_index).map(|j| j.id)
    }
    fn selected_history_id(&self) -> Option<Uuid> {
        self.history_jobs().get(self.history_index).map(|j| j.id)
    }

    pub fn pause_resume_selected(&mut self) {
        let Some(id) = self.selected_queue_id() else {
            return;
        };
        let status = self
            .jobs
            .iter()
            .find(|j| j.id == id)
            .map(|j| j.status.clone());
        if let Some(tx) = self.controllers.get(&id) {
            let control = if status == Some(DownloadStatus::Paused) {
                Control::Resume
            } else {
                Control::Pause
            };
            let _ = tx.send(control);
        }
    }

    pub fn confirm_cancel_selected(&mut self) {
        if let Some(id) = self.selected_queue_id() {
            self.modal = Some(Modal::ConfirmCancel(id));
        }
    }

    fn cancel_id(&mut self, id: Uuid) {
        if let Some(tx) = self.controllers.get(&id) {
            let _ = tx.send(Control::Cancel);
        } else if let Some(j) = self.job_mut(id) {
            j.status = DownloadStatus::Cancelled;
            j.finished_at = Some(Utc::now());
        }
        self.dirty = true;
    }

    pub fn retry_selected(&mut self, force: bool) {
        let id = if self.tab == Tab::History {
            self.selected_history_id()
        } else {
            self.selected_queue_id()
        };
        let Some(id) = id else {
            return;
        };
        if let Some(old) = self.jobs.iter().find(|j| j.id == id).cloned() {
            let mut next = Job::new(old.url, old.preset, old.options);
            if force {
                next.options.raw_args.push_str(" --force-overwrites");
            }
            next.command_preview = ytdlp::command_preview(&next, &self.settings);
            self.jobs.push(next);
            self.dirty = true;
            self.notice = "Retry queued".into();
            self.tab = Tab::Queue;
        }
    }

    fn preview_selected_queue(&mut self) {
        if let Some(id) = self.selected_queue_id() {
            self.preview_id(id);
        }
    }
    fn preview_selected_history(&mut self) {
        if let Some(id) = self.selected_history_id() {
            self.preview_id(id);
        }
    }
    fn preview_id(&mut self, id: Uuid) {
        if let Some(j) = self.jobs.iter().find(|j| j.id == id) {
            self.modal = Some(Modal::Command(j.command_preview.clone()));
        }
    }

    pub fn click(&mut self, mouse: MouseEvent) {
        if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
            return;
        }
        let action = self
            .hits
            .iter()
            .rev()
            .find(|h| contains(h.rect, mouse.column, mouse.row))
            .map(|h| h.action.clone());
        match action {
            Some(ClickAction::Tab(t)) => self.tab = t,
            Some(ClickAction::SearchInput) => self.edit(
                "YouTube search",
                self.search_query.clone(),
                EditTarget::SearchQuery,
                false,
            ),
            Some(ClickAction::RunSearch) => self.run_search(),
            Some(ClickAction::SearchResult(i)) => self.select_search(i),
            Some(ClickAction::UseSearchResult) => self.use_search_selected(),
            Some(ClickAction::QueueSearchResult) => self.queue_search_selected(false),
            Some(ClickAction::ViewSearchThumbnail) => self.view_search_thumbnail(),
            Some(ClickAction::SearchVideoQuality(i)) => self.set_search_video(i),
            Some(ClickAction::SearchAudioQuality(i)) => self.set_search_audio(i),
            Some(ClickAction::ToggleSearchSubtitles) => self.toggle_search_subtitles(),
            Some(ClickAction::TogglePlayback) => self.toggle_audio_preview(),
            Some(ClickAction::StopPlayback) => self.stop_audio_preview(),
            Some(ClickAction::Preset(i)) => {
                self.preset_index = i;
                self.apply_preset();
            }
            Some(ClickAction::Builder(i)) => {
                self.builder_index = i;
                self.adjust_builder(true);
            }
            Some(ClickAction::Queue(i)) => self.queue_index = i,
            Some(ClickAction::History(i)) => self.history_index = i,
            Some(ClickAction::Setting(i)) => {
                self.settings_index = i;
                self.adjust_setting(true);
            }
            Some(ClickAction::Add) => self.add_urls(false),
            Some(ClickAction::AddStart) => self.add_urls(true),
            Some(ClickAction::Inspect) => self.inspect(),
            Some(ClickAction::Url) => self.edit("URL", self.url.clone(), EditTarget::Url, false),
            Some(ClickAction::PauseResume) => self.pause_resume_selected(),
            Some(ClickAction::Retry) => self.retry_selected(false),
            Some(ClickAction::Cancel) => self.confirm_cancel_selected(),
            Some(ClickAction::CloseModal) => self.modal = None,
            None => {}
        }
    }

    fn adjust_builder(&mut self, next: bool) {
        match self.builder_index {
            0 => {
                self.options.mode = cycle(
                    &[
                        MediaMode::Video,
                        MediaMode::Audio,
                        MediaMode::VideoOnly,
                        MediaMode::MetadataOnly,
                    ],
                    &self.options.mode,
                    next,
                );
                if self.options.mode == MediaMode::Audio {
                    self.options.embed_thumbnail = true;
                    self.options.metadata = true;
                    self.notice =
                        "Audio mode automatically embeds the best thumbnail as cover art".into();
                }
            }
            1 => {
                let mut qualities = Vec::new();
                if matches!(self.presets[0].options.quality, Quality::Screen { .. }) {
                    qualities.push(self.presets[0].options.quality.clone());
                }
                qualities.extend([
                    Quality::Best,
                    Quality::P2160,
                    Quality::P1440,
                    Quality::P1080,
                    Quality::P720,
                    Quality::P480,
                    Quality::P360,
                    Quality::Smallest,
                ]);
                self.options.quality = cycle(&qualities, &self.options.quality, next)
            }
            2 => {
                self.options.video_container = cycle_str(
                    &["auto", "mp4", "mkv", "webm"],
                    &self.options.video_container,
                    next,
                )
            }
            3 => {
                self.options.audio_format = cycle_str(
                    &["best", "mp3", "m4a", "opus", "flac", "wav", "aac", "vorbis"],
                    &self.options.audio_format,
                    next,
                )
            }
            4 => self.options.playlist = !self.options.playlist,
            5 => self.options.subtitles = !self.options.subtitles,
            6 => self.options.auto_subtitles = !self.options.auto_subtitles,
            7 => self.options.embed_subtitles = !self.options.embed_subtitles,
            8 => self.options.thumbnail = !self.options.thumbnail,
            9 if self.options.mode == MediaMode::Audio => {
                self.notice = "Cover art is automatic for audio-only downloads".into()
            }
            9 => self.options.embed_thumbnail = !self.options.embed_thumbnail,
            10 if self.options.mode == MediaMode::Audio => {
                self.notice = "Metadata is automatic for audio-only downloads".into()
            }
            10 => self.options.metadata = !self.options.metadata,
            11 => self.options.chapters = !self.options.chapters,
            12 => self.options.sponsorblock = !self.options.sponsorblock,
            13 => self.options.comments = !self.options.comments,
            14 => self.options.info_json = !self.options.info_json,
            15 => self.options.description = !self.options.description,
            16 => self.options.live_from_start = !self.options.live_from_start,
            17..=20 => self.edit_builder_value(),
            21 => {
                self.modal = Some(Modal::Explorer {
                    query: String::new(),
                    scroll: 0,
                })
            }
            _ => {}
        }
        self.dirty = true;
    }

    fn edit_builder_value(&mut self) {
        match self.builder_index {
            3 => self.edit(
                "Audio quality (0 best, 10 worst, or bitrate e.g. 192K)",
                self.options.audio_quality.clone(),
                EditTarget::AudioQuality,
                false,
            ),
            4 => self.edit(
                "Playlist items (e.g. 1:10,15,-1)",
                self.options.playlist_items.clone(),
                EditTarget::PlaylistItems,
                false,
            ),
            5..=7 => self.edit(
                "Subtitle languages",
                self.options.subtitle_languages.clone(),
                EditTarget::SubtitleLanguages,
                false,
            ),
            12 => self.edit(
                "SponsorBlock categories",
                self.options.sponsor_categories.clone(),
                EditTarget::SponsorCategories,
                false,
            ),
            17 => self.edit(
                "Only items after date (YYYYMMDD / today-Nweeks)",
                self.options.date_after.clone(),
                EditTarget::DateAfter,
                false,
            ),
            18 => self.edit(
                "Only items before date (YYYYMMDD / today-Nweeks)",
                self.options.date_before.clone(),
                EditTarget::DateBefore,
                false,
            ),
            19 => self.edit(
                "Match filter expression",
                self.options.match_filter.clone(),
                EditTarget::MatchFilter,
                false,
            ),
            20 => self.edit(
                "Advanced yt-dlp arguments",
                self.options.raw_args.clone(),
                EditTarget::RawArgs,
                false,
            ),
            _ => {}
        }
    }

    pub fn builder_values(&self) -> Vec<(&'static str, String)> {
        vec![
            ("Media", format!("{:?}", self.options.mode)),
            ("Quality", quality_name(&self.options.quality)),
            ("Container", self.options.video_container.clone()),
            ("Audio format", self.options.audio_format.clone()),
            ("Playlist", on(self.options.playlist)),
            ("Subtitles", on(self.options.subtitles)),
            ("Auto subtitles", on(self.options.auto_subtitles)),
            ("Embed subtitles", on(self.options.embed_subtitles)),
            ("Write thumbnail", on(self.options.thumbnail)),
            (
                "Embed thumbnail",
                if self.options.mode == MediaMode::Audio {
                    "on (automatic cover)".into()
                } else {
                    on(self.options.embed_thumbnail)
                },
            ),
            (
                "Embed metadata",
                if self.options.mode == MediaMode::Audio {
                    "on (automatic)".into()
                } else {
                    on(self.options.metadata)
                },
            ),
            ("Embed chapters", on(self.options.chapters)),
            ("Remove SponsorBlock", on(self.options.sponsorblock)),
            ("Write comments", on(self.options.comments)),
            ("Write info JSON", on(self.options.info_json)),
            ("Write description", on(self.options.description)),
            ("Live from start", on(self.options.live_from_start)),
            ("Date after", empty(&self.options.date_after)),
            ("Date before", empty(&self.options.date_before)),
            ("Match filter", empty(&self.options.match_filter)),
            (
                "Extra arguments",
                if self.options.raw_args.is_empty() {
                    "none".into()
                } else {
                    self.options.raw_args.clone()
                },
            ),
            ("All yt-dlp options", "search reference…".into()),
        ]
    }

    fn adjust_setting(&mut self, next: bool) {
        match self.settings_index {
            2 => {
                self.settings.max_parallel_downloads =
                    step(self.settings.max_parallel_downloads, 1, 8, next)
            }
            3 => {
                self.settings.concurrent_fragments =
                    step(self.settings.concurrent_fragments, 1, 32, next)
            }
            9 if !self.settings.use_aria2 && !ytdlp::executable_exists("aria2c") => {
                self.modal = Some(Modal::Error(
                    "aria2c is not installed or not on PATH. Native concurrent fragments remain enabled.".into(),
                ));
            }
            9 => self.settings.use_aria2 = !self.settings.use_aria2,
            10 => {
                self.settings.aria2_connections = step(self.settings.aria2_connections, 1, 16, next)
            }
            11 => self.settings.continue_downloads = !self.settings.continue_downloads,
            12 => self.settings.no_overwrites = !self.settings.no_overwrites,
            13 => self.settings.archive = !self.settings.archive,
            15 => self.settings.restrict_filenames = !self.settings.restrict_filenames,
            16 => self.settings.write_playlist_metafiles = !self.settings.write_playlist_metafiles,
            25 => self.settings.notifications = !self.settings.notifications,
            _ => self.edit_setting(),
        }
        self.dirty = true;
    }

    fn edit_setting(&mut self) {
        let values = self.setting_values();
        if let Some((name, value)) = values.get(self.settings_index) {
            let secret = matches!(self.settings_index, 17..=19);
            let initial = if value == "none" {
                String::new()
            } else {
                value.clone()
            };
            self.edit(
                name,
                initial,
                EditTarget::Setting(self.settings_index),
                secret,
            );
        }
    }

    pub fn setting_values(&self) -> Vec<(&'static str, String)> {
        vec![
            (
                "Output directory",
                self.settings.output_dir.to_string_lossy().into(),
            ),
            ("Filename template", self.settings.output_template.clone()),
            (
                "Parallel jobs",
                self.settings.max_parallel_downloads.to_string(),
            ),
            (
                "Concurrent fragments",
                self.settings.concurrent_fragments.to_string(),
            ),
            ("Retries", self.settings.retries.clone()),
            ("Fragment retries", self.settings.fragment_retries.clone()),
            ("Retry sleep", self.settings.retry_sleep.clone()),
            ("Rate limit", empty(&self.settings.rate_limit)),
            ("Throttle threshold", empty(&self.settings.throttled_rate)),
            ("Use aria2", on(self.settings.use_aria2)),
            (
                "aria2 connections",
                self.settings.aria2_connections.to_string(),
            ),
            ("Resume partials", on(self.settings.continue_downloads)),
            ("Protect existing", on(self.settings.no_overwrites)),
            ("Download archive", on(self.settings.archive)),
            (
                "Archive file",
                self.settings.archive_path.to_string_lossy().into(),
            ),
            ("Safe filenames", on(self.settings.restrict_filenames)),
            (
                "Playlist metafiles",
                on(self.settings.write_playlist_metafiles),
            ),
            ("Cookies browser", empty(&self.settings.cookies_browser)),
            ("Cookies file", empty(&self.settings.cookies_file)),
            ("Proxy", empty(&self.settings.proxy)),
            ("Impersonate", empty(&self.settings.impersonate)),
            ("Geo country", empty(&self.settings.geo_bypass_country)),
            ("User agent", empty(&self.settings.user_agent)),
            ("ffmpeg path", empty(&self.settings.ffmpeg_location)),
            ("yt-dlp executable", self.settings.yt_dlp_path.clone()),
            ("Notifications", on(self.settings.notifications)),
            ("Socket timeout", self.settings.socket_timeout.to_string()),
            ("Sleep requests", self.settings.sleep_requests.to_string()),
        ]
    }

    fn set_setting_text(&mut self, i: usize, v: String) {
        match i {
            0 => self.settings.output_dir = PathBuf::from(v),
            1 => self.settings.output_template = v,
            2 => {
                if let Ok(n) = v.parse() {
                    self.settings.max_parallel_downloads = n;
                }
            }
            3 => {
                if let Ok(n) = v.parse() {
                    self.settings.concurrent_fragments = n;
                }
            }
            4 => self.settings.retries = v,
            5 => self.settings.fragment_retries = v,
            6 => self.settings.retry_sleep = v,
            7 => self.settings.rate_limit = none(v),
            8 => self.settings.throttled_rate = none(v),
            9 => self.settings.use_aria2 = boolish(&v),
            10 => {
                if let Ok(n) = v.parse() {
                    self.settings.aria2_connections = n;
                }
            }
            11 => self.settings.continue_downloads = boolish(&v),
            12 => self.settings.no_overwrites = boolish(&v),
            13 => self.settings.archive = boolish(&v),
            14 => self.settings.archive_path = PathBuf::from(v),
            15 => self.settings.restrict_filenames = boolish(&v),
            16 => self.settings.write_playlist_metafiles = boolish(&v),
            17 => self.settings.cookies_browser = none(v),
            18 => self.settings.cookies_file = none(v),
            19 => self.settings.proxy = none(v),
            20 => self.settings.impersonate = none(v),
            21 => self.settings.geo_bypass_country = none(v),
            22 => self.settings.user_agent = none(v),
            23 => self.settings.ffmpeg_location = none(v),
            24 => self.settings.yt_dlp_path = v,
            25 => self.settings.notifications = boolish(&v),
            26 => {
                if let Ok(n) = v.parse() {
                    self.settings.socket_timeout = n;
                }
            }
            27 => {
                if let Ok(n) = v.parse() {
                    self.settings.sleep_requests = n;
                }
            }
            _ => {}
        }
        self.settings.max_parallel_downloads = self.settings.max_parallel_downloads.clamp(1, 8);
        self.settings.concurrent_fragments = self.settings.concurrent_fragments.clamp(1, 32);
        self.dependencies = dependency_status(&self.settings);
    }
}

pub fn builder_rows() -> [&'static str; 22] {
    [
        "Media",
        "Quality",
        "Container",
        "Audio format",
        "Playlist",
        "Subtitles",
        "Auto subtitles",
        "Embed subtitles",
        "Write thumbnail",
        "Embed thumbnail",
        "Embed metadata",
        "Embed chapters",
        "Remove SponsorBlock",
        "Write comments",
        "Write info JSON",
        "Write description",
        "Live from start",
        "Date after",
        "Date before",
        "Match filter",
        "Extra arguments",
        "All yt-dlp options",
    ]
}
pub fn setting_rows() -> [&'static str; 28] {
    [
        "Output directory",
        "Filename template",
        "Parallel jobs",
        "Concurrent fragments",
        "Retries",
        "Fragment retries",
        "Retry sleep",
        "Rate limit",
        "Throttle threshold",
        "Use aria2",
        "aria2 connections",
        "Resume partials",
        "Protect existing",
        "Download archive",
        "Archive file",
        "Safe filenames",
        "Playlist metafiles",
        "Cookies browser",
        "Cookies file",
        "Proxy",
        "Impersonate",
        "Geo country",
        "User agent",
        "ffmpeg path",
        "yt-dlp executable",
        "Notifications",
        "Socket timeout",
        "Sleep requests",
    ]
}

fn normalize_url(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let candidate = if raw.starts_with("http://") || raw.starts_with("https://") {
        raw.into()
    } else {
        format!("https://{raw}")
    };
    url::Url::parse(&candidate)
        .ok()
        .filter(|u| matches!(u.scheme(), "http" | "https") && u.host().is_some())
        .map(|u| u.into())
}

fn none_choice() -> Vec<SearchFormatChoice> {
    vec![SearchFormatChoice {
        format_id: String::new(),
        label: "None".into(),
        detail: "Do not download this stream".into(),
    }]
}

fn slider_step(current: usize, len: usize, next: bool) -> usize {
    if len == 0 {
        0
    } else if next {
        (current + 1).min(len - 1)
    } else {
        current.saturating_sub(1)
    }
}

fn format_choices(info: &VideoInfo) -> (Vec<SearchFormatChoice>, Vec<SearchFormatChoice>) {
    let has_video_only = info.formats.iter().any(|format| {
        format.height.is_some()
            && format.vcodec != "none"
            && !format.vcodec.is_empty()
            && (format.acodec == "none" || format.acodec.is_empty())
    });
    let mut by_height = std::collections::BTreeMap::new();
    for format in &info.formats {
        let Some(height) = format.height else {
            continue;
        };
        if format.vcodec == "none" || format.vcodec.is_empty() {
            continue;
        }
        let video_only = format.acodec == "none" || format.acodec.is_empty();
        if has_video_only && !video_only {
            continue;
        }
        let score = format.tbr.unwrap_or(0.0) + format.fps.unwrap_or(0.0);
        let replace = by_height
            .get(&height)
            .is_none_or(|old: &&ytdlp::FormatInfo| {
                score > old.tbr.unwrap_or(0.0) + old.fps.unwrap_or(0.0)
            });
        if replace {
            by_height.insert(height, format);
        }
    }
    let mut video = none_choice();
    video.extend(by_height.into_iter().map(|(height, format)| {
        let fps = format
            .fps
            .filter(|fps| *fps > 30.0)
            .map_or_else(String::new, |fps| format!("{}fps", fps.round()));
        SearchFormatChoice {
            format_id: format.format_id.clone(),
            label: format!("{height}p{fps}"),
            detail: format!(
                "{}×{} · {} · {} · {}",
                format.width.unwrap_or(0),
                height,
                codec_name(&format.vcodec),
                bitrate(format.tbr),
                format_size(format.filesize.or(format.filesize_approx))
            ),
        }
    }));

    let original_language = info
        .formats
        .iter()
        .find(|format| {
            let note = format.note.to_ascii_lowercase();
            !format.language.is_empty() && (note.contains("original") || note.contains("(default)"))
        })
        .map(|format| format.language.as_str());
    let mut audio_formats: Vec<&ytdlp::FormatInfo> = info
        .formats
        .iter()
        .filter(|format| {
            (format.vcodec == "none" || format.vcodec.is_empty())
                && format.acodec != "none"
                && !format.acodec.is_empty()
                && original_language.is_none_or(|language| format.language == language)
        })
        .collect();
    audio_formats.sort_by(|left, right| {
        let left_rate = left.abr.or(left.tbr).unwrap_or(0.0);
        let right_rate = right.abr.or(right.tbr).unwrap_or(0.0);
        left_rate
            .total_cmp(&right_rate)
            .then_with(|| left.format_id.cmp(&right.format_id))
    });
    let mut audio = none_choice();
    for format in audio_formats {
        let rate = format.abr.or(format.tbr).unwrap_or(0.0);
        let label = if rate > 0.0 {
            format!("{}k {}", rate.round(), codec_name(&format.acodec))
        } else {
            codec_name(&format.acodec)
        };
        if audio.iter().any(|choice| choice.label == label) {
            continue;
        }
        audio.push(SearchFormatChoice {
            format_id: format.format_id.clone(),
            label,
            detail: format!(
                "{}{} · {} channels · {}",
                format.ext,
                if format.language.is_empty() {
                    String::new()
                } else {
                    format!(" · {}", format.language)
                },
                format.audio_channels.unwrap_or(2),
                format_size(format.filesize.or(format.filesize_approx))
            ),
        });
    }
    (video, audio)
}

fn codec_name(codec: &str) -> String {
    let codec = codec.split('.').next().unwrap_or(codec);
    match codec {
        "av01" => "AV1".into(),
        "vp9" => "VP9".into(),
        "avc1" | "h264" => "H.264".into(),
        "opus" => "Opus".into(),
        "mp4a" | "aac" => "AAC".into(),
        "" => "unknown".into(),
        other => other.to_uppercase(),
    }
}

fn bitrate(rate: Option<f64>) -> String {
    rate.map_or_else(|| "bitrate unknown".into(), |rate| format!("{rate:.0}k"))
}

fn format_size(size: Option<u64>) -> String {
    let Some(size) = size else {
        return "size unknown".into();
    };
    if size >= 1_000_000_000 {
        format!("{:.1} GB", size as f64 / 1_000_000_000.0)
    } else {
        format!("{:.1} MB", size as f64 / 1_000_000.0)
    }
}

fn compact_error(error: &str) -> String {
    const LIMIT: usize = 180;
    let one_line = error.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() <= LIMIT {
        one_line
    } else {
        format!("{}…", one_line.chars().take(LIMIT).collect::<String>())
    }
}
fn contains(r: Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}
fn cycle<T: Clone + PartialEq>(items: &[T], current: &T, next: bool) -> T {
    let i = items.iter().position(|x| x == current).unwrap_or(0);
    items[if next {
        (i + 1) % items.len()
    } else {
        (i + items.len() - 1) % items.len()
    }]
    .clone()
}
fn cycle_str(items: &[&str], current: &str, next: bool) -> String {
    let i = items.iter().position(|x| *x == current).unwrap_or(0);
    items[if next {
        (i + 1) % items.len()
    } else {
        (i + items.len() - 1) % items.len()
    }]
    .into()
}
fn step(v: usize, min: usize, max: usize, next: bool) -> usize {
    if next {
        (v + 1).min(max)
    } else {
        v.saturating_sub(1).max(min)
    }
}
fn on(v: bool) -> String {
    if v { "on" } else { "off" }.into()
}
fn empty(v: &str) -> String {
    if v.is_empty() {
        "none".into()
    } else {
        v.into()
    }
}
fn none(v: String) -> String {
    if v == "none" { String::new() } else { v }
}
fn boolish(v: &str) -> bool {
    matches!(v.to_ascii_lowercase().as_str(), "on" | "true" | "yes" | "1")
}
fn byte_index(s: &str, char_index: usize) -> usize {
    s.char_indices()
        .nth(char_index)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}
fn insert_at_char(value: &mut String, cursor: usize, text: &str) {
    value.insert_str(byte_index(value, cursor), text);
}
fn remove_char(value: &mut String, at: usize) {
    let start = byte_index(value, at);
    let end = byte_index(value, at + 1);
    value.replace_range(start..end, "");
}
fn delete_previous_word(value: &mut String, cursor: &mut usize) {
    while *cursor > 0
        && value
            .chars()
            .nth(*cursor - 1)
            .is_some_and(char::is_whitespace)
    {
        remove_char(value, *cursor - 1);
        *cursor -= 1;
    }
    while *cursor > 0
        && value
            .chars()
            .nth(*cursor - 1)
            .is_some_and(|c| !c.is_whitespace())
    {
        remove_char(value, *cursor - 1);
        *cursor -= 1;
    }
}
fn quality_name(q: &Quality) -> String {
    match q {
        Quality::Best => "best".into(),
        Quality::Screen { width, height } => format!("screen ≤{width}×{height}"),
        Quality::P2160 => "≤2160p".into(),
        Quality::P1440 => "≤1440p".into(),
        Quality::P1080 => "≤1080p".into(),
        Quality::P720 => "≤720p".into(),
        Quality::P480 => "≤480p".into(),
        Quality::P360 => "≤360p".into(),
        Quality::Smallest => "smallest".into(),
        Quality::Custom(s) => s.clone(),
    }
}
fn dependency_status(settings: &Settings) -> Vec<(String, bool)> {
    vec![
        (
            settings.yt_dlp_path.clone(),
            ytdlp::executable_exists(&settings.yt_dlp_path),
        ),
        ("ffmpeg".into(), ytdlp::executable_exists("ffmpeg")),
        (
            "aria2c (optional)".into(),
            ytdlp::executable_exists("aria2c"),
        ),
        (
            "notify-send (optional)".into(),
            ytdlp::executable_exists("notify-send"),
        ),
        ("viu (optional)".into(), ytdlp::executable_exists("viu")),
        (
            "mpv audio preview (optional)".into(),
            ytdlp::executable_exists("mpv"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalizes_bare_urls() {
        assert_eq!(
            normalize_url("youtu.be/abc").unwrap(),
            "https://youtu.be/abc"
        );
        assert!(normalize_url("not a url").is_none());
    }

    #[test]
    fn unicode_editor_inserts_and_deletes_by_character() {
        let mut text = "ab界d".to_string();
        insert_at_char(&mut text, 2, "🙂");
        assert_eq!(text, "ab🙂界d");
        remove_char(&mut text, 3);
        assert_eq!(text, "ab🙂d");
        let mut cursor = text.chars().count();
        delete_previous_word(&mut text, &mut cursor);
        assert_eq!(text, "");
        assert_eq!(cursor, 0);
    }

    #[test]
    fn search_sliders_use_only_available_media_levels() {
        let info = VideoInfo {
            formats: vec![
                ytdlp::FormatInfo {
                    format_id: "v720".into(),
                    ext: "webm".into(),
                    width: Some(1280),
                    height: Some(720),
                    fps: Some(30.0),
                    vcodec: "vp9".into(),
                    acodec: "none".into(),
                    tbr: Some(1200.0),
                    ..Default::default()
                },
                ytdlp::FormatInfo {
                    format_id: "v1080".into(),
                    ext: "webm".into(),
                    width: Some(1920),
                    height: Some(1080),
                    fps: Some(60.0),
                    vcodec: "av01.0".into(),
                    acodec: "none".into(),
                    tbr: Some(2200.0),
                    ..Default::default()
                },
                ytdlp::FormatInfo {
                    format_id: "a128".into(),
                    ext: "m4a".into(),
                    vcodec: "none".into(),
                    acodec: "mp4a.40.2".into(),
                    abr: Some(128.0),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let (video, audio) = format_choices(&info);
        assert_eq!(video[0].label, "None");
        assert_eq!(video[1].format_id, "v720");
        assert_eq!(video[2].format_id, "v1080");
        assert_eq!(audio[0].label, "None");
        assert_eq!(audio[1].format_id, "a128");
    }
}
