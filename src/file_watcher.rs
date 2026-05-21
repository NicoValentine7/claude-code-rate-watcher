use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;

pub enum WatcherMessage {
    FileChanged,
    StatusLineUpdate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WatchTargetKind {
    ClaudeProjects,
    CodexSessions,
    CodexArchivedSessions,
    StatuslineParent,
}

#[derive(Debug)]
struct WatchTarget {
    #[allow(dead_code)]
    kind: WatchTargetKind,
    path: PathBuf,
    mode: RecursiveMode,
    required: bool,
}

pub fn start_watcher(sender: Sender<WatcherMessage>) -> notify::Result<impl Watcher> {
    let home_dir = match dirs::home_dir() {
        Some(home) => home,
        None => return Err(notify::Error::generic("Could not find home directory")),
    };

    let statusline_data = crate::statusline::rate_data_path();
    let targets = watch_targets(&home_dir, statusline_data.clone());

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Modify(_) | EventKind::Create(_) => {
                    for path in &event.paths {
                        // Check if this is the statusline data file
                        if let Some(ref sl_path) = statusline_data {
                            if path == sl_path {
                                let _ = sender.send(WatcherMessage::StatusLineUpdate);
                                return;
                            }
                        }
                        // Check for .jsonl session files
                        if path.extension().is_some_and(|e| e == "jsonl") {
                            let _ = sender.send(WatcherMessage::FileChanged);
                        }
                    }
                }
                _ => {}
            }
        }
    })?;

    let mut watched_any = false;

    for target in targets {
        match watcher.watch(&target.path, target.mode) {
            Ok(()) => watched_any = true,
            Err(err) if target.required => return Err(err),
            Err(_) => {}
        }
    }

    if !watched_any {
        return Err(notify::Error::generic(
            "Could not find Claude or Codex session directories",
        ));
    }

    Ok(watcher)
}

fn watch_targets(home_dir: &Path, statusline_data: Option<PathBuf>) -> Vec<WatchTarget> {
    let mut targets = Vec::new();
    let projects_dir = home_dir.join(".claude").join("projects");
    let codex_sessions_dir = home_dir.join(".codex").join("sessions");
    let codex_archived_sessions_dir = home_dir.join(".codex").join("archived_sessions");

    if projects_dir.exists() {
        targets.push(WatchTarget {
            kind: WatchTargetKind::ClaudeProjects,
            path: projects_dir,
            mode: RecursiveMode::Recursive,
            required: true,
        });
    }

    if codex_sessions_dir.exists() {
        targets.push(WatchTarget {
            kind: WatchTargetKind::CodexSessions,
            path: codex_sessions_dir,
            mode: RecursiveMode::Recursive,
            required: true,
        });
    }

    if codex_archived_sessions_dir.exists() {
        targets.push(WatchTarget {
            kind: WatchTargetKind::CodexArchivedSessions,
            path: codex_archived_sessions_dir,
            mode: RecursiveMode::NonRecursive,
            required: true,
        });
    }

    if let Some(sl_path) = statusline_data {
        if let Some(parent) = sl_path.parent() {
            if parent.exists() {
                targets.push(WatchTarget {
                    kind: WatchTargetKind::StatuslineParent,
                    path: parent.to_path_buf(),
                    mode: RecursiveMode::NonRecursive,
                    required: false,
                });
            }
        }
    }

    targets
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn watch_targets_include_all_existing_rate_sources() {
        let root = test_home("all-sources");
        let claude_projects = root.join(".claude").join("projects");
        let claude_status = root.join(".claude").join("ccrw-rate-data.json");
        let codex_sessions = root.join(".codex").join("sessions");
        let codex_archived_sessions = root.join(".codex").join("archived_sessions");
        std::fs::create_dir_all(&claude_projects).unwrap();
        std::fs::create_dir_all(&codex_sessions).unwrap();
        std::fs::create_dir_all(&codex_archived_sessions).unwrap();

        let targets = watch_targets(&root, Some(claude_status));

        assert_target(
            &targets,
            WatchTargetKind::ClaudeProjects,
            &claude_projects,
            RecursiveMode::Recursive,
            true,
        );
        assert_target(
            &targets,
            WatchTargetKind::CodexSessions,
            &codex_sessions,
            RecursiveMode::Recursive,
            true,
        );
        assert_target(
            &targets,
            WatchTargetKind::CodexArchivedSessions,
            &codex_archived_sessions,
            RecursiveMode::NonRecursive,
            true,
        );
        assert_target(
            &targets,
            WatchTargetKind::StatuslineParent,
            &root.join(".claude"),
            RecursiveMode::NonRecursive,
            false,
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn watch_targets_skip_missing_directories() {
        let root = test_home("missing-sources");
        std::fs::create_dir_all(root.join(".codex").join("sessions")).unwrap();

        let targets = watch_targets(&root, None);

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].kind, WatchTargetKind::CodexSessions);
        assert_eq!(targets[0].path, root.join(".codex").join("sessions"));
        assert!(matches!(targets[0].mode, RecursiveMode::Recursive));
        assert!(targets[0].required);

        let _ = std::fs::remove_dir_all(root);
    }

    fn assert_target(
        targets: &[WatchTarget],
        kind: WatchTargetKind,
        path: &Path,
        mode: RecursiveMode,
        required: bool,
    ) {
        let target = targets
            .iter()
            .find(|target| target.kind == kind)
            .expect("missing watch target");
        assert_eq!(target.path, path);
        assert_eq!(
            matches!(target.mode, RecursiveMode::Recursive),
            matches!(mode, RecursiveMode::Recursive)
        );
        assert_eq!(target.required, required);
    }

    fn test_home(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "ccrw-watch-targets-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        path
    }
}
