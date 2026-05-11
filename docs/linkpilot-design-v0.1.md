# LinkPilot 产品需求方案 v0.1

**日期：** 2026-05-11
**定位：** macOS-first 的链接 / 浏览器 / Profile / Workspace 调度器

## 1. 产品一句话

LinkPilot 根据 URL、来源 App、当前浏览器上下文、用户身份和规则配置，把每一次链接打开行为路由到最合适的浏览器、Profile 或 Workspace。

更产品化的英文定位：

> Route every link to the right browser, profile, and workspace.

## 2. 背景与核心判断

传统 Browser Router 主要只能拦截系统层面的 `open https://...` 行为，例如 Slack、VSCode、Raycast、Terminal 打开的链接。它无法稳定处理浏览器内部点击、SPA 跳转、OAuth redirect、地址栏输入后的二次分发。

因此 LinkPilot 不应该只做“默认浏览器代理”，而应该做成：

> System URL Router + Browser Extension + Native Messaging Host + Rules Engine

浏览器插件负责观察浏览器内导航事件，原生应用负责真正执行跨浏览器 / 跨 Profile / 跨 Workspace 调度。Chrome 的 webNavigation API 可用于接收导航过程通知，tabs API 可用于创建、修改、移动、重载标签页，Native Messaging 可用于扩展与本地应用交换消息。Firefox WebExtensions 也支持 Native Messaging。Safari Web Extensions 可以迁移自其他浏览器扩展，但会被打包在 native app 中，且 Safari 的 Native Messaging 模型更偏向 extension 与 container app 通信。

参考资料：

- Chrome webNavigation API: https://developer.chrome.com/docs/extensions/reference/api/webNavigation
- Chrome tabs API: https://developer.chrome.com/docs/extensions/reference/api/tabs
- Chrome Native Messaging: https://developer.chrome.com/docs/extensions/develop/concepts/native-messaging
- Firefox Native Messaging: https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/Native_messaging
- Safari Web Extensions: https://developer.apple.com/videos/play/wwdc2020/10665/

## 3. 产品目标

### 3.1 核心目标

LinkPilot 要解决四类问题：

| 问题 | 当前痛点 | LinkPilot 方案 |
|---|---|---|
| 外部链接分流 | Slack / VSCode / Terminal 打开到错误浏览器 | macOS 默认浏览器代理拦截 |
| 浏览器内链接分流 | Chrome / Arc 内点击链接，本该进入另一个 Profile 或 Workspace | 浏览器插件检测导航并请求重路由 |
| Profile / 身份隔离 | 工作、个人、客户环境混在一起 | 规则引擎根据域名、来源、上下文路由 |
| OAuth / SSO 混乱 | 登录回调进入错误浏览器或 Profile | 上下文感知规则 + 回调保护策略 |

## 4. 非目标

MVP 阶段不做以下内容：

| 非目标 | 原因 |
|---|---|
| 不做通用浏览器替代品 | LinkPilot 是调度器，不是浏览器 |
| 不读取网页正文内容 | 权限过大，隐私风险高 |
| 不默认拦截所有浏览器内部导航 | 容易造成误跳转和循环 |
| 不默认收集远程遥测 | 本地工具应优先保持可信 |
| 不尝试绕过浏览器安全限制 | 通过官方扩展 API 和 Native Messaging 设计 |

## 5. 核心用户画像

### Persona A：多浏览器开发者

使用：

- Arc：日常浏览
- Chrome Work Profile：公司账号
- Chrome Personal Profile：个人账号
- Safari：支付、Apple 相关、银行
- Firefox：测试兼容性

需求：

> GitHub、Jira、Linear、Notion 自动去工作 Profile；YouTube、Reddit、个人 Gmail 自动去个人浏览器。

### Persona B：多客户顾问 / Freelancer

使用多个客户账号：

- Chrome Profile：Client A
- Chrome Profile：Client B
- Edge Profile：Enterprise Client
- Safari：个人

需求：

> 根据域名、账号、来源项目目录、当前 VPN 状态，把链接分发到对应客户 Workspace。

