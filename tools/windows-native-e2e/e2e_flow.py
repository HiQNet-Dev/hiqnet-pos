"""E2E nativo de HiQNet POS contra el ejecutable real de Windows.

Recorre el flujo comercial completo usando Selenium WebDriver + `tauri-driver`
sobre el shell de escritorio:

    bootstrap -> productos -> variante -> modificadores -> inventario ->
    caja y turno -> venta en efectivo -> reinicio -> persistencia -> arqueo

También guarda capturas reales de cada pantalla para revisión visual.

Los grupos y opciones de modificadores se crean por IPC porque todavía no existe
pantalla administrativa para grupos; productos, variantes, inventario, caja,
venta y arqueo se operan desde la interfaz.
"""

import json
import os
import sys
import time
from pathlib import Path

from selenium import webdriver
from selenium.webdriver.common.by import By
from selenium.webdriver.common.options import BaseOptions
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.support.ui import Select, WebDriverWait

PRECIO_PRODUCTO = "10.00"
PRECIO_VARIANTE = "12.00"
# Precio al público con IVA 16 % calculado por Rust: 10.00 → 11.60 y
# 12.00 → 13.92 (la tarjeta publica la variante más barata).
PRECIO_PUBLICO_BASE = "11.60"
PRECIO_PUBLICO_VARIANTE = "13.92"
DELTA_EXTRA = "2.00"
CANTIDAD = 2
FONDO_INICIAL = "100.00"
EFECTIVO_RECIBIDO = "50.00"

UNITARIO_TOTAL = "16.24"
TOTAL_VENTA = "32.48"
CAMBIO = "17.52"
SALDO_ESPERADO = "132.48"
GASTO_EXTERNO = "9.00"
GASTO_CAJA = "12.00"
SALDO_CIERRE = "120.48"
STOCK_INICIAL = "20"
STOCK_FINAL = "18"
STOCK_COMPRAS = "23"
STOCK_MERMA = "22"

NAV = "//nav[@aria-label='Navegación principal']//button[@data-section='{}']"
DIALOG = "//div[@role='dialog']"


class WryOptions(BaseOptions):
    def __init__(self, application: str, args: list[str]):
        super().__init__()
        self._application = application
        self._args = args

    @property
    def default_capabilities(self):
        return {"browserName": "wry"}

    def to_capabilities(self):
        return {
            "browserName": "wry",
            "tauri:options": {"application": self._application, "args": self._args},
        }


class App:
    def __init__(self, app_path: str, timeout: int = 30):
        port = os.environ.get("HIQNET_E2E_PORT", "4444")
        database_path = os.environ.get("HIQNET_POS_DB_PATH")
        args = ["--hiqnet-e2e-db-path", database_path] if database_path else []
        self.driver = webdriver.Remote(
            command_executor=f"http://127.0.0.1:{port}",
            options=WryOptions(app_path, args),
        )
        self.driver.set_script_timeout(20)
        self.wait = WebDriverWait(self.driver, timeout)
        self._screens = 0

    def quit(self):
        try:
            self.driver.quit()
        except Exception:
            pass

    # --- ventana y capturas ------------------------------------------------------

    def resize(self, width: int, height: int):
        try:
            self.driver.set_window_size(width, height)
            time.sleep(0.4)
        except Exception:
            # Algunos compositores ignoran el redimensionado; la captura sigue siendo válida.
            pass

    def screenshot(self, name: str):
        directory = os.environ.get("HIQNET_E2E_ARTIFACT_DIR")
        self._screens += 1
        if not directory:
            return
        try:
            (Path(directory) / f"{self._screens:02d}-{name}.png").unlink(missing_ok=True)
            self.driver.save_screenshot(str(Path(directory) / f"{self._screens:02d}-{name}.png"))
        except Exception:
            pass

    # --- helpers de localización -------------------------------------------------

    def nav(self, section: str):
        self.click_xpath(NAV.format(section))

    def field(self, scope: str, label: str):
        xpath = f"{scope}//label[.//span[normalize-space()='{label}']]//input"
        element = self.wait.until(EC.presence_of_element_located((By.XPATH, xpath)))
        element.clear()
        return element

    def fill(self, scope: str, label: str, value: str):
        self.field(scope, label).send_keys(value)

    def select(self, scope: str, label: str, value: str):
        xpath = f"{scope}//label[.//span[normalize-space()='{label}']]//select"
        element = self.wait.until(EC.presence_of_element_located((By.XPATH, xpath)))
        Select(element).select_by_visible_text(value)

    def click_xpath(self, xpath: str):
        element = self.wait.until(EC.element_to_be_clickable((By.XPATH, xpath)))
        element.click()
        return element

    def wait_xpath(self, xpath: str, timeout: int = 30):
        return WebDriverWait(self.driver, timeout).until(
            EC.presence_of_element_located((By.XPATH, xpath))
        )

    def wait_text(self, text: str, timeout: int = 30):
        WebDriverWait(self.driver, timeout).until(
            EC.presence_of_element_located(
                (By.XPATH, f"//*[contains(normalize-space(.),'{text}')]")
            )
        )

    def wait_text_gone(self, text: str, timeout: int = 30):
        WebDriverWait(self.driver, timeout).until(
            EC.invisibility_of_element_located(
                (By.XPATH, f"//*[contains(normalize-space(.),'{text}')]")
            )
        )

    def text(self) -> str:
        return self.driver.find_element(By.TAG_NAME, "body").text

    def text_of(self, xpath: str) -> str:
        return self.driver.find_element(By.XPATH, xpath).text

    def invoke(self, command: str, args: dict | None = None):
        """Invocación IPC directa; solo para preparar datos sin pantalla propia."""
        result = self.invoke_raw(command, args)
        if not result.get("ok"):
            raise RuntimeError(f"IPC {command} falló: {result.get('e')}")
        return result["v"]

    def invoke_raw(self, command: str, args: dict | None = None) -> dict:
        """Invocación IPC que devuelve `{ok, v}` o `{ok: false, e}`."""
        payload = json.dumps(args or {})
        script = (
            "const done=arguments[arguments.length-1];"
            f"window.__TAURI_INTERNALS__.invoke('{command}',{payload})"
            ".then(v=>done({ok:true,v})).catch(e=>done({ok:false,e}));"
        )
        return self.driver.execute_async_script(script)


