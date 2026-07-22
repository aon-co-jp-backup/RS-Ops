use serde::{Deserialize, Serialize};

/// 課題管理システム(RS-Chiketto/標準Redmine)から取得したチケット1件。
/// RS-Chikettoの`Ticket`と標準Redmineの`Issue`、フィールド名は異なるが
/// 意味的に対応するものだけをここに正規化する。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedTicket {
    pub id: u64,
    pub title: String,
    pub description: String,
    /// 元システムでの生ステータス文字列(例: "Open"/"open"等、正規化しない)。
    pub status: String,
    /// どの課題管理システムから来たか(例: "rs-chiketto"/"redmine"/
    /// "gitlab"/"github")。複数ソースを1つのキューへ集約する際、同じ
    /// 数値`id`が別システムのチケットと衝突しないよう、実際の重複排除
    /// キーは`(source, id)`の組で行う(`ticket_automation`参照)。
    #[serde(default)]
    pub source: String,
}

/// 新規チケットに対して自動生成されたAIタスク依頼。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingTask {
    /// 元システム名(例: "rs-chiketto"/"redmine"/"gitlab"/"github")。
    #[serde(default)]
    pub source: String,
    pub ticket_id: u64,
    pub target: AiTarget,
    pub prompt: String,
    pub handled: bool,
    /// タスクがキューに登録された時刻(UNIX秒)。スケジュール遵守判定の起点。
    #[serde(default)]
    pub created_at: i64,
    /// 期待される所要時間(時間単位)。管理者が`POST /api/tickets/:source/:id/estimate`
    /// で設定する任意値。未設定ならスケジュール遵守判定は行わない。
    #[serde(default)]
    pub estimated_hours: Option<f64>,
}

/// 対応完了時にAIが(またはこのシステムが自動的に)生成する完了報告。
/// 「依頼完了後、スケジュール通りだったか・何%完成したか」を記録する。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionReport {
    #[serde(default)]
    pub source: String,
    pub ticket_id: u64,
    pub target: AiTarget,
    /// 完成度(0〜100%)。呼び出し側が自己申告する値。
    pub completion_percent: u8,
    /// `estimated_hours`が設定されていた場合のみ算出される、期待
    /// スケジュール通りに完了したかどうか(実経過時間 <= 見積時間)。
    /// 見積が無い場合はNone(「スケジュール通りか」を判定できないため)。
    pub on_schedule: Option<bool>,
    pub estimated_hours: Option<f64>,
    /// 登録から完了報告までの実経過時間(時間単位)。
    pub actual_hours: f64,
    pub notes: String,
    pub reported_at: i64,
}

/// F:\runo(または任意のルート)配下でスキャンして見つかったプロジェクト。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub path: String,
    pub github_url: Option<String>,
    pub description: Option<String>,
    /// ユーザーが手動設定した優先度(1〜5、星評価)。未設定はNone。
    /// 星1 = 「今は開発などをしなくて良い」という意味(ユーザー定義)。
    #[serde(default)]
    pub priority: Option<u8>,
}

/// 対応(または対応予定)のGit/課題管理サービス。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GitService {
    GitHub,
    Bitbucket,
    Gitbucket,
    GitLab,
    /// 標準Redmine REST API。
    Redmine,
    /// このエコシステム独自のRedmine相当実装(RS-Chiketto)。標準Redmine
    /// APIとは形状が異なる箇所があるため別variantとして扱う。
    RsChiketto,
    /// このエコシステム独自のGitフォージ実装(RS-Git、旧称RGit)。
    /// `GET /api/repos`(認証無しでは公開リポジトリのみ)を叩く。
    /// 2026-07-22追記: RS-Ops・RS-Git・aruaru-llmの「SET」完成の一環として
    /// 追加。
    RsGit,
}

