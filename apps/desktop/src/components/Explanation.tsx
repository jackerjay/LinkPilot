// Recursive view of a MatcherEval tree. Renders ✓ / ✗ per node and a
// short human-readable label. Shared by the Inspector (post-hoc trace of
// a routed URL) and the Test-URL panel (live trace as the user types).

import type { MatcherEval } from "../lib/types";

export function ExplanationView({
  explanation,
  emptyMessage,
}: {
  explanation: MatcherEval | null | undefined;
  emptyMessage: string;
}) {
  if (!explanation) {
    return (
      <div className="empty" style={{ padding: 16 }}>
        {emptyMessage}
      </div>
    );
  }
  return <EvalNode node={explanation} depth={0} />;
}

export function EvalNode({
  node,
  depth,
}: {
  node: MatcherEval;
  depth: number;
}) {
  const matched = node.matched;
  return (
    <div className="matcher" style={{ marginLeft: depth * 12 }}>
      <div className="row">
        <span
          className={`tag ${matched ? "ok" : "danger"}`}
          title={matched ? "matched" : "did not match"}
        >
          {matched ? "✓" : "✗"}
        </span>
        <span className="grow mono">{describeEvalNode(node)}</span>
      </div>
      {hasChildren(node) && (
        <div className="matcher-children">
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
