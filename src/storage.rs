use crate::model::{DownloadStatus, PersistedState};
use anyhow::{Context, Result};
use directories::ProjectDirs;
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn data_dir() -> PathBuf {
    ProjectDirs::from("dev", "youtuibe", "youtuibe")
        .map(|p| p.data_local_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".youtuibe"))
}

pub fn state_path() -> PathBuf {
    data_dir().join("state.json")
}

pub fn load(path: &Path) -> Result<PersistedState> {
    if !path.exists() {
        // Import the previous application state once, leaving the original intact.
        if path == state_path()
            && let Some(legacy) = ProjectDirs::from("dev", "tide", "tide-dlp")
                .map(|dirs| dirs.data_local_dir().join("state.json"))
            && legacy.exists()
        {
            let state = load(&legacy)?;
            save_atomic(path, &state)?;
            return Ok(state);
        }
        return Ok(PersistedState::default());
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut state: PersistedState = serde_json::from_slice(&bytes).context("parse saved state")?;
    // A process cannot still be alive after a clean application restart. Requeue it;
    // yt-dlp's .part files provide byte-level continuation.
    for job in &mut state.jobs {
        if matches!(
            job.status,
            DownloadStatus::Downloading
                | DownloadStatus::Processing
                | DownloadStatus::Inspecting
                | DownloadStatus::Paused
        ) {
            job.status = DownloadStatus::Queued;
            job.error = "Recovered after an interrupted session".into();
        }
    }
    Ok(state)
}

pub fn save_atomic(path: &Path, state: &PersistedState) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(state)?;
    fs::write(&tmp, bytes).with_context(|| format!("write {}", tmp.display()))?;
    fs::rename(&tmp, path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DownloadOptions, Job};

    #[test]
    fn atomic_round_trip_and_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");
        let mut state = PersistedState::default();
        let mut job = Job::new(
            "https://example.com/v".into(),
            "Best".into(),
            DownloadOptions::default(),
        );
        job.status = DownloadStatus::Downloading;
        state.jobs.push(job);
        save_atomic(&path, &state).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.jobs[0].status, DownloadStatus::Queued);
    }
}
