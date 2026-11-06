import type { Actor, CashRegister, CashShift, CommercialState } from "../contracts";
import type { BranchSummary } from "../contracts";
import { CashIcon, KeyIcon, LogoutIcon } from "./icons";

interface Props {
  actor: Actor;
  branch: BranchSummary | null;
  register: CashRegister | null;
  shift: CashShift | null;
  commercial: CommercialState | null;
  onLogout: () => void;
  onOpenCash: () => void;
  canOpenCash: boolean;
  onChangePassword: () => void;
}

const ROLE_LABELS: Record<string, string> = {
  owner: "Propietario",
  cashier: "Cajero",
};

function planLabel(plan: string): string {
  switch (plan) {
    case "trial":
      return "Trial";
    case "local":
      return "Local";
    case "pro":
      return "Pro";
    case "enterprise":
      return "Enterprise";
    default:
      return plan;
  }
}

export function TopBar({
  actor,
  branch,
  register,
  shift,
  commercial,
  onLogout,
  onOpenCash,
  canOpenCash,
  onChangePassword,
}: Props) {
  return (
    <header className="topbar">
      <div className="topbar-brand">
        <span className="brand-mark" aria-hidden="true">
          HQ
        </span>
        <div className="brand-text">
          <span className="brand-name">HiQNet POS</span>
          <span className="brand-branch">
            {branch ? `${branch.name} · ${branch.code}` : "Sucursal activa"}
          </span>
        </div>
      </div>

      <div className="topbar-spacer" />

      <div className="topbar-meta">
        <span className="connection" title="Operación local, sin dependencia de red">
          <span className="connection-dot" aria-hidden="true" />
          Local
        </span>

        {shift ? (
          <button
            type="button"
            className="chip chip-success topbar-cash"
            onClick={onOpenCash}
            aria-label={`${register ? `${register.name} · ${register.code}` : "Caja"} · turno abierto. Ir a caja y turnos`}
          >
            <CashIcon size={14} />
            <span className="topbar-cash-label" aria-hidden="true">
              {register ? `${register.name} · ${register.code}` : "Caja"} · turno abierto
            </span>
          </button>
        ) : canOpenCash ? (
          <button
            type="button"
            className="chip chip-muted topbar-cash"
            onClick={onOpenCash}
            aria-label="Sin turno de caja. Ir a caja y turnos"
          >
            <CashIcon size={14} />
            <span className="topbar-cash-label" aria-hidden="true">
              Sin turno de caja
            </span>
          </button>
        ) : null}

        {commercial && (
          <span
            className={`${commercial.read_only ? "badge badge-warning" : "badge badge-neutral"} topbar-plan`}
            title={`Vigente hasta ${commercial.valid_until}`}
          >
            {commercial.read_only ? "Solo lectura" : `Plan ${planLabel(commercial.plan)}`}
          </span>
        )}

        <div className="topbar-user">
          <span className="topbar-user-name">{actor.display_name}</span>
          <span className="topbar-user-role">
            {ROLE_LABELS[actor.role] ?? actor.role}
          </span>
        </div>

        <button
          type="button"
          className="icon-button"
          onClick={onChangePassword}
          aria-label="Cambiar mi contraseña"
          title="Cambiar mi contraseña"
        >
          <KeyIcon />
        </button>

        <button
          type="button"
          className="icon-button"
          onClick={onLogout}
          aria-label="Cerrar sesión"
          title="Cerrar sesión"
        >
          <LogoutIcon />
        </button>
      </div>
    </header>
  );
}
