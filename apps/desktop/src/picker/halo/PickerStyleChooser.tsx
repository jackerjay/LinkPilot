// Visual card grid for picking the Halo variant. Lives in Settings →
// Appearance. Each card renders a live mini-version of the wheel so the
// user can see what they're choosing — the previous dropdown forced them
// to launch a real Ask flow before they could tell Frosted from Bezel.

import { useState } from "react";
import { useTranslation } from "react-i18next";
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
  nameKey: string;
  taglineKey: string;
}

const VARIANTS: Variant[] = [
  {
    id: "frosted",
    symbol: "α",
    nameKey: "style.variants.frosted.name",
    taglineKey: "style.variants.frosted.tagline",
  },
  {
    id: "bezel",
    symbol: "β",
    nameKey: "style.variants.bezel.name",
    taglineKey: "style.variants.bezel.tagline",
  },
  {
    id: "crown",
    symbol: "γ",
    nameKey: "style.variants.crown.name",
    taglineKey: "style.variants.crown.tagline",
  },
];

const DEFAULT_TEST_URL = "https://example.com/";

export function PickerStyleChooser({
  value,
  onChange,
}: PickerStyleChooserProps) {
  const { t } = useTranslation("picker");
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
                  {t(v.nameKey)}
                </span>
                <span className="picker-style-tag">{t(v.taglineKey)}</span>
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
          <span>{t("style.testUrl")}</span>
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
          title={t("style.openTitle")}
        >
          {tryPending ? t("style.opening") : t("style.tryPicker")}
        </button>
        <span className="picker-style-hint">{t("style.hint")}</span>
      </div>
      {tryError && <div className="picker-style-error">{tryError}</div>}
    </div>
  );
}
