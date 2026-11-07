"""Continuidad comercial tras actualizar el instalador.

Se ejecuta con ``run-e2e.ps1 -DatabasePath`` sobre el dataset que creó
``e2e_flow.py`` con la versión **anterior**. Comprueba, con el binario nuevo:

* que la base existente abre sin corromperse y la pantalla de acceso aparece
  conforme al diseño de sesiones;
* que los datos comerciales siguen completos (catálogo, existencias, historial
  de turnos, gastos, merma, compras);
* que se puede abrir un turno nuevo y vender otra vez;
* que el SQLite resultante conserva integridad, claves foráneas y exactamente
  una venta nueva con sus pagos, inventario, caja y auditoría.

No modifica la base de trabajo real: ``run-e2e.ps1`` rechaza esa ruta.
"""

from __future__ import annotations

import json
import os
import re
import sqlite3
import sys
from decimal import Decimal
from pathlib import Path

from selenium.webdriver.common.by import By
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.support.ui import WebDriverWait

from e2e_flow import (
    App,
    DIALOG,
    FONDO_INICIAL,
    iniciar_sesion,
    vender,
    wait_screen,
)

# Una venta de dos unidades rebaja 2000 milésimas las existencias.
UNIDADES_MILLI = 2000


def abrir_turno(app: App) -> str:
    """Abre un turno si no hay uno vigente y devuelve qué hizo."""
    app.nav("cash")
    wait_screen(app, "Caja y turnos")
    if app.driver.find_elements(By.XPATH, "//button[normalize-space()='Abrir turno']"):
        app.fill("//section", "Fondo inicial", FONDO_INICIAL)
        app.click_xpath("//button[normalize-space()='Abrir turno']")
        app.wait_text("Turno abierto")
        return "turno_abierto"
    app.wait_text("Turno abierto")
    return "turno_ya_abierto"


def saldo_esperado(app: App) -> Decimal:
    """Saldo esperado del turno vigente, leído de la pantalla de caja."""
    texto = app.text_of("//section[@aria-label='Operación del turno']")
    encontrado = re.search(r"SALDO ESPERADO\s*\$([0-9.,]+)", texto)
    if not encontrado:
        raise AssertionError(f"no se encontró el saldo esperado en:\n{texto}")
    return Decimal(encontrado.group(1).replace(",", ""))


def cerrar_confirmacion(app: App) -> None:
    app.click_xpath(f"{DIALOG}//button[normalize-space()='Nueva venta']")
    WebDriverWait(app.driver, 15).until(
        EC.invisibility_of_element_located((By.CSS_SELECTOR, ".modal-backdrop"))
    )


def leer_resumen(database: Path) -> dict:
    """Estado persistido antes de la venta de continuidad."""
    conexion = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    conexion.row_factory = sqlite3.Row
    try:
        ventas = conexion.execute(
            "SELECT COUNT(*) AS n FROM sale WHERE status = 'completed'"
        ).fetchone()["n"]
        stock = conexion.execute(
            "SELECT COALESCE(SUM(CASE WHEN kind IN ('inbound','adjustment_in') "
            "THEN quantity_milli ELSE -quantity_milli END), 0) AS stock "
            "FROM inventory_movement"
        ).fetchone()["stock"]
        return {"ventas_confirmadas": ventas, "stock_milli": stock}
    finally:
        conexion.close()


def leer_estado(app: App) -> dict:
    productos = app.invoke("list_products")
    existencias = {nivel["product_id"]: nivel["quantity"] for nivel in app.invoke("list_stock_levels")}
    turnos = app.invoke("list_cash_shift_history")
    return {
        "productos": len(productos),
        "skus": sorted(producto["sku"] for producto in productos),
        "existencias": existencias,
        "turnos_historicos": len(turnos),
        "turnos_cerrados": sum(1 for turno in turnos if turno.get("status") == "closed"),
        "gastos": len(app.invoke("list_expenses")),
        "merma": len(app.invoke("list_waste")),
        "compras": len(app.invoke("list_purchase_orders")),
        "usuarios": len(app.invoke("list_users")),
    }


