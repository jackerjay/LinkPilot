// Structured editor for a single Rule. Replaces the JSON-textarea editor.

import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { Plus, X } from "lucide-react";
import { AppIcon } from "@/components/AppIcon";
import { AppPickerButton } from "@/components/AppPickerButton";
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
  Workspace,
} from "@/lib/types";

interface Props {
  initial: Rule | null;
  browsers: InstalledBrowser[];
  workspaces: Workspace[];
  onSave: (rule: Rule) => Promise<void>;
  onCancel: () => void;
}

const EMPTY_MATCHER: MatcherTree = { op: "url-host", pattern: "" };

function newRule(): Rule {
  return {
    id: crypto.randomUUID(),
    enabled: true,
    when: EMPTY_MATCHER,
    then: { kind: "open", target: { browser: "" } },
    source: "gui",
    note: null,
    workspace_id: null,
  };
}

// `Select` from Radix can't represent an empty-string value, so the
// "no workspace" choice rides on this sentinel and gets mapped back to
// `null` before save.
const NO_WORKSPACE = "__none__";

export function RuleEditor({ initial, browsers, workspaces, onSave, onCancel }: Props) {
  const { t } = useTranslation("rules");
  const [draft, setDraft] = useState<Rule>(initial ?? newRule());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const issues = validate(t, draft);
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
        <CardTitle>
          {initial ? t("editor.editTitle") : t("editor.newTitle")}
        </CardTitle>
      </CardHeader>
      <CardContent className="space-y-4">
        <div className="flex items-center justify-between rounded-md border border-border bg-muted/30 px-3 py-2">
          <div className="space-y-0.5">
            <Label className="text-sm">{t("editor.enabled")}</Label>
            <p className="text-xs text-muted-foreground">
              {t("editor.priorityHint")}
            </p>
          </div>
          <Checkbox
            checked={draft.enabled}
            onCheckedChange={(v) =>
              setDraft({ ...draft, enabled: v === true })
            }
          />
        </div>

        <div className="space-y-2">
          <Label>{t("editor.when")}</Label>
          <MatcherEditor
            value={draft.when}
            browsers={browsers}
            onChange={(when) => setDraft({ ...draft, when })}
          />
        </div>

        <div className="space-y-2">
          <Label>{t("editor.then")}</Label>
          <ActionEditor
            value={draft.then}
            browsers={browsers}
            onChange={(then) => setDraft({ ...draft, then })}
          />
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="rule-workspace">{t("editor.workspace")}</Label>
          <Select
            value={draft.workspace_id ?? NO_WORKSPACE}
            onValueChange={(v) =>
              setDraft({
                ...draft,
                workspace_id: v === NO_WORKSPACE ? null : v,
              })
            }
          >
            <SelectTrigger id="rule-workspace">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NO_WORKSPACE}>
                <span className="text-muted-foreground">
                  {t("editor.ungrouped")}
                </span>
              </SelectItem>
              {workspaces.map((w) => (
                <SelectItem key={w.id} value={w.id}>
                  {w.display_name}
                  {!w.enabled && (
                    <span className="ml-2 text-muted-foreground">
                      {t("editor.off")}
                    </span>
                  )}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <div className="space-y-1.5">
          <Label htmlFor="rule-note">{t("editor.note")}</Label>
          <Input
            id="rule-note"
            value={draft.note ?? ""}
            onChange={(e) =>
              setDraft({ ...draft, note: e.target.value || null })
            }
            placeholder={t("editor.notePlaceholder")}
          />
        </div>

        {issues.length > 0 && (
          <div className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/5 p-2.5">
            <Badge variant="destructive" className="shrink-0">
              {t("editor.invalidTag")}
            </Badge>
            <span className="text-xs text-muted-foreground">
              {issues.join(" · ")}
            </span>
          </div>
        )}
        {error && (
          <div className="flex items-start gap-2 rounded-md border border-destructive/30 bg-destructive/5 p-2.5">
            <Badge variant="destructive" className="shrink-0">
              {t("editor.errorTag")}
            </Badge>
            <span className="text-xs text-muted-foreground">{error}</span>
          </div>
        )}

        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={onCancel} disabled={busy}>
            {t("editor.cancel")}
          </Button>
          <Button onClick={save} disabled={!canSave}>
            {busy ? t("editor.saving") : t("editor.save")}
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
  const { t } = useTranslation("rules");
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
                title={t("editor.removeChildTitle")}
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
            {t("editor.addChild")}
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
  const { t } = useTranslation("rules");
  switch (value.op) {
    case "always":
    case "all":
    case "any":
    case "not":
      return (
        <span className="text-xs text-muted-foreground">
          {describeOp(t, value.op)}
        </span>
      );

    case "url-host":
      return (
        <Input
          placeholder={t("matcherPlaceholders.host")}
          value={value.pattern}
          onChange={(e) => onChange({ ...value, pattern: e.target.value })}
        />
      );

    case "url-path":
      return (
        <Input
          placeholder={t("matcherPlaceholders.path")}
          value={value.pattern}
          onChange={(e) => onChange({ ...value, pattern: e.target.value })}
        />
      );

    case "source-app":
      return (
        <>
          <Input
            placeholder={t("matcherPlaceholders.sourceApp")}
            value={value.name}
            onChange={(e) =>
              // Hand-typed input clears the stored bundle id — the user
              // is now overriding what the picker captured, and matching
              // should fall back to name-only. The next picker click can
              // re-populate it.
              onChange({ ...value, name: e.target.value, bundle_id: null })
            }
          />
          <AppPickerButton
            onPicked={(p) =>
              onChange({
                ...value,
                name: p.name,
                bundle_id: p.bundleId || null,
              })
            }
          />
        </>
      );

    case "source-browser":
      return (
        <Select
          value={value.browser || undefined}
          onValueChange={(v) => onChange({ ...value, browser: v })}
        >
          <SelectTrigger>
            <SelectValue placeholder={t("target.pickBrowser")} />
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
          placeholder={t("matcherPlaceholders.sourceProfile")}
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
      return { op: "source-app", name: "", bundle_id: null };
    case "source-browser":
      return { op: "source-browser", browser: "" };
    case "source-profile":
      return { op: "source-profile", profile: "" };
  }
}

function describeOp(
  t: TFunction<"rules">,
  op: "always" | "all" | "any" | "not",
): string {
  switch (op) {
    case "always":
      return t("matcherHelp.always");
    case "all":
      return t("matcherHelp.all");
    case "any":
      return t("matcherHelp.any");
    case "not":
      return t("matcherHelp.not");
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
  const { t } = useTranslation("rules");
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
          {describeActionKind(t, value.kind)}
        </span>
      )}
    </div>
  );
}

function describeActionKind(
  t: TFunction<"rules">,
  k: "keep-source" | "ask" | "block",
): string {
  switch (k) {
    case "keep-source":
      return t("actionHelp.keepSource");
    case "ask":
      return t("actionHelp.ask");
    case "block":
      return t("actionHelp.block");
  }
}

// ---------------------------------------------------------------------------
// Validation

function validate(t: TFunction<"rules">, rule: Rule): string[] {
  const issues: string[] = [];
  collectMatcherIssues(t, rule.when, issues);
  collectActionIssues(t, rule.then, issues);
  return issues;
}

function collectMatcherIssues(
  t: TFunction<"rules">,
  m: MatcherTree,
  into: string[],
): void {
  switch (m.op) {
    case "always":
      return;
    case "all":
    case "any":
      if (m.of.length === 0) {
        into.push(t("validation.needsChild", { op: m.op }));
      }
      m.of.forEach((c) => collectMatcherIssues(t, c, into));
      return;
    case "not":
      collectMatcherIssues(t, m.of, into);
      return;
    case "url-host":
    case "url-path":
      if (m.pattern.trim() === "") {
        into.push(t("validation.patternEmpty", { op: m.op }));
      }
      return;
    case "source-app":
      if (m.name.trim() === "") into.push(t("validation.sourceAppEmpty"));
      return;
    case "source-browser":
      if (m.browser.trim() === "") into.push(t("validation.sourceBrowserEmpty"));
      return;
    case "source-profile":
      if (m.profile.trim() === "") into.push(t("validation.sourceProfileEmpty"));
      return;
  }
}

function collectActionIssues(
  t: TFunction<"rules">,
  a: Action,
  into: string[],
): void {
  if (a.kind === "open" && a.target.browser.trim() === "") {
    into.push(t("validation.openTargetMissing"));
  }
}
