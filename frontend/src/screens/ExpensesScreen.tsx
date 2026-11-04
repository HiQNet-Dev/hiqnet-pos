import { useEffect, useState } from "react";
import { api, IpcError } from "../api";
import type { Actor, CashShift, Expense, ExpenseCategory, ExpenseDraft } from "../contracts";
import { displayMoney } from "../lib/money";
import type { PendingExpenseAttempt } from "../lib/pendingAttempt";

interface Props {
  actor: Actor;
  readOnly: boolean;
  shift: CashShift | null;
  onCashChanged: () => void;
  pendingAttempt: PendingExpenseAttempt | null;
  onPendingAttemptChange: (attempt: PendingExpenseAttempt | null) => void;
}

function messageOf(cause: unknown, fallback: string) {
  return cause instanceof IpcError ? cause.message : fallback;
}

/** Un error de transporte no confirma ni descarta el hecho en SQLite. */
function isUncertain(cause: unknown): boolean {
  return !(cause instanceof IpcError) ||
    cause.code === "storage_error" ||
    cause.code === "unknown_error";
}

export function ExpensesScreen({
  actor,
  readOnly,
  shift,
  onCashChanged,
  pendingAttempt,
  onPendingAttemptChange,
}: Props) {
  const [categories, setCategories] = useState<ExpenseCategory[]>([]);
  const [expenses, setExpenses] = useState<Expense[]>([]);
  const [categoryName, setCategoryName] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const [amount, setAmount] = useState("");
  const [concept, setConcept] = useState("");
  const [fromCash, setFromCash] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const canManage = actor.permissions.includes("expenses.manage");
  const locked = pendingAttempt !== null;

  const refresh = async () => {
    const [nextCategories, nextExpenses] = await Promise.all([
      api.listExpenseCategories(),
      api.listExpenses(),
    ]);
    setCategories(nextCategories);
    setExpenses(nextExpenses);
  };

  useEffect(() => {
    if (!canManage) return;
    const timer = window.setTimeout(
      () =>
        void refresh().catch((cause) =>
          setError(messageOf(cause, "No fue posible cargar gastos.")),
        ),
      0,
    );
    return () => window.clearTimeout(timer);
  }, [canManage]);

  if (!canManage) {
    return (
      <div className="empty-state">
        <p>No tienes permiso para administrar gastos.</p>
      </div>
    );
  }

  async function createCategory(event: React.FormEvent) {
    event.preventDefault();
    if (readOnly || !categoryName.trim()) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      const category = await api.createExpenseCategory({ name: categoryName.trim() });
      setCategories((items) => [...items, category]);
      setCategoryName("");
      setNotice("Categoría creada.");
    } catch (cause) {
      setError(messageOf(cause, "No fue posible crear la categoría."));
    } finally {
      setBusy(false);
    }
  }

  /** Registra el intento antes de enviar y lo elimina sólo con resultado definitivo. */
  async function send(attempt: PendingExpenseAttempt) {
    setBusy(true);
    setError(null);
    setNotice(null);
    onPendingAttemptChange(attempt);
    try {
      await api.recordExpense(attempt.payload);
      onPendingAttemptChange(null);
      setAmount("");
      setConcept("");
      setFromCash(false);
      setNotice(
        attempt.payload.paid_from_cash
          ? "Gasto y salida de caja registrados."
          : "Gasto externo registrado.",
      );
      if (attempt.payload.paid_from_cash) onCashChanged();
      await refresh();
    } catch (cause) {
      if (isUncertain(cause)) {
        setError(
          "No se pudo confirmar el resultado. Reintenta sin modificar los datos; se usará la misma operación.",
        );
      } else {
        onPendingAttemptChange(null);
        setError(messageOf(cause, "No fue posible registrar el gasto."));
      }
    } finally {
      setBusy(false);
    }
  }

  async function record(event: React.FormEvent) {
    event.preventDefault();
    if (readOnly || locked) return;
    if (!categoryId || !amount || !concept.trim() || (fromCash && !shift)) return;
    const payload: ExpenseDraft = {
      category_id: categoryId,
      amount,
      concept: concept.trim(),
      paid_from_cash: fromCash,
      shift_id: fromCash ? shift?.id ?? null : null,
      idempotency_key: crypto.randomUUID(),
    };
    await send({
      fingerprint: [
        payload.category_id,
        payload.amount,
        payload.concept,
        payload.paid_from_cash,
        payload.shift_id ?? "",
      ].join("|"),
      payload,
    });
  }

  return (
    <div className="screen">
      <header className="screen-header">
        <div>
          <h1>Gastos</h1>
          <p className="screen-subtitle">
            Un gasto registra el hecho económico; solo el pago desde caja modifica efectivo.
          </p>
        </div>
      </header>

      {(error || notice) && (
        <div className="screen-inline-alert" aria-live="polite">
          {error && <p className="alert alert-error">{error}</p>}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      {pendingAttempt && (
        <section className="surface attention" aria-label="Gasto pendiente de confirmar">
          <header className="surface-header">
            <div>
              <h2>Gasto pendiente de confirmar</h2>
              <p className="muted">
                Se reintentará la misma operación: no se registrará un segundo gasto ni una
                segunda salida de caja.
              </p>
            </div>
          </header>
          <dl className="mini-grid">
            <div>
              <dt>Concepto</dt>
              <dd>{pendingAttempt.payload.concept}</dd>
            </div>
            <div>
              <dt>Importe</dt>
              <dd className="mono">{displayMoney(pendingAttempt.payload.amount)}</dd>
            </div>
            <div>
              <dt>Origen</dt>
              <dd>{pendingAttempt.payload.paid_from_cash ? "Caja" : "Externo"}</dd>
            </div>
          </dl>
          <button
            type="button"
            className="button button-primary"
            onClick={() => void send(pendingAttempt)}
            disabled={busy || readOnly}
          >
            {busy ? "Reintentando…" : "Reintentar gasto"}
          </button>
        </section>
      )}

      <div className="purchasing-layout">
        <section className="surface">
          <header className="surface-header">
            <div>
              <h2>Categorías</h2>
              <p className="muted">Clasifica gastos operativos.</p>
            </div>
          </header>
          <form className="inline-form" onSubmit={createCategory}>
            <label className="field">
              <span>Nueva categoría</span>
              <input
                value={categoryName}
                onChange={(event) => setCategoryName(event.target.value)}
              />
            </label>
            <button className="button button-quiet" disabled={busy || readOnly}>
              Crear
            </button>
          </form>
          <ul className="compact-list">
            {categories.map((category) => (
              <li key={category.id}>{category.name}</li>
            ))}
          </ul>
          {categories.length === 0 && (
            <p className="muted empty-cell">
              Crea una categoría para poder clasificar los gastos.
            </p>
          )}
        </section>

        <section className="surface">
          <header className="surface-header">
            <div>
              <h2>Registrar gasto</h2>
              <p className="muted">
                {fromCash ? "Creará una salida en el turno abierto." : "No afecta el saldo de caja."}
              </p>
            </div>
          </header>
          <form className="form-grid form-compact" onSubmit={record}>
            <label className="field">
              <span>Categoría</span>
              <select
                value={categoryId}
                onChange={(event) => setCategoryId(event.target.value)}
                disabled={locked}
              >
                <option value="">Selecciona una categoría</option>
                {categories
                  .filter((category) => category.active)
                  .map((category) => (
                    <option key={category.id} value={category.id}>
                      {category.name}
                    </option>
                  ))}
              </select>
            </label>
            <label className="field">
              <span>Concepto</span>
              <input
                value={concept}
                onChange={(event) => setConcept(event.target.value)}
                disabled={locked}
              />
            </label>
            <label className="field field-short">
              <span>Importe</span>
              <input
                inputMode="decimal"
                value={amount}
                onChange={(event) => setAmount(event.target.value)}
                placeholder="0.00"
                disabled={locked}
              />
            </label>
            <label className="checkbox-field">
              <input
                type="checkbox"
                checked={fromCash}
                onChange={(event) => setFromCash(event.target.checked)}
                disabled={!shift || locked}
              />
              <span>Pagar desde caja {shift ? "" : "(sin turno abierto)"}</span>
            </label>
            <button
              className="button button-primary"
              disabled={
                busy || readOnly || locked || !categoryId || !amount || !concept.trim() ||
                (fromCash && !shift)
              }
            >
              {busy ? "Registrando…" : "Registrar gasto"}
            </button>
          </form>
        </section>
      </div>

      <section className="surface">
        <header className="surface-header">
          <div>
            <h2>Historial</h2>
            <p className="muted">Los gastos son hechos append-only.</p>
          </div>
          <span className="badge badge-neutral">{expenses.length}</span>
        </header>
        {expenses.length === 0 ? (
          <div className="empty-state">
            <p>Sin gastos registrados.</p>
          </div>
        ) : (
          <div className="table-scroll">
            <table className="data-table">
              <thead>
                <tr>
                  <th>Concepto</th>
                  <th>Importe</th>
                  <th>Origen</th>
                  <th>Fecha</th>
                </tr>
              </thead>
              <tbody>
                {expenses.map((expense) => (
                  <tr key={expense.id}>
                    <td>{expense.concept}</td>
                    <td className="mono">${expense.amount}</td>
                    <td>{expense.paid_from_cash ? "Caja" : "Externo"}</td>
                    <td>{new Date(expense.occurred_at).toLocaleString("es-MX")}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  );
}
