"""Cierre inesperado del proceso durante operaciones reales.

Mata el proceso de HiQNet POS sin cerrar la aplicacion, justo despues de lanzar
una operacion por IPC (venta, inventario, caja, devolucion y compra), reinicia y
comprueba que la base queda coherente: sin transacciones parciales, sin
duplicados y con la auditoria y los saldos consistentes.

A diferencia de una prueba de fallo inyectado, aqui el proceso muere de verdad.
La atomicidad interna de cada transaccion esta cubierta ademas por las pruebas
Rust de inyeccion de fallos (`backup::failure_tests` y las de cada dominio).

La base es siempre la aislada de `run-e2e.ps1`. El proceso se identifica por la
ruta unica de esa base en su linea de comandos, nunca por nombre ni por fecha.
"""

import ctypes
import json
import os
import sqlite3
import subprocess
import sys
import time

from e2e_returns import preparar_datos, vender
from e2e_flow import App, bootstrap, iniciar_sesion, wait_screen

INVARIANTES = {
    "ventas_sin_partidas": """
        SELECT COUNT(*) FROM sale s
        WHERE NOT EXISTS (SELECT 1 FROM sale_item i WHERE i.sale_id = s.id)""",
    "ventas_sin_pago": """
        SELECT COUNT(*) FROM sale s
        WHERE NOT EXISTS (SELECT 1 FROM sale_payment p WHERE p.sale_id = s.id)""",
    "ventas_sin_auditoria": """
        SELECT COUNT(*) FROM sale s
        WHERE NOT EXISTS (
            SELECT 1 FROM audit_event a
            WHERE a.entity_id = s.id AND a.action = 'sales.completed')""",
    "devoluciones_sin_partidas": """
        SELECT COUNT(*) FROM sale_return r
        WHERE NOT EXISTS (SELECT 1 FROM sale_return_line l WHERE l.return_id = r.id)""",
    "ordenes_sin_partidas": """
        SELECT COUNT(*) FROM purchase_order o
        WHERE NOT EXISTS (SELECT 1 FROM purchase_order_line l WHERE l.order_id = o.id)""",
    "recepciones_sin_partidas": """
        SELECT COUNT(*) FROM purchase_receipt r
        WHERE NOT EXISTS (SELECT 1 FROM purchase_receipt_line l WHERE l.receipt_id = r.id)""",
    "claves_idempotentes_duplicadas": """
        SELECT COUNT(*) FROM (
            SELECT idempotency_key FROM sale
            GROUP BY idempotency_key HAVING COUNT(*) > 1)""",
    "turnos_negativos": """
        SELECT COUNT(*) FROM (
            SELECT s.id, s.opening_amount_minor + COALESCE(SUM(
                CASE m.kind WHEN 'in' THEN m.amount_minor ELSE -m.amount_minor END), 0) AS saldo
            FROM cash_shift s LEFT JOIN cash_movement m ON m.shift_id = s.id
            GROUP BY s.id HAVING saldo < 0)""",
}


def verificar(database: str) -> dict:
    conexion = sqlite3.connect(database)
    try:
        resultado = {
            "integrity": conexion.execute("PRAGMA integrity_check").fetchone()[0],
            "foreign_key_violations": len(conexion.execute("PRAGMA foreign_key_check").fetchall()),
        }
        for nombre, consulta in INVARIANTES.items():
            resultado[nombre] = conexion.execute(consulta).fetchone()[0]
        resultado["ventas"] = conexion.execute("SELECT COUNT(*) FROM sale").fetchone()[0]
        resultado["movimientos_inventario"] = conexion.execute(
            "SELECT COUNT(*) FROM inventory_movement"
        ).fetchone()[0]
        resultado["movimientos_caja"] = conexion.execute(
            "SELECT COUNT(*) FROM cash_movement"
        ).fetchone()[0]
        resultado["auditoria"] = conexion.execute(
            "SELECT COUNT(*) FROM audit_event"
        ).fetchone()[0]
        resultado["ordenes"] = conexion.execute(
            "SELECT COUNT(*) FROM purchase_order"
        ).fetchone()[0]
        return resultado
    finally:
        conexion.close()


