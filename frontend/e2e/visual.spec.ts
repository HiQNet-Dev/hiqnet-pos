import { expect, test } from "@playwright/test";
import { mkdir } from "node:fs/promises";
import {
  OUT_DIR,
  addProducts,
  borderContrast,
  expectNoHorizontalOverflow,
  login,
  mockBridge,
  shot,
  textContrast,
  waitForInventory,
} from "./support/visual";

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
    const search = page.getByRole("textbox", { name: /Buscar producto/ });
    await expect(search).toBeFocused();
    await expect(search).toHaveValue("");
    const dismiss = page.getByRole("button", { name: "Descartar mensaje" });
    if (await dismiss.count()) await dismiss.click();

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
    await waitForInventory(page);
    await shot(page, "15c-inventario-1024");
    await expectNoHorizontalOverflow(page);

    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.getByRole("navigation").getByRole("button", { name: "Inventario" }).click();
    await page.getByRole("combobox", { name: "Producto" }).selectOption("p-1");
    await expect(page.getByRole("region", { name: "Existencias del producto" })).toBeVisible();
    await waitForInventory(page);
    await shot(page, "09-inventario-1920");
    await expectNoHorizontalOverflow(page);

    await page.getByRole("navigation").getByRole("button", { name: "Cajas y turnos" }).click();
    await expect(page.getByRole("region", { name: "Operación del turno" })).toBeVisible();
    await shot(page, "10-caja-1920");

    await page.getByLabel("Efectivo contado").fill("1840.00");
    await page.getByRole("button", { name: "Revisar cierre" }).click();
    const review = page.getByRole("dialog");
    await expect(review).toBeVisible();
    await expect(review.getByText(/El turno se cerrará y el arqueo quedará registrado/)).toBeVisible();
    await review.getByRole("button", { name: "Cancelar" }).click();
    await expect(review).toHaveCount(0);
    await expect(page.getByRole("region", { name: "Operación del turno" })).toBeVisible();

    await page.getByRole("button", { name: "Revisar cierre" }).click();
    await page
      .getByRole("dialog")
      .getByRole("button", { name: "Cerrar turno y registrar arqueo" })
      .click();
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
    await waitForInventory(page);
    await shot(page, "19-tablet-inventario");
    await expectNoHorizontalOverflow(page);

    await page.getByRole("navigation").getByRole("button", { name: "Productos" }).click();
    await expect(page.getByRole("heading", { name: "Productos y catálogo" })).toBeVisible();
    await shot(page, "20-tablet-productos");
    await expectNoHorizontalOverflow(page);
  });

  test("navegación expandida, colapsada y riel compacto", async ({ page }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockBridge(page, true);
    await login(page);
    await addProducts(page);

    const nav = page.getByRole("navigation", { name: "Navegación principal" });
    await shot(page, "21-nav-expandida-1440");

    await page.getByRole("button", { name: "Contraer navegación" }).click();
    await expect(page.getByRole("button", { name: "Expandir navegación" })).toBeVisible();
    await shot(page, "22-nav-colapsada-1440");

    await nav.getByRole("button", { name: /^Ventas/ }).hover();
    await expect(nav.locator(".nav-tooltip").first()).toBeVisible();
    await shot(page, "23-nav-colapsada-tooltip-1440");

    await page.setViewportSize({ width: 1024, height: 768 });
    await shot(page, "24-nav-riel-1024");
    await expectNoHorizontalOverflow(page);

    await page.setViewportSize({ width: 768, height: 1024 });
    await shot(page, "25-nav-riel-768");
    await expectNoHorizontalOverflow(page);

    await page.setViewportSize({ width: 412, height: 915 });
    await shot(page, "26-topbar-412");
    await expectNoHorizontalOverflow(page);
  });

  test("ventana mínima de Windows y tablet vertical", async ({ page }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 1024, height: 700 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockBridge(page, true);
    await login(page);
    await addProducts(page);
    await shot(page, "27-ventas-1024x700");
    await expectNoHorizontalOverflow(page);

    const collect = await page.getByRole("button", { name: /^Cobrar/ }).boundingBox();
    expect(collect).not.toBeNull();
    expect(collect!.y + collect!.height).toBeLessThanOrEqual(700);

    await page.getByRole("button", { name: /^Cobrar/ }).click();
    await expect(page.getByRole("dialog")).toBeVisible();
    await shot(page, "28-checkout-1024x700");
    const dialog = await page.getByRole("dialog").boundingBox();
    expect(dialog!.y).toBeGreaterThanOrEqual(0);
    expect(dialog!.y + dialog!.height).toBeLessThanOrEqual(700);
    await page.keyboard.press("Escape");

    await page.getByRole("navigation").getByRole("button", { name: "Caja" }).click();
    await expect(page.getByRole("region", { name: "Operación del turno" })).toBeVisible();
    await shot(page, "29-caja-1024x700");
    await expectNoHorizontalOverflow(page);

    await page.setViewportSize({ width: 800, height: 1280 });
    await page.getByRole("navigation").getByRole("button", { name: "Ventas" }).click();
    await shot(page, "30-ventas-800x1280");
    await expectNoHorizontalOverflow(page);
  });

  test("autenticación en pantallas estrechas y con teclado abierto", async ({ page }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 412, height: 760 });
    await page.emulateMedia({ reducedMotion: "reduce" });
    await mockBridge(page, true);
    await page.goto("/");
    await expect(page.getByRole("heading", { name: "Entrar a la caja" })).toBeVisible();
    await shot(page, "40-login-412");
    await expectNoHorizontalOverflow(page);

    // Altura reducida: equivale al viewport que deja el teclado virtual.
    await page.setViewportSize({ width: 412, height: 360 });
    const scrollable = await page.evaluate(() => {
      const layout = document.querySelector(".auth-layout") as HTMLElement;
      return {
        scrollHeight: layout.scrollHeight,
        clientHeight: layout.clientHeight,
        rootScroll: document.documentElement.scrollTop,
      };
    });
    expect(scrollable.rootScroll).toBe(0);
    expect(scrollable.scrollHeight).toBeGreaterThanOrEqual(scrollable.clientHeight);

    // Todos los campos siguen alcanzables: el contenedor se desplaza, no se recorta.
    const field = page.getByLabel("Contraseña");
    await field.scrollIntoViewIfNeeded();
    await expect(field).toBeInViewport();
    await expect(page.getByRole("button", { name: "Entrar" })).toBeInViewport();
    await shot(page, "41-login-412-teclado");
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
