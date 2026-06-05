<p align="center">
  <img src="docs/brand/icon.png" alt="LinkPilot" width="128" height="128">
</p>

<h1 align="center">LinkPilot</h1>

<p align="center">
  <em>すべてのリンクを適切なブラウザ、プロファイル、ワークスペースへルーティングします。</em>
</p>

<p align="center">
  <a href="README.md">English</a> | <a href="README.zh.md">简体中文</a> | <a href="README.zh-TW.md">繁體中文</a> | 日本語
</p>

LinkPilot は macOS を主対象にしたリンクルーターです。macOS、ブラウザ、URL
を開くアプリの間に入り、ルールに最も合うブラウザとプロファイルへ各リンクを送ります。

LinkPilot はブラウザではありません。複数の Chrome プロファイル、Arc、Safari、
Firefox、ワークスペース、そして Slack、Lark、Terminal、IDE などの送信元アプリを
使い分ける人のための小さなルーティング層です。

## 現在の状態

LinkPilot は現在 macOS に注力しています。

- macOS デスクトップアプリ: アクティブ。
- CLI とバックグラウンド daemon: アクティブ。
- Windows / Linux プラットフォーム crate: 現在はスタブ実装。
- ブラウザ拡張: 後続マイルストーン用に予約。

現在のアプリには以下が含まれています。

- host、path、送信元アプリ、送信元ブラウザ、送信元プロファイルによる URL ルーティング。
- 主要な Chromium / Firefox 系ブラウザ、Arc、Safari 系アプリルーティング、カスタムブラウザの検出とプロファイル列挙。
- Ask picker: Halo プロファイルホイール（Frosted / Bezel / Crown の 3 スタイル）、キーボードショートカット、ブラウザごとのプロファイル並び替え、ダークモード、Settings の実ブラウザ向けテスト URL フロー。
- UI 言語は English、简体中文、繁體中文、日本語に対応し、システム言語に合わせる設定も利用可能。
- GitHub Release の自動更新チェック。DMG ダウンロードは SHA-256 を検証し、Settings で手動でインストーラーを開きます。
- バックグラウンド daemon と Unix socket IPC。メインウィンドウを閉じた後もルーティングを継続できます。
- メニューバートレイ、Inspector、Test URL シミュレーター、ブラウザ管理、Settings、onboarding。
- `lpt` CLI: URL を開く、ルールを管理する、設定を確認する、daemon をインストールする、既定ブラウザ状態を確認する。

## 対応ブラウザ

macOS では、LinkPilot はインストール済みの以下のブラウザを自動検出します。

- プロファイル列挙に対応する Chromium 系：Google Chrome、Microsoft Edge、Brave Browser、
  Vivaldi、Opera、Opera GX、Dia、ChatGPT Atlas、Perplexity Comet、Yandex Browser、
  Naver Whale。
- プロファイル列挙に対応する Firefox 系：Firefox、Zen Browser、LibreWolf、Waterfox、Floorp、
  Mullvad Browser、Tor Browser。
- アプリルーティング中心：Arc、Safari、Orion、DuckDuckGo Browser、カスタムブラウザ。Arc
  は picker 表示用にプロファイルデータを読めますが、外部からの Space / profile 選択は Arc 側に委ねます。

## インストール

現在の release artifacts は署名されていません。macOS は初回起動時に quarantine
を付与する場合があります。必要に応じて、インストール後に quarantine フラグを削除してください。

Homebrew はまだ対応済みのインストール経路ではありません。tap が公開され検証されるまでは、
以下の DMG または CLI tarball を使ってください。

### GUI アプリ

最新の GitHub Release から、お使いの Mac のアーキテクチャに合った DMG を
ダウンロードしてください（Apple Silicon は `aarch64`、Intel は `x86_64`）。
`LinkPilot.app` を `/Applications` にコピーして開きます。

