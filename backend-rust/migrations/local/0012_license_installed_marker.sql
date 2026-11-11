-- Marcador de licencia instalada.
--
-- Registra el momento de la última instalación de un grant verificado. Es
-- persistente y no depende del documento: si el grant o su firma desaparecen
-- de la base, la instalación queda en sólo lectura y no puede regresar al
-- trial local ni autorizar con las columnas sin verificar.
ALTER TABLE local_installation_state ADD COLUMN license_installed_at TEXT;
