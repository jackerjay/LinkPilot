import { useState } from "react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ipc } from "@/lib/ipc";
import { HELPER_CHANGED, useHelper } from "@/lib/helper";

export function HelperCard() {
  const { t } = useTranslation("app");
  const { status, refresh } = useHelper();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      await ipc.helperStart();
      window.dispatchEvent(new Event(HELPER_CHANGED));
    } catch (err) { setError(String(err)); }
    finally { setBusy(false); await refresh(); }
  };
  return <div className="mac-card mac-card-pad" style={{ marginBottom: 16, display: "grid", gap: 8 }}>
    <strong>{t("helper.title")}</strong>
    <p className="mac-muted">{t("helper.description")}</p>
    <span>{status?.connected ? t("helper.connected", { version: status.version }) : t(status ? "helper.disconnected" : "helper.checking")}</span>
    {(error || status?.error) && <p role="status" className="mac-muted" style={{ overflowWrap: "anywhere" }}>{error || status?.error}</p>}
    <div style={{ display: "flex", flexWrap: "wrap", gap: 8 }}>
      <button type="button" className="mac-tbtn" disabled={busy} onClick={() => void start()}>{t("helper.start")}</button>
      <button type="button" className="mac-tbtn" disabled={busy} onClick={() => void refresh()}>{t("helper.refresh")}</button>
      <button type="button" className="mac-tbtn" onClick={() => void ipc.openProfilePrivacySettings().catch((err) => setError(String(err)))}>{t("helper.permissions")}</button>
      <button type="button" className="mac-tbtn" onClick={() => void openUrl("https://github.com/jackerjay/LinkPilot/releases/latest").catch((err) => setError(String(err)))}>{t("helper.download")}</button>
    </div>
  </div>;
}
