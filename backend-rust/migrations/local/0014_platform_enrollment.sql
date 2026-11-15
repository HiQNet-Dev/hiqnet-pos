-- Sesión local del enrolamiento con HiQNet Platform.
-- Solo hay una fila: un challenge activo a la vez. El device_code es un
-- secreto de un solo uso y nunca se expone por IPC.
CREATE TABLE platform_enrollment (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    platform_url TEXT NOT NULL,
    device_code TEXT,
    user_code TEXT NOT NULL,
    verification_uri TEXT NOT NULL,
    nonce TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    installation_id TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('pending', 'completed')),
    organization_id TEXT,
    bound_at TEXT,
    updated_at TEXT NOT NULL,
    CHECK (
        (status = 'pending' AND device_code IS NOT NULL AND organization_id IS NULL AND bound_at IS NULL)
        OR
        (status = 'completed' AND device_code IS NULL AND organization_id IS NOT NULL AND bound_at IS NOT NULL)
    )
);
