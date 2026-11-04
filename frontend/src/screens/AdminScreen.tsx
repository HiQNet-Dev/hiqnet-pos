import { useEffect, useState } from "react";
import { api, IpcError } from "../api";
import type { Actor, AuditEvent, CommercialState } from "../contracts";

interface Props {
  actor: Actor;
  commercial: CommercialState | null;
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("es-MX", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

const PLAN_LABELS: Record<string, string> = {
  trial: "Trial",
  local: "Local",
  pro: "Pro",
  enterprise: "Enterprise",
};

export function AdminScreen({ actor, commercial }: Props) {
  const canAudit = actor.permissions.includes("audit.view");
  const canBackup = actor.permissions.includes("backups.manage");
  const [audit, setAudit] = useState<AuditEvent[]>([]);
  const [loading, setLoading] = useState(canAudit);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!canAudit) return;
    let cancelled = false;
    api
      .listAuditEvents(50)
      .then((events) => {
        if (!cancelled) setAudit(events);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(
            cause instanceof IpcError ? cause.message : "No fue posible cargar la auditoría.",
          );
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [canAudit]);

  async function backup() {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const path = await api.backupDatabase();
      setNotice(`Respaldo creado en ${path}`);
    } catch (cause) {
      setError(
        cause instanceof IpcError ? cause.message : "No fue posible crear el respaldo.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="screen admin-screen">
      <header className="screen-header">
        <div>
          <h1>Gestión y auditoría</h1>
          <p className="screen-subtitle">
            Estado comercial, respaldos locales y registro de eventos.
          </p>
        </div>
      </header>

      {(error || notice) && (
        <div aria-live="polite" className="screen-inline-alert">
          {error && <p className="alert alert-error">{error}</p>}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      <div className="admin-layout">
        <section className="surface" aria-label="Estado comercial">
          <header className="surface-header">
            <div>
              <p className="eyebrow">Licencia</p>
              <h2>Estado comercial</h2>
            </div>
            {commercial && (
              <span
                className={commercial.read_only ? "badge badge-warning" : "badge badge-success"}
              >
                {commercial.read_only ? "Solo lectura" : "Operación habilitada"}
              </span>
            )}
          </header>

          {commercial ? (
            <div className="surface-body">
              <dl className="metric-grid metric-grid-3">
                <div>
                  <dt>Plan</dt>
                  <dd>{PLAN_LABELS[commercial.plan] ?? commercial.plan}</dd>
                </div>
                <div>
                  <dt>Estado</dt>
                  <dd>{commercial.status.replace("_", " ")}</dd>
                </div>
                <div>
                  <dt>Vigente hasta</dt>
                  <dd className="metric-text">{formatDate(commercial.valid_until)}</dd>
                </div>
              </dl>
              <div>
                <p className="eyebrow">Funciones habilitadas</p>
                {commercial.features.length === 0 ? (
                  <p className="muted">Sin funciones listadas.</p>
                ) : (
                  <ul className="tag-list">
                    {commercial.features.map((feature) => (
                      <li key={feature} className="tag">
                        {feature}
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          ) : (
            <p className="muted empty-cell">Sin información comercial disponible.</p>
          )}
        </section>

        {canBackup && (
          <section className="surface" aria-label="Respaldos">
            <header className="surface-header">
              <div>
                <p className="eyebrow">Respaldos</p>
                <h2>Copia de seguridad</h2>
              </div>
            </header>
            <p className="muted">
              Genera un respaldo consistente de la base local y lo registra en auditoría.
            </p>
            <button type="button" className="button button-primary" onClick={() => void backup()} disabled={busy}>
              {busy ? "Creando respaldo…" : "Crear respaldo"}
            </button>
          </section>
        )}
      </div>

      {canAudit && (
        <section className="surface" aria-label="Auditoría">
          <header className="surface-header">
            <div>
              <p className="eyebrow">Auditoría</p>
              <h2>Últimos eventos</h2>
            </div>
            <span className="badge badge-neutral">{audit.length}</span>
          </header>

          {loading ? (
            <p className="muted empty-cell">Cargando eventos…</p>
          ) : audit.length === 0 ? (
            <div className="empty-state">
              <p>Sin eventos registrados.</p>
              <p className="muted">Las operaciones auditables aparecerán aquí.</p>
            </div>
          ) : (
            <ol className="audit-list">
              {audit.map((event) => (
                <li key={event.id}>
                  <span className="audit-action mono">{event.action}</span>
                  <span className="audit-meta">
                    {event.entity_type} {event.entity_id.slice(0, 8)} ·{" "}
                    {formatDate(event.occurred_at)}
                  </span>
                </li>
              ))}
            </ol>
          )}
        </section>
      )}
    </div>
  );
}
