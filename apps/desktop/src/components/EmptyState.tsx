// Shared empty-state block for cards: soft icon disc + title + hint.
// Keeps "nothing here yet" moments visually intentional instead of a
// bare muted sentence.

import type { ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

interface EmptyStateProps {
  icon: LucideIcon;
  title: string;
  /** Secondary line; accepts rich nodes (e.g. <Trans> with <code>). */
  hint?: ReactNode;
  /** Optional action row rendered under the hint. */
  action?: ReactNode;
}

export function EmptyState({ icon: Icon, title, hint, action }: EmptyStateProps) {
  return (
    <div
      style={{
        display: "flex",
        flexDirection: "column",
        alignItems: "center",
        textAlign: "center",
        padding: "26px 18px",
        gap: 4,
      }}
    >
      <span
        style={{
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          width: 40,
          height: 40,
          marginBottom: 6,
          borderRadius: "50%",
          background: "var(--mac-accent-soft)",
          color: "var(--mac-accent)",
        }}
      >
        <Icon size={19} strokeWidth={1.8} />
      </span>
      <span style={{ fontSize: 13, fontWeight: 600 }}>{title}</span>
      {hint && (
        <span
          className="mac-muted"
          style={{ fontSize: 12, maxWidth: 360, lineHeight: 1.5 }}
        >
          {hint}
        </span>
      )}
      {action && <div style={{ marginTop: 10 }}>{action}</div>}
    </div>
  );
}
