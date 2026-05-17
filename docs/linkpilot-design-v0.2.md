# LinkPilot 产品需求方案 v0.2

**日期：** 2026-05-17
**定位：** 让 LinkPilot 在没有 GUI 时也能完整工作，并把"规则"升级成可版本化的代码

## 1. 目标一句话

> 拆出独立 daemon，让 CLI-only 安装方式也能享受路由历史、后台服务、默认浏览器托管；同时上线 `@linkpilot/config` TypeScript DSL，让团队规则进入代码审查流。

## 2. 出发点

v0.1 已经把"在装了 GUI 的 macOS 桌面上完成链接路由"这件事做完了。v0.2 不再扩外延（不进 Windows、不进 Firefox、不进浏览器扩展），而是补齐几条现在仍有妥协的内部链路。

### 2.1 v0.1 之后的现实差距

| 现状 | 落差 |
|---|---|
| daemon 内嵌在 `linkpilot-desktop`（Tauri shell）里 | CLI-only 安装的用户没有运行中的 daemon —— 没历史、没 IPC、没 fsnotify 联动 |
| `crates/headless-daemon` 是占位空 crate | v0.1 注释里早写了"reserved for a future GUI-less daemon"，迁移没动 |
| Rule 来源只有 GUI / 原始 JSON | 团队规则不能进 git diff，PR review 看不到结构化变更 |
| `packages/config-dsl` 是占位空目录 | v0.1 PRD §20 原本说 v0.2 上 Rule DSL —— 此事仍未做 |
| 发布物未签名未公证 | 用户首次启动得 `xattr -dr com.apple.quarantine LinkPilot.app`，新人会被劝退 |
| 没有 Homebrew 渠道 | 安装走 release DMG / curl tarball 两条路，但都不是 mac 老炮的肌肉记忆 |

### 2.2 v0.1 PRD 对 v0.2 的承诺（§20）

原话："`linkpilot.config.ts`, rule priority, rule explain, config hot reload, Route Inspector"。其中 rule priority、rule explain、config hot reload、Route Inspector 在 v0.1 实际全做完了 —— 唯一漏的就是 `linkpilot.config.ts`。本版本收尾。

## 3. 范围

### 3.1 In-scope（v0.2 必做）

| 编号 | 主题 | 一句话 |
|---|---|---|
| **A** | 独立 daemon binary | `linkpilot-daemon` 二进制 + LaunchAgent，CLI-only 模式也能后台跑；GUI 首启自动安装 |
| **B** | TypeScript 配置 DSL | `@linkpilot/config` 包 + `lp config compile`（默认 bun runtime） |
| **C** | CLI history + daemon 管理 | `lp history` 走新 IPC verb，`lp daemon start/stop/status/install` 管理后台 |
| **D** | Homebrew tap 发布 | `jackerjay/homebrew-linkpilot` 上线 formula + cask；本期暂不签名 |

### 3.2 Out-of-scope（推迟到 v0.3+）

| 主题 | 推迟原因 |
|---|---|
| Chromium 浏览器扩展 + NMH | v0.1 PRD §20 已把扩展划在 v0.3，本期不破规则 |
| Firefox / Safari 扩展 | 仍按 v0.1 PRD §20 节奏，v0.5 / v0.6 |
| Windows / Linux 真实 platform 实现 | `platform-win` v0.5、`platform-linux` v0.6+；v0.2 维持 macOS-first |
| 新 MatcherTree 维度（time / VPN / referrer） | 留给 DSL 落地后驱动需求 —— 先看用户真用 DSL 表达什么再扩 |
| Tauri auto-updater | 需要先把签名链路打通；与 v0.2 平行做风险大，推 v0.3 |
| 远程同步 / 多设备 | v0.1 §4 非目标里就明确过，不进 |

### 3.3 Stretch（可选，看 M4 之后是否还有 buffer）

