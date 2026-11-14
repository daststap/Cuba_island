import { describe, expect, it } from "vitest";
import { isAppError } from "./commands";

describe("isAppError", () => {
  it("accepts the serialized Rust AppError shape", () => {
    expect(isAppError({ kind: "io", message: "disk" })).toBe(true);
  });

  it("rejects other values", () => {
    expect(isAppError("boom")).toBe(false);
    expect(isAppError(null)).toBe(false);
    expect(isAppError({ kind: "io" })).toBe(false);
  });
});
