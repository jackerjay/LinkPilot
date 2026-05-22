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
import {
  ArrowRight,
  Check,
  FileText,
  Globe,
  Mail,
  MessageSquare,
  User,
  Workflow,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { AppIcon } from "@/components/AppIcon";
import { appPathFromExecutable } from "@/lib/browsers";
import { ipc } from "@/lib/ipc";
import type {
  BrowserProfile,
  BrowserTarget,
  ConfigDocument,
  InstalledBrowser,
  MatcherTree,
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

type Step = 1 | 2 | 3 | 4 | 5;

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
    "radial-gradient(70% 60% at 50% 0%, color-mix(in srgb, #5856d6 28%, transparent), transparent 65%)," +
    "radial-gradient(60% 60% at 0% 100%, color-mix(in srgb, #5ac8fa 22%, transparent), transparent 70%)",
  5:
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
    "radial-gradient(70% 60% at 50% 0%, color-mix(in srgb, #5856d6 42%, transparent), transparent 65%)," +
    "radial-gradient(60% 60% at 0% 100%, color-mix(in srgb, #5ac8fa 30%, transparent), transparent 70%)",
  5:
    "radial-gradient(80% 60% at 100% 0%, color-mix(in srgb, #ff9500 32%, transparent), transparent 65%)," +
    "radial-gradient(60% 60% at 0% 100%, color-mix(in srgb, #ff375f 32%, transparent), transparent 70%)",
};

interface TemplateChoice {
  id: string;
  labelKey: string;
  detailKey: string;
  noteKey: string;
  swatch: string;
  enabled: boolean;
  when: MatcherTree;
  /** Priority-ordered list of browser ids. The first one present in the
   *  detected inventory wins; if none are installed the template is
   *  hidden from Step 5 instead of silently creating a dead rule.
   *  `null` for templates whose action doesn't reference a browser
   *  (e.g. keep-source). */
  candidates: string[] | null;
  /** Browser profile name to attach to the resolved target. Preserved
   *  literally — when the user's Chrome happens to have a "Work" profile
   *  this hits directly; when it doesn't, the router falls back to the
   *  browser's default profile (`null` would mean "always default"). */
  profile?: string;
}

type ResolvedTemplate =
  | { kind: "keep-source" }
  | { kind: "open"; browserId: string; displayName: string };

function resolveTemplate(
  tpl: TemplateChoice,
  browsers: InstalledBrowser[],
): ResolvedTemplate | null {
  if (tpl.candidates === null) return { kind: "keep-source" };
  const installed = new Map(browsers.map((b) => [b.id, b]));
  for (const id of tpl.candidates) {
    const found = installed.get(id);
    if (found) {
      return { kind: "open", browserId: id, displayName: found.display_name };
    }
  }
  return null;
}

// Starter templates. The user toggles the ones they want and we upsert
// them on Finish — the rule engine handles the rest. They are inserted
// in declaration order at the top of `config.rules`, so the first
// template in this list has highest priority (list order IS priority).
//
// `candidates` lets us adapt to the actual inventory: "Work → Chrome"
// becomes "Work → Edge" or "Work → Brave" on a Mac without Chrome,
// instead of writing a rule that points at a browser id that isn't
// there. Walk the list, first installed wins; nothing installed →
// template is hidden in `Step5Templates`.
const RULE_TEMPLATES: TemplateChoice[] = [
  {
    id: "tmpl-work",
    labelKey: "templates.work.label",
    detailKey: "templates.work.detail",
    noteKey: "templates.work.note",
    swatch: "#1A73E8",
    enabled: true,
    when: {
      op: "any",
      of: [
        { op: "url-host", pattern: "github.com" },
        { op: "url-host", pattern: "linear.app" },
        { op: "url-host", pattern: "notion.so" },
        { op: "url-host", pattern: "figma.com" },
      ],
    },
    // Chromium family with a profile concept. `chrome` was originally
    // `google-chrome`, which didn't match the inventory id — that bug
    // meant every "Work" rule pointed at a non-existent browser.
    candidates: ["chrome", "edge", "brave", "vivaldi", "arc"],
    profile: "Work",
  },
  {
    id: "tmpl-oauth",
    labelKey: "templates.oauth.label",
    detailKey: "templates.oauth.detail",
    noteKey: "templates.oauth.note",
    swatch: "#34c759",
    enabled: true,
    when: { op: "url-path", pattern: "/oauth/callback" },
    candidates: null,
  },
  {
    id: "tmpl-media",
    labelKey: "templates.media.label",
    detailKey: "templates.media.detail",
    noteKey: "templates.media.note",
    swatch: "#FF6F61",
    enabled: false,
    when: {
      op: "any",
      of: [
        { op: "url-host", pattern: "youtube.com" },
        { op: "url-host", pattern: "twitch.tv" },
      ],
    },
    // Arc-leaning fallback chain: Arc itself first, then other
    // Chromium-family browsers (still good as a "media corner").
    candidates: ["arc", "vivaldi", "dia", "brave", "chrome", "edge", "opera"],
  },
  {
    id: "tmpl-banking",
    labelKey: "templates.banking.label",
    detailKey: "templates.banking.detail",
    noteKey: "templates.banking.note",
    swatch: "#1E96F0",
    enabled: false,
    when: { op: "url-host", pattern: "*.bank.com" },
    // Safari ships with macOS, so this candidate list is always met.
    // Keep it narrow because the banking template's whole point is
    // Keychain integration, which is Safari-specific.
    candidates: ["safari"],
  },
];

export function OnboardingFlow({ onFinish }: Props) {
  const { t } = useTranslation("onboarding");
  const [step, setStep] = useState<Step>(1);
  const [browsers, setBrowsers] = useState<InstalledBrowser[] | null>(null);
  const [profilesByBrowser, setProfilesByBrowser] = useState<
    Record<string, BrowserProfile[]>
  >({});
  const [browserOn, setBrowserOn] = useState<Record<string, boolean>>({});
  const [templates, setTemplates] = useState<TemplateChoice[]>(RULE_TEMPLATES);
  const [isDark, setIsDark] = useState(false);
  const [isDefault, setIsDefault] = useState<boolean | null>(null);
  const [defaultTarget, setDefaultTarget] = useState<BrowserTarget | null>(
    null,
  );
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

  // Lazily fetch browsers once we hit any step that displays them: step
  // 2 stacks installed-browser icons in the "URL → LinkPilot → browser"
  // illustration, step 3 lists them, step 4 lets the user pick the
  // default routing target. We skip step 1 (welcome) so the initial
  // paint doesn't wait on a filesystem scan.
  useEffect(() => {
    if (step === 1 || browsers !== null) return;
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
        setMessage(t("messages.browserScanFailed", { error: String(err) }));
      }
    })();
  }, [step, browsers, t]);

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
        setMessage(t("messages.defaultDone"));
      } else if (outcome.kind === "user-consent-required") {
        setMessage(t("messages.defaultConsent"));
      } else {
        setMessage(t("messages.defaultUnsupported"));
      }
      const next = await ipc.isDefaultBrowser();
      setIsDefault(next);
    } catch (err) {
      setMessage(t("messages.failed", { error: String(err) }));
    } finally {
      setBusy(false);
    }
  }, [t]);

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
      const installedBrowsers = browsers ?? [];
      const newRules: Rule[] = toAdd
        .map((tpl): Rule | null => {
          const resolved = resolveTemplate(tpl, installedBrowsers);
          if (resolved === null) return null;
          const interp =
            resolved.kind === "open"
              ? { browser: resolved.displayName, profile: tpl.profile ?? "" }
              : {};
          const then: Rule["then"] =
            resolved.kind === "keep-source"
              ? { kind: "keep-source" }
              : {
                  kind: "open",
                  target: {
                    browser: resolved.browserId,
                    profile: tpl.profile ?? null,
                    incognito: false,
                    new_window: false,
                  },
                };
          return {
            // Rule.id is a UUID on the Rust side (RuleId(Uuid) newtype) — a
            // human-readable string like `tmpl-work-xyz` fails serde
            // deserialization and the entire configReplace bails. Use
            // crypto.randomUUID so the rule round-trips cleanly.
            //
            // Templates are prepended in their declared order, so the first
            // toggled template ends up at slot #1 (highest priority).
            id: crypto.randomUUID(),
            enabled: true,
            source: "gui",
            when: tpl.when,
            then,
            note: t(tpl.noteKey, interp),
          };
        })
        .filter((r): r is Rule => r !== null);
      const needsRules = newRules.length > 0;
      const needsDefault = defaultTarget !== null;
      if (needsRules || needsDefault) {
        await ipc.configReplace({
          ...cfg,
          default_target: needsDefault ? defaultTarget : cfg.default_target,
          rules: needsRules ? [...newRules, ...cfg.rules] : cfg.rules,
        });
      }
    } catch (err) {
      // Print to console so the DevTools shows the real IPC error
      // (e.g. config schema mismatch, unknown browser id) — the
      // `setMessage` text alone gives us the stringified Error which
      // hides the structured details from `invoke()`.
      console.error("onboarding: configReplace failed", err);
      setMessage(
        t("messages.templateSaveFailed", { error: String(err) }),
      );
    }
    // Reached regardless of save outcome — Finish always exits the
    // wizard so the user isn't trapped.
    markOnboardingComplete();
    onFinish();
  }, [templates, defaultTarget, browsers, onFinish, t]);

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
          {([1, 2, 3, 4, 5] as Step[]).map((n, i) => (
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
              {i < 4 && (
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
              browsers={browsers}
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
            <Step4Default
              browsers={browsers}
              profilesByBrowser={profilesByBrowser}
              value={defaultTarget}
              onPick={setDefaultTarget}
            />
          )}
          {step === 5 && (
            <Step5Templates
              templates={templates}
              setTemplates={setTemplates}
              browsers={browsers}
            />
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
            {t("footer.skip")}
          </button>
          <span style={{ flex: 1 }} />
          <span
            className="mac-muted"
            style={{ fontSize: 12, marginRight: 12 }}
          >
            {t("footer.step", { step, total: 5 })}
          </span>
          {step > 1 && (
            <button
              type="button"
              className="mac-tbtn"
              style={{ marginRight: 6 }}
              onClick={() => setStep(((step - 1) as Step))}
              disabled={busy}
            >
              {t("footer.back")}
            </button>
          )}
          <button
            type="button"
            className="mac-tbtn primary"
            disabled={busy}
            onClick={() => {
              if (step === 5) {
                finish();
              } else {
                setStep(((step + 1) as Step));
              }
            }}
          >
            {step === 5
              ? busy
                ? t("footer.saving")
                : t("footer.finish")
              : t("footer.continue")}
          </button>
        </div>
      </div>
    </div>
  );
}

function Step1() {
  const { t } = useTranslation("onboarding");
  const features = [
    {
      icon: Workflow,
      key: "rules",
      title: t("step1.features.rules.title"),
      detail: t("step1.features.rules.detail"),
    },
    {
      icon: Globe,
      key: "browsers",
      title: t("step1.features.browsers.title"),
      detail: t("step1.features.browsers.detail"),
    },
    {
      icon: User,
      key: "profiles",
      title: t("step1.features.profiles.title"),
      detail: t("step1.features.profiles.detail"),
    },
    {
      icon: FileText,
      key: "inspector",
      title: t("step1.features.inspector.title"),
      detail: t("step1.features.inspector.detail"),
    },
  ];
  return (
    <>
      <img
        src={brandIcon}
        width={96}
        height={96}
        alt={t("step1.logoAlt")}
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
        {t("step1.title")}
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
        {t("step1.body")}
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
              key={f.key}
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
                <div style={{ fontWeight: 600, fontSize: 13 }}>{f.title}</div>
                <div style={{ fontSize: 12, color: "var(--mac-fg-muted)" }}>
                  {f.detail}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </>
  );
}

// Mirrors BrowserStack on the other end of the row: three colored tiles
// representing typical link sources (mail, chat, notes). We keep these
// abstract — Step 2 fires before we know which messaging app the user
// actually has installed, and the symmetry with "your real browsers"
// only needs to land visually, not literally.
const URL_SOURCE_TILES: Array<{
  key: string;
  icon: typeof Mail;
  gradient: string;
  shadow: string;
}> = [
  {
    key: "mail",
    icon: Mail,
    gradient: "linear-gradient(135deg, #5e5ce6 0%, #4a48c9 100%)",
    shadow: "0 4px 10px rgba(94,92,230,0.32)",
  },
  {
    key: "chat",
    icon: MessageSquare,
    gradient: "linear-gradient(135deg, #7a4ad4 0%, #5f3bb8 100%)",
    shadow: "0 4px 10px rgba(122,74,212,0.32)",
  },
  {
    key: "doc",
    icon: FileText,
    gradient: "linear-gradient(135deg, #bf5af2 0%, #9b3fd1 100%)",
    shadow: "0 4px 10px rgba(191,90,242,0.32)",
  },
];

function UrlTile() {
  const offsets = [-16, 0, 16];
  const rotations = [-7, 0, 7];
  return (
    <span
      style={{
        width: 56,
        height: 56,
        position: "relative",
        display: "inline-block",
      }}
      aria-label="URL sources"
    >
      {URL_SOURCE_TILES.map((tile, i) => {
        const Icon = tile.icon;
        return (
          <span
            key={tile.key}
            style={{
              position: "absolute",
              top: "50%",
              left: "50%",
              width: 36,
              height: 36,
              marginTop: -18,
              marginLeft: -18,
              transform: `translateX(${offsets[i]}px) rotate(${rotations[i]}deg)`,
              borderRadius: 9,
              background: tile.gradient,
              color: "white",
              display: "inline-flex",
              alignItems: "center",
              justifyContent: "center",
              boxShadow: `${tile.shadow}, 0 0 0 1.5px var(--mac-window-fill)`,
              zIndex: URL_SOURCE_TILES.length - i,
            }}
          >
            <Icon size={18} strokeWidth={2.2} />
          </span>
        );
      })}
    </span>
  );
}

// Visual cue for "your installed browsers". Stacks up to three real app
// icons inside a 56×56 slot so this end of the URL → LinkPilot → ? flow
// reads as concrete (the actual apps on this Mac) rather than the
// abstract Compass placeholder we shipped before.
function BrowserStack({ browsers }: { browsers: InstalledBrowser[] | null }) {
  // Bare 56×56 placeholder while the scan is in flight or when nothing
  // was detected — keeps layout stable so the row doesn't reflow when
  // browsers resolve a moment later.
  if (!browsers || browsers.length === 0) {
    return (
      <span
        style={{
          width: 56,
          height: 56,
          borderRadius: 14,
          border: "0.5px dashed var(--mac-border)",
          background: "var(--mac-card-fill)",
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          color: "var(--mac-fg-muted)",
        }}
      >
        <Globe size={26} strokeWidth={1.7} />
      </span>
    );
  }

  // Deterministic ordering so a re-render doesn't shuffle the stack.
  // Sort by display name so the same three icons appear every time.
  const top = [...browsers]
    .sort((a, b) => a.display_name.localeCompare(b.display_name))
    .slice(0, 3);
  const single = top.length === 1;
  // Two-icon mode is centered with a small offset; one-icon mode fills
  // the slot. Three-icon mode fans across the 56px width.
  const offsets = single ? [0] : top.length === 2 ? [-10, 10] : [-16, 0, 16];
  const rotations = top.length === 3 ? [-7, 0, 7] : top.length === 2 ? [-4, 4] : [0];

  return (
    <span
      style={{
        width: 56,
        height: 56,
        position: "relative",
        display: "inline-block",
      }}
      aria-label={top.map((b) => b.display_name).join(", ")}
    >
      {top.map((b, i) => (
        <span
          key={b.id}
          style={{
            position: "absolute",
            top: "50%",
            left: "50%",
            width: 36,
            height: 36,
            marginTop: -18,
            marginLeft: -18,
            transform: `translateX(${offsets[i]}px) rotate(${rotations[i]}deg)`,
            borderRadius: 9,
            background: "var(--mac-window-fill)",
            boxShadow:
              "0 6px 14px rgba(0,0,0,0.18), 0 0 0 1.5px var(--mac-window-fill)",
            zIndex: top.length - i,
          }}
        >
          <AppIcon
            bundleId={b.platform_app_id ?? undefined}
            appPath={appPathFromExecutable(b.executable)}
            size={36}
            alt={b.display_name}
          />
        </span>
      ))}
    </span>
  );
}

function Step2({
  isDefault,
  busy,
  onSetDefault,
  message,
  browsers,
}: {
  isDefault: boolean | null;
  busy: boolean;
  onSetDefault: () => void;
  message: string | null;
  browsers: InstalledBrowser[] | null;
}) {
  const { t } = useTranslation("onboarding");
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
        {t("step2.title")}
      </h1>
      <p
        style={{
          color: "var(--mac-fg-muted)",
          maxWidth: 380,
          marginTop: 8,
        }}
      >
        {t("step2.body")}
      </p>
      <div
        style={{
          marginTop: 24,
          display: "flex",
          alignItems: "center",
          gap: 14,
        }}
      >
        <UrlTile />
        <ArrowRight
          size={18}
          strokeWidth={1.8}
          style={{ color: "var(--mac-fg-muted)" }}
        />
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
        <ArrowRight
          size={18}
          strokeWidth={1.8}
          style={{ color: "var(--mac-fg-muted)" }}
        />
        <BrowserStack browsers={browsers} />
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
            ? t("step2.alreadyDefault")
            : busy
            ? t("step2.working")
            : t("step2.setDefault")}
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
        {t("step2.help")}
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
  const { t } = useTranslation("onboarding");
  const sorted = useMemo(
    () => (browsers ? [...browsers].sort((a, b) => a.display_name.localeCompare(b.display_name)) : null),
    [browsers],
  );
  return (
    <>
      <h1 style={{ margin: 0, fontSize: 22, fontWeight: 700 }}>
        {t("step3.title")}
      </h1>
      <p style={{ color: "var(--mac-fg-muted)", marginTop: 8 }}>
        {sorted === null
          ? t("step3.scanning")
          : t("step3.found", { count: sorted.length })}
      </p>
      <div style={{ marginTop: 20, width: "100%", maxWidth: 460 }}>
        {sorted?.map((b, i) => {
          const profiles = profilesByBrowser[b.id] ?? [];
          const profileLabel =
            profiles.length === 0
              ? t("step3.noProfiles")
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

function Step4Default({
  browsers,
  profilesByBrowser,
  value,
  onPick,
}: {
  browsers: InstalledBrowser[] | null;
  profilesByBrowser: Record<string, BrowserProfile[]>;
  value: BrowserTarget | null;
  onPick: (next: BrowserTarget) => void;
}) {
  const { t } = useTranslation("onboarding");
  const sorted = useMemo(
    () =>
      browsers
        ? [...browsers].sort((a, b) =>
            a.display_name.localeCompare(b.display_name),
          )
        : null,
    [browsers],
  );

  return (
    <>
      <h1 style={{ margin: 0, fontSize: 22, fontWeight: 700 }}>
        {t("step4.title")}
      </h1>
      <p
        style={{
          color: "var(--mac-fg-muted)",
          marginTop: 8,
          maxWidth: 420,
        }}
      >
        {t("step4.body")}
      </p>

      {sorted === null ? (
        <p
          className="mac-muted"
          style={{ marginTop: 18, fontSize: 12 }}
        >
          {t("step4.scanning")}
        </p>
      ) : sorted.length === 0 ? (
        <p
          className="mac-muted"
          style={{ marginTop: 18, fontSize: 12, maxWidth: 380 }}
        >
          {t("step4.noBrowsers")}
        </p>
      ) : (
        <div
          style={{
            marginTop: 18,
            width: "100%",
            maxWidth: 460,
            display: "grid",
            gridTemplateColumns: "repeat(auto-fill, minmax(150px, 1fr))",
            gap: 8,
          }}
        >
          {sorted.map((b) => {
            const selected = value?.browser === b.id;
            const profiles = profilesByBrowser[b.id] ?? [];
            const profileHint =
              profiles.length === 0
                ? null
                : profiles
                    .slice(0, 2)
                    .map((p) => p.display_name)
                    .join(" · ");
            return (
              <button
                key={b.id}
                type="button"
                onClick={() =>
                  onPick({
                    browser: b.id,
                    profile: null,
                    incognito: false,
                    new_window: false,
                  })
                }
                style={{
                  display: "flex",
                  alignItems: "center",
                  gap: 10,
                  padding: "10px 12px",
                  background: "var(--mac-card-fill)",
                  border: selected
                    ? "1px solid var(--mac-accent)"
                    : "0.5px solid var(--mac-border)",
                  boxShadow: selected
                    ? "0 0 0 2px color-mix(in srgb, var(--mac-accent) 22%, transparent)"
                    : "none",
                  borderRadius: 10,
                  cursor: "pointer",
                  textAlign: "left",
                  color: "inherit",
                  transition: "background 120ms ease, border-color 120ms ease",
                }}
              >
                <AppIcon
                  bundleId={b.platform_app_id ?? undefined}
                  appPath={appPathFromExecutable(b.executable)}
                  size={24}
                  alt={b.display_name}
                  className="shrink-0"
                />
                <div
                  style={{
                    flex: 1,
                    minWidth: 0,
                  }}
                >
                  <div
                    style={{
                      fontWeight: 600,
                      fontSize: 12.5,
                      overflow: "hidden",
                      textOverflow: "ellipsis",
                      whiteSpace: "nowrap",
                    }}
                  >
                    {b.display_name}
                  </div>
                  {profileHint && (
                    <div
                      style={{
                        fontSize: 11,
                        color: "var(--mac-fg-muted)",
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                    >
                      {profileHint}
                    </div>
                  )}
                </div>
                {selected && (
                  <Check
                    size={14}
                    strokeWidth={2.5}
                    style={{ color: "var(--mac-accent)" }}
                  />
                )}
              </button>
            );
          })}
        </div>
      )}

      <p
        style={{
          color: "var(--mac-fg-tertiary)",
          fontSize: 11,
          marginTop: 14,
          maxWidth: 420,
        }}
      >
        {t("step4.help")}
      </p>
    </>
  );
}

function Step5Templates({
  templates,
  setTemplates,
  browsers,
}: {
  templates: TemplateChoice[];
  setTemplates: (next: TemplateChoice[]) => void;
  browsers: InstalledBrowser[] | null;
}) {
  const { t } = useTranslation("onboarding");
  const toggle = (id: string) =>
    setTemplates(
      templates.map((t) => (t.id === id ? { ...t, enabled: !t.enabled } : t)),
    );

  // Hide templates whose action targets a browser the user doesn't have.
  // We surface the same set `finish()` will actually write so the on-screen
  // list matches what gets created — no toggling something on only to
  // discover later that it was silently dropped.
  const installed = browsers ?? [];
  const visible = templates
    .map((tpl) => ({ tpl, resolved: resolveTemplate(tpl, installed) }))
    .filter(
      (entry): entry is { tpl: TemplateChoice; resolved: ResolvedTemplate } =>
        entry.resolved !== null,
    );

  return (
    <>
      <h1 style={{ margin: 0, fontSize: 22, fontWeight: 700 }}>
        {t("step5.title")}
      </h1>
      <p
        style={{
          color: "var(--mac-fg-muted)",
          marginTop: 8,
          maxWidth: 380,
        }}
      >
        {t("step5.body")}
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
        {visible.map(({ tpl, resolved }) => {
          const interp =
            resolved.kind === "open"
              ? { browser: resolved.displayName, profile: tpl.profile ?? "" }
              : {};
          return (
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
                <div style={{ fontWeight: 600, fontSize: 13 }}>
                  {t(tpl.labelKey, interp)}
                </div>
                <div style={{ fontSize: 11.5, color: "var(--mac-fg-muted)" }}>
                  {t(tpl.detailKey)}
                </div>
              </div>
              <button
                type="button"
                className={"mac-switch accent" + (tpl.enabled ? " on" : "")}
                aria-pressed={tpl.enabled}
                onClick={() => toggle(tpl.id)}
              />
            </div>
          );
        })}
        {visible.length === 0 && browsers !== null && (
          <p
            className="mac-muted"
            style={{ fontSize: 12, padding: "16px 0" }}
          >
            {t("step5.noTemplates")}
          </p>
        )}
      </div>
      <p
        style={{
          color: "var(--mac-fg-tertiary)",
          fontSize: 11,
          marginTop: 14,
        }}
      >
        {t("step5.help")}
      </p>
    </>
  );
}
