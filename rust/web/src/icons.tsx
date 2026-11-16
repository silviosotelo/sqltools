// Minimal inline icon set for the dense toolbars. Stroke-only, 16x16.
import type { SVGProps } from "react";

const paths: Record<string, string> = {
  run: "M4 3 L13 8 L4 13 Z",
  stop: "M4.5 4.5 H11.5 V11.5 H4.5 Z",
  commit: "M3 8.5 L6.5 12 L13 4.5",
  rollback: "M6 4 L2 8 L6 12 M2 8 H10 A3 3 0 0 1 10 14 H7",
  format:
    "M6 2 C4 2 4.5 6 3 8 C4.5 10 4 14 6 14 M10 2 C12 2 11.5 6 13 8 C11.5 10 12 14 10 14",
  history: "M8 2.5 A5.5 5.5 0 1 1 3 5 M8 4.5 V8 L10.5 9.5",
  plan: "M3 4 H7 M5 8 H10 M7 12 H12",
  stats: "M4 13 V8 M8 13 V4 M12 13 V10",
  export: "M8 2 V10 M5 7 L8 10 L11 7 M3 12 H13",
  save: "M3 2 H11 L13 4 V13 H3 Z M5 2 V6 H10 V2 M5 13 V9 H11 V13",
  open: "M2 4 H7 L9 6 H14 V13 H2 Z",
  commands: "M4 4 H9 M4 8 H12 M4 12 H9 M13 3 V7 M11 5 H15",
  connect: "M3 8 H8 M8 5 V11 M10 5 H13 V11 H10",
  disconnect: "M3 8 H7 M9 5 V11 M11 5 H13 V11 H11 M2 14 L14 2",
  refresh: "M3 8 A5 5 0 0 1 12 5 M13 8 A5 5 0 0 1 4 11 M11 3 H13 V5 M5 13 H3 V11",
  database:
    "M8 2 C11 2 13 3 13 4 V12 C13 13 11 14 8 14 C5 14 3 13 3 12 V4 C3 3 5 2 8 2 M3 4 C3 5 5 6 8 6 C11 6 13 5 13 4 M3 8 C3 9 5 10 8 10 C11 10 13 9 13 8",
  search: "M7 3 A4 4 0 1 1 7 11 A4 4 0 1 1 7 3 M10 10 L13.5 13.5",
  table: "M2 3 H14 V13 H2 Z M2 7 H14 M2 10 H14 M6 3 V13 M10 3 V13",
  clear: "M4 4 L12 12 M12 4 L4 12",
  update: "M8 2 V9 M5 6 L8 9 L11 6 M3 12 H13",
  settings:
    "M8 5.5 A2.5 2.5 0 1 1 8 10.5 A2.5 2.5 0 1 1 8 5.5 M8 1.5 V3 M8 13 V14.5 M1.5 8 H3 M13 8 H14.5 M3.4 3.4 L4.5 4.5 M11.5 11.5 L12.6 12.6 M12.6 3.4 L11.5 4.5 M4.5 11.5 L3.4 12.6",
  close: "M4 4 L12 12 M12 4 L4 12",
};

export function Icon({
  name,
  size = 16,
  ...rest
}: { name: keyof typeof paths | string; size?: number } & SVGProps<SVGSVGElement>) {
  const path = paths[name] ?? paths.table;
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.4}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      {...rest}
    >
      <path d={path} />
    </svg>
  );
}
