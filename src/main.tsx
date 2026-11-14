import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { Island } from "./app/Island";
import "./styles/index.css";

const root = document.getElementById("root");
if (!root) {
  throw new Error("#root element is missing in index.html");
}

createRoot(root).render(
  <StrictMode>
    <Island />
  </StrictMode>,
);
