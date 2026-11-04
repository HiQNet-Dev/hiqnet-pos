import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, InventoryMovement, Product } from "../contracts";

const api = vi.hoisted(() => ({
  currentStock: vi.fn(),
  listInventoryMovements: vi.fn(),
  recordInventoryMovement: vi.fn(),
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

const product: Product = {
  id: "p-1",
  sku: "CAF-001",
  name: "Café americano",
  unit_price: "12.50",
  tax_rate_bp: 1600,
  active: true,
};

const movement: InventoryMovement = {
  id: "m-1",
  product_id: "p-1",
  kind: "inbound",
  quantity: "20",
  reason: null,
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
      {...props}
    />,
  );
  return { onStockChanged };
}

describe("InventoryScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.currentStock.mockResolvedValue("20.000");
    api.listInventoryMovements.mockResolvedValue([movement]);
    api.recordInventoryMovement.mockResolvedValue(movement);
  });

  it("muestra existencias y historial del producto seleccionado", async () => {
    renderScreen();
    const user = userEvent.setup();

    await user.selectOptions(screen.getByLabelText("Producto"), "p-1");

    expect(await screen.findByText("20", { selector: ".stock-figure" })).toBeVisible();
    expect(await screen.findByText(/sin motivo/i)).toBeVisible();
    expect(api.listInventoryMovements).toHaveBeenCalledWith("p-1", 50);
  });

  it("registra una entrada y refresca existencias", async () => {
    const { onStockChanged } = renderScreen();
    const user = userEvent.setup();

    await user.selectOptions(screen.getByLabelText("Producto"), "p-1");
    await user.type(screen.getByLabelText("Cantidad"), "5");
    await user.click(screen.getByRole("button", { name: /^registrar$/i }));

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
    await user.click(screen.getByRole("button", { name: /^registrar$/i }));

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
});