### Persona C：安全敏感用户

关注：

- Cookie 隔离
- 公司账号不混入个人浏览器
- OAuth 回调稳定
- 能看到每条链接为什么被路由

需求：

> 所有决策可解释、可回放、可手动覆盖。

## 6. 产品模块

```
External App / Terminal / macOS open
        ↓
LinkPilot URL Handler
        ↓
LinkPilot Daemon
        ↓
Rules Engine
        ↓
Browser Adapter
        ↓
Target Browser / Profile / Workspace


Browser Internal Navigation
        ↓
LinkPilot Browser Extension
        ↓
Native Messaging Host
        ↓
LinkPilot Daemon
        ↓
Rules Engine
        ↓
Browser Adapter
        ↓
Target Browser / Profile / Workspace
```

## 7. 功能需求

### 7.1 macOS 默认浏览器代理

LinkPilot App 注册为 macOS 默认浏览器，接收系统级 URL 打开请求。

输入：

```ts
{
  url: string
  openerApp?: {
    name: string
    bundleId: string
    pid?: number
  }
  timestamp: number
}
```

输出：

```ts
{
  action: "open" | "ask" | "block"
  target?: BrowserTarget
  reason: string
}
```

示例规则：

```ts
route("github.com")
  .from(app("Slack"))
  .to(chrome.profile("Work"))

route("figma.com")
  .to(arc.profile("Design"))

route("bank.com")
  .to(safari())
```

### 7.2 浏览器插件

插件负责：

- 观察浏览器内部导航
- 判断是否需要交给 LinkPilot 重路由
- 通过 Native Messaging 与本地 LinkPilot 通信
- 在需要时关闭、替换或回退当前标签页
- 提供手动“发送到某浏览器 / Profile”的入口

### 7.3 插件事件来源

#### A. webNavigation

用于感知浏览器内导航生命周期：

```
browser.webNavigation.onBeforeNavigate
browser.webNavigation.onCommitted
browser.webNavigation.onHistoryStateUpdated
browser.webNavigation.onCreatedNavigationTarget
```

适合：

- 普通链接点击
- 新标签打开
- 地址栏跳转
- SPA URL 变化
- redirect 检测

注意：webNavigation 更像导航状态通知，而不是一个完整的跨浏览器迁移机制。因此 LinkPilot 应把它当作“观察 + 补偿重路由”能力，而不是百分百无闪烁的同步拦截能力。

#### B. Content Script 点击捕获

用于更早捕获用户点击：

```js
document.addEventListener("click", detectAnchorClick, true)
```

适合：

- 用户点击 `<a>`
- 判断 modifier key：cmd, shift, option
- 判断链接文本、target、download 属性
- 判断当前页面上下文

但 content script 需要站点权限。MVP 不应该默认申请 `<all_urls>`，而应使用最小权限策略。

#### C. Context Menu

提供手动操作：

```
Open with LinkPilot →
  Chrome / Work
  Chrome / Personal
  Arc
  Safari
  Firefox
```

适合：

- 用户临时改派
- 调试规则
- 解决误分发
- 生成新规则

#### D. Toolbar Popup

用于显示：

- 当前页面匹配了哪些规则
- 当前浏览器 / Profile 是否正确
- 一键迁移当前 Tab
- 一键创建规则
- LinkPilot Native Host 是否在线

### 7.4 Native Messaging Host

浏览器插件不能直接控制 macOS 上的所有浏览器和 Profile，因此需要本地 Native Host 作为桥梁。

```
Browser Extension
    ↓ nativeMessaging
Native Messaging Host
    ↓ local IPC
LinkPilot Daemon
    ↓
Browser Adapter
```

建议不要让 Native Messaging Host 承担完整业务逻辑。它应该只是一个轻量桥接器：

| 模块 | 职责 |
|---|---|
| Extension | 收集浏览器内上下文 |
| Native Host | 消息桥接 |
| Daemon | 规则判断、状态存储、浏览器启动 |
| App UI | 配置、权限、可视化 |