def assert_contains(text: str, expected: str, context: str):
    if expected not in text:
        raise AssertionError(f"{context}: se esperaba '{expected}' en:\n{text}")


def wait_screen(app: App, heading: str):
    WebDriverWait(app.driver, 30).until(
        EC.presence_of_element_located((By.XPATH, f"//*[normalize-space()='{heading}']"))
    )


def bootstrap(app: App):
    wait_screen(app, "Configura tu negocio")
    for label, value in [
        ("Nombre del negocio", "Cafetería E2E"),
        ("Nombre de la sucursal", "Matriz"),
        ("Código", "MX-01"),
        ("Nombre del propietario", "Ana E2E"),
        ("Usuario", "propietario"),
        ("Contraseña", "contrasena-segura"),
        ("Confirmar contraseña", "contrasena-segura"),
    ]:
        app.fill("//section", label, value)
    app.click_xpath("//button[normalize-space()='Crear instalación y entrar']")
    wait_screen(app, "Ventas")
    app.wait_xpath(NAV.format("sales"))


def crear_categoria(app: App):
    app.click_xpath("//button[normalize-space()='Nueva categoría']")
    app.wait_xpath(DIALOG)
    app.fill(DIALOG, "Nombre", "Bebidas")
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Crear categoría']")
    app.wait_text("Categoría Bebidas creada")
    # La opción aparece cuando el shell refresca el catálogo compartido.
    app.wait_xpath("//select//option[normalize-space()='Bebidas']", timeout=30)


def crear_producto(app: App):
    app.click_xpath("//button[contains(normalize-space(),'Nuevo producto')]")
    app.wait_xpath(DIALOG)
    app.fill(DIALOG, "SKU", "CAF-001")
    app.fill(DIALOG, "Nombre", "Café E2E")
    app.fill(DIALOG, "Precio base, sin IVA", PRECIO_PRODUCTO)
    app.fill(DIALOG, "IVA (%)", "16")
    app.select(DIALOG, "Categoría (opcional)", "Bebidas")
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Agregar producto']")
    app.wait_text("Producto CAF-001 creado")
    app.wait_xpath("//td[normalize-space()='CAF-001']")


def crear_variante(app: App):
    app.click_xpath("//button[normalize-space()='CAF-001']")
    detail = "//aside[@aria-label='Detalle del producto']"
    app.fill(detail, "Nueva variante", "Grande")
    app.fill(detail, "Precio neto", PRECIO_VARIANTE)
    app.click_xpath("//button[normalize-space()='Crear variante']")
    app.wait_text("Variante Grande creada")


