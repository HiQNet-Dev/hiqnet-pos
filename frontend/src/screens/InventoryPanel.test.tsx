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

const readOnlyActor: Actor = {
  ...actor,
  permissions: ["inventory.view"],
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
  {
    id: "p-2",
    sku: "JUG-001",
    name: "Jugo de naranja",
    unit_price: "20.00",
    tax_rate_bp: 1600,
    active: true,
  },
];

const movementResult = {
  id: "m-1",
  product_id: "p-1",
  kind: "inbound",
  quantity: "2",
  reason: null,
  occurred_at: "2026-11-03T00:00:00Z",
};

describe("InventoryPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.currentStock.mockResolvedValue("5");
    api.listInventoryMovements.mockResolvedValue([]);
    api.recordInventoryMovement.mockResolvedValue(movementResult);
    vi.spyOn(window, "confirm").mockReturnValue(true);
  });

  it("consulta existencias al seleccionar un producto", async () => {
    render(<InventoryPanel actor={actor} products={products} readOnly={false} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");

    await waitFor(() => {
      expect(screen.getByText(/café americano: 5/i)).toBeVisible();
    });
  });

  it("envía una entrada válida y refresca el stock", async () => {
    render(<InventoryPanel actor={actor} products={products} readOnly={false} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");
    await user.type(screen.getByLabelText(/cantidad/i), "2");
    await user.click(screen.getByRole("button", { name: /registrar/i }));

    await waitFor(() => {
      expect(api.recordInventoryMovement).toHaveBeenCalledWith({
        product_id: "p-1",
        kind: "inbound",
        quantity: "2",
        reason: undefined,
      });
      expect(api.currentStock).toHaveBeenCalledTimes(2);
    });
  });

  it("envía una salida confirmada", async () => {
    render(<InventoryPanel actor={actor} products={products} readOnly={false} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");
    await user.selectOptions(screen.getByLabelText(/tipo/i), "outbound");
    await user.type(screen.getByLabelText(/cantidad/i), "1");
    await user.click(screen.getByRole("button", { name: /registrar/i }));

    await waitFor(() => {
      expect(api.recordInventoryMovement).toHaveBeenCalledWith({
        product_id: "p-1",
        kind: "outbound",
        quantity: "1",
        reason: undefined,
      });
    });
    expect(window.confirm).toHaveBeenCalled();
  });

  it("exige motivo en ajustes", async () => {
    render(<InventoryPanel actor={actor} products={products} readOnly={false} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");
    await user.selectOptions(screen.getByLabelText(/tipo/i), "adjustment_in");
    await user.type(screen.getByLabelText(/cantidad/i), "1");
    await user.click(screen.getByRole("button", { name: /registrar/i }));

    expect(await screen.findByText(/requieren un motivo/i)).toBeVisible();
    expect(api.recordInventoryMovement).not.toHaveBeenCalled();
  });

  it("rechaza cantidades inválidas", async () => {
    render(<InventoryPanel actor={actor} products={products} readOnly={false} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");
    await user.type(screen.getByLabelText(/cantidad/i), "abc");
    await user.click(screen.getByRole("button", { name: /registrar/i }));

    expect(api.recordInventoryMovement).not.toHaveBeenCalled();
  });

  it("muestra el error del backend", async () => {
    const { IpcError } = await import("../api");
    api.recordInventoryMovement.mockRejectedValueOnce(
      new IpcError("validation_error", "stock insuficiente"),
    );

    render(<InventoryPanel actor={actor} products={products} readOnly={false} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");
    await user.type(screen.getByLabelText(/cantidad/i), "2");
    await user.click(screen.getByRole("button", { name: /registrar/i }));

    expect(await screen.findByText(/stock insuficiente/i)).toBeVisible();
  });

  it("oculta el formulario sin permiso de modificación", async () => {
    render(
      <InventoryPanel actor={readOnlyActor} products={products} readOnly={false} />,
    );

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");

    expect(
      screen.queryByRole("button", { name: /registrar/i }),
    ).not.toBeInTheDocument();
  });

  it("oculta el formulario con licencia vencida", async () => {
    render(<InventoryPanel actor={actor} products={products} readOnly={true} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");

    expect(
      screen.queryByRole("button", { name: /registrar/i }),
    ).not.toBeInTheDocument();
  });

  it("no duplica el envío mientras la operación está pendiente", async () => {
    let resolveMovement: (value: typeof movementResult) => void = () => {};
    api.recordInventoryMovement.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveMovement = resolve;
        }),
    );

    render(<InventoryPanel actor={actor} products={products} readOnly={false} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");
    await user.type(screen.getByLabelText(/cantidad/i), "2");
    const button = screen.getByRole("button", { name: /registrar/i });
    await user.click(button);
    await user.click(button);

    expect(api.recordInventoryMovement).toHaveBeenCalledTimes(1);
    resolveMovement(movementResult);
    await waitFor(() => expect(screen.getByRole("button", { name: /registrar/i })).toBeEnabled());
  });

  it("descarta respuestas obsoletas al cambiar de producto", async () => {
    let resolveFirst: (value: string) => void = () => {};
    api.currentStock
      .mockImplementationOnce(() => new Promise((resolve) => { resolveFirst = resolve; }))
      .mockResolvedValueOnce("9");

    render(<InventoryPanel actor={actor} products={products} readOnly={false} />);

    const user = userEvent.setup();
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-1");
    await user.selectOptions(screen.getByLabelText(/producto/i), "p-2");
    resolveFirst("1");

    await waitFor(() => {
      expect(screen.getByText(/jugo de naranja: 9/i)).toBeVisible();
    });
    expect(screen.queryByText(/café americano: 1/i)).not.toBeInTheDocument();
  });
});
