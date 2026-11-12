import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, CashRegister, CashShift } from "../contracts";

const api = vi.hoisted(() => ({
  createCashRegister: vi.fn(),
  openCashShift: vi.fn(),
  registerCashMovement: vi.fn(),
  closeCashShift: vi.fn(),
  listCashShiftHistory: vi.fn(),
}));

vi.mock("../api", () => ({
  api,
  IpcError: class IpcError extends Error {
    code: string;
    constructor(code: string, message: string) {
      super(message);
      this.code = code;
    }
  },
}));

import { CashScreen } from "./CashScreen";

const actor: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana",
  role: "owner",
  permissions: [
    "cash.register.manage",
    "cash.shift.view",
    "cash.shift.open",
    "cash.shift.close",
    "cash.movement.record",
  ],
};

const register: CashRegister = {
  id: "r-1",
  name: "Caja principal",
  code: "CJ-01",
  active: true,
};

const shift: CashShift = {
  id: "sh-1",
  register_id: "r-1",
  status: "open",
  opened_by: "u-1",
  opened_at: "2026-11-04T08:00:00Z",
  closed_by: null,
  closed_at: null,
  opening_amount: "100.00",
  expected_amount: "132.48",
  entry_total: "32.48",
  exit_total: "0.00",
  counted_amount: null,
  difference_minor: null,
  movements: [
    {
      id: "mv-1",
      kind: "in",
      amount: "32.48",
      reason: "venta V-ABC",
      occurred_at: "2026-11-04T08:30:00Z",
    },
  ],
};

const closed: CashShift = {
  ...shift,
  status: "closed",
  closed_by: "u-1",
  closed_at: "2026-11-04T09:00:00Z",
  counted_amount: "130.00",
  difference_minor: -248,
  movements: [],
};

interface Options {
  registers?: CashRegister[];
  shift?: CashShift | null;
  actor?: Actor;
  readOnly?: boolean;
}

function renderScreen({
  registers = [register],
  shift: activeShift = null,
  actor: currentActor = actor,
  readOnly = false,
}: Options = {}) {
  const onShiftChanged = vi.fn();
  const onRegisterCreated = vi.fn();

  const element = (shiftValue: CashShift | null) => (
    <CashScreen
      actor={currentActor}
      readOnly={readOnly}
      registers={registers}
      registerId={registers[0]?.id ?? ""}
      onRegisterChange={() => {}}
      shift={shiftValue}
      onShiftChanged={onShiftChanged}
      onRegisterCreated={onRegisterCreated}
    />
  );

  const view = render(element(activeShift));
  return {
    onShiftChanged,
    onRegisterCreated,
    // Simula el refresco del shell: al cerrar el turno deja de existir uno abierto.
    rerender: (shiftValue: CashShift | null) => view.rerender(element(shiftValue)),
  };
}

