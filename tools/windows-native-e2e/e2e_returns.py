"""E2E nativo de devoluciones y reportes sobre el ejecutable real.

Se ejecuta en una base SQLite aislada (la crea `run-e2e.ps1`) y demuestra:

    bootstrap -> venta en efectivo -> devolución parcial reintegrable ->
    reembolso exacto -> inventario -> reportes -> reinicio -> persistencia ->
    denegación al cajero

La preparación de catálogo, inventario y turno usa IPC directo (datos de
soporte); la venta, la devolución y los reportes se operan por la interfaz.
"""

import json
import os
import sys
from pathlib import Path

from selenium.webdriver.common.by import By

from e2e_flow import App, DIALOG, NAV, assert_contains, bootstrap, wait_screen

PRECIO = "10.00"
FONDO_INICIAL = "100.00"
EFECTIVO = "50.00"
TOTAL_VENTA = "23.20"
CAMBIO = "26.80"
REEMBOLSO = "11.60"
SALDO_TRAS_DEVOLUCION = "111.60"
STOCK_INICIAL = "20"
STOCK_TRAS_VENTA = "18"
STOCK_TRAS_DEVOLUCION = "19"


def abrir_caja(app: App) -> dict:
    """Crea la caja y abre el turno desde la interfaz para que el shell lo conozca."""
    app.nav("cash")
    wait_screen(app, "Caja y turnos")
    app.fill("//section", "Nombre", "Caja devoluciones")
    app.fill("//section", "Código", "CJ-DEV")
    app.click_xpath("//button[normalize-space()='Crear caja']")
    app.wait_text("Caja creada")
    app.wait_xpath("//button[normalize-space()='Abrir turno']")
    app.fill("//section", "Fondo inicial", FONDO_INICIAL)
    app.click_xpath("//button[normalize-space()='Abrir turno']")
    app.wait_text("Turno abierto")
    register = app.invoke("list_cash_registers")[0]
    shift = app.invoke("active_cash_shift", {"register_id": register["id"]})
    if shift is None:
        raise AssertionError("el turno no quedó abierto tras crearlo en la interfaz")
    return {"register_id": register["id"], "shift_id": shift["id"]}


def preparar_datos(app: App) -> dict:
    product = app.invoke(
        "create_product",
        {
            "draft": {
                "sku": "CAF-DEV",
                "name": "Café devolución",
                "price": PRECIO,
                "tax_rate_bp": 1600,
                "category_id": None,
            }
        },
    )
    app.invoke(
        "record_inventory_movement",
        {
            "draft": {
                "product_id": product["id"],
                "kind": "inbound",
                "quantity": STOCK_INICIAL,
                "reason": None,
            }
        },
    )
    caja = abrir_caja(app)
    return {"product_id": product["id"], **caja}


def vender(app: App, product_id: str, shift_id: str) -> str:
    """Venta de soporte por IPC; la devolución y los reportes se operan por UI."""
    sale = app.invoke(
        "checkout_sale",
        {
            "draft": {
                "shift_id": shift_id,
                "idempotency_key": "e2e-return-sale-1",
                "items": [
                    {
                        "product_id": product_id,
                        "variant_id": None,
                        "quantity": "2",
                        "selected_option_ids": [],
                    }
                ],
                "cash_received": EFECTIVO,
            }
        },
    )
    if sale["total"] != TOTAL_VENTA:
        raise AssertionError(f"total de venta inesperado: {sale['total']}")
    if sale["payment"]["change"] != CAMBIO:
        raise AssertionError(f"cambio inesperado: {sale['payment']['change']}")
    return sale["folio"]


def registrar_devolucion(app: App, folio: str) -> dict:
    app.nav("returns")
    wait_screen(app, "Devoluciones")
    app.fill("//section[.//h2[normalize-space()='Buscar venta']]", "Folio de venta", folio)
    app.click_xpath("//button[normalize-space()='Buscar venta']")
    app.wait_text(f"Venta {folio}")
    detalle = "//form[@aria-label='Devolución de la venta']"
    cantidad = app.wait_xpath(f"{detalle}//input[@aria-label='Cantidad a devolver Café devolución']")
    cantidad.clear()
    cantidad.send_keys("1")
    app.fill(detalle, "Motivo", "producto en buen estado")
    app.screenshot("devolucion-detalle")
    app.click_xpath(f"{detalle}//button[normalize-space()='Calcular reembolso']")
    app.wait_text("Reembolso calculado por Rust")
    assert_contains(app.text(), f"${REEMBOLSO}", "el reembolso calculado no aparece")
    app.screenshot("devolucion-preview")
    app.click_xpath(f"{detalle}//button[normalize-space()='Confirmar devolución']")
    app.wait_text("Devolución de 1 partida(s) registrada")
    cuerpo = app.text()
    assert_contains(cuerpo, f"Reembolso ${REEMBOLSO}", "reembolso informado incorrecto")
    return {"refund": REEMBOLSO, "registro": True}


