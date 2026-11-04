import type { Actor, CashRegister, CashShift, CommercialState } from "../contracts";
import type { BranchSummary } from "../contracts";
import { CashIcon, LogoutIcon } from "./icons";

interface Props {
  actor: Actor;
  branch: BranchSummary | null;
  register: CashRegister | null;
  shift: CashShift | null;
  commercial: CommercialState | null;
  onLogout: () => void;
  onOpenCash: () => void;
  canOpenCash: boolean;
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
          <button type="button" className="chip chip-success" onClick={onOpenCash}>
            <CashIcon size={14} />
            {register ? `${register.name} · ${register.code}` : "Caja"} · turno abierto
          </button>
        ) : canOpenCash ? (
          <button type="button" className="chip chip-muted" onClick={onOpenCash}>
            <CashIcon size={14} />
            Sin turno de caja
          </button>
        ) : null}

        {commercial && (
          <span
            className={commercial.read_only ? "badge badge-warning" : "badge badge-neutral"}
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
