-- Marcador de licencia instalada.
--
-- Registra el momento de la última instalación de un grant verificado. Es
-- persistente y no depende del documento: si el grant o su firma desaparecen
-- de la base, la instalación queda en sólo lectura y no puede regresar al
-- trial local ni autorizar con las columnas sin verificar.
ALTER TABLE local_installation_state ADD COLUMN license_installed_at TEXT;

-- Bases históricas: una instalación que ya tenía un documento almacenado
-- (grant + firma) conserva la evidencia de haber sido licenciada. La fecha del
-- marcador es la última escritura conocida de la fila: sólo importa el hecho,
-- no la precisión del instante. No se usan columnas comerciales sin verificar
-- (plan, license_status, valid_until) como prueba: únicamente el par
-- documento+firma persistido.
UPDATE local_installation_state
SET license_installed_at = updated_at
WHERE license_grant_json IS NOT NULL
  AND trim(license_grant_json) <> ''
  AND license_signature_b64 IS NOT NULL
  AND trim(license_signature_b64) <> '';
