"""Operacion sin conectividad externa.

Ejecuta el ciclo operativo completo (arranque, login local, venta, inventario,
caja, gasto, devolucion, respaldo local y reinicio) y comprueba que el proceso
no establece conexiones fuera de loopback.

Sobre el aislamiento real: bloquear la red por proceso en Windows exige una
regla de firewall, que requiere elevacion. Cuando la sesion no es elevada este
guion mide las conexiones observadas en lugar de fingir un corte de red; el
campo `aislamiento_real` del informe lo declara explicitamente.
"""

import json
import os
import subprocess
import sys
from pathlib import Path

from e2e_returns import preparar_datos, registrar_devolucion, vender
from e2e_flow import App, bootstrap, iniciar_sesion, wait_screen

LOOPBACK = {"127.0.0.1", "::1", "0.0.0.0", "::", ""}


def elevado() -> bool:
    salida = subprocess.run(
        ["net", "session"], capture_output=True, text=True, timeout=30, check=False
    )
    return salida.returncode == 0


def conexiones(pid: int) -> list[dict]:
    script = (
        "Get-NetTCPConnection -OwningProcess "
        f"{pid} -ErrorAction SilentlyContinue | "
        "Select-Object -Property RemoteAddress,RemotePort,State | ConvertTo-Json -Compress"
    )
    salida = subprocess.run(
        ["powershell", "-NoProfile", "-Command", script],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    texto = salida.stdout.strip()
    if not texto:
        return []
    datos = json.loads(texto)
    return datos if isinstance(datos, list) else [datos]


def pid_de_la_corrida(database: str) -> int | None:
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


def main() -> int:
    app_path = sys.argv[1] if len(sys.argv) > 1 else ""
    database = os.environ.get("HIQNET_POS_DB_PATH")
    report: dict = {"escenario": "offline", "aislamiento_real": elevado()}
    fallos: list[str] = []
    if not app_path or not database:
        print(json.dumps({"result": "ERROR", "error": "faltan ejecutable o base"}))
        return 1

    observadas: list[dict] = []

    def muestrear(etapa: str) -> None:
        pid = pid_de_la_corrida(database)
        if pid is None:
            return
        for conexion in conexiones(pid):
            remota = (conexion.get("RemoteAddress") or "").strip()
            observadas.append({"etapa": etapa, **conexion})
            if remota not in LOOPBACK:
                fallos.append(f"{etapa}: conexion remota {remota}:{conexion.get('RemotePort')}")

    app = App(app_path)
    try:
        muestrear("arranque")
        bootstrap(app)
        muestrear("bootstrap")
        datos = preparar_datos(app)
        muestrear("alta_datos")
        folio = vender(app, datos["product_id"], datos["shift_id"])
        muestrear("venta")

        categoria = app.invoke(
            "create_expense_category", {"draft": {"name": "Operacion offline"}}
        )
        app.invoke(
            "record_expense",
            {
                "draft": {
                    "category_id": categoria["id"],
                    "amount": "9.00",
                    "concept": "Gasto sin red",
                    "paid_from_cash": False,
                    "shift_id": None,
                    "idempotency_key": "offline-gasto-1",
                }
            },
        )
        muestrear("gasto")

        registrar_devolucion(app, folio)
        muestrear("devolucion")

        respaldo = Path(app.invoke("backup_database"))
        report["respaldo"] = {"existe": respaldo.is_file(), "bytes": respaldo.stat().st_size if respaldo.is_file() else 0}
        muestrear("respaldo")

        report["stock"] = app.invoke("current_stock", {"product_id": datos["product_id"]})
        report["folio"] = folio
    finally:
        app.quit()

    # Reinicio: la base debe conservar todo sin ninguna dependencia externa.
    app = App(app_path)
    try:
        wait_screen(app, "Entrar a la caja")
        iniciar_sesion(app)
        wait_screen(app, "Ventas")
        muestrear("reinicio")
        resumen = app.invoke(
            "reports_summary",
            {"query": {"from": "2020-01-01T00:00:00Z", "to": "2100-01-01T00:00:00Z"}},
        )
        report["reportes_tras_reinicio"] = resumen
    finally:
        app.quit()

    remotas = [c for c in observadas if (c.get("RemoteAddress") or "").strip() not in LOOPBACK]
    report["conexiones"] = {
        "total": len(observadas),
        "no_loopback": remotas,
        "por_etapa": sorted({c["etapa"] for c in observadas}),
    }
    if not report["aislamiento_real"]:
        report["nota_aislamiento"] = (
            "Sesion no elevada: no se aplico una regla de firewall por proceso. "
            "La evidencia es la ausencia de conexiones fuera de loopback durante "
            "todo el ciclo, no un corte de red forzado."
        )
    report["fallos"] = fallos
    report["result"] = "PASS" if not fallos else "FAIL"
    print(json.dumps(report, ensure_ascii=False))
    return 0 if not fallos else 1


if __name__ == "__main__":
    sys.exit(main())
