import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, Product, PurchaseOrder, Supplier } from "../contracts";

const api = vi.hoisted(() => ({
  listSuppliers: vi.fn(), listPurchaseOrders: vi.fn(), createSupplier: vi.fn(),
  createPurchaseOrder: vi.fn(), confirmPurchaseOrder: vi.fn(), receivePurchaseOrder: vi.fn(),
}));
vi.mock("../api", () => ({ api, IpcError: class IpcError extends Error { code = "conflict"; } }));
import { PurchasingScreen } from "./PurchasingScreen";

const actor: Actor = { user_id: "u-1", organization_id: "o-1", branch_id: "b-1", display_name: "Ana", role: "owner", permissions: ["purchasing.manage"] };
const product: Product = { id: "p-1", sku: "INS-01", name: "Insumo", unit_price: "10.00", tax_rate_bp: 0, active: true };
const supplier: Supplier = { id: "s-1", code: "SUP-1", name: "Proveedor Uno", contact: null, rfc: null, active: true };
const order: PurchaseOrder = { id: "o-1", supplier_id: "s-1", folio: "OC-1", status: "ordered", created_at: "2026-11-04T00:00:00Z", lines: [{ id: "l-1", product_id: "p-1", quantity: "5", received_quantity: "0", unit_cost: "8.00" }] };

function renderScreen(permissions = actor.permissions) {
  return render(<PurchasingScreen actor={{ ...actor, permissions }} readOnly={false} products={[product]} onInventoryChanged={vi.fn()} />);
}

describe("PurchasingScreen", () => {
  beforeEach(() => { vi.clearAllMocks(); api.listSuppliers.mockResolvedValue([supplier]); api.listPurchaseOrders.mockResolvedValue([order]); api.createSupplier.mockResolvedValue(supplier); api.confirmPurchaseOrder.mockResolvedValue({ ...order, status: "ordered" }); });

  it("muestra proveedores y confirma una orden sin exponer controles a cajeros", async () => {
    const first = renderScreen();
    expect(await screen.findByText("Proveedor Uno")).toBeVisible();
    expect(screen.getByText("Confirmada")).toBeVisible();
    await userEvent.setup().click(screen.getByRole("button", { name: "Recibir" }));
    expect(await screen.findByRole("dialog", { name: /recibir oc-1/i })).toBeVisible();
    first.unmount();
    renderScreen([]);
    expect(await screen.findByText(/no tienes permiso/i)).toBeVisible();
  });

  it("crea un proveedor y refresca la lista", async () => {
    renderScreen(); const user = userEvent.setup();
    await screen.findByText("Proveedor Uno");
    await user.click(screen.getByRole("button", { name: /nuevo proveedor/i }));
    await user.type(screen.getByLabelText("Nombre"), "Proveedor nuevo");
    await user.click(screen.getByRole("button", { name: /crear proveedor/i }));
    await waitFor(() => expect(api.createSupplier).toHaveBeenCalledWith(expect.objectContaining({ name: "Proveedor nuevo" })));
    expect(await screen.findByText(/proveedor creado/i)).toBeVisible();
  });
});
