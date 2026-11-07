import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, InventoryMovement, Product, WasteDraft } from "../contracts";
import {
  loadPendingAttempt,
  pendingAttemptStorageKey,
  savePendingAttempt,
  type PendingWasteAttempt,
} from "../lib/pendingAttempt";

const api = vi.hoisted(() => ({
  currentStock: vi.fn(),
  listInventoryMovements: vi.fn(),
  recordInventoryMovement: vi.fn(),
  recordWaste: vi.fn(),
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

import { InventoryScreen } from "./InventoryScreen";

const actor: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana",
  role: "owner",
  permissions: ["inventory.view", "inventory.manage"],
};

/** Owner con permiso de merma: el formulario convive con el de movimientos. */
const wasteActor: Actor = {
  ...actor,
  permissions: [...actor.permissions, "waste.manage"],
};

const product: Product = {
  id: "p-1",
  sku: "CAF-001",
  name: "Café americano",
  unit_price: "12.50",
  public_unit_total: "12.50",
  tax_rate_bp: 1600,
  active: true,
  active_variants: 0,
  variant_public_from: null,
  has_modifiers: false,
};

const movement: InventoryMovement = {
  id: "m-1",
  product_id: "p-1",
  kind: "inbound",
  quantity: "20",
  reason: null,
  occurred_at: "2026-11-04T08:00:00Z",
};

const storedWaste = {
  id: "w-1",
  product_id: "p-1",
  quantity: "1",
  reason: "daño",
  occurred_at: "2026-11-04T08:00:00Z",
};

function renderScreen(props: Partial<Parameters<typeof InventoryScreen>[0]> = {}) {
  const onStockChanged = vi.fn();
  render(
    <InventoryScreen
      actor={actor}
      readOnly={false}
      products={[product]}
      loading={false}
      onStockChanged={onStockChanged}
      pendingWaste={null}
      onPendingWasteChange={vi.fn()}
      {...props}
    />,
  );
  return { onStockChanged };
}

/** Reproduce el shell: el intento sobrevive al desmontaje de la pantalla. */
function WasteHarness({ storageKey }: { storageKey: string }) {
  const [attempt, setAttempt] = useState<PendingWasteAttempt | null>(() =>
    loadPendingAttempt(storageKey),
  );
  const [open, setOpen] = useState(true);
  const update = (next: PendingWasteAttempt | null) => {
    setAttempt(next);
    savePendingAttempt(storageKey, next);
  };
  return (
    <>
      <button type="button" onClick={() => setOpen((value) => !value)}>
        {open ? "Ir a Compras" : "Ir a Inventario"}
      </button>
      {open && (
        <InventoryScreen
          actor={wasteActor}
          readOnly={false}
          products={[product]}
          loading={false}
          onStockChanged={vi.fn()}
          pendingWaste={attempt}
          onPendingWasteChange={update}
        />
      )}
    </>
  );
}

async function submitWaste(user: ReturnType<typeof userEvent.setup>) {
  await user.selectOptions(screen.getByLabelText("Producto"), "p-1");
  await screen.findByText("20", { selector: ".stock-figure" });
  const form = screen.getByRole("heading", { name: "Registrar merma" }).closest("form");
  if (!form) throw new Error("no se encontró el formulario de merma");
  await user.type(
    Array.from(form.querySelectorAll("input"))[0] as HTMLInputElement,
    "1",
  );
  await user.selectOptions(
    Array.from(form.querySelectorAll("select"))[0] as HTMLSelectElement,
    "daño",
  );
  await user.click(screen.getByRole("button", { name: /^registrar merma$/i }));
}

describe("InventoryScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    window.localStorage.clear();
    api.currentStock.mockResolvedValue("20.000");
    api.listInventoryMovements.mockResolvedValue([movement]);
    api.recordInventoryMovement.mockResolvedValue(movement);
    api.recordWaste.mockResolvedValue(storedWaste);
  });

  it("muestra existencias y historial del producto seleccionado", async () => {
    renderScreen();
    const user = userEvent.setup();

    await user.selectOptions(screen.getByLabelText("Producto"), "p-1");

    expect(await screen.findByText("20", { selector: ".stock-figure" })).toBeVisible();
    expect(screen.getByText("Al público")).toBeVisible();
    expect(screen.getByText("$12.50")).toBeVisible();
    expect(await screen.findByText(/sin motivo/i)).toBeVisible();
    expect(api.listInventoryMovements).toHaveBeenCalledWith("p-1", 50);
  });

  it("repetir la misma selección no deja la lectura en curso", async () => {
    renderScreen();
    const user = userEvent.setup();
    const select = screen.getByLabelText("Producto");

    await user.selectOptions(select, "p-1");
    expect(await screen.findByText("20", { selector: ".stock-figure" })).toBeVisible();

    // Volver a elegir la misma opción no debe vaciar el panel ni dejar "…".
    await user.selectOptions(select, "p-1");

    expect(screen.getByText("20", { selector: ".stock-figure" })).toBeVisible();
    expect(screen.queryByText("…")).not.toBeInTheDocument();
    expect(api.currentStock).toHaveBeenCalledTimes(1);
  });

  it("registra una entrada y refresca existencias", async () => {
    const { onStockChanged } = renderScreen();
    const user = userEvent.setup();

    await user.selectOptions(screen.getByLabelText("Producto"), "p-1");
    await user.type(screen.getByLabelText("Cantidad"), "5");
    await user.click(screen.getByRole("button", { name: /registrar movimiento/i }));

    await waitFor(() =>
      expect(api.recordInventoryMovement).toHaveBeenCalledWith({
        product_id: "p-1",
        kind: "inbound",
        quantity: "5",
        reason: undefined,
      }),
    );
    expect(await screen.findByText(/movimiento registrado/i)).toBeVisible();
    expect(onStockChanged).toHaveBeenCalled();
  });

  it("exige motivo en ajustes", async () => {
    renderScreen();
    const user = userEvent.setup();

    await user.selectOptions(screen.getByLabelText("Producto"), "p-1");
    await user.selectOptions(screen.getByLabelText("Tipo"), "adjustment_in");
    await user.type(screen.getByLabelText("Cantidad"), "3");
    await user.click(screen.getByRole("button", { name: /registrar movimiento/i }));

    expect(await screen.findByText(/los ajustes requieren un motivo/i)).toBeVisible();
    expect(api.recordInventoryMovement).not.toHaveBeenCalled();
  });

  it("muestra error de IPC al consultar", async () => {
    const { IpcError } = await import("../api");
    api.currentStock.mockRejectedValueOnce(new IpcError("not_found", "producto inexistente"));
    renderScreen();
    const user = userEvent.setup();

    await user.selectOptions(screen.getByLabelText("Producto"), "p-1");

    expect(await screen.findByText(/producto inexistente/i)).toBeVisible();
  });

  it("oculta el formulario sin permiso de inventario", async () => {
    renderScreen({ actor: { ...actor, permissions: ["inventory.view"] }, readOnly: true });
    const user = userEvent.setup();

    await user.selectOptions(screen.getByLabelText("Producto"), "p-1");
    await screen.findByText("20", { selector: ".stock-figure" });

    expect(screen.queryByRole("button", { name: /^registrar$/i })).not.toBeInTheDocument();
  });

  it("conserva la merma incierta al navegar y la reintenta con la misma clave", async () => {
    const { IpcError } = await import("../api");
    api.recordWaste
      .mockRejectedValueOnce(new IpcError("unknown_error", "respuesta perdida"))
      .mockResolvedValueOnce(storedWaste);
    const key = pendingAttemptStorageKey("waste", wasteActor);
    render(<WasteHarness storageKey={key} />);
    const user = userEvent.setup();
    await submitWaste(user);

    expect(await screen.findByText(/merma pendiente de confirmar/i)).toBeVisible();
    expect(screen.queryByRole("heading", { name: "Registrar merma" })).not.toBeInTheDocument();
    const first = api.recordWaste.mock.calls[0]?.[0] as WasteDraft;

    await user.click(screen.getByRole("button", { name: "Ir a Compras" }));
    expect(screen.queryByText(/merma pendiente de confirmar/i)).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Ir a Inventario" }));
    expect(await screen.findByText(/merma pendiente de confirmar/i)).toBeVisible();

    await user.click(screen.getByRole("button", { name: /reintentar merma/i }));
    await waitFor(() => expect(api.recordWaste).toHaveBeenCalledTimes(2));
    expect(api.recordWaste.mock.calls[1]?.[0]).toEqual(first);
  });

  it("recupera la merma incierta después de reiniciar la aplicación", async () => {
    const { IpcError } = await import("../api");
    api.recordWaste
      .mockRejectedValueOnce(new IpcError("storage_error", "sin respuesta"))
      .mockResolvedValueOnce(storedWaste);
    const key = pendingAttemptStorageKey("waste", wasteActor);
    const first = render(<WasteHarness storageKey={key} />);
    const user = userEvent.setup();
    await submitWaste(user);
    await screen.findByText(/merma pendiente de confirmar/i);
    const original = api.recordWaste.mock.calls[0]?.[0] as WasteDraft;
    first.unmount();

    render(<WasteHarness storageKey={key} />);
    expect(await screen.findByText(/merma pendiente de confirmar/i)).toBeVisible();
    expect(screen.getByText("daño")).toBeVisible();
    await userEvent.setup().click(screen.getByRole("button", { name: /reintentar merma/i }));
    await waitFor(() => expect(api.recordWaste).toHaveBeenCalledTimes(2));
    expect(api.recordWaste.mock.calls[1]?.[0]).toEqual(original);
  });

  it("no recupera mermas pertenecientes a otra identidad", async () => {
    const otherKey = pendingAttemptStorageKey("waste", {
      ...wasteActor,
      organization_id: "o-2",
      user_id: "u-9",
    });
    savePendingAttempt<WasteDraft>(otherKey, {
      fingerprint: "ajena",
      payload: {
        product_id: "p-1",
        quantity: "9",
        reason: "caducidad",
        idempotency_key: "ajena-1",
      },
    });
    renderScreen({ actor: wasteActor });
    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText("Producto"), "p-1");
    await screen.findByText("20", { selector: ".stock-figure" });
    expect(screen.queryByText(/merma pendiente de confirmar/i)).not.toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Registrar merma" })).toBeVisible();
  });
});