def configurar_modificadores(app: App) -> dict:
    products = app.invoke("list_products")
    product = next(item for item in products if item["sku"] == "CAF-001")
    extras = app.invoke(
        "create_modifier_group",
        {
            "draft": {
                "product_id": product["id"],
                "name": "Extras",
                "min": 0,
                "max": 1,
                "sort_order": 0,
            }
        },
    )
    extra = app.invoke(
        "create_modifier_option",
        {
            "draft": {
                "group_id": extras["id"],
                "name": "Shot extra",
                "kind": "extra",
                "price_delta": DELTA_EXTRA,
                "sort_order": 0,
            }
        },
    )
    omisiones = app.invoke(
        "create_modifier_group",
        {
            "draft": {
                "product_id": product["id"],
                "name": "Sin ingredientes",
                "min": 0,
                "max": 1,
                "sort_order": 1,
            }
        },
    )
    omision = app.invoke(
        "create_modifier_option",
        {
            "draft": {
                "group_id": omisiones["id"],
                "name": "Sin azúcar",
                "kind": "omission",
                "price_delta": "0.00",
                "sort_order": 0,
            }
        },
    )
    return {"product_id": product["id"], "extra": extra["id"], "omision": omision["id"]}


def registrar_inventario(app: App):
    app.nav("inventory")
    wait_screen(app, "Inventario")
    app.select("//header[contains(@class,'screen-header')]", "Producto", "CAF-001 · Café E2E")
    app.wait_text("Café E2E")
    app.screenshot("inventario-vacio")
    app.select("//section[@aria-label='Existencias del producto']", "Tipo", "Entrada")
    app.fill("//section[@aria-label='Existencias del producto']", "Cantidad", STOCK_INICIAL)
    app.click_xpath("//button[normalize-space()='Registrar movimiento']")
    app.wait_text("Movimiento registrado")
    app.wait_text(STOCK_INICIAL)


def abrir_caja(app: App):
    app.nav("cash")
    wait_screen(app, "Caja y turnos")
    app.fill("//section", "Nombre", "Caja E2E")
    app.fill("//section", "Código", "CJ-01")
    app.click_xpath("//button[normalize-space()='Crear caja']")
    app.wait_text("Caja creada")
    app.wait_xpath("//button[normalize-space()='Abrir turno']")
    app.screenshot("caja-sin-turno")
    app.fill("//section", "Fondo inicial", FONDO_INICIAL)
    app.click_xpath("//button[normalize-space()='Abrir turno']")
    app.wait_text("Turno abierto")
    app.screenshot("caja-turno-abierto")


