<p align="center">
  <img src="docs/brand/icon.png" alt="LinkPilot" width="128" height="128">
</p>

<h1 align="center">LinkPilot</h1>

<p align="center">
  <em>把每个链接路由到正确的浏览器、用户配置和工作空间。</em>
</p>

<p align="center">
  <a href="README.md">English</a> | 简体中文
</p>

LinkPilot 是一个 macOS 优先的链接路由器。它位于 macOS、浏览器和会打开
URL 的应用之间，根据规则把每个链接送到最合适的浏览器和用户配置。

LinkPilot 不是浏览器，而是一层轻量调度器，适合同时使用多个 Chrome
Profile、Arc、Safari、Firefox、工作空间，以及 Slack、Lark、Terminal、IDE
等来源应用的人。

## 当前状态

LinkPilot 当前聚焦 macOS。

- macOS 桌面应用：可用并持续迭代。
- CLI 与后台 daemon：可用并持续迭代。
- Windows / Linux 平台 crate：目前是占位实现。
- 浏览器扩展：预留给后续里程碑。

当前应用包含：

- 按 host、path、来源应用、来源浏览器、来源 profile 路由 URL。
- Chrome 系浏览器、Arc、Firefox、Safari 和自定义浏览器的识别与 profile 枚举。
- Ask picker：Halo profile 选择环、键盘快捷键、profile 排序、暗色模式，以及 Settings 里的真实测试 URL。
- 自动检查 GitHub Release 更新并下载 DMG，在 Settings 中由用户手动打开安装包。
- 后台 daemon 与 Unix socket IPC，主窗口关闭后仍可继续路由。
- 菜单栏托盘、Inspector、Test URL 模拟器、浏览器管理、Settings 和 onboarding。
- `lpt` CLI：打开 URL、管理规则、查看配置、安装 daemon、检查默认浏览器状态。

## 安装

当前 release 产物未签名，macOS 首次启动时可能会加 quarantine。安装后如无法打开，
可以移除 quarantine 标记。

Homebrew 目前还不是可用安装渠道。在 tap 发布并验证之前，请使用下面的 DMG 或
CLI tarball。

### GUI 应用

从最新 GitHub Release 下载 universal DMG，把 `LinkPilot.app` 复制到
`/Applications` 后打开：

```sh
curl -L https://github.com/jackerjay/LinkPilot/releases/latest/download/LinkPilot_<version>_universal.dmg -o LinkPilot.dmg
hdiutil attach LinkPilot.dmg
cp -R "/Volumes/LinkPilot/LinkPilot.app" /Applications/
hdiutil detach "/Volumes/LinkPilot"
xattr -dr com.apple.quarantine /Applications/LinkPilot.app
open /Applications/LinkPilot.app
```

首次启动后，通过 onboarding 或 Settings 完成：

1. 把 LinkPilot 注册为系统默认浏览器。
2. 安装后台 daemon LaunchAgent。
3. 把内置的 `lpt` 命令安装到 `~/.local/bin`。

Settings 默认会在启动后检查 GitHub Releases 中是否有新版本。如果发现新的
macOS DMG，LinkPilot 会下载到本地更新缓存，并提示你点击 “Open installer”
后再手动升级。可以在 Settings → General → Updates 中关闭。

### 仅 CLI

CLI tarball 包含 `lpt` 和 `linkpilot-daemon`。

```sh
curl -L https://github.com/jackerjay/LinkPilot/releases/latest/download/lpt-macos.tar.gz \
  | tar -xz -C ~/.local/bin
chmod +x ~/.local/bin/lpt
```

如果只需要终端自动化，或者只想运行后台 daemon，可以走 CLI-only 安装。

## 快速开始

### 桌面应用

1. 打开 LinkPilot。
2. 在 onboarding 或 Settings 里把 LinkPilot 设为默认浏览器。
3. 在 Rules 页面新增或编辑规则。
4. 用 Test URL 对真实路由引擎做 dry-run。
5. 用 Inspector 查看实时路由决策。

对于 Ask 规则，LinkPilot 会打开 picker 窗口。在多 profile 浏览器上按住
Option 会唤起 Halo 选择环；鼠标瞄准 profile 后松开 Option，就会直接打开。
Settings 页面提供 picker style 的测试 URL，可以直接验证焦点切换、profile
命中和视觉样式，不需要先创建真实规则。

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
# 规则
lpt rules list --all
lpt rules add --host "*.figma.com" --target arc --priority 20
lpt rules add --host github.com --path "/oauth/*" --keep-source --priority 50
lpt rules add --from-app Slack --ask
lpt rules disable <id-prefix>
lpt rules delete <id-prefix>

# 工作空间
lpt workspaces add work --name Work
lpt workspaces disable work

# 配置
lpt config show
lpt config path
lpt config set-default-target chrome --profile Default
lpt config export ./linkpilot.backup.json
lpt config import ./linkpilot.backup.json

# 设置
lpt settings show
lpt settings smart-routing off
lpt settings launch-at-login on
lpt settings auto-updates off
lpt settings history-retention 30

# 浏览器
lpt browsers list
lpt browsers profiles chrome
lpt browsers custom add --id devbuild --name "Chrome Canary" \
  --kind chromium --exec /Applications/Google\ Chrome\ Canary.app

# 默认浏览器和 daemon
lpt default-browser status
lpt default-browser set
lpt daemon status
lpt daemon install
lpt daemon logs --follow
```

`lpt` 会优先连接运行中的 daemon：

```text
~/Library/Application Support/LinkPilot/linkpilot.sock
```

没有 daemon 时，可本地执行的命令会回退到本地路径。配置文件位置：

```text
~/Library/Application Support/LinkPilot/linkpilot.config.json
```

## 开发

要求：

- Rust 1.80+
- 推荐 Node.js 22
- npm
- 完整 Tauri 桌面应用需要 macOS

仓库根目录：

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

生产打包：

```sh
cd apps/desktop
npm run bundle:mac
open ../../target/release/bundle/macos/LinkPilot.app
```

`bundle:mac` 会先运行 `tauri build`，再修补生成的 `Info.plist`，让 macOS 能把
LinkPilot 识别为 HTTP/HTTPS 浏览器处理器。

## 仓库结构

```text
crates/
  core/                 # 路由引擎、规则模型、ConfigStore、历史、IPC 协议类型
  platform-mac/         # macOS 后端：浏览器枚举、启动器、默认浏览器注册
  platform-win/         # Windows 占位
  platform-linux/       # Linux 占位
  ipc/                  # Unix socket / named pipe 上的 length-prefixed JSON
  cli/                  # lpt 命令行客户端
  headless-daemon/      # 后台 daemon 二进制
  native-host/          # 预留给浏览器扩展的 native messaging 桥
apps/
  desktop/              # Tauri 2 桌面应用
    src-tauri/          # Rust shell、tray、deep links、commands、picker
    src/                # React + TypeScript UI
  extension/            # 浏览器扩展占位
packages/
  config-dsl/           # linkpilot.config.ts 的 TypeScript DSL
packaging/
  homebrew/             # 未发布的 Formula/cask 模板；当前还不是安装渠道
```

## 发布

维护者通过推 semver tag 发布：

```sh
git tag v0.2.0
git push origin v0.2.0
```

Release workflow 会构建 universal macOS CLI、daemon 和桌面应用，修补 app bundle，
打包 DMG，并上传 release artifacts 与 checksums。

## 许可证

LinkPilot 使用 MIT License。见 [LICENSE](LICENSE)。
