"""Comprobación de conservación de datos al respaldar y restaurar.

Toma una base ya existente (por ejemplo, la que deja `e2e_flow.py` con
`-KeepArtifacts`), crea un snapshot con `VACUUM INTO`, restaura con la utilidad
real del ejecutable sobre una copia con WAL y compara los registros de cada
módulo. También escribe un producto posterior al snapshot para demostrar que la
restauración lo deshace.

Uso:

    python tools/windows-native-e2e/restore_multiphase_check.py <base.db> [ejecutable]

La base de origen nunca se modifica: solo se lee y se copia.
"""

import json
import shutil
import sqlite3
import subprocess
import sys
import tempfile
from pathlib import Path

TABLES = [
    "organization",
    "app_user",
    "product",
    "sale",
    "sale_return",
    "cash_shift",
    "cash_movement",
    "supplier",
    "purchase_order",
    "purchase_receipt",
    "expense",
    "waste_record",
    "inventory_movement",
    "audit_event",
]

CANARY = (
    "INSERT INTO product (id, organization_id, sku, name, unit_price_minor, tax_rate_bp, "
    "active, created_at) SELECT 'restore-canary', id, 'CANARY-1', 'Cambio posterior', 100, 0, 1, "
    "'2026-01-01T00:00:00Z' FROM organization LIMIT 1"
)


def counts(path: Path) -> dict[str, int]:
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    try:
        return {
            table: connection.execute(f"SELECT COUNT(*) FROM {table}").fetchone()[0]
            for table in TABLES
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

        before = counts(destination)
        writer = sqlite3.connect(destination)
        try:
            writer.execute(CANARY)
            writer.commit()
        finally:
            writer.close()

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

        after = counts(destination)
        expected = counts(snapshot)
        recovery = sorted(item.name for item in destination.parent.glob("*.pre-restore-*"))

        report.update(
            {
                "verify": verify.returncode,
                "restore": restore.returncode,
                "salida_restauracion": (restore.stdout + restore.stderr).strip()[:300],
                "copias_recuperacion": recovery,
                "antes": before,
                "despues": after,
                "snapshot": expected,
                "conserva_todas_las_fases": after == expected,
                "canario_revertido": after["product"] == expected["product"],
            }
        )
        ok = (
            verify.returncode == 0
            and restore.returncode == 0
            and after == expected
            and bool(recovery)
            and after["product"] == expected["product"]
        )
        report["result"] = "PASS" if ok else "FAIL"
        print(json.dumps(report, ensure_ascii=False))
        return 0 if ok else 1
    finally:
        shutil.rmtree(workspace, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
