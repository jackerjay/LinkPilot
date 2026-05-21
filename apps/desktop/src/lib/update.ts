import type { UpdateDownload } from "@/lib/ipc";

const LATEST_RELEASE_API =
  "https://api.github.com/repos/jackerjay/LinkPilot/releases/latest";

export type UpdateCheckState =
  | { status: "idle" }
  | { status: "checking" }
  | { status: "downloading"; result: UpdateCheckResult }
  | {
      status: "downloaded";
      result: UpdateCheckResult;
      download: UpdateDownload;
    }
  | { status: "up-to-date"; result: UpdateCheckResult }
  | {
      status: "error";
      error: string;
      checkedAt: number;
      result?: UpdateCheckResult;
    };

export interface UpdateCheckResult {
  currentVersion: string;
  latestVersion: string;
  releaseUrl: string;
  asset: UpdateAsset;
  releaseName: string | null;
  publishedAt: string | null;
  available: boolean;
  checkedAt: number;
}

export interface UpdateAsset {
  name: string;
  downloadUrl: string;
  size: number | null;
  /** Lowercase hex SHA-256 sourced from the release's `checksums.txt`.
   *  `null` if the release has no checksums file or doesn't list this
   *  asset — the native side then refuses to write the downloaded DMG,
   *  so unverified binaries never reach disk. */
  sha256: string | null;
}

interface LatestRelease {
  tagName: string;
  htmlUrl: string;
  asset: UpdateAsset;
  name: string | null;
  publishedAt: string | null;
}

interface ReleaseAssetMeta {
  name: string;
  downloadUrl: string;
  size: number | null;
}

export async function checkForUpdates(
  currentVersion: string,
): Promise<UpdateCheckResult> {
  const response = await fetch(LATEST_RELEASE_API, {
    headers: {
      Accept: "application/vnd.github+json",
    },
  });

  if (!response.ok) {
    throw new Error(`GitHub Releases returned HTTP ${response.status}`);
  }

  const release = await parseLatestRelease(await response.json());
  const latestVersion = normalizeVersion(release.tagName);
  const normalizedCurrent = normalizeVersion(currentVersion);

  return {
    currentVersion: normalizedCurrent,
    latestVersion,
    releaseUrl: release.htmlUrl,
    asset: release.asset,
    releaseName: release.name,
    publishedAt: release.publishedAt,
    available: compareVersions(latestVersion, normalizedCurrent) > 0,
    checkedAt: Date.now(),
  };
}

export function compareVersions(left: string, right: string): number {
  const a = versionParts(left);
  const b = versionParts(right);
  for (let i = 0; i < Math.max(a.length, b.length); i += 1) {
    const diff = (a[i] ?? 0) - (b[i] ?? 0);
    if (diff !== 0) return diff;
  }
  return 0;
}

async function parseLatestRelease(value: unknown): Promise<LatestRelease> {
  if (!isRecord(value)) {
    throw new Error("GitHub Releases response was not an object");
  }

  const tagName = readString(value, "tag_name");
  const htmlUrl = readString(value, "html_url");
  if (!tagName || !htmlUrl) {
    throw new Error("GitHub Releases response is missing tag_name or html_url");
  }

  const assets = collectAssets(readArray(value, "assets"));
  const dmg = pickMacDmgAsset(assets);
  if (!dmg) {
    throw new Error("Latest release does not include a macOS DMG asset");
  }

  // `checksums.txt` ships with every release from M6 onward. Fetching it is
  // best-effort; if we can't reach it OR it doesn't list our DMG, surface
  // null and let the daemon refuse the download. We do NOT silently fall
  // back to "no verification" — the whole point of this check is that the
  // unsigned DMG isn't auto-installed without a checksum match.
  const sha256 = await resolveAssetSha256(assets, dmg.name);

  return {
    tagName,
    htmlUrl,
    asset: { ...dmg, sha256 },
    name: readNullableString(value, "name"),
    publishedAt: readNullableString(value, "published_at"),
  };
}

function collectAssets(values: unknown[]): ReleaseAssetMeta[] {
  return values.flatMap((value) => {
    if (!isRecord(value)) return [];
    const name = readString(value, "name");
    const downloadUrl = readString(value, "browser_download_url");
    if (!name || !downloadUrl) return [];
    return [{ name, downloadUrl, size: readNumber(value, "size") }];
  });
}

function pickMacDmgAsset(assets: ReleaseAssetMeta[]): ReleaseAssetMeta | null {
  const dmgs = assets.filter((a) => a.name.toLowerCase().endsWith(".dmg"));
  return (
    dmgs.find((asset) => /universal/i.test(asset.name)) ??
    dmgs.find((asset) => /aarch64|arm64/i.test(asset.name)) ??
    dmgs[0] ??
    null
  );
}

async function resolveAssetSha256(
  assets: ReleaseAssetMeta[],
  dmgName: string,
): Promise<string | null> {
  const checksums = assets.find(
    (a) => a.name.toLowerCase() === "checksums.txt",
  );
  if (!checksums) return null;
  try {
    const res = await fetch(checksums.downloadUrl, {
      headers: { Accept: "text/plain" },
    });
    if (!res.ok) return null;
    return parseChecksum(await res.text(), dmgName);
  } catch {
    return null;
  }
}

/** Parse a `shasum -a 256`-style file: `<hex>  <path-or-name>\n`. We match
 *  the filename basename so a checksum line like `dist/Foo.dmg` still maps
 *  to a release asset named `Foo.dmg`. */
function parseChecksum(content: string, assetName: string): string | null {
  const target = assetName.toLowerCase();
  for (const rawLine of content.split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith("#")) continue;
    const match = line.match(/^([0-9a-fA-F]{64})\s+\*?(\S.*)$/);
    if (!match) continue;
    const [, hex, path] = match;
    const base = path.split("/").pop()?.toLowerCase() ?? "";
    if (base === target) return hex.toLowerCase();
  }
  return null;
}

function normalizeVersion(version: string): string {
  const trimmed = version.trim();
  return trimmed.startsWith("v") || trimmed.startsWith("V")
    ? trimmed.slice(1)
    : trimmed;
}

function versionParts(version: string): number[] {
  // Strip a `+build` / `-pre` suffix before splitting on dots. `split(sep, 1)`
  // returns at most one element — the slice before the first match — which
  // is the semver core. Subscript `[0]` is always defined for a non-empty
  // string, but use `?? ""` to satisfy strict null checks.
  const core = normalizeVersion(version).split(/[+-]/, 1)[0] ?? "";
  return core.split(".").map((part) => {
    const parsed = Number.parseInt(part, 10);
    return Number.isFinite(parsed) ? parsed : 0;
  });
}

function readString(record: Record<string, unknown>, key: string): string | null {
  const value = record[key];
  return typeof value === "string" && value.length > 0 ? value : null;
}

function readNullableString(
  record: Record<string, unknown>,
  key: string,
): string | null {
  const value = record[key];
  return typeof value === "string" ? value : null;
}

function readNumber(record: Record<string, unknown>, key: string): number | null {
  const value = record[key];
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function readArray(record: Record<string, unknown>, key: string): unknown[] {
  const value = record[key];
  return Array.isArray(value) ? value : [];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}
