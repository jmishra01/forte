import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/**
 * A newer signed release, or null if there is none, the updater isn't set up
 * (see docs/releasing.md), or the check failed (offline, for example).
 */
export async function findUpdate(): Promise<Update | null> {
  try {
    return await check();
  } catch {
    return null;
  }
}

export async function installUpdate(update: Update) {
  await update.downloadAndInstall();
  await relaunch();
}
