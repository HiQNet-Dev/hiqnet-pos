import { expect, test, type Page } from "@playwright/test";
import { mkdir } from "node:fs/promises";
import path from "node:path";

/**
 * Capturas de las pantallas administrativas que no cubre `visual.spec.ts`:
 * reportes, compras, gastos, devoluciones y gestión. Usan un actor con todos
 * los permisos para que todas las secciones estén disponibles.
 *
 * Ejecución:
 *   VISUAL_REVIEW=1 pnpm exec playwright test e2e/visual-admin.spec.ts
 */

const OUT_DIR = path.resolve(
  process.cwd(),
  "..",
  ".devkit",
  "artifacts",
  "design-review",
  "after",
);

const actor = {
  user_id: "u-1",
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
    "waste.manage",
    "expenses.manage",
    "purchasing.manage",
    "cash.shift.view",
    "cash.shift.open",
    "cash.shift.close",
    "cash.movement.record",
    "cash.register.manage",
    "audit.view",
    "backups.manage",
    "users.manage",
    "reports.view",
  ],
};

const commercial = {
  plan: "pro",
  status: "active",
  valid_until: "2026-12-03T00:00:00Z",
  features: ["catalog", "sales"],
  read_only: false,
};

const products = [
  { id: "p-1", sku: "CAF-001", name: "Café americano", unit_price: "12.50", tax_rate_bp: 1600, active: true },
  { id: "p-2", sku: "CAF-002", name: "Capuchino", unit_price: "18.00", tax_rate_bp: 1600, active: true },
  { id: "p-3", sku: "PAN-010", name: "Croissant de mantequilla", unit_price: "9.80", tax_rate_bp: 1600, active: true },
  { id: "p-4", sku: "COM-001", name: "Combo desayuno", unit_price: "39.00", tax_rate_bp: 1600, active: true },
];

const categories = [
  { id: "c-1", name: "Bebidas calientes", sort_order: 1, active: true },
  { id: "c-2", name: "Panadería", sort_order: 2, active: true },
];

const reports = {
  from: "2026-10-30T00:00:00Z",
  to: "2026-11-06T00:00:00Z",
  sales: {
    count: 24,
    subtotal: "1240.00",
    tax: "198.40",
    total: "1438.40",
    returned_count: 1,
    returned_total: "38.00",
    net_total: "1400.40",
    products: [
      { product_id: "p-1", product_name: "Café americano", quantity: "42.000", net: "452.59", tax: "72.41", total: "525.00" },
      { product_id: "p-2", product_name: "Capuchino", quantity: "28.000", net: "434.48", tax: "69.52", total: "504.00" },
      { product_id: "p-3", product_name: "Croissant de mantequilla", quantity: "31.000", net: "262.07", tax: "41.93", total: "304.00" },
      { product_id: "p-4", product_name: "Combo desayuno", quantity: "2.000", net: "67.24", tax: "10.76", total: "78.00" },
    ],
  },
  cash: {
    shifts_opened: 2,
    shifts_closed: 1,
    entries: "200.00",
    exits: "58.00",
    expenses_from_cash: "58.00",
    refunds: "38.00",
    difference: "-0.20",
  },
  purchases: {
    orders_created: 3,
    pending_orders: 1,
    received_quantity: "120.000",
    received_cost: "1840.00",
  },
  expenses: {
    total: "258.00",
    from_cash: "58.00",
    external: "200.00",
    categories: [
      { name: "Insumos", total: "158.00" },
      { name: "Servicios", total: "100.00" },
    ],
  },
  inventory: {
    stock_quantity: "1840.000",
    inbound: "120.000",
    outbound: "96.000",
    waste: "3.000",
    adjustment_in: "2.000",
    adjustment_out: "1.000",
  },
};

const suppliers = [
  { id: "sup-1", code: "PROV-01", name: "Distribuidora Central", contact: "Laura Ruiz", rfc: "DCE010101AAA", active: true },
  { id: "sup-2", code: null, name: "Panificadora del Valle", contact: null, rfc: null, active: true },
];

const purchaseOrders = [
  {
    id: "po-1",
    supplier_id: "sup-1",
    folio: "OC-000112",
    status: "ordered",
    created_at: "2026-11-04T10:00:00Z",
    lines: [
      { id: "pol-1", product_id: "p-1", quantity: "50.000", received_quantity: "0.000", unit_cost: "8.90" },
      { id: "pol-2", product_id: "p-3", quantity: "40.000", received_quantity: "0.000", unit_cost: "4.20" },
    ],
  },
  {
    id: "po-2",
    supplier_id: "sup-2",
    folio: "OC-000111",
    status: "received",
    created_at: "2026-11-02T09:30:00Z",
    lines: [
      { id: "pol-3", product_id: "p-2", quantity: "30.000", received_quantity: "30.000", unit_cost: "11.00" },
    ],
  },
];

const expenseCategories = [
  { id: "ec-1", name: "Insumos", active: true },
  { id: "ec-2", name: "Servicios", active: true },
];

