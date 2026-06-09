import { describe, expect, it } from "vitest";
import { formatTaskDateTime } from "./formatTaskDateTime";

describe("formatTaskDateTime", () => {
  it("formats SQLite UTC datetime as DD.MM.YYYY HH:mm in local time", () => {
    const utc = "2026-03-09 13:33:00";
    const result = formatTaskDateTime(utc);
    const expected = new Date("2026-03-09T13:33:00Z");
    const pad = (n: number) => String(n).padStart(2, "0");
    const exp = `${pad(expected.getDate())}.${pad(expected.getMonth() + 1)}.${expected.getFullYear()} ${pad(expected.getHours())}:${pad(expected.getMinutes())}`;
    expect(result).toBe(exp);
  });
});
