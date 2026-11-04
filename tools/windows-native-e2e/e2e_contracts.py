"""Prueba de contrato IPC sobre el ejecutable real.

Cubre la clase de defecto detectada en G2.5: Tauri 2 renombra los argumentos a
camelCase por omisión, mientras el contrato publicado en
`docs/03-contracts/ipc.md` usa snake_case (`product_id`, `register_id`...). Si
algún comando vuelve a declarar los argumentos por omisión, esta prueba falla con
`missing required key`.

Recorre los comandos con argumentos compuestos y verifica además que el motor de
precios devuelve la aritmética exacta esperada.

Uso:
    python e2e_contracts.py <ruta-del-ejecutable>
"""

import json
import sys
import time
from pathlib import Path

from selenium import webdriver
from selenium.webdriver.common.options import BaseOptions

PRECIO_PRODUCTO = "10.00"
PRECIO_VARIANTE = "12.00"
DELTA_EXTRA = "2.00"


class WryOptions(BaseOptions):
    def __init__(self, application: str):
        super().__init__()
        self._application = application

    @property
    def default_capabilities(self):
        return {"browserName": "wry"}

    def to_capabilities(self):
        return {"browserName": "wry", "tauri:options": {"application": self._application}}


class Client:
    def __init__(self, app_path: str):
        self.driver = webdriver.Remote(
            command_executor="http://127.0.0.1:4444",
            options=WryOptions(app_path),
        )
        self.driver.set_script_timeout(20)
        self.checks: dict[str, object] = {}
        self._wait_ready()

    def _wait_ready(self, timeout: float = 30.0):
        """Tauri rechaza el IPC si el webview aún no tiene un origen válido."""
        deadline = time.time() + timeout
        while time.time() < deadline:
            ready = self.driver.execute_script(
                "return Boolean(window.__TAURI_INTERNALS__)"
                " && document.readyState === 'complete'"
                " && Boolean(document.location && String(document.location.href))"
            )
            if ready:
                return
            time.sleep(0.25)
        raise AssertionError("el webview no quedó listo para IPC")

    def quit(self):
        try:
            self.driver.quit()
        except Exception:
            pass

    def invoke(self, command: str, args: dict | None = None):
        payload = json.dumps(args or {})
        script = (
            "const done=arguments[arguments.length-1];"
            f"window.__TAURI_INTERNALS__.invoke('{command}',{payload})"
            ".then(v=>done({ok:true,v})).catch(e=>done({ok:false,e}));"
        )
        result = self.driver.execute_async_script(script)
        if not result.get("ok"):
            raise AssertionError(f"{command}: {result.get('e')}")
        return result["v"]


def main() -> int:
    app_path = str(
        Path(
            sys.argv[1]
            if len(sys.argv) > 1
            else "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe"
        ).resolve()
    )
    client = Client(app_path)
    report: dict = {"application": app_path}
    try:
        report["app_status"] = client.invoke("app_status")
        client.invoke(
            "bootstrap",
            {
                "request": {
                    "organization_name": "Contrato E2E",
                    "branch_name": "Matriz",
                    "branch_code": "CT-01",
                    "owner_username": "propietario",
                    "owner_display_name": "Ana E2E",
                    "owner_password": "contrasena-segura",
                }
            },
        )
        actor = client.invoke(
            "login",
            {"request": {"username": "propietario", "password": "contrasena-segura"}},
        )
        report["permisos"] = actor["permissions"]

        category = client.invoke("create_category", {"draft": {"name": "Bebidas", "sort_order": 0}})
        product = client.invoke(
            "create_product",
            {
                "draft": {
                    "sku": "CAF-001",
                    "name": "Café E2E",
                    "price": PRECIO_PRODUCTO,
                    "tax_rate_bp": 1600,
                }
            },
        )
        client.invoke(
            "assign_product_category",
            {"product_id": product["id"], "category_id": category["id"]},
        )
        variant = client.invoke(
            "create_product_variant",
            {
                "draft": {
                    "product_id": product["id"],
                    "name": "Grande",
                    "price": PRECIO_VARIANTE,
                    "sort_order": 0,
                }
            },
        )
        group = client.invoke(
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
        option = client.invoke(
            "create_modifier_option",
            {
                "draft": {
                    "group_id": group["id"],
                    "name": "Shot extra",
                    "kind": "extra",
                    "price_delta": DELTA_EXTRA,
                    "sort_order": 0,
                }
            },
        )
        register = client.invoke(
            "create_cash_register",
            {"draft": {"name": "Caja Contrato", "code": "CT-01"}},
        )

        # Lecturas con argumentos compuestos: todas deben aceptar snake_case.
        variants = client.invoke("list_product_variants", {"product_id": product["id"]})
        groups = client.invoke("list_modifier_groups", {"product_id": product["id"]})
        options = client.invoke("list_modifier_options", {"group_id": group["id"]})
        stock = client.invoke("current_stock", {"product_id": product["id"], "branch_id": None})
        movements = client.invoke("list_inventory_movements", {"product_id": None, "limit": 10})
        audit = client.invoke("list_audit_events", {"limit": 10})
        registers = client.invoke("list_cash_registers")
        shift = client.invoke("active_cash_shift", {"register_id": register["id"]})
        history = client.invoke("list_cash_shift_history")
        products = client.invoke("list_products")
        categories = client.invoke("list_categories")
        commercial = client.invoke("commercial_state")

        assert [item["name"] for item in variants] == ["Grande"], "variantes no consultadas"
        assert [item["name"] for item in groups] == ["Extras"], "grupos no consultados"
        assert [item["name"] for item in options] == ["Shot extra"], "opciones no consultadas"
        assert float(stock) == 0.0, f"existencia inicial inesperada: {stock}"
        assert movements == [], "movimientos de inventario inesperados"
        assert audit, "no se registraron eventos de auditoría"
        assert [item["code"] for item in registers] == ["CT-01"], "cajas no consultadas"
        assert shift is None, "no debería existir turno abierto"
        assert history == [], "historial inesperado"
        assert [item["sku"] for item in products] == ["CAF-001"], "productos no consultados"
        assert [item["name"] for item in categories] == ["Bebidas"], "categorías no consultadas"
        assert commercial["plan"], "estado comercial vacío"

        configuration = client.invoke(
            "configure_product",
            {
                "draft": {
                    "product_id": product["id"],
                    "variant_id": variant["id"],
                    "selected_option_ids": [option["id"]],
                }
            },
        )
        report["configuracion"] = configuration
        assert configuration["base_price"] == PRECIO_VARIANTE, "base de la variante incorrecta"
        assert configuration["modifiers_total"] == DELTA_EXTRA, "incremento incorrecto"
        assert configuration["tax"] == "2.24", "impuesto calculado incorrecto"
        assert configuration["unit_price"] == "16.24", "total unitario incorrecto"

        report["result"] = "PASS"
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0
    except Exception as error:  # noqa: BLE001
        report["result"] = "FAIL"
        report["error"] = str(error)
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 1
    finally:
        client.quit()


if __name__ == "__main__":
    raise SystemExit(main())
