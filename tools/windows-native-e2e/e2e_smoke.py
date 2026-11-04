import json
import sys
from pathlib import Path

from selenium import webdriver
from selenium.webdriver.common.options import BaseOptions


class WryOptions(BaseOptions):
    def __init__(self, application: str):
        super().__init__()
        self._application = application

    @property
    def default_capabilities(self):
        return {"browserName": "wry"}

    def to_capabilities(self):
        return {
            "browserName": "wry",
            "tauri:options": {"application": self._application},
        }


def main() -> int:
    app = Path(sys.argv[1] if len(sys.argv) > 1 else
               "apps/pos-desktop/src-tauri/target/debug/pos-desktop.exe").resolve()
    if not app.exists():
        print(json.dumps({"error": f"no existe {app}"}))
        return 1
    options = WryOptions(str(app))
    driver = webdriver.Remote(command_executor="http://127.0.0.1:4444", options=options)
    try:
        print(json.dumps({
            "title": driver.title,
            "url": driver.current_url,
            "source_head": driver.page_source[:500],
        }, ensure_ascii=False))
    finally:
        driver.quit()
    return 0


if __name__ == "__main__":
    raise SystemExit(main())