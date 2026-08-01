# 開発方針＆開発環境ルール(RS-Ops)

作業ドライブは`F:\runo`。この節は[`open-raid-z`](https://github.com/aon-co-jp/open-raid-z)の
`CLAUDE.md`を正本とし、各プロジェクトへコピーして同期する方針に準じる。
GitHubリポジトリ: [aon-co-jp/RS-Ops](https://github.com/aon-co-jp/RS-Ops)
(旧名`RS-AI-DevOps`、2026-07-22に`RS-Ops`へ改名)。

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

## ロードマップ(2026-07-22追記、ユーザー構想・未実装、日英併記README参照)

ユーザーから追加で以下の構想が示された(いずれも**現時点で未実装**、
次に着手すべき実装対象として記録):

1. 新ハードウェア導入対応(New hardware onboarding support)
2. ファームウェア対応(Firmware support)
3. 一部パーツ入れ替え対応(Partial parts-replacement support)
4. セキュリティの新ハードウェア導入対応(Security-hardware onboarding
   support)
5. WEBアプリ開発全体のロードマップ自動生成機能(各プロジェクトの
   HANDOFF「次にすべきこと」を横断集計してエコシステム全体の開発
   ロードマップを自動生成する機能。Auto-generated web-app development
   roadmap)
6. Redmine/RS-Chiketto連携の拡張——現状のヘルスチェックのみから、実際の
   チケット作成・更新・一覧表示までの課題管理連携へ拡張(Expanded
   Redmine/RS-Chiketto integration for real ticket management)

詳細な日英併記の説明は`README.md`の「ロードマップ」節を参照。

## HANDOFF

- **2026-07-23(続き) `.github/workflows/release.yml`を新規追加、
  3点セット完備・v0.1.0タグでCI成功・GitHub Release実在確認まで完了**:
  エコシステム全体インストーラー整備計画(正本: `open-raid-z/CLAUDE.md`
  「エコシステム全体インストーラー整備計画」節)の一環。`install.sh`/
  `install.ps1`は既存(ソースからのcargoビルド方式、変更無し)だった
  ため、`release.yml`のみ新規追加した。Linux x86_64・Windows x86_64
  向けにプリビルド済みバイナリをビルドし、`install.sh`/`install.ps1`と
  一緒にtar.gz/zipへ同梱、GitHub Releasesへ`softprops/action-gh-release@v2`
  で添付する構成(sibling path依存は無し、確認済み)。`v0.1.0`タグを
  実際にpushし、`gh run list`で2ジョブ(Linux/Windows)とも
  `completed success`、`gh release view v0.1.0`で
  `rs-ops-linux-x86_64.tar.gz`/`rs-ops-windows-x86_64.zip`の両方が
  実在することを確認した(型チェックのみでの完了報告ではない)。
  README(日本語「インストーラー」節)にもプリビルド済みバイナリの
  入手先を追記。
  - 次にすべきこと: Android版インストーラー(未着手、他リポジトリと
    共通のバックログ)。

- **2026-07-22(続き4) runo.tokyo側でRS-Ops(RS-Git/aruaru-llmとのSET)を
  「プロジェクトシリーズ索引」+「エコシステム全体を見る」節で紹介**
  (ユーザー指示「RS-Ops/RS-Git/aruaru-llmのSETが完成したら
  easy-web.tokyo/RS-Opsで運用、runo.tokyo/RS-Opsで紹介」への対応、
  実際の作業は`runo.tokyo`リポジトリ側)。
  1. 前回パス(続き3以前)で実装済みだった`src/provider.rs`の
     `RsGitProvider`(RS-Git `GET /api/repos`)・`AruaruLlmProvider`
     (aruaru-llm `/healthz`・`/v1/chat`)、`src/model.rs`の
     `GitService::RsGit`、`src/main.rs`の`/api/rs-git/repos`・
     `/api/aruaru-llm/health`ルートが、実際にファイルへ保存されている
     ことをソース読み込みで再確認した(前回セッションの要約通り、
     この session でも`Shell`ツールが応答不能だったため
     `cargo build`/`cargo test`は実行できなかったが、コード自体は
     `bail!`/`Result`のimport済み・`RepoSummary`の構造体リテラルの
     フィールド一致・ルート登録の並びを目視で確認し、明らかな
     コンパイルエラーは見当たらなかった)。
  2. `runo.tokyo`側で当初「/RS-Ops」という専用サブサービスページ・
     ルートを追加したが、ユーザーから「プロジェクトシリーズ　と　
     エコシステム全体を見るの中で紹介して」との指示を受け、専用ルート
     は撤回。代わりに`runo.tokyo`の`src/meta_index.rs::PROJECTS`に
     RS-Opsエントリ(RS-Git/aruaru-llmとのSET連携である旨を記載)を
     追加し、TOPページの「エコシステム全体を見る」節
     (`render_runo_link_section`)にもRS-Ops・RS-Git・aruaru-llmへの
     直接リンクを追記した。詳細は`runo.tokyo/CLAUDE.md`の同日HANDOFF
     参照。
  3. **easy-web.tokyo/RS-Opsでの本体運用は未着手**。これはRS-Opsの
     コード変更だけでは実現できず、(a) VPS上でRS-Opsをビルド・
     systemdサービス化、(b) `easy-web.tokyo`のnginx 443番server
     ブロックに`location /RS-Ops/`のproxy_pass追加、が必要
     (`RS-Git`が`runo.tokyo/rs-git`で運用されている際と同じパターン)。
     このセッションではSSH/VPSアクセスを行っておらず、次回セッションで
     の実施が必要。
  - 次にすべきこと: (1) Shellツール復旧後、RS-Ops・runo.tokyo両方で
    `cargo build`/`cargo test`を実行し今回までの変更を検証、
    (2) `easy-web.tokyo`へのRS-Opsデプロイ(systemdサービス化+
    nginx `location /RS-Ops/`追加)、(3) デプロイ完了後、
    `RsGitProvider`/`AruaruLlmProvider`を実際に稼働中のRS-Git/
    aruaru-llmインスタンスに対して実HTTPで検証(現状は未検証のまま)、
    (4) デプロイ完了後、`runo.tokyo`の`meta_index.rs`の`RS-Ops`
    エントリの`live_url`/`live_status`を`https://easy-web.tokyo/RS-Ops`
    に更新。

- **2026-07-22(続き3) Bitbucket/GitLab/標準Redmineを実装、AIターゲットを
  個別命名、優先度UIに正直な注記を追加**: ユーザーから「Git/課題管理
  サービスを完全に対応して」「呼び出しはClaude/Claude Code Desktop/
  Claude(ブラウザ)/ChatGPT/Gemini/DeepSeek/Grokに完全に対応して」
  「星が一つもついていないか、星一つは、今は対応しなくて良いと
  判断致します、と英語と日本語で書いておいて」との3点の指示を受けて対応。
  1. **`src/provider.rs`**: `GitLabProvider`(`/api/v4/users/:user/projects`)・
     `BitbucketProvider`(`/2.0/repositories/:workspace`)・
     `RedmineProvider`(`/issues.json`)を新規実装。いずれも認証無しの
     公開APIで、実バイナリを起動して実HTTPで検証済み——GitLabは
     `gitlab.com`の実ユーザー`sytses`のプロジェクト一覧、Bitbucketは
     `bitbucket.org`の`atlassian`workspace、Redmineは`www.redmine.org`
     自身の`/issues.json`(Redmine自身がRedmineで運営されているため、
     実在の公開Redmineインスタンスとして使えた)。`GitbucketProvider`も
     実装したが、**GitBucketには検証可能な公開デモインスタンスが
     存在しないため、この session ではライブ検証できていない**——
     GitBucket公式が謳う「GitHub API v3互換」の形状に基づいて書いた
     コードである旨を`GitService::implementation_status`・
     `provider.rs`のdocコメント両方に正直に明記した(嘘をつかない
     ため、ここだけ「未検証」を隠さない)。
  2. **`src/model.rs`の`AiTarget`拡張**: 従来`Claude`/`Cursor`/`Generic`
     の3種類だったところを、`ClaudeBrowser`(claude.ai/Claude in Chrome)・
     `ChatGpt`・`Gemini`・`DeepSeek`・`Grok`を個別のenum variantとして
     追加(8種類に拡張)。出力フォーマット自体はGeneric系(AGENTS.md
     相当のプレーンテキスト)を共有するが、API/UI上で各ツール名を
     明示的に選択・区別できるようにした。**正直な開示は変わらない**:
     これら第三者チャットUIへ本サービスからAPIを強制呼び出しする
     機能は無い(そのAPI/セッションを保有していないため)、個別化は
     「宛先ラベル付きのテキストを分けて生成・提示できる」ところまで。
     `AiTarget::all()`を追加し、`ticket_automation`の全ターゲット
     走査・テストのハードコードされた3要素配列をこれに置き換えた。
  3. **UI(`static/index.html`)**: 「対応AIツール」表を8行に展開
     (各ツール個別表示)。第三者チャットUIへAPIを強制呼び出しできない
     旨の注記(`ai_note`、日英両方)を追加。「検出されたプロジェクト」
     節にも、星0または星1が「今は対応しなくて良い」を意味する旨の
     注記(`priority_note`、ユーザー指定の日本語文言をそのまま採用、
     英語は同義訳)を追加。
  - **検証**: `cargo build`/`cargo test`(17件全green、新規2件は
     `templates`モジュール——8ターゲット全走査・第三者ツール名が
     出力に反映されること)。`cargo clippy --all-targets`は既存の
     3件の警告(いずれも今回変更していないコード、pre-existing)のみで
     新規警告0件。実バイナリを起動し`/api/gitlab/repos`・
     `/api/bitbucket/repos`・`/api/redmine/issues`をそれぞれ実HTTPで
     確認(上記1参照)、`/api/services`のレスポンスで各サービスの
     ステータス文言が更新されていることも確認。
  - 次にすべきこと: (1) GitBucketの実機検証(自前でGitBucketインスタンス
    を立てて`base_url`パラメータ経由で確認)、(2) VPS本番への再デプロイ、
    (3) UIの「検出されたプロジェクト」節で星1(=対応不要)のプロジェクトを
    折りたたみ/フィルタ表示するUI改善。

- **2026-07-22(続き2) 優先度(星1〜5評価)機能を実装**: ユーザーから
  「リポジトリやURLなどで、優先順位を星5段階で評価出来る機能を持たせて。
  星1は、今は開発などをしなくて良いと言う意味」との指示を受けて実装。
  1. `src/priority.rs`新設: `PriorityStore`(JSON永続化、`data/priorities.json`、
     `ticket_automation::TicketQueue`と同じ設計パターン)。キーは
     プロジェクトの`path`(無ければ`github_url`)。星は1〜5の範囲外は
     `set()`がErrを返し拒否する。
  2. `model::ProjectEntry`に`priority: Option<u8>`フィールドを追加
     (スキャン直後はNone、`GET /api/projects`ハンドラで`PriorityStore`
     から突き合わせて合成——優先度自体はスキャン結果ではなくユーザー
     設定値なので、スキャンとは別ファイルに永続化する設計)。
  3. 新規API: `POST /api/priorities` (`{key, stars}`)・
     `GET /api/priorities`(全件のkey→星のマップ)。
  4. `static/index.html`: プロジェクト一覧テーブルに星評価列を追加。
     ★をクリックするとその場でPOSTし、一覧を再読込して反映。
  - **検証**: `cargo test`(20件全green、新規4件は`priority`モジュール
    ——設定/取得の往復・範囲外拒否・永続化・全件取得)。実バイナリを
    起動し`POST /api/priorities`→`GET /api/projects`で実際に
    `priority`フィールドが反映されることをHTTPで確認、範囲外の値
    (0や9)が拒否されることも確認。
  - 次にすべきこと: (1) VPS本番への再デプロイ、(2) 星1(=着手不要)の
    プロジェクトを一覧上部/下部でグルーピング・フィルタ表示する
    UI改善、(3) ユーザーから示された他の未実装ロードマップ項目
    (新ハードウェア対応・ロードマップ自動生成機能等)。

- **2026-07-22(続き) チケット自動対応機能を実装、および設計意図の表現を
  ユーザー指摘により訂正**:
  1. `src/ticket_automation.rs`新設: RS-Chikettoを定期ポーリング
     (既定120秒、`POST /api/tickets/poll-now`で即時実行も可)し、
     未対応(Closed以外・未処理)チケットをClaude/Cursor/汎用AI向けの
     タスク依頼プロンプトへ変換してキュー(JSON永続化)へ登録する。
     `GET /api/tickets/pending-tasks?target=...`・
     `POST /api/tickets/:id/handled/:target`を追加。
  2. **表現の訂正(ユーザー指摘、2026-07-22)**: 当初「ChatGPT/Gemini/
     DeepSeek/Grokのような第三者チャットUIを強制的に呼び出せないのが
     制約」という説明をしたが、これは設計意図を正しく表していない
     という指摘を受けた。正しくは: **RS-Chiketto/Redmineのチケット
     情報をAIツールが実際に読み込む形(CLAUDE.md/.cursorrules/
     AGENTS.md相当)へ変換して提示することで、Claude Code Desktop等の
     AIツール自身が全体を把握し対応できるようにする**、という設計を
     再現しているのであり、これは制約ではなく本機能の狙いそのもの。
     `src/ticket_automation.rs`のモジュールdoc・README.md(日英)の
     該当箇所をこの理解に合わせて修正した。
  3. **検証**: `cargo build`/`cargo test`(12件全green、新規5件は
     `ticket_automation`モジュール——取り込み・重複防止・Closed除外・
     対応済みマーク・永続化の往復)。実バイナリを起動し
     `POST /api/tickets/poll-now`・`GET /api/tickets/pending-tasks`を
     実HTTPで確認。VPS本番(`https://runo.tokyo/chiketto`実接続、
     現状チケット0件のため取り込みも0件だがエラー無し)へのデプロイ・
     全既存パス(`/`・`/blog`・`/chiketto`・`/ec`・`/rs-git/ui/`・
     `/rs-sync/`・`/RS-AI-DevOps`)の継続動作を確認済み。
  - 次にすべきこと: (1) RS-Chikettoに実際にテストチケットを作成した
    上でのフルE2E検証(現状は0件チケットでの接続確認のみ)、
    (2) 標準Redmine REST APIでの同機能実装、(3) Claude Code Desktop等
    から実際にこのキューを参照して対応する実運用フローの確立。

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

## HANDOFF追記(2026-07-31) インストーラーの電源プロファイル選択機能(未実装、エコシステム標準方針として記録)

`open-raid-z`のCLAUDE.md(全リポジトリ共通の設計思想セクション)に、
インストーラー(`install.sh`/`install.ps1`等)実行時に以下3つの電源
プロファイルを選択させる標準方針を追記した(ユーザー指示、2026-07-31):

1. **省電力(Power-saving)**: CPU使用率・ポーリング間隔を抑えた低負荷設定。
2. **省メモリ(Low-memory)**: メモリ確保量・キャッシュサイズを抑えた設定。
3. **常時電源接続(Always-on)**: 上記の抑制を行わないフル性能設定。
   **この場合のみ**ハードウェアアクセラレータ(NPU/GPU)のサポートを
   自動検出・自動有効化する(`open-cuda`の`GpuDevice`抽象化を利用)。

**正直な開示**: このリポジトリのインストーラーへの実装はまだ未着手。
実装時は`open-raid-z/CLAUDE.md`の該当節、および先行実装予定の
`open-redmine/CLAUDE.md`を参照し、`open-cuda`側のGPU/NPUベンダー検出
ロジックを再利用すること(車輪の再発明を避ける)。
- 次にすべきこと: このリポジトリの`install.sh`/`install.ps1`に上記3
  プロファイルの選択機能を追加する。
