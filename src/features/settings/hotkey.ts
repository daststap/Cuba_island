/** What we need from a keydown event to build a hotkey label. */
export interface KeyPress {
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const MODIFIER_CODES = new Set([
  "ControlLeft",
  "ControlRight",
  "AltLeft",
  "AltRight",
  "ShiftLeft",
  "ShiftRight",
  "MetaLeft",
  "MetaRight",
]);

/** `KeyV` → `V`, `Digit1` → `1`, everything else (`Space`, `F5`, `Backquote`) as is. */
function keyName(code: string): string {
  if (code.startsWith("Key")) return code.slice(3);
  if (code.startsWith("Digit")) return code.slice(5);
  return code;
}

/**
 * Turns a key press into a hotkey label in the format Rust parses ("Win+Ctrl+Alt+Shift+Key").
 * Returns `null` while only modifiers are held or without Ctrl/Alt/Win.
 */
export function comboFromKeyPress(e: KeyPress): string | null {
  if (MODIFIER_CODES.has(e.code)) return null;
  if (!e.ctrlKey && !e.altKey && !e.metaKey) return null;
  const parts: string[] = [];
  if (e.metaKey) parts.push("Win");
  if (e.ctrlKey) parts.push("Ctrl");
  if (e.altKey) parts.push("Alt");
  if (e.shiftKey) parts.push("Shift");
  parts.push(keyName(e.code));
  return parts.join("+");
}
