"""Comprobación de conservación de datos al respaldar y restaurar.

Toma una base ya existente (por ejemplo, la que deja `e2e_flow.py` con
`-KeepArtifacts`), crea un snapshot con `VACUUM INTO`, restaura con la utilidad
real del ejecutable sobre una copia con WAL y compara el **contenido exacto** de
cada módulo mediante un hash determinista por tabla.

También escribe un producto posterior al snapshot para demostrar que la
restauración lo deshace y que ninguna fila anterior cambia.

Uso:

    python tools/windows-native-e2e/restore_multiphase_check.py <base.db> [ejecutable]

La base de origen nunca se modifica: solo se lee y se copia.
"""

import hashlib
import json
import shutil
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path

# Tablas de negocio, ordenadas por módulo para que el informe sea legible.
TABLES = [
    "organization",
    "branch",
    "app_user",
    "product",
    "product_variant",
    "category",
    "modifier_group",
    "modifier_option",
    "sale",
    "sale_item",
    "sale_payment",
    "sale_return",
    "sale_return_line",
    "cash_register",
    "cash_shift",
    "cash_movement",
    "supplier",
    "purchase_order",
    "purchase_order_line",
    "purchase_receipt",
    "purchase_receipt_line",
    "expense_category",
    "expense",
    "waste_record",
    "inventory_movement",
    "audit_event",
    "local_installation_state",
]

CANARY = (
    "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, "
    "active, created_at) SELECT 'restore-canary', id, 'CANARY-1', 'Cambio posterior', 100, 0, 1, "
    "'2026-01-01T00:00:00Z' FROM organization LIMIT 1"
)


def connect(path: Path) -> sqlite3.Connection:
    return sqlite3.connect(f"file:{path}?mode=ro", uri=True)


def table_digest(connection: sqlite3.Connection, table: str) -> dict:
    """Cuenta y hash del contenido completo, independiente del orden de filas."""
    rows = [list(row) for row in connection.execute(f"SELECT * FROM {table}")]
    rows.sort(key=lambda row: json.dumps(row, default=str, sort_keys=True))
    payload = json.dumps(rows, default=str, sort_keys=True, separators=(",", ":"))
    return {
        "filas": len(rows),
        "sha256": hashlib.sha256(payload.encode("utf-8")).hexdigest(),
    }


def snapshot_report(path: Path) -> dict:
    connection = connect(path)
    try:
        tables = {table: table_digest(connection, table) for table in TABLES}
        integrity = connection.execute("PRAGMA integrity_check").fetchone()[0]
        foreign_keys = [list(row) for row in connection.execute("PRAGMA foreign_key_check")]
        migraciones = [
            row[0]
            for row in connection.execute("SELECT version FROM _sqlx_migrations ORDER BY version")
        ]
        return {
            "tables": tables,
            "integrity_check": integrity,
            "foreign_key_violations": len(foreign_keys),
            "migraciones": migraciones,
        }
    finally:
        connection.close()


def main() -> int:
    if len(sys.argv) < 2:
        print(json.dumps({"result": "ERROR", "error": "indica la base de origen"}))
        return 1
    source = Path(sys.argv[1]).resolve()
    app = Path(
        sys.argv[2]
        if len(sys.argv) > 2
        else "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe"
    ).resolve()
    if not source.is_file() or not app.is_file():
        print(json.dumps({"result": "ERROR", "error": "falta la base o el ejecutable"}))
        return 1

    workspace = Path(tempfile.mkdtemp(prefix="hiqnet-restore-check-"))
    report: dict = {"origen": str(source), "workspace": str(workspace)}
    try:
        snapshot = workspace / "snapshot.db"
        connection = sqlite3.connect(source)
        try:
            connection.execute("VACUUM INTO ?", (str(snapshot),))
        finally:
            connection.close()

        destination = workspace / "destino" / "hiqnet-pos.db"
        destination.parent.mkdir(parents=True)
        # Copia con WAL/SHM: reproduce el estado real de una base en operación.
        for suffix in ("", "-wal", "-shm"):
            side = Path(str(source) + suffix)
            if side.exists():
                shutil.copy2(side, Path(str(destination) + suffix))

        expected = snapshot_report(snapshot)
        before = snapshot_report(destination)

        writer = sqlite3.connect(destination)
        try:
            writer.execute(CANARY)
            writer.commit()
        finally:
            writer.close()
        with_canary = snapshot_report(destination)

        verify = subprocess.run(
            [str(app), "--verify-backup", str(snapshot)],
            capture_output=True,
            text=True,
            check=False,
        )
        restore = subprocess.run(
            [str(app), "--restore-backup", str(snapshot), "--database", str(destination)],
            capture_output=True,
            text=True,
            check=False,
        )

        after = snapshot_report(destination)
        recovery = sorted(item.name for item in destination.parent.glob("*.pre-restore-*"))
        differing = [
            table
            for table in TABLES
            if before["tables"][table] != after["tables"][table]
            or expected["tables"][table] != after["tables"][table]
        ]

        report.update(
            {
                "verify": verify.returncode,
                "restore": restore.returncode,
                "salida_restauracion": (restore.stdout + restore.stderr).strip()[:300],
                "copias_recuperacion": recovery,
                "migraciones": expected["migraciones"],
                "contenido_igual_al_snapshot": after["tables"] == expected["tables"],
                "tablas_distintas": differing,
                "canario_visible_antes": with_canary["tables"]["product"]
                != before["tables"]["product"],
                "canario_eliminado": after["tables"]["product"] == expected["tables"]["product"],
                "integrity_check": after["integrity_check"],
                "foreign_key_violations": after["foreign_key_violations"],
                "filas_por_tabla": {
                    table: expected["tables"][table]["filas"] for table in TABLES
                },
            }
        )
        ok = (
            verify.returncode == 0
            and restore.returncode == 0
            and after["tables"] == expected["tables"]
            and before["tables"] == expected["tables"]
            and not differing
            and bool(recovery)
            and report["canario_visible_antes"]
            and report["canario_eliminado"]
            and after["integrity_check"] == "ok"
            and after["foreign_key_violations"] == 0
        )
        report["result"] = "PASS" if ok else "FAIL"
        print(json.dumps(report, ensure_ascii=False))
        return 0 if ok else 1
    finally:
        shutil.rmtree(workspace, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
