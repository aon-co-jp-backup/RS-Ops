# RS-AI-DevOps

日本語 | [English](#english)

エコシステム全体マップ生成 × AIエージェント向けコンテキストファイル生成 ×
複数のGit/課題管理サービス連携を1つにまとめたオープンソースツール。

`runo-scanner`(プロジェクト自動検出)と`RS-Sync`(プロバイダ抽象化・
インストーラー・デプロイパターン)の資産を土台にしている。

## できること

- 指定ディレクトリ(既定: `F:\runo`)配下のプロジェクトを自動検出し、
  各プロジェクトの役割説明・GitHubリンクを一覧表示する。
- **GitHub**: 実際にpublic REST APIを叩き、リポジトリ一覧を取得する
  (実装済み・実HTTP検証済み)。
- **Redmine相当(RS-Chiketto)**: このエコシステム独自のRust+RPoem製
  Redmine版と連携し、ヘルスチェック・チケット一覧の到達性を確認する
  (実装済み・実HTTP検証済み)。
- **Bitbucket / Gitbucket / GitLab / 標準Redmine**: アダプタのインター
  フェースは定義済みだが、実際のライブ接続は未実装(正直に「対応予定」と
  明記)。
- **AIツール向けコンテキストファイル生成**:
  - Claude / Claude Code Desktop 向け `CLAUDE.md` 雛形(実装済み)
  - Cursor 向け `.cursorrules` 雛形(実装済み)
  - ChatGPT / Gemini / DeepSeek / Grok 向けの汎用 `AGENTS.md` 雛形
    (実装済み、system promptとしてそのまま使える形式)

## 使い方(ローカル)

```bash
cargo build --release
RS_AI_DEVOPS_SCAN_ROOT=/path/to/your/repos ./target/release/rs-ai-devops
# http://127.0.0.1:8097/ を開く
```

## API

- `GET /api/projects` — 検出済みプロジェクト一覧(JSON)
- `GET /api/services` — 対応Git/課題管理サービスと実装状況
- `GET /api/github/repos` — GitHub(`aon-co-jp`)のリポジトリ一覧
- `GET /api/rs-chiketto/health` — RS-Chikettoの到達性確認
- `GET /api/generate?target=claude|cursor|generic&name=...&description=...&repo_url=...`
  — AIツール向けコンテキストファイル雛形を生成

## インストーラー

Web UI(`/`)の「ダウンロード」節から、Linux向け`install.sh`・Windows向け
`install.ps1`をダウンロードできる。いずれもソースからのcargoビルドを行う
(署名済みバイナリ・deb/msiパッケージングは未実装、正直に開示)。

## 正直な開示(既知の制約)

- Bitbucket/Gitbucket/GitLab/標準Redmineは未実装(インターフェースのみ)。
- ChatGPT/Gemini/DeepSeek/Grokは、各社固有のAPI連携(実際にAPIキーで
  呼び出す機能)ではなく、system promptとして使える汎用テンプレート生成
  までの対応。
- コード署名・deb/msiパッケージングは未実装。

---

## English

An open-source tool that combines ecosystem-map generation, AI-agent
context-file generation, and multi Git/issue-tracker integration.

Built on top of `runo-scanner` (project auto-detection) and `RS-Sync`
(provider abstraction, installer, deployment pattern).

### What it does

- Scans a directory (default: `F:\runo`) for projects and lists each
  project's role description and GitHub link.
- **GitHub**: hits the real public REST API to list repos (implemented,
  verified over real HTTP).
- **Redmine-equivalent (RS-Chiketto)**: talks to this ecosystem's own
  Rust+RPoem Redmine equivalent — health check and ticket-list reachability
  (implemented, verified over real HTTP).
- **Bitbucket / Gitbucket / GitLab / standard Redmine**: adapter interfaces
  are defined but not yet wired to a live connection (honestly marked
  "planned").
- **AI-agent context file generation**:
  - `CLAUDE.md` template for Claude / Claude Code Desktop (implemented)
  - `.cursorrules` template for Cursor (implemented)
  - generic `AGENTS.md` template for ChatGPT / Gemini / DeepSeek / Grok
    (implemented, usable as a system-prompt seed as-is)

### Local usage

```bash
cargo build --release
RS_AI_DEVOPS_SCAN_ROOT=/path/to/your/repos ./target/release/rs-ai-devops
# open http://127.0.0.1:8097/
```

### Honest disclosures (known limitations)

- Bitbucket/Gitbucket/GitLab/standard Redmine are interface-only, not wired
  to live services yet.
- ChatGPT/Gemini/DeepSeek/Grok support means generating a generic template
  usable as a system prompt — not a per-vendor API integration.
- No code signing or deb/msi packaging yet.