impl GitService {
    /// 2026-07時点の実装状況(嘘をつかない: 動くふりをしない)。
    pub fn implementation_status(self) -> &'static str {
        match self {
            GitService::GitHub => "implemented (public REST API, repo list + README fetch)",
            GitService::RsChiketto => "implemented (health check + ticket list via this ecosystem's Redmine-equivalent API)",
            GitService::GitLab => "implemented (public REST API v4, verified live against gitlab.com)",
            GitService::Bitbucket => "implemented (public REST API 2.0, verified live against bitbucket.org)",
            GitService::Redmine => "implemented (standard /issues.json REST API; verified against a live public Redmine instance)",
            GitService::Gitbucket => "implemented against GitBucket's documented GitHub-API-v3-compatible shape, but NOT verified live (no public GitBucket demo instance exists to test against — point it at your own instance's base_url to confirm)",
            GitService::RsGit => "implemented (calls this ecosystem's own RS-Git `GET /api/repos`, which returns a plain JSON array of repo names); verified live against https://runo.tokyo/rs-git on 2026-07-22 (returned an empty array — no repos are public there yet, so the empty case is what was exercised, not a populated list)",
        }
    }
}

/// 対応(または対応予定)のAIツール。各ツールが実際に読み込む
/// コンテキストファイル形式が異なるため、生成先フォーマットを列挙する。
/// 2026-07-22、ユーザー指示によりChatGPT/Gemini/DeepSeek/Grok/Claude
/// (ブラウザ版)を「Genericに丸めた1個」ではなく個別に列挙・命名する
/// よう拡張(呼び出し側API/UIで各ツール名を明示的に選べるようにする
/// ため)。**正直な開示は変わらない**: これら第三者チャットUIへ本サービス
/// からAPIを強制呼び出しする機能は無い(そのAPI/セッションを保有して
/// いないため)。個別化したのは、各ツールごとに宛先ラベル付きの
/// コンテキスト/タスク依頼テキストを生成・区別できるようにするため
/// であり、出力フォーマット自体(`AGENTS.md`相当のプレーンテキスト)は
/// 共通のまま。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiTarget {
    /// Claude Code / Claude Code Desktop(`CLAUDE.md`を読む)。
    Claude,
    /// claude.ai(ブラウザ版チャットUI)・Claude in Chrome拡張機能。
    /// `CLAUDE.md`規約は読まないため、Generic相当のプレーンテキストを返す。
    ClaudeBrowser,
    Cursor,
    ChatGpt,
    Gemini,
    DeepSeek,
    Grok,
    /// 上記以外の任意のLLMコーディングアシスタント向けの汎用フォールバック。
    Generic,
}

impl AiTarget {
    /// UI表示用の人間可読な製品名。
    pub fn display_name(self) -> &'static str {
        match self {
            AiTarget::Claude => "Claude / Claude Code Desktop",
            AiTarget::ClaudeBrowser => "Claude (claude.ai / Claude in Chrome)",
            AiTarget::Cursor => "Cursor",
            AiTarget::ChatGpt => "ChatGPT",
            AiTarget::Gemini => "Gemini",
            AiTarget::DeepSeek => "DeepSeek",
            AiTarget::Grok => "Grok",
            AiTarget::Generic => "Generic (any other LLM tool)",
        }
    }

    pub fn implementation_status(self) -> &'static str {
        match self {
            AiTarget::Claude => "implemented (CLAUDE.md template generator)",
            AiTarget::Cursor => "implemented (.cursorrules template generator)",
            AiTarget::ClaudeBrowser
            | AiTarget::ChatGpt
            | AiTarget::Gemini
            | AiTarget::DeepSeek
            | AiTarget::Grok
            | AiTarget::Generic => {
                "implemented (AGENTS.md-equivalent generic template, individually addressable per tool; \
                 this service does not call the vendor's API or launch the tool itself — no such API/session \
                 is held — it only prepares labeled context/task text for that tool to read)"
            }
        }
    }

    pub fn output_filename(self) -> &'static str {
        match self {
            AiTarget::Claude => "CLAUDE.md",
            AiTarget::Cursor => ".cursorrules",
            AiTarget::ClaudeBrowser
            | AiTarget::ChatGpt
            | AiTarget::Gemini
            | AiTarget::DeepSeek
            | AiTarget::Grok
            | AiTarget::Generic => "AGENTS.md",
        }
    }

    /// `pending_for`等のAPI経路で全ターゲットを走査するための一覧。
    pub fn all() -> [AiTarget; 8] {
        [
            AiTarget::Claude,
            AiTarget::ClaudeBrowser,
            AiTarget::Cursor,
            AiTarget::ChatGpt,
            AiTarget::Gemini,
            AiTarget::DeepSeek,
            AiTarget::Grok,
            AiTarget::Generic,
        ]
    }
}