def vender(app: App) -> str:
    app.nav("sales")
    wait_screen(app, "Ventas")
    app.wait_text_gone("Abre un turno de caja para cobrar")
    app.screenshot("ventas-catalogo")

    # La tarjeta publica el precio final con IVA, no el neto ni «+ IVA».
    tarjeta = "//button[contains(@class,'product-card')][contains(.,'CAF-001')]"
    app.wait_xpath(tarjeta)
    precio_publico = app.text_of(f"{tarjeta}//*[contains(@class,'product-card-price')]")
    assert precio_publico == f"Desde ${PRECIO_PUBLICO_VARIANTE}", (
        f"la tarjeta no publica el precio final con IVA: {precio_publico}"
    )
    app.click_xpath("//button[contains(@class,'product-card')][contains(.,'Café E2E')]")
    dialog = app.wait_xpath(DIALOG)
    app.wait_xpath(f"{DIALOG}//label[contains(.,'Grande')]//input")
    app.click_xpath(f"{DIALOG}//label[.//span[normalize-space()='Grande']]//input")
    app.click_xpath(f"{DIALOG}//label[.//span[normalize-space()='Shot extra']]//input")
    app.click_xpath(f"{DIALOG}//label[.//span[normalize-space()='Sin azúcar']]//input")
    WebDriverWait(app.driver, 30).until(
        lambda driver: len(driver.find_elements(By.CSS_SELECTOR, ".config-price-value")) >= 2
        and driver.find_elements(By.CSS_SELECTOR, ".config-price-value")[0].text.startswith("$")
    )
    config_unit = app.driver.find_elements(By.CSS_SELECTOR, ".config-price-value")[0].text
    config_line = app.driver.find_elements(By.CSS_SELECTOR, ".config-price-value")[1].text
    assert config_unit == f"${UNITARIO_TOTAL}", (
        f"el configurador no muestra el precio final unitario: {config_unit}"
    )
    assert config_line == f"${UNITARIO_TOTAL}", (
        f"el total de línea del configurador no coincide: {config_line}"
    )
    app.screenshot("configurador")
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Agregar al carrito']")

    app.wait_text_gone("Sin artículos")
    ticket = app.text_of("//aside[contains(@class,'ticket')]")
    assert_contains(ticket, f"${UNITARIO_TOTAL}", "la línea no usa el precio de Rust")

    app.click_xpath("//button[@aria-label='Aumentar Café E2E']")
    app.wait_text(f"${TOTAL_VENTA}")
    app.screenshot("ticket")

    # Undo: la partida configurada se restaura completa y con su precio de Rust.
    app.click_xpath("//button[@aria-label='Quitar Café E2E']")
    app.wait_text("Línea eliminada: Café E2E")
    app.screenshot("ticket-undo")
    app.click_xpath("//div[contains(@class,'toast')]//button[normalize-space()='Deshacer']")
    app.wait_text(f"2 × ${UNITARIO_TOTAL}")
    restored = app.text_of("//aside[contains(@class,'ticket')]")
    assert_contains(restored, "Grande", "el undo no conservó la variante")
    assert_contains(restored, "Shot extra", "el undo no conservó los modificadores")
    assert_contains(restored, "Sin azúcar", "el undo no conservó la omisión")
    assert_contains(restored, f"${TOTAL_VENTA}", "el undo no conservó el total")

    app.click_xpath("//button[starts-with(normalize-space(),'Cobrar $')]")
    app.wait_xpath(f"{DIALOG}//label[.//span[normalize-space()='Efectivo recibido']]//input")
    # El texto del portal puede tardar un tick en exponerse al WebDriver nativo;
    # se espera el copy visible y se lee del body, que sí incluye el diálogo.
    app.wait_text("Total estimado")
    checkout = app.text()
    assert_contains(checkout, f"Total estimado\n${TOTAL_VENTA}", "el checkout no coincide con el ticket")
    app.fill(DIALOG, "Efectivo recibido", EFECTIVO_RECIBIDO)
    app.wait_text(f"Cambio estimado: ${CAMBIO}")
    app.screenshot("checkout")
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Confirmar cobro']")

    app.wait_text("Venta V-")
    app.screenshot("venta-confirmada")
    confirmacion = app.text()
    assert_contains(confirmacion, f"Total cobrado\n${TOTAL_VENTA}", "total confirmado incorrecto")
    assert_contains(confirmacion, f"Cambio\n${CAMBIO}", "cambio incorrecto")

    for line in confirmacion.splitlines():
        if line.startswith("Venta V-"):
            folio = line.split()[1]
            break
    else:
        raise AssertionError("No se encontró el folio en la confirmación")

    # Post-venta: búsqueda enfocada y limpia, filtro por defecto, sin banner.
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Nueva venta']")
    WebDriverWait(app.driver, 30).until(
        lambda driver: driver.execute_script(
            "return document.activeElement?.getAttribute('aria-label') === arguments[0]",
            "Buscar producto. Enter agrega el único resultado",
        )
    )
    search = app.driver.find_element(
        By.XPATH, "//input[@aria-label='Buscar producto. Enter agrega el único resultado']"
    )
    assert search.get_attribute("value") == "", "la búsqueda no se limpió tras la venta"
    active_chip = app.driver.find_element(By.CSS_SELECTOR, ".chips .chip-active")
    assert active_chip.text.strip() == "Todas", "el filtro de categoría no volvió al default"
    assert app.driver.execute_script("return document.querySelectorAll('.banner-row').length") == 0, (
        "la venta confirmada volvió a usar el banner que desplaza el layout"
    )
    assert app.driver.execute_script(
        "return getComputedStyle(document.querySelector('.toast-region')).position"
    ) == "fixed", "el aviso de venta no es flotante"
    app.screenshot("venta-nueva-venta")

    return folio


def iniciar_sesion(app: App):
    wait_screen(app, "Entrar a la caja")
    app.screenshot("login")
    app.fill("//section", "Usuario", "propietario")
    app.fill("//section", "Contraseña", "contrasena-segura")
    app.click_xpath("//button[normalize-space()='Entrar']")
    wait_screen(app, "Ventas")


