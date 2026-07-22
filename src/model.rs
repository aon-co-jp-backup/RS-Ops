use serde::{Deserialize, Serialize};

/// F:\runo(または任意のルート)配下でスキャンして見つかったプロジェクト。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectEntry {
    pub name: String,
    pub path: String,
    pub github_url: Option<String>,
    pub description: Option<String>,
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
}

impl GitService {
    /// 2026-07時点の実装状況(嘘をつかない: 動くふりをしない)。
    pub fn implementation_status(self) -> &'static str {
        match self {
            GitService::GitHub => "implemented (public REST API, repo list + README fetch)",
            GitService::RsChiketto => "implemented (health check + ticket list via this ecosystem's Redmine-equivalent API)",
            GitService::Redmine => "planned (standard REST API shape drafted, not yet wired to a live instance)",
            GitService::Bitbucket | GitService::Gitbucket | GitService::GitLab => "planned (adapter trait defined, no live implementation yet)",
        }
    }
}

/// 対応(または対応予定)のAIツール。各ツールが実際に読み込む
/// コンテキストファイル形式が異なるため、生成先フォーマットを列挙する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiTarget {
    /// Claude Code / Claude Code Desktop(どちらも`CLAUDE.md`を読む)。
    Claude,
    Cursor,
    /// ChatGPT/Gemini/DeepSeek/Grok等、APIのsystem promptとして投入する
    /// 汎用フォーマット(`AGENTS.md`)。
    Generic,
}

impl AiTarget {
    pub fn implementation_status(self) -> &'static str {
        match self {
            AiTarget::Claude => "implemented (CLAUDE.md template generator)",
            AiTarget::Cursor => "implemented (.cursorrules template generator)",
            AiTarget::Generic => "implemented (AGENTS.md generic system-prompt template; usable as-is for ChatGPT/Gemini/DeepSeek/Grok)",
        }
    }

    pub fn output_filename(self) -> &'static str {
        match self {
            AiTarget::Claude => "CLAUDE.md",
            AiTarget::Cursor => ".cursorrules",
            AiTarget::Generic => "AGENTS.md",
        }
    }
}
