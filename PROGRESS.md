# LinkPilot v0.2 — 推进记录

> 当前分支:`claude/v0.2-dev`  
> 当前版本:`0.2.0-alpha.3`(已发布为 GitHub Prerelease)  
> 最近一次成功的 release run:`26014703643`(2026-05-18)

设计文档:[`docs/linkpilot-design-v0.2.md`](docs/linkpilot-design-v0.2.md)

## 调整后的 milestone 结构(2026-05-18)

把所有 **对外公开发布动作** 集中到 M6;M2-M5 都只做实现 + **本地验证**。这样 M4/M5 推进时不会被"npm scope 注册没?tap repo 建了没?"这类账号 / 仓库准备阻塞,做完一气呵成后再统一发布。

| Milestone | 内容 | 状态 |
|---|---|---|
| **M1** | A · headless daemon binary 跑通 | ✅ 完成(云端 PR #10 合入) |
| **M2** | A 完成 + C · daemon 管理命令 | ✅ 完成 |
| **M3** | C 剩余 · `lpt history` + 协议升 v2 | ✅ 完成 |
| **M4** | B · `@linkpilot/config` DSL + `lpt config compile`(实现 + **本地** e2e) | ✅ 后端完成(M4.1-M4.5,backend 6/6 自动化通过),GUI 行为留 manual checklist |
| **M5** | Homebrew formula / cask 起草 + **本地** `brew install` 验证(**不** push tap repo) | ✅ 完成,见下 |
| **M6** | **公开发布** · v0.2.0 tag + Release notes + npm publish workflow + push homebrew-linkpilot tap + 升级路径文档 | ⏳ 未开始 |

测试基线:**Rust 47 passed**,**Bun 12 passed**(M3 起 + M4 新增)

---

## M1 — Headless daemon binary(完成)

迁移自 v0.1 的"GUI 内置 daemon" → v0.2 的"独立 `linkpilot-daemon` 二进制 + GUI 探测"。云端完成,5 commits 已合到 `claude/v0.2-dev`:

- `5a63ef5` 抽共享 daemon 逻辑到 `core::daemon`
- `3a0cc72` `linkpilot-daemon` 二进制 + clean socket shutdown
- `02f7768` GUI 探测外部 daemon,跳过自己的 IPC server bind
- `81cac4b` LaunchAgent + 首启自动安装
- `f44e01f` `release.yml` 嵌入 daemon binary 到 `.app`

---

## M2 — Daemon CLI 管理(完成)

CLI-only 用户可以全程不打开 GUI 管理 daemon。共 4 个 commit:

| 子任务 | Commit | 关键交付 |
|---|---|---|
| M2.1 PID 文件 + stale 清理 | `980a0c8` | `core::daemon::{pid_file_path, write/read/remove/cleanup_stale_pid_file, process_is_alive}`;daemon 启动写 PID、shutdown 清,Unix `kill(pid, 0)` 检测 stale |
| M2.3 LaunchAgent 模块化 | `b318e3f` | `LaunchAgentStatus.exec_path` 从 plist 解析;`read_exec_path_from_plist`/`parse_pid_from_launchctl_output` 抽成 pub(crate) 纯函数加测试 |
| M2.2 CLI `lpt daemon ...` | `324e8df` | 7 个 action:`start`/`stop`/`restart`/`status`/`install`/`uninstall`/`logs`;status 双输出(human + `--json`);binary 定位 4 级 fallback;`stop` 拒绝 launchd-managed daemon;`start` 用 `setsid` 脱离 terminal |
| M2.4 集成测试 | `a1e881f` | `crates/cli/tests/daemon_subcommand_smoke.rs` — 4 个 tests 锁 clap 结构 + JSON schema |

**已实际验证场景**(design §14.1.4):`lpt daemon stop` 在 launchd-managed 下正确拒绝、`lpt daemon uninstall` 删除 plist + unload、`status --json` 字段完整、`logs` 读真实日志、PID stale 单测覆盖。

**已知 follow-up**(超出 M2 scope):你环境的 LaunchAgent plist 推断成 `OnDemand=true`,配合残留 unix socket 会让 daemon EADDRINUSE 立即退出。M3 后该修补 daemon 启动逻辑(bind 前清理 stale socket)。

---

## M3 — `lpt history` + IPC v2(完成)

把 `RouteHistory` 暴露给 CLI,协议升 1→2,加 forward-compat fallback。共 5 个 commit:

| 子任务 | Commit | 关键交付 |
|---|---|---|
| M3.1 协议升级 | `12199b5` | `Request::RouteHistory{limit}` + `Response::RouteHistorySnapshot{records}`;`PROTOCOL_VERSION` 2;`ERROR_UNKNOWN_VERB` 常量;`read_raw_frame`/`peek_request_id` transport helper;daemon handler arm + 5 protocol tests + 2 handler tests |
| M3.2 IPC unknown-verb fallback | `83671f7` | `serve_connection_unix` 改 raw-read + decode-then-fallback,decode 失败回 `Error{code:"unknown-verb"}` 并保持 connection 开,**不再 drop**;integration test 验证 unknown verb → Error + 同 socket 后续 Ping → Pong |
| M3.3 CLI `lpt history` | `b009c21` + clippy `4d9cf99` | 子命令 + `--limit`/`--json`/`hist` 别名;三类错误统一路由到"daemon protocol too old"升级提示(Offline / Transport(Closed/Serde) / Error{unknown-verb});表格输出沿用 `lpt rules list` 列形 |
| M3.4 集成测试 | `107a9f1` | CLI smoke(help/别名)+ IPC roundtrip(自起 DaemonRuntime,RouteHistory verb 端到端,limit 截断 + newest-first 顺序) |

**已实际验证场景**(design §14.2.5):`lpt history --json | jq` schema、daemon 离线友好提示、老 daemon 升级提示、真实 daemon + 3 record 端到端 newest-first、limit 截断。

---

## M4 — TypeScript DSL `@linkpilot/config`(完成,等 GUI 手动确认)

`packages/config-dsl/` 是个独立的 npm publishable 包(包元数据就绪 — 等 M6 才推到 npm registry)。

| 子任务 | Commit | 关键交付 |
|---|---|---|
| M4.1 包骨架 + builders | `36ece85` | `@linkpilot/config@0.2.0-rc.1`(ESM/strict TS 5.5);`src/{index,types,targets,matchers,compile}.ts`;`defineConfig` + `browser.{chrome,arc,firefox,safari,edge,brave,vivaldi,custom}`(callable & chainable)+ `route.{host,path,fromApp,fromBrowser,fromProfile,all,any,not,always,fromJson}` + `printConfig` 助手;`examples/v0.1-demo.ts` 复刻 PRD §22 demo |
| M4.2 编译器 + 双语测试 | `100e378` + `71ad235` | **Bun 12 tests** 覆盖 wire shape(snake_case)、所有 MatcherTree / Action 变体、settings/workspace 默认值、modifiers、escape hatch、v0.1 demo 等价性;**Rust 1 test** (`crates/core/tests/dsl_roundtrip.rs`)实际 spawn `bun run`,parse stdout 进 `ConfigDocument` 验证;`@types/bun` devDep 解 IDE diagnostic |
| M4.3 CLI `lpt config compile` | `4eb9fda` | `ConfigAction::Compile{source, to}`;bun 探测 + brew/curl 安装提示;TS 错误 stderr 直通;`--to PATH` 写文件或默认 `ConfigStore::replace(doc, WriterId::TsCompiled)` |
| M4.4 GUI ts-compiled 只读化 | `251655f` | `apps/desktop/src/pages/rules.tsx`:`compiled` badge + tooltip;Edit/Delete 控件禁用(`<span tabIndex>` 包装以让 tooltip 可触发);新增 `CopyPlus` 按钮,克隆 rule 为新 UUID + `source: "gui"` + note 加 audit trail |
| M4.5 本地 e2e 验证 | `scripts/m4-verify.sh` | 6/6 后端场景自动化通过(见下)。GUI 行为留 manual checklist |

### M4.5 自动化验收结果(design §14.3.6,跑 `./scripts/m4-verify.sh`)

```
✓ 1) compile DSL → isolated config; verify rule count + tags
    (7 rules,所有 source: ts-compiled)
✓ 2) --to PATH wrote 7 rules to file; live config untouched
    (binary-diff $CFG before vs after,确认未触碰)
✓ 3) compile is idempotent in shape but re-issues UUIDs
    (跑两次 compile,rule[0].id 不同,内容一致)
✓ 4) route.fromJson({op:'url-host',...}) matcher 原样落进 config
✓ 5) bun missing → exit 1 with brew + curl install hints
    (用 stripped PATH 模拟)
✓ 6) bad TS exits 1 with bun stderr surfaced
```

**Manual GUI checklist**(2026-05-19 通过 `corepack yarn tauri dev` + mixed config 验证):
- [x] Rules 页:ts-compiled rule 显示 `compiled` badge,hover 出 tooltip
- [x] Edit pencil + Delete trash 在 ts-compiled rule 上禁用,hover 提示去 `lpt config compile`
- [x] `CopyPlus` 按钮存在并工作,克隆出的 rule `source: gui` + note 追加 `(copied from ts-compiled)`
- [x] gui rules 对照组:无 compiled badge,Edit/Delete 可点,无 CopyPlus 按钮
- [x] 改 `linkpilot.config.ts` 后再 compile,GUI 在 ~1s 内自动刷新(fsnotify)

验证流程:把 `bun run packages/config-dsl/examples/v0.1-demo.ts` 输出与 prod config 用 `jq` 合并成 14-rule mixed config,`lpt config import` 写入,`corepack yarn tauri dev` 起一个独立 GUI 窗口(M1.3 探测到 prod daemon socket → 进 client mode),user 在 Rules 页逐项确认。验收后从 `/tmp/lp-prod-config-backup-*.json` 恢复原 8-rule gui-only 配置。

---

## M5 — Homebrew formula 本地起草(完成)

只做本地 implementation + `brew install` 试装,**不** push 到 `jackerjay/homebrew-linkpilot` tap repo。完整 tap 发布动作在 M6。

| 子任务 | Commit | 关键交付 |
|---|---|---|
| M5.1 + M5.2 formula + cask | `17e77ac` | `packaging/homebrew/{Formula/linkpilot-cli.rb, Casks/linkpilot.rb, README.md}`。Formula 装 `lpt` + `linkpilot-daemon`(两个 universal binary),cask 装 `LinkPilot.app`(DMG),uninstall/zap 钩子完整 |
| M5.3 本地 brew install 验证 | `c969a1a` | 通过临时 tap `jackerjay/linkpilot-local` 验证 |
| M5.4 binary rename `lp` → `lpt` | `(this commit)` | 与 macOS 系统 `/usr/bin/lp`(CUPS)的 PATH 冲突规避;sweep 整个仓库的 binary name / help text / release.yml / brew formula / Tauri Settings / m4-verify / tests / docs |

### M5.3 验证结果

跑 `brew tap-new jackerjay/linkpilot-local --no-git` 建临时 tap,把 .rb cp 进去,然后逐项验证:

| 检查 | 结果 |
|---|---|
| `brew style packaging/homebrew/Formula/linkpilot-cli.rb` | ✅ 0 offenses(修了 component order + license SPDX 数组写法) |
| `brew style packaging/homebrew/Casks/linkpilot.rb` | ✅ 0 offenses(修了 zap 数组字母序、`depends_on macos: :monterey` 习惯写法) |
| `brew install jackerjay/linkpilot-local/linkpilot-cli` | ✅ 11.2MB 装入 `/opt/homebrew/Cellar/linkpilot-cli/0.2.0-alpha.3/`,`lpt` + `linkpilot-daemon` 在 `/opt/homebrew/bin/`(注意 macOS 内置 `/usr/bin/lpt` 会 shadow,brew 启动时已警告)|
| `brew test linkpilot-cli` | ✅ 两条 `--version` assertion 全过 |
| `brew audit --strict --new jackerjay/linkpilot-local/linkpilot-cli` | ✅ 0 problems |
| `brew audit --strict --new --cask jackerjay/linkpilot-local/linkpilot` | 🟡 4 warning,M6 才能消化 |

**cask 4 个 M5 阶段无法消化的 warning**(M6 final tag 上 tap 时再处理):
- `Signature verification failed`:unsigned build → 真发布需要 Apple Developer cert 签名 + notarize,或在 cask 加 `:disable_quarantine` workaround
- `v0.2.0-alpha.3 is a GitHub pre-release`:M6 用 v0.2.0 stable tag 后自动消失
- `Version differs from livecheck`:同上,prerelease detection 在 stable tag 上正常
- `GitHub repository not notable enough (<30 stars)`:tap 不强制,核心 Homebrew core 才要;tap repo 不受影响

清理:`brew uninstall linkpilot-cli && brew untap jackerjay/linkpilot-local` 全部还原。

## M6 — 公开发布(未开始)

把 M4/M5 本地验证过的成果一次性推到外部:

- **M6.1** v0.2.0 final tag + 跑通现有 `release.yml`(已被 M1 alpha 系列试过,见下方 release pipeline 记录)
- **M6.2** npm publish workflow `.github/workflows/npm-publish.yml` — tag 触发 + `bun run build` + `npm publish --access public`;**前置**:`NPM_TOKEN` GitHub secret + `@linkpilot` npm scope reserved
- **M6.3** Push `jackerjay/homebrew-linkpilot` repo(M5 起草的 formula / cask + v0.2.0 sha256)
- **M6.4** Release notes:把 design §v0.2.0 highlights / breaking / migration 整合成 GitHub Release 描述
- **M6.5** v0.1→v0.2 升级路径文档(`docs/upgrade-v0.2.md`)

---

## Release pipeline 实验记录

为验证 `release.yml` 在 M1 后能正常打包,跑了 3 次 pre-release tag:

| Tag | Workflow run | 状态 | 失败原因 / 修复 |
|---|---|---|---|
| `v0.2.0-alpha.1` | [26010296371](https://github.com/jackerjay/LinkPilot/actions/runs/26010296371) | ❌ failed | macOS BSD `tar` 不支持 `--transform` → 改用 `mktemp` staging dir(`bba7d96`) |
| `v0.2.0-alpha.2` | [26012248948](https://github.com/jackerjay/LinkPilot/actions/runs/26012248948) | ❌ failed | repo 启用了 immutable releases;`softprops/action-gh-release@v2` 发布时锁 assets → 改 `draft: true` + 后续 `gh release edit --draft=false` 二步发布(`e477b1c`) |
| `v0.2.0-alpha.3` | [26014703643](https://github.com/jackerjay/LinkPilot/actions/runs/26014703643) | ✅ **success** | universal CLI/daemon/DMG 全部 build 完成,GitHub Prerelease 创建 |

之后的 v0.2 release tag 可以放心用现有 `release.yml`,不再需要 fix。

---

## 测试统计

| Crate / Package | Tests | 备注 |
|---|---|---|
| `linkpilot-core` | 30 + 1 integration | 含 M2.1 PID(5)、M3.1 protocol(5)、M3.1 handler(2)、M4.2 dsl_roundtrip(1) |
| `linkpilot-ipc` | 0 unit + 3 integration | M3.2 unknown_verb(1)、M3.4 route_history_roundtrip(2) |
| `linkpilot-cli` | 0 unit + 7 integration | M2.4 daemon smoke(4)、M3.4 history smoke(3) |
| `linkpilot-platform-mac` | 6 | M2.3 plist + launchctl parsers |
| `@linkpilot/config` | 12(Bun) | M4.2 compile.test.ts |

**总计:Rust 47 passed,Bun 12 passed,0 failed。**

CI gates(`.github/workflows/ci.yml`):`cargo fmt --check` + `cargo clippy --workspace --exclude linkpilot-desktop --all-targets -- -D warnings` + `cargo test --workspace --exclude linkpilot-desktop` + `cargo check -p linkpilot-desktop` + `corepack yarn run build` 全部通过。

---

## 已知 follow-up / 未来 work

1. ~~**Daemon 启动 socket cleanup**:bind 前应 unlink 残留 socket 文件~~ ✅ 完成(2026-05-19)。`crates/ipc/src/server.rs` 一直就有 belt-and-suspenders `let _ = remove_file(...)` 在 bind 前;为了可观测性,在 `crates/headless-daemon/src/main.rs` 也加了显式 cleanup + 日志(`cleaned up stale socket file`),与 M2.1 PID cleanup 排版一致。新增 `crates/ipc/tests/stale_socket_cleanup.rs` 集成测试:预先写入"stale" regular file → 调 `serve()` → 验证 bind 后真的换成 socket 类型且能响应 Ping。
2. **GUI 在 M4.4 之前** 仍然能编辑 TsCompiled rule(只是源码标签会被覆盖),没有数据丢失风险但用户体验不闭环。M4.4 完成后才闭环。
3. **`lpt history --follow`**:design §14.2.6 明确不做(IPC server push 是单独议题,待 v0.3+)。
4. **GUI 试用版本**:本机 `/Applications/LinkPilot.app` 是 M2 之前 build 的 alpha.3,不含 M2/M3 daemon 端改动。要试用 M2/M3 完整行为需重 build .app(参考 M2 本地构建步骤)。
5. **M6 公开发布前置项**(M5 完成后再办,不阻塞 M4/M5):
   - `NPM_TOKEN` GitHub secret 由 maintainer 在 repo settings 配置
   - `@linkpilot` npm scope 若未注册需先 reserve(`npm login` + `npm access` 检查)
   - `jackerjay/homebrew-linkpilot` GitHub repo 需 maintainer 提前创建空壳

---

## 下一步候选

- **A. 顺序推进 M4.4** — 修 GUI rules.tsx,完成 M4 闭环
- **B. 先做 M4.5 本地 e2e** — 当前 M4.1-M4.3 已经能 DSL → daemon JSON 端到端跑通,M4.5 是把 design §14.3.6 验收表全部跑一遍并把结果写回 PROGRESS;M4.4 可以稍后再做
- ~~**C. 修 daemon socket cleanup**~~ ✅ 已完成,见 follow-up #1
- **D. 直接进 M5 本地 Homebrew 起草** — 跳过 M4.4(承担"GUI 显示 ts-compiled 标签但仍可编辑"短期不一致)
