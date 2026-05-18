import { Target } from "./targets.js";
import type { ActionJson, MatcherTreeJson } from "./types.js";

/**
 * Pending rule: a matcher tree waiting to be paired with an action. The
 * `to()` / `keepSource()` / `ask()` / `block()` terminal methods turn it
 * into a complete `Rule` that `defineConfig` collects.
 */
export class RouteBuilder {
  private _priority = 10;
  private _enabled = true;
  private _note: string | null = null;
  private _workspaceId: string | null = null;

  constructor(private readonly matcher: MatcherTreeJson) {}

  /** Higher priority wins; ties broken by list order. Default 10. */
  priority(p: number): this {
    this._priority = p;
    return this;
  }

  /** Start the rule disabled. */
  disabled(): this {
    this._enabled = false;
    return this;
  }

  /** Free-form note shown next to the rule in the GUI list. */
  note(text: string): this {
    this._note = text;
    return this;
  }

  /** Tag the rule with a workspace id (toggling the workspace mutes the rule). */
  workspace(id: string): this {
    this._workspaceId = id;
    return this;
  }

  /** Terminal: open the URL in the given target. */
  to(target: Target): PendingRule {
    return new PendingRule(
      this.matcher,
      { kind: "open", target: target.toJSON() },
      this._priority,
      this._enabled,
      this._note,
      this._workspaceId,
    );
  }

  /** Terminal: keep the URL in the source browser (no handoff). */
  keepSource(): PendingRule {
    return this.finalize({ kind: "keep-source" });
  }

  /** Terminal: pop the browser picker. */
  ask(): PendingRule {
    return this.finalize({ kind: "ask" });
  }

  /** Terminal: drop the URL silently. */
  block(): PendingRule {
    return this.finalize({ kind: "block" });
  }

  /** Internal: read-only matcher view for combinator constructors. */
  toMatcher(): MatcherTreeJson {
    return this.matcher;
  }

  private finalize(action: ActionJson): PendingRule {
    return new PendingRule(
      this.matcher,
      action,
      this._priority,
      this._enabled,
      this._note,
      this._workspaceId,
    );
  }
}

/**
 * A rule that has both a matcher and an action. `defineConfig` collects
 * these into the final config. The `Rule` type is intentionally not
 * exported as a wire-shaped struct here — the `compile()` step assigns
 * UUIDs and stamps `source: "ts-compiled"`, which the user shouldn't
 * have to think about.
 */
export class PendingRule {
  constructor(
    public readonly when: MatcherTreeJson,
    public readonly then: ActionJson,
    public readonly priority: number,
    public readonly enabled: boolean,
    public readonly note: string | null,
    public readonly workspaceId: string | null,
  ) {}
}

/** Sentinel: matcher that always wins (used as a wildcard "catch-all" rule). */
function always(): RouteBuilder {
  return new RouteBuilder({ op: "always" });
}

/**
 * `route.*` factory namespace. Each entry produces a `RouteBuilder` you
 * chain action terminals onto.
 */
export const route = {
  /** Glob host match: `github.com`, `*.figma.com`. */
  host: (pattern: string): RouteBuilder => new RouteBuilder({ op: "url-host", pattern }),
  /** Glob path match: `/oauth/*`. */
  path: (pattern: string): RouteBuilder => new RouteBuilder({ op: "url-path", pattern }),
  /** Match by source app display name (case-insensitive) + optional bundle id. */
  fromApp: (name: string, bundleId?: string): RouteBuilder =>
    new RouteBuilder({
      op: "source-app",
      name,
      bundle_id: bundleId ?? null,
    }),
  /** Match by source-browser id (when navigation came from a browser extension). */
  fromBrowser: (id: string): RouteBuilder =>
    new RouteBuilder({ op: "source-browser", browser: id }),
  /** Match by source-profile id within a source browser. */
  fromProfile: (profileId: string): RouteBuilder =>
    new RouteBuilder({ op: "source-profile", profile: profileId }),

  /** Logical AND of every child matcher. */
  all: (...subs: RouteBuilder[]): RouteBuilder =>
    new RouteBuilder({ op: "all", of: subs.map((s) => s.toMatcher()) }),
  /** Logical OR. */
  any: (...subs: RouteBuilder[]): RouteBuilder =>
    new RouteBuilder({ op: "any", of: subs.map((s) => s.toMatcher()) }),
  /** Logical NOT. */
  not: (sub: RouteBuilder): RouteBuilder =>
    new RouteBuilder({ op: "not", of: sub.toMatcher() }),

  /** Match anything — useful as the last "catch-all" entry. */
  always,

  /**
   * Escape hatch for matcher shapes the DSL doesn't model directly.
   * The object is passed straight to the daemon, so the caller is
   * responsible for matching the on-disk schema.
   */
  fromJson: (raw: MatcherTreeJson): RouteBuilder => new RouteBuilder(raw),
};
