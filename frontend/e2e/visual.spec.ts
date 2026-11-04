import { expect, test, type Page } from "@playwright/test";
import { mkdir } from "node:fs/promises";
import path from "node:path";

/**
 * Revisión visual del shell con un puente Tauri simulado.
 *
 * No sustituye al E2E nativo (`tools/windows-native-e2e`): aquí se verifica la
 * composición real del webview (layout, densidad, desbordes y estados) con
 * datos representativos, en las resoluciones de escritorio y tablet.
 *
 * Ejecución:
 *   VISUAL_REVIEW=1 pnpm exec playwright test e2e/visual.spec.ts
 * Las capturas se guardan en `.devkit/artifacts/design-review/`.
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
    "inventory.view",
    "inventory.manage",
    "cash.shift.view",
    "cash.shift.open",
    "cash.shift.close",
    "cash.movement.record",
    "cash.register.manage",
    "audit.view",
    "backups.manage",
  ],
};

const categories = [
  { id: "c-1", name: "Bebidas calientes", sort_order: 1, active: true },
  { id: "c-2", name: "Panadería", sort_order: 2, active: true },
  { id: "c-3", name: "Refrigerados", sort_order: 3, active: true },
  { id: "c-4", name: "Combos", sort_order: 4, active: true },
];

const products = [
  {
    id: "p-1",
    sku: "CAF-001",
    name: "Café americano",
    unit_price: "12.50",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-1",
  },
  {
    id: "p-2",
    sku: "CAF-002",
    name: "Capuchino",
    unit_price: "18.00",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-1",
  },
  {
    id: "p-3",
    sku: "PAN-010",
    name: "Croissant de mantequilla",
    unit_price: "9.80",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-2",
  },
  {
    id: "p-4",
    sku: "PAN-011",
    name: "Concha de vainilla",
    unit_price: "7.50",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-2",
  },
  {
    id: "p-5",
    sku: "REF-020",
    name: "Agua mineral 600 ml",
    unit_price: "6.00",
    tax_rate_bp: 0,
    active: true,
    category_id: "c-3",
  },
  {
    id: "p-6",
    sku: "REF-021",
    name: "Jugo de naranja 355 ml",
    unit_price: "11.00",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-3",
  },
  {
    id: "p-7",
    sku: "COM-001",
    name: "Combo desayuno",
    unit_price: "39.00",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-4",
  },
  {
    id: "p-8",
    sku: "CAF-003",
    name: "Latte grande de temporada con leche de avena",
    unit_price: "24.00",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-1",
  },
];

const variantsByProduct: Record<string, unknown[]> = {
  "p-2": [
    {
      id: "v-1",
      product_id: "p-2",
      name: "Chico",
      sku: "CAF-002-CH",
      unit_price: "18.00",
      active: true,
      sort_order: 1,
    },
    {
      id: "v-2",
      product_id: "p-2",
      name: "Grande",
      sku: "CAF-002-GR",
      unit_price: "22.00",
      active: true,
      sort_order: 2,
    },
  ],
  "p-7": [
    {
      id: "v-3",
      product_id: "p-7",
      name: "Para aquí",
      sku: "COM-001-AQ",
      unit_price: "39.00",
      active: true,
      sort_order: 1,
    },
    {
      id: "v-4",
      product_id: "p-7",
      name: "Para llevar",
      sku: "COM-001-LL",
      unit_price: "39.00",
      active: true,
      sort_order: 2,
    },
  ],
};

const groupsByProduct: Record<string, unknown[]> = {
  "p-2": [
    { id: "g-1", name: "Leche", min: 1, max: 1, sort_order: 1, active: true },
    { id: "g-2", name: "Extras", min: 0, max: 3, sort_order: 2, active: true },
  ],
  "p-7": [
    { id: "g-3", name: "Bebida", min: 1, max: 1, sort_order: 1, active: true },
  ],
};

const optionsByGroup: Record<string, unknown[]> = {
  "g-1": [
    {
      id: "o-1",
      group_id: "g-1",
      name: "Entera",
      kind: "choice",
      price_delta: "0.00",
      sort_order: 1,
      active: true,
    },
    {
      id: "o-2",
      group_id: "g-1",
      name: "Deslactosada",
      kind: "choice",
      price_delta: "0.00",
      sort_order: 2,
      active: true,
    },
    {
      id: "o-3",
      group_id: "g-1",
      name: "Avena",
      kind: "choice",
      price_delta: "5.00",
      sort_order: 3,
      active: true,
    },
  ],
  "g-2": [
    {
      id: "o-4",
      group_id: "g-2",
      name: "Shot extra",
      kind: "extra",
      price_delta: "8.00",
      sort_order: 1,
      active: true,
    },
    {
      id: "o-5",
      group_id: "g-2",
      name: "Sin azúcar",
      kind: "omission",
      price_delta: "0.00",
      sort_order: 2,
      active: true,
    },
    {
      id: "o-6",
      group_id: "g-2",
      name: "Canela",
      kind: "extra",
      price_delta: "2.00",
      sort_order: 3,
      active: true,
    },
  ],
  "g-3": [
    {
      id: "o-7",
      group_id: "g-3",
      name: "Café americano",
      kind: "choice",
      price_delta: "0.00",
      sort_order: 1,
      active: true,
    },
    {
      id: "o-8",
      group_id: "g-3",
      name: "Jugo de naranja",
      kind: "choice",
      price_delta: "4.00",
      sort_order: 2,
      active: true,
    },
  ],
};

const shift = {
  id: "s-1",
  register_id: "r-1",
  status: "open",
  opened_by: "u-1",
  opened_at: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
  closed_by: null,
  closed_at: null,
  opening_amount: "500.00",
  expected_amount: "1842.80",
  entry_total: "1342.80",
  exit_total: "0.00",
  counted_amount: null,
  difference_minor: null,
  movements: [
    {
      id: "m-1",
      kind: "in",
      amount: "200.00",
      reason: "Cambio adicional",
      occurred_at: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
    },
    {
      id: "m-2",
      kind: "out",
      amount: "50.00",
      reason: "Compra de hielo",
      occurred_at: new Date(Date.now() - 60 * 60 * 1000).toISOString(),
    },
  ],
};

const closedShift = {
  ...shift,
  id: "s-0",
  status: "closed",
  closed_by: "u-1",
  closed_at: new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(),
  counted_amount: "1490.00",
  difference_minor: -20,
};

const movements = [
  {
    id: "im-1",
    product_id: "p-1",
    kind: "outbound",
    quantity: "2.000",
    reason: "Venta F-000128",
    occurred_at: new Date(Date.now() - 40 * 60 * 1000).toISOString(),
  },
  {
    id: "im-2",
    product_id: "p-1",
    kind: "inbound",
    quantity: "24.000",
    reason: "Compra a proveedor",
    occurred_at: new Date(Date.now() - 5 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: "im-3",
    product_id: "p-1",
    kind: "adjustment_out",
    quantity: "1.000",
    reason: "Producto derramado",
    occurred_at: new Date(Date.now() - 26 * 60 * 60 * 1000).toISOString(),
  },
];

const auditEvents = [
  {
    id: "a-1",
    action: "sales.completed",
    entity_type: "sale",
    entity_id: "sale-128",
    actor_user_id: "u-1",
    occurred_at: new Date(Date.now() - 12 * 60 * 1000).toISOString(),
  },
  {
    id: "a-2",
    action: "inventory.movement_registered",
    entity_type: "inventory_movement",
    entity_id: "im-1",
    actor_user_id: "u-1",
    occurred_at: new Date(Date.now() - 40 * 60 * 1000).toISOString(),
  },
  {
    id: "a-3",
    action: "cash.shift_opened",
    entity_type: "cash_shift",
    entity_id: "s-1",
    actor_user_id: "u-1",
    occurred_at: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: "a-4",
    action: "session.logged_in",
    entity_type: "session",
    entity_id: "sess-1",
    actor_user_id: "u-1",
    occurred_at: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
  },
];

async function mockBridge(page: Page, initialized = true) {
  await page.addInitScript(
    ({ initialized, actor, products, categories, variantsByProduct, groupsByProduct, optionsByGroup, shift, closedShift, movements, auditEvents }) => {
      let shiftOpen = true;

      const optionsById: Record<
        string,
        { name: string; kind: string; price_delta: string }
      > = {};
      for (const list of Object.values(optionsByGroup)) {
        for (const option of list as Array<{
          id: string;
          name: string;
          kind: string;
          price_delta: string;
        }>) {
          optionsById[option.id] = option;
        }
      }

      function priceFor(productId: string, selectedIds: string[]) {
        const product = products.find((item) => item.id === productId) ?? products[0]!;
        const delta = selectedIds.reduce(
          (sum, id) => sum + Number(optionsById[id]?.price_delta ?? 0),
          0,
        );
        const gross = Number(product.unit_price) + delta;
        const taxMinor = Math.round(
          (gross * 100 * product.tax_rate_bp) / (10000 + product.tax_rate_bp),
        );
        return {
          product,
          gross: gross.toFixed(2),
          tax: (taxMinor / 100).toFixed(2),
          modifiers: delta.toFixed(2),
        };
      }

      const responses: Record<string, unknown> = {
        app_status: {
          initialized,
          product_version: "0.1.0",
          commercial: initialized
            ? {
                plan: "pro",
                status: "active",
                valid_until: "2027-11-04T00:00:00Z",
                features: ["catalog", "sales", "inventory", "local_backups"],
                read_only: false,
              }
            : null,
          user_count: initialized ? 1 : 0,
        },
        current_session: null,
        login: actor,
        logout: null,
        bootstrap: {
          organization_id: "o-1",
          branch_id: "b-1",
          user_id: "u-1",
          trial_ends_at: "2026-12-04T00:00:00Z",
        },
        branch_summary: { id: "b-1", name: "Sucursal Centro", code: "MX-01" },
        list_products: products,
        list_categories: categories,
        list_stock_levels: products.map((product) => ({
          product_id: product.id,
          quantity: product.id === "p-8" ? "0.000" : "18.000",
        })),
        list_cash_registers: [{ id: "r-1", name: "Caja principal", code: "CJ-01", active: true }],
        active_cash_shift: shift,
        list_cash_shift_history: [closedShift],
        list_audit_events: auditEvents,
        commercial_state: {
          plan: "pro",
          status: "active",
          valid_until: "2027-11-04T00:00:00Z",
          features: ["catalog", "sales", "inventory", "local_backups"],
          read_only: false,
        },
        current_stock: "18.000",
        list_inventory_movements: movements,
        backup_database: "C:/respaldos/hiqnet-pos-2026-11-04.sqlite",
      };

      Object.assign(window, {
        __TAURI_INTERNALS__: {
          invoke: (command: string, args?: Record<string, unknown>) => {
            const request = args ?? {};
            if (command === "list_product_variants") {
              return Promise.resolve(variantsByProduct[String(request.product_id)] ?? []);
            }
            if (command === "list_modifier_groups") {
              return Promise.resolve(groupsByProduct[String(request.product_id)] ?? []);
            }
            if (command === "list_modifier_options") {
              return Promise.resolve(optionsByGroup[String(request.group_id)] ?? []);
            }
            if (command === "configure_product") {
              const draft = request.draft as
                | { product_id?: string; selected_option_ids?: string[] }
                | undefined;
              const selected = draft?.selected_option_ids ?? [];
              const productId = String(draft?.product_id ?? "p-1");
              const priced = priceFor(productId, selected);
              return Promise.resolve({
                product_id: priced.product.id,
                product_name: priced.product.name,
                variant_id: null,
                variant_name: null,
                base_price: priced.product.unit_price,
                modifiers_total: priced.modifiers,
                tax: priced.tax,
                unit_price: priced.gross,
                tax_rate_bp: priced.product.tax_rate_bp,
                selected_options: selected.map((id) => {
                  const option = optionsById[id];
                  return {
                    selected_option_id: id,
                    name: option?.name ?? "Opción",
                    kind: option?.kind ?? "choice",
                    price_delta: option?.price_delta ?? "0.00",
                  };
                }),
              });
            }
            if (command === "active_cash_shift") return Promise.resolve(shiftOpen ? shift : null);
            if (command === "checkout_sale") {
              const draft = request.draft as {
                items: Array<{
                  product_id: string;
                  selected_option_ids: string[];
                  quantity: string;
                }>;
                cash_received: string;
              };
              let totalMinor = 0;
              let taxMinor = 0;
              const items = draft.items.map((item) => {
                const priced = priceFor(item.product_id, item.selected_option_ids);
                const quantity = Number(item.quantity);
                const unitMinor = Math.round(Number(priced.gross) * 100);
                const unitTaxMinor = Math.round(Number(priced.tax) * 100);
                totalMinor += unitMinor * quantity;
                taxMinor += unitTaxMinor * quantity;
                return {
                  id: `si-${item.product_id}`,
                  product_id: item.product_id,
                  product_name: priced.product.name,
                  variant_name: null,
                  quantity: item.quantity,
                  unit_price: priced.gross,
                  unit_tax: priced.tax,
                  line_total: ((unitMinor * quantity) / 100).toFixed(2),
                  modifiers_summary: item.selected_option_ids
                    .map((id) => optionsById[id]?.name)
                    .filter(Boolean)
                    .join(", "),
                };
              });
              const cashMinor = Math.round(Number(draft.cash_received) * 100);
              return Promise.resolve({
                id: "sale-128",
                folio: "F-000128",
                status: "completed",
                subtotal: ((totalMinor - taxMinor) / 100).toFixed(2),
                tax: (taxMinor / 100).toFixed(2),
                total: (totalMinor / 100).toFixed(2),
                items,
                payment: {
                  method: "cash",
                  amount: draft.cash_received,
                  change: ((cashMinor - totalMinor) / 100).toFixed(2),
                },
              });
            }
            if (command === "close_cash_shift") {
              shiftOpen = false;
              return Promise.resolve({
                ...shift,
                status: "closed",
                closed_by: "u-1",
                closed_at: new Date().toISOString(),
                counted_amount: "1840.00",
                difference_minor: -280,
              });
            }
            if (command === "register_cash_movement") return Promise.resolve(null);
            if (command === "create_cash_register") {
              return Promise.resolve({ id: "r-2", name: "Caja 2", code: "CJ-02", active: true });
            }
            if (command === "create_product" || command === "create_category") {
              return Promise.resolve(null);
            }
            if (command === "record_inventory_movement") {
              return Promise.resolve(movements[0]);
            }
            return Promise.resolve(responses[command] ?? null);
          },
          transformCallback: (callback: unknown) => callback,
        },
      });
    },
    { initialized, actor, products, categories, variantsByProduct, groupsByProduct, optionsByGroup, shift, closedShift, movements, auditEvents },
  );
}

async function shot(page: Page, name: string) {
  await page.screenshot({ path: path.join(OUT_DIR, `${name}.png`), fullPage: false });
}

async function expectNoHorizontalOverflow(page: Page) {
  const metrics = await page.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    clientWidth: document.documentElement.clientWidth,
  }));
  expect(metrics.scrollWidth).toBeLessThanOrEqual(metrics.clientWidth + 1);
}

async function textContrast(page: Page, selector: string, backgroundSelector: string) {
  const result = await page.evaluate(
    ({ selector, backgroundSelector }) => {
      function channels(value: string): [number, number, number] {
        const parts = value.match(/[\d.]+/g)?.map(Number) ?? [0, 0, 0];
        return [parts[0] ?? 0, parts[1] ?? 0, parts[2] ?? 0];
      }
      function luminance(value: string): number {
        const [r, g, b] = channels(value).map((channel) => {
          const normalized = channel / 255;
          return normalized <= 0.03928
            ? normalized / 12.92
            : Math.pow((normalized + 0.055) / 1.055, 2.4);
        });
        return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
      }
      const element = document.querySelector(selector);
      const background = document.querySelector(backgroundSelector);
      if (!element || !background) return null;
      const style = getComputedStyle(element);
      const backgroundStyle = getComputedStyle(background);
      const foreground = luminance(style.color);
      const backdrop = luminance(backgroundStyle.backgroundColor);
      const lighter = Math.max(foreground, backdrop);
      const darker = Math.min(foreground, backdrop);
      return {
        ratio: (lighter + 0.05) / (darker + 0.05),
        color: style.color,
        background: backgroundStyle.backgroundColor,
        fontSize: Number.parseFloat(style.fontSize),
        fontWeight: Number(style.fontWeight),
      };
    },
    { selector, backgroundSelector },
  );
  expect(result, `${selector} should be visible`).not.toBeNull();
  return result!;
}

async function borderContrast(page: Page, selector: string, backgroundSelector: string) {
  const result = await page.evaluate(
    ({ selector, backgroundSelector }) => {
      function channels(value: string): [number, number, number] {
        const parts = value.match(/[\d.]+/g)?.map(Number) ?? [0, 0, 0];
        return [parts[0] ?? 0, parts[1] ?? 0, parts[2] ?? 0];
      }
      function luminance(value: string): number {
        const [r, g, b] = channels(value).map((channel) => {
          const normalized = channel / 255;
          return normalized <= 0.03928
            ? normalized / 12.92
            : Math.pow((normalized + 0.055) / 1.055, 2.4);
        });
        return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
      }
      const element = document.querySelector(selector);
      const background = document.querySelector(backgroundSelector);
      if (!element || !background) return null;
      const foreground = luminance(getComputedStyle(element).borderTopColor);
      const backdrop = luminance(getComputedStyle(background).backgroundColor);
      const lighter = Math.max(foreground, backdrop);
      const darker = Math.min(foreground, backdrop);
      return { ratio: (lighter + 0.05) / (darker + 0.05) };
    },
    { selector, backgroundSelector },
  );
  expect(result, `${selector} should be visible`).not.toBeNull();
  return result!;
}

async function login(page: Page) {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Entrar a la caja" })).toBeVisible();
  await page.getByLabel("Usuario").fill("ana");
  await page.getByLabel("Contraseña").fill("contrasena-segura");
  await page.getByRole("button", { name: "Entrar" }).click();
  await expect(page.getByRole("navigation", { name: "Navegación principal" })).toBeVisible();
}

async function addProducts(page: Page) {
  await page.getByRole("button", { name: /Café americano, CAF-001/ }).click();
  await page.getByRole("button", { name: /Croissant de mantequilla, PAN-010/ }).click();
  await page.getByRole("button", { name: /Concha de vainilla, PAN-011/ }).click();
  await page.getByRole("button", { name: /Agua mineral 600 ml, REF-020/ }).click();
}

test.describe("revisión visual del shell", () => {
  test.skip(!process.env.VISUAL_REVIEW, "Define VISUAL_REVIEW=1 para capturar.");

  test.beforeAll(async () => {
    await mkdir(OUT_DIR, { recursive: true });
  });

  test("recorre las pantallas operativas y sus resoluciones", async ({ page }) => {
    test.setTimeout(180_000);
    const consoleErrors: string[] = [];
    page.on("console", (message) => {
      if (message.type() === "error") consoleErrors.push(message.text());
    });

    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockBridge(page, true);
    await page.goto("/");
    await shot(page, "01-login-1920");
    await expectNoHorizontalOverflow(page);

    await login(page);
    await shot(page, "03-ventas-vacio-1920");
    await expectNoHorizontalOverflow(page);

    await addProducts(page);
    await shot(page, "04-ventas-ticket-1920");
    await expectNoHorizontalOverflow(page);

    await page.getByRole("button", { name: /Capuchino, CAF-002/ }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
    await page.getByText("Avena", { exact: true }).click();
    await shot(page, "05-configurador-1920");

    await page.getByRole("button", { name: "Agregar al carrito" }).click();
    await shot(page, "06-ventas-ticket-multiple-1920");

    await page.getByRole("button", { name: /^Cobrar/ }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
    await shot(page, "07-checkout-1920");

    await page.getByRole("button", { name: "Confirmar cobro" }).click();
    await expect(page.getByRole("heading", { name: /Venta F-000128/ })).toBeVisible();
    await shot(page, "08-venta-confirmada-1920");
    await page.getByRole("button", { name: "Nueva venta" }).click();
    await page.getByRole("button", { name: "Descartar mensaje" }).click();

    // Responsive con ticket y turno activos, antes de cerrar la caja.
    await addProducts(page);
    await page.setViewportSize({ width: 1440, height: 900 });
    await shot(page, "13-ventas-1440");
    await expectNoHorizontalOverflow(page);

    await page.setViewportSize({ width: 1366, height: 768 });
    await shot(page, "14-ventas-1366");
    await expectNoHorizontalOverflow(page);
    const collectBox = await page.getByRole("button", { name: /^Cobrar/ }).boundingBox();
    expect(collectBox).not.toBeNull();
    expect(collectBox!.y + collectBox!.height).toBeLessThanOrEqual(768);
    await expect(page.getByRole("button", { name: "Cerrar sesión" })).toBeInViewport();

    await page.setViewportSize({ width: 1024, height: 768 });
    await shot(page, "15-ventas-1024");
    await expectNoHorizontalOverflow(page);
    await expect(page.getByRole("navigation").getByRole("button", { name: "Caja" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Cerrar sesión" })).toBeInViewport();

    await page.getByRole("navigation").getByRole("button", { name: "Caja" }).click();
    await expect(page.getByRole("region", { name: "Operación del turno" })).toBeVisible();
    await shot(page, "15b-caja-1024");
    await expectNoHorizontalOverflow(page);

    await page.getByRole("navigation").getByRole("button", { name: "Inventario" }).click();
    await page.getByRole("combobox", { name: "Producto" }).selectOption("p-1");
    await expect(page.getByRole("region", { name: "Existencias del producto" })).toBeVisible();
    await shot(page, "15c-inventario-1024");
    await expectNoHorizontalOverflow(page);

    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.getByRole("navigation").getByRole("button", { name: "Inventario" }).click();
    await page.getByRole("combobox", { name: "Producto" }).selectOption("p-1");
    await expect(page.getByRole("region", { name: "Existencias del producto" })).toBeVisible();
    await shot(page, "09-inventario-1920");
    await expectNoHorizontalOverflow(page);

    await page.getByRole("navigation").getByRole("button", { name: "Cajas y turnos" }).click();
    await expect(page.getByRole("region", { name: "Operación del turno" })).toBeVisible();
    await shot(page, "10-caja-1920");

    await page.getByLabel("Efectivo contado").fill("1840.00");
    await page.getByRole("button", { name: "Cerrar turno" }).click();
    await expect(page.getByRole("heading", { name: "Resultado del arqueo" })).toBeVisible();
    await expect(page.getByRole("heading", { name: "Abrir turno" })).toHaveCount(0);
    await shot(page, "10b-arqueo-1920");

    await page.getByRole("navigation").getByRole("button", { name: "Gestión" }).click();
    await expect(page.getByRole("heading", { name: "Gestión y auditoría" })).toBeVisible();
    await expect(page.getByText("Venta confirmada")).toBeVisible();
    await expect(page.getByText("Activa")).toBeVisible();
    await expect(page.getByText("sale.completed")).toHaveCount(0);
    await shot(page, "11-gestion-1920");

    await page.getByRole("navigation").getByRole("button", { name: "Productos" }).click();
    await expect(page.getByRole("heading", { name: "Productos y catálogo" })).toBeVisible();
    await shot(page, "12-productos-1920");

    expect(consoleErrors).toEqual([]);
  });

  test("onboarding de primera ejecución", async ({ page }) => {
    await page.setViewportSize({ width: 1366, height: 768 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockBridge(page, false);
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Configura tu negocio" })).toBeVisible();
    await shot(page, "02-onboarding-1366");
    await expectNoHorizontalOverflow(page);
  });

  test("tablet conserva carrito y acción principal", async ({ page }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 768, height: 1024 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockBridge(page, true);
    await login(page);
    await expect(page.getByRole("button", { name: "Combos" })).toBeVisible();
    await expect(page.getByRole("navigation").getByRole("button", { name: "Caja" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Cerrar sesión" })).toBeInViewport();
    await shot(page, "16-tablet-catalogo");
    await addProducts(page);

    const mobileBar = page.locator(".sales-mobile-bar");
    await expect(mobileBar).toBeVisible();
    const barBox = await mobileBar.boundingBox();
    expect(barBox).not.toBeNull();
    expect(barBox!.y + barBox!.height).toBeLessThanOrEqual(1024);

    await page.getByRole("button", { name: /Ver ticket/ }).click();
    await shot(page, "17-tablet-ticket");
    await expectNoHorizontalOverflow(page);

    await page.getByRole("navigation").getByRole("button", { name: "Caja" }).click();
    await expect(page.getByRole("region", { name: "Operación del turno" })).toBeVisible();
    await shot(page, "18-tablet-caja");
    await expectNoHorizontalOverflow(page);

    await page.getByRole("navigation").getByRole("button", { name: "Inventario" }).click();
    await page.getByRole("combobox", { name: "Producto" }).selectOption("p-1");
    await expect(page.getByRole("region", { name: "Existencias del producto" })).toBeVisible();
    await shot(page, "19-tablet-inventario");
    await expectNoHorizontalOverflow(page);

    await page.getByRole("navigation").getByRole("button", { name: "Productos" }).click();
    await expect(page.getByRole("heading", { name: "Productos y catálogo" })).toBeVisible();
    await shot(page, "20-tablet-productos");
    await expectNoHorizontalOverflow(page);
  });

  test("contraste y foco visibles en la operación", async ({ page }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 1366, height: 768 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockBridge(page, true);
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Entrar a la caja" })).toBeVisible();

    const fieldLabel = await textContrast(page, ".field > span", ".panel");
    expect(fieldLabel.ratio).toBeGreaterThanOrEqual(4.5);

    const inputBorder = await borderContrast(page, ".field input", ".panel");
    expect(inputBorder.ratio).toBeGreaterThanOrEqual(3);

    await page.getByLabel("Usuario").focus();
    const focus = await page.evaluate(() => {
      const style = getComputedStyle(document.activeElement as Element);
      return {
        style: style.outlineStyle,
        width: Number.parseFloat(style.outlineWidth),
        color: style.outlineColor,
      };
    });
    expect(focus.style).not.toBe("none");
    expect(focus.width).toBeGreaterThanOrEqual(2);

    await login(page);
    await addProducts(page);

    const sku = await textContrast(page, ".product-card-sku", ".product-card");
    expect(sku.ratio).toBeGreaterThanOrEqual(4.5);
    const stock = await textContrast(page, ".product-card-stock", ".product-card");
    expect(stock.ratio).toBeGreaterThanOrEqual(4.5);
    const subtitle = await textContrast(page, ".screen-subtitle", "body");
    expect(subtitle.ratio).toBeGreaterThanOrEqual(4.5);
    const primary = await textContrast(
      page,
      ".ticket-footer .button-primary",
      ".ticket-footer .button-primary",
    );
    expect(primary.ratio).toBeGreaterThanOrEqual(4.5);
  });
});
