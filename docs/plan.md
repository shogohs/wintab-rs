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

実装開始は承認済み。フェーズ1の権限・listen-only診断を確認後、2026-10-07にユーザーから次のフェーズ開始の指示を受けた。まず入力抑止と指定ウィンドウの前面化を診断できる最小単位を実装し、フェーズ2の入力状態処理へつなげる。

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

現状: 診断用PoCはActionsでfmt／check／test／アプリのパッケージ作成まで成功し、実機で両権限、AXウィンドウ取得、listen-onlyイベントタップの動作を確認した。captureの正方向・commitカウントとChromeの2ウィンドウへのAXRaise・前面化・FocusedWindow属性の一致も確認した。入力抑止の終了後の復帰、実入力先、全Spaceでの個別前面化は未確認。単一PIDの有限時間切り替えを次の実装単位とし、通常利用の切り替えアプリはまだ完成していない。

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

開始済み。最初の実装単位は、明示的なCLI引数で最大10秒だけ動く`--capture-seconds`と、`--list-windows PID`／`--raise-window PID INDEX`の診断とする。Lunaが実装と純粋な入力状態処理のテストを担当し、Solが計画・レビューを担当する。常駐化、一覧UI、依存追加はこの単位に含めない。フェーズ1の抑止・個別前面化・全Space対応が実機で成立するまでは、両フェーズの完了条件を満たした扱いにしない。

captureは通常のCommand+TabとCommand+Shift+Tab、および抑止したキー押下に対応するキー解放を扱う。キーリピートで選択を進めず、Command解放・Escape取消・タップ無効化時の状態破棄を純粋なRustの状態処理として実装する。コールバックでAX問い合わせやログ出力を行わない。期限切れ・無効化時はタップを終了し、標準入力へ戻す。capture自体は実ウィンドウを切り替えない。

AXの診断は対象PIDの公開`AXWindows`配列からウィンドウ要素を取得する。INDEXはその時点の配列内の一時的な位置であり、永続的なウィンドウIDや全アプリの候補識別方式ではない。明示的なraiseだけで必要な復元・非表示解除を試し、選んだAX要素のraiseとフォーカス属性を確認する。APIの成功、フォーカス属性の一致、実際の入力先・Space遷移は区別して報告する。公開AX属性が操作に対応しない場合はエラーとし、別ウィンドウへ切り替える代替処理や非公開APIを追加しない。

次の縦切りは`--switch-pid PID --seconds N`（選択入力が最大10秒、1セッション）とする。開始時に指定PIDのAXWindows配列とAX要素を保持し、その順序で正逆方向に循環する。開始時のFocusedWindowが候補内にあれば基準位置にし、確認できなければ最初の正方向で先頭、逆方向で末尾から始める。Command解放で確定した要素を一度だけraiseし、番号で候補を再取得しない。Escape・別の通常キー入力・期限切れ・タップ無効化はraiseせず終了する。通常キーは取消時にそのまま通す。抑止したキーの解放待ち中もリピートを通さず、解放が期限に間に合わなければ保留中のcommitを取り消す。0件ではタップを開始せず、1件でも安全に扱い、閉じた候補への操作失敗で別候補を選ばない。

イベントコールバックは既存の入力状態処理と選択位置の更新だけに限定する。commitを受けたrunloopはタップをdisable・detach・invalidateしてからAX操作を行い、同じrunloopでAX応答を待つ間に入力タップを動かさない。コールバックの後続イベントが最初の確定位置を変更できないよう、1セッションの終了を固定する。保持した配列はAX操作の終了まで解放しない。Lunaが実装・純粋状態テスト、Solがレビューを担当する。MRU、全アプリの候補取得、全Spaceの成立、UI・常駐化はこの単位の完了に含めない。

