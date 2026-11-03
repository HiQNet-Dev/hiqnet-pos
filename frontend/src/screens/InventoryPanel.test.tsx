import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, Product } from "../contracts";

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

import { InventoryPanel } from "./InventoryPanel";

const actor: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana Propietaria",
  role: "owner",
  permissions: ["inventory.view", "inventory.manage"],
};

const products: Product[] = [
  {
    id: "p-1",
    sku: "CAF-001",
    name: "Café americano",
    unit_price: "12.50",
    tax_rate_bp: 1600,
    active: true,
  },
];

describe("InventoryPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.currentStock.mockResolvedValue("5");
    api.listInventoryMovements.mockResolvedValue([]);
  });

  it("consulta existencias al seleccionar un producto", async () => {
    render(<InventoryPanel actor={actor} products={products} readOnly={false} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");

    await waitFor(() => {
      expect(screen.getByText(/café americano: 5/i)).toBeVisible();
    });
  });

  it("envía una entrada válida y refresca", async () => {
    api.recordInventoryMovement.mockResolvedValue({
      id: "m-1",
      product_id: "p-1",
      kind: "inbound",
      quantity: "2",
      reason: null,
      occurred_at: "2026-11-03T00:00:00Z",
    });

    render(<InventoryPanel actor={actor} products={products} readOnly={false} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");
    await user.type(screen.getByLabelText(/cantidad/i), "2");
    await user.click(screen.getByRole("button", { name: /registrar/i }));

    await waitFor(() => {
      expect(api.recordInventoryMovement).toHaveBeenCalled();
    });
  });
});