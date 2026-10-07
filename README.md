# wintab-rs
Window-level Cmd+Tab switcher for macOS built with Rust.

## 開発ドキュメント

- [機能要件](docs/requirements.md)
- [開発計画](docs/plan.md)

## 診断PoCと次段階の実証

GitHub Actionsの`macOS PoC` workflowは、Apple Silicon向けの`.app`を成果物として作成します。Actionsの成果物から`wintab-rs-macos-arm64.zip`を取得して展開してください。ビルド・テストはGitHub Actions上で行い、ローカルでは実行しません。

この段階の`.app`は実行ファイルを束ねる最小パッケージで、ActionsではPoC用のad-hoc署名を付けます（配布用の署名・公証ではありません）。診断メッセージを確認できるよう、Finderで開くのではなくターミナルから実行します。

```sh
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs"
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --pid 1234
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --tap-seconds 5
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --capture-seconds 5
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --list-windows 1234
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --raise-window 1234 0
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --switch-pid 1234 --seconds 10
```

有効な引数で起動すると、未許可のAccessibility権限を`AXIsProcessTrustedWithOptions`（`kAXTrustedCheckOptionPrompt = true`）、入力監視権限を`CGRequestListenEventAccess`で要求し、現在のプロセスに対する許可状態を表示します。Accessibilityの要求は非同期なので、要求直後の未許可表示は拒否の確定ではありません。このCLIは権限設定を待って常駐せず、実行に必要な権限がまだなければ診断を開始せずに案内を表示し、正常終了します。[Apple: Accessibility権限要求](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions)、[入力監視権限要求](https://developer.apple.com/documentation/coregraphics/cgrequestlisteneventaccess())

システムの案内に従い、「システム設定」>「プライバシーとセキュリティ」のAccessibility（アクセシビリティ）と「入力監視」で許可してから、同じ`.app`を再起動してください。既にONでも未許可と表示される場合は、診断に表示された実行ファイルの場所と設定対象の`.app`が一致するか確認し、現在の`.app`を設定に登録し直してください。成果物の差し替えによるad-hoc署名の変更や起動経路を含め、TCCが実際に許可する対象は実機確認が必要です。

`--pid`は指定プロセスの公開AX APIからウィンドウ配列を取得し、件数のみを表示します。`--tap-seconds`は最大10秒間、Command+Tab相当のキー押下数だけをメモリ内で数えます。イベントタップはlisten-onlyで、入力を変更・抑止せず、キー内容やウィンドウタイトルを保存しません。入力監視の確認APIが許可済みでもイベントタップ作成に失敗する場合があるため、その失敗だけで権限不足と断定しません。

`--capture-seconds`は指定した最大10秒間だけ、Command+Tab／Command+Shift+Tabを抑止する実証モードです。Commandを押したままTabで正逆方向の操作を記録し、Command解放で確定、Escapeで取消を記録します。実ウィンドウの切り替えは行いません。キーリピートは移動として数えず、他のキーと修飾キーは通します。期限切れやタップ無効化で終了し、終了後は通常のCommand+Tabが戻ることを実機で確認してください。

`--list-windows PID`は指定アプリのAXウィンドウの番号とタイトルをターミナルに表示します。`--raise-window PID INDEX`は公開AX APIで指定ウィンドウの復元・前面化を試し、フォーカス属性を確認します。INDEXは0から始まるその時点のAX配列内の位置です。`AXRaise`後にも配列の順序が変わるため、毎回`--list-windows`で対象の現在位置を確認してください。番号は固定IDではありません。APIの成功やフォーカス属性の一致だけでは実入力先・Space遷移の成立を保証できないため、Chromeで対象に文字を入力できることを確認します。通常の切り替え実装では、選択中に配列を再取得して番号で解決せず、候補スナップショットに保持したAX要素を使います。

`--switch-pid PID --seconds N`は、指定した1アプリのAXウィンドウを使う、選択入力が最大10秒・1セッションの切り替え実証です。開始時に候補の順序とAX要素を保持し、ターミナルに候補を表示します。Command+Tab／Command+Shift+Tabで正逆方向に循環し、Commandを離した時点で入力タップを終了してから、選んだ要素を一度だけraiseします。AX操作の完了待ちはこの入力時間に含みません。候補順はMRUではありません。開始時に対象アプリのFocusedWindowが候補内にあればそこを基準に次／前を選び、確認できなければ最初の正方向操作で先頭、逆方向操作で末尾を選びます。

Escapeは元のウィンドウを操作せず終了します。選択中に別の通常キーを押した場合も取消し、そのキーは元のアプリへ通します。抑止したTab／Escapeの解放まで対応づけて処理し、解放を待つ間に期限へ達した場合は確定済みでもraiseしません。期限切れ・タップ無効化でもraiseせず終了し、閉じた候補を別のウィンドウで代用しません。次の切り替えを試すにはコマンドを再実行します。全アプリの候補取得、MRU、UI、常駐機能、全Space対応を完成させたモードではありません。

権限、AXウィンドウ取得、listen-onlyでのCommand+Tab検出は既存のActions成果物を実機で確認しました。captureの正方向・commitカウント、およびChromeの2ウィンドウへのAXRaise・前面化・FocusedWindow属性の一致も確認されています。capture終了後の標準Command+Tab復帰、raise後の実入力先やSpace遷移は未確認です。今回追加した単一PIDの切り替えモードも実機検証前で、要件全体は未完成です。