単一PIDモードの動作OKというユーザー報告を受け、次は`--list-candidates`と`--switch --seconds N`でPIDを指定しない候補取得・切り替えを実装する。CGWindowListCopyWindowInfoの`optionOnScreenOnly | excludeDesktopElements`で得た前面から背面への並びからlayer 0のowner PIDを重複なく取り出し、`optionAll | excludeDesktopElements`にだけ現れるPIDを後ろへ追加する。optionAllの戻り順自体を前面順と扱わない。CGはPID発見とアプリ群の初期順だけに使い、各アプリ内の順序はそのPIDのAXWindows配列順とする。CGとAXの個別ウィンドウをタイトル・位置・サイズで対応づけない。[Apple: optionOnScreenOnly](https://developer.apple.com/documentation/coregraphics/cgwindowlistoption/optiononscreenonly)、[optionAll](https://developer.apple.com/documentation/coregraphics/cgwindowlistoption/optionall)

自プロセスを除き、AXの標準ウィンドウ・非モーダルという条件を確認できた要素を候補にする。PIDごとの取得失敗と候補除外は件数・理由を診断に表示する。AXModalが非対応・読み取り失敗・不正型の場合も標準ウィンドウを除外する保守的な方針であり、その件数は別に表示してChrome等での欠落を確認する。候補は開始時に保持したAX要素で固定し、確定後の再取得や番号による再同定をしない。入力・取消・期限・タップ終了後のAX操作は既存の1セッション処理を共有する。これは画面上のowner順を優先したアプリ群＋AX配列の初期順であり、正確なMRUやウィンドウ単位のCG前面順ではない。CG一覧に現れないアプリ、AX非対応、別Space・フルスクリーン・最小化・非表示の欠落がないことは実機確認を待ち、全Space要件の成立済みとはしない。UIと常駐履歴は追加しない。

- 待機、選択中、一時停止の小さな状態機械を実装する。
- 正逆の循環、Command解放、取消、キーリピートと左右修飾キーを扱う。
- MRU、選択中の候補順固定、消滅候補と失敗時の終了を実装する。
- タップ処理とAX処理を分離し、通信の時間上限と無効化時の状態破棄を実装する。

完了条件: UIなしの検証段階でも、指定ウィンドウの切り替えと失敗時の入力復帰が安定している。

### フェーズ3: 最小UIと常駐

- 開始: 有限時間の切り替え実証にAppKitの非アクティブ化パネルを追加し、アプリ名・タイトル・選択行を表示する。Rustの候補／入力状態処理は維持し、AppKit描画だけを小さなObjective-C shimに置く。初回の実機確認では、元アプリへの入力継続、一覧の読みやすさ、別Space・フルスクリーン上でのパネル表示を調べる。
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

次の実装単位では、入力状態処理のテストで通常キーの通過、正逆方向、リピート、Command解放後のTab解放、Escapeの押下・解放、両Commandの集約状態、無効化による状態破棄を確認する。CIが成功しても、標準Command+Tabの抑止・診断終了後の復帰、対象Chromeウィンドウへの実入力、別Space／フルスクリーン、最小化・非表示からの復元はActions成果物での実機確認を待つ。

単一PIDの切り替えでは、0件／1件／複数件、正逆方向の循環、FocusedWindowの有無、リピート無視、取消、最初のcommit位置の固定と終了後のイベント無視をCI用テストで確認する。実機ではChromeの2ウィンドウで保持順による移動と実入力先を確認し、Escape・期限切れ・候補の消滅・操作失敗の際に元の状態と標準Command+Tabが戻ることを調べる。別Space・フルスクリーン・最小化・非表示への対応は個別に記録し、既存のAX属性一致から推測しない。

PID指定なしの候補取得では、画面上のowner PID順を維持した重複除去、optionAllからの補完、layer／自PID／不正PIDの除外を小さな純粋テストで確認する。Actions成果物でChromeと別アプリを跨ぐ選択、同一アプリ内の複数ウィンドウ、除外理由と候補数、取消・終了後の入力復帰を調べる。AX/CGの個別対応と正確なMRUはこの実装で検証済みとしない。

### 手動確認記録

- 2026-10-07、Actions成果物を起動し、arm64のMach-O、Info.plist、ad-hoc署名を確認した。
- 引数なしの診断コマンドは起動し、Accessibility未許可を表示した。
- `--tap-seconds 5`はイベントタップ作成時にInput Monitoring権限がないとして終了した。権限設定は変更していない。
- その後ユーザーから、設定をONにしているはずでもエラーが表示されると報告を受けた。旧PoCはAccessibilityの状態確認だけを行い、入力監視はタップ作成失敗から権限不足を推測していた。起動時の権限要求と個別の許可状態の表示を追加するが、実際の設定対象やエラーの原因は未確定。
- 権限要求後も`AXIsProcessTrusted`はfalseで、イベントタップも作成できなかった。設定がONという報告との不一致は解消していない。この結果だけで権限設定の対象や署名が原因と断定しない。
- 2026-10-07、ユーザー環境で権限要求追加後のActions成果物を起動し、`Accessibility: granted`および`Input Monitoring: granted`と表示されることを確認。その後のAX・イベントタップ確認は下記に記録する。
- 2026-10-07、`--tap-seconds 10`で10秒間のlisten-onlyイベントタップが起動し、Command+Tabのkeydownを4回検出。入力を通したまま検出できた。
- 2026-10-07、`--pid <PID>`で対象アプリのAXウィンドウを1件取得。PIDは環境固有のため記録しない。
- 2026-10-07、Chromeの2ウィンドウを`--list-windows`で列挙し、`--raise-window`でそれぞれを指定。両方でAXRaise・アプリ前面化が受け付けられ、FocusedWindow属性の一致を確認した。AXRaise後にAXWindows配列の順序が変わるため、コマンド間でordinal indexを再利用できない。切り替え実装では選択開始時の候補配列とAX要素を保持し、番号で再解決しない。
- 2026-10-07、ユーザーからcaptureのforward・commitカウントが記録され、正常に動作していそうとの報告を受けた。終了後の標準Command+Tab復帰やraise後の実入力先を明示的に確認した報告ではないため、そこは未確認のままとする。
- 2026-10-07、ユーザーから`--switch-pid`の実機動作OKの報告を受けた。単一PIDの実動作確認として記録し、取消・期限境界・候補消滅・別Spaceなど個別ケースをすべて確認したとは扱わない。
- 2026-10-07、ユーザーからPID指定なしモードについて、複数のChromeウィンドウと別アプリ間の切り替えが動作OKとの報告を受けた。プロファイル別の誤認、キー入力先、取消・期限、別Space・フルスクリーン・最小化・非表示の各ケースまで確認済みとは扱わない。
- PID指定なしモードの基本切り替えは実機確認済み。常駐UI、正確なMRU、全Spaceを含む通常利用の切り替え機能はまだ完成していない。

## 5. 追加判断が必要になる条件

- 確定要件である全Space・フルスクリーン対応が公開APIで成立しない、またはOS設定に依存する場合。
- 主要アプリで公開APIによる正確な候補識別やフォーカスが成立しない場合。
- サムネイルや画面収録権限が必要になる場合。
- MRU通知の制約で、要求する選択順を維持できない場合。
- 測定結果から、性能目標と単純な実装の間で優先順位を決める必要がある場合。

将来機能のための仕組みは先に作らず、実際に必要になった機能だけを追加する。