def verificar_persistencia(app: App) -> dict:
    app.nav("inventory")
    wait_screen(app, "Inventario")
    app.select(
        "//header[contains(@class,'screen-header')]", "Producto", "CAF-001 · Café E2E"
    )
    app.wait_xpath(
        f"//span[contains(@class,'stock-figure') and normalize-space()='{STOCK_FINAL}']",
        timeout=30,
    )
    inventario = app.text_of("//section[@aria-label='Existencias del producto']")
    assert_contains(inventario, STOCK_FINAL, "el inventario no persistió")
    app.screenshot("inventario-persistido")

    app.nav("cash")
    wait_screen(app, "Caja y turnos")
    app.wait_text("Turno abierto")
    caja = app.text_of("//section[@aria-label='Operación del turno']")
    assert_contains(caja, SALDO_ESPERADO, "el saldo esperado no incluye la venta")
    app.screenshot("caja-abierta")

    if not app.invoke("list_products"):
        raise AssertionError("el catálogo quedó vacío tras el reinicio")
    return {"inventario": inventario, "caja": caja}


def cerrar_turno(app: App):
    app.fill("//section[@aria-label='Operación del turno']", "Efectivo contado", SALDO_CIERRE)
    app.click_xpath("//button[normalize-space()='Revisar cierre']")
    app.wait_xpath(DIALOG)
    app.wait_text("Esta acción no se puede deshacer")
    review = app.text()
    assert_contains(review, SALDO_CIERRE, "la revisión no muestra el saldo esperado")
    assert_contains(review, "Esta acción no se puede deshacer", "la revisión no advierte la consecuencia")
    app.screenshot("revision-cierre")

    # Cancelar no debe mutar el turno.
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Cancelar']")
    WebDriverWait(app.driver, 30).until(EC.invisibility_of_element_located((By.XPATH, DIALOG)))
    app.wait_xpath("//button[normalize-space()='Revisar cierre']")
    assert not app.driver.find_elements(
        By.XPATH, "//h2[normalize-space()='Resultado del arqueo']"
    ), "cancelar la revisión cerró el turno"

    # Reabrir la misma revisión y confirmar el cierre.
    app.click_xpath("//button[normalize-space()='Revisar cierre']")
    app.wait_xpath(DIALOG)
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Cerrar turno y registrar arqueo']")
    app.wait_text("Turno cerrado")

    app.wait_text("Resultado del arqueo")
    arqueo = app.text_of("//section[@aria-label='Operación del turno']")
    assert_contains(arqueo, "Exacto", "la diferencia del arqueo exacto no es cero")
    assert_contains(arqueo, SALDO_CIERRE, "el cierre no conserva el saldo esperado")
    historial = app.text_of("//section[@aria-label='Historial de turnos']")
    assert_contains(historial, "Caja E2E · CJ-01", "el historial no identifica la caja")
    assert_contains(historial, SALDO_CIERRE, "el historial no conserva el esperado")
    app.screenshot("arqueo-cierre")


def gestionar_compras(app: App) -> dict:
    """Proveedor y recepción parcial/final mediante la interfaz real."""
    app.nav("purchasing")
    wait_screen(app, "Compras")
    app.screenshot("compras-vacio")
    app.click_xpath("//button[normalize-space()='Nuevo proveedor']")
    app.wait_xpath(DIALOG)
    app.fill(DIALOG, "Nombre", "Proveedor E2E")
    app.fill(DIALOG, "Código", "SUP-E2E")
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Crear proveedor']")
    app.wait_text("Proveedor creado")
    app.click_xpath("//button[normalize-space()='Nueva orden']")
    app.wait_xpath(DIALOG)
    app.select(DIALOG, "Proveedor", "Proveedor E2E")
    app.select(DIALOG, "Producto", "CAF-001 · Café E2E")
    app.fill(DIALOG, "Cantidad", "5")
    app.fill(DIALOG, "Costo neto", "8.00")
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Agregar']")
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Crear borrador']")
    app.wait_text("Orden creada como borrador")
    app.click_xpath("//button[normalize-space()='Confirmar']")
    app.wait_text("confirmada")
    app.click_xpath("//button[normalize-space()='Recibir']")
    app.wait_xpath(DIALOG)
    app.fill(DIALOG, "Recibir", "2")
    app.screenshot("compras-recepcion-parcial")
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Confirmar recepción']")
    app.wait_text("Recepción registrada")
    order = app.invoke("list_purchase_orders")[0]
    assert order["status"] == "partially_received", "la orden debe quedar parcialmente recibida"
    assert app.invoke("current_stock", {"product_id": order["lines"][0]["product_id"]}) == "20", "la recepción parcial debe sumar dos"
    app.click_xpath("//button[normalize-space()='Recibir']")
    app.wait_xpath(DIALOG)
    app.fill(DIALOG, "Recibir", "3")
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Confirmar recepción']")
    app.wait_text("Recepción registrada")
    order = app.invoke("get_purchase_order", {"order_id": order["id"]})
    assert order["status"] == "received", "la orden debe quedar recibida"
    assert app.invoke("current_stock", {"product_id": order["lines"][0]["product_id"]}) == "23", "el stock final debe sumar cinco"
    app.screenshot("compras-recepcion-final")
    return {"folio": order["folio"], "order_id": order["id"], "stock_final": STOCK_COMPRAS}


