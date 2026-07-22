use rs_ai_devops::model::{AiTarget, ProjectEntry};
use rs_ai_devops::provider::{all_service_statuses, GitHubProvider, RsChikettoProvider};
use rs_ai_devops::scanner;
use rs_ai_devops::templates::generate_template;
use rs_ai_devops::ticket_automation::{self, TicketQueue};

use poem::{get, handler, listener::TcpListener, post, web::Json, web::Path as PPath, web::Query, EndpointExt, Route, Server};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

struct AppConfig {
    scan_root: PathBuf,
    rs_chiketto_url: String,
    ticket_queue: Arc<TicketQueue>,
}

#[handler]
fn healthz() -> &'static str {
    "ok"
}

#[handler]
fn index() -> poem::web::Html<&'static str> {
    poem::web::Html(include_str!("../static/index.html"))
}

#[handler]
fn install_sh() -> poem::Response {
    poem::Response::builder()
        .header("content-type", "text/x-shellscript; charset=utf-8")
        .header("content-disposition", "attachment; filename=\"install.sh\"")
        .body(include_str!("../install.sh"))
}

#[handler]
fn install_ps1() -> poem::Response {
    poem::Response::builder()
        .header("content-type", "text/plain; charset=utf-8")
        .header("content-disposition", "attachment; filename=\"install.ps1\"")
        .body(include_str!("../install.ps1"))
}

#[handler]
async fn list_projects(cfg: poem::web::Data<&Arc<AppConfig>>) -> Json<Vec<ProjectEntry>> {
    let projects = scanner::scan_projects(&cfg.scan_root);
    Json(projects)
}

#[handler]
fn service_statuses() -> Json<serde_json::Value> {
    let statuses: Vec<_> = all_service_statuses()
        .into_iter()
        .map(|(svc, status)| serde_json::json!({"service": svc, "status": status}))
        .collect();
    Json(serde_json::json!({"git_services": statuses}))
}

