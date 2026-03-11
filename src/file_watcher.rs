use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::sync::mpsc::Sender;

pub enum WatcherMessage {
    FileChanged,
}

pub fn start_watcher(sender: Sender<WatcherMessage>) -> notify::Result<impl Watcher> {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return Err(notify::Error::generic("Could not find home directory")),
    };

    let claude_dir = home.join(".claude").join("projects");
    let codex_dir = home.join(".codex").join("archived_sessions");

    let mut watcher =
        notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                match event.kind {
                    EventKind::Modify(_) | EventKind::Create(_) => {
                        for path in event.paths {
                            if path.extension().is_some_and(|e| e == "jsonl") {
                                let _ = sender.send(WatcherMessage::FileChanged);
                            }
                        }
                    }
                    _ => {}
                }
            }
        })?;

    watcher.watch(&claude_dir, RecursiveMode::Recursive)?;

    // Watch Codex directory if it exists
    if codex_dir.exists() {
        let _ = watcher.watch(&codex_dir, RecursiveMode::NonRecursive);
    }

    Ok(watcher)
}