原因：Native Host 生命周期可能跟连接状态相关，而 daemon 更适合承载长期状态、规则热更新、多浏览器共享状态、日志和调试。

## 8. 路由决策模型

### 8.1 Routing Context

每次决策输入：

```ts
type RoutingContext = {
  url: string

  source: {
    type: "system" | "browser-extension"
    appName?: string
    bundleId?: string
    browser?: "chrome" | "arc" | "safari" | "firefox" | "edge"
    profile?: string
    tabId?: number
    windowId?: number
  }

  navigation?: {
    transitionType?: "link" | "typed" | "form_submit" | "reload" | "generated"
    isNewTab?: boolean
    isRedirect?: boolean
    isHistoryStateUpdate?: boolean
    openerUrl?: string
    referrerUrl?: string
  }

  environment?: {
    network?: string
    vpn?: string
    time?: string
    workspace?: string
  }
}
```

### 8.2 Routing Decision

```ts
type RoutingDecision =
  | {
      action: "allow"
      reason: string
    }
  | {
      action: "open"
      target: BrowserTarget
      closeSourceTab?: boolean
      replaceSourceTabWith?: string
      reason: string
    }
  | {
      action: "ask"
      candidates: BrowserTarget[]
      reason: string
    }
  | {
      action: "block"
      reason: string
    }
```

### 8.3 Browser Target

```ts
type BrowserTarget = {
  browser: "Safari" | "Google Chrome" | "Arc" | "Firefox" | "Microsoft Edge"
  profile?: string
  workspace?: string
  incognito?: boolean
  newWindow?: boolean
}
```

## 9. 规则系统

### 9.1 规则设计原则

规则应该同时支持：

- 简单配置
- TypeScript DSL
- 可解释结果
- 可组合 matcher
- 手动覆盖
- 回放调试

### 9.2 配置示例

```ts
import {
  defineConfig,
  route,
  browser,
  app,
  from,
  tags,
  network
} from "@linkpilot/config"

export default defineConfig({
  defaultTarget: browser.arc(),

  rules: [
    route("github.com")
      .when(from.app("Slack"))
      .to(browser.chrome.profile("Work")),

    route(["linear.app", "notion.so", "figma.com"])
      .to(browser.chrome.profile("Work")),

    route("youtube.com")
      .to(browser.arc.profile("Personal")),

    route(tags.oauth())
      .toSameBrowser()
      .reason("Avoid breaking OAuth login flows"),

    route("*.corp.example.com")
      .when(network.vpn("corp"))
      .to(browser.chrome.profile("Corp")),

    route("bank.example.com")
      .to(browser.safari())
      .reason("Banking sites are pinned to Safari")
  ]
})
```

### 9.3 规则匹配维度

| 维度 | 示例 |
|---|---|
| URL host | `github.com`, `*.corp.com` |
| URL path | `/login`, `/oauth/callback` |
| URL query | `utm_source`, `redirect_uri` |
| 来源 App | Slack, VSCode, Terminal |
| 来源浏览器 | Chrome, Arc, Safari |
| 来源 Profile | Work, Personal |
| 当前页面 | 从 Notion 打开的 GitHub 链接 |
| 跳转类型 | click, typed, reload, form submit |
| 网络环境 | VPN, Wi-Fi SSID |
| 时间 | 工作日、工作时间 |
| Workspace | Work, Personal, Client A |
| 用户手动选择 | Always use this target |

## 10. 浏览器插件详细需求

### 10.1 MVP 插件能力

**P0：基础桥接**

| 功能 | 说明 |
|---|---|
| Native Host 连接检测 | 插件能判断 LinkPilot 是否在线 |
| 当前 Tab 发送到 LinkPilot | 用户手动把当前页面迁移到目标浏览器 |
| 右键菜单打开链接 | 对页面链接执行 “Open with LinkPilot” |
| 基础导航观察 | 监听 top-level navigation |
| 规则预览 | 显示当前 URL 命中的规则 |

**P1：自动浏览器内重路由**

