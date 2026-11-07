import { expect, test, type Page } from "@playwright/test";
import {
  addOneProduct,
  loginThroughBridge,
  mockShellBridge,
} from "./mock-shell";

/**
 * Navegación lateral y barra superior.
 *
 * Estas pruebas miden geometría real del webview: no basta con comprobar
 * clases o atributos. El defecto que corrigen era un colapso que solo cambiaba
 * un ancho y dejaba las etiquetas encima del área de trabajo.
 */

const nav = (page: Page) => page.getByRole("navigation", { name: "Navegación principal" });
const sidenav = (page: Page) => page.locator(".sidenav");
const toggle = (page: Page) => page.getByRole("button", { name: /navegación$/ });

async function navWidth(page: Page): Promise<number> {
  const box = await sidenav(page).boundingBox();
  expect(box, "la navegación debe estar visible").not.toBeNull();
  return box!.width;
}

/** El ancho es el resultado de un cambio de estado de React: se espera a que
 * se estabilice en lugar de medirlo en el instante del clic. */
async function expectNavWidth(page: Page, width: number): Promise<void> {
  await expect.poll(() => navWidth(page), { timeout: 3_000 }).toBeCloseTo(width, 0);
}

async function expectNoHorizontalOverflow(page: Page): Promise<void> {
  const metrics = await page.evaluate(() => ({
    scrollWidth: document.documentElement.scrollWidth,
    clientWidth: document.documentElement.clientWidth,
    rootScrollTop:
      document.scrollingElement instanceof HTMLElement
        ? document.scrollingElement.scrollTop
        : 0,
  }));
  expect(metrics.scrollWidth).toBeLessThanOrEqual(metrics.clientWidth + 1);
  // El shell recorta la raíz: si algo desplaza el documento, el diseño se va.
  expect(metrics.rootScrollTop).toBe(0);
}

test.beforeEach(async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await mockShellBridge(page);
});

test("expande y colapsa sin perder la sección ni el ticket", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await loginThroughBridge(page);

  expect(await navWidth(page)).toBeCloseTo(216, 0);
  await expect(toggle(page)).toHaveAccessibleName("Contraer navegación");
  await expect(toggle(page)).toHaveAttribute("aria-expanded", "true");
  await expect(nav(page).locator(".nav-label").first()).toBeVisible();

  await addOneProduct(page);
  const badge = nav(page).locator(".nav-count");
  await expect(badge).toHaveText("1");

  // Sección activa distinta de Ventas para comprobar que el colapso no la mueve.
  await nav(page).getByRole("button", { name: "Cajas y turnos" }).click();
  await expect(
    nav(page).getByRole("button", { name: "Cajas y turnos" }),
  ).toHaveAttribute("aria-current", "page");

  await toggle(page).click();
  await expect(toggle(page)).toHaveAttribute("aria-expanded", "false");
  await expect(toggle(page)).toHaveAccessibleName("Expandir navegación");
  await expectNavWidth(page, 72);

  // Las etiquetas largas salen del layout en lugar de quedar cortadas.
  await expect(nav(page).locator(".nav-label").first()).toBeHidden();
  // El nombre accesible no depende de qué etiqueta esté visible.
  await expect(
    nav(page).getByRole("button", { name: /^Cajas y turnos/ }),
  ).toBeVisible();
  await expect(
    nav(page).getByRole("button", { name: /^Ventas, 1 artículo en el ticket$/ }),
  ).toBeVisible();
  await expect(
    nav(page).getByRole("button", { name: "Cajas y turnos" }),
  ).toHaveAttribute("aria-current", "page");

  // Iconos centrados y contador dentro del riel.
  const geometry = await page.evaluate(() => {
    const rail = document.querySelector(".sidenav") as HTMLElement;
    const item = document.querySelector('.nav-item[data-section="sales"]') as HTMLElement;
    const icon = item.querySelector("svg") as SVGElement;
    const badge = item.querySelector(".nav-count") as HTMLElement;
    const railBox = rail.getBoundingClientRect();
    const iconBox = icon.getBoundingClientRect();
    const badgeBox = badge.getBoundingClientRect();
    return {
      railCenter: railBox.left + railBox.width / 2,
      iconCenter: iconBox.left + iconBox.width / 2,
      badgeLeft: badgeBox.left,
      badgeRight: badgeBox.right,
      railLeft: railBox.left,
      railRight: railBox.right,
      labelVisible: Boolean(
        (document.querySelector(".nav-label") as HTMLElement).offsetParent,
      ),
    };
  });
  expect(Math.abs(geometry.railCenter - geometry.iconCenter)).toBeLessThanOrEqual(1);
  expect(geometry.badgeLeft).toBeGreaterThanOrEqual(geometry.railLeft);
  expect(geometry.badgeRight).toBeLessThanOrEqual(geometry.railRight);
  expect(geometry.labelVisible).toBe(false);

  // Tooltip accesible con el nombre completo al pasar el puntero.
  await nav(page).getByRole("button", { name: /^Ventas, 1 artículo/ }).hover();
  await expect(nav(page).locator(".nav-tooltip").first()).toBeVisible();
  await expect(nav(page).locator(".nav-tooltip").first()).toHaveText(
    "Ventas · 1 artículo en el ticket",
  );

  await toggle(page).click();
  await expectNavWidth(page, 216);
  await expect(nav(page).locator(".nav-label").first()).toBeVisible();
  await expect(badge).toHaveText("1");
  await expect(
    nav(page).getByRole("button", { name: "Cajas y turnos" }),
  ).toHaveAttribute("aria-current", "page");
  await expectNoHorizontalOverflow(page);
});