- 把 macOS-only 的 picker.rs `ActivationPolicy` 网关补成 `#[cfg]` —— 让 `cargo check -p linkpilot-desktop` 在 Linux 上重新过。
- `lp rules explain <id>` —— 用 `Router::evaluate_explained` 给单条规则打印 matcher 树评估迹。

## 4. 模块设计

### 4.1 A — 独立 daemon binary

#### 4.1.1 目标

`linkpilot-daemon` 二进制可以独立运行（无 Tauri 窗口），承担今天 Tauri shell 承担的全部 daemon 职责：

- 拥有 `ConfigStore`、fsnotify watcher
- 持有 `RouteHistory` ring buffer
- 跑 `linkpilot_ipc::server`，供 `lp` 客户端和未来的 NMH 接入
- 调用 `linkpilot-platform-mac` 做浏览器探测、URL 启动、默认浏览器握手

GUI 仍然存在，但启动时检查 daemon socket：

- daemon 已在跑 → GUI 作为 IPC 客户端启动，自己不开第二个 `ConfigStore` / IPC server
- daemon 未在跑 → GUI 自己起 daemon（向后兼容 v0.1 行为）

这避免了"GUI + LaunchAgent daemon 同时跑"导致两个进程抢同一份 socket / 同一份配置文件的失败模式。

#### 4.1.2 进程模型

```
┌─────────────────────┐         ┌────────────────────┐
│ linkpilot-daemon    │◀────────│ lp (CLI)           │
│ (LaunchAgent 拉起)  │  IPC    └────────────────────┘
│  - ConfigStore      │         ┌────────────────────┐
│  - RouteHistory     │◀────────│ LinkPilot.app(GUI) │
│  - IPC server       │         └────────────────────┘
│  - URL launcher     │
└─────────────────────┘
```

GUI 首次运行（dev 模式以外）自动调一次 `linkpilot-platform-mac::launch_agent::install`，把 daemon plist 写到 `~/Library/LaunchAgents/`，从此机器重启 daemon 自动起。用户可显式 `lp daemon uninstall` 关掉。dev 模式（`npx tauri dev`）跳过这步，避免污染开发机。

#### 4.1.3 文件落点

| 路径 | 内容 |
|---|---|
| `crates/headless-daemon/src/main.rs` | 二进制入口；从 `linkpilot-core` / `linkpilot-platform-mac` 拼装 `AppState` 等价物 |
| `crates/headless-daemon/src/state.rs` | 提取 GUI `AppState` 中与窗口/Tauri 无关的部分 |
| `crates/core/src/daemon.rs`（新） | 抽出 daemon 的纯逻辑（state 装配 + IPC handler 注册），让 GUI 与 headless 共享 |
| `apps/desktop/src-tauri/src/lib.rs` | 启动时检测 IPC socket 是否已被占用，决定"接管 daemon"还是"作 IPC client"两条路径 |
| `~/Library/LaunchAgents/app.linkpilot.daemon.plist`（运行时） | `linkpilot-daemon --serve` 的 LaunchAgent；由新增的 `lp daemon install` 写入 |

#### 4.1.4 排他锁

socket 是天然的互斥点：v0.1 已经在 `linkpilot-ipc::server::serve` 里检测 socket 占用并 warn。v0.2 把这条提升为硬约束 ——

- daemon 启动：抢 socket，抢不到就 fail-fast 并打 actionable 日志
- GUI 启动：试连 socket，连得上就以 client 身份跑，不再 spawn daemon

### 4.2 B — TypeScript 配置 DSL

#### 4.2.1 包结构

```
packages/config-dsl/
  package.json          # name: @linkpilot/config
  tsconfig.json
  src/
    index.ts            # defineConfig, browser, route, matchers
    targets.ts          # browser.chrome.profile(...) 等 helper
    matchers.ts         # url, fromApp, fromBrowser, ... 的 helper
    compile.ts          # DSL 对象 → ConfigDocument JSON
  README.md
```

发布到 npm 上 `@linkpilot/config@0.2.0`。

#### 4.2.2 表达力

mirror v0.1 PRD §22 的目标 demo：

