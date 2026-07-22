use crate::model::{GitService, NormalizedTicket};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

/// GitHub/Bitbucket/Gitbucket/GitLab/Redmine/RS-Chikettoを同じ形で
/// 扱うための最小インターフェース。全プロバイダを完璧に実装することは
/// 目指さない——`GitHub`と`RsChiketto`のみ実際にHTTP接続する実装を持ち、
/// 残りは`unimplemented`な結果を返す(実装しているふりをしない)。
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
                .user_agent("RS-AI-DevOps")
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
                .user_agent("RS-AI-DevOps")
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
                })
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
