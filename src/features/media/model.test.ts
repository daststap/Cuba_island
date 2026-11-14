import { describe, expect, it } from "vitest";
import type { MediaState } from "../../ipc/commands";
import { currentPositionMs, dominantColor, formatDuration, shouldPeek } from "./model";

const track = (overrides: Partial<MediaState> = {}): MediaState => ({
  trackId: "a",
  title: "Song",
  artist: "Artist",
  album: "Album",
  appId: "Spotify.exe",
  isPlaying: true,
  positionMs: 10_000,
  durationMs: 200_000,
  positionUpdatedAt: 1_000_000,
  cover: null,
  ...overrides,
});

describe("shouldPeek", () => {
  it("peeks when the track changes or music appears", () => {
    expect(shouldPeek(track(), track({ trackId: "b" }), false)).toBe(true);
    expect(shouldPeek(null, track(), false)).toBe(true);
  });

  it("does not peek on startup, on progress updates or when music stops", () => {
    expect(shouldPeek(null, track(), true)).toBe(false);
    expect(shouldPeek(track(), track({ positionMs: 20_000, isPlaying: false }), false)).toBe(false);
    expect(shouldPeek(track(), null, false)).toBe(false);
  });
});

describe("currentPositionMs", () => {
  it("interpolates while playing and clamps to the duration", () => {
    expect(currentPositionMs(track(), 1_002_500)).toBe(12_500);
    expect(currentPositionMs(track(), 1_000_000 + 500_000)).toBe(200_000);
  });

  it("does not advance while paused or before the report time", () => {
    expect(currentPositionMs(track({ isPlaying: false }), 1_009_000)).toBe(10_000);
    expect(currentPositionMs(track(), 999_000)).toBe(10_000);
  });
});

describe("formatDuration", () => {
  it("formats minutes, seconds and hours", () => {
    expect(formatDuration(7_000)).toBe("0:07");
    expect(formatDuration(222_400)).toBe("3:42");
    expect(formatDuration(3_725_000)).toBe("1:02:05");
    expect(formatDuration(-5)).toBe("0:00");
  });
});

describe("dominantColor", () => {
  const pixels = (...rgba: number[][]) => rgba.flat();

  it("prefers saturated pixels over grey ones", () => {
    const color = dominantColor(
      pixels([200, 30, 30, 255], [128, 128, 128, 255], [250, 250, 250, 255], [5, 5, 5, 255]),
    );
    expect(color).toEqual([200, 30, 30]);
  });

  it("returns null for greyscale images", () => {
    expect(dominantColor(pixels([128, 128, 128, 255], [20, 20, 20, 255]))).toBeNull();
  });
});
