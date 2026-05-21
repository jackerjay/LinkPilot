# Contributing to LinkPilot

Thanks for helping improve LinkPilot.

## Development setup

Requirements:

- Rust 1.80+
- Node.js 20+
- npm
- macOS for the full desktop app build

Common checks:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --exclude linkpilot-desktop --all-targets -- -D warnings
cargo test --workspace --exclude linkpilot-desktop
cargo check --workspace --exclude linkpilot-desktop

cd apps/desktop
npm ci
npm run build
```

Run the desktop app on macOS:

```sh
cd apps/desktop
npm ci
npm run tauri -- dev
```

## Pull requests

Before opening a PR:

1. Keep changes focused on one problem.
2. Add or update tests for behavior changes.
3. Run the relevant checks above.
4. Update README or docs when user-facing behavior changes.

## Release process

Maintainers publish releases by pushing a semver tag:

```sh
git tag v0.1.0
git push origin v0.1.0
```

The release workflow builds artifacts and creates a GitHub Release from the tag.

## License

Unless explicitly stated otherwise, contributions are licensed under the MIT
License.
