import type { BrowserTargetJson } from "./types.js";

/**
 * Build a `BrowserTarget` step by step. `Target` instances are immutable —
 * each mutator returns a fresh copy so chaining can't accidentally share
 * state across rules.
 *
 * The class is intentionally small and serialises directly via `toJSON`
 * so `JSON.stringify(target)` produces the wire shape the daemon expects.
 */
export class Target {
  constructor(
    public readonly browser: string,
    public readonly profile: string | null = null,
    public readonly workspace: string | null = null,
    public readonly incognito = false,
    public readonly newWindow = false,
  ) {}

  /** Pin a profile by its native id (e.g. Chrome's `"Profile 1"`). */
  withProfile(name: string): Target {
    return new Target(this.browser, name, this.workspace, this.incognito, this.newWindow);
  }

  /** Tag the target with a workspace id (must exist in `workspaces`). */
  withWorkspace(id: string): Target {
    return new Target(this.browser, this.profile, id, this.incognito, this.newWindow);
  }

  /** Open in incognito / private mode. */
  asIncognito(): Target {
    return new Target(this.browser, this.profile, this.workspace, true, this.newWindow);
  }

  /** Force a new window instead of a new tab. */
  asNewWindow(): Target {
    return new Target(this.browser, this.profile, this.workspace, this.incognito, true);
  }

  toJSON(): BrowserTargetJson {
    return {
      browser: this.browser,
      profile: this.profile,
      workspace: this.workspace,
      incognito: this.incognito,
      new_window: this.newWindow,
    };
  }
}

/**
 * Callable browser handle. `browser.chrome` is a function that returns a
 * fresh `Target`, AND has the same chainable methods attached so
 * `browser.chrome.profile("Work")` works without an extra `()`.
 *
 * Mirrors the design §4.2.2 examples — both forms must compile:
 *   browser.chrome()                  -> Target
 *   browser.chrome.profile("Work")    -> Target  (no `()` after chrome)
 *   browser.chrome().profile("Work")  -> Target  (with `()`)
 */
export interface BrowserHandle {
  (): Target;
  profile(name: string): Target;
  incognito(): Target;
  newWindow(): Target;
  workspace(id: string): Target;
}

function makeBrowser(id: string): BrowserHandle {
  const fn = function (): Target {
    return new Target(id);
  } as BrowserHandle;
  fn.profile = (name: string) => new Target(id).withProfile(name);
  fn.incognito = () => new Target(id).asIncognito();
  fn.newWindow = () => new Target(id).asNewWindow();
  fn.workspace = (wid: string) => new Target(id).withWorkspace(wid);
  return fn;
}

/**
 * Built-in browser shortcuts. Users with a custom browser id (e.g. one
 * they added through the GUI's "Custom browser" form) can fall back to
 * `browser.custom("their-id")`.
 */
export const browser = {
  chrome: makeBrowser("chrome"),
  arc: makeBrowser("arc"),
  firefox: makeBrowser("firefox"),
  safari: makeBrowser("safari"),
  edge: makeBrowser("edge"),
  brave: makeBrowser("brave"),
  vivaldi: makeBrowser("vivaldi"),
  /** Escape hatch for browser ids the DSL doesn't have a shortcut for. */
  custom: (id: string): BrowserHandle => makeBrowser(id),
} as const;
