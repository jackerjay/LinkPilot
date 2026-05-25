# `@linkpilot/config`

TypeScript DSL for authoring `linkpilot.config.ts`. Compiles to the same
JSON schema the LinkPilot GUI edits.

Lands in **v0.2**. CLI driver is `lp config compile`.

## Quick start

```sh
npm i -D @linkpilot/config
# or: bun add -d @linkpilot/config
```

```ts
// linkpilot.config.ts
import { browser, defineConfig, printConfig, route } from "@linkpilot/config";

const config = defineConfig({
  defaultTarget: browser.arc(),
  rules: [
    route.host("github.com").to(browser.chrome.profile("Work")),
    route.host("notion.so").to(browser.chrome.profile("Work")),
    route.host("figma.com").to(browser.arc()),
    route.host("youtube.com").to(browser.arc.profile("Personal")),
    route.path("/oauth/*").keepSource(),
    route.fromApp("Slack").to(browser.chrome.profile("Work")),
  ],
  workspaces: [{ id: "work", displayName: "Work" }],
  settings: { smartRoutingEnabled: true },
});

printConfig(config);
```

Then:

```sh
lp config compile linkpilot.config.ts
```

The CLI runs your file through [Bun][bun] (~30ms cold start, native
TypeScript), validates the JSON it emits, and atomically writes it into
the daemon's config path. The daemon's `fsnotify` watcher picks it up
and reloads within a frame — any running GUI / CLI sees the new rules
immediately.

`lp config compile --to out.json` writes to a path of your choosing
instead (useful for `git`-tracked compiled outputs).

[bun]: https://bun.sh

## API

### `defineConfig(cfg)`

Type-checks your config at edit time. Pass-through at runtime.

### `browser.*`

```ts
browser.chrome             // BrowserHandle: callable + chainable
browser.chrome()           // Target { browser: "chrome", ... }
browser.chrome.profile("Work")
browser.chrome.incognito()
browser.chrome.newWindow()
browser.chrome.workspace("work")
```

Also: `browser.arc`, `browser.firefox`, `browser.safari`, `browser.edge`,
`browser.brave`, `browser.vivaldi`, `browser.opera`, `browser.operaGx`,
`browser.dia`, `browser.atlas`, `browser.comet`, `browser.zen`,
`browser.orion`, `browser.duckduckgo`, `browser.librewolf`,
`browser.waterfox`, `browser.floorp`, `browser.mullvad`, `browser.tor`,
`browser.yandex`, `browser.whale`, and `browser.custom("your-id")` for
anything else.

### `route.*`

Matchers:

```ts
route.host("github.com")           // glob host match
route.host("*.figma.com")
route.path("/oauth/*")             // glob path match
route.fromApp("Slack", "com.tinyspeck.slackmacgap")
route.fromBrowser("chrome")
route.fromProfile("Profile 1")
route.always()                     // catch-all
```

Combinators:

```ts
route.all(route.host("github.com"), route.path("/oauth/*"))
route.any(route.host("a.com"), route.host("b.com"))
route.not(route.fromApp("Slack"))
```

Escape hatch:

```ts
route.fromJson({ op: "url-host", pattern: "github.com" })
```

Modifiers (chain before the terminal action):

```ts
route.host("foo.com").priority(20).note("hot path").workspace("work").to(...)
route.host("legacy.com").disabled().to(...)
```

Terminals (turn a `RouteBuilder` into a `PendingRule`):

```ts
.to(browser.chrome())   // open
.keepSource()           // stay in source browser
.ask()                  // pop the picker
.block()                // drop silently
```

## License

MIT.
