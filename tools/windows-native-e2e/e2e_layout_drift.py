"""Comprueba que la interfaz de escritorio no derive ni deje la ventana vacía.

Reproduce el patrón de uso intensivo descrito por el operador (``de tanto
moverle``): navegación repetida entre secciones, apertura y cierre de diálogos,
desplazamiento interno de listas, recorrido con Tab y cambios de tamaño de
ventana. Tras cada ronda verifica invariantes estructurales:

* Ningún contenedor raíz desplazado (``html``, ``body``, ``#root``, ``.app``,
  ``.app-body``, ``.app-content``). Un ``overflow: hidden`` puede desplazarse
  igualmente de forma programática o por foco, y entonces el diseño se "va"
  hacia una esquina dejando el resto de la ventana vacío.
* El shell cubre exactamente el viewport.

Se ejecuta únicamente mediante ``run-e2e.ps1`` para recibir una base SQLite
temporal y la automatización WebDriver de Tauri.
"""

from __future__ import annotations

import json
import os
import sys
import time
from pathlib import Path

from selenium import webdriver
from selenium.webdriver.common.action_chains import ActionChains
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.common.options import BaseOptions
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.support.ui import WebDriverWait

SECTIONS = ["sales", "catalog", "inventory", "cash", "reports"]

SNAPSHOT_JS = """
const rect = (element) => {
  if (!element) return null;
  const box = element.getBoundingClientRect();
  const round = (value) => Math.round(value * 100) / 100;
  return { x: round(box.x), y: round(box.y), width: round(box.width), height: round(box.height) };
};
const scroll = (element) => {
  if (!element) return null;
  return {
    top: element.scrollTop,
    left: element.scrollLeft,
    scrollHeight: element.scrollHeight,
    scrollWidth: element.scrollWidth,
    clientHeight: element.clientHeight,
    clientWidth: element.clientWidth,
  };
};
const app = document.querySelector(".app");
const body = document.body;
return {
  innerWidth: window.innerWidth,
  innerHeight: window.innerHeight,
  devicePixelRatio: window.devicePixelRatio,
  visualViewportScale: window.visualViewport ? window.visualViewport.scale : null,
  bodyInlineStyle: body.getAttribute("style"),
  bodyOverflow: getComputedStyle(body).overflow,
  htmlScroll: scroll(document.scrollingElement),
  bodyScroll: scroll(body),
  rootScroll: scroll(document.getElementById("root")),
  appScroll: scroll(app),
  appBodyScroll: scroll(document.querySelector(".app-body")),
  appContentScroll: scroll(document.querySelector(".app-content")),
  screenScroll: scroll(document.querySelector(".app-content .screen, .app-content .sales")),
  sidenavScroll: scroll(document.querySelector(".sidenav")),
  modalBodyScroll: scroll(document.querySelector(".modal-body")),
  appTransform: app ? getComputedStyle(app).transform : null,
  appZoom: app ? getComputedStyle(app).zoom : null,
  rectApp: rect(app),
  rectAppBody: rect(document.querySelector(".app-body")),
  rectAppContent: rect(document.querySelector(".app-content")),
  rectRoot: rect(document.getElementById("root")),
};
"""

# Contenedores que nunca deben desplazarse en la app de escritorio.
FIXED_SCROLLERS = ["htmlScroll", "bodyScroll", "rootScroll", "appScroll", "appBodyScroll", "appContentScroll"]


class WryOptions(BaseOptions):
    def __init__(self, application: str, args: list[str]):
        super().__init__()
        self._application = application
        self._args = args

    @property
    def default_capabilities(self):
        return {"browserName": "wry"}

    def to_capabilities(self):
        return {
            "browserName": "wry",
            "tauri:options": {"application": self._application, "args": self._args},
        }


def fill(driver: webdriver.Remote, label: str, value: str) -> None:
    field = WebDriverWait(driver, 30).until(
        EC.presence_of_element_located(
            (By.XPATH, f"//section//label[.//span[normalize-space()='{label}']]//input")
        )
    )
    field.send_keys(value)