```sh
# Apple Silicon → aarch64; Intel → x86_64
ARCH=$(uname -m | sed 's/arm64/aarch64/')
curl -L "https://github.com/jackerjay/LinkPilot/releases/latest/download/LinkPilot_<version>_${ARCH}.dmg" -o LinkPilot.dmg
hdiutil attach LinkPilot.dmg
cp -R "/Volumes/LinkPilot/LinkPilot.app" /Applications/
hdiutil detach "/Volumes/LinkPilot"
xattr -dr com.apple.quarantine /Applications/LinkPilot.app
open /Applications/LinkPilot.app
```

初回起動後、onboarding または Settings で以下を行います。

1. LinkPilot をシステムの既定ブラウザとして登録します。
2. バックグラウンド daemon LaunchAgent をインストールします。
3. 同梱の `lpt` コマンドを `~/.local/bin` にインストールします。

Settings は起動時に GitHub Releases の新しい LinkPilot build を既定で確認します。
新しい macOS DMG がある場合、LinkPilot はローカルの更新キャッシュへダウンロードし、
release の `checksums.txt` と照合して SHA-256 を検証したうえで、"Open installer"
をクリックするよう促します。`checksums.txt` がない release は未検証として扱われ、
自動ダウンロードは拒否されます。この動作は Settings → General → Updates で無効にできます。

## 対応言語

LinkPilot は現在、以下の UI 言語を同梱しています。

- English
- 简体中文
- 繁體中文
- 日本語

既定では、システム言語が上記のいずれかに一致する場合、LinkPilot はそれに従います。
Settings → Appearance → Language で手動指定することもできます。Picker ウィンドウは、
次に開いたときに同じ言語設定を読み込みます。

CLI からも同じ設定を変更できます。

```sh
lpt settings language system
lpt settings language en
lpt settings language zh-CN
lpt settings language zh-TW
lpt settings language ja-JP
```

### CLI のみ

CLI tarball には `lpt` と `linkpilot-daemon` が含まれています。

```sh
ARCH=$(uname -m | sed 's/arm64/aarch64/')
curl -L "https://github.com/jackerjay/LinkPilot/releases/latest/download/lpt-macos-${ARCH}.tar.gz" \
  | tar -xz -C ~/.local/bin
chmod +x ~/.local/bin/lpt
```

ターミナル自動化だけが必要な場合、または GUI を開かずに daemon だけを使いたい場合は、
CLI-only の経路を使ってください。

## クイックスタート

### デスクトップ

1. LinkPilot を開きます。
2. onboarding または Settings で LinkPilot を既定ブラウザにします。
3. Rules ページでルールを追加または編集します。
4. Test URL で、実際のルーティングエンジンに対して URL を dry-run します。
5. Inspector で実際のルーティング判断を確認します。

Ask ルールでは、LinkPilot が picker ウィンドウを開きます。複数プロファイルを持つ
ブラウザ上で Option を押し続けると Halo ホイールが表示され、プロファイルに狙いを合わせて
Option を離すと開きます。Settings → Appearance では、3 つの Halo スタイル
（Frosted、Bezel、Crown）の切り替え、ブラウザごとのプロファイル並び替え
（位置 1–9 はキーボードショートカットに対応）、実ブラウザ構成を使ったテスト URL 実行ができます。
ルールを作成しなくても、フォーカス移動、プロファイル選択、見た目を確認できます。保存済み順序に
まだ入っていないプロファイルをブラウザが検出した場合、Settings は `+N new` チップで知らせます。

### CLI

```sh
cargo build -p linkpilot-cli
./target/debug/lpt doctor
./target/debug/lpt open https://github.com
./target/debug/lpt open https://figma.com --dry-run
./target/debug/lpt open https://github.com --from-app Slack
```

よく使うコマンド:

