# Protección de `main` y tags de release — propuesta

> **No aplicado.** Requiere autorización del propietario y permisos de
> administración del repositorio. Verificado el 2026-11-07 tras integrar
> #17–#22: `main` **sigue sin protección** (`GET /branches/main/protection` →
> 404) y no hay rulesets (`GET /rulesets` → `[]`). Nada impide push directo,
> force push o borrado.

## Estado actual del repositorio

- `main`: sin protección; CI verde en `455718f` con los cinco jobs.
- Tags: sin ruleset; `pos-v*` dispara el workflow de release.
- Permisos de workflows: `contents: read` en CI; `contents: write` solo en
  release.
- Secretos de firma: fuera del repositorio (DPAPI en Windows / entorno local).
  No hay secretos de firma en GitHub Actions.

## Checks obligatorios propuestos

Ya existen en `main` (CI verde en `455718f`), así que pueden exigirse sin dejar
PR bloqueados por un check inexistente.

| Check | Workflow |
| --- | --- |
| `Rust (fmt, clippy, tests)` | CI |
| `Frontend (lint, tipos, pruebas, build)` | CI |
| `Tauri Windows (build de verificación)` | CI |
| `Tauri Linux (compilación real)` | CI |
| `Archivos prohibidos y auditoría` | CI |

**Android no se exige**: es opt-in (etiqueta `android` o ejecución manual) y no
debe bloquear PR de documentación o escritorio.

## Ruleset para `main`

- Target: `refs/heads/main`.
- Reglas:
  - `deletion` — bloquear borrado.
  - `non_fast_forward` — bloquear force push.
  - `pull_request` — integración solo por PR; 1 aprobación; descartar
    aprobaciones obsoletas tras push.
  - `required_status_checks` — los cinco checks de arriba, con
    `strict_required_status_checks_policy: true` (la rama debe estar al día).
  - `required_conversation_resolution: true`.
  - (no `required_linear_history`): compatible con merge commits.
- `bypass_actors`: vacío. Si el propietario necesita emergencias, añadir solo su
  rol de administrador de forma explícita y temporal.
- `enforcement: active`.

## Ruleset para tags de release

- Target: `refs/tags/pos-v*`.
- Reglas: `deletion`, `update`. No se incluye `creation`, para no
  impedir tags autorizados sin un mecanismo de excepci�n definido.
- Objetivo: que un tag publicado no se mueva ni se borre, y que solo los
  mantenedores creen tags.
- Complementario: publicar el release desde un entorno protegido con revisores
  requeridos y secretos de firma como secretos de entorno (no de repositorio).

## Aplicación (cuando se autorice)

```bash
# Los checks ya existen en main; no hace falta esperar a otro merge
gh api --method POST repos/HiQNet-Dev/hiqnet-pos/rulesets \
  --input docs/07-maintenance/ruleset-main.json

gh api --method POST repos/HiQNet-Dev/hiqnet-pos/rulesets \
  --input docs/07-maintenance/ruleset-tags.json
```

Los cuerpos JSON de ejemplo viven junto a este documento. Ajusta `id` de checks
si el nombre visible del job cambia: GitHub identifica el check por el nombre
del job, no por el workflow.

## Verificación posterior

```bash
gh api repos/HiQNet-Dev/hiqnet-pos/rulesets
gh api repos/HiQNet-Dev/hiqnet-pos/branches/main/protection
gh ruleset list
```

Comprobar con un PR de prueba: push directo a `main` rechazado, force push
rechazado, PR sin checks en rojo, y tag `pos-v0.0.0-test` no modificable tras
crearse.

## Permisos y secretos

- Workflows: mantener `permissions: contents: read` por defecto.
- Release: `contents: write` solo en el job que publica; considerar
  `environment: release` con revisores.
- Firma de instaladores/APK: secretos fuera del repositorio y rotables. Nunca
  en `.env`, keystores versionados ni artefactos.
