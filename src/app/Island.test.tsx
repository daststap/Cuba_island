import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { ru } from "../i18n/ru";
import { Island } from "./Island";

describe("Island", () => {
  it("renders the placeholder pill", () => {
    render(<Island />);
    expect(screen.getByRole("status")).toHaveTextContent(ru.appName);
  });
});
