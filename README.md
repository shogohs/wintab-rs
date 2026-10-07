# wintab-rs
Window-level Cmd+Tab switcher for macOS built with Rust.

## 開発ドキュメント

- [機能要件](docs/requirements.md)
- [開発計画](docs/plan.md)

## Rust formatter

Docker ComposeでRust 1.99.0のrustfmtを使います。初回はイメージをビルドします。

```sh
docker compose run --rm rustfmt                 # format check
docker compose run --rm rustfmt fmt --all        # format files
```

## 常駐アプリと診断コマンド

GitHub Actionsの`macOS app` workflowは、Apple Silicon向けの`.app`を成果物として作成します。Actionsの成果物から`wintab-rs-macos-arm64.zip`を取得して展開してください。ビルド・テストはGitHub Actions上で行い、ローカルでは実行しません。

`.app`はActionsでad-hoc署名を付けます（配布用の署名・公証ではありません）。Finderから起動するとメニューバーに`WT`が表示されます。Command+Tabでアプリアイコンを横並びにした一覧を開き、選択中ウィンドウのタイトルをUI中央下部に表示します。同じアプリの複数ウィンドウはタイトルで見分けられます。アイコンは通常94ポイントで表示し、候補が多い場合はプライマリディスプレイ内に全候補が収まるよう縮小します。スクロールはありません。最初に直前のウィンドウを選択し、Commandを離すか候補アイコンをクリックするとそのウィンドウへ切り替えます。Commandを押したまま左右矢印でも候補を移動できます。ポインターを合わせると候補が選択され、パネル外クリックは一覧を閉じてクリック先へ通ります。ウィンドウのサムネイルは取得せず、スクリーン収録権限も要求しません。メニューにはAccessibilityとInput Monitoringの許可状態、「権限状態を再確認」、「プライバシーとセキュリティ設定」、有効／一時停止、終了を表示します。許可後は状態を再確認してから再起動せず有効化できます。

一覧パネルはmacOS 26以降でAppKit標準Liquid Glassを背景に1面だけ使用し、背景の濃さを調整しています。旧OSでは半透明背景にフォールバックします。選択中はアイコンの輪郭に沿うシステムブルーの影で示し、タイトルは17ptで表示します。パネルは標準Command+Tabに寄せた連続的な角丸にしています。

取得できる場合はDockの通知バッジを各アプリアイコン右上に表示します。同じアプリの複数ウィンドウには同じバッジが付きます。バッジはアイコンサイズに合わせて拡大縮小します。Dockの未公開・未保証なAccessibility `AXStatusLabel` に依存するため、読めない・空・0・読み取りが遅い場合は表示しません。取得は一覧表示ごとに一度だけ短時間バックグラウンドで試し、追加権限は要求しません。

常駐中はNSWorkspaceのアプリ起動・アクティブ化通知と、各アプリのAccessibilityフォーカス変更通知でウィンドウMRUを更新します。履歴はメモリ内のみで、通知を登録できないアプリは操作開始時のFocused Window取得にフォールバックします。権限がない／一時停止中は通知監視を行いません。

Space切り替え通知とイベントタップのタイムアウト時に監視を再有効化します。非公開WindowServer APIは使わないため、macOSの公開AX APIが返さない別Spaceのウィンドウは列挙できず、対象Spaceへの切り替えも保証しません。

```sh
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --status
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --pid 1234
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --tap-seconds 5
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --capture-seconds 5
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --list-windows 1234
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --raise-window 1234 0
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --switch-pid 1234 --seconds 10
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --list-candidates
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --switch --seconds 10
```

