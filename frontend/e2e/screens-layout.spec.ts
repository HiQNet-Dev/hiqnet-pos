import { expect, test, type Page } from "@playwright/test";
import { loginThroughBridge, mockShellBridge } from "./mock-shell";

/**
 * Detalles de composición que no se pueden comprobar en jsdom porque dependen
 * del layout real: densidad de una lista lateral y estados que se quedaban
 * colgados tras una interacción repetida.
 */

const nav = (page: Page) => page.getByRole("navigation", { name: "Navegación principal" });

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await mockShellBridge(page);
});

test("el historial de turnos cerrados usa el ancho del panel", async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 1080 });
  await loginThroughBridge(page);
  await nav(page).getByRole("button", { name: "Cajas y turnos" }).click();
  await expect(page.getByRole("region", { name: "Operación del turno" })).toBeVisible();

  const row = page.locator(".ledger-history > li").first();
  await expect(row).toBeVisible();

  const metrics = await page.evaluate(() => {
    const row = document.querySelector(".ledger-history > li") as HTMLElement;
    const main = row.querySelector(".ledger-main") as HTMLElement;
    const label = row.querySelector(".ledger-label") as HTMLElement;
    const date = row.querySelector(".ledger-sub-date") as HTMLElement;
    const lineHeight = parseFloat(getComputedStyle(label).lineHeight) || 17;
    return {
      rowWidth: row.getBoundingClientRect().width,
      mainWidth: main.getBoundingClientRect().width,
      labelLines: Math.round(label.getBoundingClientRect().height / lineHeight),
      dateLines: Math.round(
        date.getBoundingClientRect().height / (parseFloat(getComputedStyle(date).lineHeight) || 15),
      ),
    };
  });

  // La columna de texto debe quedarse con el espacio libre, no con la primera
  // columna de la rejilla del libro de movimientos (92 px).
  expect(metrics.mainWidth).toBeGreaterThan(metrics.rowWidth / 2);
  expect(metrics.labelLines).toBe(1);
  expect(metrics.dateLines).toBe(1);
});

test("repetir la selección de producto conserva la lectura de existencias", async ({ page }) => {
  await page.setViewportSize({ width: 1920, height: 1080 });
  await loginThroughBridge(page);
  await nav(page).getByRole("button", { name: "Inventario" }).click();

  const select = page.getByRole("combobox", { name: "Producto" });
  await select.selectOption("p-1");
  await expect(page.locator(".stock-figure")).toHaveText("20");

  // Volver a elegir el mismo producto no debe dejar el panel en "…".
  await select.selectOption("p-1");
  await page.waitForTimeout(200);
  await expect(page.locator(".stock-figure")).toHaveText("20");
  await expect(page.getByText("Consultando inventario…")).toHaveCount(0);
});

test("la auditoría traduce toda acción conocida y no repite el tipo de entidad", async ({
  page,
}) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await loginThroughBridge(page);
  await nav(page).getByRole("button", { name: "Gestión" }).click();
  await expect(page.getByRole("heading", { name: "Gestión y auditoría" })).toBeVisible();

  const entries = await page.evaluate(() =>
    Array.from(document.querySelectorAll(".audit-list li")).map((item) => ({
      action: item.getAttribute("data-action") ?? "",
      label: item.querySelector(".audit-action")?.textContent?.trim() ?? "",
      meta: item.querySelector(".audit-meta")?.textContent?.trim() ?? "",
    })),
  );
  expect(entries.length).toBeGreaterThan(0);

  for (const entry of entries) {
    // Nunca una clave técnica en pantalla, ni siquiera para acciones nuevas.
    expect(entry.label).not.toMatch(/[._]/);
    expect(entry.label.length).toBeGreaterThan(3);
    // La meta es fecha + referencia, sin repetir el sustantivo de la acción.
    expect(entry.meta).not.toMatch(/^(Venta|Caja|Producto|Turno de caja)\s/);
    expect(entry.meta.length).toBeGreaterThan(0);
  }

  // Cada evento del fixture debe tener su propia frase: una clave sin traducir
  // se vería como "Actividad registrada" repetida.
  const labels = new Set(entries.map((entry) => entry.label));
  expect(labels.size).toBe(entries.length);
});
