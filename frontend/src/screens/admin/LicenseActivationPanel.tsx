import { useEffect, useState } from "react";
import { api } from "../../api";

interface Props {
  onCommercialChanged?: () => Promise<void>;
  onError: (cause: unknown, fallback: string) => void;
  onClearError: () => void;
}

export function LicenseActivationPanel({
  onCommercialChanged,
  onError,
  onClearError,
}: Props) {
  const [grantJson, setGrantJson] = useState("");
  const [signature, setSignature] = useState("");
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState<string | null>(null);
  const [installationId, setInstallationId] = useState<string | null>(null);

  useEffect(() => {
    api
      .commercialState()
      .then((state) => setInstallationId(state.installation_id))
      .catch((cause: unknown) =>
        onError(cause, "No fue posible cargar la instalación."),
      );
  }, [onError]);

  async function activateLicense() {
    if (busy) return;
    onClearError();
    setNotice(null);
    if (!grantJson.trim() || !signature.trim()) {
      onError(
        new Error("missing license fields"),
        "Pega el documento de licencia y su firma antes de continuar.",
      );
      return;
    }

    setBusy(true);
    try {
      await api.installLicense(grantJson.trim(), signature.trim());
      setGrantJson("");
      setSignature("");
      setNotice("Licencia instalada y verificada.");
      await onCommercialChanged?.();
    } catch (cause) {
      onError(
        cause,
        "La licencia no se pudo verificar. Revisa el documento y la firma.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="license-activation">
      <h2>Activar licencia</h2>
      <p className="muted">
        Pega el documento y la firma emitidos por HiQNet Platform para esta
        instalación. El documento se verifica antes de guardarse.
      </p>
      <dl className="metric-grid">
        <div>
          <dt>Identificador de instalación</dt>
          <dd className="metric-text">{installationId ?? "No disponible"}</dd>
        </div>
      </dl>
      <label className="field">
        <span>Documento de licencia (JSON)</span>
        <textarea
          value={grantJson}
          onChange={(event) => setGrantJson(event.target.value)}
          rows={4}
          spellCheck={false}
          autoComplete="off"
          aria-label="Documento de licencia"
        />
      </label>
      <label className="field">
        <span>Firma Ed25519 (base64)</span>
        <textarea
          value={signature}
          onChange={(event) => setSignature(event.target.value)}
          rows={2}
          spellCheck={false}
          autoComplete="off"
          aria-label="Firma de licencia"
        />
      </label>
      <div className="row-actions">
        <button
          type="button"
          className="button button-primary"
          onClick={() => void activateLicense()}
          disabled={busy}
        >
          {busy ? "Verificando…" : "Instalar licencia"}
        </button>
      </div>
      {notice && (
        <p className="muted" role="status">
          {notice}
        </p>
      )}
    </div>
  );
}