def gestionar_gastos_y_merma(app: App) -> dict:
    """Gasto externo, gasto de caja y merma operados desde la interfaz real."""
    app.nav("expenses")
    wait_screen(app, "Gastos")
    app.screenshot("gastos-vacio")
    app.fill("//section[.//h2[normalize-space()='Categorías']]", "Nueva categoría", "Operación E2E")
    app.click_xpath("//section[.//h2[normalize-space()='Categorías']]//button[normalize-space()='Crear']")
    app.wait_text("Categoría creada")
    expense_scope = "//section[.//h2[normalize-space()='Registrar gasto']]"
    app.select(expense_scope, "Categoría", "Operación E2E")
    app.fill(expense_scope, "Concepto", "Servicio externo E2E")
    app.fill(expense_scope, "Importe", GASTO_EXTERNO)
    app.click_xpath(f"{expense_scope}//button[normalize-space()='Registrar gasto']")
    app.wait_text("Gasto externo registrado")
    app.fill(expense_scope, "Concepto", "Limpieza E2E")
    app.fill(expense_scope, "Importe", GASTO_CAJA)
    app.click_xpath(f"{expense_scope}//label[contains(@class,'checkbox-field')]//input")
    app.screenshot("gasto-caja")
    app.click_xpath(f"{expense_scope}//button[normalize-space()='Registrar gasto']")
    app.wait_text("Gasto y salida de caja registrados")

    app.nav("inventory")
    wait_screen(app, "Inventario")
    app.select("//header[contains(@class,'screen-header')]", "Producto", "CAF-001 · Café E2E")
    waste_scope = "//form[.//h3[normalize-space()='Registrar merma']]"
    app.fill(waste_scope, "Cantidad", "1")
    app.select(waste_scope, "Motivo", "Daño")
    app.screenshot("merma")
    app.click_xpath(f"{waste_scope}//button[normalize-space()='Revisar merma']")
    app.wait_xpath(DIALOG)
    app.wait_text("Se descontará")
    preview = app.text()
    assert_contains(preview, f"{STOCK_COMPRAS} → {STOCK_MERMA}", "la revisión de merma no muestra el stock resultante")
    app.screenshot("revision-merma")

    # Cancelar no debe descontar existencias.
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Cancelar']")
    WebDriverWait(app.driver, 30).until(EC.invisibility_of_element_located((By.XPATH, DIALOG)))
    app.wait_xpath(
        f"//span[contains(@class,'stock-figure') and normalize-space()='{STOCK_COMPRAS}']"
    )

    # Reabrir la revisión y confirmar una sola vez.
    app.click_xpath(f"{waste_scope}//button[normalize-space()='Revisar merma']")
    app.wait_xpath(DIALOG)
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Registrar merma']")
    app.wait_text("Merma registrada")
    app.wait_xpath(f"//span[contains(@class,'stock-figure') and normalize-space()='{STOCK_MERMA}']")

    app.nav("cash")
    wait_screen(app, "Caja y turnos")
    app.wait_text(SALDO_CIERRE)
    caja = app.text_of("//section[@aria-label='Operación del turno']")
    assert_contains(caja, SALDO_CIERRE, "el gasto de caja no actualizó el saldo esperado")
    assert_contains(caja, "gasto: Limpieza E2E", "el movimiento de caja no está vinculado al gasto")
    return {"external_amount": GASTO_EXTERNO, "cash_amount": GASTO_CAJA, "stock_after_waste": STOCK_MERMA, "expected_cash": SALDO_CIERRE}


