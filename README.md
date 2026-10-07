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
```

有効な引数で起動すると、未許可のAccessibility権限を`AXIsProcessTrustedWithOptions`（`kAXTrustedCheckOptionPrompt = true`）、入力監視権限を`CGRequestListenEventAccess`で要求し、現在のプロセスに対する許可状態を表示します。Accessibilityの要求は非同期なので、要求直後の未許可表示は拒否の確定ではありません。このCLIは権限設定を待って常駐せず、実行に必要な権限がまだなければ診断を開始せずに案内を表示し、正常終了します。[Apple: Accessibility権限要求](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions)、[入力監視権限要求](https://developer.apple.com/documentation/coregraphics/cgrequestlisteneventaccess())

システムの案内に従い、「システム設定」>「プライバシーとセキュリティ」のAccessibility（アクセシビリティ）と「入力監視」で許可してから、同じ`.app`を再起動してください。既にONでも未許可と表示される場合は、診断に表示された実行ファイルの場所と設定対象の`.app`が一致するか確認し、現在の`.app`を設定に登録し直してください。成果物の差し替えによるad-hoc署名の変更や起動経路を含め、TCCが実際に許可する対象は実機確認が必要です。

`--pid`は指定プロセスの公開AX APIからウィンドウ配列を取得し、件数のみを表示します。`--tap-seconds`は最大10秒間、Command+Tab相当のキー押下数だけをメモリ内で数えます。イベントタップはlisten-onlyで、入力を変更・抑止せず、キー内容やウィンドウタイトルを保存しません。入力監視の確認APIが許可済みでもイベントタップ作成に失敗する場合があるため、その失敗だけで権限不足と断定しません。

`--capture-seconds`は指定した最大10秒間だけ、Command+Tab／Command+Shift+Tabを抑止する実証モードです。Commandを押したままTabで正逆方向の操作を記録し、Command解放で確定、Escapeで取消を記録します。実ウィンドウの切り替えは行いません。キーリピートは移動として数えず、他のキーと修飾キーは通します。期限切れやタップ無効化で終了し、終了後は通常のCommand+Tabが戻ることを実機で確認してください。

`--list-windows PID`は指定アプリのAXウィンドウの番号とタイトルをターミナルに表示します。`--raise-window PID INDEX`は公開AX APIで指定ウィンドウの復元・前面化を試し、フォーカス属性を確認します。INDEXは0から始まるその時点のAX配列内の位置で、ウィンドウの開閉や配列の並べ替えで変化します。以前の一覧の番号を固定IDとして使わないでください。APIの成功やフォーカス属性の一致だけでは実入力先・Space遷移の成立を保証できないため、Chromeで対象に文字を入力できることを確認します。

権限、AXウィンドウ取得、listen-onlyでのCommand+Tab検出は既存のActions成果物を実機で確認しました。今回のcaptureとraiseは実機検証前です。通常のウィンドウ切り替え、全アプリの候補識別とMRU、常駐メニューバーUI、候補一覧は未実装で、要件全体も未完成です。