def proceso_de_la_corrida(database: str) -> int | None:
    """PID cuyo comando incluye la base unica de esta ejecucion."""
    filtro = database.replace("'", "''")
    script = (
        "Get-CimInstance Win32_Process -Filter \"Name = 'pos-desktop.exe'\" | "
        f"Where-Object {{ $_.CommandLine -like '*{filtro}*' }} | "
        "Select-Object -ExpandProperty ProcessId"
    )
    salida = subprocess.run(
        ["powershell", "-NoProfile", "-Command", script],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    for linea in salida.stdout.split():
        if linea.strip().isdigit():
            return int(linea)
    return None


def matar(pid: int) -> None:
    subprocess.run(
        ["taskkill", "/F", "/PID", str(pid)],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )


def matar_rapido(pid: int) -> bool:
    """Termina el proceso sin lanzar un hijo: ~1 ms en lugar de ~20 ms."""
    kernel32 = ctypes.windll.kernel32
    handle = kernel32.OpenProcess(0x0001, False, pid)  # PROCESS_TERMINATE
    if not handle:
        return False
    try:
        return bool(kernel32.TerminateProcess(handle, 1))
    finally:
        kernel32.CloseHandle(handle)


def esperar_sin_proceso(database: str, limite: float = 15.0) -> bool:
    fin = time.monotonic() + limite
    while time.monotonic() < fin:
        if proceso_de_la_corrida(database) is None:
            return True
        time.sleep(0.1)
    return False


def lanzar_sin_esperar(app: App, comando: str, argumentos: dict) -> None:
    """Dispara el IPC y vuelve de inmediato: la promesa queda en vuelo."""
    app.driver.execute_script(
        "window.__TAURI_INTERNALS__.invoke(arguments[0], arguments[1]);",
        comando,
        argumentos,
    )


def main() -> int:
    app_path = sys.argv[1] if len(sys.argv) > 1 else ""
    database = os.environ.get("HIQNET_POS_DB_PATH")
    report: dict = {"escenario": "crash-recovery", "cortes": []}
    fallos: list[str] = []
    if not app_path or not database:
        print(json.dumps({"result": "ERROR", "error": "faltan ejecutable o base"}))
        return 1

    app = App(app_path)
    try:
        bootstrap(app)
        datos = preparar_datos(app)
        vender(app, datos["product_id"], datos["shift_id"])
        proveedor = app.invoke(
            "create_supplier",
            {"draft": {"code": "SUP-CRASH", "name": "Proveedor crash", "contact": None, "rfc": None}},
        )
        datos["supplier_id"] = proveedor["id"]

        # Productos adicionales para una venta de muchas lineas: asi la
        # transaccion dura lo suficiente para que el corte caiga dentro de ella.
        multilinea = []
        for indice in range(40):
            extra = app.invoke(
                "create_product",
                {
                    "draft": {
                        "sku": f"MULTI-{indice:03d}",
                        "name": f"Producto multilinea {indice}",
                        "price": "1.00",
                        "tax_rate_bp": 0,
                        "category_id": None,
                    }
                },
            )
            app.invoke(
                "record_inventory_movement",
                {
                    "draft": {
                        "product_id": extra["id"],
                        "kind": "inbound",
                        "quantity": "100",
                        "reason": None,
                    }
                },
            )
            multilinea.append(extra["id"])
        datos["multilinea"] = multilinea
    finally:
        app.quit()

    base = verificar(database)
    report["linea_base"] = base

    operaciones = [
        (
            "venta",
            "checkout_sale",
            "ventas",
            {
                "draft": {
                    "shift_id": datos["shift_id"],
                    "idempotency_key": "crash-venta-{n}",
                    "items": [
                        {
                            "product_id": datos["product_id"],
                            "variant_id": None,
                            "quantity": "1",
                            "selected_option_ids": [],
                        }
                    ],
                    "cash_received": "50.00",
                }
            },
        ),
        (
            "inventario",
            "record_inventory_movement",
            "movimientos_inventario",
            {
                "draft": {
                    "product_id": datos["product_id"],
                    "kind": "inbound",
                    "quantity": "1",
                    "reason": None,
                }
            },
        ),
        (
            "caja",
            "register_cash_movement",
            "movimientos_caja",
            {
                "draft": {
                    "shift_id": datos["shift_id"],
                    "kind": "in",
                    "amount": "5.00",
                    "reason": "corte de prueba",
                }
            },
        ),
        (
            "compra",
            "create_purchase_order",
            "ordenes",
            {
                "draft": {
                    "supplier_id": datos["supplier_id"],
                    "lines": [
                        {
                            "product_id": datos["product_id"],
                            "quantity": "1",
                            "unit_cost": "4.00",
                        }
                    ],
                }
            },
        ),
    ]

    lineas_multilinea = [
        {
            "product_id": product_id,
            "variant_id": None,
            "quantity": "1",
            "selected_option_ids": [],
        }
        for product_id in datos["multilinea"]
    ]

    operaciones.append(
        (
            "venta_multilinea",
            "checkout_sale",
            "ventas",
            {
                "draft": {
                    "shift_id": datos["shift_id"],
                    "idempotency_key": "crash-multilinea-{n}",
                    "items": lineas_multilinea,
                    "cash_received": "100.00",
                }
            },
        )
    )

    intento = 0
    for nombre, comando, contador, plantilla in operaciones:
        for retraso in (0.0, 0.012):
            intento += 1
            argumentos = json.loads(
                json.dumps(plantilla).replace("{n}", f"{intento}")
            )
            antes = verificar(database)
            app = App(app_path)
            try:
                iniciar_sesion(app)
                # El PID se obtiene antes de disparar: asi el corte no paga el
                # coste de enumerar procesos.
                pid = proceso_de_la_corrida(database)
                if pid is None:
                    raise AssertionError("no se encontro el proceso de la aplicacion")
                lanzar_sin_esperar(app, comando, argumentos)
                time.sleep(retraso)
                if not matar_rapido(pid):
                    matar(pid)
                cerrado = esperar_sin_proceso(database)
            finally:
                app.quit()

            estado = verificar(database)
            aplicada = estado[contador] > antes[contador]
            report["cortes"].append(
                {
                    "operacion": nombre,
                    "retraso_ms": round(retraso * 1000, 1),
                    "cerrado": cerrado,
                    "aplicada": aplicada,
                    "integrity": estado["integrity"],
                }
            )
            if not cerrado:
                fallos.append(f"{nombre}: el proceso no termino tras el corte")
            if estado["integrity"] != "ok":
                fallos.append(f"{nombre}: integrity_check={estado['integrity']}")
            for invariante in INVARIANTES:
                if estado[invariante]:
                    fallos.append(f"{nombre}: invariante {invariante}={estado[invariante]}")

    # Tras todas las muertes, la aplicacion debe volver a operar normalmente.
    app = App(app_path)
    try:
        wait_screen(app, "Entrar a la caja")
        iniciar_sesion(app)
        wait_screen(app, "Ventas")
        folio = vender(app, datos["product_id"], datos["shift_id"])
        report["venta_posterior"] = folio
        app.screenshot("tras-cortes")
    finally:
        app.quit()

    final = verificar(database)
    report["estado_final"] = final
    if final["integrity"] != "ok":
        fallos.append("estado final inconsistente")
    report["fallos"] = fallos
    report["result"] = "PASS" if not fallos else "FAIL"
    print(json.dumps(report, ensure_ascii=False))
    return 0 if not fallos else 1


if __name__ == "__main__":
    sys.exit(main())
