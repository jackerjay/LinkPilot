// Update check facade. The actual fetches (release metadata + checksums.txt)
// run in Rust via `update_fetch_metadata`. Doing the fetch from the
// renderer hit CORS in dev (origin http://localhost:5173) because
// `release-assets.githubusercontent.com` returns a 302 without ACAO; the
// native side uses /usr/bin/curl and so never enters the browser's
// same-origin model.

import { ipc } from "@/lib/ipc";
import type {
  UpdateAssetMeta,
  UpdateCheckResult,
  UpdateDownload,
} from "@/lib/ipc";

export type { UpdateAssetMeta, UpdateCheckResult } from "@/lib/ipc";
// Back-compat alias for the type's old name.
export type UpdateAsset = UpdateAssetMeta;

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

export function checkForUpdates(
  currentVersion: string,
): Promise<UpdateCheckResult> {
  return ipc.updateFetchMetadata({ currentVersion });
}