def verificar_reportes(app: App) -> dict:
    app.nav("reports")
    wait_screen(app, "Reportes")
    app.wait_text("Ventas netas")
    cuerpo = app.text()
    assert_contains(cuerpo, f"${TOTAL_VENTA}", "ventas brutas ausentes en reportes")
    assert_contains(cuerpo, f"${REEMBOLSO}", "devoluciones ausentes en reportes")
    assert_contains(cuerpo, "PENDIENTES DE RECIBIR", "sección de compras ausente")
    assert_contains(cuerpo, "MERMA", "sección de inventario ausente")
    app.screenshot("reportes")
    return {"ventas_brutas": TOTAL_VENTA, "devuelto": REEMBOLSO}


def iniciar_sesion(app: App, usuario: str, clave: str):
    wait_screen(app, "Entrar a la caja")
    app.fill("//section", "Usuario", usuario)
    app.fill("//section", "Contraseña", clave)
    app.click_xpath("//button[normalize-space()='Entrar']")
    wait_screen(app, "Ventas")


def crear_cajero(app: App):
    app.invoke(
        "create_user",
        {
            "draft": {
                "username": "cajero.devoluciones",
                "display_name": "Cajero Devoluciones",
                "role": "cashier",
                "password": "clave-cajero-2026",
            }
        },
    )


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

        datos = preparar_datos(app)
        steps.append("datos")

        folio = vender(app, datos["product_id"], datos["shift_id"])
        report["folio"] = folio
        steps.append("venta")
        assert (
            app.invoke("current_stock", {"product_id": datos["product_id"]}) == STOCK_TRAS_VENTA
        ), "el stock tras la venta no coincide"

        report["devolucion"] = registrar_devolucion(app, folio)
        steps.append("devolucion")
        assert (
            app.invoke("current_stock", {"product_id": datos["product_id"]})
            == STOCK_TRAS_DEVOLUCION
        ), "la devolución reintegrable no actualizó el stock"

        shift = app.invoke("active_cash_shift", {"register_id": datos["register_id"]})
        assert shift is not None, "el turno debe seguir abierto"
        assert shift["expected_amount"] == SALDO_TRAS_DEVOLUCION, "el saldo esperado es incorrecto"
        assert shift["exit_total"] == REEMBOLSO, "la salida de caja no es única"
        report["saldo_esperado"] = shift["expected_amount"]

        report["reportes"] = verificar_reportes(app)
        steps.append("reportes")

        app.quit()
        app = App(app_path)
        iniciar_sesion(app, "propietario", "contrasena-segura")
        steps.append("reinicio")

        devoluciones = app.invoke("list_sale_returns")
        assert len(devoluciones) == 1, "la devolución no persistió"
        assert devoluciones[0]["refund_total"] == REEMBOLSO, "el reembolso persistido cambió"
        assert (
            app.invoke("current_stock", {"product_id": datos["product_id"]})
            == STOCK_TRAS_DEVOLUCION
        ), "el stock no persistió"
        report["persistencia"] = {
            "devoluciones": len(devoluciones),
            "stock": STOCK_TRAS_DEVOLUCION,
        }
        steps.append("persistencia")

        app.nav("reports")
        wait_screen(app, "Reportes")
        app.wait_text("Ventas netas")
        assert (
            app.invoke("current_stock", {"product_id": datos["product_id"]})
            == STOCK_TRAS_DEVOLUCION
        )

        crear_cajero(app)
        app.click_xpath("//button[@aria-label='Cerrar sesión']")
        iniciar_sesion(app, "cajero.devoluciones", "clave-cajero-2026")
        sin_devoluciones = not app.driver.find_elements(By.XPATH, NAV.format("returns"))
        sin_reportes = not app.driver.find_elements(By.XPATH, NAV.format("reports"))
        assert sin_devoluciones, "el cajero no debe ver Devoluciones"
        assert sin_reportes, "el cajero no debe ver Reportes"
        intento = app.invoke_raw("list_sale_returns")
        assert not intento.get("ok"), "el cajero no debe consultar devoluciones por IPC"
        report["permisos"] = {
            "cajero_sin_devoluciones": sin_devoluciones,
            "cajero_sin_reportes": sin_reportes,
            "ipc_denegado": True,
        }
        steps.append("permisos")

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
                (Path(directory) / "returns-report.json").write_text(
                    json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
                )
            except Exception:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
