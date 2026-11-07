import { act, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, CashShift, ReturnableSale, SaleReturn, SaleReturnDraft } from "../contracts";
import {
  loadPendingAttempt,
  pendingAttemptStorageKey,
  savePendingAttempt,
  type PendingReturnAttempt,
} from "../lib/pendingAttempt";

const api = vi.hoisted(() => ({
  findSaleForReturn: vi.fn(),
  previewSaleReturn: vi.fn(),
  createSaleReturn: vi.fn(),
  listSaleReturns: vi.fn(),
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

import { ReturnsScreen } from "./ReturnsScreen";

const actor: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana",
  role: "owner",
  permissions: ["sales.sell", "sales.return"],
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
  expected_amount: "123.20",
  entry_total: "23.20",
  exit_total: "0.00",
  counted_amount: null,
  difference_minor: null,
  movements: [],
};

const sale: ReturnableSale = {
  id: "sale-1",
  folio: "V-ABC",
  occurred_at: "2026-11-04T08:30:00Z",
  total: "23.20",
  items: [
    {
      id: "item-1",
      product_name: "Café",
      variant_name: null,
      modifiers_summary: "",
      quantity: "2",
      returned_quantity: "0",
      returnable_quantity: "2",
      unit_refund: "11.60",
    },
  ],
};

const created: SaleReturn = {
  id: "return-1",
  sale_id: "sale-1",
  sale_folio: "V-ABC",
  reason: "producto en buen estado",
  refund_total: "11.60",
  occurred_at: "2026-11-04T09:00:00Z",
  lines: [
    {
      sale_item_id: "item-1",
      product_name: "Café",
      quantity: "1",
      disposition: "restock",
      refund_net: "10.00",
      refund_tax: "1.60",
      refund_total: "11.60",
    },
  ],
};

function renderScreen(props: Partial<Parameters<typeof ReturnsScreen>[0]> = {}) {
  const onCashChanged = vi.fn();
  render(
    <ReturnsScreen
      actor={actor}
      readOnly={false}
      shift={shift}
      onCashChanged={onCashChanged}
      pendingAttempt={null}
      onPendingAttemptChange={vi.fn()}
      {...props}
    />,
  );
  return { onCashChanged };
}

/** Shell simulado: el intento sobrevive al desmontaje y al reinicio. */
function ReturnHarness({ storageKey }: { storageKey: string }) {
  const [attempt, setAttempt] = useState<PendingReturnAttempt | null>(() =>
    loadPendingAttempt(storageKey),
  );
  const [open, setOpen] = useState(true);
  const update = (next: PendingReturnAttempt | null) => {
    setAttempt(next);
    savePendingAttempt(storageKey, next);
  };
  return (
    <>
      <button type="button" onClick={() => setOpen((value) => !value)}>
        {open ? "Ir a Ventas" : "Ir a Devoluciones"}
      </button>
      {open && (
        <ReturnsScreen
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

async function prepareReturn(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText("Folio de venta"), "V-ABC");
  await user.click(screen.getByRole("button", { name: /buscar venta/i }));
  await screen.findByText("Venta V-ABC");
  await user.clear(screen.getByLabelText("Cantidad a devolver Café"));
  await user.type(screen.getByLabelText("Cantidad a devolver Café"), "1");
  await user.type(screen.getByLabelText("Motivo"), "producto en buen estado");
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((complete) => {
    resolve = complete;
  });
  return { promise, resolve };
}

describe("ReturnsScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    api.findSaleForReturn.mockResolvedValue(sale);
    api.previewSaleReturn.mockResolvedValue({
      refund_total: "11.60",
      lines: [{ sale_item_id: "item-1", quantity: "1", refund_total: "11.60" }],
    });
    api.createSaleReturn.mockResolvedValue(created);
    api.listSaleReturns.mockResolvedValue([]);
  });

  it("muestra el reembolso calculado por Rust y registra la devolución", async () => {
    const { onCashChanged } = renderScreen();
    const user = userEvent.setup();
    await prepareReturn(user);
    await user.click(screen.getByRole("button", { name: /calcular reembolso/i }));
    expect((await screen.findAllByText("$11.60")).length).toBeGreaterThan(0);
    await user.click(screen.getByRole("button", { name: /confirmar devolución/i }));
    await waitFor(() => expect(api.createSaleReturn).toHaveBeenCalledTimes(1));
    const draft = api.createSaleReturn.mock.calls[0]?.[0] as SaleReturnDraft;
    expect(draft).toMatchObject({
      sale_id: "sale-1",
      shift_id: "shift-1",
      reason: "producto en buen estado",
      lines: [{ sale_item_id: "item-1", quantity: "1", disposition: "restock" }],
    });
    expect(draft.idempotency_key).toBeTruthy();
    expect(onCashChanged).toHaveBeenCalled();
    expect(await screen.findByText(/reembolso \$11.60/i)).toBeVisible();
  });

  it("invalida la vista previa cuando cambian las partidas", async () => {
    renderScreen();
    const user = userEvent.setup();
    await prepareReturn(user);
    await user.click(screen.getByRole("button", { name: /calcular reembolso/i }));
    await waitFor(() => expect(screen.getAllByText("$11.60").length).toBeGreaterThan(0));
    expect(screen.getByRole("button", { name: /confirmar devolución/i })).toBeEnabled();

    await user.clear(screen.getByLabelText("Cantidad a devolver Café"));
    await user.type(screen.getByLabelText("Cantidad a devolver Café"), "2");

    expect(screen.getByText("Calcula antes de confirmar")).toBeVisible();
    expect(screen.getByRole("button", { name: /confirmar devolución/i })).toBeDisabled();
    expect(api.createSaleReturn).not.toHaveBeenCalled();
  });

  it("ignora una respuesta de preview obsoleta cuando se recalculan las partidas", async () => {
    const first = deferred<{ refund_total: string }>();
    const second = deferred<{ refund_total: string }>();
    api.previewSaleReturn.mockReset();
    api.previewSaleReturn.mockImplementationOnce(() => first.promise);
    api.previewSaleReturn.mockImplementationOnce(() => second.promise);
    renderScreen();
    const user = userEvent.setup();
    await prepareReturn(user);

    await user.click(screen.getByRole("button", { name: /calcular reembolso/i }));
    await waitFor(() => expect(api.previewSaleReturn).toHaveBeenCalledTimes(1));

    await user.clear(screen.getByLabelText("Cantidad a devolver Café"));
    await user.type(screen.getByLabelText("Cantidad a devolver Café"), "2");
    await user.click(screen.getByRole("button", { name: /calcular reembolso/i }));
    await waitFor(() => expect(api.previewSaleReturn).toHaveBeenCalledTimes(2));

    await act(async () => {
      first.resolve({ refund_total: "11.60" });
      await first.promise;
    });
    expect(screen.getByText("Calculando…")).toBeVisible();
    expect(screen.getByRole("button", { name: /confirmar devolución/i })).toBeDisabled();

    await act(async () => {
      second.resolve({ refund_total: "23.20" });
      await second.promise;
    });
    await waitFor(() => expect(screen.getByText("$23.20")).toBeVisible());
    expect(screen.getByRole("button", { name: /confirmar devolución/i })).toBeEnabled();
  });

  it("bloquea el cobro sin turno abierto", async () => {
    renderScreen({ shift: null });
    const user = userEvent.setup();
    await prepareReturn(user);
    expect(screen.getByRole("button", { name: /confirmar devolución/i })).toBeDisabled();
    expect(screen.getByText(/abre un turno de caja/i)).toBeVisible();
  });

  it("permite restituir inventario sin turno cuando el reembolso es de cero", async () => {
    api.previewSaleReturn.mockResolvedValueOnce({
      refund_total: "0.00",
      lines: [{ sale_item_id: "item-1", quantity: "0.001", refund_total: "0.00" }],
    });
    api.createSaleReturn.mockResolvedValueOnce({ ...created, refund_total: "0.00" });
    renderScreen({ shift: null });
    const user = userEvent.setup();
    await prepareReturn(user);
    expect(screen.getByRole("button", { name: /confirmar devolución/i })).toBeDisabled();
    await user.click(screen.getByRole("button", { name: /calcular reembolso/i }));
    expect(await screen.findByText(/solo se restituirá inventario/i)).toBeVisible();
    expect(screen.getByRole("button", { name: /confirmar devolución/i })).toBeEnabled();
    await user.click(screen.getByRole("button", { name: /confirmar devolución/i }));
    await waitFor(() => expect(api.createSaleReturn).toHaveBeenCalledTimes(1));
    const draft = api.createSaleReturn.mock.calls[0]?.[0] as SaleReturnDraft;
    expect(draft.shift_id).toBeUndefined();
  });

  it("conserva el intento incierto al navegar y lo reintenta igual", async () => {
    const { IpcError } = await import("../api");
    api.createSaleReturn
      .mockRejectedValueOnce(new IpcError("unknown_error", "respuesta perdida"))
      .mockResolvedValueOnce(created);
    const key = pendingAttemptStorageKey("return", actor);
    render(<ReturnHarness storageKey={key} />);
    const user = userEvent.setup();
    await prepareReturn(user);
    await user.click(screen.getByRole("button", { name: /calcular reembolso/i }));
    await user.click(screen.getByRole("button", { name: /confirmar devolución/i }));
    expect(await screen.findByText(/devolución pendiente de confirmar/i)).toBeVisible();
    const first = api.createSaleReturn.mock.calls[0]?.[0] as SaleReturnDraft;

    await user.click(screen.getByRole("button", { name: "Ir a Ventas" }));
    expect(screen.queryByText(/devolución pendiente de confirmar/i)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Ir a Devoluciones" }));
    expect(await screen.findByText(/devolución pendiente de confirmar/i)).toBeVisible();
    await user.click(screen.getByRole("button", { name: /reintentar devolución/i }));
    await waitFor(() => expect(api.createSaleReturn).toHaveBeenCalledTimes(2));
    expect(api.createSaleReturn.mock.calls[1]?.[0]).toEqual(first);
  });

  it("recupera el intento después de reiniciar la aplicación", async () => {
    const { IpcError } = await import("../api");
    api.createSaleReturn
      .mockRejectedValueOnce(new IpcError("storage_error", "sin respuesta"))
      .mockResolvedValueOnce(created);
    const key = pendingAttemptStorageKey("return", actor);
    const first = render(<ReturnHarness storageKey={key} />);
    const user = userEvent.setup();
    await prepareReturn(user);
    await user.click(screen.getByRole("button", { name: /calcular reembolso/i }));
    await user.click(screen.getByRole("button", { name: /confirmar devolución/i }));
    await screen.findByText(/devolución pendiente de confirmar/i);
    const original = api.createSaleReturn.mock.calls[0]?.[0] as SaleReturnDraft;
    first.unmount();

    render(<ReturnHarness storageKey={key} />);
    expect(await screen.findByText(/devolución pendiente de confirmar/i)).toBeVisible();
    await userEvent.setup().click(screen.getByRole("button", { name: /reintentar devolución/i }));
    await waitFor(() => expect(api.createSaleReturn).toHaveBeenCalledTimes(2));
    expect(api.createSaleReturn.mock.calls[1]?.[0]).toEqual(original);
  });

  it("no recupera intentos de otra identidad y respeta permisos", async () => {
    const otherKey = pendingAttemptStorageKey("return", {
      ...actor,
      organization_id: "o-2",
      user_id: "u-9",
    });
    savePendingAttempt<SaleReturnDraft>(otherKey, {
      fingerprint: "ajena",
      payload: {
        sale_id: "sale-1",
        shift_id: "shift-1",
        reason: "ajena",
        idempotency_key: "ajena-1",
        lines: [{ sale_item_id: "item-1", quantity: "1", disposition: "restock" }],
      },
    });
    renderScreen();
    expect(screen.queryByText(/devolución pendiente de confirmar/i)).not.toBeInTheDocument();

    renderScreen({ actor: { ...actor, role: "cashier", permissions: ["sales.sell"] } });
    expect(screen.getAllByText(/no tienes permiso/i).length).toBeGreaterThan(0);
  });
});
