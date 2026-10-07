/**
 * Getting a chart out of the page: a CSV of the numbers behind it and a PNG of the picture.
 *
 * The CSV is built from the engine's full-resolution series, never from what is drawn (which is
 * decimated), so a reader who recomputes a statistic from it gets the page's number. The PNG is the
 * chart's own canvas with a title and the run's identity written above it, on the theme's panel
 * colour, so a figure pasted into a report says what it is.
 */

/** Save `blob` as `name` through the browser's download. */
export function download(name: string, blob: Blob): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.rel = "noopener";
  document.body.appendChild(a);
  a.click();
  a.remove();
  // Revoked after the click has been handled, or some browsers download nothing.
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

export function downloadCsv(name: string, csv: string): void {
  download(name, new Blob([csv], { type: "text/csv;charset=utf-8" }));
}

/** A file name from parts: letters, digits, dot, dash and underscore only. */
export function fileName(parts: readonly string[], ext: string): string {
  const stem = parts
    .filter((p) => p !== "")
    .join("_")
    .replace(/[^A-Za-z0-9._-]+/g, "-")
    .replace(/-+/g, "-")
    .slice(0, 120);
  return `${stem === "" ? "metric" : stem}.${ext}`;
}

function cssVar(name: string, fallback: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return v === "" ? fallback : v;
}

/**
 * The chart canvas as a PNG, with `title` and `subtitle` above it and `legend` (label and colour
 * per series) under the title. Resolves to `null` when the browser cannot encode one.
 */
export async function chartPng(
  canvas: HTMLCanvasElement,
  title: string,
  subtitle: string,
  legend: readonly { readonly label: string; readonly colour: string }[],
): Promise<Blob | null> {
  const dpr = canvas.width / Math.max(1, canvas.clientWidth || canvas.width);
  const head = Math.round(58 * dpr);
  const pad = Math.round(12 * dpr);
  const out = document.createElement("canvas");
  out.width = canvas.width + 2 * pad;
  out.height = canvas.height + head + pad;
  const ctx = out.getContext("2d");
  if (ctx === null) return null;
  ctx.fillStyle = cssVar("--bg-panel", "#111820");
  ctx.fillRect(0, 0, out.width, out.height);
  const sans = cssVar("--sans", "system-ui, sans-serif");
  ctx.fillStyle = cssVar("--text", "#dbe5ef");
  ctx.font = `600 ${Math.round(14 * dpr)}px ${sans}`;
  ctx.textBaseline = "top";
  ctx.fillText(title, pad, pad);
  ctx.fillStyle = cssVar("--text-dim", "#8fa3b8");
  ctx.font = `${Math.round(11 * dpr)}px ${sans}`;
  ctx.fillText(subtitle, pad, pad + Math.round(19 * dpr));
  let x = pad;
  const y = pad + Math.round(36 * dpr);
  for (const item of legend) {
    ctx.strokeStyle = item.colour;
    ctx.lineWidth = 2 * dpr;
    ctx.beginPath();
    ctx.moveTo(x, y + 6 * dpr);
    ctx.lineTo(x + 14 * dpr, y + 6 * dpr);
    ctx.stroke();
    ctx.fillStyle = cssVar("--text", "#dbe5ef");
    ctx.fillText(item.label, x + 18 * dpr, y);
    x += 18 * dpr + ctx.measureText(item.label).width + 16 * dpr;
  }
  ctx.drawImage(canvas, pad, head);
  return new Promise((resolve) => out.toBlob((b) => resolve(b), "image/png"));
}