```sh
# ルール
lpt rules list --all
lpt rules add --host "*.figma.com" --target arc --priority 20
lpt rules add --host github.com --path "/oauth/*" --keep-source --priority 50
lpt rules add --from-app Slack --ask
lpt rules disable <id-prefix>
lpt rules delete <id-prefix>

# ワークスペース
lpt workspaces add work --name Work
lpt workspaces disable work

# 設定ファイル
lpt config show
lpt config path
lpt config set-default-target chrome --profile Default
lpt config export ./linkpilot.backup.json
lpt config import ./linkpilot.backup.json

# 設定
lpt settings show
lpt settings smart-routing off
lpt settings launch-at-login on
lpt settings auto-updates off
lpt settings picker-style crown        # frosted | bezel | crown
lpt settings language ja-JP            # system | en | zh-CN | zh-TW | ja-JP
lpt settings history-retention 30

# ブラウザ
lpt browsers list
lpt browsers profiles chrome
lpt browsers custom add --id devbuild --name "Chrome Canary" \
  --kind chromium --exec /Applications/Google\ Chrome\ Canary.app

# 既定ブラウザと daemon
lpt default-browser status
lpt default-browser set
lpt daemon status
lpt daemon install
lpt daemon logs --follow
```

`lpt` は実行中の daemon へ優先的に接続します。

```text
~/Library/Application Support/LinkPilot/linkpilot.sock
```

daemon がない場合、ローカルで実行可能なコマンドはローカル実行へフォールバックします。
設定ファイルは以下に保存されます。

```text
~/Library/Application Support/LinkPilot/linkpilot.config.json
```

## 開発

要件:

- Rust 1.80+
- Node.js 22 推奨
- npm
- フル Tauri デスクトップアプリには macOS が必要

リポジトリルートから:

```sh
cargo check --workspace --exclude linkpilot-desktop
cargo test -p linkpilot-core
cargo check -p linkpilot-desktop
```

フロントエンド:

```sh
cd apps/desktop
npm install
npm run build
npx tauri dev
```

本番 bundle:

```sh
cd apps/desktop
npm run bundle:mac
open ../../target/release/bundle/macos/LinkPilot.app
```

`bundle:mac` は `tauri build` を実行した後、生成された `Info.plist` を修正し、
macOS が LinkPilot を HTTP/HTTPS ブラウザハンドラーとして認識できるようにします。

## リポジトリ構成

```text
crates/
  core/                 # ルーティングエンジン、ルールモデル、ConfigStore、履歴、IPC protocol types
  platform-mac/         # macOS backend: ブラウザ検出、launcher、既定ブラウザ連携
  platform-win/         # Windows stub
  platform-linux/       # Linux stub
  ipc/                  # Unix socket / named pipe 上の length-prefixed JSON
  cli/                  # lpt command-line client
  headless-daemon/      # バックグラウンド daemon binary
  native-host/          # ブラウザ拡張向け native messaging bridge の予約領域
apps/
  desktop/              # Tauri 2 desktop app
    src-tauri/          # Rust shell、tray、deep links、commands、picker
    src/                # React + TypeScript UI
  extension/            # ブラウザ拡張 placeholder
packages/
  config-dsl/           # linkpilot.config.ts 用 TypeScript DSL
packaging/
  homebrew/             # 未公開の Formula/cask template。現在はインストール経路ではありません
```

## リリース

メンテナーは semver tag を push してリリースします。

```sh
git tag v0.3.0
git push origin v0.3.0
```

Release workflow はアーキテクチャごと（Apple Silicon `aarch64` と Intel `x86_64`）に
macOS CLI、daemon、デスクトップアプリをビルドし、それぞれの app bundle を修正して
アーキテクチャ別の DMG に包み、統一された `checksums.txt` とともに release artifacts を
アップロードします。

## 変更履歴

バージョンごとの変更は [CHANGELOG.md](CHANGELOG.md) を参照してください。

## ライセンス

LinkPilot は MIT License で提供されています。詳しくは [LICENSE](LICENSE) を参照してください。
