export type DeleteConfirmPlacement = "left" | "right";

export type DeleteConfirmPosition = {
  top: number;
  left: number;
  placement: DeleteConfirmPlacement;
  tailTop: number;
};

export function getDeleteConfirmPosition(
  anchor: Pick<DOMRect, "left" | "right" | "top" | "bottom" | "height">,
  width: number,
  height: number,
  viewportWidth: number,
  viewportHeight: number,
): DeleteConfirmPosition {
  const margin = 8;
  const gap = 8;
  const tailInset = 14;

  let placement: DeleteConfirmPlacement = "left";
  let left = anchor.left - width - gap;
  let top = anchor.bottom - height;

  if (left < margin) {
    placement = "right";
    left = anchor.right + gap;
  }

  left = Math.max(margin, Math.min(left, viewportWidth - width - margin));
  top = Math.max(margin, Math.min(top, viewportHeight - height - margin));

  const anchorCenterY = anchor.top + anchor.height / 2;
  const tailTop = Math.max(
    tailInset,
    Math.min(anchorCenterY - top, height - tailInset),
  );

  return { top, left, placement, tailTop };
}
