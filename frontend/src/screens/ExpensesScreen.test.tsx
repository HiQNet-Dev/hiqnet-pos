import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, CashShift } from "../contracts";

const api = vi.hoisted(() => ({
  listExpenseCategories: vi.fn(),
  listExpenses: vi.fn(),
  createExpenseCategory: vi.fn(),
  recordExpense: vi.fn(),
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

import { ExpensesScreen } from "./ExpensesScreen";

const actor: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana",
  role: "owner",
  permissions: ["expenses.manage"],
};

const shift: CashShift = {
  id: "shift-1",
  register_id: "register-1",
  status: "open",
  opened_by: "u-1",
  opened_at: "2026-11-04T08:00:00Z",
  closed_by: null,
  closed_at: null,
  opening_amount: "100.00",
  expected_amount: "100.00",
  entry_total: "0.00",
  exit_total: "0.00",
  counted_amount: null,
  difference_minor: null,
  movements: [],
};

function renderScreen(props: Partial<Parameters<typeof ExpensesScreen>[0]> = {}) {
  const onCashChanged = vi.fn();
  const view = render(
    <ExpensesScreen actor={actor} readOnly={false} shift={null} onCashChanged={onCashChanged} {...props} />,
  );
  return { ...view, onCashChanged };
}

describe("ExpensesScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.listExpenseCategories.mockResolvedValue([{ id: "cat-1", name: "Servicios", active: true }]);
    api.listExpenses.mockResolvedValue([]);
    api.recordExpense.mockResolvedValue({
      id: "expense-1",
      category_id: "cat-1",
      amount: "12.50",
      concept: "Internet",
      paid_from_cash: false,
      occurred_at: "2026-11-04T08:00:00Z",
    });
  });

  it("reintenta un resultado incierto con la misma clave y el mismo payload", async () => {
    const { IpcError } = await import("../api");
    api.recordExpense
      .mockRejectedValueOnce(new IpcError("unknown_error", "respuesta perdida"))
      .mockResolvedValueOnce({
        id: "expense-1",
        category_id: "cat-1",
        amount: "12.50",
        concept: "Internet",
        paid_from_cash: false,
        occurred_at: "2026-11-04T08:00:00Z",
      });
    renderScreen();
    const user = userEvent.setup();
    await screen.findByRole("option", { name: "Servicios" });
    await user.selectOptions(screen.getByLabelText("Categoría"), "cat-1");
    await user.type(screen.getByLabelText("Concepto"), "Internet");
    await user.type(screen.getByLabelText("Importe"), "12.50");
    await user.click(screen.getByRole("button", { name: /registrar gasto/i }));
    expect(await screen.findByText(/no se pudo confirmar el resultado/i)).toBeVisible();
    const first = api.recordExpense.mock.calls[0]?.[0];
    await user.click(screen.getByRole("button", { name: /registrar gasto/i }));
    await waitFor(() => expect(api.recordExpense).toHaveBeenCalledTimes(2));
    expect(api.recordExpense.mock.calls[1]?.[0]).toEqual(first);
    expect(await screen.findByText(/gasto externo registrado/i)).toBeVisible();
  });

  it("envía el turno actual sólo para un gasto pagado desde caja", async () => {
    const { onCashChanged } = renderScreen({ shift });
    const user = userEvent.setup();
    await screen.findByRole("option", { name: "Servicios" });
    await user.selectOptions(screen.getByLabelText("Categoría"), "cat-1");
    await user.type(screen.getByLabelText("Concepto"), "Limpieza");
    await user.type(screen.getByLabelText("Importe"), "10.00");
    await user.click(screen.getByLabelText(/pagar desde caja/i));
    await user.click(screen.getByRole("button", { name: /registrar gasto/i }));
    await waitFor(() => expect(api.recordExpense).toHaveBeenCalledTimes(1));
    expect(api.recordExpense.mock.calls[0]?.[0]).toMatchObject({
      paid_from_cash: true,
      shift_id: "shift-1",
    });
    expect(onCashChanged).toHaveBeenCalledTimes(1);
  });

  it("no expone el módulo a un cajero", () => {
    renderScreen({ actor: { ...actor, role: "cashier", permissions: [] } });
    expect(screen.getByText(/no tienes permiso/i)).toBeVisible();
    expect(api.listExpenseCategories).not.toHaveBeenCalled();
  });
});
