//! 複数の課題管理システム(RS-Chiketto=RedmineのRust版・標準Redmine・
//! GitLab Issues・GitHub Issues、将来的にRS-Git/Gitea/Gitbucket/Bitbucket
//! のIssue APIが実装され次第それらも)を定期的にポーリングし、新規/未対応の
//! チケットを検出したら、Claude/Cursor/ChatGPT/Gemini/DeepSeek/Grok等
//! 向けのタスク依頼プロンプトを自動生成してキューに積む機能。
//!
//! さらに、各タスクに期待スケジュール(見積時間)を設定でき、対応完了時に
//! 「スケジュール通りだったか」「何%完成したか」をAI(または人間)が
//! 自己申告する形で自動記録・報告する完了報告機能を持つ。
//!
//! **設計意図(ユーザー確認済み、2026-07-22)**: これは「third-partyの
//! チャットUIを外部APIで強制的に呼び出す」という方式の自動化ではない。
//! 狙いは、**各課題管理システムのチケット情報を、Claude Code Desktop等の
//! AIツールが実際に読み込む形(CLAUDE.md/.cursorrules/AGENTS.md相当の
//! コンテキスト)へ変換して提示すること**——これにより、各AIツールは
//! 自分自身のセッション内で課題管理システムの状況を丸ごと把握でき、
//! 人間が個別にチケットの内容をコピー&ペーストして説明する手間を
//! 省いた上で、対応に着手できる。つまり「チケット→AIツールへの状況
//! 把握・引き継ぎ」という部分を自動化することで、実質的に「チケットが
//! 来たら自動対応する」という体験を再現している。完了報告についても
//! 同様に、「AIエージェントが実際に自己申告する」ことを外部から強制する
//! ことはできない——このAPIは、対応したAIツール(またはそれを操作する
//! 人間)が完了時に呼び出すことで初めて記録される、という設計。

