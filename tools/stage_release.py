#!/usr/bin/env python3
"""Selecciona exclusivamente los instaladores finales de un release POS."""

from __future__ import annotations

import argparse
import hashlib
import shutil
import sys
from pathlib import Path


def candidates(root: Path, suffix: str) -> list[Path]:
    return sorted(
        (path for path in root.rglob("*") if path.is_file() and path.name.lower().endswith(suffix)),
        key=lambda path: str(path),
    )


def select_one(paths: list[Path], label: str) -> Path:
    if not paths:
        raise ValueError(f"falta el instalador {label}")
    if len(paths) > 1:
        raise ValueError(f"multiples candidatos {label}: {', '.join(str(p) for p in paths)}")
    path = paths[0]
    if path.stat().st_size == 0:
        raise ValueError(f"instalador {label} vacio: {path}")
    return path


def stage(artifacts: Path, output: Path) -> list[Path]:
    if not artifacts.is_dir():
        raise ValueError(f"directorio de artefactos inexistente: {artifacts}")
    output.mkdir(parents=True, exist_ok=True)
    for old in output.iterdir():
        if old.is_file() or old.is_symlink():
            old.unlink()
        elif old.is_dir():
            shutil.rmtree(old)

    selected = [
        select_one(candidates(artifacts, ".exe"), "Windows"),
        select_one(candidates(artifacts, ".deb"), "Linux DEB"),
        select_one(candidates(artifacts, ".appimage"), "Linux AppImage"),
    ]

    staged: list[Path] = []
    seen_names: set[str] = set()
    for source in selected:
        if source.name in seen_names:
            raise ValueError(f"nombre de instalador duplicado: {source.name}")
        seen_names.add(source.name)
        target = output / source.name
        shutil.copy2(source, target)
        staged.append(target)

    manifest = output / "SHA256SUMS.txt"
    lines = []
    for path in staged:
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        lines.append(f"{digest}  {path.name}")
    manifest.write_text("\n".join(lines) + "\n", encoding="utf-8")
    staged.append(manifest)

    if len(staged) != 4:
        raise ValueError("staging incompleto")
    return staged


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--artifacts", type=Path, default=Path("artifacts"))
    parser.add_argument("--output", type=Path, default=Path("release-assets"))
    args = parser.parse_args()
    try:
        staged = stage(args.artifacts, args.output)
    except ValueError as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1
    for path in staged:
        print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