const expenses = [
  { id: "e-1", category_id: "ec-1", amount: "58.00", concept: "Compra de hielo", paid_from_cash: true, occurred_at: "2026-11-05T18:35:00Z" },
  { id: "e-2", category_id: "ec-2", amount: "200.00", concept: "Internet del mes", paid_from_cash: false, occurred_at: "2026-11-04T12:00:00Z" },
];

const saleReturns = [
  {
    id: "sr-1",
    sale_id: "sale-127",
    sale_folio: "F-000127",
    reason: "Producto frío",
    refund_total: "38.00",
    occurred_at: "2026-11-05T17:10:00Z",
    lines: [
      {
        sale_item_id: "si-1",
        product_name: "Croissant de mantequilla",
        quantity: "1.000",
        disposition: "discard",
        refund_net: "32.76",
        refund_tax: "5.24",
        refund_total: "38.00",
      },
    ],
  },
];

const users = [
  { id: "u-1", username: "ana", display_name: "Ana Martínez", role: "owner", active: true, created_at: "2026-01-10T09:00:00Z" },
  { id: "u-2", username: "luis", display_name: "Luis Ramírez", role: "cashier", active: true, created_at: "2026-03-02T10:00:00Z" },
  { id: "u-3", username: "sofia", display_name: "Sofía Ortega", role: "cashier", active: false, created_at: "2026-05-18T10:00:00Z" },
];

// Claves tal como las emite `AuditAction::as_str()` en Rust: si una deja de
// estar traducida, la captura lo muestra en lugar de ocultarlo.
const auditEvents = [
  { id: "a-1", action: "sales.completed", entity_type: "sale", entity_id: "8f2a1c40-1d2e-4f5a-9b3c-0a1b2c3d4e5f", actor_user_id: "u-1", occurred_at: "2026-11-05T18:40:00Z" },
  { id: "a-2", action: "expenses.recorded", entity_type: "expense", entity_id: "3c9d7e21-6a4b-4c8d-8e1f-2b3a4c5d6e7f", actor_user_id: "u-1", occurred_at: "2026-11-05T18:35:00Z" },
  { id: "a-3", action: "purchasing.order_created", entity_type: "purchase_order", entity_id: "b71e4a55-9f0c-4d2b-8a6e-c1d2e3f4a5b6", actor_user_id: "u-1", occurred_at: "2026-11-04T10:00:00Z" },
];

const shift = {
  id: "s-1",
  register_id: "r-1",
  status: "open",
  opened_by: "u-1",
  opened_at: "2026-11-05T15:00:00Z",
  closed_by: null,
  closed_at: null,
  opening_amount: "500.00",
  expected_amount: "1842.80",
  entry_total: "1342.80",
  exit_total: "0.00",
  counted_amount: null,
  difference_minor: null,
  movements: [
    { id: "m-1", kind: "entry", amount: "200.00", reason: "Cambio adicional", occurred_at: "2026-11-05T17:35:00Z" },
    { id: "m-2", kind: "exit", amount: "58.00", reason: "Compra de hielo", occurred_at: "2026-11-05T18:35:00Z" },
  ],
};

const stockMovements = [
  { id: "mv-1", product_id: "p-1", kind: "inbound", quantity: "50.000", reason: "Recepción OC-000112", occurred_at: "2026-11-04T10:20:00Z" },
  { id: "mv-2", product_id: "p-1", kind: "outbound", quantity: "12.000", reason: "Venta F-000128", occurred_at: "2026-11-05T18:40:00Z" },
];

async function mockAdminBridge(page: Page) {
  await page.addInitScript(
    ({
      actor,
      commercial,
      products,
      categories,
      reports,
      suppliers,
      purchaseOrders,
      expenseCategories,
      expenses,
      saleReturns,
      users,
      auditEvents,
      shift,
      stockMovements,
    }) => {
      const responses: Record<string, unknown> = {
        app_status: {
          initialized: true,
          product_version: "0.1.0",
          commercial,
          user_count: 3,
        },
        current_session: null,
        login: actor,
        logout: null,
        branch_summary: { id: "b-1", name: "Matriz Centro", code: "MX-01" },
        list_products: products,
        list_categories: categories,
        list_stock_levels: products.map((product) => ({
          product_id: product.id,
          quantity: "1840.000",
        })),
        list_product_variants: [],
        list_modifier_groups: [],
        list_cash_registers: [{ id: "r-1", name: "Caja principal", code: "CJ-01", active: true }],
        list_cash_shift_history: [
          {
            ...shift,
            status: "closed",
            closed_by: "u-1",
            closed_at: "2026-11-04T21:00:00Z",
            counted_amount: "1842.60",
            difference_minor: -20,
          },
        ],
        active_cash_shift: shift,
        list_audit_events: auditEvents,
        commercial_state: commercial,
        reports_summary: reports,
        list_suppliers: suppliers,
        list_purchase_orders: purchaseOrders,
        list_expense_categories: expenseCategories,
        list_expenses: expenses,
        list_sale_returns: saleReturns,
        list_users: users,
        current_stock: "1840.000",
        list_inventory_movements: stockMovements,
        list_waste: [
          {
            id: "w-1",
            product_id: "p-3",
            quantity: "3.000",
            reason: "Producto vencido",
            occurred_at: "2026-11-05T16:00:00Z",
          },
        ],
        backup_database: "C:/respaldos/hiqnet-pos-2026-11-05.sqlite",
      };

      Object.assign(window, {
        __TAURI_INTERNALS__: {
          invoke: (command: string, args?: Record<string, unknown>) => {
            const request = args ?? {};
            if (command === "configure_product") {
              const draft = request.draft as { product_id?: string } | undefined;
              const product =
                (products as Array<{ id: string; name: string; unit_price: string; tax_rate_bp: number }>).find(
                  (item) => item.id === draft?.product_id,
                ) ?? products[0]!;
              const unit = Number(product.unit_price);
              const tax = (unit * product.tax_rate_bp) / (10000 + product.tax_rate_bp);
              return Promise.resolve({
                product_id: product.id,
                product_name: product.name,
                variant_id: null,
                variant_name: null,
                base_price: product.unit_price,
                modifiers_total: "0.00",
                tax: tax.toFixed(2),
                unit_price: unit.toFixed(2),
                tax_rate_bp: product.tax_rate_bp,
                selected_options: [],
              });
            }
            return Promise.resolve(responses[command] ?? null);
          },
          transformCallback: (callback: unknown) => callback,
        },
      });
    },
    {
      actor,
      commercial,
      products,
      categories,
      reports,
      suppliers,
      purchaseOrders,
      expenseCategories,
      expenses,
      saleReturns,
      users,
      auditEvents,
      shift,
      stockMovements,
    },
  );
}

