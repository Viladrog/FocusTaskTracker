export function formatHotkeyLabel(hotkey: string): string {
  return hotkey
    .split("+")
    .filter((part) => part.length > 0)
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join("+");
}
