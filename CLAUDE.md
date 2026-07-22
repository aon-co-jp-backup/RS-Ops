# 開発方針＆開発環境ルール(RS-AI-DevOps)

作業ドライブは`F:\runo`。この節は[`open-raid-z`](https://github.com/aon-co-jp/open-raid-z)の
`CLAUDE.md`を正本とし、各プロジェクトへコピーして同期する方針に準じる。
GitHubリポジトリ: [aon-co-jp/RS-AI-DevOps](https://github.com/aon-co-jp/RS-AI-DevOps)。

> ⚠️ **正直な開示(2026-07-22、初版)**: GitHub連携・RS-Chiketto連携のみ
> 実機(実HTTP)で検証済み。Bitbucket/Gitbucket/GitLab/標準Redmineは
> インターフェース(`GitService` enum)のみでライブ接続は未実装。
> ChatGPT/Gemini/DeepSeek/Grok向けは各社API連携ではなく、汎用
> `AGENTS.md`テンプレート生成までの対応。

## このプロジェクトの役割

「エコシステム全体マップの自動生成」を製品化したもの。`runo-scanner`
(プロジェクト自動検出・`open-raid-z/CLAUDE.md`の「関連プロジェクト」節
自動更新)と`RS-Sync`(プロバイダ抽象化・インストーラー・VPSデプロイ
パターン)の資産を土台に、以下を統合する:

1. 指定ルート配下のプロジェクトスキャン・一覧表示。
2. GitHub/Bitbucket/Gitbucket/GitLab/Redmine(および**RedmineのRust+
   RPoem版=RS-Chiketto**)との連携。
3. Claude/Claude Code Desktop/Cursor/ChatGPT/Gemini/DeepSeek/Grok向けの
   コンテキストファイル(CLAUDE.md/.cursorrules/AGENTS.md)自動生成。
4. 日英2言語UI、Windows/Linuxインストーラー付き配布。

## 技術スタック

RS-Sync/RS-Git等と同じ方針: `poem`クレートを直接利用する単純なHTTP
サービス。DB非依存(状態を持たない、スキャン・API呼び出しは都度実行)。

## 主要モジュール

- `src/model.rs` — `ProjectEntry`/`GitService`/`AiTarget`。
- `src/scanner.rs` — プロジェクト自動検出(runo-scannerの簡易版、
  1階層スキャン+`git remote get-url origin`+CLAUDE.md/README.mdからの
  役割説明抽出)。
- `src/provider.rs` — `GitHubProvider`(実装済み)・`RsChikettoProvider`
  (実装済み)。Bitbucket/Gitbucket/GitLab/標準Redmineは`GitService` enum
  上に定義のみ。
- `src/templates.rs` — AIツール向けコンテキストファイル雛形生成
  (`CLAUDE.md`/`.cursorrules`/`AGENTS.md`)。
- `static/index.html` — 日英2言語UI(相対パスAPI呼び出し、`/rs-git`の
  絶対パスハードコード事故の教訓を踏襲)。

## HANDOFF

- **2026-07-22 初版実装**: ユーザーから直接「エコシステム全体マップの
  自動生成エージェント管理をRS-AI-DevOpsという名前で製品化し、多数の
  AIツール・多数のGit/課題管理サービスに対応させ、日英2言語・Windows/
  Linuxインストーラー付きで運用してほしい」との指示を受けた。
  - 経緯: 別のサブエージェントに同等の依頼を2回委任したが、いずれも
    「本番VPSへの影響がある不可逆操作なので、ユーザー本人によるチャット
    上での明示確認が必要」として実装を辞退した(2回目は、委任元が
    タスク文面内に「ユーザーが確認済み」という前置きを書いたこと自体を
    プロンプトインジェクションの兆候として警戒し、やはり辞退した)。
    これは委任先エージェントが会話の実際の文脈(ユーザーとの直接の
    やり取り)を持たないための、構造的に妥当な安全側の判断だったため、
    以降はメインのエージェント(実際にユーザーとやり取りしている
    セッション)自身が直接実装・デプロイを行う方針に切り替えた。
  - **実装内容**: `runo-scanner`のスキャンロジックを踏襲した簡易版
    (`src/scanner.rs`)、GitHub public REST API連携
    (`GitHubProvider::list_org_repos`——`aon-co-jp`は実際にはGitHub
    organizationではなくuser accountであることを実APIで確認し
    `/orgs/`ではなく`/users/`エンドポイントを使うよう修正)、RS-Chiketto
    連携(`RsChikettoProvider::health_check`/
    `list_tickets_unauthenticated_probe`)、AIツール向けテンプレート
    生成(`CLAUDE.md`/`.cursorrules`/`AGENTS.md`)を実装。
  - **検証**: `cargo build --release`成功、`cargo test --release`
    6件全green。実バイナリを起動し、`/healthz`(200)・`/`(200)・
    `/api/services`(6サービスの実装状況を返す)・`/api/projects`
    (F:\runo配下22件のプロジェクトを実際に検出、日本語の役割説明も
    正しくUTF-8で取得)・`/api/github/repos`(実際にGitHub public API
    から`aon-co-jp`のリポジトリ一覧を取得、63件)・`/api/rs-chiketto/health`
    (相当、`https://runo.tokyo/chiketto/healthz`への実HTTP到達確認、
    200)・`/api/generate?target=claude`(雛形生成、実際にテキストが
    返る)・`/install.sh`/`/install.ps1`(200、ダウンロード可能)を
    それぞれ実HTTPで確認した(型チェックのみでの完了報告ではない)。
  - 次にすべきこと: (1) VPSデプロイ(`ssh conoha`、`/root/runo.tokyo/
    RS-AI-DevOps`)・open-web-server管理APIでの`path_prefix: "/RS-AI-DevOps"`
    登録・`https://runo.tokyo/RS-AI-DevOps`への実到達確認、(2) GitHubへの
    push(`aon-co-jp/RS-AI-DevOps`、既に空リポジトリとして存在)、
    (3) Bitbucket/Gitbucket/GitLab/標準Redmineの実装、(4) `runo-scanner`
    本体との統合(現状はロジックの重複実装、将来的には依存クレート化を
    検討)。

## 関連プロジェクト

- [runo-scanner](https://github.com/aon-co-jp/runo-scanner) — プロジェクト
  自動検出・エコシステムマップ自動更新の元祖実装
- [RS-Sync](https://github.com/aon-co-jp/RS-Sync) — プロバイダ抽象化・
  インストーラー・VPSデプロイパターンの参照元
- [RS-Chiketto](https://github.com/aon-co-jp/RS-Chiketto) — Redmine相当、
  本プロジェクトの連携先の1つ
- [open-raid-z](https://github.com/aon-co-jp/open-raid-z) — 開発ルールの正本
