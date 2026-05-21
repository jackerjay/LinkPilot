<p align="center">
  <img src="docs/brand/icon.png" alt="LinkPilot" width="128" height="128">
</p>

<h1 align="center">LinkPilot</h1>

<p align="center">
  <em>把每個連結路由到正確的瀏覽器、個人設定檔和工作區。</em>
</p>

<p align="center">
  <a href="README.md">English</a> | <a href="README.zh.md">简体中文</a> | 繁體中文 | <a href="README.ja-JP.md">日本語</a>
</p>

LinkPilot 是一個以 macOS 為優先的連結路由器。它位於 macOS、瀏覽器，以及會
開啟 URL 的應用程式之間，依照規則把每個連結送到最合適的瀏覽器與個人設定檔。

LinkPilot 不是瀏覽器，而是一層輕量的路由層，適合同時使用多個 Chrome
Profile、Arc、Safari、Firefox、工作區，以及 Slack、Lark、Terminal、IDE
等來源應用程式的人。

## 目前狀態

LinkPilot 目前聚焦於 macOS。

- macOS 桌面應用程式：可用並持續迭代。
- CLI 與背景 daemon：可用並持續迭代。
- Windows / Linux 平台 crate：目前是佔位實作。
- 瀏覽器擴充功能：保留給後續里程碑。

目前應用程式包含：

- 依 host、path、來源應用程式、來源瀏覽器和來源 profile 路由 URL。
- Chrome 系瀏覽器、Arc、Firefox、Safari 和自訂瀏覽器的識別與 profile 枚舉。
- Ask picker：Halo profile 選擇環（Frosted / Bezel / Crown 三種風格）、鍵盤快捷鍵、每個瀏覽器的 profile 排序、深色模式，以及 Settings 裡的真實測試 URL。
- 介面語言支援 English、简体中文、繁體中文、日本語，並可選擇跟隨系統語言。
- 自動檢查 GitHub Release 更新，下載的 DMG 會驗證 SHA-256，再由使用者在 Settings 中手動開啟安裝程式。
- 背景 daemon 與 Unix socket IPC，主視窗關閉後仍可繼續路由。
- 選單列托盤、Inspector、Test URL 模擬器、瀏覽器管理、Settings 和 onboarding。
- `lpt` CLI：開啟 URL、管理規則、查看設定、安裝 daemon、檢查預設瀏覽器狀態。

## 安裝

目前 release 產物尚未簽署，macOS 第一次啟動時可能會加上 quarantine。安裝後如果
無法開啟，可以移除 quarantine 標記。

Homebrew 目前還不是支援的安裝管道。在 tap 發布並驗證之前，請使用下面的 DMG 或
CLI tarball。

### GUI 應用程式

從最新 GitHub Release 下載 universal DMG，把 `LinkPilot.app` 複製到
`/Applications` 後開啟：

```sh
curl -L https://github.com/jackerjay/LinkPilot/releases/latest/download/LinkPilot_<version>_universal.dmg -o LinkPilot.dmg
hdiutil attach LinkPilot.dmg
cp -R "/Volumes/LinkPilot/LinkPilot.app" /Applications/
hdiutil detach "/Volumes/LinkPilot"
xattr -dr com.apple.quarantine /Applications/LinkPilot.app
open /Applications/LinkPilot.app
```

首次啟動後，透過 onboarding 或 Settings 完成：

1. 把 LinkPilot 註冊為系統預設瀏覽器。
2. 安裝背景 daemon LaunchAgent。
3. 把內建的 `lpt` 命令安裝到 `~/.local/bin`。

Settings 預設會在啟動後檢查 GitHub Releases 中是否有新版本。如果發現新的
macOS DMG，LinkPilot 會下載到本機更新快取，比對 release 的 `checksums.txt`
中的 SHA-256，再提示你點擊 "Open installer" 手動升級。沒有 `checksums.txt`
的 release 會被視為未驗證，並拒絕自動下載。可以在
Settings → General → Updates 中關閉這個行為。

## 支援語言

LinkPilot 目前內建以下介面語言：

- English
- 简体中文
- 繁體中文
- 日本語

預設情況下，當系統語言命中上述語言時，LinkPilot 會自動跟隨系統。你也可以在
Settings → Appearance → Language 中手動切換。Picker 視窗會在下次開啟時讀取同一
語言偏好。

也可以透過 CLI 修改：

```sh
lpt settings language system
lpt settings language en
lpt settings language zh-CN
lpt settings language zh-TW
lpt settings language ja-JP
```

### 僅 CLI

CLI tarball 包含 `lpt` 和 `linkpilot-daemon`。

```sh
curl -L https://github.com/jackerjay/LinkPilot/releases/latest/download/lpt-macos.tar.gz \
  | tar -xz -C ~/.local/bin
chmod +x ~/.local/bin/lpt
```