def gestionar_usuarios(app: App) -> dict:
    """Alta de un cajero desde Gestión y verificación de su acceso restringido."""
    app.nav("admin")
    wait_screen(app, "Gestión y auditoría")
    app.click_xpath("//button[normalize-space()='Nuevo usuario']")
    app.wait_xpath(DIALOG)
    app.fill(DIALOG, "Nombre del empleado", "Luis Cajero")
    app.fill(DIALOG, "Usuario", "luis.cajero")
    app.select(DIALOG, "Rol", "Cajero")
    app.fill(DIALOG, "Contraseña inicial", "clave-cajero-2026")
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Crear cuenta']")
    app.wait_text("Cuenta de Luis Cajero creada")
    app.wait_xpath("//td[normalize-space()='luis.cajero']")
    app.wait_text("Usuario creado")
    app.screenshot("usuarios")

    # El cajero inicia sesión y sólo ve las secciones de su rol.
    app.click_xpath("//button[@aria-label='Cerrar sesión']")
    wait_screen(app, "Entrar a la caja")
    app.fill("//section", "Usuario", "luis.cajero")
    app.fill("//section", "Contraseña", "clave-cajero-2026")
    app.click_xpath("//button[normalize-space()='Entrar']")
    wait_screen(app, "Ventas")
    app.wait_xpath(NAV.format("sales"))
    app.screenshot("shell-cajero")

    sin_catalogo = not app.driver.find_elements(By.XPATH, NAV.format("catalog"))
    sin_gestion = not app.driver.find_elements(By.XPATH, NAV.format("admin"))
    sin_compras = not app.driver.find_elements(By.XPATH, NAV.format("purchasing"))
    assert sin_catalogo, "el cajero no debe ver la sección Productos"
    assert sin_gestion, "el cajero no debe ver la sección Gestión"
    assert sin_compras, "el cajero no debe ver la sección Compras"

    # Y tampoco puede administrar usuarios invocando el IPC directamente.
    intento = app.invoke_raw("list_users")
    assert not intento.get("ok"), "el cajero no debe poder listar usuarios por IPC"
    compra = app.invoke_raw("list_purchase_orders")
    assert not compra.get("ok"), "el cajero no debe administrar compras por IPC"

    app.click_xpath("//button[@aria-label='Cerrar sesión']")
    wait_screen(app, "Entrar a la caja")
    app.fill("//section", "Usuario", "propietario")
    app.fill("//section", "Contraseña", "contrasena-segura")
    app.click_xpath("//button[normalize-space()='Entrar']")
    wait_screen(app, "Ventas")

    return {
        "cajero_sin_catalogo": sin_catalogo,
        "cajero_sin_gestion": sin_gestion,
        "ipc_list_users_denegado": True,
        "ipc_compras_denegado": True,
    }


def capturar_tamanos(app_path: str) -> dict:
    """Capturas reales en las resoluciones de referencia y en formato tablet."""
    app = App(app_path)
    result: dict = {"capturas": [], "tablet_ticket": False}
    try:
        wait_screen(app, "Entrar a la caja")
        app.fill("//section", "Usuario", "propietario")
        app.fill("//section", "Contraseña", "contrasena-segura")
        app.click_xpath("//button[normalize-space()='Entrar']")
        wait_screen(app, "Ventas")

        for width, height, name in [
            (1920, 1080, "ventas-1920"),
            (1440, 900, "ventas-1440"),
            (1366, 768, "ventas-1366"),
            (1024, 768, "ventas-1024"),
        ]:
            app.resize(width, height)
            app.screenshot(name)
            result["capturas"].append(name)

        # Productos a 1024: el precio público debe seguir visible y no exigir
        # scroll horizontal; SKU y categoría son secundarios en este ancho.
        app.resize(1024, 768)
        app.nav("catalog")
        wait_screen(app, "Productos y catálogo")
        app.wait_xpath("//td[contains(@class,'product-public-price')]")
        columns = app.driver.execute_script(
            """
            const sku=document.querySelector('.data-table .col-sku');
            const category=document.querySelector('.data-table .col-category');
            const scroll=document.querySelector('.table-scroll');
            const cell=document.querySelector('td.product-public-price');
            if(!scroll||!cell) return null;
            const c=cell.getBoundingClientRect(), s=scroll.getBoundingClientRect();
            return {
              sku: sku ? getComputedStyle(sku).display : null,
              category: category ? getComputedStyle(category).display : null,
              scrollWidth: scroll.scrollWidth,
              clientWidth: scroll.clientWidth,
              price: cell.innerText,
              inside: c.right <= s.right + 1 && c.left >= s.left - 1,
              viewport: {width: innerWidth, height: innerHeight}
            };
            """
        )
        assert columns is not None, "no se pudo medir la tabla de productos a 1024"
        assert columns["sku"] == "none", "SKU sigue visible a 1024"
        assert columns["category"] == "none", "Categoría sigue visible a 1024"
        assert columns["scrollWidth"] <= columns["clientWidth"] + 1, (
            f"la tabla exige scroll horizontal a 1024: {columns}"
        )
        assert columns["price"] == f"${PRECIO_PUBLICO_BASE}", (
            f"el precio público no está visible a 1024: {columns['price']}"
        )
        assert columns["inside"], "el precio público queda fuera del área visible a 1024"
        result["productos_1024"] = columns
        app.screenshot("productos-1024")
        result["capturas"].append("productos-1024")

        app.resize(1366, 768)
        for section, name in [
            ("catalog", "productos-1366"),
            ("inventory", "inventario-1366"),
            ("cash", "caja-1366"),
            ("admin", "gestion-1366"),
        ]:
            app.nav(section)
            time.sleep(0.5)
            app.screenshot(name)
            result["capturas"].append(name)

        for width, height, name in [
            (1366, 768, "compras-1366"),
            (1024, 768, "compras-1024"),
            (768, 1024, "compras-tablet"),
        ]:
            app.resize(width, height)
            app.nav("purchasing")
            time.sleep(0.5)
            app.screenshot(name)
            result["capturas"].append(name)

        app.nav("sales")
        app.resize(768, 1024)
        time.sleep(0.5)
        app.screenshot("ventas-tablet-catalogo")
        result["capturas"].append("ventas-tablet-catalogo")

        buttons = app.driver.find_elements(
            By.XPATH, "//button[starts-with(normalize-space(),'Ver ticket')]"
        )
        if buttons and buttons[0].is_displayed():
            result["tablet_ticket"] = True
            buttons[0].click()
            time.sleep(0.4)
            app.screenshot("ventas-tablet-ticket")
            result["capturas"].append("ventas-tablet-ticket")

        # Gestión en tablet: la tabla de usuarios debe caber sin desbordar.
        app.nav("admin")
        time.sleep(0.5)
        app.screenshot("gestion-tablet")
        result["capturas"].append("gestion-tablet")
        return result
    finally:
        app.quit()


