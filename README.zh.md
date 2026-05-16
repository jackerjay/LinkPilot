<p align="center">
  <img src="docs/brand/icon.png" alt="LinkPilot" width="128" height="128">
</p>

<h1 align="center">LinkPilot</h1>

<p align="center">
  <em>把每个链接路由到该去的浏览器、用户配置和工作空间。</em>
</p>

<p align="center">
  <a href="README.md">English</a> | 简体中文
</p>

LinkPilot 是一个 macOS 优先（Windows / Linux 后续跟进）的链接路由器：它坐在操作系统、浏览器和会打开 URL 的应用之间，把每条链接按你配的规则派发到对应的浏览器 + 用户配置。

## 状态

**v0.1 — 功能完整。** 工作空间、带菜单栏托盘的 Tauri 外壳、基于 fsnotify 的配置存储、路由历史、五个 GUI 页面，以及端到端的 macOS `lp open` 流程。品牌素材已就绪（`docs/brand/icon.png` 出的全套图标矩阵 + `docs/brand/tray-template.svg` 出的单色菜单栏模板）；结构化规则编辑器替换了 JSON-textarea fallback（仍可在"高级：原始 JSON"下使用）。`设为默认浏览器`（LaunchServices）、`登录时启动`（LaunchAgent plist）以及 daemon 的 Unix-socket IPC 服务端均已打通。

设计细节见 `docs/linkpilot-design-v0.1.md`（PRD）。

## 快速开始（macOS）

### CLI —— 不需要 GUI

```sh
cargo build -p linkpilot-cli
./target/debug/lp doctor              # 写入默认配置 + 列出浏览器
./target/debug/lp rules list
./target/debug/lp open https://github.com/anthropics/anthropic-cookbook
./target/debug/lp open https://figma.com --dry-run
./target/debug/lp open https://github.com --from-app Slack
```

当 daemon 在运行时，`lp` 走 Unix socket
（`~/Library/Application Support/LinkPilot/linkpilot.sock`）；否则回退到本地直接执行。`--local` 可强制走本地路径。写入操作始终走本地配置文件 —— daemon 的 fsnotify watcher 通过 anti-echo token 自动拾取变更，运行中的 GUI 一帧内就能刷新。

首次运行会在
`~/Library/Application Support/LinkPilot/linkpilot.config.json` 写入一份起始配置（PRD §22 示例：github / notion → Chrome Default，figma / youtube → Arc）。编辑后再跑一次即可。

CLI 已覆盖 GUI 所有可配置项 —— 用 `lp <command> --help` 查看完整参数：

```sh
# 规则
lp rules add --host "*.figma.com" --target arc --priority 20
lp rules add --host github.com --path "/oauth/*" --keep-source --priority 50
lp rules add --from-app Slack --ask
lp rules list --all                      # 包含已禁用的规则
lp rules disable <id 前缀>               # 8 位前缀足够
lp rules set-priority <id 前缀> 99
lp rules delete <id 前缀>
lp rules add --when-json '{"op":"any","of":[...]}' --then-json '{"kind":"block"}'

# 工作空间（批量开关一组规则）
lp workspaces add work --name Work
lp workspaces disable work               # 所有 workspace_id=work 的规则跳过

# 配置查看 + 导入/导出
lp config show                           # 整份配置 JSON
lp config path
lp config set-default-target arc --profile Personal
lp config export ./backup.json
lp config import ./backup.json

# 设置
lp settings show
lp settings smart-routing off            # 路由总开关
lp settings launch-at-login on
lp settings history-retention 30         # 或 `clear` 表示无限

# 浏览器
lp browsers list                         # 自动发现 + 自定义，合并后
lp browsers profiles chrome
lp browsers custom add --id devbuild --name "Chrome Canary" \
    --kind chromium --exec /Applications/Google\ Chrome\ Canary.app

# 默认浏览器注册
lp default-browser status
lp default-browser set                   # macOS 会弹系统确认
```

### 桌面应用

```sh
# 在仓库根目录
cd apps/desktop
npm install                          # 本地安装 @tauri-apps/cli
npx tauri dev                        # 同时拉起 Vite + Tauri
```

> **注意：** 用 `npx tauri …`（或 `npm run tauri -- …`）。Tauri CLI 是 npm devDependency；`cargo tauri …` 需要你额外 `cargo install tauri-cli`，本仓库**不要求**这么做。

可以试试：

- 关掉窗口后菜单栏图标还在 —— daemon 仍在运行。
- 从 Slack / Terminal 打开 https://github.com → LinkPilot 应当出现在应用选择器里（前提是已在系统设置里把它设为默认）。
- 在 JSON 编辑器里粘贴一份新配置 → 原子重写；外部 `vim` 改动会被自动重载（anti-echo token 防止循环）。
- Inspector 标签页通过 `route-logged` Tauri 事件实时展示每次路由决策。

### 生产打包