如果只需要終端機自動化，或只想執行背景 daemon，可以使用 CLI-only 安裝。

## 快速開始

### 桌面應用程式

1. 開啟 LinkPilot。
2. 在 onboarding 或 Settings 裡把 LinkPilot 設為預設瀏覽器。
3. 在 Rules 頁面新增或編輯規則。
4. 用 Test URL 對真實路由引擎做 dry-run。
5. 用 Inspector 查看即時路由決策。

對於 Ask 規則，LinkPilot 會開啟 picker 視窗。在多 profile 瀏覽器上按住
Option 會喚起 Halo 選擇環；滑鼠瞄準 profile 後鬆開 Option，就會直接開啟。
Settings → Appearance 可以切換三種 Halo 風格（Frosted / Bezel / Crown）、
依瀏覽器自訂 profile 順序（位置 1–9 對應鍵盤快捷鍵），並用測試 URL 真實
開啟瀏覽器來驗證焦點切換、profile 命中和視覺樣式，不需要先建立真實規則。
當某個瀏覽器新增了你尚未排進 Halo 的 profile，Settings 會用 `+N new`
標記提示，避免新 profile 被靜默隱藏。

### CLI

```sh
cargo build -p linkpilot-cli
./target/debug/lpt doctor
./target/debug/lpt open https://github.com
./target/debug/lpt open https://figma.com --dry-run
./target/debug/lpt open https://github.com --from-app Slack
```

常用命令：

```sh
# 規則
lpt rules list --all
lpt rules add --host "*.figma.com" --target arc --priority 20
lpt rules add --host github.com --path "/oauth/*" --keep-source --priority 50
lpt rules add --from-app Slack --ask
lpt rules disable <id-prefix>
lpt rules delete <id-prefix>

# 工作區
lpt workspaces add work --name Work
lpt workspaces disable work

# 設定檔
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
lpt settings language zh-TW            # system | en | zh-CN | zh-TW | ja-JP
lpt settings history-retention 30

# 瀏覽器
lpt browsers list
lpt browsers profiles chrome
lpt browsers custom add --id devbuild --name "Chrome Canary" \
  --kind chromium --exec /Applications/Google\ Chrome\ Canary.app

# 預設瀏覽器和 daemon
lpt default-browser status
lpt default-browser set
lpt daemon status
lpt daemon install
lpt daemon logs --follow
```

`lpt` 會優先連接執行中的 daemon：

```text
~/Library/Application Support/LinkPilot/linkpilot.sock
```

沒有 daemon 時，可在本機執行的命令會回退到本機路徑。設定檔位置：

```text
~/Library/Application Support/LinkPilot/linkpilot.config.json
```

## 開發

需求：

- Rust 1.80+
- 建議 Node.js 22
- npm
- 完整 Tauri 桌面應用程式需要 macOS

倉庫根目錄：

```sh
cargo check --workspace --exclude linkpilot-desktop
cargo test -p linkpilot-core
cargo check -p linkpilot-desktop
```

前端：

```sh
cd apps/desktop
npm install
npm run build
npx tauri dev
```

正式打包：

```sh
cd apps/desktop
npm run bundle:mac
open ../../target/release/bundle/macos/LinkPilot.app
```

`bundle:mac` 會先執行 `tauri build`，再修補生成的 `Info.plist`，讓 macOS 能把
LinkPilot 識別為 HTTP/HTTPS 瀏覽器處理器。

## 倉庫結構

```text
crates/
  core/                 # 路由引擎、規則模型、ConfigStore、歷史、IPC 協議型別
  platform-mac/         # macOS 後端：瀏覽器枚舉、啟動器、預設瀏覽器註冊
  platform-win/         # Windows 佔位
  platform-linux/       # Linux 佔位
  ipc/                  # Unix socket / named pipe 上的 length-prefixed JSON
  cli/                  # lpt 命令列用戶端
  headless-daemon/      # 背景 daemon 二進位檔
  native-host/          # 保留給瀏覽器擴充功能的 native messaging 橋接
apps/
  desktop/              # Tauri 2 桌面應用程式
    src-tauri/          # Rust shell、tray、deep links、commands、picker
    src/                # React + TypeScript UI
  extension/            # 瀏覽器擴充功能佔位
packages/
  config-dsl/           # linkpilot.config.ts 的 TypeScript DSL
packaging/
  homebrew/             # 未發布的 Formula/cask 範本；目前還不是安裝管道
```

## 發布

維護者透過推送 semver tag 發布：

```sh
git tag v0.3.0
git push origin v0.3.0
```

Release workflow 會構建 universal macOS CLI、daemon 和桌面應用程式，修補 app
bundle，打包 DMG，並上傳 release artifacts 與 checksums。

## 更新日誌

按版本歸檔的變更見 [CHANGELOG.md](CHANGELOG.md)。

## 授權

LinkPilot 使用 MIT License。見 [LICENSE](LICENSE)。
