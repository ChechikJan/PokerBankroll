// Prevents an extra console window from appearing on Windows.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;

/// Everything the app persists, keyed exactly like the browser build:
/// "tournaments", "rates", "rooms", "seedVersion".
type State = BTreeMap<String, Value>;

struct Store {
    dir: PathBuf,
    lock: Mutex<()>,
}

impl Store {
    fn data_file(&self) -> PathBuf {
        self.dir.join("poker_data.json")
    }

    fn backup_file(&self) -> PathBuf {
        self.dir.join("poker_data.backup.json")
    }

    fn load(&self) -> State {
        for path in [self.data_file(), self.backup_file()] {
            if let Ok(text) = fs::read_to_string(&path) {
                if let Ok(parsed) = serde_json::from_str::<State>(&text) {
                    return parsed;
                }
            }
        }
        State::new()
    }

    /// Write to a temp file first, keep the previous version as a backup,
    /// then swap it in. A crash mid-save can never leave a half-written base.
    fn save(&self, state: &State) -> Result<(), String> {
        let text = serde_json::to_string(state).map_err(|e| e.to_string())?;
        let tmp = self.dir.join("poker_data.tmp");

        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        fs::write(&tmp, text).map_err(|e| e.to_string())?;

        let main = self.data_file();
        if main.exists() {
            let _ = fs::copy(&main, self.backup_file());
        }
        fs::rename(&tmp, &main).map_err(|e| e.to_string())?;
        Ok(())
    }
}

/// Prefer sitting next to the executable (portable: copy the folder, keep your
/// history). If that folder is read-only, fall back to the user's app data.
fn pick_data_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let probe = dir.join(".write_test");
            if fs::write(&probe, b"1").is_ok() {
                let _ = fs::remove_file(&probe);
                return dir.to_path_buf();
            }
        }
    }

    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .or_else(|_| std::env::var("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|_| PathBuf::from("."));

    let dir = base.join("PokerTracker");
    let _ = fs::create_dir_all(&dir);
    dir
}

#[tauri::command]
fn load_state(store: tauri::State<Store>) -> State {
    let _guard = store.lock.lock().unwrap();
    store.load()
}

#[tauri::command]
fn save_key(store: tauri::State<Store>, key: String, value: Value) -> Result<(), String> {
    let _guard = store.lock.lock().unwrap();
    let mut state = store.load();
    state.insert(key, value);
    store.save(&state)
}

#[tauri::command]
fn data_path(store: tauri::State<Store>) -> String {
    store.data_file().to_string_lossy().to_string()
}

fn main() {
    let store = Store {
        dir: pick_data_dir(),
        lock: Mutex::new(()),
    };

    tauri::Builder::default()
        .manage(store)
        .invoke_handler(tauri::generate_handler![load_state, save_key, data_path])
        .run(tauri::generate_context!())
        .expect("error while running Poker Tracker");
}
