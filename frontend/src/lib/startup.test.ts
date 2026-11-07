import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const statusMock = vi.hoisted(() => vi.fn());

vi.mock("../api", async () => {
  const actual = await vi.importActual<typeof import("../api")>("../api");
  return { ...actual, api: { ...actual.api, status: statusMock } };
});

import { IpcError } from "../api";
import { loadInitialStatus } from "./startup";

const READY_STATUS = {
  initialized: false,
  product_version: "0.1.0",
  commercial: null,
  user_count: 0,
};

describe("loadInitialStatus", () => {
  beforeEach(() => {
    statusMock.mockReset();
    vi.spyOn(console, "warn").mockImplementation(() => {});
  });

  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it("no reintenta cuando la primera consulta responde", async () => {
    statusMock.mockResolvedValueOnce(READY_STATUS);

    await expect(loadInitialStatus()).resolves.toEqual(READY_STATUS);
    expect(statusMock).toHaveBeenCalledTimes(1);
  });

  it("reintenta un rechazo no estructurado del puente IPC", async () => {
    statusMock.mockRejectedValueOnce("puente no listo").mockResolvedValueOnce(READY_STATUS);

    await expect(loadInitialStatus()).resolves.toEqual(READY_STATUS);
    expect(statusMock).toHaveBeenCalledTimes(2);
  });

  it("reintenta un unknown_error transitorio", async () => {
    statusMock
      .mockRejectedValueOnce(new IpcError("unknown_error", "sin estructura"))
      .mockResolvedValueOnce(READY_STATUS);

    await expect(loadInitialStatus()).resolves.toEqual(READY_STATUS);
    expect(statusMock).toHaveBeenCalledTimes(2);
  });

  it("no reintenta errores permanentes de SQLite", async () => {
    const permanent = new IpcError("storage_error", "No fue posible abrir la base local.");
    statusMock.mockRejectedValue(permanent);

    await expect(loadInitialStatus()).rejects.toBe(permanent);
    expect(statusMock).toHaveBeenCalledTimes(1);
  });

  it("no reintenta errores de contrato", async () => {
    const mismatch = new IpcError("contract_mismatch", "respuesta inválida");
    statusMock.mockRejectedValue(mismatch);

    await expect(loadInitialStatus()).rejects.toBe(mismatch);
    expect(statusMock).toHaveBeenCalledTimes(1);
  });

  it("agota los intentos y propaga el último error", async () => {
    const transient = new IpcError("unknown_error", "sin estructura");
    statusMock.mockRejectedValue(transient);

    await expect(loadInitialStatus()).rejects.toBe(transient);
    expect(statusMock).toHaveBeenCalledTimes(4);
  });

  it("corta una promesa pendiente por timeout, sin bucle infinito", async () => {
    vi.useFakeTimers();
    statusMock.mockImplementation(() => new Promise(() => {}));

    const pending = loadInitialStatus();
    const assertion = expect(pending).rejects.toMatchObject({ code: "startup_timeout" });
    await vi.advanceTimersByTimeAsync(60_000);
    await assertion;
    expect(statusMock).toHaveBeenCalledTimes(4);
  });
});