| 功能 | 说明 |
|---|---|
| 内部链接自动检测 | 点击或打开新 Tab 时评估规则 |
| 错误浏览器自动迁移 | 发现当前浏览器不匹配时，打开目标浏览器 |
| 源 Tab 处理 | 可选择关闭、替换为空白页、保留 |
| 防循环机制 | 同一 URL + 来源 + 目标短时间内只处理一次 |
| OAuth 保护 | OAuth / SSO 流默认不跨浏览器迁移 |

**P2：Workspace 智能化**

| 功能 | 说明 |
|---|---|
| 当前 Workspace 感知 | 当前浏览器 Profile / Workspace 参与规则 |
| Tab Migration | 一键迁移当前 Tab 到目标 Profile |
| Bulk Migration | 批量迁移一组 Tab |
| Rule Suggestion | 根据用户反复手动选择推荐规则 |
| Explain Decision | 显示“为什么这个链接去了这个浏览器” |

## 11. 插件权限策略

### 11.1 原则

LinkPilot 插件应采用：

> 最小权限 + 可选权限 + 本地优先 + 明确解释

### 11.2 Chromium 权限建议

```json
{
  "manifest_version": 3,
  "name": "LinkPilot",
  "permissions": [
    "tabs",
    "webNavigation",
    "nativeMessaging",
    "contextMenus",
    "storage"
  ],
  "optional_host_permissions": [
    "https://*/*",
    "http://*/*"
  ]
}
```

| 权限 | 用途 |
|---|---|
| `tabs` | 获取/修改当前标签页、关闭错误来源 Tab |
| `webNavigation` | 监听导航事件 |
| `nativeMessaging` | 与本地 LinkPilot 通信 |
| `contextMenus` | 右键菜单 |
| `storage` | 缓存插件状态 |
| optional host permissions | 仅在需要内容脚本时申请 |

### 11.3 Safari 权限策略

Safari 版本不要直接照搬 Chromium 版本。Safari Web Extensions 被打包在 native app 内，且 Native Messaging 是 extension 与 container app 之间通信；这意味着 Safari 版 LinkPilot 更适合和 macOS App 作为同一个 Xcode 工程分发。

建议：

| 阶段 | Safari 策略 |
|---|---|
| MVP | 暂不优先做 Safari 插件 |
| Phase 2 | 做 Safari Web Extension |
| Phase 2.5 | 和 LinkPilot macOS App 打包发布 |
| Phase 3 | App Store 分发与签名 |

## 12. 关键用户流程

### 12.1 外部 App 打开链接

```
Slack opens https://github.com/org/repo
        ↓
macOS sends URL to LinkPilot
        ↓
LinkPilot sees opener = Slack
        ↓
Rule matched: github.com + Slack → Chrome Work
        ↓
Open Chrome with Work Profile
```

用户体验：

> 用户点击 Slack 中的 GitHub 链接，自动进入 Chrome Work Profile。

### 12.2 浏览器内部链接迁移

```
User clicks GitHub link inside Arc Personal
        ↓
LinkPilot extension detects navigation
        ↓
Extension sends context to native host
        ↓
Daemon evaluates rule
        ↓
Decision: should open in Chrome Work
        ↓
Daemon opens Chrome Work
        ↓
Extension closes or reverts source tab
```

用户体验：

> 用户在个人浏览器里点到工作站点，LinkPilot 自动把它迁移到工作浏览器。

### 12.3 OAuth 保护流程

```
User starts login in Chrome Work
        ↓
OAuth redirects to login provider
        ↓
Rule sees oauth/callback/login context
        ↓
Decision: keep same browser
```

用户体验：

> 登录流程不会被 LinkPilot 搞断。

### 12.4 手动生成规则

```
User right-clicks link
        ↓
Open with LinkPilot → Chrome Work
        ↓
Toast: Always open github.com this way?
        ↓
User confirms
        ↓
Rule generated
```

## 13. 边界情况与处理策略