def main() -> int:
    app_path = str(
        Path(
            sys.argv[1]
            if len(sys.argv) > 1
            else "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe"
        ).resolve()
    )
    steps: list[str] = []
    report: dict = {"application": app_path}
    app = App(app_path)
    try:
        bootstrap(app)
        steps.append("bootstrap")
        app.screenshot("shell-ventas-vacio")

        app.nav("catalog")
        wait_screen(app, "Productos y catálogo")
        app.screenshot("productos-vacio")

        crear_categoria(app)
        steps.append("categoria")
        crear_producto(app)
        steps.append("producto")
        crear_variante(app)
        steps.append("variante")

        report["modificadores"] = configurar_modificadores(app)
        steps.append("modificadores")

        registrar_inventario(app)
        steps.append("inventario")

        abrir_caja(app)
        steps.append("caja_abierta")

        report["folio"] = vender(app)
        steps.append("venta")

        app.quit()
        app = App(app_path)
        iniciar_sesion(app)
        steps.append("reinicio_login")

        report["persistencia"] = verificar_persistencia(app)
        steps.append("persistencia")

        report["compras"] = gestionar_compras(app)
        steps.append("compras")

        report["gastos_merma"] = gestionar_gastos_y_merma(app)
        steps.append("gastos_merma")

        app.quit()
        app = App(app_path)
        iniciar_sesion(app)
        persisted_order = app.invoke("get_purchase_order", {"order_id": report["compras"]["order_id"]})
        assert persisted_order["status"] == "received", "la orden no persistió tras reiniciar"
        assert len(app.invoke("list_expenses")) == 2, "los gastos no persistieron tras reiniciar"
        assert len(app.invoke("list_waste")) == 1, "la merma no persistió tras reiniciar"
        product_id = report["modificadores"]["product_id"]
        assert app.invoke("current_stock", {"product_id": product_id}) == STOCK_MERMA, "la merma no persistió tras reiniciar"
        steps.append("compras_gastos_reinicio")

        app.nav("cash")
        wait_screen(app, "Caja y turnos")
        cerrar_turno(app)
        steps.append("arqueo_cierre")

        report["usuarios"] = gestionar_usuarios(app)
        steps.append("usuarios")

        app.quit()
        report["tamanos"] = capturar_tamanos(app_path)
        steps.append("resoluciones")

        report.update({"steps": steps, "result": "PASS"})
        print(json.dumps(report, ensure_ascii=False))
        return 0
    except Exception as error:  # noqa: BLE001
        app.screenshot("error")
        report.update({"steps": steps, "error": str(error), "text": app.text()[:3000]})
        print(json.dumps(report, ensure_ascii=False))
        return 1
    finally:
        app.quit()
        directory = os.environ.get("HIQNET_E2E_ARTIFACT_DIR")
        if directory:
            try:
                (Path(directory) / "flow-report.json").write_text(
                    json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
                )
            except Exception:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