```ts
import { defineConfig, browser, route } from "@linkpilot/config";

export default defineConfig({
  defaultTarget: browser.arc(),
  rules: [
    route.host("github.com").to(browser.chrome.profile("Work")),
    route.host("notion.so").to(browser.chrome.profile("Work")),
    route.host("figma.com").to(browser.arc()),
    route.host("youtube.com").to(browser.arc.profile("Personal")),
    route.path("/oauth/*").keepSource(),
    route.fromApp("Slack").to(browser.chrome.profile("Work")),
  ],
  workspaces: [
    { id: "work", displayName: "Work" },
  ],
  settings: {
    smartRoutingEnabled: true,
  },
});
```

DSL 必须能表达 v0.2 时 `MatcherTree` / `Action` / `Settings` 的全部已存在字段。MatcherTree 的 AND/OR/NOT 通过 helper 链式组合：

```ts
route.all(route.host("github.com"), route.path("/oauth/*")).keepSource();
route.any(route.host("a.com"), route.host("b.com")).block();
route.not(route.fromApp("Slack")).to(browser.chrome());
```

#### 4.2.3 编译路径

```sh
# 把 linkpilot.config.ts 编译成 daemon 能读的 JSON 配置
lp config compile linkpilot.config.ts

# 默认写到 ~/Library/Application Support/LinkPilot/linkpilot.config.json
# --to PATH 自定义输出
lp config compile linkpilot.config.ts --to ./compiled.json
```

`lp config compile` 内部：

1. 用 `bun` 执行 TS 文件，拿到 `defineConfig` 返回的对象。bun 单次启动 ~30ms、原生支持 .ts、不需要项目级 tsconfig，是 DSL 编译器最轻量的选择。
2. 用 `@linkpilot/config` 的 `compile()` 把对象转成 `ConfigDocument` 形状
3. 走 v0.1 已有的 `ConfigStore::replace(doc, WriterId::TsCompiled)`

`WriterId::TsCompiled` 在 v0.1 已经存在（`crates/core/src/config/mod.rs`），但还没生产者。v0.2 让 `lp config compile` 成为它的第一个生产者。

`bun` 不存在时 `lp config compile` 直接报错并附 actionable 提示：

```
linkpilot: bun not found on PATH. Install with:
    brew install oven-sh/bun/bun
or:
    curl -fsSL https://bun.sh/install | bash
```

不做 `node` / `deno` fallback —— 多 runtime 等于多三倍 CI 矩阵和三倍 bug 表面。bun 不在的机器先装 bun。

#### 4.2.4 来源标记 + GUI 行为

`RuleSource::TsCompiled` 在 v0.1 也已经存在。GUI 在 v0.2 要做的事：

- Rules 页面把 `source: TsCompiled` 的规则渲染成只读（右上角标"compiled from .ts"）
- 用户编辑动作（toggle enabled / 改 priority）改成 disabled，hover 提示"请在 linkpilot.config.ts 里改并重跑 `lp config compile`"
- 提供"覆盖"按钮：把那条 TsCompiled 规则复制成 `RuleSource::Gui` 后再编辑（破坏一次性 ts → json 单向流，但给用户应急出口）

### 4.3 C — CLI history + daemon 管理子命令

#### 4.3.1 新 IPC verb

```rust
// crates/core/src/protocol.rs
pub enum Request {
    // ... 现有
    RouteHistory { request_id: String, limit: Option<usize> },
}

pub enum Response {
    // ... 现有
    RouteHistorySnapshot { request_id: String, records: Vec<RouteRecord> },
}
```

`PROTOCOL_VERSION` 升到 `2`。daemon handler 直接调 `state.history.recent(limit)`。

#### 4.3.2 新 CLI 子命令

