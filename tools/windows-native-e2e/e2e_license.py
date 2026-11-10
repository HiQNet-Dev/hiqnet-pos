"""Licencia firmada sobre la aplicación Tauri real.

Requiere que el ejecutable se haya iniciado con
`HIQNET_POS_LICENSE_PUBLIC_KEY` (compilación de depuración) apuntando a la
clave pública de la semilla de pruebas. La semilla se pasa al firmante de
ejemplo por `POS_LICENSE_SEED_B64` y no vive en el repositorio.

Recorre: trial sin licencia, instalación de un grant válido, operaciones
permitidas por sus entitlements, persistencia tras reiniciar, rechazo de una
firma manipulada y licencia vencida.
"""

import json
import os
import sqlite3
import subprocess
import sys
from pathlib import Path

from e2e_flow import App, bootstrap, iniciar_sesion, wait_screen

SEED_B64 = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8="
REPO = Path(__file__).resolve().parents[2]
USD = "contrasena-segura"


def identificadores(database: str) -> tuple[str, str]:
    conexion = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    try:
        fila = conexion.execute(
            "SELECT installation_id, organization_id FROM local_installation_state WHERE id = 1"
        ).fetchone()
        return fila[0], fila[1]
    finally:
        conexion.close()


def firmar(installation: str, organization: str, plan: str, features: str, dias: str) -> dict:
    entorno = dict(os.environ)
    entorno["POS_LICENSE_SEED_B64"] = SEED_B64
    salida = subprocess.run(
        [
            "cargo",
            "run",
            "-q",
            "-p",
            "pos-licensing",
            "--example",
            "sign_grant",
            "--",
            installation,
            organization,
            plan,
            features,
            dias,
        ],
        cwd=REPO / "backend-rust",
        capture_output=True,
        text=True,
        timeout=300,
        env=entorno,
        check=False,
    )
    if salida.returncode != 0:
        raise AssertionError(f"no se pudo firmar el grant: {salida.stderr[-400:]}")
    return json.loads(salida.stdout.strip().splitlines()[-1])


def estado(app: App) -> dict:
    return app.invoke("app_status")["commercial"] or {}


def main() -> int:
    app_path = sys.argv[1] if len(sys.argv) > 1 else ""
    database = os.environ.get("HIQNET_POS_DB_PATH")
    report: dict = {"escenario": "license"}
    fallos: list[str] = []
    if not app_path or not database:
        print(json.dumps({"result": "ERROR", "error": "faltan ejecutable o base"}))
        return 1

    app = App(app_path)
    try:
        bootstrap(app)
        trial = estado(app)
        report["trial"] = trial
        if trial.get("status") != "trial" or trial.get("read_only"):
            fallos.append(f"el trial inicial no es operable: {trial}")
        app.invoke(
            "create_product",
            {"draft": {"sku": "LIC-E2E", "name": "Producto trial", "price": "10.00", "tax_rate_bp": 0, "category_id": None}},
        )
    finally:
        app.quit()

    installation, organization = identificadores(database)
    grant = firmar(installation, organization, "pro", "catalog,sales", "30")

    app = App(app_path)
    try:
        iniciar_sesion(app)
        vista = app.invoke(
            "install_license",
            {"grant_json": grant["grant"], "signature_b64": grant["signature"]},
        )
        report["instalada"] = vista
        if vista["plan"] != "pro" or vista["read_only"]:
            fallos.append(f"la licencia valida no se aplico: {vista}")
        if "expenses" in vista["features"]:
            fallos.append("la licencia concedio una feature no firmada")

        app.invoke(
            "create_product",
            {"draft": {"sku": "LIC-E2E-2", "name": "Producto pro", "price": "10.00", "tax_rate_bp": 0, "category_id": None}},
        )
        denegado = app.invoke_raw(
            "record_inventory_movement",
            {
                "draft": {
                    "product_id": "00000000-0000-0000-0000-000000000000",
                    "kind": "inbound",
                    "quantity": "1",
                    "reason": None,
                }
            },
        )
        report["feature_no_contratada"] = denegado.get("e")
        if denegado.get("ok"):
            fallos.append("una feature no contratada fue aceptada")
        audit = app.invoke("list_audit_events", {"limit": 50})
        if not any(evento["action"] == "license.installed" for evento in audit):
            fallos.append("no se audito la instalacion de la licencia")
    finally:
        app.quit()

    # Persistencia tras reiniciar.
    app = App(app_path)
    try:
        iniciar_sesion(app)
        persistida = estado(app)
        report["tras_reinicio"] = persistida
        if persistida.get("plan") != "pro" or persistida.get("read_only"):
            fallos.append(f"la licencia no persistio: {persistida}")
    finally:
        app.quit()

    # Firma manipulada: solo lectura.
    conexion = sqlite3.connect(database)
    try:
        conexion.execute(
            "UPDATE local_installation_state SET license_signature_b64 = 'ZmlybWEtZmFsc2E=' WHERE id = 1"
        )
        conexion.commit()
    finally:
        conexion.close()

    app = App(app_path)
    try:
        wait_screen(app, "Entrar a la caja")
        iniciar_sesion(app)
        alterada = estado(app)
        report["firma_alterada"] = alterada
        if not alterada.get("read_only"):
            fallos.append(f"una firma manipulada no dejo solo lectura: {alterada}")
        rechazo = app.invoke_raw(
            "create_product",
            {"draft": {"sku": "LIC-TAMPER", "name": "No debe crearse", "price": "1.00", "tax_rate_bp": 0, "category_id": None}},
        )
        report["mutacion_con_firma_invalida"] = rechazo.get("e")
        if rechazo.get("ok"):
            fallos.append("se permitio mutar con una firma invalida")
    finally:
        app.quit()

    # Licencia vencida: solo lectura.
    app = App(app_path)
    try:
        iniciar_sesion(app)
        vencido = firmar(installation, organization, "pro", "catalog,sales", "-1")
        vista = app.invoke(
            "install_license",
            {"grant_json": vencido["grant"], "signature_b64": vencido["signature"]},
        )
        report["vencida"] = vista
        if not vista["read_only"]:
            fallos.append(f"una licencia vencida no dejo solo lectura: {vista}")
    finally:
        app.quit()

    report["fallos"] = fallos
    report["result"] = "PASS" if not fallos else "FAIL"
    print(json.dumps(report, ensure_ascii=False))
    return 0 if not fallos else 1


if __name__ == "__main__":
    sys.exit(main())
