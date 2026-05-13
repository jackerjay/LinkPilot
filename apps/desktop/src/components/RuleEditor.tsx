// Structured editor for a single Rule. Replaces the JSON-textarea editor.

import { useState } from "react";
import { Plus, X } from "lucide-react";
import { AppIcon } from "@/components/AppIcon";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { TargetEditor } from "@/components/TargetEditor";
import { appPathFromExecutable } from "@/lib/browsers";
import type {
  Action,
  InstalledBrowser,
  MatcherTree,
  Rule,
} from "@/lib/types";

interface Props {
  initial: Rule | null;
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
    <Card className="border-primary/40 ring-1 ring-primary/20">
      <CardHeader>
        <CardTitle>{initial ? "Edit rule" : "New rule"}</CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="grid grid-cols-2 gap-4">
          <div className="space-y-1.5">
            <Label htmlFor="rule-priority">Priority</Label>
            <Input
              id="rule-priority"
              type="number"
              value={draft.priority}
              onChange={(e) =>
                setDraft({ ...draft, priority: Number(e.target.value) || 0 })
              }
            />
          </div>
          <div className="space-y-1.5">
            <Label>Enabled</Label>
            <div className="flex h-8 items-center">
              <Checkbox
                checked={draft.enabled}
                onCheckedChange={(v) =>
                  setDraft({ ...draft, enabled: v === true })
                }
              />
            </div>
          </div>
        </div>

        <div className="space-y-2">
          <Label>When</Label>
          <MatcherEditor
            value={draft.when}
            browsers={browsers}
            onChange={(when) => setDraft({ ...draft, when })}
          />
        </div>

        <div className="space-y-2">
          <Label>Then</Label>
          <ActionEditor
            value={draft.then}
            browsers={browsers}
            onChange={(then) => setDraft({ ...draft, then })}
          />
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="rule-note">Note (optional, shown in Inspector)</Label>
          <Input
            id="rule-note"
            value={draft.note ?? ""}
            onChange={(e) =>
              setDraft({ ...draft, note: e.target.value || null })
            }
            placeholder="Why this rule exists…"
          />
        </div>

        {issues.length > 0 && (
          <div className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/5 p-2.5">
            <Badge variant="destructive" className="shrink-0">invalid</Badge>
            <span className="text-xs text-muted-foreground">
              {issues.join(" · ")}
            </span>
          </div>
        )}
        {error && (
          <div className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/5 p-2.5">
            <Badge variant="destructive" className="shrink-0">error</Badge>
            <span className="text-xs text-muted-foreground">{error}</span>
          </div>
        )}

        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={onCancel} disabled={busy}>
            Cancel
          </Button>
          <Button onClick={save} disabled={!canSave}>
            {busy ? "Saving…" : "Save"}
          </Button>
        </div>
      </CardContent>
    </Card>
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
    <div className={depth > 0 ? "ml-3 border-l-2 border-border pl-3" : ""}>
      <div className="flex items-center gap-2">
        <Select value={value.op} onValueChange={(v) => setOp(v as MatcherTree["op"])}>
          <SelectTrigger className="w-[160px] shrink-0">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {MATCHER_OPS.map((op) => (
              <SelectItem key={op} value={op}>
                {op}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <MatcherLeafFields
          value={value}
          browsers={browsers}
          onChange={onChange}
        />
      </div>

      {(value.op === "all" || value.op === "any") && (
        <div className="mt-2 space-y-2">
          {value.of.map((child, idx) => (
            <div key={idx} className="flex items-start gap-2">
              <div className="flex-1">
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
              </div>
              <Button
                variant="ghost"
                size="icon"
                onClick={() =>
                  onChange({
                    ...value,
                    of: value.of.filter((_, i) => i !== idx),
                  })
                }
                className="text-muted-foreground hover:text-destructive"
                title="Remove"
              >
                <X />
              </Button>
            </div>
          ))}
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              onChange({ ...value, of: [...value.of, EMPTY_MATCHER] })
            }
          >
            <Plus />
            Add child
          </Button>
        </div>
      )}

      {value.op === "not" && (
        <div className="mt-2">
          <MatcherEditor
            value={value.of}
            browsers={browsers}
            depth={depth + 1}
            onChange={(next) => onChange({ op: "not", of: next })}
          />
        </div>
      )}
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
      return (
        <span className="text-xs text-muted-foreground">
          {describeOp(value.op)}
        </span>
      );

    case "url-host":
      return (
        <Input
          placeholder="github.com or *.corp.example.com"
          value={value.pattern}
          onChange={(e) => onChange({ ...value, pattern: e.target.value })}
        />
      );

    case "url-path":
      return (
        <Input
          placeholder="/login or /oauth/callback"
          value={value.pattern}
          onChange={(e) => onChange({ ...value, pattern: e.target.value })}
        />
      );

    case "source-app":
      return (
        <Input
          placeholder="Slack, VSCode, Terminal…"
          value={value.name}
          onChange={(e) => onChange({ ...value, name: e.target.value })}
        />
      );

    case "source-browser":
      return (
        <Select
          value={value.browser || undefined}
          onValueChange={(v) => onChange({ ...value, browser: v })}
        >
          <SelectTrigger>
            <SelectValue placeholder="— pick a browser —" />
          </SelectTrigger>
          <SelectContent>
            {browsers.map((b) => (
              <SelectItem key={b.id} value={b.id}>
                <span className="flex items-center gap-2">
                  <AppIcon
                    bundleId={b.platform_app_id ?? undefined}
                    appPath={appPathFromExecutable(b.executable)}
                    size={16}
                    alt={b.display_name}
                  />
                  {b.display_name}
                </span>
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      );

    case "source-profile":
      return (
        <Input
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
    <div className="flex items-center gap-2">
      <Select
        value={value.kind}
        onValueChange={(v) => setKind(v as Action["kind"])}
      >
        <SelectTrigger className="w-[160px] shrink-0">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {ACTION_KINDS.map((k) => (
            <SelectItem key={k} value={k}>
              {k}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      {value.kind === "open" ? (
        <TargetEditor
          value={value.target}
          browsers={browsers}
          onChange={(target) => onChange({ kind: "open", target })}
        />
      ) : (
        <span className="text-xs text-muted-foreground">
          {describeActionKind(value.kind)}
        </span>
      )}
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