```
lp history [--limit N] [--json]   # 默认 100；--json 每行一条 RouteRecord JSON

lp daemon
  status                          # 是否在跑、socket 路径、daemon 版本
  start                           # 启动后台 daemon（直接 fork 或 launchctl）
  stop                            # 杀掉 daemon 进程
  restart                         # stop + start
  install                         # 写入 LaunchAgent plist（开机自启）
  uninstall                       # 移除 LaunchAgent plist
  logs [--follow] [--lines N]     # tail daemon 日志
```

`lp daemon install/uninstall` 复用 `linkpilot-platform-mac::launch_agent` 模块（v0.1 已有，给 GUI 用）。

`lp daemon logs` 假定 daemon 日志在 `~/Library/Logs/LinkPilot/daemon.log`（LaunchAgent plist 的 `StandardOutPath` 指过去）。

#### 4.3.3 写操作语义不变

v0.1 已确立"CLI 写操作走本地文件原子重写，daemon 靠 fsnotify 拾取"的约定 ——
v0.2 daemon 独立后这条约定继续成立。daemon 和 GUI 都监听同一份文件，CLI 的写依然不需要走 IPC。

### 4.4 D — Homebrew tap 发布

v0.2 暂不接 Apple Developer Program。release.yml 继续输出 unsigned DMG + unsigned `lp`，用户首次启动仍需要 `xattr -dr com.apple.quarantine LinkPilot.app`（v0.1 既有行为）。签名 + 公证延后到 v0.3 或者 maintainer 决定开 Developer 账号时再做 —— 届时只需要往 release.yml 加 `codesign` / `notarytool` 两个 step，本期所有上游产物路径不变。

#### 4.4.1 Homebrew tap

新建仓库：`jackerjay/homebrew-linkpilot`

```ruby
# Formula/linkpilot-cli.rb  —— 仅 CLI
class LinkpilotCli < Formula
  desc "Route every link to the right browser, profile, and workspace (CLI)"
  homepage "https://github.com/jackerjay/LinkPilot"
  url "https://github.com/jackerjay/LinkPilot/releases/download/v0.2.0/lp-macos.tar.gz"
  sha256 "..."
  license "MIT OR Apache-2.0"

  def install
    bin.install "lp-macos" => "lp"
  end

  test do
    system "#{bin}/lp", "--version"
  end
end

# Casks/linkpilot.rb  —— GUI + CLI
cask "linkpilot" do
  version "0.2.0"
  sha256 "..."
  url "https://github.com/jackerjay/LinkPilot/releases/download/v0.2.0/LinkPilot_#{version}_universal.dmg"
  name "LinkPilot"
  desc "Route every link to the right browser, profile, and workspace"
  homepage "https://github.com/jackerjay/LinkPilot"
  app "LinkPilot.app"
  zap trash: [
    "~/Library/Application Support/LinkPilot",
    "~/Library/LaunchAgents/app.linkpilot.daemon.plist",
    "~/Library/Logs/LinkPilot",
  ]
end
```

`release.yml` 加一个 job，在 release 发布后自动更新 tap 仓库的 formula/cask（用 `mislav/bump-homebrew-formula-action` 或类似）。需要在 LinkPilot 仓库存一个 `HOMEBREW_TAP_TOKEN` secret —— maintainer 自己的 personal access token，作用域限定到 `jackerjay/homebrew-linkpilot` 仓库的 `contents: write`。

#### 4.4.2 用户最终命令

```sh
brew tap jackerjay/linkpilot
brew install linkpilot-cli                # 仅 CLI
brew install --cask linkpilot             # GUI + CLI（DMG 内嵌 lp）
```

## 5. 协议变更

### 5.1 新增

| Verb | 用途 |
|---|---|
| `Request::RouteHistory { limit }` | 拉取 daemon 内存里的最近路由 |
| `Response::RouteHistorySnapshot { records }` | 对应响应 |

### 5.2 升版

`PROTOCOL_VERSION: 1 → 2`。客户端在 `StatePing` 后比较版本；不匹配 → 警告但仍尝试发请求（向后兼容：v0.2 daemon 收到 v0.1 客户端的请求时 verb 集合是超集，没问题；反过来 v0.1 daemon 收到 v0.2 的 `RouteHistory` 会 fall through 到 `Response::Error { code: "unknown-verb" }`，CLI 优雅降级到"日志只在 GUI"提示）。

