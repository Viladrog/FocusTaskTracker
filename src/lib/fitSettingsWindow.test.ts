import { describe, expect, it } from "vitest";
import { measureSettingsContentWidth } from "./fitSettingsWindow";

describe("measureSettingsContentWidth", () => {
  it("uses widest field row plus padding", () => {
    const root = document.createElement("div");
    root.innerHTML = `
      <h1 class="settings-title">Настройки</h1>
      <label class="settings-field"><span class="settings-field-label">Короткая</span></label>
      <label class="settings-field"><span class="settings-field-label">Интервал удаления выполненных (ч)</span></label>
    `;
    document.body.appendChild(root);

    const title = root.querySelector<HTMLElement>(".settings-title")!;
    const fields = root.querySelectorAll<HTMLElement>(".settings-field");
    Object.defineProperty(title, "offsetWidth", { value: 120 });
    Object.defineProperty(fields[0], "offsetWidth", { value: 300 });
    Object.defineProperty(fields[1], "offsetWidth", { value: 420 });

    expect(measureSettingsContentWidth(root)).toBe(460);

    root.remove();
  });
});
