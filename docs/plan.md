# wintab-rs 開発計画

更新日: 2026-10-07
前提: [機能要件](requirements.md)は2026-10-07にユーザー承認済み。CI/CD・ビルド環境と担当分担は確定方針。技術方式は動作実証済みではなく、検証結果に応じて更新する。

## 1. 最小構成の方針

Rustの単一バイナリを最小の`.app`バンドルに収める。ロジック、状態管理、履歴、エラー処理は標準ライブラリを使う。macOS固有機能はAppKit、Core Graphics、Application ServicesのAccessibility API、Core Foundationで扱う。

Rust標準ライブラリだけではmacOSのGUI・入力介入・他アプリのウィンドウ操作は扱えない。小さなC API面は限定的なFFI、Objective-C連携は必要な範囲のバインディングを候補とする。依存ゼロ自体を目的にして大量のunsafeコードや独自Objective-Cランタイム層を作らない。具体的なcrateとfeatureは最小実証時に選定し、採用理由を記録する。

最初からworkspace、プラグイン構造、汎用イベント基盤を作らない。入力、ウィンドウ操作、UIの境界は小さく保ち、必要になった分だけファイルを分ける。

### 担当分担

- GPT-6.1 Sol: 仕様整理、計画、作業分割、必要に応じた実装担当への指揮、レビュー、各フェーズの完了判断を担当する。要件変更の最終判断はユーザーに求める。
- GPT-6 Luna: 指示された実装、テストの作成、GitHub Actionsの整備、検証結果に基づく修正を担当する。

実装開始は承認済み。現段階ではフェーズ1のCI・ビルド基盤と公開APIを確認する診断用PoCを実装する。

### CI/CDとビルド

