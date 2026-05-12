// Structured editor for a single Rule. Replaces the JSON-textarea editor.
//
// Renders an inline form: priority + enabled + recursive MatcherTree builder +
// Action picker (with cascading browser/profile dropdowns) + optional note.
// Emits a validated Rule to onSave, or a list of human-readable errors via
// onError-less return: validation runs inline and disables Save when invalid.

import { useEffect, useState } from "react";
import { ipc } from "../lib/ipc";
import type {
  Action,
  BrowserProfile,
  BrowserTarget,
  InstalledBrowser,
  MatcherTree,
  Rule,
} from "../lib/types";

interface Props {
  initial: Rule | null; // null = create new
  browsers: InstalledBrowser[];
  onSave: (rule: Rule) => Promise<void>;
  onCancel: () => void;
}

const EMPTY_MATCHER: MatcherTree = { op: "url-host", pattern: "" };

function newRule(): Rule {
  return {
    id: crypto.randomUUID(),
    priority: 100,
    enabled: true,
    when: EMPTY_MATCHER,
    then: { kind: "open", target: { browser: "" } },
    source: "gui",
    note: null,
  };
}

export function RuleEditor({ initial, browsers, onSave, onCancel }: Props) {
  const [draft, setDraft] = useState<Rule>(initial ?? newRule());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const issues = validate(draft);
  const canSave = issues.length === 0 && !busy;

  const save = async () => {
    setBusy(true);
    setError(null);
    try {
      await onSave({ ...draft, source: draft.source ?? "gui" });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="card rule-editor">
      <h3>{initial ? "Edit rule" : "New rule"}</h3>

      <div className="row">
        <label className="grow">
          <div className="muted">Priority</div>
          <input
            type="number"
            value={draft.priority}
            onChange={(e) =>
              setDraft({ ...draft, priority: Number(e.target.value) || 0 })
            }
          />
        </label>
        <label className="grow">
          <div className="muted">Enabled</div>
          <input
            type="checkbox"
            checked={draft.enabled}
            onChange={(e) => setDraft({ ...draft, enabled: e.target.checked })}
          />
        </label>
      </div>

      <div className="rule-editor-section">
        <div className="muted">When</div>
        <MatcherEditor
          value={draft.when}
          browsers={browsers}
          onChange={(when) => setDraft({ ...draft, when })}
        />
      </div>

      <div className="rule-editor-section">
        <div className="muted">Then</div>
        <ActionEditor
          value={draft.then}
          browsers={browsers}
          onChange={(then) => setDraft({ ...draft, then })}
        />
      </div>

      <label>
        <div className="muted">Note (optional, shown in Inspector)</div>
        <input
          type="text"
          value={draft.note ?? ""}
          onChange={(e) =>
            setDraft({ ...draft, note: e.target.value || null })
          }
          placeholder="Why this rule exists…"
        />
      </label>

      {issues.length > 0 && (
        <div className="row">
          <span className="tag danger">invalid</span>
          <span className="muted grow">{issues.join(" · ")}</span>
        </div>
      )}
      {error && (
        <div className="row">
          <span className="tag danger">error</span>
          <span className="muted grow">{error}</span>
        </div>
      )}

      <div className="row">
        <span className="grow" />
        <button onClick={onCancel} disabled={busy}>
          Cancel
        </button>
        <button className="primary" onClick={save} disabled={!canSave}>
          {busy ? "Saving…" : "Save"}
        </button>
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Matcher subtree editor

const MATCHER_OPS: ReadonlyArray<MatcherTree["op"]> = [
  "url-host",
  "url-path",
  "source-app",
  "source-browser",
  "source-profile",
  "always",
  "all",
  "any",
  "not",
];

interface MatcherProps {
  value: MatcherTree;
  browsers: InstalledBrowser[];
  onChange: (next: MatcherTree) => void;
  depth?: number;
}

function MatcherEditor({ value, browsers, onChange, depth = 0 }: MatcherProps) {
  const setOp = (op: MatcherTree["op"]) => onChange(matcherFromOp(op));

  return (
    <div className="matcher" style={{ marginLeft: depth * 12 }}>
      <div className="row">
        <select
          value={value.op}
          onChange={(e) => setOp(e.target.value as MatcherTree["op"])}
          style={{ width: 160 }}
        >
          {MATCHER_OPS.map((op) => (
            <option key={op} value={op}>
              {op}
            </option>
          ))}
        </select>
        <MatcherLeafFields
          value={value}
          browsers={browsers}
          onChange={onChange}
        />
      </div>

      {value.op === "all" || value.op === "any" ? (
        <div className="matcher-children">
          {value.of.map((child, idx) => (
            <div key={idx} className="matcher-child">
              <MatcherEditor
                value={child}
                browsers={browsers}
                depth={depth + 1}
                onChange={(next) => {
                  const of = [...value.of];
                  of[idx] = next;
                  onChange({ ...value, of });
                }}
              />
              <button
                className="danger small"
                onClick={() =>
                  onChange({
                    ...value,
                    of: value.of.filter((_, i) => i !== idx),
                  })
                }
              >
                Remove
              </button>
            </div>
          ))}
          <button
            onClick={() => onChange({ ...value, of: [...value.of, EMPTY_MATCHER] })}
          >
            + Add child
          </button>
        </div>
      ) : null}

      {value.op === "not" ? (
        <div className="matcher-children">
          <MatcherEditor
            value={value.of}
            browsers={browsers}
            depth={depth + 1}
            onChange={(next) => onChange({ op: "not", of: next })}
          />
        </div>
      ) : null}
    </div>
  );
}

function MatcherLeafFields({
  value,
  browsers,
  onChange,
}: {
  value: MatcherTree;
  browsers: InstalledBrowser[];
  onChange: (next: MatcherTree) => void;
}) {
  switch (value.op) {
    case "always":
    case "all":
    case "any":
    case "not":
      return <span className="muted">{describeOp(value.op)}</span>;

    case "url-host":
      return (
        <input
          placeholder="github.com or *.corp.example.com"
          value={value.pattern}
          onChange={(e) => onChange({ ...value, pattern: e.target.value })}
        />
      );

    case "url-path":
      return (
        <input
          placeholder="/login or /oauth/callback"
          value={value.pattern}
          onChange={(e) => onChange({ ...value, pattern: e.target.value })}
        />
      );

    case "source-app":
      return (
        <input
          placeholder="Slack, VSCode, Terminal…"
          value={value.name}
          onChange={(e) => onChange({ ...value, name: e.target.value })}
        />
      );

    case "source-browser":
      return (
        <select
          value={value.browser}
          onChange={(e) => onChange({ ...value, browser: e.target.value })}
        >
          <option value="">— pick a browser —</option>
          {browsers.map((b) => (
            <option key={b.id} value={b.id}>
              {b.display_name} ({b.id})
            </option>
          ))}
        </select>
      );

    case "source-profile":
      return (
        <input
          placeholder="Work / Personal / Default"
          value={value.profile}
          onChange={(e) => onChange({ ...value, profile: e.target.value })}
        />
      );
  }
}

function matcherFromOp(op: MatcherTree["op"]): MatcherTree {
  switch (op) {
    case "always":
      return { op: "always" };
    case "all":
      return { op: "all", of: [EMPTY_MATCHER] };
    case "any":
      return { op: "any", of: [EMPTY_MATCHER] };
    case "not":
      return { op: "not", of: EMPTY_MATCHER };
    case "url-host":
      return { op: "url-host", pattern: "" };
    case "url-path":
      return { op: "url-path", pattern: "" };
    case "source-app":
      return { op: "source-app", name: "" };
    case "source-browser":
      return { op: "source-browser", browser: "" };
    case "source-profile":
      return { op: "source-profile", profile: "" };
  }
}

function describeOp(op: "always" | "all" | "any" | "not"): string {
  switch (op) {
    case "always":
      return "(always matches)";
    case "all":
      return "(every child must match)";
    case "any":
      return "(any child matches)";
    case "not":
      return "(child must NOT match)";
  }
}

// ---------------------------------------------------------------------------
// Action editor

interface ActionProps {
  value: Action;
  browsers: InstalledBrowser[];
  onChange: (next: Action) => void;
}

const ACTION_KINDS: ReadonlyArray<Action["kind"]> = [
  "open",
  "keep-source",
  "ask",
  "block",
];

function ActionEditor({ value, browsers, onChange }: ActionProps) {
  const setKind = (kind: Action["kind"]) => {
    switch (kind) {
      case "open":
        onChange({ kind: "open", target: { browser: "" } });
        break;
      case "keep-source":
        onChange({ kind: "keep-source" });
        break;
      case "ask":
        onChange({ kind: "ask" });
        break;
      case "block":
        onChange({ kind: "block" });
        break;
    }
  };

  return (
    <div className="matcher">
      <div className="row">
        <select
          value={value.kind}
          onChange={(e) => setKind(e.target.value as Action["kind"])}
          style={{ width: 160 }}
        >
          {ACTION_KINDS.map((k) => (
            <option key={k} value={k}>
              {k}
            </option>
          ))}
        </select>
        {value.kind === "open" ? (
          <TargetEditor
            value={value.target}
            browsers={browsers}
            onChange={(target) => onChange({ kind: "open", target })}
          />
        ) : (
          <span className="muted">{describeActionKind(value.kind)}</span>
        )}
      </div>
    </div>
  );
}

function describeActionKind(k: "keep-source" | "ask" | "block"): string {
  switch (k) {
    case "keep-source":
      return "(keep the navigation in the source browser — good for OAuth)";
    case "ask":
      return "(prompt the user to pick a target)";
    case "block":
      return "(do not open the URL)";
  }
}

function TargetEditor({
  value,
  browsers,
  onChange,
}: {
  value: BrowserTarget;
  browsers: InstalledBrowser[];
  onChange: (next: BrowserTarget) => void;
}) {
  const [profiles, setProfiles] = useState<BrowserProfile[]>([]);
  const [profilesError, setProfilesError] = useState<string | null>(null);

  useEffect(() => {
    if (!value.browser) {
      setProfiles([]);
      setProfilesError(null);
      return;
    }
    let alive = true;
    ipc
      .listProfiles(value.browser)
      .then((p) => {
        if (alive) {
          setProfiles(p);
          setProfilesError(null);
        }
      })
      .catch((e) => {
        if (alive) {
          setProfiles([]);
          setProfilesError(String(e));
        }
      });
    return () => {
      alive = false;
    };
  }, [value.browser]);

  return (
    <>
      <select
        value={value.browser}
        onChange={(e) =>
          onChange({ ...value, browser: e.target.value, profile: null })
        }
      >
        <option value="">— pick a browser —</option>
        {browsers.map((b) => (
          <option key={b.id} value={b.id}>
            {b.display_name}
          </option>
        ))}
      </select>
      <select
        value={value.profile ?? ""}
        onChange={(e) =>
          onChange({ ...value, profile: e.target.value || null })
        }
        disabled={!value.browser}
        title={profilesError ?? ""}
      >
        <option value="">(any profile)</option>
        {profiles.map((p) => (
          <option key={p.id} value={p.id}>
            {p.display_name}
          </option>
        ))}
      </select>
      <label className="muted" style={{ display: "flex", gap: 4, alignItems: "center" }}>
        <input
          type="checkbox"
          checked={value.incognito ?? false}
          onChange={(e) =>
            onChange({ ...value, incognito: e.target.checked || undefined })
          }
        />
        incognito
      </label>
    </>
  );
}

// ---------------------------------------------------------------------------
// Validation

function validate(rule: Rule): string[] {
  const issues: string[] = [];
  collectMatcherIssues(rule.when, issues);
  collectActionIssues(rule.then, issues);
  return issues;
}

function collectMatcherIssues(m: MatcherTree, into: string[]): void {
  switch (m.op) {
    case "always":
      return;
    case "all":
    case "any":
      if (m.of.length === 0) into.push(`${m.op} needs at least one child`);
      m.of.forEach((c) => collectMatcherIssues(c, into));
      return;
    case "not":
      collectMatcherIssues(m.of, into);
      return;
    case "url-host":
    case "url-path":
      if (m.pattern.trim() === "") into.push(`${m.op} pattern is empty`);
      return;
    case "source-app":
      if (m.name.trim() === "") into.push("source-app name is empty");
      return;
    case "source-browser":
      if (m.browser.trim() === "") into.push("source-browser is empty");
      return;
    case "source-profile":
      if (m.profile.trim() === "") into.push("source-profile is empty");
      return;
  }
}

function collectActionIssues(a: Action, into: string[]): void {
  if (a.kind === "open" && a.target.browser.trim() === "") {
    into.push("open action needs a target browser");
  }
}
