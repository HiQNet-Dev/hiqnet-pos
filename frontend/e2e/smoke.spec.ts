import { expect, test } from "@playwright/test";

const commercial = {
  plan: "trial",
  status: "trial",
  valid_until: "2026-12-03T00:00:00Z",
  features: ["catalog", "sales"],
  read_only: false,
};

/**
 * Smoke de navegador con un puente Tauri simulado. No sustituye al E2E nativo
 * (`tools/windows-native-e2e`), que se ejecuta contra el ejecutable real.
 */
async function mockBridge(page: import("@playwright/test").Page, initialized: boolean) {
  await page.addInitScript(
    ({ initialized, commercial }) => {
      const actor = {
        user_id: "u-1",
        organization_id: "o-1",
        branch_id: "b-1",
        display_name: "Ana Propietaria",
        role: "owner",
        permissions: [
          "catalog.manage",
          "sales.sell",
          "inventory.view",
          "cash.shift.view",
          "audit.view",
          "backups.manage",
        ],
      };

      const product = {
        id: "p-1",
        sku: "CAF-001",
        name: "Café americano",
        unit_price: "12.50",
        public_unit_total: "14.50",
        tax_rate_bp: 1600,
        active: true,
        active_variants: 0,
        variant_public_from: null,
        has_modifiers: false,
      };

      const responses: Record<string, unknown> = {
        app_status: {
          initialized,
          product_version: "0.1.0",
          commercial: initialized ? commercial : null,
          user_count: initialized ? 1 : 0,
        },
        current_session: null,
        login: actor,
        logout: null,
        branch_summary: { id: "b-1", name: "Matriz", code: "MX-01" },
        list_products: [product],
        list_categories: [],
        list_stock_levels: [{ product_id: "p-1", quantity: "20.000" }],
        list_product_variants: [],
        list_modifier_groups: [],
        list_cash_registers: [],
        list_cash_shift_history: [],
        active_cash_shift: null,
        list_audit_events: [],
        commercial_state: commercial,
      };

      // Puente mínimo usado por @tauri-apps/api cuando corre en navegador.
      Object.assign(window, {
        __TAURI_INTERNALS__: {
          invoke: (command: string) => Promise.resolve(responses[command] ?? null),
          transformCallback: (callback: unknown) => callback,
        },
      });
    },
    { initialized, commercial },
  );
}

test("muestra la configuración inicial de una instalación nueva", async ({ page }) => {
  await mockBridge(page, false);
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "Configura tu negocio" })).toBeVisible();
  await expect(page.getByLabel("Nombre del negocio")).toBeVisible();
});

test("permite iniciar sesión y navegar al catálogo", async ({ page }) => {
  await mockBridge(page, true);
  await page.goto("/");

  await page.getByLabel("Usuario").fill("propietario");
  await page.getByLabel("Contraseña").fill("contrasena-segura");
  await page.getByRole("button", { name: "Entrar" }).click();

  const nav = page.getByRole("navigation", { name: "Navegación principal" });
  await expect(page.getByRole("heading", { name: "Ventas" })).toBeVisible();
  await expect(nav.getByRole("button", { name: "Productos" })).toBeVisible();
  await expect(page.getByRole("button", { name: /Café americano, CAF-001/ })).toBeVisible();

  await nav.getByRole("button", { name: "Productos" }).click();
  await expect(page.getByRole("heading", { name: "Productos y catálogo" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "CAF-001" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "$12.50" })).toBeVisible();
});
