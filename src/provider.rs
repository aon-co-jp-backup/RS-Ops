use crate::model::{GitService, NormalizedTicket};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// GitHub/Bitbucket/Gitbucket/GitLab/Redmine/RS-Chikettoを同じ形で
/// 扱うための最小インターフェース。2026-07-22時点、GitHub/RsChiketto/
/// GitLab/Bitbucket/標準Redmineは実際にHTTP接続する実装を持つ
/// (それぞれ公開APIへの実HTTP呼び出しで検証済み、詳細はCLAUDE.md
/// HANDOFF参照)。Gitbucketのみ、自前でホストする公開デモ環境が
/// 存在しないため、GitBucket自身が公称する「GitHub API v3互換」仕様に
/// 基づいて実装したが、**この session では実機(生きているGitBucket
/// インスタンス)に対して検証できていない**(正直な開示)。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    pub name: String,
    pub description: Option<String>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketSummary {
    pub id: String,
    pub subject: String,
    pub status: String,
}

pub struct GitHubProvider {
    client: reqwest::Client,
}

impl GitHubProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("RS-Ops")
                .build()
                .expect("reqwest client"),
        }
    }

    /// 公開APIのみ(認証無し)。ユーザー/organizationのpublicリポジトリ
    /// 一覧を取得する(`aon-co-jp`は実際にはGitHub organizationではなく
    /// user accountであることを実APIで確認済みのため`/users/`エンドポイント
    /// を使う——`/orgs/`だと404になる)。
    pub async fn list_org_repos(&self, org: &str) -> Result<Vec<RepoSummary>> {
        let url = format!("https://api.github.com/users/{org}/repos?per_page=100");
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("GitHub API returned {}", resp.status());
        }
        let body: serde_json::Value = resp.json().await?;
        let arr = body.as_array().cloned().unwrap_or_default();
        Ok(arr
            .into_iter()
            .map(|r| RepoSummary {
                name: r["name"].as_str().unwrap_or_default().to_string(),
                description: r["description"].as_str().map(str::to_string),
                url: r["html_url"].as_str().unwrap_or_default().to_string(),
            })
            .collect())
    }

    pub async fn fetch_readme_raw(&self, org: &str, repo: &str) -> Result<String> {
        for branch in ["main", "master"] {
            let url = format!("https://raw.githubusercontent.com/{org}/{repo}/{branch}/README.md");
            if let Ok(resp) = self.client.get(&url).send().await {
                if resp.status().is_success() {
                    return Ok(resp.text().await?);
                }
            }
        }
        bail!("README.md not found on main or master branch")
    }
}

/// RS-Chiketto(このエコシステム独自のRedmine相当実装)への接続。
/// 標準Redmine REST APIとの既知の差異: RS-Chikettoは`/healthz`・
/// `/api/tickets`(要OTPセッション)を持つが、標準Redmineの
/// `/issues.json`とは形状が異なる(項目名・認証方式が異なる)。
/// 今回はセッション無しで到達可能な範囲(ヘルスチェック)のみ実装する。
pub struct RsChikettoProvider {
    client: reqwest::Client,
    base_url: String,
}

impl RsChikettoProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("RS-Ops")
                .build()
                .expect("reqwest client"),
            base_url: base_url.into(),
        }
    }

    pub async fn health_check(&self) -> Result<bool> {
        let url = format!("{}/healthz", self.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;
        Ok(resp.status().is_success())
    }

    /// 標準Redmineの`/issues.json`と同じ意味論を狙うが、RS-Chiketto側は
    /// 認証(OTPセッション)必須のため、未認証では401が返ることを想定した
    /// 呼び出し。SMTP未設定環境では実際のログインまで検証できないため、
    /// ここでは「到達できるか・認証境界が機能しているか」までを確認する。
    pub async fn list_tickets_unauthenticated_probe(&self) -> Result<u16> {
        let url = format!("{}/api/tickets", self.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;
        Ok(resp.status().as_u16())
    }

    /// 実際にチケット一覧を取得し正規化する(未ログインでも閲覧可能な
    /// 範囲のチケットのみ返る——RS-Chikettoの`access.rs`によるフィルタ
    /// 済み結果。管理者権限が必要なチケットまでは取得できないが、
    /// チケット自動対応機能の第一歩としては十分な範囲)。
    pub async fn list_tickets(&self) -> Result<Vec<NormalizedTicket>> {
        let url = format!("{}/api/tickets", self.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("RS-Chiketto ticket list returned {}", resp.status());
        }
        let raw: Vec<serde_json::Value> = resp.json().await?;
        Ok(raw
            .into_iter()
            .filter_map(|t| {
                Some(NormalizedTicket {
                    id: t["id"].as_u64()?,
                    title: t["title"].as_str().unwrap_or_default().to_string(),
                    description: t["description"].as_str().unwrap_or_default().to_string(),
                    status: t["status"].as_str().unwrap_or_default().to_string(),
                    source: "rs-chiketto".to_string(),
                })
            })
            .collect())
    }
}

/// GitLab公開REST API(v4)への接続。認証無しでも公開プロジェクトの
/// 一覧は取得できる(gitlab.comで実HTTP検証済み)。
pub struct GitLabProvider {
    client: reqwest::Client,
    base_url: String,
}

impl GitLabProvider {
    /// `base_url`省略時は`https://gitlab.com`(自前ホストのGitLab
    /// インスタンスがあればそのURLを渡せる)。
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder().user_agent("RS-Ops").build().expect("reqwest client"),
            base_url: base_url.into(),
        }
    }

    /// `/api/v4/users/:username/projects`。ユーザーの公開プロジェクト一覧。
    pub async fn list_user_projects(&self, username: &str) -> Result<Vec<RepoSummary>> {
        let url = format!("{}/api/v4/users/{username}/projects", self.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("GitLab API returned {}", resp.status());
        }
        let body: Vec<serde_json::Value> = resp.json().await?;
        Ok(body
            .into_iter()
            .map(|r| RepoSummary {
                name: r["name"].as_str().unwrap_or_default().to_string(),
                description: r["description"].as_str().map(str::to_string),
                url: r["web_url"].as_str().unwrap_or_default().to_string(),
            })
            .collect())
    }
}

