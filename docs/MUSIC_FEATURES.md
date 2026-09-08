# Music experience

youTUIbe's music surface is designed around a short loop: search, listen, queue, save.
Every primary operation has a single key and a mouse target.

## Shipped

- Search is the startup workspace; no API key is required.
- Play a result immediately or add it to the end/front of Up Next.
- Persistent Up Next queue with direct selection, removal, automatic progression, and
  failed-track skipping.
- Crash-safe session recovery: the interrupted current track returns to the front of Up Next.
- Previous/next, pause, seek, volume, mute, speed, audio-only, and optional video playback.
- Persistent recently played history used by the Previous action.
- Custom playlists with create, rename, delete, deduplicated save, track removal, playback,
  and enqueue actions.
- Three-pane playlist workspace: playlists, tracks, and Up Next.
- Artwork, duration, artist, and source URL stored with every saved track.
- Existing download presets, format controls, captions, cover art, and resilient downloads
  remain available without interrupting playback.

## Planned refinements

### Queue and playback

- Shuffle and repeat-one/repeat-all modes.
- Reorder, remove, clear, and save the current Up Next queue as a playlist.
- Related-track radio and optional autoplay when Up Next ends.
- Session restore prompt for the last track and playback position.
- Sleep timer, audio normalization, and configurable skip intervals.

### Library and discovery

- Favorites, pinned playlists, albums, artists, and a richer recently played browser.
- Search/filter/sort within large playlists and listening history.
- Search history, suggestions, music/video filters, and related-song discovery.
- Import/export M3U, PLS, and JSON playlists.
- Duplicate finder and playlist cleanup tools.

### Local and offline listening

- Prefer an already-downloaded local file before opening the network stream.
- Scan and index a local music directory without replacing the download archive.
- One-key offline download for a playlist with storage estimates and progress.
- Verify missing/moved local files and repair library paths safely.

### Terminal and desktop integration

- Synchronized lyrics/captions view and a distraction-free now-playing screen.
- Optional media-key and MPRIS integration.
- Optional notifications on track change and configurable Discord/Last.fm integrations.
- Theme presets, compact layouts, and accessibility-oriented high-contrast modes.
