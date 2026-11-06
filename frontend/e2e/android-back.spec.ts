import { expect, test, type Page } from "@playwright/test";
import { loginThroughBridge, mockShellBridge } from "./mock-shell";

/**
 * Botón y gesto Back de Android.
 *
 * El evento nativo lo emite `AppPlugin` de Tauri; aquí se dispara sobre el canal
 * que registra la aplicación, con el mismo user agent de Android para que el
 * guardia se instale. La geometría y el proceso real se verifican en el
 * emulador; esta prueba protege la prioridad de capas y el ciclo de vida.
 */

test.use({
  userAgent:
    "Mozilla/5.0 (Linux; Android 17; Pixel Tablet) AppleWebKit/537.36 Chrome/141 Mobile Safari/537.36",
});

async function pressBack(page: Page) {
  await page.evaluate(() => {
    const channel = (window as unknown as { __hiqnetBack?: { onmessage?: (p: unknown) => void } })
      .__hiqnetBack;
    if (!channel?.onmessage) throw new Error("el listener de Back no está registrado");
    channel.onmessage({ canGoBack: false });
  });
  await page.waitForTimeout(120);
}

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await mockShellBridge(page);
});

test("registra un único listener nativo", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await loginThroughBridge(page);

  const registrations = await page.evaluate(
    () =>
      Number((window as unknown as Record<string, unknown>).__hiqnetBackRegistrations) || 0,
  );
  expect(registrations).toBe(1);
});

test("cierra solo el diálogo abierto y no navega", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await loginThroughBridge(page);

  await page.getByRole("button", { name: "Cambiar mi contraseña" }).click();
  const dialog = page.getByRole("dialog", { name: "Cambiar contraseña" });
  await expect(dialog).toBeVisible();

  await pressBack(page);
  await expect(dialog).toBeHidden();

  // Sin diálogo, Back no cambia de sección.
  await pressBack(page);
  await expect(page.getByRole("heading", { name: "Ventas" })).toBeVisible();
  await expect(page.getByRole("navigation", { name: "Navegación principal" })).toBeVisible();
});

test("en la vista de ticket vuelve al catálogo conservando las partidas", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 1280 });
  await loginThroughBridge(page);

  await page.getByRole("button", { name: /Café americano, CAF-001/ }).click();
  const sales = page.locator(".sales");
  await page.getByRole("button", { name: /Ver ticket \(1\)/ }).click();
  await expect(sales).toHaveAttribute("data-view", "ticket");

  await pressBack(page);

  await expect(sales).toHaveAttribute("data-view", "catalog");
  await expect(page.getByRole("button", { name: /Ver ticket \(1\)/ })).toBeVisible();
});

test("el diálogo tiene prioridad sobre la vista de ticket", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 1280 });
  await loginThroughBridge(page);

  await page.getByRole("button", { name: /Café americano, CAF-001/ }).click();
  const sales = page.locator(".sales");
  await page.getByRole("button", { name: /Ver ticket \(1\)/ }).click();
  await expect(sales).toHaveAttribute("data-view", "ticket");

  await page.getByRole("button", { name: "Vaciar" }).click();
  const dialog = page.getByRole("dialog", { name: "Vaciar ticket" });
  await expect(dialog).toBeVisible();

  // Primer Back: solo el diálogo.
  await pressBack(page);
  await expect(dialog).toBeHidden();
  await expect(sales).toHaveAttribute("data-view", "ticket");

  // Segundo Back: la vista.
  await pressBack(page);
  await expect(sales).toHaveAttribute("data-view", "catalog");
  await expect(page.getByRole("button", { name: /Ver ticket \(1\)/ })).toBeVisible();
});

test("no consume Back cuando no queda ninguna capa", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await loginThroughBridge(page);

  await page.getByRole("button", { name: "Cambiar mi contraseña" }).click();
  await expect(page.getByRole("dialog")).toBeVisible();
  await pressBack(page);
  await expect(page.getByRole("dialog")).toBeHidden();

  // La capa del diálogo se retiró: un segundo Back no debe cerrar nada más.
  await pressBack(page);
  await expect(page.getByRole("heading", { name: "Ventas" })).toBeVisible();
  await expect(page.getByRole("dialog")).toHaveCount(0);
});
