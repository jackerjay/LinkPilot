// Reproduces the v0.1 PRD §22 demo config in DSL form. Used as a manual
// smoke test: `bun run examples/v0.1-demo.ts` prints the wire JSON that
// `lp config compile` would feed into the daemon.

import { browser, defineConfig, printConfig, route } from "../src/index.js";

const config = defineConfig({
  defaultTarget: browser.arc(),
  rules: [
    route.host("github.com").to(browser.chrome.profile("Default")),
    route.host("notion.so").to(browser.chrome.profile("Default")),
    route.host("figma.com").to(browser.arc()),
    route.host("youtube.com").to(browser.arc()),
    route.path("/oauth/*").keepSource(),
    route.fromApp("Slack").to(browser.chrome.profile("Work")),
  ],
  workspaces: [{ id: "work", displayName: "Work" }],
  settings: {
    smartRoutingEnabled: true,
  },
});

printConfig(config);
