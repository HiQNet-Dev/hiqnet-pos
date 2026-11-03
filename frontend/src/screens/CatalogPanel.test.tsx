import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { Actor, Category, Product, ProductVariant } from "../contracts";

const api = vi.hoisted(() => ({
  listCategories: vi.fn(),
  createCategory: vi.fn(),
  listProductVariants: vi.fn(),
  createProductVariant: vi.fn(),
  assignProductCategory: vi.fn(),
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

import { CatalogPanel } from "./CatalogPanel";

const actor: Actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana",
  role: "owner",
  permissions: ["catalog.manage", "sales.sell"],
};

const category: Category = {
  id: "c-1",
  name: "Bebidas",
  sort_order: 1,
  active: true,
};

const product: Product = {
  id: "p-1",
  sku: "CAF-001",
  name: "Café americano",
  unit_price: "12.50",
  tax_rate_bp: 1600,
  active: true,
  category_id: "c-1",
};

const variant: ProductVariant = {
  id: "v-1",
  product_id: "p-1",
  name: "Grande",
  sku: "CAF-001-G",
  unit_price: "15.00",
  active: true,
  sort_order: 1,
};

describe("CatalogPanel", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.listCategories.mockResolvedValue([category]);
    api.listProductVariants.mockResolvedValue([variant]);
    api.assignProductCategory.mockResolvedValue(product);
  });

  it("filtra productos por categoría y permite seleccionar variante", async () => {
    render(
      <CatalogPanel actor={actor} products={[product]} readOnly={false} />,
    );

    const user = userEvent.setup();
    await screen.findByRole("option", { name: "Bebidas" });
    await user.selectOptions(screen.getByLabelText("Categoría", { exact: true }), "c-1");
    expect(screen.getByRole("button", { name: /café americano/i })).toBeVisible();

    await user.click(screen.getByRole("button", { name: /café americano/i }));

    const variantButton = await screen.findByRole("button", { name: /grande/i });
    await user.click(variantButton);
    expect(variantButton).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByText("$15.00")).toBeVisible();
  });

  it("asigna categoría a un producto existente", async () => {
    render(
      <CatalogPanel actor={actor} products={[product]} readOnly={false} />,
    );

    const user = userEvent.setup();
    await user.click(screen.getByRole("button", { name: /café americano/i }));
    await user.selectOptions(screen.getByLabelText(/categoría del producto/i), "c-1");

    await waitFor(() => {
      expect(api.assignProductCategory).toHaveBeenCalledWith("p-1", "c-1");
    });
  });
});