/// Bitbucket Cloud公開REST API(2.0)への接続。認証無しでも公開
/// workspaceのリポジトリ一覧は取得できる(bitbucket.orgで実HTTP検証済み)。
pub struct BitbucketProvider {
    client: reqwest::Client,
}

impl BitbucketProvider {
    pub fn new() -> Self {
        Self { client: reqwest::Client::builder().user_agent("RS-Ops").build().expect("reqwest client") }
    }

    /// `/2.0/repositories/:workspace`。指定workspaceの公開リポジトリ一覧。
    pub async fn list_workspace_repos(&self, workspace: &str) -> Result<Vec<RepoSummary>> {
        let url = format!("https://api.bitbucket.org/2.0/repositories/{workspace}");
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("Bitbucket API returned {}", resp.status());
        }
        let body: serde_json::Value = resp.json().await?;
        let arr = body["values"].as_array().cloned().unwrap_or_default();
        Ok(arr
            .into_iter()
            .map(|r| RepoSummary {
                name: r["name"].as_str().unwrap_or_default().to_string(),
                description: r["description"].as_str().filter(|s| !s.is_empty()).map(str::to_string),
                url: r["links"]["html"]["href"].as_str().unwrap_or_default().to_string(),
            })
            .collect())
    }
}

impl Default for BitbucketProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// 標準Redmine REST APIへの接続。`/issues.json`は多くの公開Redmine
/// インスタンスで匿名閲覧が許可されている(認証無しでも200が返る)。
/// RS-Chikettoとは異なりOTPセッション等の独自認証は要求しない前提。
pub struct RedmineProvider {
    client: reqwest::Client,
    base_url: String,
}

impl RedmineProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder().user_agent("RS-Ops").build().expect("reqwest client"),
            base_url: base_url.into(),
        }
    }

    /// `/issues.json`。匿名閲覧が許可されていない場合は401/403が返るので
    /// (それ自体を`bail!`で正直にエラーとして返す、黙って空配列にはしない)。
    pub async fn list_issues(&self) -> Result<Vec<NormalizedTicket>> {
        let url = format!("{}/issues.json", self.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("Redmine API returned {}", resp.status());
        }
        let body: serde_json::Value = resp.json().await?;
        let arr = body["issues"].as_array().cloned().unwrap_or_default();
        Ok(arr
            .into_iter()
            .filter_map(|t| {
                Some(NormalizedTicket {
                    id: t["id"].as_u64()?,
                    title: t["subject"].as_str().unwrap_or_default().to_string(),
                    description: t["description"].as_str().unwrap_or_default().to_string(),
                    status: t["status"]["name"].as_str().unwrap_or_default().to_string(),
                    source: "redmine".to_string(),
                })
            })
            .collect())
    }
}

/// GitBucket(セルフホスト型、GitHub API v3互換を公称)への接続。
///
/// **正直な開示**: GitBucketには`GitHubProvider`同様のパブリックデモ
/// サーバーが存在しないため、この実装は公式ドキュメントが謳う
/// 「GitHub API v3互換」エンドポイント形状に基づいて書いたが、実際に
/// 稼働しているGitBucketインスタンスに対しては未検証(実HTTP確認が
/// できていない)。ユーザーが自前のGitBucketインスタンスURLを持って
/// いれば、それを`base_url`に渡すことで動作確認できる設計にしてある。
pub struct GitbucketProvider {
    client: reqwest::Client,
    base_url: String,
}

impl GitbucketProvider {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder().user_agent("RS-Ops").build().expect("reqwest client"),
            base_url: base_url.into(),
        }
    }

    /// `/api/v3/users/:owner/repos`(GitHub API v3互換パス)。
    pub async fn list_owner_repos(&self, owner: &str) -> Result<Vec<RepoSummary>> {
        let url = format!("{}/api/v3/users/{owner}/repos", self.base_url.trim_end_matches('/'));
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            bail!("GitBucket API returned {}", resp.status());
        }
        let body: Vec<serde_json::Value> = resp.json().await?;
        Ok(body
            .into_iter()
            .map(|r| RepoSummary {
                name: r["name"].as_str().unwrap_or_default().to_string(),
                description: r["description"].as_str().map(str::to_string),
                url: r["html_url"].as_str().unwrap_or_default().to_string(),
            })
            .collect())
    }
}

/// GitService列挙値ごとの実装状況をまとめて返す(UI表示用)。
pub fn all_service_statuses() -> Vec<(GitService, &'static str)> {
    [
        GitService::GitHub,
        GitService::RsChiketto,
        GitService::Redmine,
        GitService::Bitbucket,
        GitService::Gitbucket,
        GitService::GitLab,
    ]
    .into_iter()
    .map(|s| (s, s.implementation_status()))
    .collect()
}
