const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "OS"]);

const NAMED_KEYS: Record<string, string> = {
  " ": "space",
  Space: "space",
  ArrowUp: "arrowup",
  ArrowDown: "arrowdown",
  ArrowLeft: "arrowleft",
  ArrowRight: "arrowright",
  Escape: "escape",
  Enter: "enter",
  Tab: "tab",
  Backspace: "backspace",
  Delete: "delete",
  Home: "home",
  End: "end",
  PageUp: "pageup",
  PageDown: "pagedown",
};

const NAMED_CODES: Record<string, string> = {
  Space: "space",
  ArrowUp: "arrowup",
  ArrowDown: "arrowdown",
  ArrowLeft: "arrowleft",
  ArrowRight: "arrowright",
  Escape: "escape",
  Enter: "enter",
  Tab: "tab",
  Backspace: "backspace",
  Delete: "delete",
  Home: "home",
  End: "end",
  PageUp: "pageup",
  PageDown: "pagedown",
};

function normalizeKey(key: string): string | null {
  if (NAMED_KEYS[key]) return NAMED_KEYS[key];
  if (key.length === 1 && /[a-z0-9]/i.test(key)) return key.toLowerCase();
  if (/^F\d{1,2}$/i.test(key)) return key.toLowerCase();
  return null;
}

function normalizeCode(code: string): string | null {
  if (NAMED_CODES[code]) return NAMED_CODES[code];
  if (code.startsWith("Key")) return code.slice(3).toLowerCase();
  if (code.startsWith("Digit")) return code.slice(5);
  if (code.startsWith("Numpad")) return `num${code.slice(6).toLowerCase()}`;
  return null;
}

export function buildShortcutFromEvent(e: KeyboardEvent): string | null {
  if (e.repeat) return null;
  if (MODIFIER_KEYS.has(e.key)) return null;

  // Physical key (e.code) — layout-independent; Russian "с" on KeyC → "c".
  const key = normalizeCode(e.code) ?? normalizeKey(e.key);
  if (!key) return null;

  const hasModifier = e.ctrlKey || e.altKey || e.shiftKey || e.metaKey;
  if (!hasModifier) return null;

  const parts: string[] = [];
  if (e.ctrlKey) parts.push("ctrl");
  if (e.altKey) parts.push("alt");
  if (e.shiftKey) parts.push("shift");
  if (e.metaKey) parts.push("super");
  parts.push(key);

  return parts.join("+");
}
