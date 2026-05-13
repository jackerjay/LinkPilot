// Recursive view of a MatcherEval tree. Renders ✓ / ✗ per node and a
// short human-readable label. Shared by Inspector and Test-URL pages.

import { Check, X } from "lucide-react";
import { cn } from "@/lib/utils";
import type { MatcherEval } from "@/lib/types";

export function ExplanationView({
  explanation,
  emptyMessage,
}: {
  explanation: MatcherEval | null | undefined;
  emptyMessage: string;
}) {
  if (!explanation) {
    return (
      <div className="rounded-md border border-dashed border-border p-4 text-center text-sm text-muted-foreground">
        {emptyMessage}
      </div>
    );
  }
  return <EvalNode node={explanation} depth={0} />;
}

function EvalNode({ node, depth }: { node: MatcherEval; depth: number }) {
  const matched = node.matched;
  return (
    <div className={depth > 0 ? "ml-3 border-l-2 border-border pl-3" : ""}>
      <div className="flex items-center gap-2 py-1">
        <span
          className={cn(
            "flex h-5 w-5 shrink-0 items-center justify-center rounded-full",
            matched
              ? "bg-success/15 text-success"
              : "bg-destructive/15 text-destructive",
          )}
          title={matched ? "matched" : "did not match"}
        >
          {matched ? <Check className="h-3.5 w-3.5" /> : <X className="h-3.5 w-3.5" />}
        </span>
        <span className="font-mono text-xs">{describeEvalNode(node)}</span>
      </div>
      {hasChildren(node) && (
        <div className="mt-1">
          {childList(node).map((c, i) => (
            <EvalNode key={i} node={c} depth={depth + 1} />
          ))}
        </div>
      )}
    </div>
  );
}

function hasChildren(n: MatcherEval): boolean {
  return n.op === "all" || n.op === "any" || n.op === "not";
}

function childList(n: MatcherEval): MatcherEval[] {
  switch (n.op) {
    case "all":
    case "any":
      return n.of;
    case "not":
      return [n.of];
    default:
      return [];
  }
}

function describeEvalNode(n: MatcherEval): string {
  switch (n.op) {
    case "always":
      return "always";
    case "all":
      return `AND (${n.of.length})`;
    case "any":
      return `OR (${n.of.length})`;
    case "not":
      return "NOT";
    case "url-host":
      return `host ${n.pattern}`;
    case "url-path":
      return `path ${n.pattern}`;
    case "source-app":
      return `from app ${n.name}`;
    case "source-browser":
      return `from browser ${n.browser}`;
    case "source-profile":
      return `from profile ${n.profile}`;
  }
}