/**
 * Captura estable: espera a que las fuentes estén listas, a que React haya
 * aplicado el cambio en dos frames consecutivos y a que el compositor pinte el
 * estado final. Sin esta espera el riel de navegación podía aparecer con el
 * resaltado del estado anterior en la captura.
 */
async function shot(page: Page, name: string) {
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
  });
  await page.waitForTimeout(120);
  await page.screenshot({ path: path.join(OUT_DIR, `${name}.png`), fullPage: false });
}

async function expectNoHorizontalOverflow(page: Page) {
  const metrics = await page.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    clientWidth: document.documentElement.clientWidth,
  }));
  expect(metrics.scrollWidth).toBeLessThanOrEqual(metrics.clientWidth + 1);
}

async function login(page: Page) {
  await page.goto("/");
  await page.getByLabel("Usuario").fill("ana");
  await page.getByLabel("Contraseña").fill("contrasena-segura");
  await page.getByRole("button", { name: "Entrar" }).click();
  await page.getByRole("navigation", { name: "Navegación principal" }).waitFor();
}

test.describe("capturas de pantallas administrativas", () => {
  test.skip(!process.env.VISUAL_REVIEW, "Define VISUAL_REVIEW=1 para capturar.");

  test.beforeAll(async () => {
    await mkdir(OUT_DIR, { recursive: true });
  });

  test("reportes, compras, gastos, devoluciones y gestión", async ({ page }) => {
    test.setTimeout(180_000);
    const consoleErrors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") consoleErrors.push(message.text());
    });

    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockAdminBridge(page);
    await login(page);

    const nav = page.getByRole("navigation", { name: "Navegación principal" });

    await nav.getByRole("button", { name: "Reportes" }).click();
    await expect(page.getByRole("region", { name: "Reporte de ventas" })).toBeVisible();
    await shot(page, "31-reportes-1440");
    await expectNoHorizontalOverflow(page);

    await nav.getByRole("button", { name: "Compras" }).click();
    await expect(page.getByRole("heading", { name: "Compras" })).toBeVisible();
    await shot(page, "32-compras-1440");
    await expectNoHorizontalOverflow(page);

    await nav.getByRole("button", { name: "Productos" }).click();
    await page.getByRole("cell", { name: "CAF-001" }).click();
    await shot(page, "33-productos-1440");
    await expectNoHorizontalOverflow(page);

    await page.setViewportSize({ width: 1024, height: 700 });

    await nav.getByRole("button", { name: "Reportes" }).click();
    await expect(page.getByRole("region", { name: "Reporte de ventas" })).toBeVisible();
    await shot(page, "34-reportes-1024x700");
    await expectNoHorizontalOverflow(page);

    await nav.getByRole("button", { name: "Gastos" }).click();
    await shot(page, "35-gastos-1024x700");
    await expectNoHorizontalOverflow(page);

    await nav.getByRole("button", { name: "Devoluciones" }).click();
    await shot(page, "36-devoluciones-1024x700");
    await expectNoHorizontalOverflow(page);

    await nav.getByRole("button", { name: "Gestión" }).click();
    await shot(page, "37-gestion-1024x700");
    await expectNoHorizontalOverflow(page);

    await page.setViewportSize({ width: 768, height: 1024 });

    await nav.getByRole("button", { name: "Reportes" }).click();
    await shot(page, "38-reportes-768");
    await expectNoHorizontalOverflow(page);

    await nav.getByRole("button", { name: "Gestión" }).click();
    await shot(page, "39-gestion-768");
    await expectNoHorizontalOverflow(page);

    expect(consoleErrors).toEqual([]);
  });
});