### 5.3 不变

`ConfigDocument` schema 不动。`SCHEMA_VERSION` 仍是 `1`。这是 v0.2 不破坏 v0.1 用户配置的硬约束。

## 6. 迁移与兼容

### 6.1 v0.1 → v0.2 升级路径

1. 用户从 brew 或 DMG 升级到 v0.2 的 `LinkPilot.app`。
2. 启动后 GUI 检测：本机有 `linkpilot-daemon` 二进制 + 没有跑着的 daemon socket → 后台 fork `linkpilot-daemon`。
3. GUI 自身降级为 IPC 客户端。
4. 用户配置 `linkpilot.config.json` 不变，daemon 直接接管。

### 6.2 卸载

`brew uninstall --zap linkpilot` 触发 Cask 里 `zap` 段：清除 config、LaunchAgent、日志。仅 CLI 用户走 `brew uninstall linkpilot-cli` —— 不动 config（保留路由历史相关的偏好）。

### 6.3 GUI 与 daemon 版本不一致

只允许同主版本号配对运行。GUI 启动时通过 `StatePing` 拿到 daemon 版本：

- 同 v0.2.x → OK
- daemon 是 v0.1.x → GUI 提示"请把 LinkPilot 卸了重装让 daemon 升级"
- daemon 是 v0.3+ → GUI 报错并提示"GUI 太旧"

## 7. 验收

### 7.1 功能验收

| 编号 | 验收点 |
|---|---|
| A-1 | `brew install linkpilot-cli` 安装；`pgrep linkpilot-daemon` 空（daemon 不自动启）；`lp daemon start` 后能跑 |
| A-2 | 关闭 GUI，daemon 继续跑；`lp open https://figma.com` 命中规则、调起浏览器 |
| A-3 | GUI 首启自动写入 LaunchAgent；机器重启后 daemon 自动起；`lp daemon uninstall` 可移除 |
| B-1 | `linkpilot.config.ts` 用 DSL 重写 v0.1 demo 配置，`lp config compile` 后 daemon 加载，路由结果与 JSON 配置等价 |
| B-2 | GUI 打开 TsCompiled 来源的规则：编辑控件 disabled，hover 提示存在 |
| C-1 | `lp history --limit 5 --json` 输出最近 5 条 `RouteRecord` JSON |
| C-2 | `lp daemon status` 显示 daemon 版本、socket 路径、PID、是否 LaunchAgent 拉起 |
| D-1 | 从 GitHub Release 下载 DMG，按 README 指示 `xattr -dr com.apple.quarantine` 后能启动（与 v0.1 一致；签名公证延后） |
| D-2 | `brew install --cask linkpilot` 把 DMG 装好，`/Applications/LinkPilot.app` 可启动 |
| D-3 | `brew tap jackerjay/linkpilot && brew install linkpilot-cli` 装到 `lp` 命令在 PATH 上 |

### 7.2 非功能验收

- Daemon 冷启动 ≤ 500ms（v0.1 GUI 冷启动 ~2s 主要是 Tauri webview，daemon 无窗口应当显著更快）。
- Daemon 内存常驻 ≤ 30MB（v0.1 GUI 整体 ~80MB，拆分后 daemon 部分应当显著少）。
- DSL 编译 ≤ 1s（典型 ~50 条规则的 .ts 文件）。
- 协议向后兼容：v0.1 客户端连接 v0.2 daemon 的所有 v0.1 verb 仍工作。

## 8. 风险与缓解

