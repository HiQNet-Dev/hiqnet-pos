-- Vínculo separado de la identidad local. El documento firmado se vuelve a
-- comprobar al resolver una licencia; la columna de tenant nunca es autoridad sola.
CREATE TABLE commercial_binding (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    installation_id TEXT NOT NULL,
    organization_id TEXT NOT NULL,
    proof_source TEXT NOT NULL CHECK (proof_source = 'signed_license_grant_v1'),
    proof_json TEXT NOT NULL,
    proof_signature_b64 TEXT NOT NULL,
    bound_at TEXT NOT NULL
);