| 场景 | 风险 | 策略 |
|---|---|---|
| POST 表单提交 | 无法安全跨浏览器重放 | 不迁移，只提示 |
| OAuth / SSO | 跨浏览器会丢 session | 默认 keep same browser |
| SPA pushState | 频繁触发 | 默认仅记录，不自动迁移 |
| 下载链接 | 迁移可能丢失下载动作 | 不自动迁移 |
| `chrome://`, `about:`, `file://` | 浏览器内部协议 | 默认忽略 |
| 无限重路由 | A → B → A 循环 | 加 routing token / TTL |
| Native Host 离线 | 插件无法路由 | 显示 degraded mode |
| 用户地址栏输入 | 自动迁移可能突兀 | 默认 ask 或 allow |
| 新窗口 / 新标签 | 关闭源 Tab 体验敏感 | 用户可配置策略 |

## 14. 技术架构建议

### 14.1 推荐技术栈

| 层 | 技术 |
|---|---|
| macOS App | SwiftUI |
| Daemon / Core | Rust |
| Rule Engine | TypeScript DSL + Rust evaluator 或 JS runtime |
| Browser Extension | TypeScript + Manifest V3 |
| Native Host | Rust 小型桥接进程 |
| IPC | Unix Domain Socket |
| Config | `linkpilot.config.ts` |
| CLI | `lp` |

### 14.2 为什么 Native Host 只做桥接

推荐：

```
Extension
  ↓
Native Host
  ↓ Unix socket
Daemon
  ↓
Rule Engine
```

不推荐：

```
Extension
  ↓
Native Host with full rules
```

原因：

- Native Host 可能随连接生命周期启动/退出
- 状态管理困难
- 规则热更新困难
- 多浏览器插件共享状态困难
- 日志和调试困难

## 15. 消息协议草案

### 15.1 插件 → Native Host

```ts
type ExtensionMessage =
  | {
      type: "route.evaluate"
      requestId: string
      context: RoutingContext
    }
  | {
      type: "route.open"
      requestId: string
      context: RoutingContext
      userInitiated: boolean
    }
  | {
      type: "state.ping"
      requestId: string
    }
```

### 15.2 Native Host → 插件

```ts
type ExtensionResponse =
  | {
      type: "route.decision"
      requestId: string
      decision: RoutingDecision
    }
  | {
      type: "state.pong"
      requestId: string
      daemonOnline: boolean
      version: string
    }
  | {
      type: "error"
      requestId: string
      code: string
      message: string
    }
```

### 15.3 防循环 Token

每次 LinkPilot 主动打开 URL 时追加内存级 routing token，不一定写入 URL，可存在 daemon 的短期 cache：

```ts
{
  urlHash: "sha256(url)",
  source: "arc.personal",
  target: "chrome.work",
  expiresAt: Date.now() + 3000
}
```

插件检测到同 URL 再次触发时，询问 daemon 是否已处理，避免循环。

## 16. UI 需求

### 16.1 Menu Bar App

菜单栏显示：

```
LinkPilot
✓ Running

Default Router: Enabled
Browser Extension:
  Chrome: Connected
  Arc: Connected
  Firefox: Not Installed
  Safari: Not Installed

Recent Routes
Rules
Profiles
Settings
Quit
```

### 16.2 Route Inspector

每条路由记录显示：

| 字段 | 示例 |
|---|---|
| URL | `https://github.com/org/repo` |
| Source | Slack |
| Matched Rule | `github.com from Slack → Chrome Work` |
| Target | Chrome / Work |
| Decision | Open |
| Time | 10:42:31 |
| Override | “Always use Arc instead” |

### 16.3 插件 Popup

```
LinkPilot

Current Page:
github.com/org/repo

Matched:
github.com → Chrome Work

Current:
Arc Personal

Actions:
[Move to Chrome Work]
[Always open this domain here]
[Explain]
```

## 17. MVP 范围

### Phase 0：Native Router MVP

目标：先做出一个可用的 LinkPilot。

功能：

- 注册为 macOS 默认浏览器
- 解析配置
- 根据 URL 分发到浏览器
- 支持 Chrome Profile
- 支持 opener app
- Menu Bar 状态
- 基础日志