| 风险 | 缓解 |
|---|---|
| Daemon 拆分破坏 v0.1 GUI 内嵌 daemon 的代码路径 | 共享逻辑下沉到 `linkpilot-core::daemon`，GUI 和 headless 都从那构造；测试覆盖必须包含"daemon-in-GUI" 和 "daemon-out-of-GUI"两种模式 |
| 同时跑双 daemon 引发 socket 抢占 / 配置撕裂 | socket 抢占作硬约束，第二实例直接退出；fsnotify echo token 仍生效防止配置写丢失 |
| DSL 表达力跟不上后续 MatcherTree 扩展 | DSL 用 builder pattern + escape hatch `route.fromJson({...})`；新 matcher 上线时同步发 `@linkpilot/config` minor 版本 |
| bun 没装 | `lp config compile` 先探测 `bun --version`；没有就报错并附 `brew install oven-sh/bun/bun` 提示 |
| 默认 LaunchAgent 自启被用户视为"偷偷写文件" | 首启 GUI 弹一次系统通知"LinkPilot daemon installed to LaunchAgents — manage via Settings > Background service"；Settings 页加"Uninstall background service"按钮，对应 `lp daemon uninstall` |
| Homebrew 用户期望 `brew upgrade linkpilot` 也升 CLI | tap 里 formula 和 cask 同时 bump；release.yml 加 bump action 串联 |

## 9. 里程碑

| 里程碑 | 内容 | 交付物 |
|---|---|---|
| **M1** | A：headless daemon binary 跑通 | `cargo build -p linkpilot-headless-daemon` 出 `linkpilot-daemon`；本地能起、能被 `lp` 连上 |
| **M2** | A 完成 + C：daemon 管理命令 | `lp daemon start/stop/status/install/uninstall/logs` 全部跑通；LaunchAgent 安装可逆 |
| **M3** | C 剩余：`lp history` + 协议升 v2 | `lp history --json` 端到端；v0.1 客户端连 v0.2 daemon 仍 OK |
| **M4** | B：`@linkpilot/config` 包 + `lp config compile` | npm 上 `@linkpilot/config@0.2.0-rc.1`；DSL 能编出与 v0.1 demo 等价的 JSON |
| **M5** | D：Homebrew tap 上线 | `jackerjay/homebrew-linkpilot` 仓库 + formula + cask；`brew install --cask linkpilot` / `brew install linkpilot-cli` 可装；release.yml bump tap |
| **M6** | v0.2.0 tag + Release notes | DMG + tarball + Homebrew tap 同步更新；v0.1 用户走文档升级路径无回归 |

## 10. 已敲定的决策

| 决策 | 选定 | 影响 |
|---|---|---|
| Apple Developer Program | **暂不开**（推迟到 v0.3 或更晚） | D 不含签名/公证，DMG 维持 unsigned；用户首启仍需 `xattr -dr com.apple.quarantine` —— 与 v0.1 一致。release.yml 不动 codesign 流程 |
| TS DSL runtime | **bun**（不做 node/deno fallback） | `lp config compile` 硬依赖 bun，没装时给 actionable 安装提示 |
| Homebrew tap 仓库 | **`jackerjay/homebrew-linkpilot`** | 私人 owner；release.yml 用 maintainer 的 PAT bump tap |
| daemon LaunchAgent 自启 | **默认开**（GUI 首启自动 install） | 用户装完即用；Settings 页提供 "Uninstall background service" 出口，首启发系统通知告知 |

## 11. 与 v0.1 的边界

v0.2 不会重写 v0.1 任何已发布功能。v0.1 路由引擎、规则模型、配置文件 schema、GUI 页面布局保持不动。v0.2 只在以下点新增：

- 一个新二进制：`linkpilot-daemon`
- 一个新 npm 包：`@linkpilot/config`
- 五个新 CLI 子命令：`history`、`daemon {start/stop/status/install/uninstall/logs}`、`config compile`
- 两个新 IPC verb
- 一份 LaunchAgent plist 模板
- 一个 Homebrew tap 仓库

v0.1 用户即使不动 `linkpilot.config.json`、不安装 LaunchAgent、不写 .ts，升级到 v0.2 后行为与 v0.1 一致。

## 12. 文档更新计划

v0.2 发布同步更新：

