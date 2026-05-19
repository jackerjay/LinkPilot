// First-run onboarding wizard. Four steps with distinct gradient
// backgrounds so the user feels progression. Renders as a full-bleed
// overlay above the main window content; gated by localStorage so it
// only shows once per install (cleared on a reset).
//
// The Rust side doesn't know about onboarding — the entire flow is
// driven by IPC calls already exposed (`request_set_default_browser`,
// `list_browsers`, `config_replace`). That keeps the wizard
// self-contained inside the renderer.

import { useCallback, useEffect, useMemo, useState } from "react";
import { ArrowUpRight, Check, Compass, FileText, Globe, User, Workflow } from "lucide-react";
import { AppIcon } from "@/components/AppIcon";
import { appPathFromExecutable } from "@/lib/browsers";
import { ipc } from "@/lib/ipc";
import type {
  BrowserProfile,
  ConfigDocument,
  InstalledBrowser,
  Rule,
  SetDefaultOutcome,
} from "@/lib/types";
import brandIcon from "@/assets/brand.png";

const ONBOARDING_KEY = "linkpilot.onboarding.completed";

export function isOnboardingNeeded(): boolean {
  if (typeof localStorage === "undefined") return false;
  return localStorage.getItem(ONBOARDING_KEY) !== "1";
}

function markOnboardingComplete(): void {
  if (typeof localStorage !== "undefined") {
    localStorage.setItem(ONBOARDING_KEY, "1");
  }
}

interface Props {
  onFinish: () => void;
}

type Step = 1 | 2 | 3 | 4;

// Per-step gradient — distinct warm/cool palettes so the user feels
// progression. Painted as a soft full-bleed wash. The frosted overlay
// underneath fades cards back to a legible neutral.
const LIGHT_GRADIENTS: Record<Step, string> = {
  1:
    "radial-gradient(80% 70% at 20% 0%, color-mix(in srgb, #5057e8 38%, transparent), transparent 60%)," +
    "radial-gradient(70% 60% at 100% 0%, color-mix(in srgb, #bf5af2 28%, transparent), transparent 65%)," +
    "radial-gradient(60% 50% at 50% 100%, color-mix(in srgb, #5ac8fa 22%, transparent), transparent 70%)",
  2:
    "radial-gradient(70% 60% at 50% 0%, color-mix(in srgb, #007aff 28%, transparent), transparent 65%)," +
    "radial-gradient(60% 50% at 100% 100%, color-mix(in srgb, #5057e8 22%, transparent), transparent 70%)",
  3:
    "radial-gradient(70% 60% at 0% 0%, color-mix(in srgb, #34c759 22%, transparent), transparent 65%)," +
    "radial-gradient(60% 60% at 100% 50%, color-mix(in srgb, #5ac8fa 22%, transparent), transparent 70%)",
  4:
    "radial-gradient(80% 60% at 100% 0%, color-mix(in srgb, #ff9500 22%, transparent), transparent 65%)," +
    "radial-gradient(60% 60% at 0% 100%, color-mix(in srgb, #ff375f 22%, transparent), transparent 70%)",
};

const DARK_GRADIENTS: Record<Step, string> = {
  1:
    "radial-gradient(80% 70% at 20% 0%, color-mix(in srgb, #5057e8 55%, transparent), transparent 60%)," +
    "radial-gradient(70% 60% at 100% 0%, color-mix(in srgb, #bf5af2 38%, transparent), transparent 65%)," +
    "radial-gradient(60% 50% at 50% 100%, color-mix(in srgb, #5ac8fa 28%, transparent), transparent 70%)",
  2:
    "radial-gradient(70% 60% at 50% 0%, color-mix(in srgb, #007aff 40%, transparent), transparent 65%)," +
    "radial-gradient(60% 50% at 100% 100%, color-mix(in srgb, #5057e8 32%, transparent), transparent 70%)",
  3:
    "radial-gradient(70% 60% at 0% 0%, color-mix(in srgb, #34c759 30%, transparent), transparent 65%)," +
    "radial-gradient(60% 60% at 100% 50%, color-mix(in srgb, #5ac8fa 30%, transparent), transparent 70%)",
  4:
    "radial-gradient(80% 60% at 100% 0%, color-mix(in srgb, #ff9500 32%, transparent), transparent 65%)," +
    "radial-gradient(60% 60% at 0% 100%, color-mix(in srgb, #ff375f 32%, transparent), transparent 70%)",
};

