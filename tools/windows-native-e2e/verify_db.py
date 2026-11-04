"""Verificación de solo lectura de la base SQLite usada por el E2E nativo.

Comprueba que la venta, el pago, el inventario, la caja y la auditoría quedaron
persistidos exactamente como exige el dominio. Se ejecuta DESPUÉS del E2E y
sobre la base aislada; nunca sobre la instalación real.

Uso:
    python verify_db.py <ruta-de-la-base>
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
}


def q(connection: sqlite3.Connection, sql: str, *params):
    return connection.execute(sql, params).fetchall()


def one(connection: sqlite3.Connection, sql: str, *params):
    rows = q(connection, sql, *params)
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
        migrations = {row["description"] for row in q(connection, "SELECT description FROM _sqlx_migrations")}
        checks["migraciones"] = sorted(migrations)
        missing = sorted(MIGRACIONES_ESPERADAS - migrations)
        if missing:
            failures.append(f"faltan migraciones: {missing}")

        sale = one(connection, "SELECT * FROM sale")
        if sale is None:
            failures.append("no hay ninguna venta persistida")
        else:
            checks["folio"] = sale["folio"]
            checks["importes"] = {
                "subtotal_minor": sale["subtotal_minor"],
                "tax_minor": sale["tax_minor"],
                "total_minor": sale["total_minor"],
            }
            if (sale["subtotal_minor"], sale["tax_minor"], sale["total_minor"]) != (2800, 448, 3248):
                failures.append("los importes de la venta no coinciden con el cálculo de Rust")

            if one(connection, "SELECT COUNT(*) AS n FROM sale WHERE status = 'completed'")["n"] != 1:
                failures.append("se esperaba exactamente una venta confirmada")

            item = one(connection, "SELECT * FROM sale_item WHERE sale_id = ?", sale["id"])
            if item is None:
                failures.append("la venta no tiene partidas")
            else:
                checks["partida"] = {
                    "producto": item["product_name"],
                    "variante": item["variant_name"],
                    "cantidad_milli": item["quantity_milli"],
                    "precio_neto": item["unit_price_minor"],
                    "impuesto": item["unit_tax_minor"],
                    "total_linea": item["line_total_minor"],
                    "modificadores": item["modifiers_summary"],
                }
                if (item["quantity_milli"], item["unit_price_minor"], item["unit_tax_minor"],
                        item["line_total_minor"]) != (2000, 1400, 224, 3248):
                    failures.append("la partida no refleja variante + modificadores + impuesto")
                if item["variant_name"] != "Grande":
                    failures.append("la variante de la partida no es 'Grande'")
                if "Shot extra" not in item["modifiers_summary"]:
                    failures.append("la partida no registra el modificador seleccionado")

            payment = one(connection, "SELECT * FROM sale_payment WHERE sale_id = ?", sale["id"])
            if payment is None:
                failures.append("la venta no tiene pago registrado")
            else:
                checks["pago"] = {
                    "metodo": payment["method"],
                    "recibido_minor": payment["amount_minor"],
                    "cambio_minor": payment["change_minor"],
                }
                if (payment["amount_minor"], payment["change_minor"]) != (5000, 1752):
                    failures.append("el pago o el cambio persistido es incorrecto")

            shift = one(connection, "SELECT * FROM cash_shift WHERE id = ?", sale["shift_id"])
            if shift is None:
                failures.append("la venta no está asociada a un turno")
            else:
                checks["turno"] = {
                    "status": shift["status"],
                    "esperado_minor": shift["expected_amount_minor"],
                    "contado_minor": shift["counted_amount_minor"],
                    "diferencia_minor": shift["difference_minor"],
                }
                if shift["status"] != "closed":
                    failures.append("el turno quedó abierto")
                if shift["expected_amount_minor"] != 13248:
                    failures.append("el saldo esperado del turno no incorpora la venta")
                if shift["counted_amount_minor"] != 13248 or shift["difference_minor"] != 0:
                    failures.append("el arqueo exacto no se persistió como diferencia cero")

            ingreso = one(
                connection,
                "SELECT * FROM cash_movement WHERE shift_id = ? AND kind = 'in' AND amount_minor = 3248",
                sale["shift_id"],
            )
            if ingreso is None:
                failures.append("no existe el movimiento de caja por la venta")

        entries = one(connection, "SELECT COUNT(*) AS n FROM sale")["n"]
        checks["ventas"] = entries

        product = one(connection, "SELECT * FROM product")
        if product is None:
            failures.append("no hay producto persistido")
        else:
            stock = one(
                connection,
                "SELECT COALESCE(SUM(CASE WHEN kind IN ('inbound','adjustment_in') THEN quantity_milli ELSE -quantity_milli END), 0) AS stock FROM inventory_movement WHERE product_id = ?",
                product["id"],
            )
            checks["stock_milli"] = stock["stock"]
            if stock["stock"] != 18000:
                failures.append("el inventario no descontó la venta")

        auditoria = one(
            connection,
            "SELECT COUNT(*) AS n FROM audit_event WHERE action = 'sales.completed'",
        )["n"]
        checks["auditoria_venta"] = auditoria
        if auditoria != 1:
            failures.append("no se registró exactamente un evento de auditoría de venta")

        for action in ("cash.shift_opened", "cash.shift_closed"):
            if one(connection, "SELECT COUNT(*) AS n FROM audit_event WHERE action = ?", action)["n"] < 1:
                failures.append(f"falta auditoría de {action}")
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
