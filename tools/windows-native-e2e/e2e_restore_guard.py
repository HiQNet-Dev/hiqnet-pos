"""Verifica en proceso real que la restauración no reemplaza una base en uso.

Se ejecuta en una base SQLite aislada (la crea `run-e2e.ps1`) y demuestra:

    bootstrap -> respaldo desde la aplicación -> intento de restauración con la
    aplicación abierta (debe rechazarse) -> cierre -> verificación y restauración
    correctas -> reapertura con los datos intactos

La restauración se invoca como lo haría un operador: ejecutando el mismo
binario en modo utilidad, nunca a través de mocks del puente Tauri.
"""

import json
import os
import sqlite3
import subprocess
import sys
from pathlib import Path

from e2e_flow import App, bootstrap, wait_screen

USUARIO = "propietario"
CLAVE = "contrasena-segura"


def ejecutar_utilidad(app_path: str, argumentos: list[str]) -> subprocess.CompletedProcess:
    return subprocess.run(
        [app_path, *argumentos],
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )


def iniciar_sesion(app: App):
    wait_screen(app, "Entrar a la caja")
    app.fill("//section", "Usuario", USUARIO)
    app.fill("//section", "Contraseña", CLAVE)
    app.click_xpath("//button[normalize-space()='Entrar']")
    wait_screen(app, "Ventas")


def productos_en_disco(database: str) -> int:
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    try:
        return connection.execute("SELECT COUNT(*) FROM product").fetchone()[0]
    finally:
        connection.close()


def main() -> int:
    app_path = sys.argv[1] if len(sys.argv) > 1 else ""
    database = os.environ.get("HIQNET_POS_DB_PATH")
    report: dict = {"escenario": "restore-guard"}
    if not app_path or not database:
        print(json.dumps({"result": "ERROR", "error": "faltan ejecutable o base"}, ensure_ascii=False))
        return 1

    app = App(app_path)
    try:
        bootstrap(app)
        app.invoke(
            "create_product",
            {
                "draft": {
                    "sku": "RES-001",
                    "name": "Producto respaldo",
                    "price": "10.00",
                    "tax_rate_bp": 0,
                    "category_id": None,
                }
            },
        )
        respaldo = app.invoke("backup_database")
        report["respaldo"] = respaldo
        if not Path(respaldo).is_file():
            raise AssertionError("la aplicación no creó el archivo de respaldo")
        app.screenshot("gestion-respaldo")

        # La aplicación sigue abierta: la restauración debe rechazarse.
        bloqueado = ejecutar_utilidad(
            app_path, ["--restore-backup", respaldo, "--database", database]
        )
        report["intento_con_app_abierta"] = {
            "codigo": bloqueado.returncode,
            "salida": (bloqueado.stdout + bloqueado.stderr).strip()[:400],
        }
        if bloqueado.returncode != 2:
            raise AssertionError(
                "la restauración con la aplicación abierta no fue rechazada: "
                f"código {bloqueado.returncode}"
            )
        if "cierra HiQNet POS" not in (bloqueado.stdout + bloqueado.stderr):
            raise AssertionError("el rechazo no explica que hay que cerrar la aplicación")

        # La base en uso permanece intacta después del intento.
        if len(app.invoke("list_products")) != 1:
            raise AssertionError("el rechazo modificó la base en uso")
        app.screenshot("ventas-tras-rechazo")

        app.quit()
        app = None

        # Con la aplicación cerrada, verificación y restauración funcionan.
        verificacion = ejecutar_utilidad(app_path, ["--verify-backup", respaldo])
        if verificacion.returncode != 0:
            raise AssertionError(
                "la verificación del respaldo falló: " + (verificacion.stderr or "").strip()
            )

        restauracion = ejecutar_utilidad(
            app_path, ["--restore-backup", respaldo, "--database", database]
        )
        if restauracion.returncode != 0:
            raise AssertionError(
                "la restauración con la aplicación cerrada falló: "
                + (restauracion.stderr or "").strip()
            )
        report["restauracion"] = {
            "codigo": restauracion.returncode,
            "salida": restauracion.stdout.strip()[:400],
        }
        report["productos_tras_restaurar"] = productos_en_disco(database)
        if report["productos_tras_restaurar"] != 1:
            raise AssertionError("la base restaurada no conserva el producto respaldado")
        copias = [name for name in os.listdir(Path(database).parent) if ".pre-restore-" in name]
        report["copias_recuperacion"] = copias
        if not copias:
            raise AssertionError("no se creó una copia de recuperación")

        # La aplicación vuelve a abrir la base restaurada.
        app = App(app_path)
        iniciar_sesion(app)
        nombres = [item["name"] for item in app.invoke("list_products")]
        if "Producto respaldo" not in nombres:
            raise AssertionError("la reapertura no muestra el producto restaurado")
        report["reapertura"] = nombres
        app.screenshot("ventas-tras-restaurar")

        report["result"] = "PASS"
        print(json.dumps(report, ensure_ascii=False))
        return 0
    except Exception as error:  # noqa: BLE001
        report.update({"result": "FAIL", "error": str(error)})
        print(json.dumps(report, ensure_ascii=False))
        return 1
    finally:
        if app is not None:
            app.quit()
        directory = os.environ.get("HIQNET_E2E_ARTIFACT_DIR")
        if directory:
            try:
                (Path(directory) / "restore-guard-report.json").write_text(
                    json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
                )
            except Exception:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
