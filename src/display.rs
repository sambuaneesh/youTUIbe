use serde::Deserialize;
use std::{fs, path::Path, process::Command};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayInfo {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub source: &'static str,
}

/// Detect the currently relevant physical display. Compositor-native data is
/// preferred because Xwayland may expose a synthetic desktop rectangle.
pub fn detect() -> Option<DisplayInfo> {
    detect_hyprland().or_else(detect_xrandr).or_else(detect_drm)
}

#[derive(Debug, Deserialize)]
struct HyprMonitor {
    #[serde(default)]
    name: String,
    #[serde(default)]
    width: i64,
    #[serde(default)]
    height: i64,
    #[serde(default)]
    transform: i64,
    #[serde(default)]
    focused: bool,
    #[serde(default)]
    disabled: bool,
}

fn detect_hyprland() -> Option<DisplayInfo> {
    let out = Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let monitors: Vec<HyprMonitor> = serde_json::from_slice(&out.stdout).ok()?;
    let monitor = monitors
        .iter()
        .filter(|m| !m.disabled && m.width > 0 && m.height > 0)
        .max_by_key(|m| (m.focused, m.width * m.height))?;
    let (width, height) = if matches!(monitor.transform, 1 | 3 | 5 | 7) {
        (monitor.height as u32, monitor.width as u32)
    } else {
        (monitor.width as u32, monitor.height as u32)
    };
    Some(DisplayInfo {
        name: monitor.name.clone(),
        width,
        height,
        source: "Hyprland",
    })
}

fn detect_xrandr() -> Option<DisplayInfo> {
    let out = Command::new("xrandr").arg("--current").output().ok()?;
    if !out.status.success() {
        return None;
    }
    parse_xrandr(&String::from_utf8_lossy(&out.stdout))
}

fn parse_xrandr(text: &str) -> Option<DisplayInfo> {
    let mut connected = Vec::new();
    for line in text.lines().filter(|line| line.contains(" connected ")) {
        let mut parts = line.split_whitespace();
        let name = parts.next()?.to_string();
        let primary = line.contains(" connected primary ");
        let Some(dimensions) = parts.find_map(parse_geometry) else {
            continue;
        };
        connected.push((primary, name, dimensions));
    }
    let (_, name, (width, height)) = connected
        .into_iter()
        .max_by_key(|(primary, _, (w, h))| (*primary, w * h))?;
    Some(DisplayInfo {
        name,
        width,
        height,
        source: "xrandr",
    })
}

fn parse_geometry(token: &str) -> Option<(u32, u32)> {
    let dimensions = token.split('+').next()?;
    let (width, height) = dimensions.split_once('x')?;
    Some((width.parse().ok()?, height.parse().ok()?))
}

fn detect_drm() -> Option<DisplayInfo> {
    let drm = Path::new("/sys/class/drm");
    let mut displays = Vec::new();
    for entry in fs::read_dir(drm).ok()?.flatten() {
        let path = entry.path();
        let Ok(status) = fs::read_to_string(path.join("status")) else {
            continue;
        };
        if status.trim() != "connected" {
            continue;
        }
        let Ok(modes) = fs::read_to_string(path.join("modes")) else {
            continue;
        };
        let mode = modes.lines().find_map(parse_geometry);
        if let Some((width, height)) = mode {
            displays.push(DisplayInfo {
                name: entry.file_name().to_string_lossy().into_owned(),
                width,
                height,
                source: "DRM",
            });
        }
    }
    displays.into_iter().max_by_key(|d| d.width * d.height)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_primary_xrandr_monitor() {
        let text = "DP-1 connected 2560x1440+1920+0\neDP-1 connected primary 1920x1080+0+0";
        let display = parse_xrandr(text).unwrap();
        assert_eq!(display.name, "eDP-1");
        assert_eq!((display.width, display.height), (1920, 1080));
    }

    #[test]
    fn parses_geometry_without_accepting_mode_rates() {
        assert_eq!(parse_geometry("3440x1440+0+0"), Some((3440, 1440)));
        assert_eq!(parse_geometry("143.88*+"), None);
    }
}