不包含：

- 浏览器插件
- 浏览器内导航迁移

### Phase 1：Chromium 插件 MVP

目标：补齐浏览器内点击能力。

支持浏览器：

- Google Chrome
- Arc
- Microsoft Edge
- Chromium

功能：

- Manifest V3 插件
- Native Messaging Host
- 右键菜单 “Open with LinkPilot”
- 当前 Tab “Move to…”
- webNavigation 监听 top-level navigation
- 防循环
- 基础规则解释

### Phase 2：Firefox 插件

目标：覆盖 Firefox 用户。

功能：

- Firefox WebExtension
- Native Messaging Host manifest
- Firefox tabs / webNavigation 对接
- Firefox-specific adapter

### Phase 3：Safari 插件

目标：覆盖 Safari，尤其是支付、Apple、银行类场景。

功能：

- Safari Web Extension
- 与 LinkPilot macOS App 打包
- Native Messaging 到 container app
- App Store / notarization / signing 流程

## 18. 成功指标

### MVP 指标

| 指标 | 目标 |
|---|---|
| 外部链接分发成功率 | ≥ 99% |
| 插件 Native Host 连接成功率 | ≥ 95% |
| 错误重路由循环 | 0 个已知稳定复现 |
| 手动 override 生效率 | ≥ 99% |
| 用户能解释路由原因 | Route Inspector 可展示每次决策 |

### 体验指标

| 指标 | 目标 |
|---|---|
| 首次配置完成时间 | < 5 分钟 |
| 创建第一条规则 | < 60 秒 |
| 插件权限解释 | 用户能理解为什么需要权限 |
| 错误恢复 | Native Host 离线时可提示并 fallback |

## 19. 风险与缓解

| 风险 | 说明 | 缓解 |
|---|---|---|
| 插件权限吓人 | `tabs`, `webNavigation`, host permissions 容易让用户不安 | 默认最小权限，按需申请 |
| 跳转闪烁 | webNavigation 更像通知，不是完美同步拦截 | Content Script 提前捕获点击；可配置保守模式 |
| OAuth 被打断 | 跨浏览器后 session 丢失 | OAuth 标签默认 keep same browser |
| Safari 实现复杂 | Safari extension 需要 native app packaging | Safari 放 Phase 3 |
| Profile 控制不稳定 | 不同浏览器 Profile 启动参数不同 | Browser Adapter 抽象层 |
| 规则冲突 | 多条规则同时命中 | 明确 priority + explain |
| 无限循环 | A 浏览器插件把链接送 B，B 又送回 A | routing token + TTL |
| Native Host 生命周期 | Host 可能随连接断开退出 | Daemon 常驻，host 仅桥接 |

## 20. 版本路线图

### v0.1 — Native Router

- macOS 默认浏览器接管
- URL rule routing
- Chrome Profile 支持
- Menu Bar 基础状态
- CLI：`lp open`, `lp doctor`

### v0.2 — Rule DSL

- `linkpilot.config.ts`
- rule priority
- rule explain
- config hot reload
- Route Inspector

### v0.3 — Chromium Extension

- Chrome / Arc / Edge 插件
- Native Messaging Host
- 当前 Tab 迁移
- 右键菜单
- 基础自动重路由

### v0.4 — Browser Workspace

- Browser/Profile inventory
- Workspace model
- OAuth protection
- rule suggestions

### v0.5 — Firefox

- Firefox extension
- Firefox Native Messaging
- Firefox-specific adapter

### v0.6 — Safari

- Safari Web Extension
- macOS App container
- Safari-specific permission UX

## 21. 推荐 MVP 定义

第一版不要野心太大。最合理的 MVP 是：

> LinkPilot v0.1：一个 macOS 默认浏览器代理，支持 TypeScript 规则配置和 Chrome Profile 分发。

第二版再上：

> LinkPilot v0.3：一个 Chromium 插件，能把浏览器内部误开的链接迁移到正确浏览器 / Profile。