interface TemplateChoice {
  id: string;
  label: string;
  detail: string;
  swatch: string;
  enabled: boolean;
  rule: Omit<Rule, "id">;
}

// 4 starter templates. The user toggles the ones they want and we
// upsert them on Finish — the rule engine handles the rest. They are
// inserted in declaration order at the top of `config.rules`, so the
// first template in this list has highest priority (list order IS
// priority).
const RULE_TEMPLATES: TemplateChoice[] = [
  {
    id: "tmpl-work",
    label: "Work → Chrome / Work",
    detail: "github.com, linear.app, notion.so, figma.com",
    swatch: "#1A73E8",
    enabled: true,
    rule: {
      enabled: true,
      source: "gui",
      note: "Work tools → Chrome / Work profile",
      when: {
        op: "any",
        of: [
          { op: "url-host", pattern: "github.com" },
          { op: "url-host", pattern: "linear.app" },
          { op: "url-host", pattern: "notion.so" },
          { op: "url-host", pattern: "figma.com" },
        ],
      },
      then: {
        kind: "open",
        target: { browser: "google-chrome", profile: "Work" },
      },
    },
  },
  {
    id: "tmpl-oauth",
    label: "OAuth → Keep source",
    detail: "/oauth/callback paths stay in the originating browser",
    swatch: "#34c759",
    enabled: true,
    rule: {
      enabled: true,
      source: "gui",
      note: "OAuth redirects must complete in the source browser",
      when: { op: "url-path", pattern: "/oauth/callback" },
      then: { kind: "keep-source" },
    },
  },
  {
    id: "tmpl-media",
    label: "Media → Arc",
    detail: "youtube.com, twitch.tv",
    swatch: "#FF6F61",
    enabled: false,
    rule: {
      enabled: true,
      source: "gui",
      note: "Streaming media → Arc",
      when: {
        op: "any",
        of: [
          { op: "url-host", pattern: "youtube.com" },
          { op: "url-host", pattern: "twitch.tv" },
        ],
      },
      then: { kind: "open", target: { browser: "arc" } },
    },
  },
  {
    id: "tmpl-banking",
    label: "Banking → Safari",
    detail: "Personal banking sites stay in Safari for keychain support",
    swatch: "#1E96F0",
    enabled: false,
    rule: {
      enabled: true,
      source: "gui",
      note: "Banking → Safari (keychain)",
      when: { op: "url-host", pattern: "*.bank.com" },
      then: { kind: "open", target: { browser: "safari" } },
    },
  },
];

