import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, Product, SaleView } from "../contracts";

const api = vi.hoisted(() => ({
  listProducts: vi.fn(),
  listCategories: vi.fn(),
  listCashRegisters: vi.fn(),
  activeCashShift: vi.fn(),
  listProductVariants: vi.fn(),
  listModifierGroups: vi.fn(),
  listModifierOptions: vi.fn(),
  configureProduct: vi.fn(),
  checkoutSale: vi.fn(),
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

import { SalesPanel } from "./SalesPanel";

const actor: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana",
  role: "owner",
  permissions: ["sales.sell"],
};

const product: Product = {
  id: "p-1",
  sku: "CAF-001",
  name: "Café",
  unit_price: "10.00",
  tax_rate_bp: 1600,
  active: true,
};

const sale: SaleView = {
  id: "sale-1",
  folio: "V-ABC",
  status: "completed",
  subtotal: "10.00",
  tax: "1.60",
  total: "11.60",
  items: [],
  payment: { method: "cash", amount: "20.00", change: "8.40" },
};

describe("SalesPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.listProducts.mockResolvedValue([product]);
    api.listCategories.mockResolvedValue([]);
    api.listCashRegisters.mockResolvedValue([{ id: "r-1", name: "Caja", code: "CJ-01", active: true }]);
    api.activeCashShift.mockResolvedValue({ id: "sh-1", register_id: "r-1", status: "open" });
    api.listProductVariants.mockResolvedValue([]);
    api.listModifierGroups.mockResolvedValue([]);
    api.listModifierOptions.mockResolvedValue([]);
    api.configureProduct.mockResolvedValue({
      product_id: "p-1",
      product_name: "Café",
      variant_id: null,
      variant_name: null,
      base_price: "10.00",
      modifiers_total: "0.00",
      tax: "1.60",
      unit_price: "11.60",
      tax_rate_bp: 1600,
      selected_options: [],
    });
    api.checkoutSale.mockResolvedValue(sale);
  });

  it("agrega un producto y muestra el carrito", async () => {
    render(<SalesPanel actor={actor} readOnly={false} />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /café/i }));
    await user.click(screen.getByRole("button", { name: /agregar al carrito/i }));
    expect(await screen.findByText(/1 partidas/i)).toBeVisible();
    expect(screen.getByText("$11.60")).toBeVisible();
  });

  it("incrementa y elimina cantidades", async () => {
    render(<SalesPanel actor={actor} readOnly={false} />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /café/i }));
    await user.click(screen.getByRole("button", { name: /agregar al carrito/i }));
    await user.click(screen.getByRole("button", { name: "+" }));
    expect(screen.getByText(/× 2/)).toBeVisible();
    await user.click(screen.getByRole("button", { name: /quitar/i }));
    expect(screen.getByText(/carrito vacío/i)).toBeVisible();
  });

  it("confirma una venta y muestra folio", async () => {
    render(<SalesPanel actor={actor} readOnly={false} />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /café/i }));
    await user.click(screen.getByRole("button", { name: /agregar al carrito/i }));
    await user.type(screen.getByLabelText(/efectivo recibido/i), "20.00");
    await user.click(screen.getByRole("button", { name: /cobrar/i }));
    await waitFor(() => expect(api.checkoutSale).toHaveBeenCalled());
    expect((await screen.findAllByText(/V-ABC/)).length).toBeGreaterThan(0);
  });

  it("muestra error de IPC", async () => {
    const { IpcError } = await import("../api");
    api.checkoutSale.mockRejectedValueOnce(new IpcError("validation_error", "pago insuficiente"));
    render(<SalesPanel actor={actor} readOnly={false} />);
    const user = userEvent.setup();
    await user.click(await screen.findByRole("button", { name: /café/i }));
    await user.click(screen.getByRole("button", { name: /agregar al carrito/i }));
    await user.type(screen.getByLabelText(/efectivo recibido/i), "1.00");
    await user.click(screen.getByRole("button", { name: /cobrar/i }));
    expect(await screen.findByText(/pago insuficiente/i)).toBeVisible();
  });

  it("no muestra punto de venta sin permiso", () => {
    render(
      <SalesPanel
        actor={{ ...actor, permissions: [] }}
        readOnly={false}
      />,
    );
    expect(screen.queryByRole("heading", { name: /punto de venta/i })).not.toBeInTheDocument();
  });
});