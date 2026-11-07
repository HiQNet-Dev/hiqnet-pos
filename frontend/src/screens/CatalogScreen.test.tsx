import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, Category, ModifierGroup, ModifierOption, Product } from "../contracts";

const api = vi.hoisted(() => ({
  listProductVariants: vi.fn(),
  createProductVariant: vi.fn(),
  listModifierGroups: vi.fn(),
  createModifierGroup: vi.fn(),
  listModifierOptions: vi.fn(),
  createModifierOption: vi.fn(),
  createProduct: vi.fn(),
  createCategory: vi.fn(),
  assignProductCategory: vi.fn(),
  configureProduct: vi.fn(),
  previewProductPrice: vi.fn(),
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

import { CatalogScreen } from "./CatalogScreen";

const actor: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana",
  role: "owner",
  permissions: ["catalog.manage", "sales.sell"],
};

const category: Category = { id: "c-1", name: "Bebidas", sort_order: 1, active: true };

const product: Product = {
  id: "p-1",
  sku: "CAF-001",
  name: "Café americano",
  unit_price: "12.50",
  public_unit_total: "12.50",
  tax_rate_bp: 1600,
  active: true,
  category_id: "c-1",
  active_variants: 0,
  variant_public_from: null,
  has_modifiers: false,
};

const variant = {
  id: "v-1",
  product_id: "p-1",
  name: "Grande",
  sku: null,
  unit_price: "15.00",
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

const option: ModifierOption = {
  id: "o-1",
  group_id: "g-1",
  name: "Shot extra",
  kind: "extra",
  price_delta: "2.00",
  sort_order: 0,
  active: true,
};

function renderScreen(props: Partial<Parameters<typeof CatalogScreen>[0]> = {}) {
  const onDataChanged = vi.fn();
  render(
    <CatalogScreen
      actor={actor}
      readOnly={false}
      products={[product]}
      categories={[category]}
      loading={false}
      onDataChanged={onDataChanged}
      {...props}
    />,
  );
  return { onDataChanged };
}

describe("CatalogScreen", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.listProductVariants.mockResolvedValue([variant]);
    api.listModifierGroups.mockResolvedValue([group]);
    api.listModifierOptions.mockResolvedValue([option]);
    api.createProductVariant.mockResolvedValue(variant);
    api.createCategory.mockResolvedValue({ ...category, id: "c-2", name: "Postres" });
    api.createProduct.mockResolvedValue(product);
    api.createModifierGroup.mockResolvedValue(group);
    api.createModifierOption.mockResolvedValue(option);
    api.assignProductCategory.mockResolvedValue(product);
    api.previewProductPrice.mockResolvedValue({
      unit_price: "12.50",
      tax: "2.00",
      public_unit_total: "14.50",
      tax_rate_bp: 1600,
    });
  });

  it("lista productos y muestra la configuración del seleccionado", async () => {
    renderScreen();

    expect(screen.getByRole("cell", { name: "CAF-001" })).toBeVisible();
    expect(screen.getByRole("cell", { name: /café americano/i })).toBeVisible();
    expect(await screen.findByText("Grande")).toBeVisible();
    expect(await screen.findByText("Extras")).toBeVisible();
    expect(screen.getAllByText("Shot extra").length).toBeGreaterThan(0);
  });

  it("filtra por búsqueda y por categoría", async () => {
    renderScreen();
    const user = userEvent.setup();

    await user.type(screen.getByLabelText(/buscar producto/i), "latte");
    expect(screen.queryByRole("cell", { name: "CAF-001" })).not.toBeInTheDocument();

    await user.clear(screen.getByLabelText(/buscar producto/i));
    await user.selectOptions(screen.getByLabelText("Categoría", { exact: true }), "c-1");
    expect(screen.getByRole("cell", { name: "CAF-001" })).toBeVisible();
  });

  it("crea una categoría desde el diálogo", async () => {
    const { onDataChanged } = renderScreen();
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /nueva categoría/i }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByLabelText(/nombre/i), "Postres");
    await user.click(within(dialog).getByRole("button", { name: /crear categoría/i }));

    await waitFor(() => expect(api.createCategory).toHaveBeenCalledWith({
      name: "Postres",
      sort_order: 1,
    }));
    expect(onDataChanged).toHaveBeenCalled();
  });

  it("crea un producto con precio e impuesto", async () => {
    const { onDataChanged } = renderScreen();
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /nuevo producto/i }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByLabelText("SKU"), "LAT-001");
    await user.type(within(dialog).getByLabelText(/^nombre$/i), "Latte");
    await user.type(within(dialog).getByLabelText(/precio base, sin iva/i), "18.00");
    await user.selectOptions(within(dialog).getByLabelText(/categoría/i), "c-1");
    await user.click(within(dialog).getByRole("button", { name: /agregar producto/i }));

    await waitFor(() =>
      expect(api.createProduct).toHaveBeenCalledWith({
        sku: "LAT-001",
        name: "Latte",
        price: "18.00",
        tax_rate_bp: 1600,
        category_id: "c-1",
      }),
    );
    expect(onDataChanged).toHaveBeenCalled();
  });

  it("muestra el precio final al público que calcula Rust en el formulario", async () => {
    api.previewProductPrice.mockResolvedValue({
      unit_price: "100.00",
      tax: "16.00",
      public_unit_total: "116.00",
      tax_rate_bp: 1600,
    });
    renderScreen();
    const user = userEvent.setup();

    await user.click(screen.getByRole("button", { name: /nuevo producto/i }));
    const dialog = await screen.findByRole("dialog");
    await user.type(within(dialog).getByLabelText(/precio base, sin iva/i), "100.00");

    await waitFor(() => expect(api.previewProductPrice).toHaveBeenCalledWith("100.00", 1600));
    const preview = await within(dialog).findByText("$116.00");
    expect(preview).toBeVisible();
    expect(within(dialog).getByText(/IVA 16%: \$16\.00/)).toBeVisible();
  });

  it("crea una variante para el producto seleccionado", async () => {
    const { onDataChanged } = renderScreen();
    const user = userEvent.setup();

    await user.type(await screen.findByLabelText(/nueva variante/i), "Mediano");
    await user.type(screen.getByLabelText(/^precio neto$/i), "14.00");
    await user.click(screen.getByRole("button", { name: /crear variante/i }));

    await waitFor(() =>
      expect(api.createProductVariant).toHaveBeenCalledWith({
        product_id: "p-1",
        name: "Mediano",
        price: "14.00",
        sort_order: 1,
      }),
    );
    expect(onDataChanged).toHaveBeenCalled();
  });

  it("crea un grupo de modificadores y una opción", async () => {
    const { onDataChanged } = renderScreen();
    const user = userEvent.setup();

    await user.click(await screen.findByRole("button", { name: /nuevo grupo/i }));
    await user.type(screen.getByLabelText(/^grupo$/i), "Leche");
    await user.clear(screen.getByLabelText(/^mín$/i));
    await user.type(screen.getByLabelText(/^mín$/i), "1");
    await user.click(screen.getByRole("button", { name: /crear grupo/i }));

    await waitFor(() =>
      expect(api.createModifierGroup).toHaveBeenCalledWith({
        product_id: "p-1",
        name: "Leche",
        min: 1,
        max: 1,
        sort_order: 1,
      }),
    );

    const groupCard = (await screen.findByText("Extras")).closest("article")!;
    await user.click(within(groupCard).getByRole("button", { name: /^opción$/i }));
    await user.type(within(groupCard).getByLabelText(/^opción$/i), "Avena");
    await user.clear(within(groupCard).getByLabelText(/^delta$/i));
    await user.type(within(groupCard).getByLabelText(/^delta$/i), "1.50");
    await user.click(within(groupCard).getByRole("button", { name: /^agregar$/i }));

    await waitFor(() =>
      expect(api.createModifierOption).toHaveBeenCalledWith({
        group_id: "g-1",
        name: "Avena",
        kind: "extra",
        price_delta: "1.50",
        sort_order: 1,
      }),
    );
    expect(onDataChanged).toHaveBeenCalled();
  });

  it("valida una configuración y muestra el desglose de Rust", async () => {
    api.configureProduct.mockResolvedValue({
      product_id: "p-1",
      product_name: "Café americano",
      variant_id: "v-1",
      variant_name: "Grande",
      base_price: "15.00",
      modifiers_total: "2.00",
      tax: "2.72",
      unit_price: "19.72",
      tax_rate_bp: 1600,
      selected_options: [
        { selected_option_id: "o-1", name: "Shot extra", kind: "extra", price_delta: "2.00" },
      ],
    });
    renderScreen();
    const user = userEvent.setup();

    await user.click(await screen.findByRole("radio", { name: /shot extra/i }));
    await user.click(screen.getByRole("button", { name: /validar configuración/i }));

    expect(await screen.findByText("$19.72")).toBeVisible();
    expect(screen.getAllByText("$15.00").length).toBeGreaterThan(0);
    expect(screen.getByText("$2.72")).toBeVisible();
    expect(api.configureProduct).toHaveBeenCalledWith({
      product_id: "p-1",
      variant_id: "v-1",
      selected_option_ids: ["o-1"],
    });
  });

  it("bloquea la creación en modo solo lectura", async () => {
    renderScreen({ readOnly: true });
    expect(screen.getByRole("button", { name: /nuevo producto/i })).toBeDisabled();
    expect(screen.getByRole("button", { name: /nueva categoría/i })).toBeDisabled();
    expect(await screen.findByRole("button", { name: /crear variante/i })).toBeDisabled();
  });

  it("no muestra el panel administrativo sin permiso de catálogo", () => {
    renderScreen({ actor: { ...actor, permissions: [] } });
    expect(screen.getByText(/no administra el catálogo/i)).toBeVisible();
  });
});
