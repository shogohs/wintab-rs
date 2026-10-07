# wintab-rs
Window-level Cmd+Tab switcher for macOS built with Rust.

## 開発ドキュメント

- [機能要件](docs/requirements.md)
- [開発計画](docs/plan.md)

## フェーズ1: 診断PoC

GitHub Actionsの`macOS PoC` workflowは、Apple Silicon向けの`.app`を成果物として作成します。Actionsの成果物から`wintab-rs-macos-arm64.zip`を取得して展開してください。ビルド・テストはGitHub Actions上で行い、ローカルでは実行しません。

この段階の`.app`は実行ファイルを束ねる最小パッケージで、ActionsではPoC用のad-hoc署名を付けます（配布用の署名・公証ではありません）。診断メッセージを確認できるよう、Finderで開くのではなくターミナルから実行します。

```sh
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs"
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --pid 1234
"/path/to/wintab-rs.app/Contents/MacOS/wintab-rs" --tap-seconds 5
```

有効な引数で起動すると、未許可のAccessibility権限を`AXIsProcessTrustedWithOptions`（`kAXTrustedCheckOptionPrompt = true`）、入力監視権限を`CGRequestListenEventAccess`で要求し、現在のプロセスに対する許可状態を表示します。Accessibilityの要求は非同期なので、要求直後の未許可表示は拒否の確定ではありません。このCLIは権限設定を待って常駐せず、実行に必要な権限がまだなければ診断を開始せずに案内を表示し、正常終了します。[Apple: Accessibility権限要求](https://developer.apple.com/documentation/applicationservices/1459186-axisprocesstrustedwithoptions)、[入力監視権限要求](https://developer.apple.com/documentation/coregraphics/cgrequestlisteneventaccess())

システムの案内に従い、「システム設定」>「プライバシーとセキュリティ」のAccessibility（アクセシビリティ）と「入力監視」で許可してから、同じ`.app`を再起動してください。既にONでも未許可と表示される場合は、診断に表示された実行ファイルの場所と設定対象の`.app`が一致するか確認し、現在の`.app`を設定に登録し直してください。成果物の差し替えによるad-hoc署名の変更や起動経路を含め、TCCが実際に許可する対象は実機確認が必要です。

`--pid`は指定プロセスの公開AX APIからウィンドウ配列を取得し、件数のみを表示します。`--tap-seconds`は最大10秒間、Command+Tab相当のキー押下数だけをメモリ内で数えます。イベントタップはlisten-onlyで、入力を変更・抑止せず、キー内容やウィンドウタイトルを保存しません。入力監視の確認APIが許可済みでもイベントタップ作成に失敗する場合があるため、その失敗だけで権限不足と断定しません。

このPoCはコマンドライン診断のみです。通常のウィンドウ切り替え、常駐メニューバーUI、候補一覧、キー入力の抑止・再送、個別ウィンドウの前面化は未実装です。既存のActions成果物は実機で起動を確認しましたが、今回追加した権限要求、許可後のAX応答とイベントタップ動作は未検証で、要件全体も未完成です。
