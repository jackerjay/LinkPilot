# M6 release runbook — v0.2.0 public release

> **Historical note:** this runbook was written for the v0.2.0 milestone.
> The reusable, per-release **Homebrew tap-publish procedure now lives in
> [`homebrew/README.md`](homebrew/README.md) → "Publishing a release to the
> tap"** — Phase 3 below just points there. Version numbers and the
> `claude/v0.2-dev` branch references below are historical.

Operational steps the maintainer runs to take v0.2 from
"feature-complete on `claude/v0.2-dev`" to "tagged + published".
Every step is destructive/public — done in this exact order so you
can stop after any phase without leaving partial state.

## Phase 0 — Prereqs (one-time)

### npm

> **If your default `npm` is hitting a private internal mirror** — the package itself carries
> `publishConfig.registry = "https://registry.npmjs.org/"` so the
> CI workflow + any local `npm publish --workspace` always targets
> the public registry regardless of your global `.npmrc`. But you
> still need an auth token for the public registry, separately from
> whatever internal token you have. Do that with `--registry`:
>
> ```sh
> # One-time login against public npmjs (not your internal mirror):
> npm login --registry=https://registry.npmjs.org/ --scope=@linkpilot
> # then `npm whoami --registry=https://registry.npmjs.org/`
> # should print your public npmjs username.
> ```

- [ ] `NPM_TOKEN` GitHub repo secret set (Settings → Secrets and
      variables → Actions). Scope: `@linkpilot` publish, from
      [npmjs.com/settings/<user>/tokens](https://www.npmjs.com/settings)
      (Granular access token → packages and scopes:
      `@linkpilot/*` → Read and write). Note this is a public-npmjs
      token, not your internal-mirror token.
- [ ] `@linkpilot` npm scope registered to the publishing account on
      the public registry: visit
      <https://www.npmjs.com/org/create> if your scope is for an org,
      or just `npm publish` once with a user-scoped name like
      `@<your-handle>/config` (cheaper for solo maintainers). Verify
      with:
      ```sh
      npm access list packages --registry=https://registry.npmjs.org/
      ```

### Homebrew

- [ ] `jackerjay/homebrew-linkpilot` GitHub repo created (empty).
      Will receive `Formula/linkpilot-cli.rb` and `Casks/linkpilot.rb`
      from `packaging/homebrew/` in this repo.

## Phase 1 — Prep the merge

- [ ] PR `claude/v0.2-dev` → `main`. CI green
      (`cargo fmt --check`, clippy `-D warnings`, full test suite,
      yarn frontend build).
- [ ] Final `M4.4` GUI manual checklist still ✓ on the candidate
      build (re-verify with `npx tauri dev` if doubt).
- [ ] Merge to `main`.

## Phase 2 — Tag v0.2.0

This fires both `release.yml` AND `npm-publish.yml`.

```sh
git checkout main && git pull
# Confirm version files are already at 0.2.0 (M6.E commit landed)
grep '^version' Cargo.toml
grep '"version"' apps/desktop/package.json apps/desktop/src-tauri/tauri.conf.json packages/config-dsl/package.json

git tag v0.2.0
git push origin v0.2.0
```

Watch the two workflow runs:
- [ ] `release.yml` — per-arch CLI + daemon + DMG, GitHub Release
      created as draft, then auto-published. Check the assets list
      contains, for EACH of `aarch64` and `x86_64`:
      `lpt-macos-<arch>` (+ `.tar.gz`),
      `linkpilot-daemon-macos-<arch>` (+ `.tar.gz`),
      `LinkPilot_<version>_<arch>.dmg`, plus one unified `checksums.txt`.
- [ ] `npm-publish.yml` — `@linkpilot/config@0.2.0` published.
      Confirm: `npm view @linkpilot/config@0.2.0`.

## Phase 3 — Update Homebrew tap

The durable, per-release tap-publish procedure now lives in
[`homebrew/README.md`](homebrew/README.md) → **"Publishing a release to the
tap"**: pull the six shas from `checksums.txt`, fill the two per-arch cask
shas + four formula shas, mirror the recipes into `jackerjay/homebrew-linkpilot`,
and verify with `brew install`. Follow it there. The tap repo needs no tag —
`brew update` picks up the next commit, so a bad sha is fixed by a follow-up
commit to the tap.

## Phase 4 — Announce

- [ ] Edit the auto-generated GitHub Release body — paste
      `docs/release-notes/v0.2.0.md` ABOVE the auto-generated commit
      list. Save.
- [ ] Update `README.md` `## Releases` to point at v0.2.0 + add a
      one-liner about Homebrew.
- [ ] Mark `docs/linkpilot-design-v0.2.md` with `status: shipped
      (2026-MM-DD)` at the top.
- [ ] Commit doc changes to main; tag `v0.2.0-docs` if you want a
      stable doc anchor (optional).

## Phase 5 — Tidy up

- [ ] Delete the `claude/v0.2-dev` branch (it's merged).
- [ ] Move PROGRESS.md to a milestone-archived form (e.g.
      `docs/milestones/v0.2-progress.md`) and start a fresh root-level
      PROGRESS.md for v0.3.
- [ ] Create the v0.3 milestone tracker per design §14.5.6.

## Rollback (if Phase 2 or 3 went sideways)

| Problem | Fix |
|---|---|
| `release.yml` failed mid-build | The tag exists but no Release. Delete the tag (`git push --delete origin v0.2.0 && git tag -d v0.2.0`), fix forward, re-tag. |
| `npm-publish.yml` failed but `release.yml` succeeded | Re-run the failed job from the Actions UI. If still broken, fix forward and publish a 0.2.1 patch (npm doesn't permit republishing the same version even after `npm unpublish`). |
| Homebrew formula serves a broken bottle | Push a hotfix commit to `jackerjay/homebrew-linkpilot` with corrected sha256s. No tag is needed for tap repos — `brew update` picks up the next commit. |
