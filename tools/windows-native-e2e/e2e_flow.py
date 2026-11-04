"""E2E nativo de HiQNet POS contra el ejecutable real de Windows.

Recorre el flujo comercial completo usando Selenium WebDriver + `tauri-driver`:

    bootstrap -> categoría -> producto -> variante -> modificadores ->
    inventario -> caja y turno -> venta en efectivo -> reinicio ->
    verificación de persistencia -> arqueo y cierre

Todo el estado se guarda en la base SQLite aislada indicada por
`HIQNET_POS_DB_PATH`; nunca se toca la instalación del usuario.

Los grupos y opciones de modificadores se crean por IPC porque todavía no existe
una pantalla administrativa para ellos (ver `docs/03-contracts/ipc.md`). La venta
completa, el carrito, el cobro y el arqueo se operan desde la interfaz.

Uso:
    python e2e_flow.py <ruta-del-ejecutable>

Variables opcionales:
    HIQNET_E2E_ARTIFACT_DIR  Directorio donde escribir el informe y las capturas.
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

# Escenario monetario determinista del fixture (importes ilustrativos).
PRECIO_PRODUCTO = "10.00"
PRECIO_VARIANTE = "12.00"
DELTA_EXTRA = "2.00"
CANTIDAD = 2
FONDO_INICIAL = "100.00"
EFECTIVO_RECIBIDO = "50.00"

UNITARIO_NETO = "14.00"
IMPUESTO_UNITARIO = "2.24"
UNITARIO_TOTAL = "16.24"
TOTAL_VENTA = "32.48"
CAMBIO = "17.52"
SALDO_ESPERADO = "132.48"
STOCK_INICIAL = "20"
STOCK_FINAL = "18"


class WryOptions(BaseOptions):
    def __init__(self, application: str):
        super().__init__()
        self._application = application

    @property
    def default_capabilities(self):
        return {"browserName": "wry"}

    def to_capabilities(self):
        return {"browserName": "wry", "tauri:options": {"application": self._application}}


class App:
    def __init__(self, app_path: str, timeout: int = 30):
        self.driver = webdriver.Remote(
            command_executor="http://127.0.0.1:4444",
            options=WryOptions(app_path),
        )
        self.driver.set_script_timeout(20)
        self.wait = WebDriverWait(self.driver, timeout)

    def quit(self):
        try:
            self.driver.quit()
        except Exception:
            pass

    # --- helpers de localización -------------------------------------------------

    def section(self, heading: str, tag: str = "h2") -> str:
        return f"//section[.//{tag}[normalize-space()='{heading}']]"

    def _input(self, heading: str, label: str, tag: str):
        xpath = (
            f"{self.section(heading, tag)}"
            f"//label[.//span[normalize-space()='{label}']]//input"
        )
        return self.wait.until(EC.presence_of_element_located((By.XPATH, xpath)))

    def fill(self, heading: str, label: str, value: str, tag: str = "h2"):
        element = self._input(heading, label, tag)
        element.clear()
        element.send_keys(value)

    def select(self, heading: str, label: str, value: str, tag: str = "h2"):
        xpath = (
            f"{self.section(heading, tag)}"
            f"//label[.//span[normalize-space()='{label}']]//select"
        )
        element = self.wait.until(EC.presence_of_element_located((By.XPATH, xpath)))
        Select(element).select_by_visible_text(value)

    def click(self, heading: str, button: str, tag: str = "h2"):
        xpath = f"{self.section(heading, tag)}//button[normalize-space()='{button}']"
        element = self.wait.until(EC.element_to_be_clickable((By.XPATH, xpath)))
        element.click()

    def click_xpath(self, xpath: str):
        element = self.wait.until(EC.element_to_be_clickable((By.XPATH, xpath)))
        element.click()
        return element

    def toggle_option(self, heading: str, option_label: str):
        xpath = (
            f"{self.section(heading)}"
            f"//label[.//span[normalize-space()='{option_label}']]//input"
        )
        element = self.wait.until(EC.element_to_be_clickable((By.XPATH, xpath)))
        element.click()

    def wait_text(self, text: str, timeout: int = 20):
        WebDriverWait(self.driver, timeout).until(
            EC.presence_of_element_located(
                (By.XPATH, f"//*[contains(normalize-space(.),'{text}')]")
            )
        )

    def wait_text_gone(self, text: str, timeout: int = 20):
        WebDriverWait(self.driver, timeout).until(
            EC.invisibility_of_element_located(
                (By.XPATH, f"//*[contains(normalize-space(.),'{text}')]")
            )
        )

    def text(self) -> str:
        return self.driver.find_element(By.TAG_NAME, "body").text

    def section_text(self, heading: str, tag: str = "h2") -> str:
        element = self.driver.find_element(By.XPATH, self.section(heading, tag))
        return element.text

    def invoke(self, command: str, args: dict | None = None) -> dict:
        """Invocación IPC directa; solo para preparar datos sin pantalla propia."""
        payload = json.dumps(args or {})
        script = (
            "const done=arguments[arguments.length-1];"
            f"window.__TAURI_INTERNALS__.invoke('{command}',{payload})"
            ".then(v=>done({ok:true,v})).catch(e=>done({ok:false,e}));"
        )
        result = self.driver.execute_async_script(script)
        if not result.get("ok"):
            raise RuntimeError(f"IPC {command} falló: {result.get('e')}")
        return result["v"]

    def screenshot(self, name: str):
        directory = os.environ.get("HIQNET_E2E_ARTIFACT_DIR")
        if not directory:
            return
        try:
            target = Path(directory) / f"{name}.png"
            self.driver.save_screenshot(str(target))
        except Exception:
            pass


def wait_heading(app: App, heading: str, timeout: int = 30):
    WebDriverWait(app.driver, timeout).until(
        EC.presence_of_element_located(
            (By.XPATH, f"//*[normalize-space()='{heading}']")
        )
    )


def assert_contains(text: str, expected: str, context: str):
    if expected not in text:
        raise AssertionError(f"{context}: se esperaba '{expected}' en:\n{text}")


def bootstrap(app: App):
    wait_heading(app, "Configura tu negocio")
    app.fill("Configura tu negocio", "Nombre del negocio", "Cafetería E2E", tag="h1")
    app.fill("Configura tu negocio", "Nombre de la sucursal", "Matriz", tag="h1")
    app.fill("Configura tu negocio", "Código", "MX-01", tag="h1")
    app.fill("Configura tu negocio", "Nombre del propietario", "Ana E2E", tag="h1")
    app.fill("Configura tu negocio", "Usuario", "propietario", tag="h1")
    app.fill("Configura tu negocio", "Contraseña", "contrasena-segura", tag="h1")
    app.fill("Configura tu negocio", "Confirmar contraseña", "contrasena-segura", tag="h1")
    app.click("Configura tu negocio", "Crear instalación y entrar", tag="h1")
    wait_heading(app, "Productos")


def crear_catalogo(app: App):
    app.fill("Categorías, productos y variantes", "Nueva categoría", "Bebidas")
    app.click("Categorías, productos y variantes", "Crear categoría")
    app.wait_text("Categoría Bebidas creada")

    app.fill("Productos", "SKU", "CAF-001", tag="h1")
    app.fill("Productos", "Nombre", "Café E2E", tag="h1")
    app.fill("Productos", "Precio (MXN)", PRECIO_PRODUCTO, tag="h1")
    app.fill("Productos", "IVA (%)", "16", tag="h1")
    app.select("Productos", "Categoría (opcional)", "Bebidas", tag="h1")
    app.click("Productos", "Agregar producto", tag="h1")
    app.wait_text("Producto CAF-001 creado")

    app.click_xpath(
        "//section[.//h2[normalize-space()='Categorías, productos y variantes']]"
        "//button[contains(.,'Café E2E')]"
    )
    app.fill("Categorías, productos y variantes", "Nueva variante", "Grande")
    app.fill("Categorías, productos y variantes", "Precio", PRECIO_VARIANTE)
    app.click("Categorías, productos y variantes", "Crear variante")
    app.wait_text("Variante Grande creada")


def configurar_modificadores(app: App) -> dict:
    """Crea grupos y opciones con los casos de uso auditados (sin pantalla admin)."""
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
    return {
        "product_id": product["id"],
        "extras": extras["id"],
        "extra": extra["id"],
        "omisiones": omisiones["id"],
        "omision": omision["id"],
    }


def registrar_inventario(app: App):
    app.select("Existencias y movimientos", "Producto", "CAF-001 · Café E2E")
    app.select("Existencias y movimientos", "Tipo", "Entrada")
    app.fill("Existencias y movimientos", "Cantidad", STOCK_INICIAL)
    app.click("Existencias y movimientos", "Registrar")
    app.wait_text("Movimiento registrado")


def abrir_caja(app: App):
    app.fill("Turno de efectivo", "Nombre", "Caja E2E")
    app.fill("Turno de efectivo", "Código", "CJ-01")
    app.click("Turno de efectivo", "Crear caja")
    app.wait_text("Caja creada")
    app.fill("Turno de efectivo", "Fondo inicial", FONDO_INICIAL)
    app.click("Turno de efectivo", "Abrir turno")
    app.wait_text("Turno abierto")


def esperar_turno_en_venta(app: App):
    """El panel de venta debe reflejar el turno abierto en otra sección."""
    app.wait_text_gone("Abre un turno de caja para cobrar")
    assert_contains(
        app.section_text("Punto de venta"),
        "Caja E2E",
        "el panel de venta no muestra la caja activa",
    )


def vender(app: App) -> str:
    app.click_xpath(
        "//section[.//h2[normalize-space()='Punto de venta']]"
        "//button[contains(.,'Café E2E')]"
    )
    # La variante llega del backend; su ausencia indicaría un IPC roto.
    WebDriverWait(app.driver, 20).until(
        EC.presence_of_element_located(
            (
                By.XPATH,
                "//section[.//h2[normalize-space()='Punto de venta']]"
                f"//option[normalize-space()='Grande · ${PRECIO_VARIANTE}']",
            )
        )
    )
    app.select("Punto de venta", "Variante", f"Grande · ${PRECIO_VARIANTE}")
    app.toggle_option("Punto de venta", f"Shot extra (+{DELTA_EXTRA})")
    app.toggle_option("Punto de venta", "Sin azúcar")
    app.click("Punto de venta", "Agregar al carrito")
    app.wait_text("Producto agregado al carrito")

    cart = app.section_text("Punto de venta")
    assert_contains(cart, f"${UNITARIO_TOTAL} × 1", "el carrito no usa el precio de Rust")

    app.click_xpath(
        "//section[.//h2[normalize-space()='Punto de venta']]"
        "//button[normalize-space()='+']"
    )
    app.wait_text(f"× {CANTIDAD}")

    total_carrito = app.section_text("Punto de venta")
    assert_contains(total_carrito, f"${TOTAL_VENTA}", "total del carrito incorrecto")

    app.fill("Punto de venta", "Efectivo recibido", EFECTIVO_RECIBIDO)
    app.click("Punto de venta", "Cobrar")
    app.wait_text("Folio V-")

    confirmacion = app.section_text("Punto de venta")
    assert_contains(confirmacion, f"Total ${TOTAL_VENTA}", "total confirmado incorrecto")
    assert_contains(confirmacion, f"Cambio ${CAMBIO}", "cambio incorrecto")
    app.screenshot("01-venta-confirmada")

    for line in confirmacion.splitlines():
        if line.startswith("Folio V-"):
            return line.split(" ", 1)[1].split(".")[0]
    raise AssertionError("No se encontró el folio en la confirmación")


def iniciar_sesion(app: App):
    wait_heading(app, "Entrar a la caja")
    app.fill("Entrar a la caja", "Usuario", "propietario", tag="h1")
    app.fill("Entrar a la caja", "Contraseña", "contrasena-segura", tag="h1")
    app.click("Entrar a la caja", "Entrar", tag="h1")
    wait_heading(app, "Productos")


def verificar_persistencia(app: App) -> dict:
    app.select("Existencias y movimientos", "Producto", "CAF-001 · Café E2E")
    app.wait_text("Café E2E:")
    inventory = app.section_text("Existencias y movimientos")
    assert_contains(inventory, f"Café E2E: {STOCK_FINAL}", "el inventario no persistió")

    caja = app.section_text("Turno de efectivo")
    assert_contains(caja, "open", "el turno no sigue abierto tras reiniciar")
    assert_contains(caja, SALDO_ESPERADO, "el saldo esperado no incluye la venta")
    app.screenshot("02-reinicio-saldo")

    venta = app.invoke("list_products")
    if not venta:
        raise AssertionError("el catálogo quedó vacío tras el reinicio")
    return {"inventario": inventory, "caja": caja}


def cerrar_turno(app: App):
    app.fill("Turno de efectivo", "Efectivo contado", SALDO_ESPERADO)
    app.click("Turno de efectivo", "Cerrar turno")
    app.wait_text("Turno cerrado")

    cierre = app.section_text("Turno de efectivo")
    assert_contains(cierre, "Turno cerrado", "no se confirmó el cierre")
    assert_contains(cierre, "Diferencia\nExacto", "la diferencia del arqueo exacto no es cero")
    assert_contains(cierre, SALDO_ESPERADO, "el cierre no conserva el saldo esperado")
    assert_contains(cierre, "Caja E2E · CJ-01", "el historial no identifica la caja")
    assert_contains(cierre, "Ana E2E", "el historial no identifica al operador")
    app.screenshot("03-turno-cerrado")


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

        crear_catalogo(app)
        steps.append("catalogo")

        modificadores = configurar_modificadores(app)
        report["modificadores"] = modificadores
        steps.append("modificadores")

        registrar_inventario(app)
        steps.append("inventario")

        abrir_caja(app)
        steps.append("caja_abierta")

        esperar_turno_en_venta(app)
        steps.append("turno_visible_en_venta")

        report["folio"] = vender(app)
        steps.append("venta")

        app.quit()
        app = App(app_path)
        iniciar_sesion(app)
        steps.append("reinicio_login")

        report["persistencia"] = verificar_persistencia(app)
        steps.append("persistencia")

        cerrar_turno(app)
        steps.append("arqueo_cierre")

        report.update({"steps": steps, "result": "PASS"})
        print(json.dumps(report, ensure_ascii=False))
        return 0
    except Exception as error:  # noqa: BLE001
        app.screenshot("99-error")
        report.update(
            {
                "steps": steps,
                "error": str(error),
                "text": app.text()[:3000],
            }
        )
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
