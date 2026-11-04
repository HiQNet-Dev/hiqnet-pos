import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, CashShift, ExpenseDraft } from "../contracts";
import {
  loadPendingAttempt,
  pendingAttemptStorageKey,
  savePendingAttempt,
  type PendingExpenseAttempt,
} from "../lib/pendingAttempt";

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

const storedExpense = {
  id: "expense-1",
  category_id: "cat-1",
  amount: "12.50",
  concept: "Internet",
  paid_from_cash: false,
  occurred_at: "2026-11-04T08:00:00Z",
};

/** Reproduce el shell: el intento vive fuera de la pantalla y se persiste. */
function AttemptHarness({ storageKey }: { storageKey: string }) {
  const [attempt, setAttempt] = useState<PendingExpenseAttempt | null>(() =>
    loadPendingAttempt(storageKey),
  );
  const [open, setOpen] = useState(true);
  const update = (next: PendingExpenseAttempt | null) => {
    setAttempt(next);
    savePendingAttempt(storageKey, next);
  };
  return (
    <>
      <button type="button" onClick={() => setOpen((value) => !value)}>
        {open ? "Ir a Ventas" : "Ir a Gastos"}
      </button>
      {open && (
        <ExpensesScreen
          actor={actor}
          readOnly={false}
          shift={shift}
          onCashChanged={vi.fn()}
          pendingAttempt={attempt}
          onPendingAttemptChange={update}
        />
      )}
    </>
  );
}

function renderScreen(props: Partial<Parameters<typeof ExpensesScreen>[0]> = {}) {
  const onCashChanged = vi.fn();
  const view = render(
    <ExpensesScreen
      actor={actor}
      readOnly={false}
      shift={null}
      onCashChanged={onCashChanged}
      pendingAttempt={null}
      onPendingAttemptChange={vi.fn()}
      {...props}
    />,
  );
  return { ...view, onCashChanged };
}

async function fillExpense(user: ReturnType<typeof userEvent.setup>, fromCash = false) {
  await screen.findByRole("option", { name: "Servicios" });
  await user.selectOptions(screen.getByLabelText("Categoría"), "cat-1");
  await user.type(screen.getByLabelText("Concepto"), "Internet");
  await user.type(screen.getByLabelText("Importe"), "12.50");
  if (fromCash) await user.click(screen.getByLabelText(/pagar desde caja/i));
}

describe("ExpensesScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    api.listExpenseCategories.mockResolvedValue([{ id: "cat-1", name: "Servicios", active: true }]);
    api.listExpenses.mockResolvedValue([]);
    api.recordExpense.mockResolvedValue(storedExpense);
  });

  it("conserva el gasto incierto al navegar y lo reintenta con la misma clave", async () => {
    const { IpcError } = await import("../api");
    api.recordExpense
      .mockRejectedValueOnce(new IpcError("unknown_error", "respuesta perdida"))
      .mockResolvedValueOnce(storedExpense);
    const key = pendingAttemptStorageKey("expense", actor);
    render(<AttemptHarness storageKey={key} />);
    const user = userEvent.setup();
    await fillExpense(user);
    await user.click(screen.getByRole("button", { name: /registrar gasto/i }));

    expect(await screen.findByText(/gasto pendiente de confirmar/i)).toBeVisible();
    expect(screen.getByLabelText("Importe")).toBeDisabled();
    expect(screen.getByLabelText("Concepto")).toBeDisabled();
    const first = api.recordExpense.mock.calls[0]?.[0] as ExpenseDraft;

    await user.click(screen.getByRole("button", { name: "Ir a Ventas" }));
    expect(screen.queryByText(/gasto pendiente de confirmar/i)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Ir a Gastos" }));
    expect(await screen.findByText(/gasto pendiente de confirmar/i)).toBeVisible();
    expect(screen.getByText("$12.50")).toBeVisible();

    await user.click(screen.getByRole("button", { name: /reintentar gasto/i }));
    await waitFor(() => expect(api.recordExpense).toHaveBeenCalledTimes(2));
    expect(api.recordExpense.mock.calls[1]?.[0]).toEqual(first);
    await waitFor(() =>
      expect(screen.queryByText(/gasto pendiente de confirmar/i)).not.toBeInTheDocument(),
    );
  });

  it("recupera el gasto incierto después de reiniciar la aplicación", async () => {
    const { IpcError } = await import("../api");
    api.recordExpense
      .mockRejectedValueOnce(new IpcError("storage_error", "sin respuesta"))
      .mockResolvedValueOnce(storedExpense);
    const key = pendingAttemptStorageKey("expense", actor);
    const first = render(<AttemptHarness storageKey={key} />);
    const user = userEvent.setup();
    await fillExpense(user);
    await user.click(screen.getByRole("button", { name: /registrar gasto/i }));
    await screen.findByText(/gasto pendiente de confirmar/i);
    const original = api.recordExpense.mock.calls[0]?.[0] as ExpenseDraft;
    first.unmount();

    // Nuevo montaje del shell: sólo queda lo persistido localmente.
    render(<AttemptHarness storageKey={key} />);
    expect(await screen.findByText(/gasto pendiente de confirmar/i)).toBeVisible();
    expect(screen.getByText("$12.50")).toBeVisible();
    await userEvent.setup().click(screen.getByRole("button", { name: /reintentar gasto/i }));
    await waitFor(() => expect(api.recordExpense).toHaveBeenCalledTimes(2));
    expect(api.recordExpense.mock.calls[1]?.[0]).toEqual(original);
  });

  it("no recupera intentos pertenecientes a otra identidad", async () => {
    const otherKey = pendingAttemptStorageKey("expense", {
      ...actor,
      organization_id: "o-2",
      branch_id: "b-9",
      user_id: "u-9",
    });
    expect(otherKey).not.toBe(pendingAttemptStorageKey("expense", actor));
    savePendingAttempt<ExpenseDraft>(otherKey, {
      fingerprint: "ajeno",
      payload: {
        category_id: "cat-1",
        amount: "99.00",
        concept: "Ajeno",
        paid_from_cash: false,
        shift_id: null,
        idempotency_key: "ajena-1",
      },
    });
    renderScreen();
    await screen.findByRole("option", { name: "Servicios" });
    expect(screen.queryByText(/gasto pendiente de confirmar/i)).not.toBeInTheDocument();
    expect(screen.queryByText("$99.00")).not.toBeInTheDocument();
  });

  it("envía el turno actual sólo para un gasto pagado desde caja", async () => {
    const { onCashChanged } = renderScreen({ shift });
    const user = userEvent.setup();
    await fillExpense(user, true);
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