export function OnboardingFlow({ onFinish }: Props) {
  const [step, setStep] = useState<Step>(1);
  const [browsers, setBrowsers] = useState<InstalledBrowser[] | null>(null);
  const [profilesByBrowser, setProfilesByBrowser] = useState<
    Record<string, BrowserProfile[]>
  >({});
  const [browserOn, setBrowserOn] = useState<Record<string, boolean>>({});
  const [templates, setTemplates] = useState<TemplateChoice[]>(RULE_TEMPLATES);
  const [isDark, setIsDark] = useState(false);
  const [isDefault, setIsDefault] = useState<boolean | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  // Track the resolved theme so the gradients use the right palette.
  useEffect(() => {
    const update = () =>
      setIsDark(document.documentElement.classList.contains("dark"));
    update();
    const obs = new MutationObserver(update);
    obs.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    });
    return () => obs.disconnect();
  }, []);

  // Lazily fetch browsers when we hit step 3 — saves a few ms on the
  // initial paint of step 1.
  useEffect(() => {
    if (step !== 3 || browsers !== null) return;
    (async () => {
      try {
        const installed = await ipc.listBrowsers();
        setBrowsers(installed);
        setBrowserOn(
          Object.fromEntries(installed.map((b) => [b.id, true])),
        );
        const profileEntries = await Promise.all(
          installed.map(async (b) => {
            try {
              return [b.id, await ipc.listProfiles(b.id)] as const;
            } catch {
              return [b.id, [] as BrowserProfile[]] as const;
            }
          }),
        );
        setProfilesByBrowser(Object.fromEntries(profileEntries));
      } catch (err) {
        setMessage(`Browser scan failed: ${err}`);
      }
    })();
  }, [step, browsers]);

  // Reflect current default-browser status at the top of step 2 so the
  // user can see when the macOS prompt has been accepted.
  useEffect(() => {
    if (step !== 2) return;
    ipc.isDefaultBrowser().then(setIsDefault).catch(() => setIsDefault(null));
  }, [step]);

  const setDefault = useCallback(async () => {
    setBusy(true);
    setMessage(null);
    try {
      const outcome: SetDefaultOutcome = await ipc.requestSetDefaultBrowser();
      if (outcome.kind === "done") {
        setMessage("macOS will prompt you to confirm.");
      } else if (outcome.kind === "user-consent-required") {
        setMessage(
          "Confirm the switch in System Settings → Desktop & Dock.",
        );
      } else {
        setMessage("Default-browser API isn't available on this platform.");
      }
      const next = await ipc.isDefaultBrowser();
      setIsDefault(next);
    } catch (err) {
      setMessage(`Failed: ${err}`);
    } finally {
      setBusy(false);
    }
  }, []);

  const finish = useCallback(async () => {
    setBusy(true);
    setMessage(null);
    // Persist the toggled rule templates. Failures here are NON-FATAL:
    // the user came to "Finish", they want out of onboarding. If we
    // can't write the templates (schema mismatch, IPC down) we surface
    // a warning but still mark onboarding complete so the user lands
    // on the main app — they can always recreate the rules manually.
    try {
      const cfg: ConfigDocument = await ipc.configGet();
      const toAdd = templates.filter((t) => t.enabled);
      // Rule.id is a UUID on the Rust side (RuleId(Uuid) newtype) — a
      // human-readable string like `tmpl-work-xyz` fails serde
      // deserialization and the entire configReplace bails. Use
      // crypto.randomUUID so the rule round-trips cleanly.
      //
      // Templates are prepended in their declared order, so the first
      // toggled template ends up at slot #1 (highest priority).
      const newRules: Rule[] = toAdd.map((t) => ({
        id: crypto.randomUUID(),
        ...t.rule,
      }));
      if (newRules.length > 0) {
        await ipc.configReplace({
          ...cfg,
          rules: [...newRules, ...cfg.rules],
        });
      }
    } catch (err) {
      // Print to console so the DevTools shows the real IPC error
      // (e.g. config schema mismatch, unknown browser id) — the
      // `setMessage` text alone gives us the stringified Error which
      // hides the structured details from `invoke()`.
      console.error("onboarding: configReplace failed", err);
      setMessage(
        `Template save failed (${err}). Continuing without templates — you can add rules from the Rules tab.`,
      );
    }
    // Reached regardless of save outcome — Finish always exits the
    // wizard so the user isn't trapped.
    markOnboardingComplete();
    onFinish();
  }, [templates, onFinish]);

  const skip = useCallback(() => {
    markOnboardingComplete();
    onFinish();
  }, [onFinish]);

  const gradient = isDark ? DARK_GRADIENTS[step] : LIGHT_GRADIENTS[step];
  const overlay = isDark
    ? "linear-gradient(180deg, rgba(31,31,34,0.05) 0%, rgba(31,31,34,0.55) 100%)"
    : "linear-gradient(180deg, rgba(255,255,255,0.05) 0%, rgba(236,236,238,0.55) 100%)";

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        zIndex: 50,
        overflow: "hidden",
        background: "var(--mac-window-fill)",
        color: "var(--mac-fg)",
      }}
    >
      {/* Gradient wash */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          background: gradient,
          filter: "saturate(110%)",
          pointerEvents: "none",
        }}
      />
      {/* Frosted overlay so cards / text read against the wash */}
      <div
        style={{
          position: "absolute",
          inset: 0,
          background: overlay,
          pointerEvents: "none",
        }}
      />

      <div
        style={{
          position: "absolute",
          inset: 0,
          padding: 32,
          display: "flex",
          flexDirection: "column",
        }}
      >
        {/* Stepper */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "center",
            gap: 4,
            marginTop: 24,
            marginBottom: 28,
          }}
        >
          {([1, 2, 3, 4] as Step[]).map((n, i) => (
            <span
              key={n}
              style={{ display: "inline-flex", alignItems: "center", gap: 4 }}
            >
              <span
                className={
                  "mac-onboarding-step" +
                  (n === step ? " active" : n < step ? " done" : "")
                }
              >
                {n < step ? <Check size={12} strokeWidth={2.5} /> : n}
              </span>
              {i < 3 && (
                <span
                  className={
                    "mac-onboarding-trail" + (n < step ? " done" : "")
                  }
                />
              )}
            </span>
          ))}
        </div>

        {/* Body */}
        <div
          style={{
            flex: 1,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            textAlign: "center",
            maxWidth: 520,
            margin: "0 auto",
            width: "100%",
          }}
        >
          {step === 1 && <Step1 />}
          {step === 2 && (
            <Step2
              isDefault={isDefault}
              busy={busy}
              onSetDefault={setDefault}
              message={message}
            />
          )}
          {step === 3 && (
            <Step3
              browsers={browsers}
              browserOn={browserOn}
              setBrowserOn={setBrowserOn}
              profilesByBrowser={profilesByBrowser}
            />
          )}
          {step === 4 && (
            <Step4 templates={templates} setTemplates={setTemplates} />
          )}
        </div>

        {/* Global message bar — visible from every step. Previously
            messages were only rendered inside <Step2/>, so any error
            triggered by finish() on Step 4 (e.g. a rule that the Rust
            config validator rejected) showed nothing and looked like
            the Finish button was inert. */}
        {message && step !== 2 && (
          <div
            className="mac-muted"
            style={{
              marginTop: 12,
              padding: "8px 12px",
              borderRadius: 8,
              background: "var(--mac-card-fill)",
              border: "0.5px solid var(--mac-border)",
              fontSize: 12,
              textAlign: "center",
            }}
          >
            {message}
          </div>
        )}

        {/* Footer */}
        <div
          style={{
            display: "flex",
            alignItems: "center",
            paddingTop: 14,
            marginTop: 14,
            borderTop: "0.5px solid var(--mac-divider)",
          }}
        >
          <button type="button" className="mac-tbtn" onClick={skip}>
            Skip setup
          </button>
          <span style={{ flex: 1 }} />
          <span
            className="mac-muted"
            style={{ fontSize: 12, marginRight: 12 }}
          >
            Step {step} of 4
          </span>
          {step > 1 && (
            <button
              type="button"
              className="mac-tbtn"
              style={{ marginRight: 6 }}
              onClick={() => setStep(((step - 1) as Step))}
              disabled={busy}
            >
              Back
            </button>
          )}
          <button
            type="button"
            className="mac-tbtn primary"
            disabled={busy}
            onClick={() => {
              if (step === 4) {
                finish();
              } else {
                setStep(((step + 1) as Step));
              }
            }}
          >
            {step === 4 ? (busy ? "Saving…" : "Finish") : "Continue"}
          </button>
        </div>
      </div>
    </div>
  );
}

