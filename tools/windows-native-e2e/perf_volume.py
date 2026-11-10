"""Rendimiento y estabilidad con volumen representativo.

Complementa `perf_smoke.py` (200 ventas) con un escenario de negocio en
crecimiento: 2 000 productos, 1 000 ventas, tres turnos y respaldo/restauracion
medidos sobre el ejecutable real.

Los umbrales son guardas anchas contra regresiones patologicas, no un SLA. El
informe conserva las mediciones para comparar entre ejecuciones.
"""

import json
import os
import sqlite3
import subprocess
import sys
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path

from e2e_flow import App, bootstrap, wait_screen

PRODUCTOS = 2000
VENTAS = 1000
TURNOS = 3
CICLOS_ESTABILIDAD = 3


def iso(momento: datetime) -> str:
    return momento.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def memoria_mb(database: str) -> float:
    filtro = database.replace("'", "''")
    script = (
        "Get-CimInstance Win32_Process -Filter \"Name = 'pos-desktop.exe'\" | "
        f"Where-Object {{ $_.CommandLine -like '*{filtro}*' }} | "
        "Select-Object -ExpandProperty WorkingSetSize"
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
            return round(int(linea) / (1024 * 1024), 1)
    return 0.0


def medir(app: App, comando: str, argumentos: dict | None = None) -> float:
    inicio = time.perf_counter()
    app.invoke(comando, argumentos) if argumentos else app.invoke(comando)
    return round((time.perf_counter() - inicio) * 1000, 2)


def main() -> int:
    app_path = str(
        Path(
            sys.argv[1]
            if len(sys.argv) > 1
            else "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe"
        ).resolve()
    )
    database = os.environ.get("HIQNET_POS_DB_PATH")
    report: dict = {
        "escenario": "perf-volume",
        "volumen": {"productos": PRODUCTOS, "ventas": VENTAS, "turnos": TURNOS},
    }
    fallos: list[str] = []
    if not database:
        print(json.dumps({"result": "ERROR", "error": "falta HIQNET_POS_DB_PATH"}))
        return 1

    inicio = time.perf_counter()
    app = App(app_path)
    try:
        wait_screen(app, "Configura tu negocio")
        report["startup_s"] = round(time.perf_counter() - inicio, 3)
        bootstrap(app)

        alta = time.perf_counter()
        productos = []
        for indice in range(PRODUCTOS):
            producto = app.invoke(
                "create_product",
                {
                    "draft": {
                        "sku": f"VOL-{indice:05d}",
                        "name": f"Producto sintetico {indice}",
                        "price": "10.00",
                        "tax_rate_bp": 1600,
                        "category_id": None,
                    }
                },
            )
            productos.append(producto["id"])
        report["alta_productos_s"] = round(time.perf_counter() - alta, 3)

        existencias = time.perf_counter()
        for product_id in productos:
            app.invoke(
                "record_inventory_movement",
                {
                    "draft": {
                        "product_id": product_id,
                        "kind": "inbound",
                        "quantity": "1000",
                        "reason": None,
                    }
                },
            )
        report["stock_inicial_s"] = round(time.perf_counter() - existencias, 3)

        turnos = []
        for indice in range(TURNOS):
            caja = app.invoke(
                "create_cash_register",
                {"draft": {"name": f"Caja {indice}", "code": f"VOL-{indice}"}},
            )
            turno = app.invoke(
                "open_cash_shift",
                {"draft": {"register_id": caja["id"], "opening_amount": "500.00"}},
            )
            turnos.append(turno["id"])

        ventas_inicio = time.perf_counter()
        for indice in range(VENTAS):
            app.invoke(
                "checkout_sale",
                {
                    "draft": {
                        "shift_id": turnos[indice % TURNOS],
                        "idempotency_key": f"vol-{indice}",
                        "items": [
                            {
                                "product_id": productos[indice % PRODUCTOS],
                                "variant_id": None,
                                "quantity": "1",
                                "selected_option_ids": [],
                            }
                        ],
                        "cash_received": "20.00",
                    }
                },
            )
        transcurrido = time.perf_counter() - ventas_inicio
        report["checkout_total_s"] = round(transcurrido, 3)
        report["checkout_avg_ms"] = round(transcurrido / VENTAS * 1000, 2)

        ahora = datetime.now(timezone.utc)
        consultas = {
            "list_products": medir(app, "list_products"),
            "stock_levels": medir(app, "list_stock_levels"),
            "list_audit_events": medir(app, "list_audit_events", {"limit": 200}),
            "list_sale_returns": medir(app, "list_sale_returns"),
            "list_cash_shift_history": medir(app, "list_cash_shift_history"),
            "reports": medir(
                app,
                "reports_summary",
                {
                    "query": {
                        "from": iso(ahora - timedelta(days=1)),
                        "to": iso(ahora + timedelta(days=1)),
                    }
                },
            ),
        }
        report["consultas_ms"] = consultas
        report["memoria_mb"] = memoria_mb(database)

        respaldo_inicio = time.perf_counter()
        respaldo = Path(app.invoke("backup_database"))
        report["respaldo_s"] = round(time.perf_counter() - respaldo_inicio, 3)
        report["respaldo_bytes"] = respaldo.stat().st_size if respaldo.is_file() else 0

        cierre_inicio = time.perf_counter()
        app.invoke(
            "close_cash_shift",
            {
                "draft": {
                    "shift_id": turnos[0],
                    "counted_amount": "1000.00",
                }
            },
        )
        report["cierre_turno_s"] = round(time.perf_counter() - cierre_inicio, 3)
    finally:
        app.quit()

    # Estabilidad: ciclos de arranque con el volumen ya cargado.
    ciclos = []
    for _ in range(CICLOS_ESTABILIDAD):
        arranque = time.perf_counter()
        app = App(app_path)
        try:
            wait_screen(app, "Entrar a la caja")
            ciclos.append(
                {
                    "arranque_s": round(time.perf_counter() - arranque, 3),
                    "memoria_mb": memoria_mb(database),
                }
            )
        finally:
            app.quit()
    report["ciclos"] = ciclos

    # Integridad y tamano en disco tras el volumen.
    conexion = sqlite3.connect(database, isolation_level=None)
    try:
        conexion.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()
        report["integrity"] = conexion.execute("PRAGMA integrity_check").fetchone()[0]
        report["foreign_key_violations"] = len(
            conexion.execute("PRAGMA foreign_key_check").fetchall()
        )
        report["filas"] = {
            tabla: conexion.execute(f"SELECT COUNT(*) FROM {tabla}").fetchone()[0]
            for tabla in ("product", "sale", "sale_item", "cash_movement", "audit_event")
        }
    finally:
        conexion.close()
    report["db_bytes"] = Path(database).stat().st_size

    # Restauracion sobre el mismo binario (modo utilidad) medida en frio.
    if respaldo.is_file():
        restauracion = time.perf_counter()
        resultado = subprocess.run(
            [app_path, "--restore-backup", str(respaldo), "--database", database],
            capture_output=True,
            text=True,
            timeout=300,
            check=False,
        )
        report["restauracion_s"] = round(time.perf_counter() - restauracion, 3)
        report["restauracion_codigo"] = resultado.returncode
        if resultado.returncode != 0:
            fallos.append("la restauracion de control fallo")

    umbrales = {
        "startup_s": 20.0,
        "checkout_avg_ms": 100.0,
        "consultas_ms.list_products": 3000.0,
        "consultas_ms.reports": 3000.0,
        "respaldo_s": 30.0,
        "restauracion_s": 60.0,
    }
    for clave, limite in umbrales.items():
        actual = report
        for parte in clave.split("."):
            actual = actual.get(parte, {}) if isinstance(actual, dict) else {}
        if isinstance(actual, (int, float)) and actual > limite:
            fallos.append(f"{clave}={actual} supera la guarda {limite}")
    if report["integrity"] != "ok":
        fallos.append(f"integrity_check={report['integrity']}")
    if report["foreign_key_violations"]:
        fallos.append("violaciones de clave foranea tras el volumen")

    arranques = [c["arranque_s"] for c in ciclos]
    memorias = [c["memoria_mb"] for c in ciclos]
    if arranques and arranques[-1] > arranques[0] * 2 + 2:
        fallos.append(f"el arranque se degrada entre ciclos: {arranques}")
    if memorias and memorias[-1] > memorias[0] * 1.5 + 50:
        fallos.append(f"la memoria crece entre ciclos: {memorias}")

    report["fallos"] = fallos
    report["result"] = "PASS" if not fallos else "FAIL"
    print(json.dumps(report, ensure_ascii=False))
    return 0 if not fallos else 1


if __name__ == "__main__":
    sys.exit(main())
