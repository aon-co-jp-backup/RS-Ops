//! リポジトリ/URL(プロジェクトの`path`または`github_url`をキーとして
//! 使う)に対する優先度(1〜5の星評価)の永続ストア。
//!
//! 星1 = 「今は開発などをしなくて良い」という意味(ユーザー定義、
//! 2026-07-22指示)。スキャン結果(`scanner::scan_projects`)自体は
//! 都度再計算される非永続データなので、優先度だけを別ファイルに
//! 永続化し、APIレスポンス組み立て時にキーで突き合わせて合成する。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Default, Serialize, Deserialize)]
struct PriorityFile {
    ratings: HashMap<String, u8>,
}

pub struct PriorityStore {
    path: PathBuf,
    state: Mutex<PriorityFile>,
}

impl PriorityStore {
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let state = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { path, state: Mutex::new(state) }
    }

    fn save(&self, state: &PriorityFile) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(state) {
            let _ = std::fs::write(&self.path, bytes);
        }
    }

    /// `key`(プロジェクトの`path`または`github_url`)に星評価(1〜5)を設定する。
    pub fn set(&self, key: &str, stars: u8) -> Result<(), String> {
        if !(1..=5).contains(&stars) {
            return Err("priority must be between 1 and 5".to_string());
        }
        let mut state = self.state.lock().unwrap();
        state.ratings.insert(key.to_string(), stars);
        self.save(&state);
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<u8> {
        self.state.lock().unwrap().ratings.get(key).copied()
    }

    pub fn all(&self) -> HashMap<String, u8> {
        self.state.lock().unwrap().ratings.clone()
    }
}

pub fn default_priority_path(data_dir: &Path) -> PathBuf {
    data_dir.join("priorities.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_get_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = PriorityStore::load(dir.path().join("p.json"));
        store.set("F:\\runo\\aruaru-llm", 3).unwrap();
        assert_eq!(store.get("F:\\runo\\aruaru-llm"), Some(3));
        assert_eq!(store.get("unknown"), None);
    }

    #[test]
    fn rejects_out_of_range_priority() {
        let dir = tempfile::tempdir().unwrap();
        let store = PriorityStore::load(dir.path().join("p.json"));
        assert!(store.set("x", 0).is_err());
        assert!(store.set("x", 6).is_err());
        assert!(store.get("x").is_none());
    }

    #[test]
    fn persists_across_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("p.json");
        {
            let store = PriorityStore::load(&path);
            store.set("k", 1).unwrap();
        }
        let reloaded = PriorityStore::load(&path);
        assert_eq!(reloaded.get("k"), Some(1));
    }

    #[test]
    fn all_returns_every_rating() {
        let dir = tempfile::tempdir().unwrap();
        let store = PriorityStore::load(dir.path().join("p.json"));
        store.set("a", 5).unwrap();
        store.set("b", 1).unwrap();
        let all = store.all();
        assert_eq!(all.get("a"), Some(&5));
        assert_eq!(all.get("b"), Some(&1));
    }
}
