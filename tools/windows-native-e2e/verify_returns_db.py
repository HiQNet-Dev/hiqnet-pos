"""Verificación de solo lectura de la base del escenario de devoluciones.

Comprueba la venta, la devolución, el reembolso en caja, la restitución de
inventario y la auditoría. Nunca se ejecuta sobre la base real.

Uso:
    python verify_returns_db.py <ruta-de-la-base>
"""

import json
import sqlite3
import sys
from pathlib import Path

MIGRACIONES_ESPERADAS = {
    "init",
    "inventory",
    "inventory adjustments",
    "catalog categories variants",
    "catalog modifiers",
    "cash registers shifts",
    "sales checkout",
    "purchasing",
    "expenses waste",
    "sale returns",
}


def one(connection: sqlite3.Connection, sql: str, *params):
    rows = connection.execute(sql, params).fetchall()
    return rows[0] if rows else None


def main() -> int:
    if len(sys.argv) < 2:
        print(json.dumps({"result": "FAIL", "error": "falta la ruta de la base"}))
        return 1
    path = Path(sys.argv[1])
    if not path.exists():
        print(json.dumps({"result": "FAIL", "error": f"no existe la base {path}"}))
        return 1

    checks: dict[str, object] = {}
    failures: list[str] = []
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    connection.row_factory = sqlite3.Row
    try:
        migrations = {
            row["description"] for row in connection.execute("SELECT description FROM _sqlx_migrations")
        }
        checks["migraciones"] = sorted(migrations)
        missing = sorted(MIGRACIONES_ESPERADAS - migrations)
        if missing:
            failures.append(f"faltan migraciones: {missing}")

        sale = one(connection, "SELECT * FROM sale")
        if sale is None:
            failures.append("no hay venta persistida")
        else:
            checks["folio"] = sale["folio"]
            if (sale["subtotal_minor"], sale["tax_minor"], sale["total_minor"]) != (2000, 320, 2320):
                failures.append("los importes de la venta no coinciden")

            returns = connection.execute(
                "SELECT * FROM sale_return WHERE sale_id = ?", (sale["id"],)
            ).fetchall()
            checks["devoluciones"] = len(returns)
            if len(returns) != 1:
                failures.append("se esperaba exactamente una devolución")
            else:
                refund = returns[0]
                checks["reembolso_minor"] = refund["refund_total_minor"]
                if refund["refund_total_minor"] != 1160:
                    failures.append("el reembolso no es el exacto de una unidad")
                lines = connection.execute(
                    "SELECT * FROM sale_return_line WHERE return_id = ?", (refund["id"],)
                ).fetchall()
                if len(lines) != 1:
                    failures.append("se esperaba una partida devuelta")
                else:
                    line = lines[0]
                    checks["partida"] = {
                        "cantidad_milli": line["quantity_milli"],
                        "destino": line["disposition"],
                        "neto": line["refund_net_minor"],
                        "impuesto": line["refund_tax_minor"],
                        "total": line["refund_total_minor"],
                        "inbound": line["inbound_movement_id"] is not None,
                    }
                    if (
                        line["quantity_milli"],
                        line["disposition"],
                        line["refund_net_minor"],
                        line["refund_tax_minor"],
                        line["refund_total_minor"],
                    ) != (1000, "restock", 1000, 160, 1160):
                        failures.append("la partida devuelta no cuadra con los snapshots")
                    if line["inbound_movement_id"] is None:
                        failures.append("la devolución reintegrable no creó su inbound")

                cash_out = connection.execute(
                    "SELECT * FROM cash_movement WHERE id = ? AND kind = 'out'",
                    (refund["cash_movement_id"],),
                ).fetchone()
                if cash_out is None or cash_out["amount_minor"] != 1160:
                    failures.append("no existe una única salida de caja por el reembolso")
                if connection.execute(
                    "SELECT COUNT(*) AS n FROM cash_movement WHERE kind = 'out'"
                ).fetchone()["n"] != 1:
                    failures.append("hay salidas de caja duplicadas")

        shift = one(connection, "SELECT * FROM cash_shift")
        if shift is None:
            failures.append("no hay turno persistido")
        elif shift["status"] != "open" or shift["expected_amount_minor"] is not None:
            # Un turno abierto no persiste el esperado hasta el cierre.
            if shift["status"] != "open":
                failures.append("el turno debía seguir abierto")
        stock = one(
            connection,
            "SELECT COALESCE(SUM(CASE WHEN kind IN ('inbound','adjustment_in') THEN quantity_milli ELSE -quantity_milli END), 0) AS stock FROM inventory_movement",
        )["stock"]
        checks["stock_milli"] = stock
        if stock != 19000:
            failures.append("el inventario no refleja venta y devolución reintegrable")

        if connection.execute(
            "SELECT COUNT(*) AS n FROM audit_event WHERE action = 'sales.returned'"
        ).fetchone()["n"] != 1:
            failures.append("falta el evento de auditoría de la devolución")
        if connection.execute(
            "SELECT COUNT(*) AS n FROM audit_event WHERE action = 'sales.completed'"
        ).fetchone()["n"] != 1:
            failures.append("falta el evento de auditoría de la venta")

        integrity = one(connection, "PRAGMA integrity_check")[0]
        if integrity != "ok":
            failures.append(f"integrity_check devolvió {integrity}")
        if connection.execute("PRAGMA foreign_key_check").fetchall():
            failures.append("foreign_key_check encontró referencias inválidas")
    finally:
        connection.close()

    result = {
        "result": "PASS" if not failures else "FAIL",
        "checks": checks,
        "failures": failures,
    }
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
