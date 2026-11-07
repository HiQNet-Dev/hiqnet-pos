import { useState } from "react";
import { api, IpcError } from "../api";
import type { CashShift, CheckoutDraft, SaleView } from "../contracts";
import { cartTotals, estimatedChange, lineTotalMinor, type CartLine } from "../lib/cart";
import type { PendingSaleAttempt } from "../lib/pendingAttempt";
import { displayMoney, formatMinor } from "../lib/money";
import { Modal } from "../components/Modal";

interface Props {
  cart: CartLine[];
  shift: CashShift | null;
  readOnly: boolean;
  pendingAttempt: PendingSaleAttempt | null;
  onPendingAttemptChange: (attempt: PendingSaleAttempt | null) => void;
  onClose: () => void;
  onCompleted: (sale: SaleView) => void;
}

function ipcErrorOf(cause: unknown): IpcError | null {
  return cause && typeof cause === "object" && typeof (cause as IpcError).code === "string"
    ? (cause as IpcError)
    : null;
}

function isUncertain(cause: unknown): boolean {
  const code = ipcErrorOf(cause)?.code;
  return code === undefined || ["storage_error", "unknown_error", "contract_mismatch"].includes(code);
}

/** Siguiente múltiplo exacto en centavos; aritmética entera, sin flotantes. */
function roundUpTo(minor: number, step: number): number {
  return Math.ceil(minor / step) * step;
}

/**
 * Importes habituales que el cajero suele recibir. No cobra nada: solo
 * rellena el campo para que el cambio se vea de inmediato.
 */
function cashSuggestions(totalMinor: number): number[] {
  const values = new Set<number>([totalMinor]);
  for (const step of [2_000, 5_000, 10_000]) {
    const rounded = roundUpTo(totalMinor, step);
    if (rounded > totalMinor) values.add(rounded);
  }
  return [...values].slice(0, 4);
}

/**
 * Cobro en efectivo. Los importes definitivos los confirma Rust; lo mostrado
 * antes de confirmar es una estimación de la misma aritmética entera.
 */