```sh
cd apps/desktop
npm run bundle:mac      # tauri build + 对 .app 跑 patch-info-plist.sh
open ../../target/release/bundle/macos/LinkPilot.app
```

`bundle:mac` 在 `tauri build` 之后修补 `.app` 里的 Info.plist，让 macOS"默认浏览器"选择器能识别 LinkPilot（细节见 `apps/desktop/scripts/patch-info-plist.sh`）。它同时产出的 DMG 是**未打 patch** 的 —— 要拿到带 plist patch 的 DMG，请推一个 `v*.*.*` tag，从 `release.yml` 产物里下载（见下文"发布"）。

品牌素材已经在 `apps/desktop/src-tauri/icons/` 备齐（从 `docs/brand/icon.png` + `docs/brand/tray-template.svg` 生成）。重新生成方法见 `apps/desktop/src-tauri/icons/README.md`。

## 仓库结构

```
crates/
  core/                # 路由引擎、规则模型、配置存储、fsnotify、
                       #   路由历史、平台 trait、IPC 协议类型
  platform-mac/        # macOS 后端（v0.1 真实实现）
  platform-win/        # Windows 占位（v0.5 真实实现）
  platform-linux/      # Linux 占位（v0.6+ 真实实现）
  ipc/                 # Unix socket / Named pipe 上的 length-prefixed JSON
                       #   （传输层就绪；服务端在后续切片上线）
  native-host/         # NMH stdio 桥（v0.3）
  cli/                 # `lp` 命令行客户端
  headless-daemon/     # 预留给未来 GUI-less daemon 二进制
apps/
  desktop/             # Tauri 2 应用
    src-tauri/         # Rust：tray、deep-link、commands、fsnotify 接线
    src/               # React + TypeScript 前端
      pages/           # menu-bar、rules、inspector、browsers、settings
      lib/             # 强类型 Tauri 命令封装
  extension/           # MV3 浏览器扩展（v0.3）
packages/
  config-dsl/          # @linkpilot/config TS DSL（v0.2+）
```

## 开发构建

```sh
cargo check --workspace --exclude linkpilot-desktop   # 核心栈，任何 OS 都能跑
cargo test  -p linkpilot-core                         # 路由 + 历史 + 配置
cargo check -p linkpilot-desktop                      # Tauri 应用，需要 macOS
                                                      # （或装了 GTK/WebKit
                                                      #  开发库的 Linux）
```

在 Linux 上交叉检查 macOS-only 的 Rust 代码：

```sh
rustup target add x86_64-apple-darwin
cargo check --target x86_64-apple-darwin -p linkpilot-platform-mac -p linkpilot-cli
```

（Tauri 外壳无法从 Linux 交叉检查，因为它通过 cc 链接真实的 Cocoa 框架；那部分得在 Mac 上构建。）

前端：

```sh
cd apps/desktop
npm install
npm run build       # tsc --noEmit + vite build → apps/desktop/dist/
```

## 参与贡献

LinkPilot 在 MIT OR Apache-2.0 双协议下开源。本地搭建、PR 期望、发布流程见 `CONTRIBUTING.md`。安全问题请走私密渠道上报，见 `SECURITY.md`。

## 发布

维护者通过推 semver tag 触发发布：

```sh
git tag v0.1.0
git push origin v0.1.0
```

tag 触发 `.github/workflows/release.yml`，在 `macos-latest` 上：

1. 把 `lp` CLI 同时编出 `x86_64-apple-darwin` 与 `aarch64-apple-darwin` 两份，再用 `lipo` 合成一个 universal 二进制。
2. 用 `--target universal-apple-darwin --bundles app` 跑 Tauri，让 `.app` 也是 universal。
3. 对产出的 `.app` 跑 `apps/desktop/scripts/patch-info-plist.sh` —— 把 `tauri-plugin-deep-link` 自动注入的 `CFBundleURLTypes` 改成 Viewer/`Default`，并加上 HTML 的 `CFBundleDocumentTypes`，让 macOS"默认浏览器"选择器真的能看到 LinkPilot。
4. 用 `hdiutil` 把打了 patch 的 `.app` 包成 `LinkPilot_<version>_universal.dmg`（如果第二次再跑 `tauri build --bundles dmg`，会重新生成 `.app` 把 patch 覆盖掉，所以这里走手工 `hdiutil`）。
5. 把 `lp-macos`、`lp-macos.tar.gz`、DMG 以及 `checksums.txt` 上传到 GitHub Release。

一份 universal DMG 同时支持 Apple Silicon 和 Intel Mac。

每个 PR 还会在 `macos-latest` 上跑 `desktop-bundle` 烟雾测试（`tauri build --debug --bundles app`），打包流水线一旦坏掉，在出 tag 前就会被发现。

当前的发布产物**未签名也未公证**。macOS 上首次启动可能需要先解除隔离：

```sh
xattr -dr com.apple.quarantine LinkPilot.app
```
