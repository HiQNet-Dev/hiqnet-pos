import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, ReportsView } from "../contracts";

const api = vi.hoisted(() => ({ reportsSummary: vi.fn() }));

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

import { ReportsScreen } from "./ReportsScreen";

const actor: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana",
  role: "owner",
  permissions: ["reports.view"],
};

const report: ReportsView = {
  from: "2026-10-27T00:00:00Z",
  to: "2026-11-05T00:00:00Z",
  sales: {
    count: 2,
    subtotal: "20.00",
    tax: "3.20",
    total: "23.20",
    returned_count: 1,
    returned_total: "11.60",
    net_total: "11.60",
    products: [
      {
        product_id: "p-1",
        product_name: "Café",
        quantity: "2",
        net: "20.00",
        tax: "3.20",
        total: "23.20",
      },
    ],
  },
  cash: {
    shifts_opened: 1,
    shifts_closed: 0,
    entries: "23.20",
    exits: "23.60",
    expenses_from_cash: "12.00",
    refunds: "11.60",
    difference: "0.00",
  },
  purchases: {
    orders_created: 1,
    pending_orders: 1,
    received_quantity: "2",
    received_cost: "16.00",
  },
  expenses: {
    total: "21.00",
    from_cash: "12.00",
    external: "9.00",
    categories: [{ name: "Operación", total: "21.00" }],
  },
  inventory: {
    stock_quantity: "20",
    inbound: "23",
    outbound: "2",
    waste: "1",
    adjustment_in: "0",
    adjustment_out: "0",
  },
};

describe("ReportsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.reportsSummary.mockResolvedValue(report);
  });

  it("muestra ventas netas separadas de las devoluciones y cada fuente", async () => {
    render(<ReportsScreen actor={actor} readOnly={false} />);
    expect(await screen.findByText("Ventas netas")).toBeVisible();
    expect((await screen.findAllByText("$23.20")).length).toBeGreaterThan(0);
    expect((await screen.findAllByText("$11.60")).length).toBeGreaterThan(0);
    expect(screen.getByText("Pendientes de recibir")).toBeVisible();
    expect(screen.getByText("Merma")).toBeVisible();
    expect(screen.getByText(/no existe una política de costeo aprobada/i)).toBeVisible();
  });

  it("envía un intervalo UTC semiabierto al recalcular el período", async () => {
    render(<ReportsScreen actor={actor} readOnly={false} />);
    const user = userEvent.setup();
    await screen.findByText("Ventas netas");
    expect(api.reportsSummary).toHaveBeenCalledTimes(1);
    // El rango por defecto depende del día actual: se elige una fecha distinta
    // para que la prueba no dependa del reloj ni de la zona horaria.
    const desde = screen.getByLabelText("Desde") as HTMLInputElement;
    const elegida = desde.value === "2020-01-01" ? "2020-01-02" : "2020-01-01";
    await user.clear(desde);
    await user.type(desde, elegida);
    expect(api.reportsSummary).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/los resultados mostrados corresponden/i)).toBeVisible();
    await user.click(screen.getByRole("button", { name: /actualizar/i }));
    await waitFor(() => expect(screen.queryByText(/los resultados mostrados corresponden/i)).not.toBeInTheDocument());
    await waitFor(() => expect(api.reportsSummary).toHaveBeenCalledTimes(2));
    const last = api.reportsSummary.mock.calls.at(-1)?.[0];
    expect(last?.from).toBe(`${elegida}T00:00:00.000Z`);
    expect(last?.to).toMatch(/T00:00:00\.000Z$/);
  });

  it("muestra el error de IPC", async () => {
    const { IpcError } = await import("../api");
    api.reportsSummary.mockRejectedValueOnce(new IpcError("validation_error", "período inválido"));
    render(<ReportsScreen actor={actor} readOnly={false} />);
    expect(await screen.findByText(/período inválido/i)).toBeVisible();
  });

  it("oculta el módulo sin permiso", () => {
    render(<ReportsScreen actor={{ ...actor, permissions: [] }} readOnly={false} />);
    expect(screen.getByText(/no tienes permiso/i)).toBeVisible();
    expect(api.reportsSummary).not.toHaveBeenCalled();
  });
});
