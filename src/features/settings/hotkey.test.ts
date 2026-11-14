import { describe, expect, it } from "vitest";
import { comboFromKeyPress, type KeyPress } from "./hotkey";

const press = (code: string, mods: Partial<KeyPress> = {}): KeyPress => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("comboFromKeyPress", () => {
  it("builds labels in the order Rust expects", () => {
    expect(comboFromKeyPress(press("KeyV", { ctrlKey: true, altKey: true }))).toBe("Ctrl+Alt+V");
    expect(comboFromKeyPress(press("Space", { altKey: true }))).toBe("Alt+Space");
    expect(comboFromKeyPress(press("Digit1", { metaKey: true, shiftKey: true }))).toBe(
      "Win+Shift+1",
    );
    expect(comboFromKeyPress(press("F5", { ctrlKey: true }))).toBe("Ctrl+F5");
  });

  it("waits while only modifiers are held and rejects plain keys", () => {
    expect(comboFromKeyPress(press("AltLeft", { altKey: true }))).toBeNull();
    expect(comboFromKeyPress(press("KeyV"))).toBeNull();
    expect(comboFromKeyPress(press("KeyV", { shiftKey: true }))).toBeNull();
  });
});
