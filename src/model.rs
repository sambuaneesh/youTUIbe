use chrono::{DateTime, Local, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DownloadStatus {
    Queued,
    Inspecting,
    Downloading,
    Paused,
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl DownloadStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MediaMode {
    Video,
    Audio,
    VideoOnly,
    MetadataOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Quality {
    Best,
    Screen { width: u32, height: u32 },
    P2160,
    P1440,
    P1080,
    P720,
    P480,
    P360,
    Smallest,
    Custom(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DownloadOptions {
    pub mode: MediaMode,
    pub quality: Quality,
    pub video_container: String,
    pub audio_format: String,
    pub audio_quality: String,
    pub playlist: bool,
    pub playlist_items: String,
    pub subtitles: bool,
    pub auto_subtitles: bool,
    pub subtitle_languages: String,
    pub embed_subtitles: bool,
    pub thumbnail: bool,
    pub embed_thumbnail: bool,
    pub metadata: bool,
    pub chapters: bool,
    pub sponsorblock: bool,
    pub sponsor_categories: String,
    pub comments: bool,
    pub info_json: bool,
    pub description: bool,
    pub live_from_start: bool,
    pub date_after: String,
    pub date_before: String,
    pub match_filter: String,
    pub raw_args: String,
}

impl Default for DownloadOptions {
    fn default() -> Self {
        Self {
            mode: MediaMode::Video,
            quality: Quality::Best,
            video_container: "auto".into(),
            audio_format: "best".into(),
            audio_quality: "0".into(),
            playlist: false,
            playlist_items: String::new(),
            subtitles: false,
            auto_subtitles: false,
            subtitle_languages: "en.*,en".into(),
            embed_subtitles: false,
            thumbnail: false,
            embed_thumbnail: false,
            metadata: true,
            chapters: true,
            sponsorblock: false,
            sponsor_categories: "sponsor,selfpromo,interaction".into(),
            comments: false,
            info_json: false,
            description: false,
            live_from_start: false,
            date_after: String::new(),
            date_before: String::new(),
            match_filter: String::new(),
            raw_args: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub output_dir: PathBuf,
    pub output_template: String,
    pub max_parallel_downloads: usize,
    pub concurrent_fragments: usize,
    pub retries: String,
    pub fragment_retries: String,
    pub retry_sleep: String,
    pub socket_timeout: u64,
    pub rate_limit: String,
    pub throttled_rate: String,
    pub sleep_requests: f64,
    pub use_aria2: bool,
    pub aria2_connections: usize,
    pub continue_downloads: bool,
    pub no_overwrites: bool,
    pub archive: bool,
    pub archive_path: PathBuf,
    pub restrict_filenames: bool,
    pub write_playlist_metafiles: bool,
    pub cookies_browser: String,
    pub cookies_file: String,
    pub proxy: String,
    pub impersonate: String,
    pub geo_bypass_country: String,
    pub user_agent: String,
    pub ffmpeg_location: String,
    pub yt_dlp_path: String,
    pub notifications: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let output_dir = dirs_download().unwrap_or_else(|| PathBuf::from("downloads"));
        let archive_path = output_dir.join(".youtuibe-archive.txt");
        Self {
            output_dir,
            output_template: "%(uploader|Unknown)s/%(title)s [%(id)s].%(ext)s".into(),
            max_parallel_downloads: 2,
            concurrent_fragments: 4,
            retries: "10".into(),
            fragment_retries: "10".into(),
            retry_sleep: "fragment:exp=1:20".into(),
            socket_timeout: 30,
            rate_limit: String::new(),
            throttled_rate: "100K".into(),
            sleep_requests: 0.25,
            use_aria2: false,
            aria2_connections: 8,
            continue_downloads: true,
            no_overwrites: true,
            archive: true,
            archive_path,
            restrict_filenames: false,
            write_playlist_metafiles: false,
            cookies_browser: String::new(),
            cookies_file: String::new(),
            proxy: String::new(),
            impersonate: String::new(),
            geo_bypass_country: String::new(),
            user_agent: String::new(),
            ffmpeg_location: String::new(),
            yt_dlp_path: "yt-dlp".into(),
            notifications: true,
        }
    }
}

fn dirs_download() -> Option<PathBuf> {
    directories::UserDirs::new().and_then(|d| d.download_dir().map(PathBuf::from))
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub downloaded: u64,
    pub total: u64,
    pub speed: f64,
    pub eta: Option<u64>,
    pub percent: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: Uuid,
    pub url: String,
    pub title: String,
    pub preset: String,
    pub options: DownloadOptions,
    pub status: DownloadStatus,
    pub progress: Progress,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub output_path: String,
    pub error: String,
    pub command_preview: String,
}

impl Job {
    pub fn new(url: String, preset: String, options: DownloadOptions) -> Self {
        Self {
            id: Uuid::new_v4(),
            title: url.clone(),
            url,
            preset,
            options,
            status: DownloadStatus::Queued,
            progress: Progress::default(),
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
            output_path: String::new(),
            error: String::new(),
            command_preview: String::new(),
        }
    }

    pub fn short_time(&self) -> String {
        self.created_at
            .with_timezone(&Local)
            .format("%b %d %H:%M")
            .to_string()
    }
}

#[derive(Debug, Clone)]
pub struct Preset {
    pub name: &'static str,
    pub summary: &'static str,
    pub options: DownloadOptions,
}

pub fn presets() -> Vec<Preset> {
    let base = DownloadOptions::default();
    vec![
        Preset {
            name: "Best for screen",
            summary: "Caps video to the active display; keeps best audio",
            options: DownloadOptions {
                quality: Quality::Screen {
                    width: 0,
                    height: 0,
                },
                ..base.clone()
            },
        },
        Preset {
            name: "Compatible MP4",
            summary: "Up to 1080p H.264/AAC-friendly MP4",
            options: DownloadOptions {
                quality: Quality::P1080,
                video_container: "mp4".into(),
                ..base.clone()
            },
        },
        Preset {
            name: "4K archive",
            summary: "Best up to 2160p, MKV, thumbnail, JSON and subtitles",
            options: DownloadOptions {
                quality: Quality::P2160,
                video_container: "mkv".into(),
                thumbnail: true,
                embed_thumbnail: true,
                subtitles: true,
                auto_subtitles: true,
                embed_subtitles: true,
                info_json: true,
                description: true,
                ..base.clone()
            },
        },
        Preset {
            name: "Audio MP3",
            summary: "High quality V0 MP3 with cover art and metadata",
            options: DownloadOptions {
                mode: MediaMode::Audio,
                audio_format: "mp3".into(),
                audio_quality: "0".into(),
                thumbnail: true,
                embed_thumbnail: true,
                ..base.clone()
            },
        },
        Preset {
            name: "Audio original",
            summary: "Best source audio, embedded HD cover and metadata",
            options: DownloadOptions {
                mode: MediaMode::Audio,
                audio_format: "best".into(),
                embed_thumbnail: true,
                ..base.clone()
            },
        },
        Preset {
            name: "720p saver",
            summary: "Good quality with modest size and broad compatibility",
            options: DownloadOptions {
                quality: Quality::P720,
                video_container: "mp4".into(),
                ..base.clone()
            },
        },
        Preset {
            name: "Metadata kit",
            summary: "No media; JSON, thumbnail, description and subtitles",
            options: DownloadOptions {
                mode: MediaMode::MetadataOnly,
                thumbnail: true,
                subtitles: true,
                auto_subtitles: true,
                info_json: true,
                description: true,
                ..base
            },
        },
    ]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PersistedState {
    pub settings: Settings,
    pub jobs: Vec<Job>,
}
