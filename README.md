# Tide

Tide is a responsive, persistent Ratatui command center for `yt-dlp`. It keeps the common path simple—paste, pick a preset, download—while still making the complete option set of the installed `yt-dlp` searchable inside the app.

## Highlights

- Seven practical presets: display-aware best video, compatible 1080p MP4, 4K archival, MP3, original audio, 720p saver, and metadata kit
- Automatic high-quality cover art for every audio-only download: Tide embeds yt-dlp's best thumbnail as compatible JPEG artwork together with media metadata
- Active-monitor detection through Hyprland, xrandr, or Linux DRM; the default preset caps both video width and height to that screen while retaining the best audio stream
- YouTube search through yt-dlp itself (no separate API key), with 20 navigable results, metadata, view counts, and one-key use, inspect, or queue actions
- Per-result video and audio quality sliders built from the formats actually reported by yt-dlp; either stream can independently be set to None
- Subtitle discovery with a one-key toggle that prefers manual English captions, then another manual language, then automatic captions
- In-terminal YouTube Music-style audio preview through optional `mpv`, including play, pause, resume, stop, and format-aware streaming
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

Tide uses `--part` and `--continue`, stores only successful IDs in a `--download-archive`, protects completed files with `--no-overwrites`, and atomically replaces its state file. If Tide or the machine stops, active jobs become queued at the next launch and yt-dlp resumes their partial files where the server permits it. Cancelling keeps partial data intentionally.

Every subprocess receives an argument vector directly; Tide never interpolates a job into a shell command. Extra arguments support normal shell-style quoting only as an input format. Password-like fields are masked in previews.

## Requirements

- Rust 1.85 or newer to build
- `yt-dlp` on `PATH` (required)
- `ffmpeg` on `PATH` (strongly recommended for merging, remuxing, metadata, and audio extraction)
- `aria2c` (optional and off by default)
- `mpv` (optional, for streaming audio previews from Search)
- `viu` (optional, for full-terminal true-color thumbnail previews)
- A terminal with Kitty, Sixel, or iTerm2 image support for native thumbnails; all other terminals use a Unicode fallback

Tide checks these executables on startup and shows their status under Settings.

## Build and run

```bash
cargo build --release
./target/release/tide
```

Or install it into Cargo's binary directory:

```bash
cargo install --path .
tide
```

You can seed the input field and override the output directory:

```bash
tide --output ~/Videos 'https://youtu.be/…'
```

Use `tide --help` for CLI flags. Inside Tide, press `?` for the full key map and `o` to search every option supported by your installed yt-dlp.

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

On the Search tab, `/` or `e` edits and runs a query, `s`/`r` reruns it, `[`/`]` changes the video stream, `{`/`}` changes the audio stream, `t` toggles discovered captions, `p`/`Space` plays or pauses the audio preview, `x` stops it, and `v` opens the thumbnail in a full-terminal `viu` preview. `Enter` sends the configured result to the Download builder, `a` queues it, `d` queues and opens the Queue tab, and `i` performs a full format inspection. Search, metadata, playback, and thumbnail networking are independent of the download engine: failures leave every direct-URL feature operational.

## Performance guidance

The defaults are intentionally balanced: two jobs and four fragments per job. More concurrency is not automatically faster; it can add random disk I/O, increase throttling, and provoke HTTP 429 responses. aria2 is therefore opt-in. Try native fragments first, measure on the specific host and connection, and increase gradually.

## Data locations

By default, state is written to the platform's local application-data directory (on Linux, typically `~/.local/share/tide-dlp/state.json`). Downloads go to the platform Downloads directory, and the archive defaults to `.tide-archive.txt` there. Both are editable in Settings. Pass `--state path.json` for a portable state file.

## Scope and responsible use

Tide is a frontend, not a downloader implementation; site support and extraction behavior come from yt-dlp. Download only material you are permitted to access and follow the platform's terms and local law. Tide does not bypass DRM.
