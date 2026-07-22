use rs_ops::accounts::{AccountStore, Role};
use rs_ops::auth::{AuthStore, RequestOtpOutcome};
use rs_ops::mail::{self, SmtpConfig};
use rs_ops::model::{AiTarget, ProjectEntry};
use rs_ops::priority::{self, PriorityStore};
use rs_ops::provider::{
    all_service_statuses, AruaruLlmProvider, BitbucketProvider, GitHubProvider, GitLabProvider, GitbucketProvider, RedmineProvider,
    RsChikettoProvider, RsGitProvider,
};
use rs_ops::scanner;
use rs_ops::templates::generate_template;
use rs_ops::ticket_automation::{self, TicketQueue};

use poem::{get, handler, listener::TcpListener, post, web::Json, web::Path as PPath, web::Query, EndpointExt, Request, Route, Server};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

struct AppConfig {
    scan_root: PathBuf,
    rs_chiketto_url: String,
    redmine_url: Option<String>,
    /// RS-Git(このエコシステム独自のGitフォージ)インスタンスのURL。
    /// 2026-07-22、RS-Ops/RS-Git/aruaru-llmの「SET」完成の一環として追加。
    rs_git_url: String,
    /// aruaru-llm(契約不要の独自AI、open-cudaとSET構成)インスタンスのURL。
    aruaru_llm_url: String,
    ticket_queue: Arc<TicketQueue>,
    priority_store: Arc<PriorityStore>,
    auth: Arc<AuthStore>,
    accounts: Arc<AccountStore>,
    admin_email: String,
    smtp: Option<SmtpConfig>,
}

/// `Authorization: Bearer <token>`からログイン中のメールアドレスを
/// 特定する(RS-Gitの`session_identity`と同じ方式、gitプロトコルを
/// 持たないRS-OpsではBasic認証は不要なためBearerのみ対応)。
fn session_identity(req: &Request, cfg: &AppConfig) -> Option<String> {
    let header = req.header(poem::http::header::AUTHORIZATION)?;
    let token = header.strip_prefix("Bearer ")?;
    cfg.auth.session_email(token)
}

fn session_role(req: &Request, cfg: &AppConfig) -> Option<Role> {
    let email = session_identity(req, cfg)?;
    cfg.accounts.role_of(&email)
}

fn unauthorized(msg: &str) -> poem::Response {
    poem::Response::builder()
        .status(poem::http::StatusCode::UNAUTHORIZED)
        .header("content-type", "application/json")
        .body(serde_json::json!({"ok": false, "error": msg}).to_string())
}

fn forbidden(msg: &str) -> poem::Response {
    poem::Response::builder()
        .status(poem::http::StatusCode::FORBIDDEN)
        .header("content-type", "application/json")
        .body(serde_json::json!({"ok": false, "error": msg}).to_string())
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
    let mut projects = scanner::scan_projects(&cfg.scan_root);
    for p in projects.iter_mut() {
        p.priority = cfg
            .priority_store
            .get(&p.path)
            .or_else(|| p.github_url.as_deref().and_then(|u| cfg.priority_store.get(u)));
    }
    Json(projects)
}

#[derive(Deserialize)]
struct SetPriorityBody {
    /// プロジェクトの`path`または`github_url`(リポジトリ/URLどちらでも指定可能)。
    key: String,
    /// 星評価(1〜5)。1 = 今は開発などをしなくて良い。
    stars: u8,
}

/// リポジトリ/URLに対する優先度(星1〜5)を設定する。ログイン済みかつ
/// `Editor`以上のロールが必要(誰でも書き換えられる状態を防ぐ、
/// チーム運用の権限管理システムの一部)。
#[handler]
async fn set_priority(req: &Request, cfg: poem::web::Data<&Arc<AppConfig>>, Json(body): Json<SetPriorityBody>) -> poem::Response {
    match session_role(req, &cfg) {
        Some(role) if role.can_edit() => {}
        Some(_) => return forbidden("editor or admin role required"),
        None => return unauthorized("login required"),
    }
    match cfg.priority_store.set(&body.key, body.stars) {
        Ok(()) => poem::Response::builder()
            .header("content-type", "application/json")
            .body(serde_json::json!({"ok": true}).to_string()),
        Err(e) => poem::Response::builder()
            .status(poem::http::StatusCode::BAD_REQUEST)
            .header("content-type", "application/json")
            .body(serde_json::json!({"ok": false, "error": e}).to_string()),
    }
}

