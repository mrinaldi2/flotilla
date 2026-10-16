import type { ReactElement } from "react";
import { encode } from "uqr";

/**
 * A QR code for `text`, drawn locally as one SVG path on a white tile so it
 * scans on the dark theme too (UX-043 §2). `border` is the quiet zone, in
 * modules, drawn inside the tile.
 */
export default function QrCode({
  text,
  label,
  size = 120,
  border = 2,
  className = "install-qr-code",
}: {
  readonly text: string;
  readonly label: string;
  readonly size?: number;
  readonly border?: number;
  readonly className?: string;
}): ReactElement {
  const { data } = encode(text, { ecc: "M", border });
  const cells = data.length;
  const path = data
    .flatMap((row, y) => row.map((dark, x) => (dark ? `M${x} ${y}h1v1h-1z` : "")))
    .join("");
  return (
    <svg
      className={className}
      role="img"
      aria-label={label}
      width={size}
      height={size}
      viewBox={`0 0 ${cells} ${cells}`}
      shapeRendering="crispEdges"
    >
      <rect width={cells} height={cells} fill="#fff" />
      <path d={path} fill="#000" />
    </svg>
  );
}
