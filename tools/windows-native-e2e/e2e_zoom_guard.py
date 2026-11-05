"""Verifica que los atajos de zoom no desplacen ni reduzcan el WebView de Tauri.

Se ejecuta únicamente mediante ``run-e2e.ps1``: el ejecutable recibe una base
SQLite temporal y el arnés activa la automatización WebDriver de Tauri.
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

from selenium import webdriver
from selenium.webdriver.common.action_chains import ActionChains
from selenium.webdriver.common.by import By
from selenium.webdriver.common.keys import Keys
from selenium.webdriver.common.options import BaseOptions
from selenium.webdriver.support import expected_conditions as EC
from selenium.webdriver.support.ui import WebDriverWait


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
        ("Nombre del negocio", "Sonda zoom"),
        ("Nombre de la sucursal", "Matriz"),
        ("Código", "ZX-01"),
        ("Nombre del propietario", "Operador E2E"),
        ("Usuario", "operador"),
        ("Contraseña", "contrasena-segura"),
        ("Confirmar contraseña", "contrasena-segura"),
    ]:
        fill(driver, label, value)
    driver.find_element(By.XPATH, "//button[normalize-space()='Crear instalación y entrar']").click()
    WebDriverWait(driver, 30).until(
        EC.presence_of_element_located((By.CSS_SELECTOR, ".app"))
    )


def viewport_metrics(driver: webdriver.Remote) -> dict[str, float | int | None]:
    return driver.execute_script(
        """
        const rect = (element) => {
          if (!element) return null;
          const { x, y, width, height } = element.getBoundingClientRect();
          return { x, y, width, height };
        };
        return {
          devicePixelRatio: window.devicePixelRatio,
          visualViewportScale: window.visualViewport ? window.visualViewport.scale : null,
          innerWidth: window.innerWidth,
          innerHeight: window.innerHeight,
          document: rect(document.documentElement),
          root: rect(document.getElementById("root")),
          app: rect(document.querySelector(".app")),
        };
        """
    )


def main() -> int:
    if len(sys.argv) != 2:
        print(json.dumps({"error": "uso: e2e_zoom_guard.py <ejecutable>"}))
        return 2

    app = Path(sys.argv[1]).resolve()
    database = os.environ.get("HIQNET_POS_DB_PATH")
    if not app.exists() or not database:
        print(json.dumps({"error": "ejecutable o base temporal no disponible"}))
        return 2

    options = WryOptions(str(app), ["--hiqnet-e2e-db-path", database])
    driver = webdriver.Remote(
        command_executor=f"http://127.0.0.1:{os.environ.get('HIQNET_E2E_PORT', '4444')}",
        options=options,
    )
    try:
        bootstrap(driver)
        before = viewport_metrics(driver)
        for _ in range(10):
            ActionChains(driver).key_down(Keys.CONTROL).send_keys("-").key_up(Keys.CONTROL).perform()
        for _ in range(10):
            ActionChains(driver).key_down(Keys.CONTROL).send_keys("+").key_up(Keys.CONTROL).perform()
        ActionChains(driver).key_down(Keys.CONTROL).send_keys("0").key_up(Keys.CONTROL).perform()
        after = viewport_metrics(driver)

        if before != after:
            print(json.dumps({"before": before, "after": after}, ensure_ascii=False))
            return 1

        print(json.dumps({"before": before, "after": after}, ensure_ascii=False))
        return 0
    finally:
        driver.quit()


if __name__ == "__main__":
    raise SystemExit(main())
