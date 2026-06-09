import { describe, expect, it } from "vitest";
import { formatHotkeyLabel } from "./formatHotkeyLabel";

describe("formatHotkeyLabel", () => {
  it("formats default hotkey", () => {
    expect(formatHotkeyLabel("ctrl+shift+space")).toBe("Ctrl+Shift+Space");
  });

  it("formats single modifier and key", () => {
    expect(formatHotkeyLabel("alt+f4")).toBe("Alt+F4");
  });
});