/// 現在設定されている全ての優先度(key→星評価)を返す。
#[handler]
fn list_priorities(cfg: poem::web::Data<&Arc<AppConfig>>) -> Json<std::collections::HashMap<String, u8>> {
    Json(cfg.priority_store.all())
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

#[derive(Deserialize)]
struct GitLabQuery {
    #[serde(default = "default_gitlab_base_url")]
    base_url: String,
    username: String,
}
fn default_gitlab_base_url() -> String {
    "https://gitlab.com".to_string()
}

#[handler]
async fn gitlab_repos(Query(q): Query<GitLabQuery>) -> Json<serde_json::Value> {
    let gl = GitLabProvider::new(q.base_url);
    match gl.list_user_projects(&q.username).await {
        Ok(repos) => Json(serde_json::json!({"ok": true, "repos": repos})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

#[derive(Deserialize)]
struct BitbucketQuery {
    workspace: String,
}

#[handler]
async fn bitbucket_repos(Query(q): Query<BitbucketQuery>) -> Json<serde_json::Value> {
    let bb = BitbucketProvider::new();
    match bb.list_workspace_repos(&q.workspace).await {
        Ok(repos) => Json(serde_json::json!({"ok": true, "repos": repos})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

#[derive(Deserialize)]
struct RedmineQuery {
    base_url: String,
}

#[handler]
async fn redmine_issues(Query(q): Query<RedmineQuery>) -> Json<serde_json::Value> {
    let rm = RedmineProvider::new(q.base_url);
    match rm.list_issues().await {
        Ok(issues) => Json(serde_json::json!({"ok": true, "issues": issues})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

#[derive(Deserialize)]
struct GitbucketQuery {
    base_url: String,
    owner: String,
}

#[handler]
async fn gitbucket_repos(Query(q): Query<GitbucketQuery>) -> Json<serde_json::Value> {
    let gb = GitbucketProvider::new(q.base_url);
    match gb.list_owner_repos(&q.owner).await {
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

/// RS-Git(このエコシステム独自のGitフォージ)の公開リポジトリ一覧。
/// `RS_OPS_RS_GIT_URL`で接続先を指定する(未設定時は既定でローカル
/// `http://127.0.0.1:8090`——RS-Git単体をローカルで起動して確認する
/// 際のデフォルトポートに合わせている)。
#[handler]
async fn rs_git_repos(cfg: poem::web::Data<&Arc<AppConfig>>) -> Json<serde_json::Value> {
    let provider = RsGitProvider::new(cfg.rs_git_url.clone());
    match provider.list_repos().await {
        Ok(repos) => Json(serde_json::json!({"ok": true, "repos": repos})),
        Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
    }
}

/// aruaru-llm(契約不要の独自AI、open-cudaとSET構成)のヘルスチェック
/// + 実際の応答エンジン名の取得(「本物の対話生成AIかどうか」を隠さず
/// 表示する、aruaru-llm自身の設計方針をそのままRS-Ops側でも反映する)。
#[handler]
async fn aruaru_llm_health(cfg: poem::web::Data<&Arc<AppConfig>>) -> Json<serde_json::Value> {
    let provider = AruaruLlmProvider::new(cfg.aruaru_llm_url.clone());
    match provider.health_check().await {
        Ok(healthy) => {
            let engine = provider.probe_engine().await.ok();
            Json(serde_json::json!({"ok": true, "healthy": healthy, "engine": engine}))
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
                .body("unknown target (expected: claude|claude_browser|cursor|chatgpt|gemini|deepseek|grok|generic)");
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
        "claude_browser" => Some(AiTarget::ClaudeBrowser),
        "cursor" => Some(AiTarget::Cursor),
        "chatgpt" => Some(AiTarget::ChatGpt),
        "gemini" => Some(AiTarget::Gemini),
        "deepseek" => Some(AiTarget::DeepSeek),
        "grok" => Some(AiTarget::Grok),
        "generic" => Some(AiTarget::Generic),
        _ => None,
    }
}

/// チケット自動対応機能: 「今すぐポーリング」— バックグラウンドの定期
/// ポーリングを待たず、手動で全ソース(RS-Chiketto・設定されていれば
/// 標準Redmine)から最新のチケットを取得しキューへ反映する。
#[handler]
async fn poll_tickets_now(cfg: poem::web::Data<&Arc<AppConfig>>) -> Json<serde_json::Value> {
    let (fetched, added) =
        ticket_automation::poll_once(&cfg.ticket_queue, &cfg.rs_chiketto_url, cfg.redmine_url.as_deref()).await;
    Json(serde_json::json!({"ok": true, "fetched": fetched, "newly_queued": added}))
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
                .body("unknown target (expected: claude|claude_browser|cursor|chatgpt|gemini|deepseek|grok|generic)".to_string());
        }
    };
    let tasks = cfg.ticket_queue.pending_for(target);
    poem::Response::builder()
        .header("content-type", "application/json")
        .body(serde_json::to_string(&tasks).unwrap_or_else(|_| "[]".to_string()))
}

#[derive(Deserialize, Default)]
struct CompletionReportBody {
    /// 完成度(0〜100%)。呼び出し側(AIツールまたはそれを操作する人間)の
    /// 自己申告値。省略時は100(単純な「完了マークのみ」の後方互換)。
    #[serde(default = "default_completion_percent")]
    completion_percent: u8,
    #[serde(default)]
    notes: String,
}
fn default_completion_percent() -> u8 {
    100
}

/// 対応完了マーク兼、完了報告(スケジュール遵守・完成度の自動記録)。
/// 対応したAIツール(またはそれを操作する人間)がこのAPIを呼び出すことで
/// 記録される——外部からAIツールを強制的に呼び出して報告させる機能では
/// ない(第三者チャットUIのAPI/セッションは保有していないため)。
#[handler]
fn mark_task_handled(
    req: &Request,
    cfg: poem::web::Data<&Arc<AppConfig>>,
    PPath((source, ticket_id, target)): PPath<(String, u64, String)>,
    body: poem::Result<Json<CompletionReportBody>>,
) -> poem::Response {
    match session_role(req, &cfg) {
        Some(role) if role.can_edit() => {}
        Some(_) => return forbidden("editor or admin role required"),
        None => return unauthorized("login required"),
    }
    let Some(target) = parse_target(&target) else {
        return poem::Response::builder()
            .header("content-type", "application/json")
            .body(serde_json::json!({"ok": false, "error": "unknown target"}).to_string());
    };
    let Json(report_body) = body.unwrap_or(Json(CompletionReportBody { completion_percent: default_completion_percent(), notes: String::new() }));
    match cfg.ticket_queue.mark_handled_with_report(&source, ticket_id, target, report_body.completion_percent, report_body.notes) {
        Some(report) => poem::Response::builder()
            .header("content-type", "application/json")
            .body(serde_json::json!({"ok": true, "report": report}).to_string()),
        None => poem::Response::builder().header("content-type", "application/json").body(serde_json::json!({"ok": false}).to_string()),
    }
}

#[derive(Deserialize)]
struct SetEstimateBody {
    /// 期待される所要時間(時間単位)。
    hours: f64,
}

/// チケットへ期待スケジュール(見積時間)を設定する。完了時に
/// `mark_task_handled`がこの見積と実経過時間を比較し、スケジュール
/// 通りだったかを自動判定する。
#[handler]
fn set_ticket_estimate(
    req: &Request,
    cfg: poem::web::Data<&Arc<AppConfig>>,
    PPath((source, ticket_id)): PPath<(String, u64)>,
    Json(body): Json<SetEstimateBody>,
) -> poem::Response {
    match session_role(req, &cfg) {
        Some(role) if role.can_edit() => {}
        Some(_) => return forbidden("editor or admin role required"),
        None => return unauthorized("login required"),
    }
    let updated = cfg.ticket_queue.set_estimate(&source, ticket_id, body.hours);
    poem::Response::builder().header("content-type", "application/json").body(serde_json::json!({"ok": true, "updated": updated}).to_string())
}

/// 全ての完了報告(スケジュール遵守・完成度%)の一覧。管理画面での
/// 「どのくらいのスケジュールでどのくらい完成したか」の可視化に使う。
#[handler]
fn completion_reports(cfg: poem::web::Data<&Arc<AppConfig>>) -> poem::Response {
    let reports = cfg.ticket_queue.completion_reports();
    poem::Response::builder()
        .header("content-type", "application/json")
        .body(serde_json::to_string(&reports).unwrap_or_else(|_| "[]".to_string()))
}

#[derive(Deserialize)]
struct RequestOtpBody {
    email: String,
}

/// ログインの入口。管理者メール、または管理者が事前に`Role`を割り当てた
/// メンバーのみOTPを受け取れる(未登録メールへは何もせず、登録済みか
/// どうかをこの応答から推測されないよう常に同じ`{"ok": true}`を返す
/// ——ただし実際にはメールは送らない、というenumeration対策)。
#[handler]
async fn request_otp(cfg: poem::web::Data<&Arc<AppConfig>>, Json(body): Json<RequestOtpBody>) -> Json<serde_json::Value> {
    let is_known = body.email == cfg.admin_email || cfg.accounts.role_of(&body.email).is_some();
    if !is_known {
        return Json(serde_json::json!({"ok": true}));
    }
    let RequestOtpOutcome::Issued(code) = cfg.auth.request_otp(&body.email);
    match &cfg.smtp {
        Some(smtp) => match mail::send_otp(smtp.clone(), body.email.clone(), code).await {
            Ok(()) => Json(serde_json::json!({"ok": true})),
            Err(e) => Json(serde_json::json!({"ok": false, "error": e.to_string()})),
        },
        None => Json(serde_json::json!({"ok": false, "error": "server SMTP is not configured"})),
    }
}

#[derive(Deserialize)]
struct VerifyOtpBody {
    email: String,
    code: String,
}

#[handler]
fn verify_otp(cfg: poem::web::Data<&Arc<AppConfig>>, Json(body): Json<VerifyOtpBody>) -> poem::Response {
    match cfg.auth.consume_otp(&body.email, &body.code) {
        Ok(()) => {
            let token = cfg.auth.create_session(&body.email);
            let role = if body.email == cfg.admin_email { Role::Admin } else { cfg.accounts.role_of(&body.email).unwrap_or(Role::Viewer) };
            poem::Response::builder()
                .header("content-type", "application/json")
                .body(serde_json::json!({"ok": true, "token": token, "email": body.email, "role": role}).to_string())
        }
        Err(e) => poem::Response::builder()
            .status(poem::http::StatusCode::BAD_REQUEST)
            .header("content-type", "application/json")
            .body(serde_json::json!({"ok": false, "error": e.message()}).to_string()),
    }
}

#[handler]
fn logout(req: &Request, cfg: poem::web::Data<&Arc<AppConfig>>) -> Json<serde_json::Value> {
    if let Some(header) = req.header(poem::http::header::AUTHORIZATION) {
        if let Some(token) = header.strip_prefix("Bearer ") {
            cfg.auth.logout(token);
        }
    }
    Json(serde_json::json!({"ok": true}))
}

/// 管理者のみ: 登録メンバー一覧(管理者自身も含む)。
#[handler]
fn list_accounts(req: &Request, cfg: poem::web::Data<&Arc<AppConfig>>) -> poem::Response {
    if session_role(req, &cfg) != Some(Role::Admin) {
        return unauthorized("admin login required");
    }
    let list: Vec<_> = cfg.accounts.list().into_iter().map(|(email, role)| serde_json::json!({"email": email, "role": role})).collect();
    poem::Response::builder().header("content-type", "application/json").body(serde_json::json!({"ok": true, "accounts": list}).to_string())
}

#[derive(Deserialize)]
struct SetAccountRoleBody {
    email: String,
    role: String,
}

/// 管理者のみ: メンバーを登録/ロール変更する(未登録メールを指定すると
/// そのまま新規登録になる)。
#[handler]
fn set_account_role(req: &Request, cfg: poem::web::Data<&Arc<AppConfig>>, Json(body): Json<SetAccountRoleBody>) -> poem::Response {
    if session_role(req, &cfg) != Some(Role::Admin) {
        return unauthorized("admin login required");
    }
    let Some(role) = Role::parse(&body.role) else {
        return poem::Response::builder()
            .status(poem::http::StatusCode::BAD_REQUEST)
            .header("content-type", "application/json")
            .body(serde_json::json!({"ok": false, "error": "role must be one of: viewer|editor|admin"}).to_string());
    };
    cfg.accounts.set_role(&body.email, role);
    poem::Response::builder().header("content-type", "application/json").body(serde_json::json!({"ok": true}).to_string())
}

/// 管理者のみ: メンバーの権限を剥奪する。
#[handler]
fn remove_account(req: &Request, cfg: poem::web::Data<&Arc<AppConfig>>, PPath(email): PPath<String>) -> poem::Response {
    if session_role(req, &cfg) != Some(Role::Admin) {
        return unauthorized("admin login required");
    }
    let removed = cfg.accounts.remove(&email);
    poem::Response::builder().header("content-type", "application/json").body(serde_json::json!({"ok": removed}).to_string())
}

fn build_app(cfg: Arc<AppConfig>) -> impl poem::Endpoint {
    Route::new()
        .at("/", get(index))
        .at("/healthz", get(healthz))
        .at("/install.sh", get(install_sh))
        .at("/install.ps1", get(install_ps1))
        .at("/api/projects", get(list_projects))
        .at("/api/priorities", get(list_priorities).post(set_priority))
        .at("/api/services", get(service_statuses))
        .at("/api/github/repos", get(github_org_repos))
        .at("/api/gitlab/repos", get(gitlab_repos))
        .at("/api/bitbucket/repos", get(bitbucket_repos))
        .at("/api/redmine/issues", get(redmine_issues))
        .at("/api/gitbucket/repos", get(gitbucket_repos))
        .at("/api/rs-chiketto/health", get(rs_chiketto_health))
        .at("/api/rs-git/repos", get(rs_git_repos))
        .at("/api/aruaru-llm/health", get(aruaru_llm_health))
        .at("/api/generate", get(generate))
        .at("/api/tickets/poll-now", post(poll_tickets_now))
        .at("/api/tickets/pending-tasks", get(pending_tasks))
        .at("/api/tickets/completion-reports", get(completion_reports))
        .at("/api/tickets/:source/:ticket_id/handled/:target", post(mark_task_handled))
        .at("/api/tickets/:source/:ticket_id/estimate", post(set_ticket_estimate))
        .at("/api/auth/request-otp", post(request_otp))
        .at("/api/auth/verify-otp", post(verify_otp))
        .at("/api/auth/logout", post(logout))
        .at("/api/accounts", get(list_accounts).post(set_account_role))
        .at("/api/accounts/:email", poem::delete(remove_account))
        .data(cfg)
}

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();

    let scan_root = std::env::var("RS_OPS_SCAN_ROOT")
        .unwrap_or_else(|_| "F:\\runo".to_string());
    let rs_chiketto_url = std::env::var("RS_OPS_RS_CHIKETTO_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8100".to_string());
    let redmine_url = std::env::var("RS_OPS_REDMINE_URL").ok();
    let rs_git_url = std::env::var("RS_OPS_RS_GIT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8090".to_string());
    let aruaru_llm_url = std::env::var("RS_OPS_ARUARU_LLM_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:4600".to_string());
    let data_dir = std::env::var("RS_OPS_DATA_DIR")
        .unwrap_or_else(|_| "data".to_string());
    let poll_interval: u64 = std::env::var("RS_OPS_TICKET_POLL_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    let queue_path = ticket_automation::default_queue_path(std::path::Path::new(&data_dir));
    let ticket_queue = Arc::new(TicketQueue::load(queue_path));

    let priority_path = priority::default_priority_path(std::path::Path::new(&data_dir));
    let priority_store = Arc::new(PriorityStore::load(priority_path));

    let admin_email = std::env::var("RS_OPS_ADMIN_EMAIL").unwrap_or_default();
    let accounts_path = std::path::Path::new(&data_dir).join("accounts.json");
    let accounts = Arc::new(AccountStore::load(accounts_path, admin_email.clone()));
    let auth = Arc::new(AuthStore::default());
    let smtp = SmtpConfig::from_env();
    if admin_email.is_empty() {
        tracing::warn!("RS_OPS_ADMIN_EMAIL is not set — no one can log in as admin");
    }
    if smtp.is_none() {
        tracing::warn!("RS_OPS_SMTP_* is not fully configured — OTP emails cannot be sent");
    }

    let cfg = Arc::new(AppConfig {
        scan_root: PathBuf::from(scan_root),
        rs_chiketto_url: rs_chiketto_url.clone(),
        redmine_url: redmine_url.clone(),
        rs_git_url,
        aruaru_llm_url,
        ticket_queue: ticket_queue.clone(),
        priority_store,
        auth,
        accounts,
        admin_email,
        smtp,
    });

    tokio::spawn(ticket_automation::run_polling_loop(ticket_queue, rs_chiketto_url, redmine_url, poll_interval));

    let port: u16 = std::env::var("RS_OPS_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8097);

    let app = build_app(cfg);
    tracing::info!("rs-ops listening on 0.0.0.0:{port}");
    Server::new(TcpListener::bind(("0.0.0.0", port)))
        .run(app)
        .await
}

#[cfg(test)]
mod handler_tests {
    use super::*;
    use poem::test::TestClient;

    fn test_cfg() -> Arc<AppConfig> {
        let dir = tempfile::tempdir().unwrap();
        Arc::new(AppConfig {
            scan_root: dir.path().to_path_buf(),
            rs_chiketto_url: "http://127.0.0.1:1".to_string(),
            redmine_url: None,
            rs_git_url: "http://127.0.0.1:1".to_string(),
            aruaru_llm_url: "http://127.0.0.1:1".to_string(),
            ticket_queue: Arc::new(TicketQueue::load(dir.path().join("queue.json"))),
            priority_store: Arc::new(PriorityStore::load(dir.path().join("priorities.json"))),
            auth: Arc::new(AuthStore::default()),
            accounts: Arc::new(AccountStore::load(dir.path().join("accounts.json"), "admin@example.com")),
            admin_email: "admin@example.com".to_string(),
            smtp: None,
        })
    }

    #[tokio::test]
    async fn set_priority_without_login_is_401() {
        let cfg = test_cfg();
        let client = TestClient::new(build_app(cfg));
        let resp = client.post("/api/priorities").body_json(&serde_json::json!({"key": "x", "stars": 3})).send().await;
        resp.assert_status(poem::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn set_priority_as_admin_session_succeeds() {
        let cfg = test_cfg();
        let token = cfg.auth.create_session("admin@example.com");
        let client = TestClient::new(build_app(cfg));
        let resp = client
            .post("/api/priorities")
            .header("Authorization", format!("Bearer {token}"))
            .body_json(&serde_json::json!({"key": "x", "stars": 3}))
            .send()
            .await;
        resp.assert_status_is_ok();
    }

    #[tokio::test]
    async fn set_priority_as_viewer_role_is_403() {
        let cfg = test_cfg();
        cfg.accounts.set_role("viewer@example.com", Role::Viewer);
        let token = cfg.auth.create_session("viewer@example.com");
        let client = TestClient::new(build_app(cfg));
        let resp = client
            .post("/api/priorities")
            .header("Authorization", format!("Bearer {token}"))
            .body_json(&serde_json::json!({"key": "x", "stars": 3}))
            .send()
            .await;
        resp.assert_status(poem::http::StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn set_priority_as_editor_role_succeeds() {
        let cfg = test_cfg();
        cfg.accounts.set_role("editor@example.com", Role::Editor);
        let token = cfg.auth.create_session("editor@example.com");
        let client = TestClient::new(build_app(cfg));
        let resp = client
            .post("/api/priorities")
            .header("Authorization", format!("Bearer {token}"))
            .body_json(&serde_json::json!({"key": "x", "stars": 3}))
            .send()
            .await;
        resp.assert_status_is_ok();
    }

    #[tokio::test]
    async fn accounts_management_requires_admin_not_just_any_login() {
        let cfg = test_cfg();
        cfg.accounts.set_role("editor@example.com", Role::Editor);
        let editor_token = cfg.auth.create_session("editor@example.com");
        let admin_token = cfg.auth.create_session("admin@example.com");
        let client = TestClient::new(build_app(cfg));

        let resp = client.get("/api/accounts").header("Authorization", format!("Bearer {editor_token}")).send().await;
        resp.assert_status(poem::http::StatusCode::UNAUTHORIZED);

        let resp = client.get("/api/accounts").header("Authorization", format!("Bearer {admin_token}")).send().await;
        resp.assert_status_is_ok();
    }

    #[tokio::test]
    async fn admin_can_register_and_remove_a_member() {
        let cfg = test_cfg();
        let admin_token = cfg.auth.create_session("admin@example.com");
        let client = TestClient::new(build_app(cfg));

        let resp = client
            .post("/api/accounts")
            .header("Authorization", format!("Bearer {admin_token}"))
            .body_json(&serde_json::json!({"email": "new@example.com", "role": "editor"}))
            .send()
            .await;
        resp.assert_status_is_ok();

        let resp = client.delete("/api/accounts/new@example.com").header("Authorization", format!("Bearer {admin_token}")).send().await;
        resp.assert_status_is_ok();
    }
}
