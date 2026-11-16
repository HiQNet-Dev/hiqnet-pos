import { useEffect, useState } from "react";
import { api } from "../../api";
import type { PlatformEnrollment } from "../../contracts";
import { formatDate } from "../../lib/formatDate";

interface Props {
  onError: (cause: unknown, fallback: string) => void;
}

/** Vinculación del equipo con HiQNet Platform desde Gestión. */
export function PlatformEnrollmentPanel({ onError }: Props) {
  const [enrollment, setEnrollment] = useState<PlatformEnrollment | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .platformEnrollmentStatus()
      .then((view) => {
        if (!cancelled) setEnrollment(view);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, []);

  async function beginEnrollment() {
    if (busy) return;
    setBusy(true);
    try {
      setEnrollment(await api.beginPlatformEnrollment());
    } catch (cause) {
      onError(cause, "No fue posible iniciar la vinculación con Platform.");
    } finally {
      setBusy(false);
    }
  }

  async function completeEnrollment() {
    if (busy) return;
    setBusy(true);
    try {
      setEnrollment(await api.completePlatformEnrollment());
    } catch (cause) {
      onError(cause, "La vinculación aún no está aprobada o el código venció.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="license-activation">
      <h2>Vincular con HiQNet Platform</h2>
      <p className="muted">
        Genera un código, apruébalo en el Portal con la cuenta propietaria y
        completa la vinculación. Solo se comparte la clave pública del equipo.
      </p>
      {enrollment?.status === "completed" ? (
        <p className="muted" role="status">
          Equipo vinculado con la organización comercial{" "}
          {enrollment.organization_id}.
        </p>
      ) : enrollment?.status === "pending" ? (
        <>
          <dl className="metric-grid">
            <div>
              <dt>Código para el Portal</dt>
              <dd className="metric-text">{enrollment.user_code}</dd>
            </div>
            <div>
              <dt>Vence</dt>
              <dd>
                {enrollment.expires_at
                  ? formatDate(enrollment.expires_at)
                  : "—"}
              </dd>
            </div>
          </dl>
          <p className="muted">
            Abre {enrollment.verification_uri} e introduce el código desde la
            cuenta propietaria de la organización.
          </p>
          <div className="row-actions">
            <button
              type="button"
              className="button button-primary"
              onClick={() => void completeEnrollment()}
              disabled={busy}
            >
              {busy ? "Verificando…" : "Ya está aprobado, completar"}
            </button>
            <button
              type="button"
              className="button"
              onClick={() => void beginEnrollment()}
              disabled={busy}
            >
              Generar nuevo código
            </button>
          </div>
        </>
      ) : (
        <div className="row-actions">
          <button
            type="button"
            className="button"
            onClick={() => void beginEnrollment()}
            disabled={busy}
          >
            {busy ? "Generando…" : "Generar código de vinculación"}
          </button>
        </div>
      )}
    </div>
  );
}