CI/CDとアプリのビルドはGitHub Actions上で実行し、ローカルではビルドしない。CIでは`cargo fmt --check`、`cargo check`、`cargo test`を実行する。初期workflowはmacOS 27／arm64の`xcode-27` runner（public preview）を採用し、`aarch64-apple-darwin`のreleaseバイナリを最小の`.app`バンドルに収め、ZIP形式のActions成果物として保存する。runnerの提供状況は変わり得るため、実行時のOS・SDKと結果を確認する。[GitHub: macOS 27 runnerの案内（2026-09-10）](https://github.blog/changelog/2026-09-10-xcode-27-runner-image-now-runs-on-macos-27/)、[GitHub-hosted runners](https://docs.github.com/en/actions/reference/runners/github-hosted-runners)

Actionsの成果物を対象のmacOS 27／Apple Silicon実機へ取得し、手動で起動・権限設定・動作・性能を検証する。runner上の自動チェックだけで、実機のGUI・権限・Space切り替えの成立を判定しない。

初期の受け渡しはActions成果物を基本とする。タグを契機とするGitHub Releasesへの配布、公開範囲、配布用の署名・公証は未決定であり、必要性と運用を提案してから決める。公開リリースを自動作成しない。実機起動に必要な署名方式と権限への影響は最小実証で確認する。

## 2. 技術方針と未検証点

### 入力介入

アクティブフィルタとしてCore Graphicsイベントタップを使用し、対象イベントだけを抑止する。セッション側のタップを候補とし、通常ユーザーの権限でCommand+Tabを標準UIより先に扱えることを最初に確認する。NSEventのグローバルモニターはイベントを抑止できないため、この用途の主経路にしない。[Apple: イベントタップ](https://developer.apple.com/documentation/coregraphics/cgevent/tapcreate(tap:place:options:eventsofinterest:callback:userinfo:)), [コールバック](https://developer.apple.com/documentation/coregraphics/cgeventtapcallback), [イベントモニター](https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/EventOverview/MonitoringEvents/MonitoringEvents.html)

コールバックはキー判定と短い状態更新・通知に限定する。AppKitのUIはメインスレッドで処理し、遅いAX問い合わせは入力経路から分離する。必要な場合だけ標準スレッドとチャネルを使う。待ち行列を無制限に増やさない。

### ウィンドウ列挙・識別・前面化

AXUIElementを候補列挙、タイトル取得、対象操作の主経路とし、CGWindowListCopyWindowInfoを前面順などの補助情報にする。全Spaceを対象とするため、画面上のウィンドウだけを返す`optionOnScreenOnly`で候補を絞らない。全件取得とAX列挙を検証し、最小化・非表示・フルスクリーンが欠落しないことを確認する。CGの前面順も正確なMRU履歴そのものではない。[Apple: optionOnScreenOnly](https://developer.apple.com/documentation/coregraphics/cgwindowlistoption/optiononscreenonly)

AX要素とCGWindowIDを公開APIだけでどう対応付けるかは最大の検証項目とする。PID、位置、サイズだけの照合は同位置・同サイズのウィンドウを誤認し得る。タイトル一致だけにも依存しない。公開APIで十分な同定ができない場合は、非公開APIを黙って追加せず、対象範囲や方式の変更をユーザーに相談する。

対象アプリのアクティブ化とAXによるウィンドウのraise／focus操作を組み合わせる案を検証する。アプリのアクティブ化だけで完了とせず、同じアプリ内の指定ウィンドウが実際に入力先になることを確認する。

macOS 27／Apple Silicon上のChromeを最優先に、異なるプロファイル・同一プロファイルの複数ウィンドウ・同名タイトルを検証する。最小化解除や非表示解除は確定時だけ行う。別Space／フルスクリーンへの移動は公開APIによる対象ウィンドウのアクティブ化で成立するか実測し、Space切り替えに関するmacOS設定への依存も記録する。非公開のSpace制御APIやキー入力の疑似送信を前提にしない。

全Space対応は確定要件なので、成立しない場合に現在のSpaceだけへ黙って縮小しない。再現条件、利用可能な公開API、必要な設定変更や仕様の選択肢を示して判断を求める。

CG経由の他アプリのウィンドウ名には画面収録権限による制限があるため、タイトル取得はAXを優先する。既存のApple説明はCatalina時点のものなので、対象OSで権限別の挙動を再確認する。[Apple: ウィンドウメタデータと権限](https://developer.apple.com/videos/play/wwdc2019/701/?time=1460)

### 履歴と一覧

AXのフォーカス変更通知とアプリの起動・終了・アクティブ化通知を使い、稼働中のMRUを保持する案とする。通知非対応アプリでは切り替え開始時の再取得で補完し、完全な履歴を推測したことにしない。通知登録が不要になった時点で解除する。

一覧はAppKitの非アクティブ化パネルを候補にする。元アプリのフォーカスを奪わずに選択を描画できることを確認する。表示先は操作開始時のアクティブウィンドウがあるディスプレイ、取得不能時はメインディスプレイとする案。文字は標準コントロールで表示し、長い一覧は選択行が見える範囲にスクロールする。

## 3. 開発手順

### フェーズ0: 仕様の決定

- ユーザー回答を要件に反映する。
- 使用OS・CPU、主要アプリ、ディスプレイとSpaceの運用を確定する。
- 最小実証の制約が見つかった場合だけ、追加の判断を求める。

完了条件: 初期版の候補範囲と表示方式が決まっている。2026-10-07に機能要件は承認済みで、macOS 27／Apple Silicon、全ウィンドウ、文字一覧、Chrome優先は決定済み。ディスプレイ等の利用条件は該当する実機検証前に確認する。

### フェーズ1: 実現性の最小実証

最初の縦切りはActions成果物の生成、Accessibility権限とAX APIへのアクセス、入力を抑止しないイベントタップの診断までとする。診断用PoCは完成版の切り替えアプリではない。以下の入力抑止・個別前面化・全Space対応の実機検証が終わるまで、フェーズ1を完了扱いにしない。

現状: 診断用PoCとworkflowの初期実装段階。既存のActions成果物を使った起動確認は下記に記録した。起動時に未許可のAccessibility・入力監視権限を要求する処理を追加し、現在のプロセスの許可状態と実行場所、設定後の再起動手順を表示する。この変更のCI結果と、権限要求・許可後のAX応答・入力監視の実機検証は未確認。

権限要求は公開APIの`AXIsProcessTrustedWithOptions`（`kAXTrustedCheckOptionPrompt = true`）と`CGRequestListenEventAccess`を使う。Accessibility要求は非同期で、その戻り値は要求時点の許可状態を示す。CLIは許可を待って常駐せず、未許可なら設定と再起動を案内する。許可を拒否されたと即断せず、設定がONでも未許可なら実行場所・登録対象・ad-hoc署名変更を確認する。[Apple: Accessibility権限要求](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions)、[入力監視権限要求](https://developer.apple.com/documentation/coregraphics/cgrequestlisteneventaccess())

1. 最小のRustプロジェクトとGitHub ActionsのCIを用意し、fmt／check／testとarm64 releaseの`.app`生成・アーカイブ保存までの基盤を先に整える。
2. 最小限のmacOSバインディングを追加し、安定したバンドルIDの最小`.app`をActionsで生成する。実機の起動場所を定め、成果物の起動と権限の付与先を確認する。
3. イベントタップでCommand+Tabを検出・抑止し、停止後に標準動作が戻ることを確認する。
4. 対象アプリの複数ウィンドウを列挙し、指定した1枚を前面化して入力できることを確認する。
5. ChromeのプロファイルA／Bと同一プロファイル内の複数ウィンドウで、同名・無題・同位置の識別とAX/CGの対応付けを検証する。
6. 別Space・フルスクリーンへの遷移、最小化・非表示からの復元を含む個別前面化を検証する。
7. Accessibilityのみ、追加の入力監視権限あり、画面収録権限なしの各条件を記録する。

完了条件: GitHub Actionsで自動チェックと`.app`成果物の生成が成功し、その成果物を使って対象実機の主要アプリで入力介入・個別前面化・対象範囲の判定が成立する。成立しなければ、UIを作り込む前に制約と代案を提示する。

### フェーズ2: 切り替えの動作

- 待機、選択中、一時停止の小さな状態機械を実装する。
- 正逆の循環、Command解放、取消、キーリピートと左右修飾キーを扱う。
- MRU、選択中の候補順固定、消滅候補と失敗時の終了を実装する。
- タップ処理とAX処理を分離し、通信の時間上限と無効化時の状態破棄を実装する。

完了条件: UIなしの検証段階でも、指定ウィンドウの切り替えと失敗時の入力復帰が安定している。

### フェーズ3: 最小UIと常駐

- 文字一覧、選択表示、長いタイトルの省略、空タイトル表示を実装する。
- メニューバーの状態、有効／一時停止、権限案内、終了を追加する。
- フェーズ1のActionsによるバンドル生成をUI・常駐機能に対応させ、成果物の取得・配置・手動起動の手順を整える。開発中の署名変更による権限再設定も確認する。

完了条件: ターミナルを開かず起動・停止でき、通常の切り替え操作が完結する。

### フェーズ4: 日常利用の検証

- 主要アプリで受け入れ条件を確認する。
- スリープ／ロック復帰、権限取消、タップ無効化、終了時の入力復帰を確認する。
- Actionsで生成したrelease成果物を対象実機で起動し、アイドルCPU、RSS、切り替え遅延を測る。
- READMEに導入、権限、終了方法、対応環境、既知の制約を記載する。

完了条件: 要件の受け入れ条件を満たし、残る制約がユーザーの利用範囲で許容されている。

## 4. 検証の範囲

純粋なRustの選択ロジックには標準の`cargo test`を使う。循環、逆方向、0件／1件、MRUの往復、取消、選択候補の消滅を少数のテストで確認する。巨大なモック基盤は作らない。

macOSの権限・イベント抑止・実ウィンドウの入力フォーカスは、Actions成果物を使ってmacOS 27／Apple Silicon実機で確認する。手動検証では対象コミット・Actionsの実行、OS、アプリ名、操作、期待結果、実結果を短く記録する。候補の見た目だけで前面化成功と判定しない。

GitHub Actions上で`cargo fmt --check`、`cargo check`、`cargo test`を実行し、アプリのビルドも同環境で行う。診断用PoCの実装だけで実機動作を検証済みとはしない。Actionsの実行結果と実機での確認結果は区別して記録する。

### 手動確認記録

- 2026-10-07、Actions成果物を起動し、arm64のMach-O、Info.plist、ad-hoc署名を確認した。
- 引数なしの診断コマンドは起動し、Accessibility未許可を表示した。
- `--tap-seconds 5`はイベントタップ作成時にInput Monitoring権限がないとして終了した。権限設定は変更していない。
- その後ユーザーから、設定をONにしているはずでもエラーが表示されると報告を受けた。旧PoCはAccessibilityの状態確認だけを行い、入力監視はタップ作成失敗から権限不足を推測していた。起動時の権限要求と個別の許可状態の表示を追加するが、実際の設定対象やエラーの原因は未確定。
- 権限要求後も`AXIsProcessTrusted`はfalseで、イベントタップも作成できなかった。設定がONという報告との不一致は解消していない。この結果だけで権限設定の対象や署名が原因と断定しない。
- AX件数の取得とGUI動作は未確認。切り替え機能自体もまだ実装されていない。

## 5. 追加判断が必要になる条件

- 確定要件である全Space・フルスクリーン対応が公開APIで成立しない、またはOS設定に依存する場合。
- 主要アプリで公開APIによる正確な候補識別やフォーカスが成立しない場合。
- サムネイルや画面収録権限が必要になる場合。
- MRU通知の制約で、要求する選択順を維持できない場合。
- 測定結果から、性能目標と単純な実装の間で優先順位を決める必要がある場合。

将来機能のための仕組みは先に作らず、実際に必要になった機能だけを追加する。
