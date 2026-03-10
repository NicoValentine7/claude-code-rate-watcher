use notify::{Event, EventKind, RecursiveMode, Watcher};
use std::sync::mpsc::Sender;

pub enum WatcherMessage {
    FileChanged,
}

pub fn start_watcher(sender: Sender<WatcherMessage>) -> notify::Result<impl Watcher> {
    let projects_dir = match dirs::home_dir() {
        Some(home) => home.join(".claude").join("projects"),
        None => return Err(notify::Error::generic("Could not find home directory")),
    };

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

    watcher.watch(&projects_dir, RecursiveMode::Recursive)?;
    Ok(watcher)
}