test("alterna varias veces de forma estable", async ({ page }) => {
  await page.setViewportSize({ width: 1366, height: 768 });
  await loginThroughBridge(page);

  for (let round = 0; round < 3; round += 1) {
    await toggle(page).click();
    await expect(toggle(page)).toHaveAttribute("aria-expanded", "false");
    await expectNavWidth(page, 72);
    await expectNoHorizontalOverflow(page);

    await toggle(page).click();
    await expect(toggle(page)).toHaveAttribute("aria-expanded", "true");
    await expectNavWidth(page, 216);
    await expectNoHorizontalOverflow(page);
  }

  await expect(nav(page).getByRole("button", { name: "Ventas" })).toHaveAttribute(
    "aria-current",
    "page",
  );
  await expect(page.getByRole("heading", { name: "Ventas" })).toBeVisible();
});

test("recuerda la preferencia de la instalación", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await loginThroughBridge(page);

  await toggle(page).click();
  await expectNavWidth(page, 72);

  // La sesión del puente simulado no sobrevive a la recarga: se vuelve a entrar.
  await loginThroughBridge(page);
  await expectNavWidth(page, 72);
  await expect(toggle(page)).toHaveAttribute("aria-expanded", "false");

  await toggle(page).click();
  await expectNavWidth(page, 216);
});

test("el riel compacto sustituye al colapso en ventanas estrechas", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await loginThroughBridge(page);
  await addOneProduct(page);

  await toggle(page).click();
  await expectNavWidth(page, 72);

  // Escritorio → compacto: la etiqueta corta es la representación válida y el
  // control de colapso desaparece porque no tendría efecto.
  await page.setViewportSize({ width: 1024, height: 768 });
  await expectNavWidth(page, 88);
  await expect(toggle(page)).toBeHidden();
  await expect(nav(page).locator(".nav-label-short").first()).toBeVisible();
  await expect(nav(page).locator(".nav-label").first()).toBeHidden();
  await expect(nav(page).locator(".nav-count")).toHaveText("1");
  await expectNoHorizontalOverflow(page);

  // El ticket y el cobro siguen accesibles con el riel compacto.
  await expect(page.getByRole("button", { name: /^Cobrar/ })).toBeInViewport();

  // Compacto → escritorio: vuelve la preferencia guardada, no un estado perdido.
  await page.setViewportSize({ width: 1440, height: 900 });
  await expectNavWidth(page, 72);
  await expect(toggle(page)).toBeVisible();
  await expect(nav(page).locator(".nav-count")).toHaveText("1");
  await expectNoHorizontalOverflow(page);
});

test("tablet: navegación con etiquetas cortas y sin recortes", async ({ page }) => {
  await page.setViewportSize({ width: 768, height: 1024 });
  await loginThroughBridge(page);
  await addOneProduct(page);

  expect(await navWidth(page)).toBeCloseTo(88, 0);
  await expect(nav(page).locator(".nav-label-short").first()).toBeVisible();

  const labels = await page.evaluate(() =>
    Array.from(document.querySelectorAll<HTMLElement>(".nav-label-short")).map((node) => ({
      text: node.textContent?.trim() ?? "",
      clipped: node.scrollWidth > node.clientWidth + 1,
    })),
  );
  expect(labels.length).toBeGreaterThan(4);
  for (const label of labels) {
    expect(label.text.length).toBeGreaterThan(0);
    expect(label.clipped, `la etiqueta ${label.text} no debe recortarse`).toBe(false);
  }

  await expect(page.locator(".sales-mobile-bar")).toBeVisible();
  await expectNoHorizontalOverflow(page);
});

