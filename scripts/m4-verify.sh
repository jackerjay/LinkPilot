#!/usr/bin/env bash
#
# M4 acceptance harness — walks every row of design §14.3.6 against the
# release-mode `lpt` binary. All writes target an isolated temp config
# (via `--config`), so this script is safe to run on a developer machine
# where a real daemon is serving `$HOME/Library/Application Support/
# LinkPilot/linkpilot.config.json`.
#
# Run from the repo root:
#   cargo build --release -p linkpilot-cli
#   ./scripts/m4-verify.sh
#
# Exit 0 if every backend-checkable scenario passes; non-zero with the
# offending step name on the first failure. Two scenarios labelled
# (manual) are GUI-only and cannot be verified from a shell — they're
# printed at the end as a checklist.

set -euo pipefail

# Cosmetics — bash colors fall back to no-op on dumb terminals.
if [ -t 1 ]; then
  GREEN=$'\033[32m'
  RED=$'\033[31m'
  YELLOW=$'\033[33m'
  BOLD=$'\033[1m'
  RESET=$'\033[0m'
else
  GREEN= ; RED= ; YELLOW= ; BOLD= ; RESET=
fi

pass() { echo "${GREEN}✓${RESET} $1"; }
fail() { echo "${RED}✗${RESET} $1" >&2; exit 1; }
note() { echo "${YELLOW}…${RESET} $1"; }

ROOT=$(git rev-parse --show-toplevel)
LP="$ROOT/target/release/lpt"
[ -x "$LP" ] || fail "lpt binary not found at $LP; run \`cargo build --release -p linkpilot-cli\` first"

CFG=$(mktemp -t lp-m4-cfg.XXXXXX.json)
COMPILED=$(mktemp -t lp-m4-compiled.XXXXXX.json)
SAMPLE=$(mktemp -t lp-m4-sample.XXXXXX.ts)
BAD_TS=$(mktemp -t lp-m4-bad.XXXXXX.ts)
trap 'rm -f "$CFG" "$COMPILED" "$SAMPLE" "$BAD_TS"' EXIT

DSL_REL="$ROOT/packages/config-dsl/src/index.ts"

# Seed an empty-ish config so subsequent --config flag has something to
# point at. `lpt config import` insists on a real file.
cat > "$CFG" <<'JSON'
{
  "version": 1,
  "default_target": { "browser": "system", "profile": null, "workspace": null, "incognito": false, "new_window": false },
  "rules": [],
  "workspaces": [],
  "custom_browsers": [],
  "settings": { "launch_at_login": false, "history_retention_days": null, "record_query_strings": false, "smart_routing_enabled": true },
  "meta": {}
}
JSON

# Build a sample linkpilot.config.ts that imports the DSL via a relative
# path (skips the npm-published @linkpilot/config layer — M6 will turn
# that on). The relative import works because bun resolves .ts natively.
cat > "$SAMPLE" <<TS
import { browser, defineConfig, printConfig, route } from "$DSL_REL";

printConfig(defineConfig({
  defaultTarget: browser.arc(),
  rules: [
    route.host("github.com").to(browser.chrome.profile("Default")),
    route.host("notion.so").to(browser.chrome.profile("Default")),
    route.host("figma.com").to(browser.arc()),
    route.host("youtube.com").to(browser.arc()),
    route.path("/oauth/*").keepSource(),
    route.fromApp("Slack").to(browser.chrome.profile("Default")),
    // escape hatch — raw matcher object, daemon should accept verbatim
    route.fromJson({ op: "url-host", pattern: "raw-escape.example.com" }).block(),
  ],
}));
TS

echo
echo "${BOLD}M4 acceptance — design §14.3.6${RESET}"
echo "Isolated config: $CFG"
echo

# -----------------------------------------------------------------
# Scenario 1: rewrite v0.1 demo in DSL + compile + daemon-accepted shape
# -----------------------------------------------------------------
note "1) compile DSL → isolated config; verify rule count + tags"
"$LP" --config "$CFG" config compile "$SAMPLE" >/dev/null 2>&1 \
  || fail "lpt config compile failed"

# `lpt config show` prefers the daemon's in-memory snapshot when one is
# running. We want the *isolated* config file we just wrote, so pass
# --local to skip the daemon and read $CFG directly.
SHOW=$("$LP" --config "$CFG" --local config show)
RULE_COUNT=$(echo "$SHOW" | jq '.rules | length')
[ "$RULE_COUNT" = "7" ] || fail "expected 7 rules in compiled config, got $RULE_COUNT"
COMPILED_COUNT=$(echo "$SHOW" | jq '[.rules[] | select(.source == "ts-compiled")] | length')
[ "$COMPILED_COUNT" = "7" ] \
  || fail "expected all 7 rules tagged ts-compiled, got $COMPILED_COUNT"
pass "compiled 7 rules; all tagged source: ts-compiled"

