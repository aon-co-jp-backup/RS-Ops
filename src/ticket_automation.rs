//! 課題管理システム(RS-Chiketto/将来的に標準Redmine)を定期的にポーリングし、
//! 新規/未対応のチケットを検出したら、Claude/Cursor/汎用(ChatGPT/Gemini/
//! DeepSeek/Grok)向けのタスク依頼プロンプトを自動生成してキューに積む機能。
//!
//! **設計意図(ユーザー確認済み、2026-07-22)**: これは「third-partyの
//! チャットUIを外部APIで強制的に呼び出す」という方式の自動化ではない。
//! 狙いは、**RS-Chiketto/Redmineのチケット情報を、Claude Code Desktop等の
//! AIツールが実際に読み込む形(CLAUDE.md/.cursorrules/AGENTS.md相当の
//! コンテキスト)へ変換して提示すること**——これにより、各AIツールは
//! 自分自身のセッション内で課題管理システムの状況を丸ごと把握でき、
//! 人間が個別にチケットの内容をコピー&ペーストして説明する手間を
//! 省いた上で、対応に着手できる。つまり「チケット→AIツールへの状況
//! 把握・引き継ぎ」という部分を自動化することで、実質的に「チケットが
//! 来たら自動対応する」という体験を再現している(AIツール本体を
//! こちらから強制起動するのではなく、AIツール側がこのコンテキストを
//! 参照することで全体を把握し、自律的に対応できるようにする、という
//! 設計)。

use crate::model::{AiTarget, PendingTask};
use crate::provider::RsChikettoProvider;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Default, Serialize, Deserialize)]
struct QueueFile {
    /// 既にプロンプト生成済みのチケットID(重複生成防止)。
    seen_ticket_ids: HashSet<u64>,
    tasks: Vec<PendingTask>,
}

pub struct TicketQueue {
    path: PathBuf,
    state: Mutex<QueueFile>,
}

impl TicketQueue {
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let state = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { path, state: Mutex::new(state) }
    }

    fn save(&self, state: &QueueFile) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec_pretty(state) {
            let _ = std::fs::write(&self.path, bytes);
        }
    }

    /// 新規チケット(未対応ステータスかつ未処理)を検出し、対象AIツール
    /// それぞれ向けのタスク依頼プロンプトを生成してキューへ追加する。
    /// 返り値は今回新規に追加したチケット数。
    pub fn ingest_new_tickets(&self, tickets: &[crate::model::NormalizedTicket], tracker_url: &str) -> usize {
        let mut state = self.state.lock().unwrap();
        let mut added = 0;
        for ticket in tickets {
            // "Closed" 系のチケットは対応不要とみなす(大文字小文字を問わず
            // 部分一致で判定——RS-Chikettoは"Closed"、標準Redmineの英語ロケール
            // でも"Closed"系が多いため)。
            if ticket.status.eq_ignore_ascii_case("closed") {
                continue;
            }
            if state.seen_ticket_ids.contains(&ticket.id) {
                continue;
            }
            state.seen_ticket_ids.insert(ticket.id);
            for target in [AiTarget::Claude, AiTarget::Cursor, AiTarget::Generic] {
                let prompt = crate::templates::generate_ticket_task_prompt(
                    target,
                    ticket.id,
                    &ticket.title,
                    &ticket.description,
                    tracker_url,
                );
                state.tasks.push(PendingTask { ticket_id: ticket.id, target, prompt, handled: false });
            }
            added += 1;
        }
        if added > 0 {
            self.save(&state);
        }
        added
    }

    pub fn pending_for(&self, target: AiTarget) -> Vec<PendingTask> {
        let state = self.state.lock().unwrap();
        state.tasks.iter().filter(|t| t.target == target && !t.handled).cloned().collect()
    }

    pub fn mark_handled(&self, ticket_id: u64, target: AiTarget) -> bool {
        let mut state = self.state.lock().unwrap();
        let mut found = false;
        for task in state.tasks.iter_mut() {
            if task.ticket_id == ticket_id && task.target == target {
                task.handled = true;
                found = true;
            }
        }
        if found {
            self.save(&state);
        }
        found
    }
}

/// バックグラウンドで定期的にRS-Chikettoをポーリングし、キューへ登録する。
pub async fn run_polling_loop(queue: std::sync::Arc<TicketQueue>, rs_chiketto_url: String, interval_secs: u64) {
    let provider = RsChikettoProvider::new(rs_chiketto_url.clone());
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
    loop {
        ticker.tick().await;
        match provider.list_tickets().await {
            Ok(tickets) => {
                let added = queue.ingest_new_tickets(&tickets, &format!("{rs_chiketto_url}"));
                if added > 0 {
                    tracing::info!(added, "ingested new tickets into automation queue");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "ticket automation poll failed (will retry next interval)");
            }
        }
    }
}

pub fn default_queue_path(data_dir: &Path) -> PathBuf {
    data_dir.join("ticket-queue.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NormalizedTicket;

    fn ticket(id: u64, status: &str) -> NormalizedTicket {
        NormalizedTicket { id, title: format!("Ticket {id}"), description: "desc".to_string(), status: status.to_string() }
    }

    #[test]
    fn ingests_new_open_tickets_and_generates_prompts_for_all_targets() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        let added = queue.ingest_new_tickets(&[ticket(1, "Open")], "https://example.test");
        assert_eq!(added, 1);
        assert_eq!(queue.pending_for(AiTarget::Claude).len(), 1);
        assert_eq!(queue.pending_for(AiTarget::Cursor).len(), 1);
        assert_eq!(queue.pending_for(AiTarget::Generic).len(), 1);
    }

    #[test]
    fn does_not_reingest_already_seen_ticket() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        queue.ingest_new_tickets(&[ticket(1, "Open")], "https://example.test");
        let added_again = queue.ingest_new_tickets(&[ticket(1, "Open")], "https://example.test");
        assert_eq!(added_again, 0);
        assert_eq!(queue.pending_for(AiTarget::Claude).len(), 1);
    }

    #[test]
    fn skips_closed_tickets() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        let added = queue.ingest_new_tickets(&[ticket(1, "Closed")], "https://example.test");
        assert_eq!(added, 0);
        assert!(queue.pending_for(AiTarget::Claude).is_empty());
    }

    #[test]
    fn mark_handled_removes_from_pending() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        queue.ingest_new_tickets(&[ticket(1, "Open")], "https://example.test");
        assert!(queue.mark_handled(1, AiTarget::Claude));
        let pending = queue.pending_for(AiTarget::Claude);
        assert!(pending.is_empty());
        // 他のtargetは引き続きpendingのまま
        assert_eq!(queue.pending_for(AiTarget::Cursor).len(), 1);
    }

    #[test]
    fn persists_across_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("queue.json");
        {
            let queue = TicketQueue::load(&path);
            queue.ingest_new_tickets(&[ticket(1, "Open")], "https://example.test");
        }
        let reloaded = TicketQueue::load(&path);
        assert_eq!(reloaded.pending_for(AiTarget::Claude).len(), 1);
    }
}
