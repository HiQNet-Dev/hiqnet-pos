import { useState } from "react";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { handleBackButton } from "../lib/backNavigation";
import type {
  Actor,
  CashShift,
  Category,
  ModifierGroup,
  ModifierOption,
  Product,
  SaleView,
} from "../contracts";
import type { CartLine } from "../lib/cart";
import {
  loadPendingAttempt,
  pendingAttemptStorageKey,
  savePendingAttempt,
  type PendingSaleAttempt,
} from "../lib/pendingAttempt";

const api = vi.hoisted(() => ({
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

import { SalesScreen } from "./SalesScreen";

const owner: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana",
  role: "owner",
  permissions: ["sales.sell"],
};

const category: Category = { id: "c-1", name: "Bebidas", sort_order: 1, active: true };

const product: Product = {
  id: "p-1",
  sku: "CAF-001",
  name: "Café americano",
  unit_price: "12.50",
  tax_rate_bp: 1600,
  active: true,
  category_id: "c-1",
};

const latte: Product = {
  id: "p-2",
  sku: "LAT-001",
  name: "Latte",
  unit_price: "15.00",
  tax_rate_bp: 1600,
  active: true,
  category_id: "c-1",
};

const variant = {
  id: "v-1",
  product_id: "p-2",
  name: "Grande",
  sku: null,
  unit_price: "18.00",
  active: true,
  sort_order: 0,
};

const group: ModifierGroup = {
  id: "g-1",
  name: "Extras",
  min: 0,
  max: 1,
  sort_order: 0,
  active: true,
};

const extra: ModifierOption = {
  id: "o-1",
  group_id: "g-1",
  name: "Shot extra",
  kind: "extra",
  price_delta: "2.00",
  sort_order: 0,
  active: true,
};

const shift: CashShift = {
  id: "sh-1",
  register_id: "r-1",
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

const sale: SaleView = {
  id: "s-1",
  folio: "V-ABC123",
  status: "completed",
  subtotal: "12.50",
  tax: "2.00",
  total: "14.50",
  items: [],
  payment: { method: "cash", amount: "20.00", change: "5.50" },
};

const configuration = {
  product_id: "p-1",
  product_name: "Café americano",
  variant_id: null,
  variant_name: null,
  base_price: "12.50",
  modifiers_total: "0.00",
  tax: "2.00",
  unit_price: "14.50",
  tax_rate_bp: 1600,
  selected_options: [],
};

interface HarnessProps {
  actor?: Actor;
  readOnly?: boolean;
  products?: Product[];
  shift?: CashShift | null;
  initialCart?: CartLine[];
  onSaleCompleted?: (value: SaleView) => void;
  pendingAttempt?: PendingSaleAttempt | null;
  onPendingAttemptChange?: (attempt: PendingSaleAttempt | null) => void;
  loading?: boolean;
}

function PersistentSalesAttemptHarness({ storageKey }: { storageKey: string }) {
  const [pendingAttempt, setPendingAttempt] = useState<PendingSaleAttempt | null>(() =>
    loadPendingAttempt<PendingSaleAttempt["payload"]>(storageKey),
  );
  const [open, setOpen] = useState(true);
  const updatePendingAttempt = (attempt: PendingSaleAttempt | null) => {
    setPendingAttempt(attempt);
    savePendingAttempt(storageKey, attempt);
  };

  return (
    <>
      <button type="button" onClick={() => setOpen((value) => !value)}>
        {open ? "Ir a Catálogo" : "Ir a Ventas"}
      </button>
      {open && (
        <Harness
          shift={shift}
          pendingAttempt={pendingAttempt}
          onPendingAttemptChange={updatePendingAttempt}
        />
      )}
    </>
  );
}

function Harness({
  actor = owner,
  readOnly = false,
  products = [product],
  shift: activeShift = null,
  initialCart = [],
  onSaleCompleted = () => {},
  pendingAttempt = null,
  onPendingAttemptChange = () => {},
  loading = false,
}: HarnessProps) {
  const [cart, setCart] = useState<CartLine[]>(initialCart);
  return (
    <SalesScreen
      actor={actor}
      readOnly={readOnly}
      products={products}
      categories={[category]}
      stockLevels={{ "p-1": "20.000", "p-2": "8.000" }}
      registers={[{ id: "r-1", name: "Caja principal", code: "CJ-01", active: true }]}
      registerId="r-1"
      onRegisterChange={() => {}}
      onRegisterRequired={() => {}}
      shift={activeShift}
      cart={cart}
      onCartChange={setCart}
      onSaleCompleted={onSaleCompleted}
      pendingAttempt={pendingAttempt}
      onPendingAttemptChange={onPendingAttemptChange}
      onOpenCatalog={() => {}}
      loading={loading}
    />
  );
}

describe("SalesScreen", () => {
  beforeEach(() => {
    cleanup();
    document.body.replaceChildren();
    vi.clearAllMocks();
    window.localStorage.clear();
    api.listProductVariants.mockResolvedValue([]);
    api.listModifierGroups.mockResolvedValue([]);
    api.listModifierOptions.mockResolvedValue([]);
    api.configureProduct.mockResolvedValue(configuration);
    api.checkoutSale.mockResolvedValue(sale);
  });

  it("muestra catálogo y ticket simultáneos con disponibilidad", async () => {
    render(<Harness shift={shift} />);
    expect(screen.getByRole("heading", { name: /^ventas$/i })).toBeVisible();
    expect(screen.getByRole("heading", { name: /ticket actual/i })).toBeVisible();
    expect(screen.getByText("Café americano")).toBeVisible();
    expect(screen.getByText(/20 disp\./)).toBeVisible();
    expect(screen.getByText(/sin artículos/i)).toBeVisible();
  });

  it("Back devuelve del ticket al catálogo sin tocar las partidas", async () => {
    render(<Harness shift={shift} />);
    const user = userEvent.setup();
    const root = document.querySelector(".sales")!;

    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));
    await user.click(screen.getByRole("button", { name: /ver ticket/i }));
    expect(root).toHaveAttribute("data-view", "ticket");

    expect(handleBackButton()).toBe(true);
    await waitFor(() => expect(root).toHaveAttribute("data-view", "catalog"));

    // La partida sigue en el ticket: Back solo cambia la vista.
    const ticket = screen.getByRole("complementary", { name: /ticket actual/i });
    expect(within(ticket).getByText(/1 artículo/i)).toBeVisible();
  });

  it("agrega un producto simple al ticket con el precio de Rust", async () => {
    render(<Harness shift={shift} />);
    const user = userEvent.setup();
    const ticket = screen.getByRole("complementary", { name: /ticket actual/i });

    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));

    await waitFor(() =>
      expect(within(ticket).getAllByText("$14.50").length).toBeGreaterThan(0),
    );
    expect(within(ticket).getByText(/1 artículo/i)).toBeVisible();
    expect(api.configureProduct).toHaveBeenCalledWith({
      product_id: "p-1",
      variant_id: null,
      selected_option_ids: [],
    });
  });

  it("configura variante y modificadores en un diálogo", async () => {
    api.listProductVariants.mockResolvedValue([variant]);
    api.listModifierGroups.mockResolvedValue([group]);
    api.listModifierOptions.mockResolvedValue([extra]);
    api.configureProduct.mockResolvedValue({
      ...configuration,
      product_id: "p-2",
      product_name: "Latte",
      variant_id: "v-1",
      variant_name: "Grande",
      base_price: "18.00",
      modifiers_total: "2.00",
      tax: "3.20",
      unit_price: "23.20",
      selected_options: [
        {
          selected_option_id: "o-1",
          name: "Shot extra",
          kind: "extra",
          price_delta: "2.00",
        },
      ],
    });

    render(<Harness shift={shift} products={[product, latte]} />);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /latte, lat-001/i }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("radio", { name: /grande/i })).toBeChecked();
    await user.click(within(dialog).getByRole("radio", { name: /shot extra/i }));
    await user.click(within(dialog).getByRole("button", { name: /agregar al carrito/i }));

    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(api.configureProduct).toHaveBeenCalledWith({
      product_id: "p-2",
      variant_id: "v-1",
      selected_option_ids: ["o-1"],
    });
    expect(await screen.findByText("Shot extra")).toBeVisible();
    expect(screen.getAllByText(/grande/i).length).toBeGreaterThan(0);
    expect(screen.getAllByText("$23.20").length).toBeGreaterThan(0);
  });

  it("ajusta cantidades y quita partidas del ticket", async () => {
    render(<Harness shift={shift} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));

    await user.click(await screen.findByRole("button", { name: /aumentar café americano/i }));
    expect(screen.getAllByText("$29.00").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: /quitar café americano/i }));
    expect(await screen.findByText(/sin artículos/i)).toBeVisible();
  });

  it("mantiene el cobro bloqueado sin turno de caja", async () => {
    render(<Harness />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));

    expect(await screen.findByText(/abre un turno de caja para cobrar/i)).toBeVisible();
    expect(screen.getByRole("button", { name: /^cobrar \$/i })).toBeDisabled();
  });

  it("cobra en efectivo y muestra el folio confirmado", async () => {
    const completed = vi.fn();
    render(<Harness shift={shift} onSaleCompleted={completed} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));

    await user.click(await screen.findByRole("button", { name: /^cobrar \$/i }));

    const dialog = await screen.findByRole("dialog");
    const cash = within(dialog).getByLabelText(/efectivo recibido/i);
    await user.clear(cash);
    await user.type(cash, "20.00");
    expect(within(dialog).getByText(/cambio estimado: \$5\.50/i)).toBeVisible();

    await user.click(within(dialog).getByRole("button", { name: /confirmar cobro/i }));

    expect(await within(dialog).findByText(/venta v-abc123/i)).toBeVisible();
    expect(within(dialog).getByText(/\$14\.50/)).toBeVisible();
    expect(api.checkoutSale).toHaveBeenCalledTimes(1);
    expect(completed).toHaveBeenCalledWith(sale);
  });

  it("conserva y recupera un cobro incierto con el mismo borrador después de navegar", async () => {
    const { IpcError } = await import("../api");
    api.checkoutSale
      .mockRejectedValueOnce(new IpcError("unknown_error", "respuesta perdida"))
      .mockResolvedValueOnce(sale);
    const storageKey = pendingAttemptStorageKey("sale", owner);
    render(<PersistentSalesAttemptHarness storageKey={storageKey} />);
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));
    await user.click(await screen.findByRole("button", { name: /^cobrar \$/i }));
    await user.click(
      within(await screen.findByRole("dialog")).getByRole("button", { name: /confirmar cobro/i }),
    );

    expect(await screen.findByRole("heading", { name: /cobro pendiente de confirmar/i })).toBeVisible();
    const original = api.checkoutSale.mock.calls[0]?.[0];
    expect(loadPendingAttempt(storageKey)?.payload).toEqual(original);

    await user.click(screen.getByRole("button", { name: "Ir a Catálogo" }));
    await user.click(screen.getByRole("button", { name: "Ir a Ventas" }));
    expect(screen.getByRole("button", { name: /café americano, caf-001/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /recuperar cobro pendiente/i })).toBeEnabled();

    await user.click(screen.getByRole("button", { name: /recuperar cobro pendiente/i }));
    await user.click(
      within(await screen.findByRole("dialog")).getByRole("button", { name: /^recuperar cobro$/i }),
    );
    await waitFor(() => expect(api.checkoutSale).toHaveBeenCalledTimes(2));
    expect(api.checkoutSale.mock.calls[1]?.[0]).toEqual(original);
    expect(await screen.findByText(/venta v-abc123/i)).toBeVisible();
    expect(loadPendingAttempt(storageKey)).toBeNull();
  });

  it("recupera el cobro pendiente al iniciar una nueva aplicación", async () => {
    const storageKey = pendingAttemptStorageKey("sale", owner);
    const pending: PendingSaleAttempt = {
      fingerprint: "checkout pendiente",
      payload: {
        shift_id: "sh-1",
        idempotency_key: "sale-pending-1",
        items: [
          {
            product_id: "p-1",
            variant_id: null,
            quantity: "1",
            selected_option_ids: [],
          },
        ],
        cash_received: "20.00",
      },
    };
    savePendingAttempt(storageKey, pending);
    render(<PersistentSalesAttemptHarness storageKey={storageKey} />);

    expect(await screen.findByText(/hay un cobro pendiente de confirmar/i)).toBeVisible();
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /recuperar cobro pendiente/i }));
    await user.click(
      within(await screen.findByRole("dialog")).getByRole("button", { name: /^recuperar cobro$/i }),
    );

    await waitFor(() => expect(api.checkoutSale).toHaveBeenCalledWith(pending.payload));
    expect(loadPendingAttempt(storageKey)).toBeNull();
  });

  it("impide bajar la cantidad por debajo de uno", async () => {
    render(<Harness shift={shift} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));

    expect(
      await screen.findByRole("button", { name: /disminuir café americano/i }),
    ).toBeDisabled();
    expect(within(screen.getByRole("complementary", { name: /ticket actual/i })).getByText(/1 artículo/i)).toBeVisible();
  });

  it("confirma antes de vaciar el ticket", async () => {
    render(<Harness shift={shift} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));

    await user.click(screen.getByRole("button", { name: /^vaciar$/i }));
    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("heading", { name: /vaciar ticket/i })).toBeVisible();

    await user.click(within(dialog).getByRole("button", { name: /conservar ticket/i }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.queryByText(/sin artículos/i)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^vaciar$/i }));
    await user.click(
      within(await screen.findByRole("dialog")).getByRole("button", { name: /^vaciar ticket$/i }),
    );
    expect(await screen.findByText(/sin artículos/i)).toBeVisible();
  });

  it("bloquea el cobro cuando el efectivo no cubre el total", async () => {
    render(<Harness shift={shift} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));
    await user.click(await screen.findByRole("button", { name: /^cobrar \$/i }));

    const dialog = await screen.findByRole("dialog");
    const cash = within(dialog).getByLabelText(/efectivo recibido/i);
    await user.clear(cash);
    await user.type(cash, "5.00");

    expect(within(dialog).getByText(/faltan \$9\.50/i)).toBeVisible();
    expect(within(dialog).getByRole("button", { name: /confirmar cobro/i })).toBeDisabled();
  });

  it("no pierde el ticket cuando el cobro falla", async () => {
    const { IpcError } = await import("../api");
    api.checkoutSale.mockRejectedValueOnce(new IpcError("validation_error", "pago insuficiente"));

    render(<Harness shift={shift} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));
    await user.click(await screen.findByRole("button", { name: /^cobrar \$/i }));

    const dialog = await screen.findByRole("dialog");
    await user.click(within(dialog).getByRole("button", { name: /confirmar cobro/i }));

    expect(await within(dialog).findByText(/pago insuficiente/i)).toBeVisible();
    expect(within(dialog).getByRole("button", { name: /volver al ticket/i })).toBeEnabled();
  });

  it("descarta la configuración de un producto anterior", async () => {
    let resolveLatte: (value: ModifierGroup[]) => void = () => {};
    api.listProductVariants.mockResolvedValue([variant]);
    api.listModifierGroups
      .mockImplementationOnce(
        () => new Promise<ModifierGroup[]>((resolve) => (resolveLatte = resolve)),
      )
      .mockResolvedValue([group]);
    api.listModifierOptions.mockResolvedValue([extra]);

    render(<Harness shift={shift} products={[product, latte]} />);
    const user = userEvent.setup();

    // El primer producto tarda; el segundo responde antes.
    await user.click(screen.getByRole("button", { name: /latte, lat-001/i }));
    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));

    const dialog = await screen.findByRole("dialog");
    expect(within(dialog).getByRole("heading", { name: /café americano/i })).toBeVisible();

    resolveLatte([group]);
    await waitFor(() => {
      expect(screen.queryByRole("heading", { name: /latte/i })).not.toBeInTheDocument();
    });
  });

  it("explica el error de IPC al configurar", async () => {
    const { IpcError } = await import("../api");
    api.configureProduct.mockRejectedValueOnce(
      new IpcError("validation_error", "opción obligatoria faltante"),
    );

    render(<Harness shift={shift} />);
    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /café americano, caf-001/i }));

    expect(await screen.findByText(/opción obligatoria faltante/i)).toBeVisible();
  });

  it("oculta la operación sin permiso de venta", () => {
    render(<Harness actor={{ ...owner, permissions: [] }} />);
    expect(screen.queryByRole("heading", { name: /^ventas$/i })).not.toBeInTheDocument();
    expect(screen.getByText(/no puede registrar ventas/i)).toBeVisible();
  });
});