use crate::model::{AiTarget, CompletionReport, NormalizedTicket, PendingTask};
use crate::provider::{RedmineProvider, RsChikettoProvider};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct QueueFile {
    /// 既にプロンプト生成済みの(source, ticket_id)組(重複生成防止)。
    /// 複数システムを1つのキューへ集約するため、数値IDだけでは
    /// システムをまたいで衝突しうる。
    #[serde(default)]
    seen: HashSet<(String, u64)>,
    tasks: Vec<PendingTask>,
    #[serde(default)]
    reports: Vec<CompletionReport>,
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
    /// `source`はどの課題管理システムから取得したかのラベル(例:
    /// "rs-chiketto"/"redmine"/"gitlab"/"github")。返り値は今回新規に
    /// 追加したチケット数。
    pub fn ingest_new_tickets(&self, source: &str, tickets: &[NormalizedTicket], tracker_url: &str) -> usize {
        let mut state = self.state.lock().unwrap();
        let mut added = 0;
        let now = now_unix();
        for ticket in tickets {
            if ticket.status.eq_ignore_ascii_case("closed") {
                continue;
            }
            let key = (source.to_string(), ticket.id);
            if state.seen.contains(&key) {
                continue;
            }
            state.seen.insert(key);
            for target in AiTarget::all() {
                let prompt = crate::templates::generate_ticket_task_prompt(
                    target,
                    ticket.id,
                    &format!("[{source}] {}", ticket.title),
                    &ticket.description,
                    tracker_url,
                );
                state.tasks.push(PendingTask {
                    source: source.to_string(),
                    ticket_id: ticket.id,
                    target,
                    prompt,
                    handled: false,
                    created_at: now,
                    estimated_hours: None,
                });
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

    /// 指定チケット(全ターゲット共通)へ期待所要時間(見積、時間単位)を設定する。
    pub fn set_estimate(&self, source: &str, ticket_id: u64, hours: f64) -> usize {
        let mut state = self.state.lock().unwrap();
        let mut updated = 0;
        for task in state.tasks.iter_mut() {
            if task.source == source && task.ticket_id == ticket_id {
                task.estimated_hours = Some(hours);
                updated += 1;
            }
        }
        if updated > 0 {
            self.save(&state);
        }
        updated
    }

    /// 対応完了をマークし、完了報告(スケジュール遵守・完成度)を記録する。
    /// `completion_percent`は呼び出し側(AIツールまたは人間)の自己申告値。
    pub fn mark_handled_with_report(
        &self,
        source: &str,
        ticket_id: u64,
        target: AiTarget,
        completion_percent: u8,
        notes: String,
    ) -> Option<CompletionReport> {
        let mut state = self.state.lock().unwrap();
        let mut found_task: Option<PendingTask> = None;
        for task in state.tasks.iter_mut() {
            if task.source == source && task.ticket_id == ticket_id && task.target == target {
                task.handled = true;
                found_task = Some(task.clone());
            }
        }
        let task = found_task?;
        let now = now_unix();
        let actual_hours = ((now - task.created_at).max(0) as f64) / 3600.0;
        let on_schedule = task.estimated_hours.map(|est| actual_hours <= est);
        let report = CompletionReport {
            source: source.to_string(),
            ticket_id,
            target,
            completion_percent: completion_percent.min(100),
            on_schedule,
            estimated_hours: task.estimated_hours,
            actual_hours,
            notes,
            reported_at: now,
        };
        state.reports.push(report.clone());
        self.save(&state);
        Some(report)
    }

    pub fn completion_reports(&self) -> Vec<CompletionReport> {
        let state = self.state.lock().unwrap();
        state.reports.clone()
    }
}

/// バックグラウンドで定期的に複数の課題管理システムをポーリングし、
/// キューへ登録する。`rs_chiketto_url`は必須(このエコシステムの主要な
/// 課題管理先)、`redmine_url`は任意(標準Redmineインスタンスを別途
/// 監視したい場合、`RS_OPS_REDMINE_URL`環境変数で設定)。
pub async fn run_polling_loop(
    queue: std::sync::Arc<TicketQueue>,
    rs_chiketto_url: String,
    redmine_url: Option<String>,
    interval_secs: u64,
) {
    let rs_chiketto = RsChikettoProvider::new(rs_chiketto_url.clone());
    let redmine = redmine_url.clone().map(RedmineProvider::new);
    let mut ticker = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
    loop {
        ticker.tick().await;
        match rs_chiketto.list_tickets().await {
            Ok(tickets) => {
                let added = queue.ingest_new_tickets("rs-chiketto", &tickets, &rs_chiketto_url);
                if added > 0 {
                    tracing::info!(added, source = "rs-chiketto", "ingested new tickets into automation queue");
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, source = "rs-chiketto", "ticket automation poll failed (will retry next interval)");
            }
        }
        if let (Some(redmine), Some(url)) = (&redmine, &redmine_url) {
            match redmine.list_issues().await {
                Ok(tickets) => {
                    let added = queue.ingest_new_tickets("redmine", &tickets, url);
                    if added > 0 {
                        tracing::info!(added, source = "redmine", "ingested new tickets into automation queue");
                    }
                }
                Err(e) => {
                    tracing::warn!(error = %e, source = "redmine", "ticket automation poll failed (will retry next interval)");
                }
            }
        }
    }
}

/// 手動での即時ポーリング(`POST /api/tickets/poll-now`から呼ばれる)。
/// GitLab/GitHubは「Issues」ではなく現状「リポジトリ一覧」のみの実装
/// (`provider.rs`参照)のため、ここでは既存のチケット取得元
/// (RS-Chiketto・標準Redmine)のみを対象とする——GitLab Issues API/
/// GitHub Issues APIの追加実装は今後の課題として正直に明記する。
pub async fn poll_once(
    queue: &TicketQueue,
    rs_chiketto_url: &str,
    redmine_url: Option<&str>,
) -> (usize, usize) {
    let rs_chiketto = RsChikettoProvider::new(rs_chiketto_url.to_string());
    let mut fetched = 0;
    let mut added = 0;
    if let Ok(tickets) = rs_chiketto.list_tickets().await {
        fetched += tickets.len();
        added += queue.ingest_new_tickets("rs-chiketto", &tickets, rs_chiketto_url);
    }
    if let Some(url) = redmine_url {
        let redmine = RedmineProvider::new(url.to_string());
        if let Ok(tickets) = redmine.list_issues().await {
            fetched += tickets.len();
            added += queue.ingest_new_tickets("redmine", &tickets, url);
        }
    }
    (fetched, added)
}

pub fn default_queue_path(data_dir: &Path) -> PathBuf {
    data_dir.join("ticket-queue.json")
}

// 正直な開示: GitLabProvider/GitHubProviderは現状リポジトリ一覧取得のみの
// 実装で、Issues APIはまだこのポーリングに組み込んでいない(provider.rs
// 参照)。将来的にGitLab Issues API(`/api/v4/projects/:id/issues`)・
// GitHub Issues API(`/repos/:owner/:repo/issues`)を追加する際は、
// poll_once/run_polling_loopに同様のパターンで組み込む。

#[cfg(test)]
mod tests {
    use super::*;

    fn ticket(id: u64, status: &str) -> NormalizedTicket {
        NormalizedTicket { id, title: format!("Ticket {id}"), description: "desc".to_string(), status: status.to_string(), source: String::new() }
    }

    #[test]
    fn ingests_new_open_tickets_and_generates_prompts_for_all_targets() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        let added = queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Open")], "https://example.test");
        assert_eq!(added, 1);
        assert_eq!(queue.pending_for(AiTarget::Claude).len(), 1);
        assert_eq!(queue.pending_for(AiTarget::Cursor).len(), 1);
        assert_eq!(queue.pending_for(AiTarget::Generic).len(), 1);
    }

    #[test]
    fn does_not_reingest_already_seen_ticket() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Open")], "https://example.test");
        let added_again = queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Open")], "https://example.test");
        assert_eq!(added_again, 0);
        assert_eq!(queue.pending_for(AiTarget::Claude).len(), 1);
    }

    #[test]
    fn same_ticket_id_from_different_sources_are_distinct() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Open")], "https://a.test");
        let added = queue.ingest_new_tickets("redmine", &[ticket(1, "Open")], "https://b.test");
        assert_eq!(added, 1, "same numeric id from a different source must not be treated as a duplicate");
    }

    #[test]
    fn skips_closed_tickets() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        let added = queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Closed")], "https://example.test");
        assert_eq!(added, 0);
        assert!(queue.pending_for(AiTarget::Claude).is_empty());
    }

    #[test]
    fn mark_handled_removes_from_pending() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Open")], "https://example.test");
        let report = queue.mark_handled_with_report("rs-chiketto", 1, AiTarget::Claude, 100, "done".to_string());
        assert!(report.is_some());
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
            queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Open")], "https://example.test");
        }
        let reloaded = TicketQueue::load(&path);
        assert_eq!(reloaded.pending_for(AiTarget::Claude).len(), 1);
    }

    #[test]
    fn on_schedule_true_when_completed_within_estimate() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Open")], "https://example.test");
        queue.set_estimate("rs-chiketto", 1, 24.0);
        let report = queue.mark_handled_with_report("rs-chiketto", 1, AiTarget::Claude, 100, "done".to_string()).unwrap();
        // 実経過時間はテスト実行内でほぼ0時間なので、見積24時間以内=on_schedule=true
        assert_eq!(report.on_schedule, Some(true));
        assert_eq!(report.completion_percent, 100);
        assert_eq!(report.estimated_hours, Some(24.0));
    }

    #[test]
    fn on_schedule_is_none_without_estimate() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Open")], "https://example.test");
        let report = queue.mark_handled_with_report("rs-chiketto", 1, AiTarget::Claude, 50, "partial".to_string()).unwrap();
        assert_eq!(report.on_schedule, None);
        assert_eq!(report.completion_percent, 50);
    }

    #[test]
    fn completion_percent_is_clamped_to_100() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Open")], "https://example.test");
        let report = queue.mark_handled_with_report("rs-chiketto", 1, AiTarget::Claude, 150, "over".to_string()).unwrap();
        assert_eq!(report.completion_percent, 100);
    }

    #[test]
    fn completion_reports_lists_all_reports() {
        let dir = tempfile::tempdir().unwrap();
        let queue = TicketQueue::load(dir.path().join("queue.json"));
        queue.ingest_new_tickets("rs-chiketto", &[ticket(1, "Open"), ticket(2, "Open")], "https://example.test");
        queue.mark_handled_with_report("rs-chiketto", 1, AiTarget::Claude, 100, "a".to_string());
        queue.mark_handled_with_report("rs-chiketto", 2, AiTarget::Cursor, 80, "b".to_string());
        assert_eq!(queue.completion_reports().len(), 2);
    }
}
