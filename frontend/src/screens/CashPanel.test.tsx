import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, CashRegister, CashShift } from "../contracts";

const api = vi.hoisted(() => ({
  listCashRegisters: vi.fn(),
  listCashShiftHistory: vi.fn(),
  activeCashShift: vi.fn(),
  createCashRegister: vi.fn(),
  openCashShift: vi.fn(),
  registerCashMovement: vi.fn(),
  closeCashShift: vi.fn(),
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

import { CashPanel } from "./CashPanel";

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
  id: "s-1",
  register_id: "r-1",
  status: "open",
  opened_by: "u-1",
  opened_at: "2026-11-04T00:00:00Z",
  closed_by: null,
  closed_at: null,
  opening_amount: "100.00",
  expected_amount: "120.00",
  entry_total: "20.00",
  exit_total: "0.00",
  counted_amount: null,
  difference_minor: null,
  movements: [
    {
      id: "m-1",
      kind: "in",
      amount: "20.00",
      reason: "cambio",
      occurred_at: "2026-11-04T00:00:00Z",
    },
  ],
};

describe("CashPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.listCashRegisters.mockResolvedValue([register]);
    api.listCashShiftHistory.mockResolvedValue([]);
    api.activeCashShift.mockResolvedValue(null);
    api.openCashShift.mockResolvedValue(shift);
  });

  it("abre un turno con fondo inicial", async () => {
    render(<CashPanel actor={actor} readOnly={false} />);
    const user = userEvent.setup();
    await user.type(await screen.findByLabelText(/fondo inicial/i), "100.00");
    await user.click(screen.getByRole("button", { name: /abrir turno/i }));
    await waitFor(() => {
      expect(api.openCashShift).toHaveBeenCalledWith({
        register_id: "r-1",
        opening_amount: "100.00",
      });
    });
    expect(await screen.findByText(/120\.00/)).toBeVisible();
  });

  it("muestra saldo esperado y movimientos del turno", async () => {
    api.activeCashShift.mockResolvedValue(shift);
    render(<CashPanel actor={actor} readOnly={false} />);
    expect(await screen.findByText(/120\.00/)).toBeVisible();
    expect(screen.getByText(/cambio/i)).toBeVisible();
    expect(screen.getByText(/100\.00/)).toBeVisible();
  });

  it("muestra historial de turnos cerrados con diferencia", async () => {
    const closed: CashShift = {
      ...shift,
      status: "closed",
      closed_by: "u-1",
      closed_at: "2026-11-04T01:00:00Z",
      counted_amount: "118.00",
      difference_minor: -200,
    };
    api.listCashRegisters.mockResolvedValue([register]);
    api.listCashShiftHistory.mockResolvedValue([closed]);
    api.activeCashShift.mockResolvedValue(null);
    render(<CashPanel actor={actor} readOnly={false} />);
    expect(await screen.findByText(/historial de turnos cerrados/i)).toBeVisible();
    expect(screen.getByText(/118\.00/)).toBeVisible();
    expect(screen.getByText(/-200 centavos/i)).toBeVisible();
  });

  it("oculta acciones sin permiso de caja", () => {
    render(
      <CashPanel
        actor={{ ...actor, permissions: ["cash.shift.view"] }}
        readOnly={false}
      />,
    );
    expect(screen.queryByRole("button", { name: /abrir turno/i })).not.toBeInTheDocument();
  });
});