function Step1() {
  const features = [
    { icon: Workflow, t: "Rules engine", d: "Match by host, app, or path." },
    { icon: Globe, t: "Multi-browser", d: "Chrome, Arc, Safari, Firefox." },
    { icon: User, t: "Profiles", d: "Send work to Work, fun to Fun." },
    { icon: FileText, t: "Inspector", d: "Every decision is auditable." },
  ];
  return (
    <>
      <img
        src={brandIcon}
        width={96}
        height={96}
        alt="LinkPilot"
        style={{
          borderRadius: 24,
          boxShadow:
            "0 18px 40px rgba(60,72,200,0.3), 0 0 0 0.5px rgba(0,0,0,0.08)",
          marginBottom: 24,
        }}
      />
      <h1
        style={{
          margin: 0,
          fontSize: 26,
          fontWeight: 700,
          letterSpacing: "-0.02em",
        }}
      >
        Welcome to LinkPilot.
      </h1>
      <p
        style={{
          color: "var(--mac-fg-muted)",
          maxWidth: 360,
          marginTop: 8,
          fontSize: 14,
          lineHeight: 1.5,
        }}
      >
        Route every link to the right browser, profile, and workspace — without
        thinking about it.
      </p>
      <div
        style={{
          marginTop: 28,
          display: "grid",
          gridTemplateColumns: "1fr 1fr",
          gap: 10,
          width: "100%",
          maxWidth: 420,
        }}
      >
        {features.map((f) => {
          const Icon = f.icon;
          return (
            <div
              key={f.t}
              style={{
                padding: 12,
                background: "var(--mac-card-fill)",
                border: "0.5px solid var(--mac-border)",
                borderRadius: 10,
                textAlign: "left",
                display: "flex",
                gap: 10,
                alignItems: "flex-start",
              }}
            >
              <span
                style={{
                  width: 28,
                  height: 28,
                  borderRadius: 7,
                  background: "var(--mac-accent-soft)",
                  color: "var(--mac-accent)",
                  display: "inline-flex",
                  alignItems: "center",
                  justifyContent: "center",
                  flex: "0 0 28px",
                }}
              >
                <Icon size={15} strokeWidth={1.8} />
              </span>
              <div>
                <div style={{ fontWeight: 600, fontSize: 13 }}>{f.t}</div>
                <div style={{ fontSize: 12, color: "var(--mac-fg-muted)" }}>
                  {f.d}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </>
  );
}

function Step2({
  isDefault,
  busy,
  onSetDefault,
  message,
}: {
  isDefault: boolean | null;
  busy: boolean;
  onSetDefault: () => void;
  message: string | null;
}) {
  return (
    <>
      <h1
        style={{
          margin: 0,
          fontSize: 22,
          fontWeight: 700,
          letterSpacing: "-0.01em",
        }}
      >
        Set LinkPilot as your default browser
      </h1>
      <p
        style={{
          color: "var(--mac-fg-muted)",
          maxWidth: 380,
          marginTop: 8,
        }}
      >
        macOS will show a confirmation dialog. LinkPilot doesn't render web
        pages — it just decides which browser to hand each link to.
      </p>
      <div
        style={{
          marginTop: 24,
          display: "flex",
          alignItems: "center",
          gap: 16,
        }}
      >
        <span
          style={{
            width: 56,
            height: 56,
            borderRadius: 14,
            background: "#7a4ad4",
            color: "white",
            display: "inline-flex",
            alignItems: "center",
            justifyContent: "center",
            fontSize: 22,
            fontWeight: 600,
          }}
        >
          URL
        </span>
        <ArrowUpRight size={18} strokeWidth={1.8} />
        <img
          src={brandIcon}
          width={56}
          height={56}
          alt=""
          style={{
            borderRadius: 14,
            boxShadow:
              "0 8px 20px rgba(60,72,200,0.25), 0 0 0 0.5px rgba(0,0,0,0.08)",
          }}
        />
        <ArrowUpRight size={18} strokeWidth={1.8} />
        <span style={{ color: "var(--mac-fg-muted)" }}>
          <Compass size={32} strokeWidth={1.8} />
        </span>
      </div>
      <div style={{ marginTop: 32 }}>
        <button
          type="button"
          className="mac-tbtn primary"
          style={{ height: 32, padding: "0 20px", fontSize: 13 }}
          disabled={busy || !!isDefault}
          onClick={onSetDefault}
        >
          {isDefault
            ? "Already default ✓"
            : busy
            ? "Working…"
            : "Set LinkPilot as default…"}
        </button>
      </div>
      {message && (
        <p
          style={{
            color: "var(--mac-fg-muted)",
            fontSize: 12,
            marginTop: 12,
            maxWidth: 380,
          }}
        >
          {message}
        </p>
      )}
      <p
        style={{
          color: "var(--mac-fg-tertiary)",
          fontSize: 11,
          marginTop: 14,
          maxWidth: 360,
        }}
      >
        You can change this at any time in System Settings → Desktop & Dock.
      </p>
    </>
  );
}

function Step3({
  browsers,
  browserOn,
  setBrowserOn,
  profilesByBrowser,
}: {
  browsers: InstalledBrowser[] | null;
  browserOn: Record<string, boolean>;
  setBrowserOn: (next: Record<string, boolean>) => void;
  profilesByBrowser: Record<string, BrowserProfile[]>;
}) {
  const sorted = useMemo(
    () => (browsers ? [...browsers].sort((a, b) => a.display_name.localeCompare(b.display_name)) : null),
    [browsers],
  );
  return (
    <>
      <h1 style={{ margin: 0, fontSize: 22, fontWeight: 700 }}>
        Pick which browsers LinkPilot can route to
      </h1>
      <p style={{ color: "var(--mac-fg-muted)", marginTop: 8 }}>
        {sorted === null
          ? "Scanning…"
          : `We scanned /Applications and found ${sorted.length} browsers.`}
      </p>
      <div style={{ marginTop: 20, width: "100%", maxWidth: 460 }}>
        {sorted?.map((b, i) => {
          const profiles = profilesByBrowser[b.id] ?? [];
          const profileLabel =
            profiles.length === 0
              ? "No profiles"
              : profiles.map((p) => p.display_name).join(" · ");
          const isFirst = i === 0;
          const isLast = i === (sorted.length - 1);
          return (
            <div
              key={b.id}
              style={{
                display: "flex",
                alignItems: "center",
                gap: 12,
                padding: 12,
                background: "var(--mac-card-fill)",
                border: "0.5px solid var(--mac-border)",
                borderTopWidth: isFirst ? "0.5px" : 0,
                borderRadius: isFirst
                  ? "10px 10px 0 0"
                  : isLast
                  ? "0 0 10px 10px"
                  : 0,
              }}
            >
              <AppIcon
                bundleId={b.platform_app_id ?? undefined}
                appPath={appPathFromExecutable(b.executable)}
                size={32}
                alt={b.display_name}
                className="shrink-0"
              />
              <div style={{ flex: 1, textAlign: "left" }}>
                <div style={{ fontWeight: 600, fontSize: 13 }}>
                  {b.display_name}
                </div>
                <div style={{ fontSize: 11.5, color: "var(--mac-fg-muted)" }}>
                  {profileLabel}
                </div>
              </div>
              <button
                type="button"
                className={
                  "mac-switch accent" + (browserOn[b.id] ? " on" : "")
                }
                aria-pressed={!!browserOn[b.id]}
                onClick={() =>
                  setBrowserOn({
                    ...browserOn,
                    [b.id]: !browserOn[b.id],
                  })
                }
              />
            </div>
          );
        })}
      </div>
    </>
  );
}

function Step4({
  templates,
  setTemplates,
}: {
  templates: TemplateChoice[];
  setTemplates: (next: TemplateChoice[]) => void;
}) {
  const toggle = (id: string) =>
    setTemplates(
      templates.map((t) => (t.id === id ? { ...t, enabled: !t.enabled } : t)),
    );
  return (
    <>
      <h1 style={{ margin: 0, fontSize: 22, fontWeight: 700 }}>
        Start from a template
      </h1>
      <p
        style={{
          color: "var(--mac-fg-muted)",
          marginTop: 8,
          maxWidth: 380,
        }}
      >
        We've drafted four rules. Toggle the ones you want; tweak the rest in
        the Rules tab later.
      </p>
      <div
        style={{
          marginTop: 18,
          width: "100%",
          maxWidth: 460,
          display: "grid",
          gap: 8,
        }}
      >
        {templates.map((tpl) => (
          <div
            key={tpl.id}
            style={{
              display: "flex",
              alignItems: "center",
              gap: 12,
              padding: "12px 14px",
              background: "var(--mac-card-fill)",
              border: "0.5px solid var(--mac-border)",
              borderRadius: 10,
            }}
          >
            <span
              style={{
                width: 8,
                height: 32,
                borderRadius: 4,
                background: tpl.swatch,
                flex: "0 0 8px",
              }}
            />
            <div style={{ flex: 1, textAlign: "left" }}>
              <div style={{ fontWeight: 600, fontSize: 13 }}>{tpl.label}</div>
              <div style={{ fontSize: 11.5, color: "var(--mac-fg-muted)" }}>
                {tpl.detail}
              </div>
            </div>
            <button
              type="button"
              className={"mac-switch accent" + (tpl.enabled ? " on" : "")}
              aria-pressed={tpl.enabled}
              onClick={() => toggle(tpl.id)}
            />
          </div>
        ))}
      </div>
      <p
        style={{
          color: "var(--mac-fg-tertiary)",
          fontSize: 11,
          marginTop: 14,
        }}
      >
        You can edit, reorder, or delete rules any time.
      </p>
    </>
  );
}