#[handler]
async fn github_org_repos() -> Json<serde_json::Value> {
    let gh = GitHubProvider::new();
    match gh.list_org_repos("aon-co-jp").await {
        Ok(repos) => Json(serde_json::json!({"ok": true, "repos": repos})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

#[handler]
async fn rs_chiketto_health(cfg: poem::web::Data<&Arc<AppConfig>>) -> Json<serde_json::Value> {
    let provider = RsChikettoProvider::new(cfg.rs_chiketto_url.clone());
    match provider.health_check().await {
        Ok(healthy) => {
            let probe_status = provider
                .list_tickets_unauthenticated_probe()
                .await
                .ok();
            Json(serde_json::json!({
                "ok": true,
                "healthy": healthy,
                "unauthenticated_tickets_probe_status": probe_status,
            }))
        }
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

#[derive(Deserialize)]
struct GenerateQuery {
    target: String,
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    repo_url: String,
}

#[handler]
fn generate(Query(q): Query<GenerateQuery>) -> poem::Response {
    let target = match parse_target(&q.target) {
        Some(t) => t,
        None => {
            return poem::Response::builder()
                .status(poem::http::StatusCode::BAD_REQUEST)
                .body("unknown target (expected: claude|cursor|generic)");
        }
    };
    let body = generate_template(target, &q.name, &q.description, &q.repo_url);
    poem::Response::builder()
        .header("content-type", "text/plain; charset=utf-8")
        .body(body)
}

fn parse_target(s: &str) -> Option<AiTarget> {
    match s {
        "claude" => Some(AiTarget::Claude),
        "cursor" => Some(AiTarget::Cursor),
        "generic" => Some(AiTarget::Generic),
        _ => None,
    }
}

/// チケット自動対応機能: 「今すぐポーリング」— バックグラウンドの定期
/// ポーリングを待たず、手動でRS-Chikettoから最新のチケットを取得し
/// キューへ反映する。
#[handler]
async fn poll_tickets_now(cfg: poem::web::Data<&Arc<AppConfig>>) -> Json<serde_json::Value> {
    let provider = RsChikettoProvider::new(cfg.rs_chiketto_url.clone());
    match provider.list_tickets().await {
        Ok(tickets) => {
            let added = cfg.ticket_queue.ingest_new_tickets(&tickets, &cfg.rs_chiketto_url);
            Json(serde_json::json!({"ok": true, "fetched": tickets.len(), "newly_queued": added}))
        }
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

#[derive(Deserialize)]
struct PendingTasksQuery {
    target: String,
}

/// 対象AIツール向けに、未対応チケットから自動生成されたタスク依頼
/// プロンプトの一覧を返す。Claude Code/Cursor等はこれをファイル経由・
/// API経由で取得し、実際の対応に着手できる(このAPIが返すのは「タスクの
/// 提示」までであり、対象AIツールを強制的に起動させるものではない)。
#[handler]
fn pending_tasks(cfg: poem::web::Data<&Arc<AppConfig>>, Query(q): Query<PendingTasksQuery>) -> poem::Response {
    let target = match parse_target(&q.target) {
        Some(t) => t,
        None => {
            return poem::Response::builder()
                .status(poem::http::StatusCode::BAD_REQUEST)
                .body("unknown target (expected: claude|cursor|generic)".to_string());
        }
    };
    let tasks = cfg.ticket_queue.pending_for(target);
    poem::Response::builder()
        .header("content-type", "application/json")
        .body(serde_json::to_string(&tasks).unwrap_or_else(|_| "[]".to_string()))
}

#[handler]
fn mark_task_handled(cfg: poem::web::Data<&Arc<AppConfig>>, PPath((ticket_id, target)): PPath<(u64, String)>) -> Json<serde_json::Value> {
    let Some(target) = parse_target(&target) else {
        return Json(serde_json::json!({"ok": false, "error": "unknown target"}));
    };
    let found = cfg.ticket_queue.mark_handled(ticket_id, target);
    Json(serde_json::json!({"ok": found}))
}

fn build_app(cfg: Arc<AppConfig>) -> impl poem::Endpoint {
    Route::new()
        .at("/", get(index))
        .at("/healthz", get(healthz))
        .at("/install.sh", get(install_sh))
        .at("/install.ps1", get(install_ps1))
        .at("/api/projects", get(list_projects))
        .at("/api/services", get(service_statuses))
        .at("/api/github/repos", get(github_org_repos))
        .at("/api/rs-chiketto/health", get(rs_chiketto_health))
        .at("/api/generate", get(generate))
        .at("/api/tickets/poll-now", post(poll_tickets_now))
        .at("/api/tickets/pending-tasks", get(pending_tasks))
        .at("/api/tickets/:ticket_id/handled/:target", post(mark_task_handled))
        .data(cfg)
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();

    let scan_root = std::env::var("RS_AI_DEVOPS_SCAN_ROOT")
        .unwrap_or_else(|_| "F:\\runo".to_string());
    let rs_chiketto_url = std::env::var("RS_AI_DEVOPS_RS_CHIKETTO_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8100".to_string());
    let data_dir = std::env::var("RS_AI_DEVOPS_DATA_DIR")
        .unwrap_or_else(|_| "data".to_string());
    let poll_interval: u64 = std::env::var("RS_AI_DEVOPS_TICKET_POLL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    let queue_path = ticket_automation::default_queue_path(std::path::Path::new(&data_dir));
    let ticket_queue = Arc::new(TicketQueue::load(queue_path));

    let cfg = Arc::new(AppConfig {
        scan_root: PathBuf::from(scan_root),
        rs_chiketto_url: rs_chiketto_url.clone(),
        ticket_queue: ticket_queue.clone(),
    });

    tokio::spawn(ticket_automation::run_polling_loop(ticket_queue, rs_chiketto_url, poll_interval));

    let port: u16 = std::env::var("RS_AI_DEVOPS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8097);

    let app = build_app(cfg);
    tracing::info!("rs-ai-devops listening on 0.0.0.0:{port}");
    Server::new(TcpListener::bind(("0.0.0.0", port)))
        .run(app)
        .await
}