describe("CashScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.listCashShiftHistory.mockResolvedValue([]);
    api.createCashRegister.mockResolvedValue(register);
    api.openCashShift.mockResolvedValue(shift);
    api.registerCashMovement.mockResolvedValue({});
    api.closeCashShift.mockResolvedValue(closed);
  });

  it("permite crear la caja principal cuando no existe", async () => {
    const { onRegisterCreated } = renderScreen({ registers: [] });
    const user = userEvent.setup();

    expect(screen.getByText("Sin turno", { selector: ".badge" })).toBeVisible();
    await user.type(screen.getByLabelText("Nombre"), "Caja principal");
    await user.type(screen.getByLabelText("Código"), "CJ-01");
    await user.click(screen.getByRole("button", { name: /crear caja/i }));

    await waitFor(() =>
      expect(api.createCashRegister).toHaveBeenCalledWith({
        name: "Caja principal",
        code: "CJ-01",
      }),
    );
    expect(onRegisterCreated).toHaveBeenCalled();
  });

  it("abre un turno con fondo inicial", async () => {
    const { onShiftChanged } = renderScreen();
    const user = userEvent.setup();

    await user.type(screen.getByLabelText(/fondo inicial/i), "100.00");
    await user.click(screen.getByRole("button", { name: /abrir turno/i }));

    await waitFor(() =>
      expect(api.openCashShift).toHaveBeenCalledWith({
        register_id: "r-1",
        opening_amount: "100.00",
      }),
    );
    expect(onShiftChanged).toHaveBeenCalled();
  });

  it("muestra el turno abierto con saldo esperado y movimientos", () => {
    renderScreen({ shift });
    expect(screen.getByText(/turno abierto/i)).toBeVisible();
    expect(screen.getByText("$100.00")).toBeVisible();
    expect(screen.getByText("$132.48")).toBeVisible();
    expect(screen.getByText(/venta v-abc/i)).toBeVisible();
  });

  it("registra una entrada manual y refresca el turno", async () => {
    const { onShiftChanged } = renderScreen({ shift });
    const user = userEvent.setup();

    await user.type(screen.getByLabelText("Importe"), "50.00");
    await user.type(screen.getByLabelText("Motivo"), "cambio");
    await user.click(screen.getByRole("button", { name: /^registrar$/i }));

    await waitFor(() =>
      expect(api.registerCashMovement).toHaveBeenCalledWith({
        shift_id: "sh-1",
        kind: "in",
        amount: "50.00",
        reason: "cambio",
      }),
    );
    expect(onShiftChanged).toHaveBeenCalled();
  });

  it("revisa el arqueo antes de cerrar y permite cancelar", async () => {
    const { onShiftChanged } = renderScreen({ shift });
    const user = userEvent.setup();

    await user.type(screen.getByLabelText(/efectivo contado/i), "130.00");
    await user.click(screen.getByRole("button", { name: /revisar cierre/i }));

    const review = await screen.findByRole("dialog");
    expect(within(review).getByRole("heading", { name: /cerrar turno/i })).toBeVisible();
    expect(within(review).getByText(/\$132\.48/)).toBeVisible();
    expect(within(review).getByText(/faltante \$2\.48/i)).toBeVisible();
    expect(api.closeCashShift).not.toHaveBeenCalled();

    await user.click(within(review).getByRole("button", { name: /^cancelar$/i }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(api.closeCashShift).not.toHaveBeenCalled();
    expect(onShiftChanged).not.toHaveBeenCalled();
    expect(screen.getByRole("region", { name: /operación del turno/i })).toBeVisible();
  });

  it("cierra el turno y conserva el resultado del arqueo", async () => {
    const { onShiftChanged, rerender } = renderScreen({ shift });
    const user = userEvent.setup();

    await user.type(screen.getByLabelText(/efectivo contado/i), "130.00");
    await user.click(screen.getByRole("button", { name: /revisar cierre/i }));
    const review = await screen.findByRole("dialog");
    await user.click(
      within(review).getByRole("button", { name: /cerrar turno y registrar arqueo/i }),
    );

    await waitFor(() =>
      expect(api.closeCashShift).toHaveBeenCalledWith({
        shift_id: "sh-1",
        counted_amount: "130.00",
      }),
    );
    expect(onShiftChanged).toHaveBeenCalled();
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());

    rerender(null);

    expect(await screen.findByText(/resultado del arqueo/i)).toBeVisible();
    expect(screen.getAllByText(/faltante \$2\.48/i).length).toBeGreaterThan(0);
    expect(screen.getByText("$130.00", { selector: "dd" })).toBeVisible();
  });

  it("muestra error de IPC al abrir el turno", async () => {
    const { IpcError } = await import("../api");
    api.openCashShift.mockRejectedValueOnce(new IpcError("conflict", "ya existe un turno abierto"));
    renderScreen();
    const user = userEvent.setup();

    await user.type(screen.getByLabelText(/fondo inicial/i), "100.00");
    await user.click(screen.getByRole("button", { name: /abrir turno/i }));

    expect(await screen.findByText(/ya existe un turno abierto/i)).toBeVisible();
  });

  it("respeta el modo solo lectura", () => {
    renderScreen({ shift, readOnly: true });
    expect(screen.queryByRole("button", { name: /cerrar turno/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^registrar$/i })).not.toBeInTheDocument();
  });

  it("lista el historial de turnos cerrados", async () => {
    api.listCashShiftHistory.mockResolvedValue([closed]);
    renderScreen();
    expect(await screen.findByText(/turnos cerrados/i)).toBeVisible();
    expect(await screen.findByText(/contado \$130\.00/i)).toBeVisible();
    expect(screen.getByText(/faltante \$2\.48/i)).toBeVisible();
  });
});
