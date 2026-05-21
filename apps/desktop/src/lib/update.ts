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
}

interface LatestRelease {
  tagName: string;
  htmlUrl: string;
  asset: UpdateAsset;
  name: string | null;
  publishedAt: string | null;
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

  const release = parseLatestRelease(await response.json());
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

function parseLatestRelease(value: unknown): LatestRelease {
  if (!isRecord(value)) {
    throw new Error("GitHub Releases response was not an object");
  }

  const tagName = readString(value, "tag_name");
  const htmlUrl = readString(value, "html_url");
  if (!tagName || !htmlUrl) {
    throw new Error("GitHub Releases response is missing tag_name or html_url");
  }

  const assets = readArray(value, "assets");
  const asset = pickMacDmgAsset(assets);
  if (!asset) {
    throw new Error("Latest release does not include a macOS DMG asset");
  }

  return {
    tagName,
    htmlUrl,
    asset,
    name: readNullableString(value, "name"),
    publishedAt: readNullableString(value, "published_at"),
  };
}

function pickMacDmgAsset(values: unknown[]): UpdateAsset | null {
  const assets = values.flatMap((value) => {
    if (!isRecord(value)) return [];
    const name = readString(value, "name");
    const downloadUrl = readString(value, "browser_download_url");
    if (!name || !downloadUrl || !name.toLowerCase().endsWith(".dmg")) {
      return [];
    }
    return [
      {
        name,
        downloadUrl,
        size: readNumber(value, "size"),
      },
    ];
  });
  return (
    assets.find((asset) => /universal/i.test(asset.name)) ??
    assets.find((asset) => /aarch64|arm64/i.test(asset.name)) ??
    assets[0] ??
    null
  );
}

function normalizeVersion(version: string): string {
  const trimmed = version.trim();
  return trimmed.startsWith("v") || trimmed.startsWith("V")
    ? trimmed.slice(1)
    : trimmed;
}

function versionParts(version: string): number[] {
  return normalizeVersion(version)
    .split(/[+-]/, 1)[0]
    .split(".")
    .map((part) => {
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
