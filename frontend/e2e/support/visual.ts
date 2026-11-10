import {
  actor,
  auditEvents,
  categories,
  closedShift,
  groupsByProduct,
  movements,
  optionsByGroup,
  products,
  shift,
  variantsByProduct,
} from "./fixtures";

import { expect, type Page } from "@playwright/test";
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

export const OUT_DIR = path.resolve(
  process.cwd(),
  "..",
  ".devkit",
  "artifacts",
  "design-review",
  "after",
);

export async function mockBridge(page: Page, initialized = true) {
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
    {
      initialized,
      actor,
      // Los fixtures usan `unit_price` como precio al público; el mock añade los
      // campos que Rust calcula para que la tarjeta muestre el mismo importe.
      products: products.map((product) => {
        const variants = (variantsByProduct[product.id] ?? []) as { unit_price: string }[];
        return {
          ...product,
          public_unit_total: product.unit_price,
          active_variants: variants.length,
          variant_public_from: variants.length
            ? Math.min(...variants.map((variant) => Number(variant.unit_price))).toFixed(2)
            : null,
          has_modifiers: (groupsByProduct[product.id] ?? []).length > 0,
        };
      }),
      categories,
      variantsByProduct,
      groupsByProduct,
      optionsByGroup,
      shift,
      closedShift,
      movements,
      auditEvents,
    },
  );
}

/**
 * Captura estable: espera a que las fuentes estén listas, a que React haya
 * aplicado el cambio en dos frames consecutivos y a que el compositor pinte el
 * estado final. Sin esta espera el riel de navegación podía aparecer con el
 * resaltado del estado anterior en la captura.
 */
export async function shot(page: Page, name: string) {
  await page.evaluate(async () => {
    await document.fonts.ready;
    await new Promise<void>((resolve) =>
      requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
    );
  });
  await page.waitForTimeout(120);
  await page.screenshot({ path: path.join(OUT_DIR, `${name}.png`), fullPage: false });
}

export async function expectNoHorizontalOverflow(page: Page) {
  const metrics = await page.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    clientWidth: document.documentElement.clientWidth,
  }));
  expect(metrics.scrollWidth).toBeLessThanOrEqual(metrics.clientWidth + 1);
}

export async function textContrast(page: Page, selector: string, backgroundSelector: string) {
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

export async function borderContrast(page: Page, selector: string, backgroundSelector: string) {
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

export async function waitForInventory(page: Page) {
  // El panel muestra "…" y "Consultando inventario…" mientras llega la lectura.
  // Una captura en ese estado no es evidencia del diseño final.
  await expect(page.locator(".stock-figure")).toHaveText(/^\d/);
  await expect(page.getByText("Consultando inventario…")).toHaveCount(0);
}

export async function login(page: Page) {
  await page.goto("/");
  await expect(page.getByRole("heading", { name: "Entrar a la caja" })).toBeVisible();
  await page.getByLabel("Usuario").fill("ana");
  await page.getByLabel("Contraseña").fill("contrasena-segura");
  await page.getByRole("button", { name: "Entrar" }).click();
  await expect(page.getByRole("navigation", { name: "Navegación principal" })).toBeVisible();
}

export async function addProducts(page: Page) {
  await page.getByRole("button", { name: /Café americano, CAF-001/ }).click();
  await page.getByRole("button", { name: /Croissant de mantequilla, PAN-010/ }).click();
  await page.getByRole("button", { name: /Concha de vainilla, PAN-011/ }).click();
  await page.getByRole("button", { name: /Agua mineral 600 ml, REF-020/ }).click();
}
