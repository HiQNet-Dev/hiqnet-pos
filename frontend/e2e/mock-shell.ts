import type { Page } from "@playwright/test";

/**
 * Puente Tauri simulado mínimo para las pruebas de shell.
 *
 * `frontend/e2e/visual.spec.ts` mantiene su propio fixture con datos ricos para
 * las capturas; este se limita a lo que necesita la navegación, de modo que sus
 * aserciones de geometría no dependan de la forma del catálogo.
 */
const ACTOR = {
  user_id: "u-nav",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana Martínez",
  role: "owner",
  permissions: [
    "catalog.manage",
    "sales.sell",
    "sales.return",
    "inventory.view",
    "inventory.manage",
    "expenses.manage",
    "purchasing.manage",
    "cash.shift.view",
    "cash.shift.open",
    "cash.shift.close",
    "cash.movement.record",
    "cash.register.manage",
    "audit.view",
    "backups.manage",
    "reports.view",
  ],
};

const COMMERCIAL = {
  plan: "pro",
  status: "active",
  valid_until: "2026-12-03T00:00:00Z",
  features: ["catalog", "sales"],
  read_only: false,
};

const PRODUCTS = [
  {
    id: "p-1",
    sku: "CAF-001",
    name: "Café americano",
    unit_price: "12.50",
    public_unit_total: "12.50",
    tax_rate_bp: 1600,
    active: true,
    active_variants: 0,
    variant_public_from: null,
    has_modifiers: false,
  },
  {
    id: "p-2",
    sku: "PAN-010",
    name: "Croissant de mantequilla",
    unit_price: "9.80",
    public_unit_total: "9.80",
    tax_rate_bp: 1600,
    active: true,
    active_variants: 0,
    variant_public_from: null,
    has_modifiers: false,
  },
];

