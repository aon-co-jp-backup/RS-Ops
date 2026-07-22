# RS-Ops

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
- **GitLab / Bitbucket / 標準Redmine**: それぞれ公開REST API(GitLab
  v4・Bitbucket 2.0・Redmineの`/issues.json`)へ実際にHTTP接続する実装
  (実装済み・実HTTP検証済み、2026-07-22)。
- **Gitbucket**: GitHub API v3互換の仕様に基づいて実装したが、検証可能な
  公開デモインスタンスが存在しないためこの時点では未検証(正直に開示、
  自前のGitBucketインスタンスがあれば`base_url`で確認可能)。
- **AIツール向けコンテキストファイル生成**(2026-07-22、ツールごとに
  個別のターゲットとして命名・区別できるよう拡張):
  - Claude / Claude Code Desktop 向け `CLAUDE.md` 雛形(実装済み)
  - Cursor 向け `.cursorrules` 雛形(実装済み)
  - Claude(claude.ai / Claude in Chrome)・ChatGPT・Gemini・DeepSeek・
    Grok・その他汎用ツール向けの`AGENTS.md`雛形(いずれも実装済み。
    第三者チャットUIへAPIを強制呼び出す機能ではなく、ツール名を
    明示したテキストを生成するところまでが対応範囲)

## 使い方(ローカル)

```bash
cargo build --release
RS_OPS_SCAN_ROOT=/path/to/your/repos ./target/release/rs-ops
# http://127.0.0.1:8097/ を開く
```

## チケット自動対応機能(2026-07-22実装)

RS-Chiketto(将来的に標準Redmineも)を定期ポーリングし、未対応のチケットを
検出したらClaude/Claude Code Desktop/Cursor/ChatGPT/Gemini/DeepSeek/Grok
向けのタスク依頼プロンプトを自動生成する機能。

**設計意図**: 第三者のチャットUI(ChatGPT/Gemini/DeepSeek/Grok)を外部
APIで強制的に呼び出す方式ではない。狙いは、**RS-Chiketto/Redmineの
チケット情報を、Claude Code Desktop等のAIツールが実際に読み込む形
(CLAUDE.md/.cursorrules/AGENTS.md相当のコンテキスト)へ変換して提示
すること**——これにより各AIツールは自分自身のセッション内で課題管理
システムの状況を丸ごと把握でき、人間が個別にチケットの内容を説明する
手間を省いた上で対応に着手できる。「チケット→AIツールへの状況把握・
引き継ぎ」を自動化することで、「チケットが来たら自動対応する」という
体験を再現している。

## API

- `GET /api/projects` — 検出済みプロジェクト一覧(JSON)
- `GET /api/services` — 対応Git/課題管理サービスと実装状況
- `GET /api/github/repos` — GitHub(`aon-co-jp`)のリポジトリ一覧
- `GET /api/gitlab/repos?username=...&base_url=...`(base_url省略時gitlab.com) — GitLabの公開プロジェクト一覧
- `GET /api/bitbucket/repos?workspace=...` — Bitbucketの公開リポジトリ一覧
- `GET /api/redmine/issues?base_url=...` — 標準Redmineの`/issues.json`
- `GET /api/gitbucket/repos?base_url=...&owner=...` — GitBucket(未検証)
- `GET /api/rs-chiketto/health` — RS-Chikettoの到達性確認
- `GET /api/priorities` / `POST /api/priorities {key, stars}` — プロジェクト優先度(星1〜5)の取得・設定
- `GET /api/generate?target=claude|claude_browser|cursor|chatgpt|gemini|deepseek|grok|generic&name=...&description=...&repo_url=...`
  — AIツール向けコンテキストファイル雛形を生成
- `POST /api/tickets/poll-now` — RS-Chikettoを即座にポーリングし、
  未対応チケットをタスクキューへ反映
- `GET /api/tickets/pending-tasks?target=<同上のいずれか>` —
  対象AIツール向けの未対応タスク依頼プロンプト一覧
- `POST /api/tickets/:ticket_id/handled/:target` — 対応済みとしてマーク

## インストーラー

Web UI(`/`)の「ダウンロード」節から、Linux向け`install.sh`・Windows向け
`install.ps1`をダウンロードできる。いずれもソースからのcargoビルドを行う
(署名済みバイナリ・deb/msiパッケージングは未実装、正直に開示)。

## 正直な開示(既知の制約)

- Gitbucketは検証可能な公開デモインスタンスが無いため未検証(コードは
  GitHub API v3互換仕様に基づいて実装済み)。
- ChatGPT/Gemini/DeepSeek/Grok/claude.aiは、各社固有のAPI連携(実際に
  APIキーで呼び出す機能)ではなく、ツールごとに宛先を明示した
  system-promptテキストを生成するところまでの対応。
- コード署名・deb/msiパッケージングは未実装。

## ロードマップ(2026-07-22追記、ユーザー構想・未実装)

以下はユーザーから示された今後の構想であり、**現時点ではいずれも未実装**
(設計・優先度の記録のみ)。今後の開発対象として明記する:

