import { useEffect, useState } from "react";
import { dominantColor, type Rgb } from "./model";

const SAMPLE_SIZE = 16;

/** Dominant colour of a cover image, computed on a 16×16 canvas (PLAN.md stage 2). */
export function useCoverColor(cover: string | null): Rgb | null {
  const [sample, setSample] = useState<{ cover: string; color: Rgb | null } | null>(null);

  useEffect(() => {
    if (!cover) return;
    let active = true;
    const image = new Image();
    image.onload = () => {
      if (!active) return;
      const canvas = document.createElement("canvas");
      canvas.width = SAMPLE_SIZE;
      canvas.height = SAMPLE_SIZE;
      const context = canvas.getContext("2d", { willReadFrequently: true });
      if (!context) return;
      context.drawImage(image, 0, 0, SAMPLE_SIZE, SAMPLE_SIZE);
      const color = dominantColor(context.getImageData(0, 0, SAMPLE_SIZE, SAMPLE_SIZE).data);
      setSample({ cover, color });
    };
    image.src = cover;
    return () => {
      active = false;
    };
  }, [cover]);

  // Only trust a colour computed for the current cover.
  return sample && sample.cover === cover ? sample.color : null;
}
