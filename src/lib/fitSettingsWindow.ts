import { LogicalSize } from "@tauri-apps/api/dpi";
import { getCurrentWindow } from "@tauri-apps/api/window";

export const SETTINGS_WINDOW_HEIGHT = 520;

const HORIZONTAL_PADDING = 40;

let fitTimer: ReturnType<typeof setTimeout> | undefined;

export function measureSettingsContentWidth(contentEl: HTMLElement): number {
  const title = contentEl.querySelector<HTMLElement>(".settings-title");
  const fields = contentEl.querySelectorAll<HTMLElement>(".settings-field");

  let max = title?.offsetWidth ?? 0;
  fields.forEach((field) => {
    max = Math.max(max, field.offsetWidth);
  });

  return Math.max(Math.ceil(max + HORIZONTAL_PADDING), 280);
}

export function scheduleFitSettingsWindowWidth(
  contentEl: HTMLElement,
): void {
  if (fitTimer !== undefined) {
    clearTimeout(fitTimer);
  }
  fitTimer = setTimeout(() => {
    fitTimer = undefined;
    void fitSettingsWindowWidth(contentEl);
  }, 50);
}

export async function fitSettingsWindowWidth(
  contentEl: HTMLElement,
): Promise<void> {
  const win = getCurrentWindow();
  if (win.label !== "settings") return;

  try {
    const width = measureSettingsContentWidth(contentEl);
    const current = await win.innerSize();
    if (current.width === width) return;

    await win.setSize(new LogicalSize(width, SETTINGS_WINDOW_HEIGHT));
    await win.center();
  } catch {
    // Window may be hidden or permissions unavailable — ignore.
  }
}