def bootstrap(driver: webdriver.Remote) -> None:
    WebDriverWait(driver, 30).until(
        EC.presence_of_element_located((By.XPATH, "//*[normalize-space()='Configura tu negocio']"))
    )
    for label, value in [
        ("Nombre del negocio", "Sonda deriva"),
        ("Nombre de la sucursal", "Matriz"),
        ("Código", "DR-01"),
        ("Nombre del propietario", "Operador E2E"),
        ("Usuario", "operador"),
        ("Contraseña", "contrasena-segura"),
        ("Confirmar contraseña", "contrasena-segura"),
    ]:
        fill(driver, label, value)
    driver.find_element(By.XPATH, "//button[normalize-space()='Crear instalación y entrar']").click()
    WebDriverWait(driver, 30).until(EC.presence_of_element_located((By.CSS_SELECTOR, ".app")))
    # La pantalla de ventas usa `.sales`, no `.screen`; el contenedor estable es
    # `.app-content`.
    WebDriverWait(driver, 30).until(
        EC.presence_of_element_located((By.CSS_SELECTOR, ".app-content"))
    )
    time.sleep(0.5)


def snapshot(driver: webdriver.Remote) -> dict:
    return driver.execute_script(SNAPSHOT_JS)


def drift_problems(state: dict, baseline: dict | None = None) -> list[str]:
    problems: list[str] = []
    for name in FIXED_SCROLLERS:
        value = state.get(name)
        if not value:
            continue
        if value["top"] or value["left"]:
            problems.append(f"{name} desplazado a top={value['top']} left={value['left']}")

    app = state.get("rectApp")
    if not app:
        problems.append("el shell .app no existe")
        return problems
    if abs(app["x"]) > 1 or abs(app["y"]) > 1:
        problems.append(f".app desplazado a x={app['x']} y={app['y']}")
    if abs(app["width"] - state["innerWidth"]) > 1:
        problems.append(f".app ancho {app['width']} != viewport {state['innerWidth']}")
    if abs(app["height"] - state["innerHeight"]) > 1:
        problems.append(f".app alto {app['height']} != viewport {state['innerHeight']}")

    transform = state.get("appTransform")
    if transform not in (None, "none"):
        problems.append(f".app tiene transform {transform}")

    if baseline:
        for key in ("devicePixelRatio", "visualViewportScale"):
            if baseline.get(key) != state.get(key):
                problems.append(f"{key} cambió de {baseline.get(key)} a {state.get(key)}")

    scrolling = state.get("htmlScroll") or {}
    if scrolling.get("scrollHeight", 0) > scrolling.get("clientHeight", 0) + 1:
        problems.append(
            "el documento es desplazable "
            f"(scrollHeight={scrolling.get('scrollHeight')} clientHeight={scrolling.get('clientHeight')})"
        )
    return problems


def cycle_sections(driver: webdriver.Remote) -> None:
    for section in SECTIONS:
        try:
            button = driver.find_element(By.CSS_SELECTOR, f".nav-item[data-section='{section}']")
        except Exception:
            continue
        driver.execute_script("arguments[0].scrollIntoView({block:'nearest'});", button)
        button.click()
        time.sleep(0.15)


def scroll_lists(driver: webdriver.Remote) -> None:
    driver.execute_script(
        """
        const targets = document.querySelectorAll(
          ".product-scroll, .app-content .screen, .app-content .sales, .sidenav, .chips"
        );
        for (const target of targets) {
          target.scrollTop = target.scrollHeight;
          target.scrollLeft = target.scrollWidth;
        }
        """
    )
    time.sleep(0.1)
    driver.execute_script(
        """
        const targets = document.querySelectorAll(
          ".product-scroll, .app-content .screen, .app-content .sales, .sidenav, .chips"
        );
        for (const target of targets) {
          target.scrollTop = 0;
          target.scrollLeft = 0;
        }
        """
    )


def toggle_dialog(driver: webdriver.Remote) -> None:
    """Abre y cierra un diálogo modal: foco, portal y bloqueo del fondo."""
    opened = driver.execute_script(
        """
        const buttons = Array.from(document.querySelectorAll("button"));
        const target = buttons.find((button) => /Nueva categoría/i.test(button.textContent || ""));
        if (!target) return false;
        target.click();
        return true;
        """
    )
    if not opened:
        return
    WebDriverWait(driver, 10).until(
        EC.presence_of_element_located((By.CSS_SELECTOR, ".modal-backdrop"))
    )
    time.sleep(0.2)
    ActionChains(driver).send_keys(Keys.ESCAPE).perform()
    WebDriverWait(driver, 10).until(
        EC.invisibility_of_element_located((By.CSS_SELECTOR, ".modal-backdrop"))
    )


