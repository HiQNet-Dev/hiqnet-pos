"""Medición de rendimiento sobre el ejecutable real con SQLite aislada.

No valida reglas de negocio: mide tiempos de arranque, checkout y consultas con
un volumen pequeño pero real (200 ventas). Sirve como referencia local; no es un
SLA ni sustituye una prueba de carga.
"""

import json
import os
import sys
import time
from datetime import datetime, timedelta, timezone
from pathlib import Path

from e2e_flow import App, wait_screen

VENTAS = 200


def iso(moment: datetime) -> str:
    return moment.astimezone(timezone.utc).isoformat().replace("+00:00", "Z")


def main() -> int:
    app_path = str(
        Path(
            sys.argv[1]
            if len(sys.argv) > 1
            else "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe"
        ).resolve()
    )
    report: dict = {"application": app_path, "ventas_sembradas": VENTAS}
    started = time.perf_counter()
    app = App(app_path)
    try:
        wait_screen(app, "Configura tu negocio")
        report["startup_to_onboarding_s"] = round(time.perf_counter() - started, 3)

        from e2e_flow import bootstrap

        bootstrap(app)

        product = app.invoke(
            "create_product",
            {
                "draft": {
                    "sku": "PERF-1",
                    "name": "Producto rendimiento",
                    "price": "10.00",
                    "tax_rate_bp": 0,
                    "category_id": None,
                }
            },
        )
        app.invoke(
            "record_inventory_movement",
            {
                "draft": {
                    "product_id": product["id"],
                    "kind": "inbound",
                    "quantity": "10000",
                    "reason": None,
                }
            },
        )
        register = app.invoke(
            "create_cash_register", {"draft": {"name": "Caja perf", "code": "PERF-1"}}
        )
        shift = app.invoke(
            "open_cash_shift",
            {"draft": {"register_id": register["id"], "opening_amount": "0.00"}},
        )

        checkout_started = time.perf_counter()
        for index in range(VENTAS):
            app.invoke(
                "checkout_sale",
                {
                    "draft": {
                        "shift_id": shift["id"],
                        "idempotency_key": f"perf-{index}",
                        "items": [
                            {
                                "product_id": product["id"],
                                "variant_id": None,
                                "quantity": "1",
                                "selected_option_ids": [],
                            }
                        ],
                        "cash_received": "10.00",
                    }
                },
            )
        elapsed = time.perf_counter() - checkout_started
        report["checkout_total_s"] = round(elapsed, 3)
        report["checkout_avg_ms"] = round(elapsed / VENTAS * 1000, 2)

        now = datetime.now(timezone.utc)
        measurements = {}
        for name, command, args in [
            ("list_products", "list_products", None),
            ("stock_levels", "list_stock_levels", None),
            (
                "reports",
                "reports_summary",
                {"query": {"from": iso(now - timedelta(days=1)), "to": iso(now + timedelta(days=1))}},
            ),
            ("sale_returns", "list_sale_returns", None),
        ]:
            query_started = time.perf_counter()
            app.invoke(command, args) if args else app.invoke(command)
            measurements[name] = round((time.perf_counter() - query_started) * 1000, 2)
        report["queries_ms"] = measurements

        app.quit()
        restart_started = time.perf_counter()
        app = App(app_path)
        wait_screen(app, "Entrar a la caja")
        report["restart_to_login_s"] = round(time.perf_counter() - restart_started, 3)

        report["result"] = "PASS"
        print(json.dumps(report, ensure_ascii=False))
        return 0
    except Exception as error:  # noqa: BLE001
        report.update({"result": "FAIL", "error": str(error)})
        print(json.dumps(report, ensure_ascii=False))
        return 1
    finally:
        app.quit()
        directory = os.environ.get("HIQNET_E2E_ARTIFACT_DIR")
        if directory:
            try:
                (Path(directory) / "perf-report.json").write_text(
                    json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
                )
            except Exception:
                pass


if __name__ == "__main__":
    raise SystemExit(main())