test("la barra superior no recorta las acciones de cuenta", async ({ page }) => {
  for (const width of [1024, 800, 640, 412]) {
    await page.setViewportSize({ width, height: 900 });
    await loginThroughBridge(page);

    const bar = await page.evaluate(() => {
      const topbar = document.querySelector(".topbar") as HTMLElement;
      const logout = document.querySelector(
        ".topbar-meta > .icon-button:last-child",
      ) as HTMLElement;
      const logoutBox = logout.getBoundingClientRect();
      return {
        scrollWidth: topbar.scrollWidth,
        clientWidth: topbar.clientWidth,
        logoutRight: logoutBox.right,
        logoutWidth: logoutBox.width,
        viewport: document.documentElement.clientWidth,
      };
    });

    expect(bar.logoutWidth).toBeGreaterThan(0);
    expect(bar.logoutRight).toBeLessThanOrEqual(bar.viewport + 1);
    expect(bar.scrollWidth).toBeLessThanOrEqual(bar.clientWidth + 1);
    await expect(page.getByRole("button", { name: "Cerrar sesión" })).toBeInViewport();
    expect(await navWidth(page)).toBeCloseTo(width <= 1080 ? 88 : 216, 0);
  }
});

test("navegación por teclado y tooltip al enfocar", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await loginThroughBridge(page);
  await toggle(page).click();
  await expectNavWidth(page, 72);

  // Recorre con Tab desde el último control de la barra superior: es el paso
  // natural hacia la navegación.
  await page.getByRole("button", { name: "Cerrar sesión" }).focus();
  let reachedNav = false;
  for (let step = 0; step < 15 && !reachedNav; step += 1) {
    await page.keyboard.press("Tab");
    reachedNav = await page.evaluate(() =>
      Boolean(document.activeElement?.closest(".sidenav")),
    );
  }
  expect(reachedNav, "la navegación debe ser alcanzable con Tab").toBe(true);

  const focusedLabel = await page.evaluate(
    () => document.activeElement?.getAttribute("aria-label") ?? "",
  );
  expect(focusedLabel.length).toBeGreaterThan(0);
  // El tooltip también aparece con foco de teclado, no solo con el puntero.
  await expect(page.locator(".nav-item:focus-visible .nav-tooltip")).toBeVisible();

  // Enter activa el control enfocado sin romper el layout.
  await page.keyboard.press("Enter");
  await expectNoHorizontalOverflow(page);
});

test("el diálogo no desplaza la raíz y devuelve el foco", async ({ page }) => {
  await page.setViewportSize({ width: 1366, height: 768 });
  await loginThroughBridge(page);

  const opener = page.getByRole("button", { name: "Cambiar mi contraseña" });
  await opener.click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();
  await expect(
    page.evaluate(() => document.body.getAttribute("style")),
  ).resolves.toBeNull();

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();

  const state = await page.evaluate(() => ({
    scrollTop:
      document.scrollingElement instanceof HTMLElement
        ? document.scrollingElement.scrollTop
        : 0,
    focused: document.activeElement?.getAttribute("aria-label") ?? "",
    bodyStyle: document.body.getAttribute("style"),
  }));
  expect(state.scrollTop).toBe(0);
  expect(state.bodyStyle).toBeNull();
  expect(state.focused).toBe("Cambiar mi contraseña");
  await expectNoHorizontalOverflow(page);
});

test("diálogo mantiene footer accesible con viewport reducido", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await loginThroughBridge(page);

  await page.getByRole("button", { name: "Cambiar mi contraseña" }).click();
  const dialog = page.getByRole("dialog");
  await expect(dialog).toBeVisible();

  const footer = dialog.locator(".modal-footer");
  await expect(footer).toBeVisible();
  const box = await footer.boundingBox();
  expect(box, "el footer debe existir").not.toBeNull();
  expect(box!.y + box!.height, "el footer no debe quedar fuera del viewport").toBeLessThanOrEqual(844);

  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
});
