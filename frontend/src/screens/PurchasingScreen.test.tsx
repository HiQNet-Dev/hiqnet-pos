import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { useState } from "react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, Product, PurchaseOrder, Supplier } from "../contracts";

const api = vi.hoisted(() => ({
  listSuppliers: vi.fn(), listPurchaseOrders: vi.fn(), createSupplier: vi.fn(),
  createPurchaseOrder: vi.fn(), confirmPurchaseOrder: vi.fn(), receivePurchaseOrder: vi.fn(),
}));
vi.mock("../api", () => ({ api, IpcError: class IpcError extends Error { constructor(code: string, message: string) { super(message); this.code = code; } code: string; } }));
import { PurchasingScreen } from "./PurchasingScreen";

const actor: Actor = { user_id: "u-1", organization_id: "o-1", branch_id: "b-1", display_name: "Ana", role: "owner", permissions: ["purchasing.manage"] };
const product: Product = { id: "p-1", sku: "INS-01", name: "Insumo", unit_price: "10.00", tax_rate_bp: 0, active: true };
const supplier: Supplier = { id: "s-1", code: "SUP-1", name: "Proveedor Uno", contact: null, rfc: null, active: true };
const order: PurchaseOrder = { id: "o-1", supplier_id: "s-1", folio: "OC-1", status: "ordered", created_at: "2026-11-04T00:00:00Z", lines: [{ id: "l-1", product_id: "p-1", quantity: "5", received_quantity: "0", unit_cost: "8.00" }] };

function renderScreen(permissions = actor.permissions) {
  return render(<PurchasingScreen actor={{ ...actor, permissions }} readOnly={false} products={[product]} onInventoryChanged={vi.fn()} pendingReceipt={null} onPendingReceiptChange={vi.fn()} />);
}

function ReceiptAttemptHarness() {
  const [pendingReceipt, setPendingReceipt] = useState<import("./PurchasingScreen").PendingReceiptAttempt | null>(null);
  const [purchasingOpen, setPurchasingOpen] = useState(true);
  return <>
    <button type="button" onClick={() => setPurchasingOpen(false)}>Ir a Ventas</button>
    <button type="button" onClick={() => setPurchasingOpen(true)}>Ir a Compras</button>
    {purchasingOpen && <PurchasingScreen actor={actor} readOnly={false} products={[product]} onInventoryChanged={vi.fn()} pendingReceipt={pendingReceipt} onPendingReceiptChange={setPendingReceipt} />}
  </>;
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

  it("reintenta una recepción incierta con la misma clave y payload", async () => {
    const { IpcError } = await import("../api");
    api.receivePurchaseOrder
      .mockRejectedValueOnce(new IpcError("unknown_error", "respuesta perdida"))
      .mockResolvedValueOnce({ ...order, status: "partially_received" });
    render(<ReceiptAttemptHarness />);
    const user = userEvent.setup();
    await screen.findByText("Proveedor Uno");
    await user.click(screen.getByRole("button", { name: "Recibir" }));
    await user.clear(screen.getByLabelText("Recibir"));
    await user.type(screen.getByLabelText("Recibir"), "2");
    await user.click(screen.getByRole("button", { name: /confirmar recepción/i }));
    expect(await screen.findByText(/no se confirmó el resultado/i)).toBeVisible();
    const first = api.receivePurchaseOrder.mock.calls[0]?.[0];
    expect(screen.getByLabelText("Recibir")).toBeDisabled();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: /recibir oc-1/i })).not.toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent(/recepción pendiente/i);
    await user.click(screen.getByRole("button", { name: "Ir a Ventas" }));
    expect(screen.queryByText("Órdenes de compra")).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Ir a Compras" }));
    await user.click(await screen.findByRole("button", { name: "Recibir" }));
    expect(screen.getByLabelText("Recibir")).toBeDisabled();
    await user.click(screen.getByRole("button", { name: /reintentar recepción/i }));
    await waitFor(() => expect(api.receivePurchaseOrder).toHaveBeenCalledTimes(2));
    expect(api.receivePurchaseOrder.mock.calls[1]?.[0]).toEqual(first);
  });

  it("conserva el intento incierto al cerrar con botón o backdrop", async () => {
    const { IpcError } = await import("../api");
    api.receivePurchaseOrder.mockRejectedValueOnce(new IpcError("storage_error", "resultado incierto"));
    render(<ReceiptAttemptHarness />);
    const user = userEvent.setup();
    await screen.findByText("Proveedor Uno");
    await user.click(screen.getByRole("button", { name: "Recibir" }));
    await user.clear(screen.getByLabelText("Recibir"));
    await user.type(screen.getByLabelText("Recibir"), "2");
    await user.click(screen.getByRole("button", { name: /confirmar recepción/i }));
    await screen.findByText(/no se confirmó el resultado/i);
    await user.click(screen.getByRole("button", { name: "Cerrar" }));
    await user.click(screen.getByRole("button", { name: "Recibir" }));
    expect(screen.getByLabelText("Recibir")).toBeDisabled();
    fireEvent.mouseDown(document.querySelector(".modal-backdrop")!);
    await user.click(screen.getByRole("button", { name: "Recibir" }));
    expect(screen.getByLabelText("Recibir")).toBeDisabled();
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
