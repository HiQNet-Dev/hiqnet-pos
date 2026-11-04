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

const STATUS_LABELS: Record<string, string> = {
  trial: "Prueba",
  active: "Activa",
  grace: "Periodo de gracia",
  read_only: "Solo lectura",
  expired: "Vencida",
};

const FEATURE_LABELS: Record<string, string> = {
  catalog: "Catálogo",
  sales: "Ventas",
  inventory: "Inventario",
  purchasing: "Compras",
  expenses: "Gastos",
  reports: "Reportes",
  local_backups: "Respaldos locales",
  cloud_sync: "Sincronización cloud",
  remote_admin: "Administración remota",
  enterprise_integrations: "Integraciones enterprise",
};

const ACTION_LABELS: Record<string, string> = {
  "installation.bootstrapped": "Instalación configurada",
  "session.logged_in": "Inicio de sesión",
  "session.login_rejected": "Inicio de sesión rechazado",
  "catalog.product_created": "Producto creado",
  "catalog.category_created": "Categoría creada",
  "catalog.variant_created": "Variante creada",
  "catalog.product_category_changed": "Categoría de producto actualizada",
  "catalog.modifier_group_created": "Grupo de modificadores creado",
  "catalog.modifier_option_created": "Opción de modificador creada",
  "cash.register_created": "Caja creada",
  "cash.shift_opened": "Turno abierto",
  "cash.movement_registered": "Movimiento de caja registrado",
  "cash.shift_closed": "Turno cerrado",
  "sales.completed": "Venta confirmada",
  "inventory.movement_registered": "Movimiento de inventario registrado",
  "backup.created": "Respaldo creado",
};

const ENTITY_LABELS: Record<string, string> = {
  sale: "Venta",
  inventory_movement: "Movimiento de inventario",
  cash_shift: "Turno de caja",
  cash_register: "Caja",
  product: "Producto",
  category: "Categoría",
  modifier_group: "Grupo de modificadores",
  modifier_option: "Opción de modificador",
  session: "Sesión",
  organization: "Organización",
  backup: "Respaldo",
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
                  <dd>{STATUS_LABELS[commercial.status] ?? commercial.status}</dd>
                </div>
                <div>
                  <dt>Vigente hasta</dt>
                  <dd className="metric-text">{formatDate(commercial.valid_until)}</dd>
                </div>
              </dl>
              <div>
                <p className="section-label">Funciones habilitadas</p>
                {commercial.features.length === 0 ? (
                  <p className="muted">Sin funciones listadas.</p>
                ) : (
                  <ul className="tag-list">
                    {commercial.features.map((feature) => (
                      <li key={feature} className="tag">
                        {FEATURE_LABELS[feature] ?? feature}
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
                <h2>Copia de seguridad</h2>
                <p className="muted">Respaldo consistente de la base local</p>
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
              <h2>Últimos eventos</h2>
              <p className="muted">Operaciones auditables de la sucursal</p>
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
                  <span className="audit-action">
                    {ACTION_LABELS[event.action] ?? event.action}
                  </span>
                  <span className="audit-meta">
                    {ENTITY_LABELS[event.entity_type] ?? event.entity_type}{" "}
                    {event.entity_id.slice(0, 8)} · {formatDate(event.occurred_at)}
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
