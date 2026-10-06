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

引数なしではAccessibility権限の状態と設定場所を表示します。`--pid`は指定プロセスの公開AX APIからウィンドウ配列を取得し、件数のみを表示します。`--tap-seconds`は最大10秒間、Command+Tab相当のキー押下数だけをメモリ内で数えます。イベントタップはlisten-onlyで、入力を変更・抑止せず、キー内容やウィンドウタイトルを保存しません。Accessibility権限は「システム設定」>「プライバシーとセキュリティ」>「アクセシビリティ」で許可し、再起動してください。イベントタップ作成が拒否された場合は同じ「プライバシーとセキュリティ」の「入力監視」を確認してください。必要な権限とTCC上の許可対象は、ad-hoc署名・起動経路を含め実機で確認が必要です。

このPoCはコマンドライン診断のみです。通常のウィンドウ切り替え、常駐メニューバーUI、候補一覧、キー入力の抑止・再送、個別ウィンドウの前面化は未実装です。macOS 27／Apple Silicon実機での起動、権限付与、AX応答、イベントタップ動作は未検証で、要件全体も未完成です。
