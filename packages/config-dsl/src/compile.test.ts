// Unit tests for the DSL compiler. Run with `bun test`.
//
// What we lock here:
//   1. The wire shape (snake_case fields, kebab-case discriminants)
//      matches what the Rust serde layer in crates/core expects.
//   2. Builders produce structurally-equivalent rules to the v0.1 demo
//      config that ships with the daemon — guaranteeing a DSL-authored
//      LinkPilot is feature-parity with a GUI-authored one.
//   3. Every MatcherTree variant and Action variant round-trips.
//
// What we do NOT test here:
//   - The Rust side parsing the JSON. That's covered by the Rust
//     integration test in crates/cli/tests/dsl_roundtrip.rs (added in
//     the same M4.2 commit).

import { describe, expect, test } from "bun:test";

import { compile } from "./compile.js";
import { defineConfig } from "./index.js";
import { route } from "./matchers.js";
import { browser } from "./targets.js";

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

describe("compile()", () => {
  test("wire shape: top-level keys are snake_case", () => {
    const out = compile(
      defineConfig({
        defaultTarget: browser.arc(),
        rules: [],
      }),
    );
    expect(Object.keys(out).sort()).toEqual(
      ["custom_browsers", "default_target", "meta", "rules", "settings", "version", "workspaces"].sort(),
    );
  });

  test("schema_version is 1", () => {
    const out = compile(defineConfig({ defaultTarget: browser.arc(), rules: [] }));
    expect(out.version).toBe(1);
  });

  test("default_target serialises to snake_case", () => {
    const out = compile(
      defineConfig({
        defaultTarget: browser.chrome.profile("Work").asNewWindow(),
        rules: [],
      }),
    );
    expect(out.default_target).toEqual({
      browser: "chrome",
      profile: "Work",
      workspace: null,
      incognito: false,
      new_window: true,
    });
  });

  test("expanded browser shortcuts serialise to stable ids", () => {
    for (const [id, target] of [
      ["vivaldi", browser.vivaldi()],
      ["opera", browser.opera()],
      ["opera-gx", browser.operaGx()],
      ["dia", browser.dia()],
      ["atlas", browser.atlas()],
      ["comet", browser.comet()],
      ["zen", browser.zen()],
      ["orion", browser.orion()],
      ["duckduckgo", browser.duckduckgo()],
      ["librewolf", browser.librewolf()],
      ["waterfox", browser.waterfox()],
      ["floorp", browser.floorp()],
      ["mullvad-browser", browser.mullvad()],
      ["tor-browser", browser.tor()],
      ["yandex", browser.yandex()],
      ["whale", browser.whale()],
    ] as const) {
      expect(target.toJSON().browser).toBe(id);
    }
  });

  test("rules stamp source: ts-compiled + random v4 UUIDs", () => {
    const out = compile(
      defineConfig({
        defaultTarget: browser.arc(),
        rules: [
          route.host("a.com").to(browser.chrome()),
          route.host("b.com").to(browser.chrome()),
        ],
      }),
    );
    expect(out.rules).toHaveLength(2);
    for (const r of out.rules) {
      expect(r.source).toBe("ts-compiled");
      expect(r.id).toMatch(UUID_RE);
    }
    // Two compiles must not collide.
    expect(out.rules[0]?.id).not.toBe(out.rules[1]?.id);
  });

  test("MatcherTree round-trip: every variant", () => {
    const out = compile(
      defineConfig({
        defaultTarget: browser.arc(),
        rules: [
          route.host("a.com").to(browser.chrome()),
          route.path("/oauth/*").keepSource(),
          route.fromApp("Slack", "com.tinyspeck.slack").ask(),
          route.fromBrowser("chrome").block(),
          route.fromProfile("Profile 1").to(browser.arc()),
          route.all(route.host("a.com"), route.path("/x")).to(browser.chrome()),
          route.any(route.host("a.com"), route.host("b.com")).block(),
          route.not(route.fromApp("Slack")).to(browser.chrome()),
          route.always().to(browser.firefox()),
        ],
      }),
    );

    const ops = out.rules.map((r) => r.when.op);
    expect(ops).toEqual([
      "url-host",
      "url-path",
      "source-app",
      "source-browser",
      "source-profile",
      "all",
      "any",
      "not",
      "always",
    ]);

    // Compound matchers carry nested children with the same kebab-case
    // op discriminants.
    const all = out.rules[5]!.when;
    if (all.op !== "all") throw new Error("expected all matcher");
    expect(all.of.map((m) => m.op)).toEqual(["url-host", "url-path"]);

    const not = out.rules[7]!.when;
    if (not.op !== "not") throw new Error("expected not matcher");
    expect(not.of.op).toBe("source-app");
  });

  test("Action round-trip: every variant", () => {
    const out = compile(
      defineConfig({
        defaultTarget: browser.arc(),
        rules: [
          route.host("a.com").to(browser.chrome.profile("Work").asIncognito()),
          route.path("/x").keepSource(),
          route.host("b.com").ask(),
          route.host("c.com").block(),
        ],
      }),
    );
    expect(out.rules.map((r) => r.then.kind)).toEqual([
      "open",
      "keep-source",
      "ask",
      "block",
    ]);
    // Open action carries the target with snake_case `new_window`.
    const open = out.rules[0]!.then;
    if (open.kind !== "open") throw new Error("expected open action");
    expect(open.target).toEqual({
      browser: "chrome",
      profile: "Work",
      workspace: null,
      incognito: true,
      new_window: false,
    });
  });

  test("workspaces compile to snake_case + enabled defaults true", () => {
    const out = compile(
      defineConfig({
        defaultTarget: browser.arc(),
        rules: [],
        workspaces: [
          { id: "work", displayName: "Work" },
          { id: "play", displayName: "Play", description: "after hours", enabled: false },
        ],
      }),
    );
    expect(out.workspaces).toEqual([
      { id: "work", display_name: "Work", description: null, enabled: true },
      { id: "play", display_name: "Play", description: "after hours", enabled: false },
    ]);
  });

  test("settings: omitted fields default to daemon expectations", () => {
    const out = compile(defineConfig({ defaultTarget: browser.arc(), rules: [] }));
    expect(out.settings).toEqual({
      launch_at_login: false,
      history_retention_days: null,
      record_query_strings: false,
      // The daemon defaults this true; the DSL preserves that.
      smart_routing_enabled: true,
    });
  });

  test("settings: every camelCase key maps to snake_case", () => {
    const out = compile(
      defineConfig({
        defaultTarget: browser.arc(),
        rules: [],
        settings: {
          launchAtLogin: true,
          historyRetentionDays: 30,
          recordQueryStrings: true,
          smartRoutingEnabled: false,
        },
      }),
    );
    expect(out.settings).toEqual({
      launch_at_login: true,
      history_retention_days: 30,
      record_query_strings: true,
      smart_routing_enabled: false,
    });
  });

  test("RouteBuilder modifiers stick", () => {
    const out = compile(
      defineConfig({
        defaultTarget: browser.arc(),
        rules: [
          route.host("a.com").disabled().note("hot").workspace("work").to(browser.chrome()),
        ],
      }),
    );
    expect(out.rules[0]).toMatchObject({
      enabled: false,
      note: "hot",
      workspace_id: "work",
    });
    // List order IS priority — no numeric priority field on the wire.
    expect(out.rules[0]).not.toHaveProperty("priority");
  });

  test("rule array order is preserved (list-order priority)", () => {
    const out = compile(
      defineConfig({
        defaultTarget: browser.arc(),
        rules: [
          route.host("first.example.com").to(browser.chrome()),
          route.host("second.example.com").to(browser.chrome()),
          route.host("third.example.com").to(browser.chrome()),
        ],
      }),
    );
    expect(out.rules.map((r) => (r.when as { pattern: string }).pattern)).toEqual([
      "first.example.com",
      "second.example.com",
      "third.example.com",
    ]);
  });

  test("route.fromJson escape hatch passes raw matcher", () => {
    const out = compile(
      defineConfig({
        defaultTarget: browser.arc(),
        rules: [
          route.fromJson({ op: "url-host", pattern: "example.com" }).block(),
        ],
      }),
    );
    expect(out.rules[0]!.when).toEqual({ op: "url-host", pattern: "example.com" });
  });

  test("v0.1 demo config: structurally equivalent rule set", () => {
    // The demo rule set routes github/notion -> chrome.Default and
    // figma/youtube -> arc. The DSL version below must produce the same
    // shape (modulo random UUIDs and source).
    const out = compile(
      defineConfig({
        defaultTarget: browser.arc(),
        rules: [
          route.host("github.com").to(browser.chrome.profile("Default")),
          route.host("notion.so").to(browser.chrome.profile("Default")),
          route.host("figma.com").to(browser.arc()),
          route.host("youtube.com").to(browser.arc()),
        ],
      }),
    );
    expect(out.rules).toHaveLength(4);
    expect(out.rules.map((r) => (r.when.op === "url-host" ? r.when.pattern : ""))).toEqual([
      "github.com",
      "notion.so",
      "figma.com",
      "youtube.com",
    ]);
    expect(out.rules.map((r) => (r.then.kind === "open" ? r.then.target.browser : ""))).toEqual([
      "chrome",
      "chrome",
      "arc",
      "arc",
    ]);
  });
});
