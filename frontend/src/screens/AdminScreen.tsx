import { useCallback, useEffect, useState } from "react";
import { api, IpcError } from "../api";
import {
  userDraftSchema,
  type Actor,
  type AuditEvent,
  type CommercialState,
  type User,
} from "../contracts";
import { Modal } from "../components/Modal";

interface Props {
  actor: Actor;
  commercial: CommercialState | null;
  readOnly: boolean;
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("es-MX", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

function messageOf(cause: unknown, fallback: string): string {
  return cause instanceof IpcError ? cause.message : fallback;
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
  "backups.local": "Respaldos locales",
  cloud_sync: "Sincronización cloud",
  remote_admin: "Administración remota",
  enterprise_integrations: "Integraciones enterprise",
};

const ROLE_LABELS: Record<string, string> = {
  owner: "Propietario",
  cashier: "Cajero",
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
  "users.created": "Usuario creado",
  "users.deactivated": "Usuario desactivado",
  "users.reactivated": "Usuario reactivado",
  "users.password_reset": "Contraseña restablecida",
  "users.password_changed": "Contraseña actualizada",
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
  user: "Usuario",
};

export function AdminScreen({ actor, commercial, readOnly }: Props) {
  const canAudit = actor.permissions.includes("audit.view");
  const canBackup = actor.permissions.includes("backups.manage");
  const canManageUsers = actor.permissions.includes("users.manage");

  const [audit, setAudit] = useState<AuditEvent[]>([]);
  const [auditLoading, setAuditLoading] = useState(canAudit);
  const [users, setUsers] = useState<User[]>([]);
  const [usersLoading, setUsersLoading] = useState(canManageUsers);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [creating, setCreating] = useState(false);
  const [resetting, setResetting] = useState<User | null>(null);
  const [confirming, setConfirming] = useState<{ user: User; active: boolean } | null>(null);

  const loadAudit = useCallback(async () => {
    if (!canAudit) return;
    try {
      setAudit(await api.listAuditEvents(50));
    } catch (cause) {
      setError(messageOf(cause, "No fue posible cargar la auditoría."));
    } finally {
      setAuditLoading(false);
    }
  }, [canAudit]);

  useEffect(() => {
    let cancelled = false;
    if (!canAudit) return;
    api
      .listAuditEvents(50)
      .then((events) => {
        if (!cancelled) setAudit(events);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(messageOf(cause, "No fue posible cargar la auditoría."));
        }
      })
      .finally(() => {
        if (!cancelled) setAuditLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [canAudit]);

  useEffect(() => {
    let cancelled = false;
    if (!canManageUsers) return;
    api
      .listUsers()
      .then((list) => {
        if (!cancelled) setUsers(list);
      })
      .catch((cause: unknown) => {
        if (!cancelled) setError(messageOf(cause, "No fue posible cargar los usuarios."));
      })
      .finally(() => {
        if (!cancelled) setUsersLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [canManageUsers]);

  async function refresh() {
    if (canManageUsers) setUsers(await api.listUsers());
    await loadAudit();
  }

  async function backup() {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const path = await api.backupDatabase();
      setNotice(`Respaldo creado en ${path}`);
      await loadAudit();
    } catch (cause) {
      setError(messageOf(cause, "No fue posible crear el respaldo."));
    } finally {
      setBusy(false);
    }
  }

  async function toggleActive(user: User, active: boolean) {
    setError(null);
    try {
      await api.setUserActive(user.id, active);
      setNotice(
        active
          ? `${user.display_name} puede volver a iniciar sesión.`
          : `${user.display_name} ya no puede iniciar sesión.`,
      );
      setConfirming(null);
      await refresh();
    } catch (cause) {
      setError(messageOf(cause, "No fue posible actualizar la cuenta."));
      setConfirming(null);
    }
  }

  return (
    <div className="screen admin-screen">
      <header className="screen-header">
        <div>
          <h1>Gestión y auditoría</h1>
          <p className="screen-subtitle">
            Empleados, estado comercial, respaldos y registro de eventos.
          </p>
        </div>
      </header>

      {(error || notice) && (
        <div aria-live="polite" className="screen-inline-alert">
          {error && <p className="alert alert-error">{error}</p>}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      {canManageUsers && (
        <section className="surface" aria-label="Usuarios">
          <header className="surface-header">
            <div>
              <h2>Usuarios</h2>
              <p className="muted">
                {usersLoading
                  ? "Cargando cuentas…"
                  : `${users.length} ${users.length === 1 ? "cuenta" : "cuentas"} en la sucursal`}
              </p>
            </div>
            <button
              type="button"
              className="button button-primary"
              onClick={() => setCreating(true)}
              disabled={readOnly}
            >
              Nuevo usuario
            </button>
          </header>

          {usersLoading ? (
            <p className="muted empty-cell">Cargando usuarios…</p>
          ) : users.length === 0 ? (
            <div className="empty-state">
              <p>Sin cuentas registradas.</p>
              <p className="muted">Crea la primera cuenta con “Nuevo usuario”.</p>
            </div>
          ) : (
            <div className="table-scroll">
              <table className="data-table">
                <caption className="visually-hidden">Empleados de la sucursal</caption>
                <thead>
                  <tr>
                    <th scope="col">Empleado</th>
                    <th scope="col">Usuario</th>
                    <th scope="col">Rol</th>
                    <th scope="col">Estado</th>
                    <th scope="col" className="align-end">
                      Acciones
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {users.map((user) => (
                    <tr key={user.id}>
                      <td>
                        {user.display_name}
                        {user.id === actor.user_id && (
                          <span className="badge badge-neutral user-self">Tú</span>
                        )}
                      </td>
                      <td className="mono">{user.username}</td>
                      <td>{ROLE_LABELS[user.role] ?? user.role}</td>
                      <td>
                        <span
                          className={
                            user.active ? "badge badge-success" : "badge badge-neutral"
                          }
                        >
                          {user.active ? "Activo" : "Inactivo"}
                        </span>
                      </td>
                      <td className="align-end">
                        <div className="row-actions">
                          <button
                            type="button"
                            className="button button-quiet button-small"
                            onClick={() => setResetting(user)}
                            disabled={readOnly}
                          >
                            Contraseña
                          </button>
                          <button
                            type="button"
                            className="button button-quiet button-small"
                            onClick={() => setConfirming({ user, active: !user.active })}
                            disabled={readOnly || (user.active && user.id === actor.user_id)}
                          >
                            {user.active ? "Desactivar" : "Activar"}
                          </button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          <p className="surface-note">
            Desactivar una cuenta revoca su acceso de inmediato, incluso si tenía una
            sesión abierta. Siempre debe quedar al menos un propietario activo.
          </p>
        </section>
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
            <button
              type="button"
              className="button button-primary"
              onClick={() => void backup()}
              disabled={busy}
            >
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

          {auditLoading ? (
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

      {creating && (
        <CreateUserDialog
          onClose={() => setCreating(false)}
          onCreated={async (name) => {
            setCreating(false);
            setNotice(`Cuenta de ${name} creada.`);
            setError(null);
            await refresh();
          }}
          onError={(message) => setError(message)}
        />
      )}

      {resetting && (
        <ResetPasswordDialog
          user={resetting}
          onClose={() => setResetting(null)}
          onDone={async (name) => {
            setResetting(null);
            setNotice(`Contraseña de ${name} restablecida.`);
            setError(null);
            await refresh();
          }}
          onError={(message) => setError(message)}
        />
      )}

      {confirming && (
        <ConfirmUserDialog
          user={confirming.user}
          active={confirming.active}
          onClose={() => setConfirming(null)}
          onConfirm={() => void toggleActive(confirming.user, confirming.active)}
        />
      )}
    </div>
  );
}

interface CreateUserDialogProps {
  onClose: () => void;
  onCreated: (displayName: string) => Promise<void> | void;
  onError: (message: string) => void;
}

function CreateUserDialog({ onClose, onCreated, onError }: CreateUserDialogProps) {
  const [displayName, setDisplayName] = useState("");
  const [username, setUsername] = useState("");
  const [role, setRole] = useState<"cashier" | "owner">("cashier");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const parsed = userDraftSchema.safeParse({
      username,
      display_name: displayName,
      role,
      password,
    });
    if (!parsed.success) {
      setLocalError(parsed.error.issues[0]?.message ?? "Revisa los datos.");
      return;
    }
    setBusy(true);
    try {
      await api.createUser(parsed.data);
      await onCreated(parsed.data.display_name);
    } catch (cause) {
      onError(messageOf(cause, "No fue posible crear el usuario."));
      onClose();
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      title="Nuevo usuario"
      subtitle="La cuenta pertenece a la sucursal activa."
      onClose={onClose}
      footer={
        <>
          <button type="button" className="button" onClick={onClose} disabled={busy}>
            Cancelar
          </button>
          <button
            type="submit"
            form="user-form"
            className="button button-primary"
            disabled={busy}
          >
            Crear cuenta
          </button>
        </>
      }
    >
      <form id="user-form" className="form-grid" onSubmit={submit} noValidate>
        <label className="field">
          <span>Nombre del empleado</span>
          <input
            value={displayName}
            onChange={(event) => setDisplayName(event.target.value)}
            autoFocus
            required
          />
        </label>
        <div className="field-row">
          <label className="field">
            <span>Usuario</span>
            <input
              value={username}
              onChange={(event) => setUsername(event.target.value)}
              spellCheck={false}
              autoComplete="off"
              required
            />
          </label>
          <label className="field field-short">
            <span>Rol</span>
            <select
              value={role}
              onChange={(event) => setRole(event.target.value as "cashier" | "owner")}
            >
              <option value="cashier">Cajero</option>
              <option value="owner">Propietario</option>
            </select>
          </label>
        </div>
        <label className="field">
          <span>Contraseña inicial</span>
          <input
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            autoComplete="new-password"
            required
          />
        </label>
        <p className="hint">
          Mínimo 10 caracteres. Compártela con el empleado; podrá cambiarla al
          iniciar sesión.
        </p>
        {localError && (
          <p className="alert alert-error" role="alert">
            {localError}
          </p>
        )}
      </form>
    </Modal>
  );
}

interface ResetPasswordDialogProps {
  user: User;
  onClose: () => void;
  onDone: (displayName: string) => Promise<void> | void;
  onError: (message: string) => void;
}

function ResetPasswordDialog({ user, onClose, onDone, onError }: ResetPasswordDialogProps) {
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  return (
    <Modal
      title="Restablecer contraseña"
      subtitle={`${user.display_name} · ${user.username}`}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="button" onClick={onClose} disabled={busy}>
            Cancelar
          </button>
          <button
            type="submit"
            form="reset-password-form"
            className="button button-primary"
            disabled={busy}
          >
            Restablecer
          </button>
        </>
      }
    >
      <form
        id="reset-password-form"
        className="form-grid"
        onSubmit={async (event) => {
          event.preventDefault();
          if (password.length < 10) {
            setLocalError("La contraseña debe tener al menos 10 caracteres.");
            return;
          }
          setBusy(true);
          try {
            await api.resetUserPassword(user.id, password);
            await onDone(user.display_name);
          } catch (cause) {
            onError(messageOf(cause, "No fue posible restablecer la contraseña."));
            onClose();
          } finally {
            setBusy(false);
          }
        }}
        noValidate
      >
        <label className="field">
          <span>Nueva contraseña</span>
          <input
            type="password"
            value={password}
            onChange={(event) => {
              setPassword(event.target.value);
              setLocalError(null);
            }}
            autoComplete="new-password"
            autoFocus
            required
          />
        </label>
        <p className="hint">
          La contraseña anterior deja de funcionar de inmediato. No se muestra ni se
          registra en auditoría.
        </p>
        {localError && (
          <p className="alert alert-error" role="alert">
            {localError}
          </p>
        )}
      </form>
    </Modal>
  );
}

interface ConfirmUserDialogProps {
  user: User;
  active: boolean;
  onClose: () => void;
  onConfirm: () => void;
}

function ConfirmUserDialog({ user, active, onClose, onConfirm }: ConfirmUserDialogProps) {
  return (
    <Modal
      title={active ? "Activar cuenta" : "Desactivar cuenta"}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="button" onClick={onClose}>
            Cancelar
          </button>
          <button type="button" className="button button-primary" onClick={onConfirm}>
            {active ? "Activar" : "Desactivar"}
          </button>
        </>
      }
    >
      <p>
        {active
          ? `${user.display_name} podrá iniciar sesión de nuevo con su contraseña actual.`
          : `${user.display_name} perderá el acceso de inmediato, incluso si tiene una sesión abierta. Su historial de ventas se conserva.`}
      </p>
    </Modal>
  );
}
