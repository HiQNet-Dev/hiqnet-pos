import { useEffect, useState } from "react";
import { api, IpcError } from "../api";
import type { Actor, CashRegister, CashMovementDraft, CashShift } from "../contracts";

interface Props {
  actor: Actor;
  readOnly: boolean;
}

export function CashPanel({ actor, readOnly }: Props) {
  const canManage = actor.permissions.includes("cash.register.manage");
  const canOpen = actor.permissions.includes("cash.shift.open");
  const canClose = actor.permissions.includes("cash.shift.close");
  const canMove = actor.permissions.includes("cash.movement.record");
  const canView = actor.permissions.includes("cash.shift.view");

  const [registers, setRegisters] = useState<CashRegister[]>([]);
  const [registerId, setRegisterId] = useState("");
  const [shift, setShift] = useState<CashShift | null>(null);
  const [openingAmount, setOpeningAmount] = useState("");
  const [countedAmount, setCountedAmount] = useState("");
  const [kind, setKind] = useState<"in" | "out">("in");
  const [amount, setAmount] = useState("");
  const [reason, setReason] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [registerName, setRegisterName] = useState("");
  const [registerCode, setRegisterCode] = useState("");

  useEffect(() => {
    let cancelled = false;
    if (!canView) return;
    api
      .listCashRegisters()
      .then((list) => {
        if (cancelled) return;
        setRegisters(list);
        const first = list[0];
        if (first) {
          setRegisterId((current) => current || first.id);
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(cause instanceof IpcError ? cause.message : "No fue posible cargar las cajas.");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [canView]);

  useEffect(() => {
    if (!registerId) return;
    let cancelled = false;
    api
      .activeCashShift(registerId)
      .then((value) => {
        if (!cancelled) setShift(value);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(cause instanceof IpcError ? cause.message : "No fue posible cargar el turno.");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [registerId]);

  async function createRegister(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canManage || readOnly) return;
    setBusy(true);
    setError(null);
    try {
      const created = await api.createCashRegister({ name: registerName, code: registerCode });
      setRegisters((current) => [...current, created]);
      setRegisterId(created.id);
      setRegisterName("");
      setRegisterCode("");
      setNotice("Caja creada.");
    } catch (cause) {
      setError(cause instanceof IpcError ? cause.message : "No fue posible crear la caja.");
    } finally {
      setBusy(false);
    }
  }

  async function openShift(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canOpen || readOnly) return;
    setBusy(true);
    setError(null);
    try {
      const created = await api.openCashShift({ register_id: registerId, opening_amount: openingAmount });
      setShift(created);
      setOpeningAmount("");
      setNotice("Turno abierto.");
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
      const draft: CashMovementDraft = {
        shift_id: shift.id,
        kind,
        amount,
        reason,
      };
      await api.registerCashMovement(draft);
      const refreshed = await api.activeCashShift(registerId);
      setShift(refreshed);
      setAmount("");
      setReason("");
      setNotice("Movimiento registrado.");
    } catch (cause) {
      setError(cause instanceof IpcError ? cause.message : "No fue posible registrar el movimiento.");
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
      const closed = await api.closeCashShift({ shift_id: shift.id, counted_amount: countedAmount });
      setShift(closed);
      setCountedAmount("");
      setNotice(
        `Turno cerrado. Diferencia: ${closed.difference_minor ?? 0} centavos.`,
      );
    } catch (cause) {
      setError(cause instanceof IpcError ? cause.message : "No fue posible cerrar el turno.");
    } finally {
      setBusy(false);
    }
  }

  if (!canView) {
    return null;
  }

  return (
    <section className="panel" aria-labelledby="cash-title">
      <header className="panel-header">
        <div>
          <p className="eyebrow">Caja</p>
          <h2 id="cash-title">Turno de efectivo</h2>
        </div>
        {shift && <span className="badge badge-neutral">{shift.status}</span>}
      </header>

      {(error || notice) && (
        <div aria-live="polite">
          {error && <p className="alert alert-error">{error}</p>}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      {registers.length === 0 && canManage && !readOnly && (
        <form className="form-grid form-compact" onSubmit={createRegister} noValidate>
          <p className="muted">Crea la caja principal.</p>
          <div className="field-row">
            <label className="field">
              <span>Nombre</span>
              <input value={registerName} onChange={(event) => setRegisterName(event.target.value)} required />
            </label>
            <label className="field field-short">
              <span>Código</span>
              <input
                value={registerCode}
                onChange={(event) => setRegisterCode(event.target.value)}
                required
                maxLength={16}
              />
            </label>
            <div className="field field-action">
              <button type="submit" className="button button-primary" disabled={busy}>
                Crear caja
              </button>
            </div>
          </div>
        </form>
      )}

      {registers.length > 0 && (
        <div className="field-row">
          <label className="field field-short">
            <span>Caja</span>
            <select
              value={registerId}
              onChange={(event) => {
                setRegisterId(event.target.value);
                setShift(null);
              }}
            >
              {registers.map((register) => (
                <option key={register.id} value={register.id}>
                  {register.name} · {register.code}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}

      {shift?.status !== "closed" && !shift && registerId && canOpen && !readOnly && (
        <form className="form-grid form-compact" onSubmit={openShift} noValidate>
          <p className="muted">No hay turno abierto.</p>
          <div className="field-row">
            <label className="field field-short">
              <span>Fondo inicial</span>
              <input
                inputMode="decimal"
                value={openingAmount}
                onChange={(event) => setOpeningAmount(event.target.value)}
                required
              />
            </label>
            <div className="field field-action">
              <button type="submit" className="button button-primary" disabled={busy}>
                Abrir turno
              </button>
            </div>
          </div>
        </form>
      )}

      {shift && shift.status !== "closed" && (
        <>
          <dl className="definition-list">
            <div>
              <dt>Fondo</dt>
              <dd>{shift.opening_amount}</dd>
            </div>
            <div>
              <dt>Entradas</dt>
              <dd>{shift.entry_total}</dd>
            </div>
            <div>
              <dt>Salidas</dt>
              <dd>{shift.exit_total}</dd>
            </div>
            <div>
              <dt>Saldo esperado</dt>
              <dd>{shift.expected_amount}</dd>
            </div>
          </dl>

          {canMove && !readOnly && (
            <form className="form-grid form-compact" onSubmit={addMovement} noValidate>
              <div className="field-row">
                <label className="field field-short">
                  <span>Tipo</span>
                  <select value={kind} onChange={(event) => setKind(event.target.value as "in" | "out")}>
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
                  <input value={reason} onChange={(event) => setReason(event.target.value)} required />
                </label>
                <div className="field field-action">
                  <button type="submit" className="button button-primary" disabled={busy}>
                    Registrar
                  </button>
                </div>
              </div>
            </form>
          )}

          {shift.movements.length === 0 ? (
            <p className="muted">Sin movimientos en este turno.</p>
          ) : (
            <ol className="timeline">
              {shift.movements.map((movement) => (
                <li key={movement.id}>
                  <span className="timeline-action">{movement.kind}</span>
                  <span className="timeline-meta">
                    {movement.amount} · {movement.reason}
                  </span>
                </li>
              ))}
            </ol>
          )}

          {canClose && !readOnly && (
            <form className="form-grid form-compact" onSubmit={closeShift} noValidate>
              <div className="field-row">
                <label className="field field-short">
                  <span>Efectivo contado</span>
                  <input
                    inputMode="decimal"
                    value={countedAmount}
                    onChange={(event) => setCountedAmount(event.target.value)}
                    required
                  />
                </label>
                <div className="field field-action">
                  <button type="submit" className="button button-primary" disabled={busy}>
                    Cerrar turno
                  </button>
                </div>
              </div>
            </form>
          )}
        </>
      )}

      {shift?.status === "closed" && (
        <div className="catalog-detail">
          <p>Turno cerrado.</p>
          <dl className="definition-list">
            <div>
              <dt>Esperado</dt>
              <dd>{shift.expected_amount}</dd>
            </div>
            <div>
              <dt>Contado</dt>
              <dd>{shift.counted_amount}</dd>
            </div>
            <div>
              <dt>Diferencia (centavos)</dt>
              <dd>{shift.difference_minor}</dd>
            </div>
          </dl>
        </div>
      )}
    </section>
  );
}