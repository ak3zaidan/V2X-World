/**
 * A resize to the size the canvas already has does nothing.
 *
 * `WebGLRenderer.setSize` assigns `canvas.width`, which reallocates the drawing buffer and waits on
 * the GPU process every time, whatever the value. The Studio resizes from a ResizeObserver that
 * also watches the toolbar and the HUD, so every change of theirs re-sized an unchanged canvas;
 * under a software rasteriser on a loaded machine each was a stall of seconds, enough to time out
 * the end-to-end suite (8.7 s of `setSize` in 29 s of page load, profiled).
 */

import { describe, expect, it } from "vitest";
import { Viewer } from "../src/scene.js";
import type { ViewerCanvas } from "../src/types.js";
import { NullRenderer } from "./support/null-renderer.js";

const CANVAS = { width: 800, height: 600, clientWidth: 800, clientHeight: 600 } as unknown as ViewerCanvas;

function counted(): { viewer: Viewer; sizes: () => number[][]; renderers: NullRenderer[] } {
  const renderers: NullRenderer[] = [];
  const calls: number[][] = [];
  const viewer = new Viewer({
    canvas: CANVAS,
    theme: "dark",
    autoStart: false,
    createRenderer: (c) => {
      const r = new NullRenderer(c);
      const setSize = r.setSize.bind(r);
      r.setSize = (w: number, h: number) => {
        calls.push([w, h]);
        setSize(w, h);
      };
      renderers.push(r);
      return r;
    },
  });
  return { viewer, sizes: () => calls, renderers };
}

describe("Viewer.resize", () => {
  it("sizes the renderer once per real change of size", () => {
    const { viewer, sizes } = counted();
    const atMount = sizes().length;
    expect(atMount).toBeGreaterThan(0);
    for (let i = 0; i < 10; i++) viewer.resize(800, 600);
    expect(sizes().length).toBe(atMount);
    viewer.resize(1024, 600);
    expect(sizes().length).toBe(atMount + 1);
    expect(sizes().at(-1)).toEqual([1024, 600]);
    expect(viewer.size).toEqual({ width: 1024, height: 600 });
    viewer.resize(1024.6, 600.2); // the same pixels
    expect(sizes().length).toBe(atMount + 1);
    viewer.dispose();
  });

  it("sizes a new renderer even at the same size", () => {
    const { viewer, sizes, renderers } = counted();
    viewer.resize(800, 600);
    const before = sizes().length;
    viewer.unmount();
    viewer.mount({ ...CANVAS } as ViewerCanvas);
    expect(renderers.length).toBe(2);
    expect(sizes().length).toBeGreaterThan(before);
    expect(sizes().at(-1)).toEqual([800, 600]);
    viewer.dispose();
  });
});
