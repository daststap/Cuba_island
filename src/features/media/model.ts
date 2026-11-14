import type { MediaState } from "../../ipc/commands";

/**
 * Whether a media update deserves a peek: a different track appeared.
 * The very first state after startup never peeks.
 */
export function shouldPeek(
  prev: MediaState | null,
  next: MediaState | null,
  isInitial: boolean,
): boolean {
  if (isInitial || !next) return false;
  return prev?.trackId !== next.trackId;
}

/** Position interpolated from the last report, clamped to the track length. */
export function currentPositionMs(media: MediaState, nowMs: number): number {
  const elapsed = media.isPlaying ? Math.max(0, nowMs - media.positionUpdatedAt) : 0;
  const position = media.positionMs + elapsed;
  return media.durationMs > 0 ? Math.min(position, media.durationMs) : position;
}

/** 0:07, 3:42, 1:02:05 */
export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${String(h)}:${String(m).padStart(2, "0")}:${s}` : `${String(m)}:${s}`;
}

export type Rgb = readonly [number, number, number];

/**
 * Dominant colour of RGBA pixels (a 16×16 downscale of the cover): an average
 * weighted towards saturated, mid-brightness pixels so the glow is colourful
 * rather than the grey average. `null` when the image is essentially greyscale.
 */
export function dominantColor(pixels: ArrayLike<number>): Rgb | null {
  let r = 0;
  let g = 0;
  let b = 0;
  let total = 0;
  for (let i = 0; i + 3 < pixels.length; i += 4) {
    const pr = pixels[i] ?? 0;
    const pg = pixels[i + 1] ?? 0;
    const pb = pixels[i + 2] ?? 0;
    const alpha = (pixels[i + 3] ?? 255) / 255;
    const max = Math.max(pr, pg, pb);
    const min = Math.min(pr, pg, pb);
    const saturation = max === 0 ? 0 : (max - min) / max;
    const brightness = max / 255;
    // Favour saturated pixels; fade out the near-black and near-white ones.
    const weight = alpha * saturation * saturation * Math.min(brightness * 3, 1);
    r += pr * weight;
    g += pg * weight;
    b += pb * weight;
    total += weight;
  }
  if (total < 0.5) return null;
  return [Math.round(r / total), Math.round(g / total), Math.round(b / total)];
}