def tab_walk(driver: webdriver.Remote, presses: int) -> None:
    body = driver.find_element(By.TAG_NAME, "body")
    for _ in range(presses):
        body.send_keys(Keys.TAB)
    for _ in range(presses):
        ActionChains(driver).key_down(Keys.SHIFT).send_keys(Keys.TAB).key_up(Keys.SHIFT).perform()


def ctrl_wheel_zoom(driver: webdriver.Remote, steps: int = 8) -> str:
    """Rueda con Ctrl: único gesto de zoom que WebView2 puede seguir aceptando."""
    try:
        chain = ActionChains(driver).key_down(Keys.CONTROL)
        for _ in range(steps):
            chain.scroll_by_amount(0, -120)
        chain.key_up(Keys.CONTROL).perform()
        chain = ActionChains(driver).key_down(Keys.CONTROL)
        for _ in range(steps):
            chain.scroll_by_amount(0, 120)
        chain.key_up(Keys.CONTROL).perform()
        return "ok"
    except Exception as error:  # el driver W3C puede no soportar la rueda
        return f"no soportado: {error}"


def move_window(driver: webdriver.Remote) -> str:
    """Desplaza y redimensiona la ventana real como haría el operador."""
    moved = 0
    for x, y, width, height in [
        (40, 40, 1280, 800),
        (320, 180, 1100, 720),
        (900, 500, 1280, 800),
        (60, 300, 1024, 700),
        (500, 60, 1440, 900),
        (200, 200, 1280, 800),
    ]:
        try:
            driver.set_window_position(x, y)
            driver.set_window_size(width, height)
            moved += 1
            time.sleep(0.2)
        except Exception:
            break
    return f"posiciones={moved}"


def maximize_cycle(driver: webdriver.Remote) -> str:
    restored = 0
    for _ in range(3):
        try:
            driver.maximize_window()
            time.sleep(0.25)
            driver.set_window_size(1280, 800)
            time.sleep(0.25)
            restored += 1
        except Exception:
            break
    return f"ciclos={restored}"


def clipping_report(driver: webdriver.Remote) -> dict:
    """Comprueba que los contenedores del shell recortan sin poder desplazarse.

    ``overflow: hidden`` sigue siendo desplazable; ``clip`` no lo es. Además se
    verifica que un contenedor con desplazamiento legítimo (tabla o catálogo)
    sí pueda moverse, para no haber roto el scroll interno.
    """
    return driver.execute_script(
        """
        const probe = (element) => {
          if (!element) return null;
          const style = getComputedStyle(element);
          return {
            overflowY: style.overflowY,
            overflowX: style.overflowX,
            scrollHeight: element.scrollHeight,
            clientHeight: element.clientHeight,
          };
        };
        const legacy = document.querySelector(".product-scroll, .table-scroll");
        let legacyMoved = null;
        if (legacy && legacy.scrollHeight > legacy.clientHeight + 1) {
          const before = legacy.scrollTop;
          legacy.scrollTop = legacy.scrollHeight;
          legacyMoved = legacy.scrollTop > before;
          legacy.scrollTop = before;
        }

        // Prueba estructural: un contenedor con desplazamiento propio dentro de
        // ancestros `clip` debe seguir desplazándose.
        const nested = document.createElement("div");
        nested.style.cssText = "overflow-y:auto;height:40px;width:80px;position:absolute;left:-9999px";
        const filler = document.createElement("div");
        filler.style.height = "400px";
        nested.appendChild(filler);
        const host = document.querySelector(".app-content") || document.body;
        host.appendChild(nested);
        nested.scrollTop = 120;
        const nestedMoved = nested.scrollTop > 0;
        nested.remove();

        return {
          app: probe(document.querySelector(".app")),
          appBody: probe(document.querySelector(".app-body")),
          appContent: probe(document.querySelector(".app-content")),
          productScroll: probe(document.querySelector(".product-scroll")),
          body: probe(document.body),
          html: probe(document.documentElement),
          innerScrollerMoved: legacyMoved,
          nestedScrollerMoved: nestedMoved,
        };
        """
    )


def clipping_problems(report: dict) -> list[str]:
    problems: list[str] = []
    for key in ("app", "appBody", "appContent"):
        value = report.get(key)
        if not value:
            problems.append(f"{key} no existe")
            continue
        if value["overflowY"] != "clip" or value["overflowX"] != "clip":
            problems.append(
                f"{key} usa overflow {value['overflowX']}/{value['overflowY']} en lugar de clip"
            )
    if report.get("innerScrollerMoved") is False:
        problems.append("un contenedor interno con desbordamiento ya no se desplaza")
    if report.get("nestedScrollerMoved") is False:
        problems.append("el recorte `clip` de los ancestros bloqueó un desplazamiento interno")
    product_scroll = report.get("productScroll")
    if product_scroll and product_scroll["overflowY"] != "auto":
        problems.append(
            f".product-scroll usa overflow-y {product_scroll['overflowY']} en lugar de auto"
        )
    return problems