export async function mockShellBridge(page: Page, products = PRODUCTS): Promise<void> {
  await page.addInitScript(
    ({ actor, commercial, products }) => {
      const responses: Record<string, unknown> = {
        app_status: {
          initialized: true,
          product_version: "0.1.0",
          commercial,
          user_count: 1,
        },
        current_session: null,
        login: actor,
        logout: null,
        branch_summary: { id: "b-1", name: "Matriz Centro", code: "MX-01" },
        list_products: products,
        list_categories: [
          { id: "c-1", name: "Bebidas calientes", sort_order: 1, active: true },
          { id: "c-2", name: "Panadería", sort_order: 2, active: true },
          { id: "c-3", name: "Refrigerados", sort_order: 3, active: true },
          { id: "c-4", name: "Combos", sort_order: 4, active: true },
        ],
        list_stock_levels: products.map((product) => ({
          product_id: product.id,
          quantity: "20.000",
        })),
        list_product_variants: [],
        list_modifier_groups: [],
        list_cash_registers: [
          { id: "r-1", name: "Caja principal", code: "CJ-01", active: true },
        ],
        list_cash_shift_history: [
          {
            id: "s-0",
            register_id: "r-1",
            status: "closed",
            opened_by: "u-nav",
            opened_at: "2026-11-04T15:00:00Z",
            closed_by: "u-nav",
            closed_at: "2026-11-04T21:43:00Z",
            opening_amount: "500.00",
            expected_amount: "1842.80",
            entry_total: "1342.80",
            exit_total: "0.00",
            counted_amount: "1840.00",
            difference_minor: -280,
            movements: [],
          },
        ],
        active_cash_shift: {
          id: "s-1",
          register_id: "r-1",
          status: "open",
          opened_by: "u-nav",
          opened_at: "2026-11-05T08:00:00Z",
          closed_by: null,
          closed_at: null,
          opening_amount: "1000.00",
          expected_amount: "1000.00",
          entry_total: "0",
          exit_total: "0",
          counted_amount: null,
          difference_minor: null,
          movements: [],
        },
        list_audit_events: [
          {
            id: "a-1",
            action: "sales.completed",
            entity_type: "sale",
            entity_id: "8f2a1c40-1d2e-4f5a-9b3c-0a1b2c3d4e5f",
            actor_user_id: "u-nav",
            occurred_at: "2026-11-05T18:40:00Z",
          },
          {
            id: "a-2",
            action: "sales.returned",
            entity_type: "sale",
            entity_id: "c2b3a4d5-6e7f-4a8b-9c0d-1e2f3a4b5c6d",
            actor_user_id: "u-nav",
            occurred_at: "2026-11-05T18:20:00Z",
          },
          {
            id: "a-3",
            action: "waste.recorded",
            entity_type: "inventory_movement",
            entity_id: "d3c4b5a6-7f8e-4b9c-8d1e-2f3a4b5c6d7e",
            actor_user_id: "u-nav",
            occurred_at: "2026-11-05T16:00:00Z",
          },
          {
            id: "a-4",
            action: "purchasing.received",
            entity_type: "purchase_order",
            entity_id: "e4d5c6b7-8a9f-4c0d-9e2f-3a4b5c6d7e8f",
            actor_user_id: "u-nav",
            occurred_at: "2026-11-04T12:00:00Z",
          },
          {
            id: "a-5",
            action: "expenses.category_created",
            entity_type: "category",
            entity_id: "f5e6d7c8-9b0a-4d1e-8f3a-4b5c6d7e8f9a",
            actor_user_id: "u-nav",
            occurred_at: "2026-11-03T09:00:00Z",
          },
        ],
        commercial_state: commercial,
        current_stock: "20.000",
        list_inventory_movements: [
          {
            id: "mv-1",
            product_id: "p-1",
            kind: "inbound",
            quantity: "20.000",
            reason: "Compra a proveedor",
            occurred_at: "2026-11-04T08:00:00Z",
          },
        ],
        list_waste: [],
      };

      Object.assign(window, {
        __TAURI_INTERNALS__: {
          invoke: (command: string, args?: Record<string, unknown>) => {
            const request = args ?? {};
            if (command === "plugin:app|register_listener") {
              // Permite a las pruebas disparar el evento nativo de Back como lo
              // haría `AppPlugin.trigger("back-button", …)` en Android.
              const internals = window as unknown as Record<string, unknown>;
              internals.__hiqnetBack = request.handler;
              internals.__hiqnetBackRegistrations =
                (Number(internals.__hiqnetBackRegistrations) || 0) + 1;
              return Promise.resolve(1);
            }
            if (command === "plugin:app|remove_listener") {
              (window as unknown as Record<string, unknown>).__hiqnetBack = null;
              return Promise.resolve(null);
            }
            if (command === "preview_product_price") {
              // Espejo del cálculo de Rust para el fixture: enteros en centavos
              // y redondeo half-up del impuesto.
              const net = Math.round(Number(request.price) * 100);
              const rate = Number(request.tax_rate_bp);
              if (!Number.isFinite(net) || !Number.isFinite(rate) || rate < 0 || rate > 10_000) {
                return Promise.reject({ code: "validation_error", message: "precio inválido" });
              }
              const tax = Math.round((net * rate) / 10_000);
              return Promise.resolve({
                unit_price: (net / 100).toFixed(2),
                tax: (tax / 100).toFixed(2),
                public_unit_total: ((net + tax) / 100).toFixed(2),
                tax_rate_bp: rate,
              });
            }
            if (command === "configure_product") {
              const draft = request.draft as { product_id?: string } | undefined;
              const product =
                products.find((item) => item.id === draft?.product_id) ?? products[0]!;
              const unit = Number(product.unit_price);
              const rate = product.tax_rate_bp;
              const tax = (unit * rate) / (10000 + rate);
              return Promise.resolve({
                product_id: product.id,
                product_name: product.name,
                variant_id: null,
                variant_name: null,
                base_price: product.unit_price,
                modifiers_total: "0.00",
                tax: tax.toFixed(2),
                unit_price: unit.toFixed(2),
                tax_rate_bp: rate,
                selected_options: [],
              });
            }
            return Promise.resolve(responses[command] ?? null);
          },
          transformCallback: (callback: unknown) => callback,
        },
      });
    },
    { actor: ACTOR, commercial: COMMERCIAL, products },
  );
}

export async function loginThroughBridge(page: Page): Promise<void> {
  await page.goto("/");
  await page.getByLabel("Usuario").fill("ana");
  await page.getByLabel("Contraseña").fill("contrasena-segura");
  await page.getByRole("button", { name: "Entrar" }).click();
  await page.getByRole("navigation", { name: "Navegación principal" }).waitFor();
}

export async function addOneProduct(page: Page): Promise<void> {
  await page.getByRole("button", { name: /Café americano, CAF-001/ }).click();
}
