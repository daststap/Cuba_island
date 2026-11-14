import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Island } from "./app/Island";
import "./styles/index.css";

async function start() {
  const root = document.getElementById("root");
  if (!root) {
    throw new Error("#root element is missing in index.html");
  }

  // Dev only: `?demo=<scene>` renders the island with fake data for screenshots.
  // The demo module is not part of production builds.
  if (import.meta.env.DEV) {
    const scene = new URLSearchParams(window.location.search).get("demo");
    if (scene) {
      const { SCENES, setupDemo } = await import("./demo/demo");
      const known = SCENES.find((s) => s === scene);
      if (known) await setupDemo(known);
    }
  }

  createRoot(root).render(
    <StrictMode>
      <Island />
    </StrictMode>,
  );
}

void start();
