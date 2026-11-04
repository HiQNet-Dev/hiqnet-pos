import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  Actor,
  ModifierConfiguration,
  ModifierGroup,
  ModifierOption,
  Product,
  ProductVariant,
} from "../contracts";

const api = vi.hoisted(() => ({
  listProducts: vi.fn(),
  listProductVariants: vi.fn(),
  listModifierGroups: vi.fn(),
  listModifierOptions: vi.fn(),
  configureProduct: vi.fn(),
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

import { ModifierConfigurator } from "./ModifierConfigurator";

const actor: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana Propietaria",
  role: "owner",
  permissions: ["sales.sell", "catalog.manage"],
};



const product: Product = {
  id: "p-1",
  sku: "LIC-001",
  name: "Licuado de fresa",
  unit_price: "30.00",
  tax_rate_bp: 1600,
  active: true,
};

const variant: ProductVariant = {
  id: "v-1",
  product_id: "p-1",
  name: "Grande",
  sku: "LIC-001-G",
  unit_price: "35.00",
  active: true,
  sort_order: 1,
};

const group: ModifierGroup = {
  id: "g-1",
  name: "Extras",
  min: 1,
  max: 2,
  sort_order: 1,
  active: true,
};

const option: ModifierOption = {
  id: "o-1",
  group_id: "g-1",
  name: "Proteína",
  kind: "extra",
  price_delta: "5.00",
  sort_order: 1,
  active: true,
};

const configuration: ModifierConfiguration = {
  product_id: "p-1",
  product_name: "Licuado de fresa",
  variant_id: "v-1",
  variant_name: "Grande",
  base_price: "35.00",
  modifiers_total: "5.00",
  tax: "6.40",
  unit_price: "46.40",
  tax_rate_bp: 1600,
  selected_options: [
    {
      selected_option_id: "o-1",
      name: "Proteína",
      kind: "extra",
      price_delta: "5.00",
    },
  ],
};

describe("ModifierConfigurator", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.listProducts.mockResolvedValue([product]);
    api.listProductVariants.mockResolvedValue([variant]);
    api.listModifierGroups.mockResolvedValue([group]);
    api.listModifierOptions.mockResolvedValue([option]);
    api.configureProduct.mockResolvedValue(configuration);
  });

  it("carga productos", async () => {
    render(<ModifierConfigurator actor={actor} readOnly={false} />);
    expect(await screen.findByRole("option", { name: /licuado de fresa/i })).toBeVisible();
  });

  it("selecciona variantes y opciones", async () => {
    render(<ModifierConfigurator actor={actor} readOnly={false} />);
    const user = userEvent.setup();
    await user.selectOptions(await screen.findByLabelText("Producto", { exact: true }), "p-1");
    const variantSelect = await screen.findByLabelText(/variante/i);
    expect(variantSelect).toHaveValue("v-1");
    await user.click(screen.getByRole("checkbox", { name: /proteína/i }));
    await user.click(screen.getByRole("button", { name: /validar configuración/i }));
    await waitFor(() => expect(api.configureProduct).toHaveBeenCalled());
    expect(screen.getByText(/\$46.40/)).toBeVisible();
  });

  it("muestra la validación fallida", async () => {
    const { IpcError } = await import("../api");
    api.configureProduct.mockRejectedValueOnce(
      new IpcError("validation_error", "falta una opción obligatoria"),
    );
    render(<ModifierConfigurator actor={actor} readOnly={false} />);
    const user = userEvent.setup();
    await user.selectOptions(await screen.findByLabelText("Producto", { exact: true }), "p-1");
    await user.click(screen.getByRole("button", { name: /validar configuración/i }));
    expect(await screen.findByText(/falta una opción obligatoria/i)).toBeVisible();
  });

  it("oculta el botón sin permiso de venta", async () => {
    render(
      <ModifierConfigurator
        actor={{ ...actor, permissions: ["catalog.manage"] }}
        readOnly={false}
      />,
    );
    const user = userEvent.setup();
    await user.selectOptions(await screen.findByLabelText("Producto", { exact: true }), "p-1");
    expect(screen.queryByRole("button", { name: /validar configuración/i })).not.toBeInTheDocument();
  });

  it("oculta el botón con licencia vencida", async () => {
    render(<ModifierConfigurator actor={actor} readOnly={true} />);
    const user = userEvent.setup();
    await user.selectOptions(await screen.findByLabelText("Producto", { exact: true }), "p-1");
    expect(screen.queryByRole("button", { name: /validar configuración/i })).not.toBeInTheDocument();
  });

  it("invalida la validación anterior al cambiar opción", async () => {
    render(<ModifierConfigurator actor={actor} readOnly={false} />);
    const user = userEvent.setup();
    await user.selectOptions(await screen.findByLabelText("Producto", { exact: true }), "p-1");
    await user.click(screen.getByRole("button", { name: /validar configuración/i }));
    await screen.findByText(/\$46.40/);
    await user.click(screen.getByRole("checkbox", { name: /proteína/i }));
    expect(screen.queryByText(/\$46.40/)).not.toBeInTheDocument();
  });

  it("ignora respuestas obsoletas al cambiar de producto", async () => {
    let resolveFirst: (value: ModifierConfiguration) => void = () => {};
    const product2: Product = { ...product, id: "p-2", name: "Jugo", sku: "JUG-001" };
    api.listProducts.mockResolvedValue([product, product2]);
    api.configureProduct
      .mockImplementationOnce(() => new Promise((resolve) => { resolveFirst = resolve; }))
      .mockResolvedValueOnce({ ...configuration, product_id: "p-2", product_name: "Jugo", unit_price: "20.00" });

    render(<ModifierConfigurator actor={actor} readOnly={false} />);
    const user = userEvent.setup();
    await user.selectOptions(await screen.findByLabelText("Producto", { exact: true }), "p-1");
    api.listModifierGroups.mockResolvedValue([group]);
    api.listModifierOptions.mockResolvedValue([option]);
    await user.click(screen.getByRole("button", { name: /validar configuración/i }));
    await user.selectOptions(screen.getByLabelText("Producto", { exact: true }), "p-2");
    resolveFirst({ ...configuration, product_name: "Producto antiguo", unit_price: "999.00" });

    await waitFor(() => expect(screen.queryByText(/\$999\.00/)).not.toBeInTheDocument());
    expect(screen.getByRole("option", { name: /jugo/i })).toBeVisible();
  });
});