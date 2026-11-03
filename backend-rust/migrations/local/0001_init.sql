-- Esquema inicial de HiQNet POS Local.
--
-- Reglas:
--   * Importes en unidades menores (centavos) y cantidades en milésimas.
--   * Importes y cantidades no negativos mediante CHECK.
--   * `audit_event` es append-only (triggers al final del archivo).
--   * Los archivos de bases reales nunca se versionan; esto es solo el esquema.

CREATE TABLE organization (
    id         TEXT PRIMARY KEY NOT NULL,
    name       TEXT NOT NULL CHECK (length(trim(name)) > 0),
    created_at TEXT NOT NULL
);

CREATE TABLE branch (
    id              TEXT PRIMARY KEY NOT NULL,
    organization_id TEXT NOT NULL REFERENCES organization (id) ON DELETE CASCADE,
    name            TEXT NOT NULL CHECK (length(trim(name)) > 0),
    code            TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    UNIQUE (organization_id, code)
);

CREATE TABLE app_user (
    id              TEXT PRIMARY KEY NOT NULL,
    organization_id TEXT NOT NULL REFERENCES organization (id) ON DELETE CASCADE,
    branch_id       TEXT REFERENCES branch (id) ON DELETE SET NULL,
    username        TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    role            TEXT NOT NULL CHECK (role IN ('owner', 'cashier')),
    password_hash   TEXT NOT NULL,
    active          INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at      TEXT NOT NULL,
    UNIQUE (username)
);

CREATE TABLE product (
    id                TEXT PRIMARY KEY NOT NULL,
    organization_id   TEXT NOT NULL REFERENCES organization (id) ON DELETE CASCADE,
    sku               TEXT NOT NULL,
    name              TEXT NOT NULL CHECK (length(trim(name)) > 0),
    unit_price_minor  INTEGER NOT NULL CHECK (unit_price_minor >= 0),
    tax_rate_bp       INTEGER NOT NULL CHECK (tax_rate_bp BETWEEN 0 AND 10000),
    active            INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at        TEXT NOT NULL,
    UNIQUE (organization_id, sku)
);

CREATE TABLE audit_event (
    id              TEXT PRIMARY KEY NOT NULL,
    organization_id TEXT NOT NULL REFERENCES organization (id),
    branch_id       TEXT REFERENCES branch (id),
    actor_user_id   TEXT REFERENCES app_user (id),
    action          TEXT NOT NULL,
    entity_type     TEXT NOT NULL,
    entity_id       TEXT NOT NULL,
    payload_json    TEXT NOT NULL,
    occurred_at     TEXT NOT NULL
);

CREATE INDEX idx_audit_event_org_time
    ON audit_event (organization_id, occurred_at DESC);

CREATE TABLE local_installation_state (
    id                    INTEGER PRIMARY KEY CHECK (id = 1),
    installation_id       TEXT NOT NULL,
    organization_id       TEXT NOT NULL REFERENCES organization (id),
    plan                  TEXT NOT NULL CHECK (plan IN ('trial', 'local', 'pro', 'enterprise')),
    license_status        TEXT NOT NULL
        CHECK (license_status IN ('trial', 'active', 'grace', 'read_only', 'expired')),
    features_json         TEXT NOT NULL,
    trial_started_at      TEXT NOT NULL,
    valid_until           TEXT NOT NULL,
    license_grant_json    TEXT,
    license_signature_b64 TEXT,
    updated_at            TEXT NOT NULL
);

-- La auditoría no se modifica ni se borra: cualquier intento aborta la
-- transacción completa.
CREATE TRIGGER audit_event_no_update
BEFORE UPDATE ON audit_event
BEGIN
    SELECT RAISE(ABORT, 'audit_event es append-only');
END;

CREATE TRIGGER audit_event_no_delete
BEFORE DELETE ON audit_event
BEGIN
    SELECT RAISE(ABORT, 'audit_event es append-only');
END;
