import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const appApi = vi.hoisted(() => ({ onBackButtonPress: vi.fn() }));

vi.mock("@tauri-apps/api/app", () => appApi);

import {
  handleBackButton,
  installAndroidBackGuard,
  isAndroidTauri,
  registerBackHandler,
} from "./backNavigation";

const ANDROID_UA =
  "Mozilla/5.0 (Linux; Android 17; Pixel Tablet) AppleWebKit/537.36 Chrome/141 Mobile Safari/537.36";

function setUserAgent(value: string) {
  vi.stubGlobal("navigator", { userAgent: value });
}

function markTauri() {
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
}

function clearTauri() {
  delete (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__;
}

type Listener = { unregister: ReturnType<typeof vi.fn> };

function listenerStub(): Listener {
  return { unregister: vi.fn().mockResolvedValue(undefined) };
}

beforeEach(() => {
  vi.clearAllMocks();
  setUserAgent(ANDROID_UA);
  clearTauri();
});

afterEach(() => {
  // Las capas que deje una prueba no deben filtrarse a la siguiente.
  while (handleBackButton()) {
    // consume y descarta
  }
  vi.unstubAllGlobals();
  clearTauri();
});

describe("pila de capas del botón Back", () => {
  it("no consume nada cuando no hay capas", () => {
    expect(handleBackButton()).toBe(false);
  });

  it("da prioridad a la capa registrada más tarde", () => {
    const first = vi.fn(() => true);
    const second = vi.fn(() => true);
    const releaseFirst = registerBackHandler(first);
    const releaseSecond = registerBackHandler(second);

    expect(handleBackButton()).toBe(true);
    expect(second).toHaveBeenCalledTimes(1);
    expect(first).not.toHaveBeenCalled();

    releaseSecond();
    releaseFirst();
  });

  it("atraviesa una capa que no aplica y consume en la siguiente", () => {
    const screen = vi.fn(() => true);
    const modal = vi.fn(() => false);
    const releaseScreen = registerBackHandler(screen);
    const releaseModal = registerBackHandler(modal);

    expect(handleBackButton()).toBe(true);
    expect(modal).toHaveBeenCalledTimes(1);
    expect(screen).toHaveBeenCalledTimes(1);

    releaseModal();
    releaseScreen();
  });

  it("retirar una capa dos veces no afecta a las demás", () => {
    const screen = vi.fn(() => true);
    const release = registerBackHandler(screen);
    release();
    release();

    expect(handleBackButton()).toBe(false);
  });
});

describe("listener nativo", () => {
  it("no registra nada fuera de Android", async () => {
    setUserAgent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");
    markTauri();

    const release = installAndroidBackGuard();
    await Promise.resolve();
    release();

    expect(isAndroidTauri()).toBe(false);
    expect(appApi.onBackButtonPress).not.toHaveBeenCalled();
  });

  it("no registra nada fuera de Tauri aunque el user agent sea Android", async () => {
    const release = installAndroidBackGuard();
    await Promise.resolve();
    release();

    expect(isAndroidTauri()).toBe(false);
    expect(appApi.onBackButtonPress).not.toHaveBeenCalled();
  });

  it("registra un solo listener aunque se monte dos veces (StrictMode)", async () => {
    markTauri();
    const listener = listenerStub();
    appApi.onBackButtonPress.mockResolvedValue(listener);

    const releaseFirst = installAndroidBackGuard();
    const releaseSecond = installAndroidBackGuard();
    await Promise.resolve();
    await Promise.resolve();

    expect(appApi.onBackButtonPress).toHaveBeenCalledTimes(1);

    releaseFirst();
    expect(listener.unregister).not.toHaveBeenCalled();

    releaseSecond();
    expect(listener.unregister).toHaveBeenCalledTimes(1);
  });

  it("conecta el evento nativo con la capa superior", async () => {
    markTauri();
    let nativeHandler: (() => void) | null = null;
    appApi.onBackButtonPress.mockImplementation((handler: () => void) => {
      nativeHandler = handler;
      return Promise.resolve(listenerStub());
    });

    const close = vi.fn(() => true);
    const releaseLayer = registerBackHandler(close);
    const release = installAndroidBackGuard();
    await Promise.resolve();
    await Promise.resolve();

    expect(nativeHandler).not.toBeNull();
    nativeHandler!();
    expect(close).toHaveBeenCalledTimes(1);

    releaseLayer();
    release();
  });

  it("desregistra si la promesa se resuelve después del desmontaje", async () => {
    markTauri();
    const listener = listenerStub();
    let resolveRegistration: ((value: Listener) => void) | null = null;
    appApi.onBackButtonPress.mockImplementation(
      () =>
        new Promise<Listener>((resolve) => {
          resolveRegistration = resolve;
        }),
    );

    const release = installAndroidBackGuard();
    release();
    resolveRegistration!(listener);
    await Promise.resolve();
    await Promise.resolve();

    expect(listener.unregister).toHaveBeenCalledTimes(1);
  });

  it("un fallo de registro no rompe el arranque", async () => {
    markTauri();
    appApi.onBackButtonPress.mockRejectedValue(new Error("plugin app no disponible"));
    const warn = vi.spyOn(console, "warn").mockImplementation(() => {});

    const release = installAndroidBackGuard();
    await Promise.resolve();
    await Promise.resolve();

    expect(warn).toHaveBeenCalled();
    expect(() => release()).not.toThrow();
    warn.mockRestore();
  });
});
