import { describe, expect, it } from "vitest";
import { buildShortcutFromEvent } from "./buildShortcutFromEvent";

function keyEvent(init: Partial<KeyboardEventInit> & { key: string }): KeyboardEvent {
  return new KeyboardEvent("keydown", {
    bubbles: true,
    cancelable: true,
    ...init,
  });
}

describe("buildShortcutFromEvent", () => {
  it("maps space character to ctrl+shift+space", () => {
    const shortcut = buildShortcutFromEvent(
      keyEvent({
        key: " ",
        code: "Space",
        ctrlKey: true,
        shiftKey: true,
      }),
    );
    expect(shortcut).toBe("ctrl+shift+space");
  });

  it("maps Space key name via code fallback", () => {
    const shortcut = buildShortcutFromEvent(
      keyEvent({
        key: "Space",
        code: "Space",
        ctrlKey: true,
        shiftKey: true,
      }),
    );
    expect(shortcut).toBe("ctrl+shift+space");
  });

  it("ignores modifier-only keydown", () => {
    expect(
      buildShortcutFromEvent(keyEvent({ key: "Control", code: "ControlLeft", ctrlKey: true })),
    ).toBeNull();
  });

  it("ignores key without modifiers", () => {
    expect(buildShortcutFromEvent(keyEvent({ key: "a", code: "KeyA" }))).toBeNull();
  });

  it("uses physical key code when Russian layout produces Cyrillic letter", () => {
    const shortcut = buildShortcutFromEvent(
      keyEvent({
        key: "с",
        code: "KeyC",
        ctrlKey: true,
        shiftKey: true,
      }),
    );
    expect(shortcut).toBe("ctrl+shift+c");
  });
});
