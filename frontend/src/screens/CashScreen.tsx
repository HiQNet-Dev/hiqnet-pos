import { useEffect, useState } from "react";
import { api, IpcError } from "../api";
import type { Actor, CashRegister, CashMovementDraft, CashShift } from "../contracts";

interface Props {
  actor: Actor;
  readOnly: boolean;
  registers: CashRegister[];
  registerId: string;
  onRegisterChange: (registerId: string) => void;
  shift: CashShift | null;
  onShiftChanged: () => void;
  onRegisterCreated: () => void;
}

function formatDifference(minor: number | null): string {
  if (minor === null) return "—";
  if (minor === 0) return "Exacto";
  const absolute = Math.abs(minor);
  const amount = `$${Math.floor(absolute / 100)}.${String(absolute % 100).padStart(2, "0")}`;
  return minor > 0 ? `Sobrante ${amount}` : `Faltante ${amount}`;
}

function formatTimestamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("es-MX", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(date);
}

function formatDuration(from: string, to?: string | null): string {
  const start = new Date(from).getTime();
  const end = to ? new Date(to).getTime() : Date.now();
  if (Number.isNaN(start) || Number.isNaN(end) || end < start) return "—";
  const minutes = Math.round((end - start) / 60000);
  if (minutes < 60) return `${minutes} min`;
  const hours = Math.floor(minutes / 60);
  return `${hours} h ${minutes % 60} min`;
}