def main() -> int:
    if len(sys.argv) != 2:
        print(json.dumps({"error": "uso: e2e_layout_drift.py <ejecutable>"}))
        return 2

    app = Path(sys.argv[1]).resolve()
    database = os.environ.get("HIQNET_POS_DB_PATH")
    artifacts = Path(os.environ.get("HIQNET_E2E_ARTIFACT_DIR", "."))
    if not app.exists() or not database:
        print(json.dumps({"error": "ejecutable o base temporal no disponible"}))
        return 2

    options = WryOptions(str(app), ["--hiqnet-e2e-db-path", database])
    driver = webdriver.Remote(
        command_executor=f"http://127.0.0.1:{os.environ.get('HIQNET_E2E_PORT', '4444')}",
        options=options,
    )

    rounds = 8
    try:
        bootstrap(driver)
        driver.set_window_size(1280, 800)
        time.sleep(0.4)
        baseline = snapshot(driver)
        (artifacts / "drift-before.png").write_bytes(driver.get_screenshot_as_png())

        clipping = clipping_report(driver)
        clipping_issues = clipping_problems(clipping)

        first_failure: dict | None = None

        # Fase nativa: gestos que el estrés del DOM no puede provocar.
        native: dict[str, str] = {}
        native["move_window"] = move_window(driver)
        native["maximize_cycle"] = maximize_cycle(driver)
        native["ctrl_wheel"] = ctrl_wheel_zoom(driver)
        driver.set_window_size(1280, 800)
        time.sleep(0.4)
        after_native = snapshot(driver)
        native_problems = drift_problems(after_native, baseline)
        if native_problems:
            first_failure = {"round": "nativo", "problems": native_problems, "state": after_native}
            (artifacts / "drift-native.png").write_bytes(driver.get_screenshot_as_png())

        for round_index in range(rounds):
            cycle_sections(driver)
            if round_index % 2 == 0:
                try:
                    toggle_dialog(driver)
                except Exception as error:  # la sonda no debe fallar por el diálogo
                    print(json.dumps({"dialog_skipped": str(error)}, ensure_ascii=False))
            scroll_lists(driver)
            tab_walk(driver, 6)
            if round_index % 3 == 2:
                for width, height in [(1024, 700), (1600, 1000), (1280, 800)]:
                    driver.set_window_size(width, height)
                    time.sleep(0.25)

            state = snapshot(driver)
            problems = drift_problems(state, baseline)
            if problems and first_failure is None:
                first_failure = {"round": round_index + 1, "problems": problems, "state": state}
                (artifacts / "drift-failure.png").write_bytes(driver.get_screenshot_as_png())

        driver.set_window_size(1280, 800)
        time.sleep(0.4)
        final = snapshot(driver)
        (artifacts / "drift-after.png").write_bytes(driver.get_screenshot_as_png())

        final_problems = drift_problems(final, baseline)
        report = {
            "rounds": rounds,
            "clipping": clipping,
            "clipping_problems": clipping_issues,
            "native": native,
            "native_problems": native_problems,
            "baseline": baseline,
            "after_native": after_native,
            "final": final,
            "final_problems": final_problems,
            "first_failure": first_failure,
        }
        (artifacts / "drift-report.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8"
        )
        print(json.dumps(
            {
                "rounds": rounds,
                "clipping": clipping,
                "clipping_problems": clipping_issues,
                "native": native,
                "native_problems": native_problems,
                "final_problems": final_problems,
                "first_failure": None if first_failure is None else {
                    "round": first_failure["round"],
                    "problems": first_failure["problems"],
                },
                "baseline_viewport": [baseline["innerWidth"], baseline["innerHeight"]],
                "final_viewport": [final["innerWidth"], final["innerHeight"]],
                "baseline_body_overflow": baseline["bodyOverflow"],
                "final_body_overflow": final["bodyOverflow"],
                "final_body_inline_style": final["bodyInlineStyle"],
            },
            ensure_ascii=False,
        ))
        return 0 if not final_problems and not clipping_issues else 1
    finally:
        driver.quit()


if __name__ == "__main__":
    raise SystemExit(main())
