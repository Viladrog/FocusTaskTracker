import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const BACKEND_NOT_READY = /state not managed/i;
const MAX_ATTEMPTS = 30;
const RETRY_MS = 40;

function isBackendNotReadyError(error: unknown): boolean {
  return BACKEND_NOT_READY.test(String(error));
}

async function waitForBackendReady(): Promise<void> {
  try {
    if (await invoke<boolean>("backend_ready")) return;
  } catch {
    // Fall through to event listen + invoke retries.
  }

  await new Promise<void>((resolve) => {
    let settled = false;
    const finish = () => {
      if (settled) return;
      settled = true;
      clearInterval(poll);
      clearTimeout(timeout);
      void unlistenPromise.then((unlisten) => unlisten());
      resolve();
    };

    const unlistenPromise = listen("app-ready", finish);
    const poll = setInterval(() => {
      void invoke<boolean>("backend_ready")
        .then((ready) => {
          if (ready) finish();
        })
        .catch(() => {});
    }, RETRY_MS);
    const timeout = setTimeout(finish, MAX_ATTEMPTS * RETRY_MS);
  });
}

export async function invokeWhenBackendReady<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  await waitForBackendReady();
  for (let attempt = 0; attempt < MAX_ATTEMPTS; attempt++) {
    try {
      return await invoke<T>(cmd, args);
    } catch (error) {
      if (!isBackendNotReadyError(error) || attempt === MAX_ATTEMPTS - 1) {
        throw error;
      }
      await new Promise((r) => setTimeout(r, RETRY_MS));
    }
  }
  throw new Error(`invoke ${cmd} failed`);
}
