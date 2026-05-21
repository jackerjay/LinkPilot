// Visual card grid for picking the Halo variant. Lives in Settings →
// Appearance. Each card renders a live mini-version of the wheel so the
// user can see what they're choosing — the previous dropdown forced them
// to launch a real Ask flow before they could tell Frosted from Bezel.

import { useState } from "react";
import { Input } from "@/components/ui/input";
import { HaloPreview } from "./HaloPreview";
import { ipc } from "@/lib/ipc";
import type { PickerStyle } from "@/lib/types";

interface PickerStyleChooserProps {
  value: PickerStyle;
  onChange: (next: PickerStyle) => void;
}

interface Variant {
  id: PickerStyle;
  symbol: string;
  name: string;
  tagline: string;
}

const VARIANTS: Variant[] = [
  {
    id: "frosted",
    symbol: "α",
    name: "Frosted",
    tagline: "Glass sectors, color on the rim",
  },
  {
    id: "bezel",
    symbol: "β",
    name: "Bezel",
    tagline: "Ticks + dots, instrument ring",
  },
  {
    id: "crown",
    symbol: "γ",
    name: "Crown",
    tagline: "Apple Watch — center display",
  },
];

const DEFAULT_TEST_URL = "https://example.com/";

export function PickerStyleChooser({
  value,
  onChange,
}: PickerStyleChooserProps) {
  const [tryError, setTryError] = useState<string | null>(null);
  const [tryPending, setTryPending] = useState(false);
  const [testUrl, setTestUrl] = useState(DEFAULT_TEST_URL);

  const tryPicker = async () => {
    setTryError(null);
    setTryPending(true);
    try {
      await ipc.pickerPreview(testUrl);
    } catch (e) {
      setTryError(String(e));
    } finally {
      // Reset the pending flag on a short delay so the user can see
      // the action registered even when the new window appears
      // instantly. The window itself is owned by Rust and we don't
      // have a "preview closed" event yet.
      setTimeout(() => setTryPending(false), 400);
    }
  };

  return (
    <div className="picker-style-block">
      <div className="picker-style-grid">
        {VARIANTS.map((v) => {
          const selected = value === v.id;
          return (
            <button
              key={v.id}
              type="button"
              className={`picker-style-card${selected ? " selected" : ""}`}
              onClick={() => onChange(v.id)}
              aria-pressed={selected}
            >
              <div className="picker-style-preview">
                <HaloPreview style={v.id} size={160} />
              </div>
              <div className="picker-style-meta">
                <span className="picker-style-name">
                  <span className="picker-style-symbol">{v.symbol}</span>
                  {v.name}
                </span>
                <span className="picker-style-tag">{v.tagline}</span>
              </div>
              {selected && (
                <span className="picker-style-check" aria-hidden>
                  ✓
                </span>
              )}
            </button>
          );
        })}
      </div>
      <div className="picker-style-actions">
        <label className="picker-style-url-field">
          <span>Test URL</span>
          <Input
            value={testUrl}
            onChange={(e) => setTestUrl(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !tryPending) {
                void tryPicker();
              }
            }}
            placeholder={DEFAULT_TEST_URL}
          />
        </label>
        <button
          type="button"
          className="mac-tbtn primary"
          onClick={tryPicker}
          disabled={tryPending}
          title="Open the picker and launch the selected browser/profile"
        >
          {tryPending ? "Opening…" : "Try picker"}
        </button>
        <span className="picker-style-hint">
          Opens the real picker window with your installed browsers and saved
          profile order. Selection opens the test URL.
        </span>
      </div>
      {tryError && <div className="picker-style-error">{tryError}</div>}
    </div>
  );
}
