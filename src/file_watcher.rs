use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::sync::mpsc::Sender;

pub enum WatcherMessage {
    FileChanged,
    StatusLineUpdate,
}

pub fn start_watcher(sender: Sender<WatcherMessage>) -> notify::Result<impl Watcher> {
    let projects_dir = match dirs::home_dir() {
        Some(home) => home.join(".claude").join("projects"),
        None => return Err(notify::Error::generic("Could not find home directory")),
    };

    let statusline_data = crate::statusline::rate_data_path();

    let mut watcher =
        notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
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

    watcher.watch(&projects_dir, RecursiveMode::Recursive)?;

    // Also watch the statusline data file's parent directory
    if let Some(sl_path) = crate::statusline::rate_data_path() {
        if let Some(parent) = sl_path.parent() {
            if parent.exists() {
                // Use non-recursive watch on ~/.claude/ for the data file
                let _ = watcher.watch(parent, RecursiveMode::NonRecursive);
            }
        }
    }

    Ok(watcher)
}
