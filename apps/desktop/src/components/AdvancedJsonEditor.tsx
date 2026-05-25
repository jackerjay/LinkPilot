// Raw-JSON editor for `Rules → Advanced`. Pulled into its own module so
// CodeMirror + the JSON language pack land in a lazy chunk — they're
// ~140KB gzipped and irrelevant to users who never expand this panel.

import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { AlertCircle, CheckCircle2 } from "lucide-react";
import CodeMirror from "@uiw/react-codemirror";
import { json, jsonParseLinter } from "@codemirror/lang-json";
import { linter, lintGutter } from "@codemirror/lint";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ipc } from "@/lib/ipc";
import { useTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";
import type { ConfigDocument } from "@/lib/types";

type SyntaxStatus =
  | { kind: "valid" }
  | { kind: "error"; msg: string; line: number | null; col: number | null };

// Best-effort JSON syntax check. V8 / WebKit attach "position N" to
// SyntaxError messages — we walk the draft up to that index to translate
// it into a 1-indexed line/col the user can find by eye. Browsers that
// omit the position still get a usable error message.
function checkJsonSyntax(draft: string): SyntaxStatus {
  try {
    JSON.parse(draft);
    return { kind: "valid" };
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    const posMatch = msg.match(/position (\d+)/);
    if (posMatch) {
      const pos = parseInt(posMatch[1], 10);
      const upto = draft.slice(0, pos);
      const lastNl = upto.lastIndexOf("\n");
      const line = upto.split("\n").length;
      const col = pos - (lastNl === -1 ? -1 : lastNl);
      return { kind: "error", msg, line, col };
    }
    return { kind: "error", msg, line: null, col: null };
  }
}

// Stable references — defining extensions at module scope means React's
// reconciler doesn't tear down the editor view on every parent re-render
// (which would lose cursor / selection / undo).
const EXTENSIONS = [json(), linter(jsonParseLinter()), lintGutter()];

interface Props {
  doc: ConfigDocument;
  onSaved: () => Promise<void>;
}

export default function AdvancedJsonEditor({ doc, onSaved }: Props) {
  const { t } = useTranslation("rules");
  const { active: themeActive } = useTheme();
  const [draft, setDraft] = useState(() => JSON.stringify(doc, null, 2));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setDraft(JSON.stringify(doc, null, 2));
  }, [doc]);

  const syntax = useMemo(() => checkJsonSyntax(draft), [draft]);
  const invalid = syntax.kind === "error";

  const save = async () => {
    if (invalid) return;
    setBusy(true);
    setError(null);
    try {
      const parsed = JSON.parse(draft) as ConfigDocument;
      await ipc.configReplace(parsed);
      await onSaved();
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <div
        className={cn(
          "rounded-md border overflow-hidden",
          invalid ? "border-destructive" : "border-input",
        )}
      >
        <CodeMirror
          value={draft}
          onChange={(v) => setDraft(v)}
          extensions={EXTENSIONS}
          theme={themeActive === "dark" ? "dark" : "light"}
          height="320px"
          basicSetup={{
            lineNumbers: true,
            highlightActiveLineGutter: true,
            highlightActiveLine: true,
            foldGutter: true,
            bracketMatching: true,
            closeBrackets: true,
            indentOnInput: true,
            autocompletion: false,
          }}
        />
      </div>
      <div className="flex items-center gap-2 min-h-[20px]">
        {syntax.kind === "valid" ? (
          <>
            <CheckCircle2 size={14} className="text-emerald-600 shrink-0" />
            <span className="text-xs text-muted-foreground">
              {t("advanced.validTag")}
            </span>
          </>
        ) : (
          <>
            <AlertCircle size={14} className="text-destructive shrink-0" />
            <span className="text-xs text-destructive">
              {syntax.line !== null && syntax.col !== null
                ? t("advanced.parseAt", {
                    line: syntax.line,
                    col: syntax.col,
                    msg: syntax.msg,
                  })
                : syntax.msg}
            </span>
          </>
        )}
      </div>
      {error && (
        <div className="flex items-center gap-2">
          <Badge variant="destructive">{t("advanced.errorTag")}</Badge>
          <span className="text-xs text-muted-foreground">{error}</span>
        </div>
      )}
      <div className="flex justify-end gap-2">
        <Button
          variant="outline"
          onClick={() => setDraft(JSON.stringify(doc, null, 2))}
          disabled={busy}
        >
          {t("advanced.revert")}
        </Button>
        <Button onClick={save} disabled={busy || invalid}>
          {busy ? t("advanced.saving") : t("advanced.save")}
        </Button>
      </div>
    </div>
  );
}
