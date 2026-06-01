import { describe, expect, it } from "vitest";
import { getDeleteConfirmPosition } from "./deleteConfirmPosition";

function rect(
  left: number,
  top: number,
  right: number,
  bottom: number,
): DOMRect {
  return {
    left,
    top,
    right,
    bottom,
    width: right - left,
    height: bottom - top,
    x: left,
    y: top,
    toJSON: () => ({}),
  } as DOMRect;
}

describe("getDeleteConfirmPosition", () => {
  it("places popover to the left when there is room", () => {
    const pos = getDeleteConfirmPosition(
      rect(200, 100, 240, 130),
      168,
      72,
      800,
      600,
    );
    expect(pos.placement).toBe("left");
    expect(pos.left).toBe(200 - 168 - 8);
  });

  it("flips to the right near the left viewport edge", () => {
    const pos = getDeleteConfirmPosition(
      rect(10, 100, 40, 130),
      168,
      72,
      800,
      600,
    );
    expect(pos.placement).toBe("right");
    expect(pos.left).toBe(40 + 8);
  });

  it("clamps within viewport margins", () => {
    const pos = getDeleteConfirmPosition(
      rect(790, 580, 795, 590),
      168,
      72,
      800,
      600,
    );
    expect(pos.left).toBeLessThanOrEqual(800 - 168 - 8);
    expect(pos.top).toBeGreaterThanOrEqual(8);
  });
});