def verificar_base(database: Path, folio: str, inicial: dict) -> dict:
    conexion = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    conexion.row_factory = sqlite3.Row
    fallos: list[str] = []
    try:
        ventas = conexion.execute(
            "SELECT COUNT(*) AS n FROM sale WHERE status = 'completed'"
        ).fetchone()["n"]
        esperadas = inicial["ventas_confirmadas"] + 1
        if ventas != esperadas:
            fallos.append(
                "se esperaba exactamente una venta nueva tras actualizar; "
                f"había {inicial['ventas_confirmadas']} y hay {ventas}"
            )

        nueva = conexion.execute(
            "SELECT * FROM sale WHERE folio = ?", (folio,)
        ).fetchone()
        if nueva is None:
            fallos.append(f"la venta nueva {folio} no está persistida")
        else:
            if (nueva["subtotal_minor"], nueva["tax_minor"], nueva["total_minor"]) != (2800, 448, 3248):
                fallos.append("los importes de la venta nueva no coinciden con el cálculo de Rust")

            pago = conexion.execute(
                "SELECT * FROM sale_payment WHERE sale_id = ?", (nueva["id"],)
            ).fetchone()
            if pago is None:
                fallos.append("la venta nueva no tiene pago")
            elif (pago["amount_minor"], pago["change_minor"]) != (5000, 1752):
                fallos.append("el pago o el cambio de la venta nueva es incorrecto")

            partida = conexion.execute(
                "SELECT * FROM sale_item WHERE sale_id = ?", (nueva["id"],)
            ).fetchone()
            if partida is None:
                fallos.append("la venta nueva no tiene partidas")

            movimiento = conexion.execute(
                "SELECT COUNT(*) AS n FROM cash_movement WHERE kind = 'in' AND reason = ?",
                (f"venta {folio}",),
            ).fetchone()["n"]
            if movimiento != 1:
                fallos.append(
                    f"se esperaba un único ingreso de caja para la venta nueva; hay {movimiento}"
                )

            auditoria = conexion.execute(
                "SELECT COUNT(*) AS n FROM audit_event WHERE action = 'sales.completed' AND entity_id = ?",
                (nueva["id"],),
            ).fetchone()["n"]
            if auditoria != 1:
                fallos.append(f"se esperaba una auditoría de la venta nueva; hay {auditoria}")

        producto = conexion.execute(
            "SELECT id FROM product WHERE sku = 'CAF-001'"
        ).fetchone()
        if producto is None:
            fallos.append("el producto de la versión anterior desapareció")
        else:
            existencias = conexion.execute(
                "SELECT COALESCE(SUM(CASE WHEN kind IN ('inbound','adjustment_in') "
                "THEN quantity_milli ELSE -quantity_milli END), 0) AS stock "
                "FROM inventory_movement WHERE product_id = ?",
                (producto["id"],),
            ).fetchone()["stock"]
            esperadas = inicial["stock_milli"] - UNIDADES_MILLI
            if existencias != esperadas:
                fallos.append(
                    "las existencias tras la venta nueva no cuadran: "
                    f"{existencias} en lugar de {esperadas}"
                )

        integridad = conexion.execute("PRAGMA integrity_check").fetchone()[0]
        if integridad != "ok":
            fallos.append(f"integrity_check devolvió {integridad}")
        referencias = conexion.execute("PRAGMA foreign_key_check").fetchall()
        if referencias:
            fallos.append(f"foreign_key_check encontró {len(referencias)} referencias inválidas")

        return {
            "ventas_confirmadas": ventas,
            "folio_nuevo": folio,
            "integridad": integridad,
            "referencias_invalidas": len(referencias),
            "fallos": fallos,
        }
    finally:
        conexion.close()


def main() -> int:
    if len(sys.argv) != 2:
        print(json.dumps({"error": "uso: e2e_post_upgrade.py <ejecutable>"}))
        return 2

    app_path = str(Path(sys.argv[1]).resolve())
    database = os.environ.get("HIQNET_POS_DB_PATH")
    if not database:
        print(json.dumps({"error": "falta HIQNET_POS_DB_PATH; usa run-e2e.ps1 -DatabasePath"}))
        return 2

    report: dict = {"application": app_path, "database": database}
    # Se lee antes de abrir la aplicación: el límite de la prueba es "una venta
    # nueva", no un número fijo que dependa de ejecuciones anteriores.
    inicial = leer_resumen(Path(database))
    report["inicial"] = inicial

    app = App(app_path)
    try:
        # La base de la versión anterior debe abrir y mostrar el acceso.
        wait_screen(app, "Entrar a la caja")
        app.screenshot("actualizado-login")
        iniciar_sesion(app)

        antes = leer_estado(app)
        report["antes_de_vender"] = antes
        if "CAF-001" not in antes["skus"]:
            raise AssertionError("el catálogo de la versión anterior no está disponible")
        if antes["turnos_cerrados"] < 1:
            raise AssertionError("el historial de turnos no conserva el turno cerrado")
        if antes["usuarios"] < 2:
            raise AssertionError("los usuarios de la versión anterior no están disponibles")

        app.nav("inventory")
        wait_screen(app, "Inventario")
        app.select(
            "//header[contains(@class,'screen-header')]", "Producto", "CAF-001 · Café E2E"
        )
        stock_ui = str(inicial["stock_milli"] // 1000)
        app.wait_xpath(
            f"//span[contains(@class,'stock-figure') and normalize-space()='{stock_ui}']",
            timeout=30,
        )
        app.screenshot("actualizado-inventario")

        report["turno"] = abrir_turno(app)
        saldo_antes = saldo_esperado(app)
        folio = vender(app)
        report["folio"] = folio
        cerrar_confirmacion(app)

        app.nav("cash")
        wait_screen(app, "Caja y turnos")
        saldo_despues = saldo_esperado(app)
        esperado = saldo_antes + Decimal("32.48")
        if saldo_despues != esperado:
            raise AssertionError(
                f"el saldo del turno no incorpora la venta: {saldo_despues} en lugar de {esperado}"
            )
        report["saldo_turno"] = {"antes": str(saldo_antes), "despues": str(saldo_despues)}
        app.screenshot("actualizado-caja")

        report["en_base"] = verificar_base(Path(database), folio, inicial)
        if report["en_base"]["fallos"]:
            print(json.dumps(report, ensure_ascii=False))
            return 1

        report["result"] = "PASS"
        print(json.dumps(report, ensure_ascii=False))
        return 0
    except Exception as error:
        report["result"] = "FAIL"
        report["error"] = str(error)
        try:
            app.screenshot("actualizado-fallo")
        except Exception:
            pass
        print(json.dumps(report, ensure_ascii=False))
        return 1
    finally:
        app.quit()


if __name__ == "__main__":
    raise SystemExit(main())
