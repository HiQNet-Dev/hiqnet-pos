"""Resiliencia de la restauración ante respaldos inválidos o interrumpidos.

Se ejecuta sobre una base SQLite aislada. No usa el puente Tauri simulado: la
restauración se invoca con el binario real en modo utilidad, igual que lo haría
un operador.

Escenarios:
    - respaldo truncado
    - respaldo vacío
    - respaldo inexistente
    - archivo que no es SQLite
    - respaldo con bytes alterados (hash/contenido incorrecto)
    - sidecars inconsistentes junto al destino
    - interrupción abrupta de la restauración

En todos los casos la base operativa debe conservarse intacta y verificable.
"""

import hashlib
import json
import os
import shutil
import sqlite3
import subprocess
import sys
import tempfile
import time
from pathlib import Path

from e2e_flow import App, bootstrap


def ejecutar(app_path: str, argumentos: list[str], timeout: float = 120) -> subprocess.CompletedProcess:
    return subprocess.run(
        [app_path, *argumentos],
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )


def checkpoint(database: str) -> None:
    connection = sqlite3.connect(database, isolation_level=None)
    try:
        connection.execute("PRAGMA wal_checkpoint(TRUNCATE)").fetchone()
    finally:
        connection.close()


def huella(database: str) -> str:
    """Hash del archivo principal con el WAL ya integrado."""
    checkpoint(database)
    digest = hashlib.sha256()
    with open(database, "rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def estado(database: str) -> dict:
    connection = sqlite3.connect(f"file:{database}?mode=ro", uri=True)
    try:
        integrity = connection.execute("PRAGMA integrity_check").fetchone()[0]
        productos = connection.execute("SELECT COUNT(*) FROM product").fetchone()[0]
        auditoria = connection.execute("SELECT COUNT(*) FROM audit_event").fetchone()[0]
        return {"integrity": integrity, "productos": productos, "auditoria": auditoria}
    finally:
        connection.close()


def variantes(carpeta: Path, respaldo: Path) -> dict[str, Path]:
    datos = respaldo.read_bytes()
    salida = {}

    truncado = carpeta / "truncado.db"
    truncado.write_bytes(datos[: max(4096, len(datos) // 3)])
    salida["truncado"] = truncado

    vacio = carpeta / "vacio.db"
    vacio.write_bytes(b"")
    salida["vacio"] = vacio

    salida["inexistente"] = carpeta / "no-existe.db"

    texto = carpeta / "no-sqlite.db"
    texto.write_bytes(b"esto no es una base de datos\n" * 10)
    salida["no_sqlite"] = texto

    alterado = carpeta / "alterado.db"
    alterado.write_bytes(datos)
    with open(alterado, "r+b") as handle:
        # Se corrompe una pagina intermedia manteniendo el tamano y la cabecera.
        handle.seek(max(4096, len(datos) // 2))
        previo = handle.read(1)
        handle.seek(max(4096, len(datos) // 2))
        handle.write(bytes([previo[0] ^ 0xFF]))
    salida["alterado"] = alterado
    return salida


def main() -> int:
    app_path = sys.argv[1] if len(sys.argv) > 1 else ""
    database = os.environ.get("HIQNET_POS_DB_PATH")
    report: dict = {"escenario": "restore-corruption", "casos": {}}
    if not app_path or not database:
        print(json.dumps({"result": "ERROR", "error": "faltan ejecutable o base"}))
        return 1

    app = App(app_path)
    try:
        bootstrap(app)
        app.invoke(
            "create_product",
            {
                "draft": {
                    "sku": "COR-001",
                    "name": "Producto canario",
                    "price": "10.00",
                    "tax_rate_bp": 0,
                    "category_id": None,
                }
            },
        )
        respaldo_valido = Path(app.invoke("backup_database"))
        if not respaldo_valido.is_file():
            raise AssertionError("la aplicacion no creo el respaldo")
    finally:
        app.quit()

    base_antes = huella(database)
    estado_antes = estado(database)

    carpeta = Path(tempfile.mkdtemp(prefix="hiqnet-corrupt-"))
    fallos = []
    try:
        casos = variantes(carpeta, respaldo_valido)
        for nombre, archivo in casos.items():
            verificacion = ejecutar(app_path, ["--verify-backup", str(archivo)])
            restauracion = ejecutar(
                app_path, ["--restore-backup", str(archivo), "--database", database]
            )
            intacta = huella(database) == base_antes
            report["casos"][nombre] = {
                "verify_codigo": verificacion.returncode,
                "restore_codigo": restauracion.returncode,
                "base_intacta": intacta,
                "respuesta": (restauracion.stdout + restauracion.stderr).strip()[:200],
            }
            if verificacion.returncode == 0:
                fallos.append(f"{nombre}: --verify-backup acepto un respaldo invalido")
            if restauracion.returncode == 0:
                fallos.append(f"{nombre}: --restore-backup acepto un respaldo invalido")
            if not intacta:
                fallos.append(f"{nombre}: la base cambio tras un intento rechazado")

        # Sidecars inconsistentes junto al destino: la restauracion no debe
        # aplicarse sobre un estado dudoso ni dejar la base rota.
        wal_falso = Path(database + "-wal")
        wal_falso.write_bytes(b"wal inconsistente de prueba" * 64)
        con_sidecar = ejecutar(
            app_path, ["--restore-backup", str(respaldo_valido), "--database", database]
        )
        report["casos"]["sidecar_inconsistente"] = {
            "restore_codigo": con_sidecar.returncode,
            "respuesta": (con_sidecar.stdout + con_sidecar.stderr).strip()[:200],
            "estado": estado(database),
        }
        if estado(database)["integrity"] != "ok":
            fallos.append("sidecar_inconsistente: la base quedo corrupta")

        # Interrupcion abrupta durante la restauracion: se mide cuanto tarda una
        # restauracion real y se corta el proceso en fracciones de ese tiempo,
        # que es la unica forma de alcanzar la ventana de copia y promocion.
        inicio = time.monotonic()
        referencia = ejecutar(
            app_path, ["--restore-backup", str(respaldo_valido), "--database", database]
        )
        duracion = time.monotonic() - inicio
        report["duracion_restauracion_s"] = round(duracion, 4)
        if referencia.returncode != 0:
            fallos.append("la restauracion de referencia fallo")

        interrupciones = []
        for intento, fraccion in enumerate((0.15, 0.3, 0.5, 0.7, 0.9), start=1):
            limite = max(0.005, duracion * fraccion)
            cortado = False
            try:
                ejecutar(
                    app_path,
                    ["--restore-backup", str(respaldo_valido), "--database", database],
                    timeout=limite,
                )
            except subprocess.TimeoutExpired:
                cortado = True
            actual = estado(database)
            interrupciones.append(
                {
                    "intento": intento,
                    "limite_ms": round(limite * 1000, 1),
                    "cortado": cortado,
                    "integrity": actual["integrity"],
                }
            )
            if actual["integrity"] != "ok":
                fallos.append(f"interrupcion {intento}: integrity={actual['integrity']}")
                break
        report["interrupciones"] = interrupciones
        if not any(item["cortado"] for item in interrupciones):
            fallos.append("ninguna interrupcion alcanzo la ventana de restauracion")

        # Tras las interrupciones la aplicacion debe seguir abriendo.
        recuperada = App(app_path)
        try:
            app_title = recuperada.driver.title
            report["reapertura_tras_interrupciones"] = {"titulo": app_title}
        finally:
            recuperada.quit()

        # Al final una restauracion valida debe seguir funcionando.
        final = ejecutar(
            app_path, ["--restore-backup", str(respaldo_valido), "--database", database]
        )
        report["restauracion_final"] = {
            "codigo": final.returncode,
            "estado": estado(database),
        }
        if final.returncode != 0:
            fallos.append("la restauracion valida dejo de funcionar tras los rechazos")
        if estado(database)["productos"] < 1:
            fallos.append("la restauracion final no dejo el estado respaldado")
    finally:
        shutil.rmtree(carpeta, ignore_errors=True)

    report["estado_previo"] = estado_antes
    report["fallos"] = fallos
    report["result"] = "PASS" if not fallos else "FAIL"
    print(json.dumps(report, ensure_ascii=False))
    return 0 if not fallos else 1


if __name__ == "__main__":
    sys.exit(main())