export function CheckoutDialog({
  cart,
  shift,
  readOnly,
  pendingAttempt,
  onPendingAttemptChange,
  onClose,
  onCompleted,
}: Props) {
  const totals = cartTotals(cart);
  const [cashReceived, setCashReceived] = useState(totals.total);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sale, setSale] = useState<SaleView | null>(null);
  const recovering = pendingAttempt !== null;

  const change = estimatedChange(cashReceived, totals.totalMinor);
  const insufficient = change !== null && change < 0;
  const suggestions = cashSuggestions(totals.totalMinor);
  const canConfirm =
    !busy &&
    !readOnly &&
    (recovering || (shift !== null && cart.length > 0 && change !== null && change >= 0));

  function newAttempt(): PendingSaleAttempt | null {
    if (!shift) return null;
    const payload: CheckoutDraft = {
      shift_id: shift.id,
      idempotency_key: crypto.randomUUID(),
      items: cart.map((line) => ({
        product_id: line.productId,
        variant_id: line.variantId,
        quantity: String(line.quantity),
        selected_option_ids: line.selectedOptionIds,
      })),
      cash_received: cashReceived,
    };
    return { fingerprint: JSON.stringify(payload), payload };
  }

  /** Persiste primero y sólo libera un borrador ante una respuesta concluyente. */
  async function send(attempt: PendingSaleAttempt) {
    setBusy(true);
    setError(null);
    onPendingAttemptChange(attempt);
    try {
      const result = await api.checkoutSale(attempt.payload);
      onPendingAttemptChange(null);
      setSale(result);
      onCompleted(result);
    } catch (cause) {
      if (isUncertain(cause)) {
        setError(
          "No se pudo confirmar el resultado. Recupera este mismo cobro sin modificar los datos; no se creará una segunda venta.",
        );
      } else {
        onPendingAttemptChange(null);
        setError(ipcErrorOf(cause)?.message ?? "No fue posible confirmar la venta.");
      }
    } finally {
      setBusy(false);
    }
  }

  async function confirm() {
    if (!canConfirm) return;
    const attempt = pendingAttempt ?? newAttempt();
    if (attempt) await send(attempt);
  }

  if (sale) {
    return (
      <Modal
        title={`Venta ${sale.folio}`}
        subtitle="Venta confirmada por el sistema"
        onClose={onClose}
        footer={
          <button type="button" className="button button-primary" onClick={onClose}>
            Nueva venta
          </button>
        }
      >
        <dl className="receipt">
          <div>
            <dt>Total cobrado</dt>
            <dd>${sale.total}</dd>
          </div>
          <div>
            <dt>Efectivo recibido</dt>
            <dd>${sale.payment?.amount ?? "—"}</dd>
          </div>
          <div>
            <dt>Cambio</dt>
            <dd>${sale.payment?.change ?? "0.00"}</dd>
          </div>
        </dl>
        <p className="muted">
          El inventario y el efectivo de la caja ya quedaron actualizados.
        </p>
      </Modal>
    );
  }

  if (recovering) {
    const draft = pendingAttempt.payload;
    return (
      <Modal
        title="Cobro pendiente de confirmar"
        subtitle="El resultado del cobro anterior no llegó a la aplicación"
        size="lg"
        onClose={onClose}
        footer={
          <>
            <button type="button" className="button" onClick={onClose} disabled={busy}>
              Cerrar
            </button>
            <button
              type="button"
              className="button button-primary"
              onClick={() => void confirm()}
              disabled={!canConfirm}
            >
              {busy ? "Recuperando…" : "Recuperar cobro"}
            </button>
          </>
        }
      >
        <p className="alert alert-warning" role="status">
          Reintentaremos exactamente el mismo cobro. No prepares ni confirmes otra venta hasta
          recibir el resultado.
        </p>
        <dl className="receipt">
          <div>
            <dt>Partidas</dt>
            <dd>{draft.items.length}</dd>
          </div>
          <div>
            <dt>Unidades</dt>
            <dd>{draft.items.reduce((total, item) => total + Number(item.quantity), 0)}</dd>
          </div>
          <div>
            <dt>Efectivo recibido</dt>
            <dd>${draft.cash_received}</dd>
          </div>
        </dl>
        {error && (
          <p className="alert alert-error" role="alert">
            {error}
          </p>
        )}
      </Modal>
    );
  }

  return (
    <Modal
      title="Cobrar venta"
      subtitle="Ticket actual · pago en efectivo"
      size="lg"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="button" onClick={onClose} disabled={busy}>
            Volver al ticket
          </button>
          <button
            type="button"
            className="button button-primary"
            onClick={() => void confirm()}
            disabled={!canConfirm}
          >
            {busy ? "Procesando…" : "Confirmar cobro"}
          </button>
        </>
      }
    >
      <ul className="checkout-lines">
        {cart.map((line) => (
          <li key={line.key}>
            <span className="checkout-line-name">
              {line.quantity} × {line.name}
              {line.variantName ? ` · ${line.variantName}` : ""}
              {line.modifiers.length > 0 ? ` · ${line.modifiers.join(", ")}` : ""}
            </span>
            <span className="mono">${formatMinor(lineTotalMinor(line))}</span>
          </li>
        ))}
      </ul>

      <dl className="receipt">
        <div>
          <dt>Subtotal</dt>
          <dd>${totals.subtotal}</dd>
        </div>
        <div>
          <dt>Impuestos</dt>
          <dd>${totals.tax}</dd>
        </div>
        <div className="receipt-total">
          <dt>Total estimado</dt>
          <dd>${totals.total}</dd>
        </div>
      </dl>

      <label className="field">
        <span>Efectivo recibido</span>
        <input
          inputMode="decimal"
          value={cashReceived}
          onChange={(event) => setCashReceived(event.target.value)}
          onFocus={(event) => event.target.select()}
          autoFocus
          required
        />
      </label>

      {suggestions.length > 1 && (
        <div className="chips chips-amounts" role="group" aria-label="Importes habituales">
          {suggestions.map((value) => (
            <button
              key={value}
              type="button"
              className={value === totals.totalMinor ? "chip chip-active" : "chip"}
              onClick={() => setCashReceived(formatMinor(value))}
            >
              {value === totals.totalMinor ? `Exacto $${formatMinor(value)}` : `$${formatMinor(value)}`}
            </button>
          ))}
        </div>
      )}

      <p className={insufficient ? "alert alert-warning" : "muted"}>
        {change === null
          ? "Escribe un importe válido para ver el cambio."
          : insufficient
            ? `Faltan ${displayMoney(formatMinor(-change))} para cubrir el total.`
            : `Cambio estimado: ${displayMoney(formatMinor(change))}.`}
      </p>

      <p className="hint">
        El total definitivo, el impuesto y el cambio los calcula Rust al confirmar.
      </p>

      {error && (
        <p className="alert alert-error" role="alert">
          {error}
        </p>
      )}
    </Modal>
  );
}
