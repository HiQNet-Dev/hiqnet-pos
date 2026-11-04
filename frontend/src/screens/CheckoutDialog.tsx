import { useRef, useState } from "react";
import { api, IpcError } from "../api";
import type { CashShift, SaleView } from "../contracts";
import { cartTotals, estimatedChange, lineTotalMinor, type CartLine } from "../lib/cart";
import { displayMoney, formatMinor } from "../lib/money";
import { Modal } from "../components/Modal";

interface Props {
  cart: CartLine[];
  shift: CashShift;
  readOnly: boolean;
  onClose: () => void;
  onCompleted: (sale: SaleView) => void;
}

interface Attempt {
  key: string;
  payload: string;
}

/**
 * Cobro en efectivo. Los importes definitivos los confirma Rust; lo mostrado
 * antes de confirmar es una estimación de la misma aritmética entera.
 */
export function CheckoutDialog({ cart, shift, readOnly, onClose, onCompleted }: Props) {
  const totals = cartTotals(cart);
  const [cashReceived, setCashReceived] = useState(totals.total);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sale, setSale] = useState<SaleView | null>(null);
  const attempt = useRef<Attempt | null>(null);

  const change = estimatedChange(cashReceived, totals.totalMinor);
  const insufficient = change !== null && change < 0;

  function keyFor(payload: string): string {
    if (attempt.current?.payload === payload) return attempt.current.key;
    const key = crypto.randomUUID();
    attempt.current = { key, payload };
    return key;
  }

  async function confirm() {
    if (busy || readOnly) return;
    setBusy(true);
    setError(null);
    const payload = JSON.stringify({
      shift: shift.id,
      cash: cashReceived,
      items: cart.map((line) => [line.productId, line.variantId, line.quantity, line.selectedOptionIds]),
    });
    try {
      const result = await api.checkoutSale({
        shift_id: shift.id,
        idempotency_key: keyFor(payload),
        items: cart.map((line) => ({
          product_id: line.productId,
          variant_id: line.variantId,
          quantity: String(line.quantity),
          selected_option_ids: line.selectedOptionIds,
        })),
        cash_received: cashReceived,
      });
      setSale(result);
      onCompleted(result);
    } catch (cause) {
      setError(
        cause instanceof IpcError ? cause.message : "No fue posible confirmar la venta.",
      );
    } finally {
      setBusy(false);
    }
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
            disabled={busy || readOnly || cart.length === 0}
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
          autoFocus
          required
        />
      </label>

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