引数なしの起動では未許可のAccessibility権限を`AXIsProcessTrustedWithOptions`（`kAXTrustedCheckOptionPrompt = true`）、入力監視権限を`CGRequestListenEventAccess`で要求してメニューバーに常駐します。権限が不足している間は切り替えを無効にし、メニューから設定画面を開けます。`--status`または診断引数を付けた起動では現在の許可状態を表示します。Accessibilityの要求は非同期なので、要求直後の未許可表示は拒否の確定ではありません。[Apple: Accessibility権限要求](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions)、[入力監視権限要求](https://developer.apple.com/documentation/coregraphics/cgrequestlisteneventaccess())

システムの案内に従い、「システム設定」>「プライバシーとセキュリティ」のAccessibility（アクセシビリティ）と「入力監視」で許可してから、同じ`.app`を再起動してください。既にONでも未許可と表示される場合は、診断に表示された実行ファイルの場所と設定対象の`.app`が一致するか確認し、現在の`.app`を設定に登録し直してください。成果物の差し替えによるad-hoc署名の変更や起動経路を含め、TCCが実際に許可する対象は実機確認が必要です。

`--pid`は指定プロセスの公開AX APIからウィンドウ配列を取得し、件数のみを表示します。`--tap-seconds`は最大10秒間、Command+Tab相当のキー押下数だけをメモリ内で数えます。イベントタップはlisten-onlyで、入力を変更・抑止せず、キー内容やウィンドウタイトルを保存しません。入力監視の確認APIが許可済みでもイベントタップ作成に失敗する場合があるため、その失敗だけで権限不足と断定しません。

`--capture-seconds`は指定した最大10秒間だけ、Command+Tab／Command+Shift+Tabを抑止する実証モードです。Commandを押したままTabで正逆方向の操作を記録し、Command解放で確定します。実ウィンドウの切り替えは行いません。キーリピートは移動として数えず、選択中に他のキーを押すと取消し、その入力は通します。Escapeに専用の取消処理はありません。期限切れやタップ無効化で終了し、終了後は通常のCommand+Tabが戻ることを実機で確認してください。

`--list-windows PID`は指定アプリのAXウィンドウの番号とタイトルをターミナルに表示します。`--raise-window PID INDEX`は公開AX APIで指定ウィンドウの復元・前面化を試し、フォーカス属性を確認します。INDEXは0から始まるその時点のAX配列内の位置です。`AXRaise`後にも配列の順序が変わるため、毎回`--list-windows`で対象の現在位置を確認してください。番号は固定IDではありません。APIの成功やフォーカス属性の一致だけでは実入力先・Space遷移の成立を保証できないため、Chromeで対象に文字を入力できることを確認します。通常の切り替え実装では、選択中に配列を再取得して番号で解決せず、候補スナップショットに保持したAX要素を使います。

`--switch-pid PID --seconds N`は、指定した1アプリのAXウィンドウを使う、選択入力が最大10秒・1セッションの切り替え実証です。開始時に候補の順序とAX要素を保持し、ターミナルと非アクティブ化されたネイティブパネルに候補を表示します。Command+Tab／Command+Shift+Tabで正逆方向に循環し、選択行を更新します。Commandを離した時点でパネルと入力タップを終了してから、選んだ要素を一度だけraiseします。AX操作の完了待ちはこの入力時間に含みません。候補順はMRUではありません。開始時に対象アプリのFocusedWindowが候補内にあればそこを基準に次／前を選び、確認できなければ最初の正方向操作で先頭、逆方向操作で末尾を選びます。

選択中に別の通常キーを押すと取消し、そのキーは元のアプリへ通します。Escapeに専用の取消処理はなく、入力を抑止しません。抑止したTabの解放まで対応づけて処理し、解放を待つ間に期限へ達した場合は確定済みでもraiseしません。期限切れ・タップ無効化でもraiseせず終了し、閉じた候補を別のウィンドウで代用しません。次の切り替えを試すにはコマンドを再実行します。これは有限時間の診断モードであり、常時のCommand+Tab介入、MRU、常駐、全Space対応を完成させたモードではありません。

`--list-candidates`はPID指定なしで候補を取得します。CGの画面上の一覧から前面順にowner PIDを取り出し、全ウィンドウ一覧にだけ現れるPIDを補完して、それぞれのAX標準ウィンドウをまとめます。アプリ群は画面上のowner順優先、同じアプリ内はAX配列順で、正確なMRUやウィンドウ単位のCG前面順ではありません。optionAllの戻り順も前面順とは保証されません。CGとAXの個別ウィンドウをタイトルや位置で対応づける処理は行いません。

`--switch --seconds N`はその保持済み候補で、単一PIDモードと同じ1セッションの切り替えを行います。候補はターミナルとネイティブパネルに表示します。取得失敗・非対応・除外の件数と理由を診断に表示します。`AXModal=false`を確認できない標準ウィンドウも保守的に除外し、未対応・読み取り失敗のAXModal件数を別に表示します。CG一覧に現れないアプリやAX非対応による欠落、全Space・フルスクリーン・最小化・非表示の完全性は未確認です。画面収録権限を要求するモードではありません。

権限、AXウィンドウ取得、listen-onlyでのCommand+Tab検出は既存のActions成果物を実機で確認しました。captureの正方向・commitカウント、Chromeの2ウィンドウへのAXRaise・前面化・FocusedWindow属性の一致、単一PIDの`--switch-pid`に加え、PID指定なしモードで複数のChromeウィンドウと別アプリ間の切り替えが動作したとの報告を受けています。ユーザーからパネル表示と選択行移動の動作OK、Escape取消は不要との判断を受けています。個別の一般キー取消・期限境界・終了後の標準Command+Tab復帰、プロファイル識別、別Space・フルスクリーン等をすべて検証済みとは扱いません。常駐メニューとCommand+Tab連携は実装中で、今回の変更はActionsビルドと実機確認待ちです。