- `README.md` / `README.zh.md` —— "Install" 段加 Homebrew 命令；"Quick start" 加 `lp daemon` 例子；新增 "Config as code" 段示范 `.ts` 用法
- `CLAUDE.md` —— "CI / Release" 段补 Homebrew tap bump 步骤；"Project skills" 加可能新建的 `add-cli-command` skill
- `docs/linkpilot-design-v0.2.md` —— 本文档
- `docs/migration-v0.1-to-v0.2.md`（新） —— 一页升级指南
- `packages/config-dsl/README.md`（新） —— DSL 使用文档

## 13. M1 详细开工清单

M1 = "headless daemon binary 跑通"。一旦本里程碑过线，剩下 4 个里程碑（M2 daemon CLI、M3 history、M4 DSL、M5 Homebrew）就有了独立 daemon 这个共同基础。

### 13.1 拆分共享 daemon 逻辑到 core

| 任务 | 文件 |
|---|---|
| 抽 `DaemonRuntime` —— 持有 `ConfigStore`、`RouteHistory`、`Arc<dyn PlatformProvider>` 的纯数据结构 | `crates/core/src/daemon.rs`（新） |
| 抽 `bootstrap()` 函数 —— 从 config path + bundle id 装配 `DaemonRuntime` | 同上 |
| 抽 `RequestHandler` 实现 —— 把 `ipc_host::DaemonHandler::handle` 的逻辑搬过来，让 Tauri shell 与 headless 共用 | 同上 |
| GUI `apps/desktop/src-tauri/src/state.rs` 改成包一层 `DaemonRuntime` 而不是自己拼装 | `state.rs` |
| GUI `ipc_host.rs` 删除重复逻辑，转调 `core::daemon::handle_request` | `ipc_host.rs` |

验收：`cargo test -p linkpilot-core` 12 个测试仍全过；GUI 编译过、跑起来路由行为与 v0.1 一致。

### 13.2 实现 `linkpilot-headless-daemon` binary

| 任务 | 文件 |
|---|---|
| Cargo.toml 加依赖：`linkpilot-core`、`linkpilot-ipc`、`tokio`、`tracing`、`tracing-subscriber`、`anyhow`；macOS-only 加 `linkpilot-platform-mac` | `crates/headless-daemon/Cargo.toml` |
| `main.rs` 入口：`clap` 解析 `--socket PATH` / `--config PATH` / `--version`；`tracing_subscriber` 初始化（默认日志到 stderr，LaunchAgent plist 把 StandardErrorPath 指过去） | `crates/headless-daemon/src/main.rs`（新） |
| 调 `core::daemon::bootstrap` 装配 runtime；调 `linkpilot_ipc::server::serve` 绑 socket | 同上 |
| socket 占用检测：抢不到立刻 `eprintln!` actionable 信息后 `exit(1)`，避免静默 | 同上 |
| 安装 SIGTERM/SIGINT handler，shutdown 时干净关 socket | 同上 |
| Cargo.toml workspace 顶层确认 `members` 已含 `crates/headless-daemon`（v0.1 已有 ✓） | `Cargo.toml` |

验收：`cargo build --release -p linkpilot-headless-daemon` 出 `target/release/linkpilot-daemon`；手动 `target/release/linkpilot-daemon --serve` 在 foreground 跑得起来、`lp doctor` 走 IPC 拿到 daemon 版本。

### 13.3 GUI 协同：探测 → 客户端模式 OR 自起 daemon

| 任务 | 文件 |
|---|---|
| `lib.rs` setup 阶段：试 `linkpilot_ipc::client::send(StatePing)` —— 成功 → daemon 已在跑，记录 "client mode"；失败 → 走原 v0.1 路径（in-process daemon） | `apps/desktop/src-tauri/src/lib.rs` |
| Client mode 下：跳过 fsnotify 自建（监听 daemon 广播）、跳过 IPC server 绑定；其余 Tauri command 改成转发到 daemon | `lib.rs` + `commands/mod.rs` |
| 标记 dev 模式（检查 `tauri::is_dev()` 或 `current_exe()` 落在 `target/debug`）→ 永远走 in-process daemon，方便迭代 | `lib.rs` |

