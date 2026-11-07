#!/usr/bin/env bash
# Falla si el árbol de trabajo contiene archivos prohibidos o patrones de
# secretos. Se ejecuta en CI y antes de publicar cambios.
set -euo pipefail

status=0

# 1. Archivos que nunca deben versionarse.
forbidden=$(git ls-files --cached --others --exclude-standard \
  | grep -Ei '\.(db|db-wal|db-shm|sqlite|sqlite3|dump)$|^\.env$|^secrets/|^signing/|\.(pem|key|p12|pfx|jks|keystore|apk|aab|idsig)$' || true)
if [[ -n "${forbidden}" ]]; then
  echo "Archivos prohibidos detectados:"
  echo "${forbidden}"
  status=1
fi

# 2. Patrones de credenciales en archivos de texto versionados.
secret_patterns='-----BEGIN (RSA |EC |OPENSSH )?PRIVATE KEY-----|gh[pousr]_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}'
matches=$(git grep -nEI -e "${secret_patterns}" -- \
  ':!frontend/pnpm-lock.yaml' ':!**/*.lock' ':!docs/**' ':!.github/**' || true)
if [[ -n "${matches}" ]]; then
  echo "Posibles secretos en archivos versionados:"
  echo "${matches}"
  status=1
fi

# 3. SQLite debe existir solo como migraciones SQL, no como base binaria.
bases=$(git ls-files | grep -Ei '\.(db|sqlite3?)($|-)' || true)
if [[ -n "${bases}" ]]; then
  echo "Bases de datos versionadas:"
  echo "${bases}"
  status=1
fi

if [[ "${status}" -eq 0 ]]; then
  echo "OK: no se detectaron archivos prohibidos ni secretos obvios."
fi
exit "${status}"