- **新ハードウェア導入対応**: サーバー/エッジ機器の新ハードウェアを
  エコシステムに組み込む際の手順・互換性チェックリストの自動生成。
- **ファームウェア対応**: 各ハードウェアのファームウェアバージョン管理・
  更新手順の追跡。
- **一部パーツ入れ替え対応**: サーバー機器の部分的な部品交換(ディスク・
  メモリ・NIC等)の際の作業手順・影響範囲の自動チェック。
- **セキュリティ新ハードウェア導入対応**: TPM・HSM・セキュリティキー等、
  セキュリティ関連の新ハードウェア導入時の手順・対応状況の管理。
- **WEBアプリ開発全体のロードマップ自動生成機能**: `runo-scanner`が検出
  したプロジェクト群のHANDOFF「次にすべきこと」を横断的に集計し、
  エコシステム全体としての開発ロードマップを自動生成する機能。
- ~~**Redmine/RS-Chiketto連携の拡張(チケット管理対応)**~~ → **2026-07-22
  実装済み**: RS-Chikettoのチケットを定期ポーリングし、未対応チケットを
  Claude/Cursor/汎用AI向けのタスク依頼プロンプトへ自動変換する機能を
  実装(詳細は上記「チケット自動対応機能」節参照)。標準Redmine REST API
  への対応は引き続き未着手。

これらは全て「未着手」であり、承認待ちの提案ではなく次に着手すべき実装
対象として記録している(このエコシステムの既存運用ルールに準拠)。

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
- **GitLab / Bitbucket / standard Redmine**: real HTTP integrations against
  each service's public REST API (GitLab v4, Bitbucket 2.0, Redmine's
  `/issues.json`) — implemented and verified live (2026-07-22).
- **Gitbucket**: implemented against its documented GitHub-API-v3-compatible
  shape, but not verified live (no public demo instance exists — point it at
  your own instance's `base_url` to confirm).
- **AI-agent context file generation** (2026-07-22: each tool is now a
  distinct, named target rather than one generic bucket):
  - `CLAUDE.md` template for Claude / Claude Code Desktop (implemented)
  - `.cursorrules` template for Cursor (implemented)
  - `AGENTS.md` templates for Claude (claude.ai / Claude in Chrome), ChatGPT,
    Gemini, DeepSeek, Grok, and any other generic tool (all implemented —
    this does not call the third-party's API, only generates tool-labeled text)

### Ticket automation (implemented 2026-07-22)

Polls RS-Chiketto (and, in future, standard Redmine) for open tickets and
auto-generates ready-to-use task prompts for Claude / Claude Code Desktop /
Cursor / ChatGPT / Gemini / DeepSeek / Grok.

**Design intent**: this is not "force-calling" a third-party chat UI (ChatGPT/
Gemini/DeepSeek/Grok) through an external API. The point is to **translate
RS-Chiketto/Redmine ticket data into the context format an AI tool actually
reads** (CLAUDE.md/.cursorrules/AGENTS.md-equivalent) — so any of those
tools can pick up the full picture of the issue tracker inside its own
session, without a human manually copy-pasting each ticket's contents. By
automating the "ticket → situational handoff to an AI tool" step, this
reproduces the experience of "a ticket comes in and gets handled
automatically."

### Local usage

```bash
cargo build --release
RS_OPS_SCAN_ROOT=/path/to/your/repos ./target/release/rs-ops
# open http://127.0.0.1:8097/
```

### Honest disclosures (known limitations)

- Gitbucket is unverified live (no public demo instance exists to test
  against); the code is implemented against its documented API shape.
- ChatGPT/Gemini/DeepSeek/Grok/claude.ai support means generating a
  tool-labeled system-prompt template — not a per-vendor API integration.
- No code signing or deb/msi packaging yet.

### Roadmap (added 2026-07-22, user vision, not yet implemented)

The following were proposed by the user as future direction; **none of these
are implemented yet** — this section records the design intent and priority
only:

- **New hardware onboarding support**: auto-generated compatibility
  checklists / procedures for introducing new server or edge hardware into
  the ecosystem.
- **Firmware support**: tracking firmware versions and update procedures
  across hardware.
- **Partial parts-replacement support**: procedures and blast-radius checks
  for partial component swaps (disks, memory, NICs, etc.) on server
  hardware.
- **Security-hardware onboarding support**: tracking TPMs, HSMs, security
  keys, and other security-related hardware introductions.
- **Auto-generated web-app development roadmap**: aggregate the "next
  steps" from every project's HANDOFF (as detected by `runo-scanner`) into
  a single, ecosystem-wide development roadmap.
- ~~**Expanded Redmine/RS-Chiketto integration (ticket management)**~~ →
  **implemented 2026-07-22**: polls RS-Chiketto tickets and converts
  unhandled ones into task prompts for Claude/Cursor/generic AI targets
  (see "Ticket automation" above). Standard Redmine REST API support is
  still not started.

All of the above are "not yet started" — per this ecosystem's existing
operational policy, that means they are recorded as the next implementation
targets, not proposals awaiting approval.