export function CashScreen({
  actor,
  readOnly,
  registers,
  registerId,
  onRegisterChange,
  shift,
  onShiftChanged,
  onRegisterCreated,
}: Props) {
  const canManage = actor.permissions.includes("cash.register.manage");
  const canOpen = actor.permissions.includes("cash.shift.open");
  const canClose = actor.permissions.includes("cash.shift.close");
  const canMove = actor.permissions.includes("cash.movement.record");

  const [history, setHistory] = useState<CashShift[]>([]);
  const [lastClosed, setLastClosed] = useState<CashShift | null>(null);
  const [registerName, setRegisterName] = useState("");
  const [registerCode, setRegisterCode] = useState("");
  const [openingAmount, setOpeningAmount] = useState("");
  const [countedAmount, setCountedAmount] = useState("");
  const [kind, setKind] = useState<"in" | "out">("in");
  const [amount, setAmount] = useState("");
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .listCashShiftHistory()
      .then((items) => {
        if (!cancelled) setHistory(items);
      })
      .catch(() => {
        // El historial es complementario: un fallo no bloquea la operación.
      });
    return () => {
      cancelled = true;
    };
  }, [shift?.id]);

  const register = registers.find((item) => item.id === registerId) ?? null;

  async function createRegister(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canManage || readOnly) return;
    setBusy(true);
    setError(null);
    try {
      await api.createCashRegister({ name: registerName, code: registerCode });
      setRegisterName("");
      setRegisterCode("");
      onRegisterCreated();
      setNotice("Caja creada. Registra el fondo inicial para abrir el turno.");
    } catch (cause) {
      setError(cause instanceof IpcError ? cause.message : "No fue posible crear la caja.");
    } finally {
      setBusy(false);
    }
  }

  async function openShift(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canOpen || readOnly || !registerId) return;
    setBusy(true);
    setError(null);
    try {
      await api.openCashShift({ register_id: registerId, opening_amount: openingAmount });
      setOpeningAmount("");
      setLastClosed(null);
      setNotice("Turno abierto.");
      onShiftChanged();
    } catch (cause) {
      setError(cause instanceof IpcError ? cause.message : "No fue posible abrir el turno.");
    } finally {
      setBusy(false);
    }
  }

  async function addMovement(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canMove || readOnly || !shift) return;
    setBusy(true);
    setError(null);
    try {
      const draft: CashMovementDraft = { shift_id: shift.id, kind, amount, reason };
      await api.registerCashMovement(draft);
      setAmount("");
      setReason("");
      setNotice("Movimiento registrado.");
      onShiftChanged();
    } catch (cause) {
      setError(
        cause instanceof IpcError ? cause.message : "No fue posible registrar el movimiento.",
      );
    } finally {
      setBusy(false);
    }
  }

  async function closeShift(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canClose || readOnly || !shift) return;
    setBusy(true);
    setError(null);
    try {
      const closed = await api.closeCashShift({
        shift_id: shift.id,
        counted_amount: countedAmount,
      });
      setLastClosed(closed);
      setCountedAmount("");
      setHistory(await api.listCashShiftHistory());
      setNotice(`Turno cerrado. Diferencia: ${formatDifference(closed.difference_minor)}.`);
      onShiftChanged();
    } catch (cause) {
      setError(cause instanceof IpcError ? cause.message : "No fue posible cerrar el turno.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="screen cash-screen">
      <header className="screen-header">
        <div>
          <h1>Caja y turnos</h1>
          <p className="screen-subtitle">
            Fondo inicial más movimientos manuales; las ventas en efectivo se suman al cobrar.
          </p>
        </div>
        <div className="screen-actions">
          {registers.length > 0 && (
            <label className="field field-inline">
              <span>Caja</span>
              <select
                value={registerId}
                onChange={(event) => onRegisterChange(event.target.value)}
              >
                {registers.map((item) => (
                  <option key={item.id} value={item.id}>
                    {item.name} · {item.code}
                  </option>
                ))}
              </select>
            </label>
          )}
          <span className={shift ? "badge badge-success" : "badge badge-neutral"}>
            {shift ? "Turno abierto" : "Sin turno"}
          </span>
        </div>
      </header>

      {(error || notice) && (
        <div aria-live="polite" className="screen-inline-alert">
          {error && <p className="alert alert-error">{error}</p>}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      {registers.length === 0 && canManage && !readOnly && (
        <section className="surface">
          <header className="surface-header">
            <div>
              <p className="eyebrow">Configuración inicial</p>
              <h2>Crea la caja principal</h2>
            </div>
          </header>
          <form className="inline-form" onSubmit={createRegister} noValidate>
            <label className="field">
              <span>Nombre</span>
              <input
                value={registerName}
                onChange={(event) => setRegisterName(event.target.value)}
                required
              />
            </label>
            <label className="field field-short">
              <span>Código</span>
              <input
                value={registerCode}
                onChange={(event) => setRegisterCode(event.target.value)}
                maxLength={16}
                required
              />
            </label>
            <button type="submit" className="button button-primary" disabled={busy}>
              Crear caja
            </button>
          </form>
        </section>
      )}

      {registers.length > 0 && !shift && canOpen && !readOnly && (
        <section className="surface attention">
          <header className="surface-header">
            <div>
              <p className="eyebrow">Apertura</p>
              <h2>Abrir turno</h2>
            </div>
            <span className="badge badge-neutral">
              {register ? `${register.name} · ${register.code}` : "Caja"}
            </span>
          </header>
          <form className="inline-form" onSubmit={openShift} noValidate>
            <label className="field">
              <span>Fondo inicial</span>
              <input
                inputMode="decimal"
                value={openingAmount}
                onChange={(event) => setOpeningAmount(event.target.value)}
                placeholder="0.00"
                required
              />
            </label>
            <button type="submit" className="button button-primary" disabled={busy}>
              Abrir turno
            </button>
          </form>
          <p className="hint">
            El saldo esperado se calcula en Rust: fondo inicial + entradas − salidas.
          </p>
        </section>
      )}

      <div className="cash-layout">
        <section className="surface" aria-label="Operación del turno">
          {shift && (
            <>
              <header className="surface-header">
                <div>
                  <p className="eyebrow">Turno en curso</p>
                  <h2>{register ? `${register.name} · ${register.code}` : "Caja"}</h2>
                </div>
                <span className="badge badge-success">
                  Abierto hace {formatDuration(shift.opened_at)}
                </span>
              </header>

              <dl className="metric-grid">
                <div>
                  <dt>Fondo inicial</dt>
                  <dd>${shift.opening_amount}</dd>
                </div>
                <div>
                  <dt>Entradas</dt>
                  <dd>${shift.entry_total}</dd>
                </div>
                <div>
                  <dt>Salidas</dt>
                  <dd>${shift.exit_total}</dd>
                </div>
                <div className="metric-strong">
                  <dt>Saldo esperado</dt>
                  <dd>${shift.expected_amount}</dd>
                </div>
              </dl>

              {canMove && !readOnly && (
                <form className="inline-form" onSubmit={addMovement} noValidate>
                  <label className="field field-short">
                    <span>Tipo</span>
                    <select
                      value={kind}
                      onChange={(event) => setKind(event.target.value as "in" | "out")}
                    >
                      <option value="in">Entrada</option>
                      <option value="out">Salida</option>
                    </select>
                  </label>
                  <label className="field field-short">
                    <span>Importe</span>
                    <input
                      inputMode="decimal"
                      value={amount}
                      onChange={(event) => setAmount(event.target.value)}
                      required
                    />
                  </label>
                  <label className="field">
                    <span>Motivo</span>
                    <input
                      value={reason}
                      onChange={(event) => setReason(event.target.value)}
                      required
                    />
                  </label>
                  <button type="submit" className="button" disabled={busy}>
                    Registrar
                  </button>
                </form>
              )}

              <div className="movement-list">
                <p className="eyebrow">Movimientos del turno</p>
                {shift.movements.length === 0 ? (
                  <p className="muted">Sin movimientos manuales.</p>
                ) : (
                  <ol className="ledger">
                    {shift.movements.map((movement) => (
                      <li key={movement.id}>
                        <span
                          className={
                            movement.kind === "in"
                              ? "ledger-kind ledger-in"
                              : "ledger-kind ledger-out"
                          }
                        >
                          {movement.kind === "in" ? "Entrada" : "Salida"}
                        </span>
                        <span className="ledger-quantity mono">${movement.amount}</span>
                        <span className="ledger-reason">{movement.reason}</span>
                        <span className="ledger-date">
                          {formatTimestamp(movement.occurred_at)}
                        </span>
                      </li>
                    ))}
                  </ol>
                )}
              </div>

              {canClose && !readOnly && (
                <form className="close-panel" onSubmit={closeShift} noValidate>
                  <div>
                    <p className="eyebrow">Arqueo</p>
                    <h3>Cerrar turno</h3>
                    <p className="muted">
                      Cuenta el efectivo físico. La diferencia se registra tal cual.
                    </p>
                  </div>
                  <label className="field">
                    <span>Efectivo contado</span>
                    <input
                      inputMode="decimal"
                      value={countedAmount}
                      onChange={(event) => setCountedAmount(event.target.value)}
                      required
                    />
                  </label>
                  <button type="submit" className="button button-primary" disabled={busy}>
                    Cerrar turno
                  </button>
                </form>
              )}
            </>
          )}

          {!shift && lastClosed && (
            <>
              <header className="surface-header">
                <div>
                  <p className="eyebrow">Turno cerrado</p>
                  <h2>Resultado del arqueo</h2>
                </div>
                <span className="badge badge-neutral">
                  {formatTimestamp(lastClosed.closed_at ?? lastClosed.opened_at)}
                </span>
              </header>
              <dl className="metric-grid">
                <div>
                  <dt>Esperado</dt>
                  <dd>${lastClosed.expected_amount}</dd>
                </div>
                <div>
                  <dt>Contado</dt>
                  <dd>${lastClosed.counted_amount ?? "—"}</dd>
                </div>
                <div className="metric-strong">
                  <dt>Diferencia</dt>
                  <dd>{formatDifference(lastClosed.difference_minor)}</dd>
                </div>
              </dl>
            </>
          )}

          {!shift && !lastClosed && (
            <p className="muted empty-cell">
              {canOpen && !readOnly
                ? "Abre un turno para ver el resumen y los movimientos."
                : "No hay turno abierto en esta caja."}
            </p>
          )}
        </section>

        <section className="surface" aria-label="Historial de turnos">
          <header className="surface-header">
            <div>
              <p className="eyebrow">Historial</p>
              <h2>Turnos cerrados</h2>
            </div>
            <span className="badge badge-neutral">{history.length}</span>
          </header>
          {history.length === 0 ? (
            <p className="muted empty-cell">Aún no hay turnos cerrados.</p>
          ) : (
            <ol className="ledger">
              {history.map((item) => (
                <li key={item.id} className="ledger-row">
                  <span className="ledger-main">
                    <span className="ledger-label">
                      {registerLabel(registers, item.register_id)}
                    </span>
                    <span className="ledger-sub">Contado ${item.counted_amount ?? "—"}</span>
                    <span className="ledger-sub ledger-sub-date">
                      {formatTimestamp(item.closed_at ?? item.opened_at)}
                    </span>
                  </span>
                  <span className="ledger-side">
                    <span className="mono">${item.expected_amount}</span>
                    <span
                      className={
                        item.difference_minor === 0 ? "ledger-diff" : "ledger-diff ledger-diff-alert"
                      }
                    >
                      {formatDifference(item.difference_minor)}
                    </span>
                  </span>
                </li>
              ))}
            </ol>
          )}
        </section>
      </div>
    </div>
  );
}

function registerLabel(registers: CashRegister[], registerId: string): string {
  const register = registers.find((item) => item.id === registerId);
  return register ? `${register.name} · ${register.code}` : registerId.slice(0, 8);
}