这样可以避免一开始就陷入 Safari、Firefox、权限、App Store、插件审查、Native Messaging 多浏览器差异这些复杂度里。

## 22. 最小可行 Demo

建议第一个 Demo 做这个：

```ts
export default defineConfig({
  defaultTarget: browser.arc(),

  rules: [
    route("github.com").to(browser.chrome.profile("Work")),
    route("notion.so").to(browser.chrome.profile("Work")),
    route("figma.com").to(browser.arc()),
    route("youtube.com").to(browser.arc.profile("Personal")),
    route(tags.oauth()).toSameBrowser()
  ]
})
```

Demo 验收：

- 从 Slack 打开 GitHub → Chrome Work
- 从 Terminal `open https://figma.com` → Arc
- 在 Arc Personal 内点击 GitHub → 自动迁移到 Chrome Work
- OAuth 登录流程不被迁移
- Route Inspector 能解释每一次决策

## 23. Arc 支持策略补充

Arc 可以作为 LinkPilot 的目标浏览器，但 Arc Space 需要谨慎建模。

### 23.1 Arc 能力判断

Arc 官方提供 Air Traffic Control，可在 Arc 内部按 URL 规则把链接路由到指定 Space。入口为：

```
Arc > Settings > Links > Air Traffic Control
```

该能力适合：

- `github.com` → Work Space
- `figma.com` → Design Space
- `youtube.com` → Personal Space

参考资料：

- Arc Air Traffic Control: https://resources.arc.net/hc/en-us/articles/22932014625431-Air-Traffic-Control-Automate-Your-Link-Routing
- Arc Spaces: https://resources.arc.net/hc/en-us/articles/19228064149143-Spaces-Distinct-Browsing-Areas
- Arc Profiles: https://resources.arc.net/hc/en-us/articles/19227964556183-Profiles-Separate-Work-Personal-Browsing

### 23.2 外部直接指定 Arc Space 的限制

目前不应假设 Arc 存在稳定公开的外部 API / CLI / URL scheme，可以让 LinkPilot 直接执行：

```
open -a "Arc" --space "Work" "https://github.com"
```

或：

```
arc://open?url=https://github.com&space=Work
```

因此 Arc Space 应被视为“软目标”，不是像 Chrome Profile 那样稳定可外部强制指定的目标。

### 23.3 LinkPilot 的 Arc Adapter 建议

```ts
type ArcTarget = {
  browser: "Arc"
  space?: string
  profile?: string
  mode?: "native" | "air-traffic-control" | "automation"
}
```

能力矩阵：

| 能力 | Arc 支持情况 |
|---|---|
| 打开 URL 到 Arc | 支持 |
| 根据 URL 自动进指定 Space | Arc 内置 Air Traffic Control 支持 |
| LinkPilot 外部直接指定 Space | 暂无稳定公开接口 |
| LinkPilot 外部直接指定 Profile | 暂无稳定公开接口 |
| 通过插件迁移当前 Tab | 可做 |
| 通过自动化切 Space 再打开 | 可实验，不稳定 |
| 让用户手动选择目标 Space | 可通过 Arc 内部机制处理，不应强依赖 |

MVP 阶段建议：

```ts
route("figma.com").to(arc())
route("github.com").to(chrome.profile("Work"))
```

不要一开始承诺：

```ts
route("github.com").to(arc.space("Work"))
```

除非标成：

```ts
route("github.com").to(arc.space("Work", {
  strategy: "air-traffic-control"
}))
```

含义是：

> LinkPilot 负责识别规则，Arc Space 路由由 Arc 自己的 Air Traffic Control 完成。

## 24. 最终产品方向

LinkPilot 不只是“浏览器切换器”。

更准确的方向是：

> Personal Link Orchestration Layer

即用户工作流里的链接调度层。它最终可以扩展到：

- Browser routing
- Profile routing
- Workspace routing
- OAuth/session protection
- VPN-aware routing
- Project-aware routing
- Team policy routing
- Browser extension-assisted tab migration

这个方向比单纯复刻 Finicky 更有空间。
