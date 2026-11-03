import { expect, test } from "@playwright/test";

const commercial = {
  plan: "trial",
  status: "trial",
  valid_until: "2026-12-03T00:00:00Z",
  features: ["catalog", "sales"],
  read_only: false,
};

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
          "audit.view",
          "backups.manage",
        ],
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
        list_products: [
          {
            id: "p-1",
            sku: "CAF-001",
            name: "Café americano",
            unit_price: "12.50",
            tax_rate_bp: 1600,
            active: true,
          },
        ],
        list_audit_events: [],
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

  await expect(
    page.getByRole("heading", { name: "Configura tu negocio" }),
  ).toBeVisible();
  await expect(page.getByLabel("Nombre del negocio")).toBeVisible();
});

test("permite iniciar sesión y muestra el catálogo", async ({ page }) => {
  await mockBridge(page, true);
  await page.goto("/");

  await page.getByLabel("Usuario").fill("propietario");
  await page.getByLabel("Contraseña").fill("contrasena-segura");
  await page.getByRole("button", { name: "Entrar" }).click();

  await expect(page.getByRole("heading", { name: "Productos" })).toBeVisible();
  await expect(page.getByRole("cell", { name: "CAF-001" })).toBeVisible();
  await expect(page.getByText("$12.50")).toBeVisible();
});
