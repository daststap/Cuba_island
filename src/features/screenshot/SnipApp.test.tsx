import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ru } from "../../i18n/ru";
import { packImage } from "./draw";
import { SnipApp } from "./SnipApp";

const WIDTH = 200;
const HEIGHT = 100;

let calls: { cmd: string; args: unknown }[] = [];

/** A 2D context that accepts every call; reads return blank pixels. */
function fakeContext(): CanvasRenderingContext2D {
  return new Proxy(
    {},
    {
      get(_target, prop) {
        if (prop === "getImageData") {
          return (_x: number, _y: number, w: number, h: number) => ({
            data: new Uint8ClampedArray(w * h * 4),
          });
        }
        if (prop === "canvas") return document.createElement("canvas");
        return () => undefined;
      },
      set: () => true,
    },
  ) as CanvasRenderingContext2D;
}

const at = (x: number, y: number) => ({ clientX: x, clientY: y, button: 0, pointerId: 1 });

async function mount() {
  render(<SnipApp />);
  await vi.waitFor(() => {
    expect(calls.some((c) => c.cmd === "screenshot_ready")).toBe(true);
  });
  return screen.getByText(ru.screenshot.hint).closest("div.fixed") as HTMLElement;
}

describe("SnipApp", () => {
  beforeEach(() => {
    calls = [];
    Object.defineProperty(window, "innerWidth", { configurable: true, value: WIDTH });
    Object.defineProperty(window, "innerHeight", { configurable: true, value: HEIGHT });
    vi.stubGlobal(
      "ImageData",
      class {
        constructor(
          public data: Uint8ClampedArray,
          public width: number,
          public height: number,
        ) {}
      },
    );
    // jsdom has no PointerEvent; mouse events carry the coordinates we need.
    vi.stubGlobal("PointerEvent", MouseEvent);
    HTMLCanvasElement.prototype.getContext = (() =>
      fakeContext()) as unknown as HTMLCanvasElement["getContext"];
    Element.prototype.setPointerCapture = () => undefined;
    mockIPC((cmd, args) => {
      calls.push({ cmd, args });
      if (cmd === "screenshot_frame") {
        return packImage(WIDTH, HEIGHT, new Uint8ClampedArray(WIDTH * HEIGHT * 4)).buffer;
      }
      return null;
    });
  });

  afterEach(() => {
    cleanup();
    clearMocks();
    vi.unstubAllGlobals();
  });

  it("shows the frozen screen once it is drawn", async () => {
    await mount();
    expect(screen.getByText(ru.screenshot.hint)).toBeInTheDocument();
  });

  it("closes on Esc", async () => {
    await mount();
    fireEvent.keyDown(window, { key: "Escape" });
    expect(calls.some((c) => c.cmd === "screenshot_close")).toBe(true);
  });

  it("selects a region and copies it with Enter", async () => {
    const root = await mount();
    fireEvent.pointerDown(root, at(10, 10));
    fireEvent.pointerMove(root, at(60, 40));
    fireEvent.pointerUp(root, at(60, 40));
    expect(await screen.findByRole("toolbar", { name: ru.screenshot.tools })).toBeInTheDocument();
    expect(screen.getByText(ru.screenshot.size(50, 30))).toBeInTheDocument();
    act(() => {
      fireEvent.keyDown(window, { key: "Enter" });
    });
    await vi.waitFor(() => {
      expect(calls.some((c) => c.cmd === "screenshot_copy")).toBe(true);
    });
    const sent = calls.find((c) => c.cmd === "screenshot_copy")?.args as Uint8Array;
    const view = new DataView(sent.buffer, sent.byteOffset);
    expect([view.getUint32(0, true), view.getUint32(4, true)]).toEqual([50, 30]);
    expect(sent.length).toBe(8 + 50 * 30 * 4);
  });

  it("takes the whole screen on a plain click", async () => {
    const root = await mount();
    fireEvent.pointerDown(root, at(30, 30));
    fireEvent.pointerUp(root, at(30, 30));
    expect(await screen.findByText(ru.screenshot.size(WIDTH, HEIGHT))).toBeInTheDocument();
  });

  it("switches tools and colours from the toolbar", async () => {
    const root = await mount();
    fireEvent.pointerDown(root, at(30, 30));
    fireEvent.pointerUp(root, at(30, 30));
    const blur = await screen.findByRole("button", { name: ru.screenshot.blur });
    fireEvent.click(blur);
    expect(blur).toHaveAttribute("aria-pressed", "true");
    const blue = screen.getByRole("button", {
      name: ru.screenshot.color(ru.screenshot.colors.blue ?? ""),
    });
    fireEvent.click(blue);
    expect(blue).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: ru.screenshot.cancel }));
    expect(calls.some((c) => c.cmd === "screenshot_close")).toBe(true);
  });
});