验收：先起 `linkpilot-daemon --serve`，再 `npx tauri dev` —— GUI 启动日志出现 "joining existing daemon"；killdaemon 后重启 GUI —— 日志变 "spawned in-process daemon"。

### 13.4 GUI 首启自动安装 LaunchAgent

| 任务 | 文件 |
|---|---|
| 复用 v0.1 `linkpilot-platform-mac::launch_agent::install`；如果 v0.1 该模块原本只装 GUI 的 plist，这次加重载支持，让 plist 指向 daemon 二进制 | `crates/platform-mac/src/launch_agent.rs` |
| Plist 模板：`StandardOutPath` / `StandardErrorPath` 写到 `~/Library/Logs/LinkPilot/daemon.{out,err}.log`；`ProgramArguments` 指 `LinkPilot.app/Contents/MacOS/linkpilot-daemon --serve` | 同上 |
| GUI 首次启动检测：plist 不存在 + 非 dev 模式 → 调 install + spawn daemon；plist 存在 → 跳过（已装过） | `apps/desktop/src-tauri/src/lib.rs` |
| 首次安装后用 `tauri::Notification` 弹一次："LinkPilot background service installed. Manage in Settings." | 同上 |
| Settings 页加 "Background service" 卡片：状态 + Uninstall 按钮（调 `linkpilot-platform-mac::launch_agent::uninstall` + 终止 daemon） | `apps/desktop/src/pages/settings.tsx` |

验收：首次开 LinkPilot.app —— LaunchAgent plist 写进 `~/Library/LaunchAgents/`；`launchctl list | grep linkpilot` 显示 daemon；重启机器后 daemon 自动起；Settings 页 Uninstall 后 plist 消失、daemon 进程消失。

### 13.5 release.yml 嵌 daemon binary

| 任务 | 文件 |
|---|---|
| 编译 `linkpilot-headless-daemon` 两个 arch + lipo（与现行 `lp` 套同样模板） | `.github/workflows/release.yml` |
| 嵌入 `LinkPilot.app/Contents/MacOS/linkpilot-daemon`，权限 +x | 同上 |
| 上传 `linkpilot-daemon-macos.tar.gz` 作为独立 release 产物，给"CLI-only 用户也要后台服务"的场景用 | 同上 |

验收：tag `v0.2.0-alpha.1` 推上去，release artifacts 含 `lp-macos.tar.gz` + `linkpilot-daemon-macos.tar.gz` + 含 daemon 的 DMG + checksums.txt。

### 13.6 集成回归测试

| 场景 | 期望 |
|---|---|
| `linkpilot-daemon --serve` 后台跑 + `lp open https://figma.com --dry-run` | IPC 命中、返回 figma 路由决策 |
| daemon kill 后 `lp open https://figma.com --dry-run` | 退化到 local 路径，路由结果一致 |
| daemon 跑着时启动 GUI | GUI 显示 "joined daemon"，路由历史在 GUI Inspector 与 daemon `route_history` IPC 都能看到 |
| 两个 daemon 同时启动 | 第二个 fail-fast，不破坏第一个 |
| 卸载 LaunchAgent 后机器重启 | daemon 不自动起；`lp daemon status` 报 "not running" |

### 13.7 M1 不做的事

留给后续里程碑，M1 不引入额外动作面，避免范围爆炸：

- ❌ `lp daemon start/stop/restart/logs` 子命令 —— M2 做
- ❌ `Request::RouteHistory` 协议变更 —— M3 做
- ❌ DSL —— M4 做
- ❌ Homebrew —— M5 做

M1 完成时 `lp daemon install/uninstall/status` 这三个子命令也可以先到位（属于 daemon 管理面里最常用的 3 个），但不是硬要求；如果对齐 M1 时间盒紧，留 M2 集中做也行。
