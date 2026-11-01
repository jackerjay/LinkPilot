import type { UpdateCheckResult, UpdateDownload } from "./ipc";
import type { UpdateCheckState } from "./update";

interface Dependencies {
  currentVersion: () => Promise<string>;
  check: (version: string) => Promise<UpdateCheckResult>;
  download: (result: UpdateCheckResult) => Promise<UpdateDownload>;
  install: (version: string, automatic: boolean) => Promise<boolean>;
  enabled: () => Promise<boolean>;
  state: (state: UpdateCheckState) => void;
  checksumMissing: () => string;
}

/** One update at a time, including manual checks during an automatic download. */
export function createUpdateRunner(deps: Dependencies) {
  let running: Promise<void> | undefined;
  return (automatic = false, install = false): Promise<void> => {
    if (running) return running;
    running = (async () => {
      let result: UpdateCheckResult | undefined;
      let download: UpdateDownload | undefined;
      try {
        if (automatic && !(await deps.enabled())) return;
        deps.state({ status: "checking" });
        result = await deps.check(await deps.currentVersion());
        if (!result.available) {
          deps.state({ status: "up-to-date", result });
          return;
        }
        if (automatic && result.installError) throw new Error(result.installError);
        if (!result.asset.sha256) throw new Error(deps.checksumMissing());
        deps.state({ status: "downloading", result });
        download = await deps.download(result);
        if ((automatic && await deps.enabled()) || install) {
          deps.state({ status: "installing", result });
          if (await deps.install(result.latestVersion, automatic)) return;
        }
        deps.state({ status: "downloaded", result, download });
      } catch (error) {
        deps.state({ status: "error", error: String(error), checkedAt: Date.now(), result, download });
      }
    })().finally(() => { running = undefined; });
    return running;
  };
}
