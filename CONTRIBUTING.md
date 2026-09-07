# Contributing to youTUIbe

Use a current stable Rust toolchain on Linux. Install yt-dlp and FFmpeg; mpv,
viu, and ffmpegthumbnailer enable the optional player and artwork features.

1. Describe a bug or proposed feature in a GitHub issue.
2. Keep changes focused and preserve saved-state compatibility.
3. Run `cargo fmt --check`, `cargo test --locked`, and
   `cargo clippy --locked --all-targets --all-features -- -D warnings`.
4. Test relevant keyboard, mouse, terminal resize, and subprocess failure paths.
5. Open a pull request with the behavior change and validation results.

Do not commit cookies, credentials, downloaded media, state files, build output,
or terminal logs containing private information. Keep optional integration
failures separate from download success. Avoid transcoding audio by default.
Contributions are provided under the repository's MIT license.
