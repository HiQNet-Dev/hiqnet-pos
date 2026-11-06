import { expect, test } from "@playwright/test";
import { loginThroughBridge, mockShellBridge } from "./mock-shell";

/**
 * Medición de navegador del catálogo. El puente IPC se simula deliberadamente:
 * el coste SQLite/Rust se mide en `catalog_scale_benchmark.rs`; aquí se mide el
 * parseo, filtrado y cantidad de nodos de React sin mezclar procesos.
 */
function catalog(count: number) {
  return Array.from({ length: count }, (_, index) => ({
    id: `scale-${index}`,
    sku: `SKU-${index.toString().padStart(5, "0")}`,
    name: `Producto de escala ${index.toString().padStart(5, "0")}`,
    unit_price: "10.00",
    public_unit_total: "11.60",
    tax_rate_bp: 1600,
    active: true,
    category_id: null,
    active_variants: index % 10 === 0 ? 1 : 0,
    variant_public_from: index % 10 === 0 ? "10.44" : null,
    has_modifiers: index % 20 === 0,
  }));
}

for (const count of [1_000, 10_000]) {
  test(`catálogo de ${count.toLocaleString("en-US")} mantiene el DOM acotado y filtra`, async ({
    page,
  }) => {
    test.setTimeout(120_000);
    await page.setViewportSize({ width: 1440, height: 900 });
    await mockShellBridge(page, catalog(count));

    const loadingStarted = performance.now();
    await loginThroughBridge(page);
    await expect(page.locator(".product-card")).toHaveCount(200);
    await expect(page.getByRole("button", { name: /producto de escala 00000, sku-00000/i })).toBeVisible();
    await expect(page.getByRole("button", { name: /producto de escala 00198, sku-00198/i })).toBeVisible();
    await expect(page.getByRole("button", { name: /producto de escala 00200, sku-00200/i })).toHaveCount(0);
    await page.getByRole("button", { name: /mostrar 200 más/i }).click();
    await expect(page.locator(".product-card")).toHaveCount(400);
    await expect(page.getByRole("button", { name: /producto de escala 00200, sku-00200/i })).toBeVisible();
    const firstRenderMs = performance.now() - loadingStarted;

    const search = page.getByRole("textbox", { name: /buscar producto/i });
    if (count >= 5_000) {
      await search.fill("Producto de escala 04999");
      await expect(page.getByRole("button", { name: /producto de escala 04999, sku-04999/i })).toBeVisible();
    }
    const filterStarted = performance.now();
    await search.fill(`SKU-${(count - 1).toString().padStart(5, "0")}`);
    await expect(page.locator(".product-card")).toHaveCount(1);
    const filterMs = performance.now() - filterStarted;

    const metrics = await page.evaluate(() => ({
      cards: document.querySelectorAll(".product-card").length,
      nodes: document.querySelectorAll("*").length,
    }));
    console.log(
      JSON.stringify({ count, firstRenderMs, filterMs, ...metrics }),
    );

    expect(metrics.cards).toBe(1);
    // No se usa como SLA de hardware: solo evita volver a renderizar el catálogo
    // completo cuando existe una coincidencia.
    expect(metrics.nodes).toBeLessThan(1_000);
  });
}
