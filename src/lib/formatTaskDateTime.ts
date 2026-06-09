function pad2(n: number): string {
  return String(n).padStart(2, "0");
}

/** SQLite UTC `"YYYY-MM-DD HH:MM:SS"` → local `DD.MM.YYYY HH:mm`. */
export function formatTaskDateTime(value: string): string {
  const d = new Date(value.replace(" ", "T") + "Z");
  return `${pad2(d.getDate())}.${pad2(d.getMonth() + 1)}.${d.getFullYear()} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}
