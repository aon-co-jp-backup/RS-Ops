//! チーム運用向けの権限管理。管理者(`RS_OPS_ADMIN_EMAIL`)は常にフル権限を
//! 持つ固定値で、`AccountStore`には登録しない(RS-Gitの`RGIT_ADMIN_EMAIL`と
//! 同じ設計方針)。それ以外のメールアドレスは、管理者が明示的に登録し
//! `Role`を割り当てるまでログインしても何の権限も持たない
//! (`ticket_automation::TicketQueue`/`priority::PriorityStore`と同じ
//! JSON永続化パターン)。
//!
//! ロール設計は3段階のみ(RS-Gitのリポジトリ単位のような細かいアクセス
//! 制御は持たない、RS-Opsはツール全体に対する権限で十分と判断):
//! - `Admin`: 環境変数で固定、アカウント管理・全操作が可能。
//! - `Editor`: 優先度設定・チケット対応済みマークなど、書き込み操作が可能。
//! - `Viewer`: 閲覧のみ(ログインは要求するが書き込みAPIは401)。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Viewer,
    Editor,
    Admin,
}

impl Role {
    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "viewer" => Some(Role::Viewer),
            "editor" => Some(Role::Editor),
            "admin" => Some(Role::Admin),
            _ => None,
        }
    }

    pub fn can_edit(self) -> bool {
        self >= Role::Editor
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AccountsFile {
    /// 管理者以外の登録メンバー。管理者自身はここには入れない。
    members: HashMap<String, Role>,
}

pub struct AccountStore {
    path: PathBuf,
    admin_email: String,
    state: Mutex<AccountsFile>,
}

impl AccountStore {
    pub fn load(path: impl Into<PathBuf>, admin_email: impl Into<String>) -> Self {
        let path = path.into();
        let state = std::fs::read(&path).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        Self { path, admin_email: admin_email.into(), state: Mutex::new(state) }
    }

    fn save(&self, state: &AccountsFile) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(state) {
            let _ = std::fs::write(&self.path, bytes);
        }
    }

    /// `email`が持つロールを返す。管理者メールは常に`Role::Admin`。
    /// 未登録メンバーは`None`(=権限無し、ログインはできても操作不可)。
    pub fn role_of(&self, email: &str) -> Option<Role> {
        if email == self.admin_email {
            return Some(Role::Admin);
        }
        self.state.lock().unwrap().members.get(email).copied()
    }

    /// 管理者のみが呼べる操作(呼び出し側で権限チェック済みを想定)。
    /// 管理者自身のメールを登録しようとした場合は無視する(固定値のため)。
    pub fn set_role(&self, email: &str, role: Role) {
        if email == self.admin_email {
            return;
        }
        let mut state = self.state.lock().unwrap();
        state.members.insert(email.to_string(), role);
        self.save(&state);
    }

    pub fn remove(&self, email: &str) -> bool {
        let mut state = self.state.lock().unwrap();
        let removed = state.members.remove(email).is_some();
        if removed {
            self.save(&state);
        }
        removed
    }

    /// 管理者+登録メンバー全員を`(email, role)`で返す(UI表示用)。
    pub fn list(&self) -> Vec<(String, Role)> {
        let state = self.state.lock().unwrap();
        let mut out: Vec<(String, Role)> = vec![(self.admin_email.clone(), Role::Admin)];
        out.extend(state.members.iter().map(|(e, r)| (e.clone(), *r)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_email_always_has_admin_role() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::load(dir.path().join("accounts.json"), "admin@example.com");
        assert_eq!(store.role_of("admin@example.com"), Some(Role::Admin));
    }

    #[test]
    fn unregistered_member_has_no_role() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::load(dir.path().join("accounts.json"), "admin@example.com");
        assert_eq!(store.role_of("stranger@example.com"), None);
    }

    #[test]
    fn admin_can_register_and_update_member_role() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::load(dir.path().join("accounts.json"), "admin@example.com");
        store.set_role("member@example.com", Role::Viewer);
        assert_eq!(store.role_of("member@example.com"), Some(Role::Viewer));
        store.set_role("member@example.com", Role::Editor);
        assert_eq!(store.role_of("member@example.com"), Some(Role::Editor));
    }

    #[test]
    fn cannot_override_the_fixed_admin_account() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::load(dir.path().join("accounts.json"), "admin@example.com");
        store.set_role("admin@example.com", Role::Viewer);
        assert_eq!(store.role_of("admin@example.com"), Some(Role::Admin));
    }

    #[test]
    fn remove_revokes_membership() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::load(dir.path().join("accounts.json"), "admin@example.com");
        store.set_role("member@example.com", Role::Editor);
        assert!(store.remove("member@example.com"));
        assert_eq!(store.role_of("member@example.com"), None);
        assert!(!store.remove("member@example.com"));
    }

    #[test]
    fn editor_and_admin_can_edit_viewer_cannot() {
        assert!(Role::Admin.can_edit());
        assert!(Role::Editor.can_edit());
        assert!(!Role::Viewer.can_edit());
    }

    #[test]
    fn persists_across_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("accounts.json");
        {
            let store = AccountStore::load(&path, "admin@example.com");
            store.set_role("member@example.com", Role::Editor);
        }
        let reloaded = AccountStore::load(&path, "admin@example.com");
        assert_eq!(reloaded.role_of("member@example.com"), Some(Role::Editor));
    }

    #[test]
    fn list_includes_admin_and_all_members() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::load(dir.path().join("accounts.json"), "admin@example.com");
        store.set_role("a@example.com", Role::Viewer);
        store.set_role("b@example.com", Role::Editor);
        let list = store.list();
        assert_eq!(list.len(), 3);
        assert!(list.contains(&("admin@example.com".to_string(), Role::Admin)));
    }
}
