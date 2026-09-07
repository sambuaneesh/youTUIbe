# youTUIbe

Search, listen, and download from your terminal. **youTUIbe** combines a persistent download queue, a media player, and a guided yt-dlp builder in one keyboard and mouse interface. The command is always `youtuibe`.

[Releases](https://github.com/sambuaneesh/youTUIbe/releases) · [Report an issue](https://github.com/sambuaneesh/youTUIbe/issues) · [Release and AUR guide](docs/RELEASING.md)

Linux is the supported platform. Video playback opens an mpv window; audio playback stays in the terminal. Search requires no YouTube API key.

## Highlights

- Seven practical presets: display-aware best video, compatible 1080p MP4, 4K archival, MP3, original audio, 720p saver, and metadata kit
- Automatic high-quality cover art for every audio-only download while retaining the original compressed stream; youTUIbe also installs a user-level Opus thumbnailer registration when supported so desktop file managers can display that embedded artwork
- Active-monitor detection through Hyprland, xrandr, or Linux DRM; the default preset caps both video width and height to that screen while retaining the best audio stream
- YouTube search through yt-dlp itself (no separate API key), with 20 navigable results, metadata, view counts, and one-key use, inspect, or queue actions
- Per-result video and audio quality sliders built from the formats actually reported by yt-dlp; either stream can independently be set to None
- Subtitle discovery with a one-key toggle that prefers manual English captions, then another manual language, then automatic captions
- Full YouTube Music-style player through optional `mpv`: complete audio playback, external-window video playback, clickable seek and volume sliders, pause/resume, mute, speed control, and format-aware streaming
- Asynchronous thumbnail fetching with Kitty, Sixel, iTerm2, and Unicode half-block fallback rendering; thumbnail or search errors never stop direct downloads
- Full-terminal `viu --blocks` preview from the Search tab for a larger true-color image (`v` or the View HD button), avoiding false-positive terminal graphics detection
- Guided builder for media mode, resolution cap, container, audio codec/quality, playlist ranges, subtitles, thumbnails, metadata, chapters, SponsorBlock, comments, live downloads, date filters, and match filters
- Searchable live reference built from the installed `yt-dlp --help`, with safely parsed extra arguments for every uncommon feature
- Persistent multi-job queue with progress, speed, ETA, pause/resume, cancellation, retry, and force retry
- Inspect-before-download view with metadata and available formats
- Keyboard-first navigation plus mouse tabs, rows, presets, buttons, and dialogs
- Per-job logs and exact command previews (password and proxy values are redacted)
- Cookie-browser/file, proxy, impersonation, geo, user-agent, custom ffmpeg, archive, rate, timeout, retry, and filename settings
- Native concurrent fragment downloads and optional aria2 acceleration

## Resilience model

youTUIbe uses `--part` and `--continue`, stores only successful IDs in a `--download-archive`, protects completed files with `--no-overwrites`, and atomically replaces its state file. If youTUIbe or the machine stops, active jobs become queued at the next launch and yt-dlp resumes their partial files where the server permits it. Cancelling keeps partial data intentionally.

Every subprocess receives an argument vector directly; youTUIbe never interpolates a job into a shell command. Extra arguments support normal shell-style quoting only as an input format. Password-like fields are masked in previews.

## Requirements

- Current stable Rust (minimum 1.88; dependency requirements may be higher) to build
- `yt-dlp` on `PATH` (required)
- `ffmpeg` on `PATH` (strongly recommended for merging, remuxing, metadata, and audio extraction)
- `aria2c` (optional and off by default)
- `mpv` (optional, for complete audio/video playback and interactive controls from Search)
- `viu` (optional, for full-terminal true-color thumbnail previews)
- `ffmpegthumbnailer` (optional, for desktop artwork icons on source-quality Opus files)
- A terminal with Kitty, Sixel, or iTerm2 image support for native thumbnails; all other terminals use a Unicode fallback

youTUIbe checks these executables on startup and shows their status under Settings.

## Build and run

```bash
cargo build --release --locked
./target/release/youtuibe
```

Or install it into Cargo's binary directory:

```bash
cargo install --locked --path .
youtuibe
```

You can seed the input field and override the output directory:

```bash
youtuibe --output ~/Videos 'https://youtu.be/…'
```

Use `youtuibe --help` for CLI flags. Inside youTUIbe, press `?` for the full key map and `o` to search every option supported by your installed yt-dlp.

## Main controls

| Key | Action |
|---|---|
| `1`…`7` | Switch tabs |
| `/` | Edit URL or search input |
| `j` / `k` | Change preset or selected row |
| arrows / `Space` | Navigate and change options |
| `e` | Edit the selected detailed value |
| `i` | Inspect media and formats |
| `a` / `d` | Add to queue / add and open queue |
| `p` | Pause or resume selected job |
| `x` | Cancel while retaining partial data |
| `r` / `R` | Retry safely / retry with overwrite |
| `c` | Show command preview (clear logs on Logs tab) |
| `o` | Search installed yt-dlp options |
| `q` | Save and quit |

On the Search tab, `/` or `e` edits and runs a query, `s`/`r` reruns it, `[`/`]` changes the video stream, `{`/`}` changes the audio stream, and `t` toggles discovered captions. `p` starts complete audio, `P` opens complete video in mpv, `Space` pauses/resumes, `x` stops, arrows seek, `-`/`+` adjust volume, `m` mutes, and `,`/`.` adjust playback speed (`0` resets it). The timeline and volume bars are mouse-seekable. `v` opens artwork in a full-terminal `viu` view. `Enter` sends the configured result to the Download builder, `a` queues it, `d` queues and opens the Queue tab, and `i` performs a full format inspection. Search, metadata, playback, and thumbnail networking are independent of the download engine: failures leave every direct-URL feature operational.

## Performance guidance

The defaults are intentionally balanced: two jobs and four fragments per job. More concurrency is not automatically faster; it can add random disk I/O, increase throttling, and provoke HTTP 429 responses. aria2 is therefore opt-in. Try native fragments first, measure on the specific host and connection, and increase gradually.

## Data locations

By default, state is written to the platform's local application-data directory (on Linux, typically `~/.local/share/youtuibe/state.json`). Downloads go to the platform Downloads directory, and the archive defaults to `.youtuibe-archive.txt` there. Both are editable in Settings. Pass `--state path.json` for a portable state file.

On first launch, youTUIbe imports the previous Tide state if present and retains its existing archive path. The original state remains intact. Quit the old app before migrating; do not run two instances against the same state file.

Audio original and the Search audio-only controls preserve the compressed source using `--audio-format best`. They never convert Opus to FLAC automatically. MP3, FLAC, and other conversions are explicit choices in the builder. Embedded artwork retains its aspect ratio. Desktop thumbnails fit the entire cover inside a padded square, without cropping or stretching. Optional desktop integration writes `~/.local/share/thumbnailers/youtuibe-opus.thumbnailer` and standard PNG caches under `~/.cache/thumbnails`.

## Arch Linux and AUR

Packaging is prepared for `youtuibe` (source build) and `youtuibe-bin` (x86_64 prebuilt release). Both install `/usr/bin/youtuibe` and conflict with one another. They require yt-dlp, FFmpeg, mpv, viu, and ffmpegthumbnailer; optional acceleration, notifications, and display detection are listed in their package metadata.

Install either with `yay -S youtuibe` or `yay -S youtuibe-bin`. Maintainers should follow [the release guide](docs/RELEASING.md) to generate checksummed archives and AUR metadata. Do not upload binaries to an AUR Git repository.

## Troubleshooting

- Pixelated artwork means the terminal selected the Unicode fallback. Native Kitty/Sixel/iTerm2 graphics depend on the terminal and any multiplexer; `v` provides a larger fallback view.
- Missing search qualities can mean the result is a channel or playlist, extraction failed, or metadata is still loading. Direct URL downloads remain available.
- A missing desktop icon does not imply missing embedded artwork. Install `ffmpegthumbnailer`, restart youTUIbe, and refresh the file manager.
- A completed item may have been skipped because its ID was already in the download archive. Resuming depends on server support.
- For reports, include app and yt-dlp versions, terminal, reproduction steps, and sanitized logs. Remove cookies, proxy credentials, and private URLs.

## Development and license

Run `cargo fmt --check`, `cargo test --locked`, and `cargo clippy --locked --all-targets --all-features -- -D warnings` before contributing. See [CONTRIBUTING.md](CONTRIBUTING.md).

MIT licensed; see [LICENSE](LICENSE). Dependencies retain their own licenses. youTUIbe is independent and is not affiliated with or endorsed by YouTube or Google.

## Scope and responsible use

youTUIbe is a frontend, not a downloader implementation; site support and extraction behavior come from yt-dlp. Download only material you are permitted to access and follow the platform's terms and local law. youTUIbe does not bypass DRM.
