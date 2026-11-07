import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { installViewportGuard } from "./viewportGuard";

let frames: FrameRequestCallback[] = [];

function flushFrames() {
  const pending = frames;
  frames = [];
  for (const callback of pending) callback(0);
}

function stubScroll(tracker: { writes: number[] }, initial: number) {
  Object.defineProperty(document.body, "scrollTop", {
    configurable: true,
    get: () => initial,
    set: (value: number) => {
      tracker.writes.push(value);
    },
  });
}

describe("installViewportGuard", () => {
  beforeEach(() => {
    frames = [];
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      frames.push(callback);
      return frames.length;
    });
    vi.stubGlobal("cancelAnimationFrame", () => {});
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
    // `scrollTop` de jsdom no refleja desplazamientos; se restaura el descriptor
    // nativo tras cada prueba.
    delete (document.body as unknown as Record<string, unknown>).scrollTop;
  });

  it("devuelve la raíz a su origen cuando el documento se desplaza", () => {
    const tracker = { writes: [] as number[] };
    stubScroll(tracker, 240);
    const uninstall = installViewportGuard();
    // La instalación ya programa una corrección; se consume para dejar el
    // guardia en reposo y medir solo el desplazamiento posterior.
    flushFrames();
    tracker.writes = [];

    document.body.dispatchEvent(new Event("scroll"));
    flushFrames();

    expect(tracker.writes).toEqual([0]);
    uninstall();
  });

  it("no interfiere con el desplazamiento propio de una pantalla", () => {
    const tracker = { writes: [] as number[] };
    stubScroll(tracker, 240);
    const uninstall = installViewportGuard();
    flushFrames();
    tracker.writes = [];

    const screen = document.createElement("div");
    screen.className = "screen";
    document.body.appendChild(screen);
    screen.dispatchEvent(new Event("scroll"));
    flushFrames();

    expect(tracker.writes).toEqual([]);
    uninstall();
    screen.remove();
  });

  it("deja de observar tras desinstalarse", () => {
    const tracker = { writes: [] as number[] };
    stubScroll(tracker, 120);
    const uninstall = installViewportGuard();
    flushFrames();
    tracker.writes = [];

    uninstall();
    frames = [];
    document.body.dispatchEvent(new Event("scroll"));
    flushFrames();

    expect(tracker.writes).toEqual([]);
  });
});