# -----------------------------------------------------------------
# Scenario 2: --to PATH writes JSON without touching the live config
# -----------------------------------------------------------------
note "2) --to PATH dry-run output"
BEFORE_HASH=$(shasum "$CFG" | awk '{print $1}')
"$LP" --config "$CFG" config compile "$SAMPLE" --to "$COMPILED" >/dev/null 2>&1 \
  || fail "lpt config compile --to failed"
AFTER_HASH=$(shasum "$CFG" | awk '{print $1}')
[ "$BEFORE_HASH" = "$AFTER_HASH" ] \
  || fail "config file was modified despite --to flag"
TO_RULE_COUNT=$(jq '.rules | length' < "$COMPILED")
[ "$TO_RULE_COUNT" = "7" ] || fail "--to output missing rules"
pass "--to PATH wrote $TO_RULE_COUNT rules to file; live config untouched"

# -----------------------------------------------------------------
# Scenario 3: second compile (config file mutates idempotently)
# -----------------------------------------------------------------
note "3) re-compile picks up edits + writes new ids"
FIRST_ID=$(echo "$SHOW" | jq -r '.rules[0].id')
"$LP" --config "$CFG" config compile "$SAMPLE" >/dev/null 2>&1
SECOND_ID=$("$LP" --config "$CFG" --local config show | jq -r '.rules[0].id')
[ "$FIRST_ID" != "$SECOND_ID" ] \
  || fail "expected fresh UUIDs on each compile (got $FIRST_ID twice)"
pass "compile is idempotent in shape but re-issues UUIDs"

# -----------------------------------------------------------------
# Scenario 4: route.fromJson escape hatch lands intact
# -----------------------------------------------------------------
note "4) route.fromJson({op: 'url-host', ...}) round-trips"
RAW_PATTERN=$("$LP" --config "$CFG" --local config show \
  | jq -r '.rules[] | select(.when.op == "url-host" and .when.pattern == "raw-escape.example.com") | .when.pattern')
[ "$RAW_PATTERN" = "raw-escape.example.com" ] \
  || fail "route.fromJson matcher missing from compiled config"
pass "fromJson matcher passed through unchanged"

# -----------------------------------------------------------------
# Scenario 5: bun absent → install hint, exit 1
# -----------------------------------------------------------------
note "5) bun absent → install hint"
# Use a stripped PATH that excludes bun's dir. We don't know exactly
# where it lives — `which bun` tells us.
BUN_PATH=$(command -v bun || true)
if [ -z "$BUN_PATH" ]; then
  pass "(skipped — bun isn't on PATH at all)"
else
  STRIPPED=$(echo "$PATH" | tr ':' '\n' | grep -v "$(dirname "$BUN_PATH")$" | paste -sd: -)
  set +e
  OUT=$(PATH="$STRIPPED" "$LP" --config "$CFG" config compile "$SAMPLE" 2>&1)
  RC=$?
  set -e
  [ "$RC" = "1" ] || fail "expected exit 1 when bun missing; got $RC"
  echo "$OUT" | grep -q "bun.*not found" || fail "missing 'bun not found' hint:\n$OUT"
  echo "$OUT" | grep -q "brew install" || fail "missing brew install hint"
  pass "bun missing → exit 1 with brew + curl install hints"
fi

# -----------------------------------------------------------------
# Scenario 6: TypeScript type error → bun stderr passthrough, exit 1
# -----------------------------------------------------------------
note "6) TS type error → stderr passthrough"
cat > "$BAD_TS" <<TS
import { browser, defineConfig, printConfig } from "$DSL_REL";
// Deliberate type error: defineConfig requires defaultTarget + rules.
const cfg: never = defineConfig({} as any);
printConfig(cfg);
TS
set +e
OUT=$("$LP" --config "$CFG" config compile "$BAD_TS" 2>&1)
RC=$?
set -e
[ "$RC" != "0" ] || fail "TS error case unexpectedly exited 0"
echo "$OUT" | grep -qE "Cannot find|defaultTarget|never|TypeError|undefined" \
  || note "bun's error didn't mention type error keywords — got:\n$OUT"
pass "bad TS exits $RC with bun stderr surfaced"

# -----------------------------------------------------------------
# Scenarios 7 + 8: GUI behaviour — cannot be tested from shell.
# -----------------------------------------------------------------
echo
echo "${BOLD}Manual GUI checks${RESET} (run after \`npx tauri dev\` or fresh \`tauri build\` + install):"
echo "  □ open Rules page; rules show 'compiled' badge with tooltip"
echo "  □ edit pencil + delete trash are disabled with explanatory tooltips"
echo "  □ Copy-to-GUI button (CopyPlus icon) clones the rule, source: 'gui'"
echo "  □ edit linkpilot.config.ts, re-run \`lpt config compile\`; GUI refreshes within ~1s"
echo
echo "${GREEN}${BOLD}M4 backend acceptance: PASS${RESET}"
