import { useEffect, useState } from "react";
import { api, IpcError } from "../api";
import type {
  Actor,
  CashShift,
  ReturnableSale,
  SaleReturn,
  SaleReturnDisposition,
  SaleReturnDraft,
  SaleReturnLineDraft,
} from "../contracts";
import type { PendingReturnAttempt } from "../lib/pendingAttempt";

interface Props {
  actor: Actor;
  readOnly: boolean;
  shift: CashShift | null;
  onCashChanged: () => void;
  pendingAttempt: PendingReturnAttempt | null;
  onPendingAttemptChange: (attempt: PendingReturnAttempt | null) => void;
}

interface Selection {
  quantity: string;
  disposition: SaleReturnDisposition;
}

function messageOf(cause: unknown, fallback: string) {
  return cause instanceof IpcError ? cause.message : fallback;
}

function isUncertain(cause: unknown): boolean {
  return (
    !(cause instanceof IpcError) ||
    cause.code === "storage_error" ||
    cause.code === "unknown_error"
  );
}

function formatTimestamp(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("es-MX", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}

export function ReturnsScreen({
  actor,
  readOnly,
  shift,
  onCashChanged,
  pendingAttempt,
  onPendingAttemptChange,
}: Props) {
  const canReturn = actor.permissions.includes("sales.return");
  const [folio, setFolio] = useState("");
  const [sale, setSale] = useState<ReturnableSale | null>(null);
  const [selections, setSelections] = useState<Record<string, Selection>>({});
  const [reason, setReason] = useState("");
  const [previewTotal, setPreviewTotal] = useState<string | null>(null);
  const [history, setHistory] = useState<SaleReturn[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const locked = pendingAttempt !== null;

  useEffect(() => {
    if (!canReturn) return;
    let cancelled = false;
    api
      .listSaleReturns()
      .then((items) => {
        if (!cancelled) setHistory(items);
      })
      .catch(() => {
        // El historial es complementario; no bloquea registrar una devolución.
      });
    return () => {
      cancelled = true;
    };
  }, [canReturn]);

  if (!canReturn) {
    return (
      <div className="empty-state">
        <p>No tienes permiso para registrar devoluciones.</p>
      </div>
    );
  }

  function buildLines(): SaleReturnLineDraft[] {
    if (!sale) return [];
    return sale.items
      .filter((item) => Number(selections[item.id]?.quantity ?? "0") > 0)
      .map((item) => ({
        sale_item_id: item.id,
        quantity: selections[item.id]?.quantity ?? "0",
        disposition: selections[item.id]?.disposition ?? "restock",
      }));
  }

  async function search(event: React.FormEvent) {
    event.preventDefault();
    if (!folio.trim()) return;
    setBusy(true);
    setError(null);
    setNotice(null);
    setPreviewTotal(null);
    try {
      const found = await api.findSaleForReturn(folio.trim());
      setSale(found);
      setSelections(
        Object.fromEntries(
          found.items.map((item) => [item.id, { quantity: "0", disposition: "restock" }]),
        ),
      );
    } catch (cause) {
      setSale(null);
      setSelections({});
      setError(messageOf(cause, "No fue posible consultar la venta."));
    } finally {
      setBusy(false);
    }
  }

  async function preview() {
    if (!sale) return;
    const lines = buildLines();
    if (lines.length === 0) {
      setError("Indica al menos una cantidad a devolver.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const result = await api.previewSaleReturn({ sale_id: sale.id, lines });
      setPreviewTotal(result.refund_total);
    } catch (cause) {
      setPreviewTotal(null);
      setError(messageOf(cause, "No fue posible calcular el reembolso."));
    } finally {
      setBusy(false);
    }
  }

  /** Registra el intento antes de enviar y lo conserva si el resultado es incierto. */
  async function send(attempt: PendingReturnAttempt) {
    setBusy(true);
    setError(null);
    setNotice(null);
    onPendingAttemptChange(attempt);
    try {
      const created = await api.createSaleReturn(attempt.payload);
      onPendingAttemptChange(null);
      setNotice(
        `Devolución de ${attempt.payload.lines.length} partida(s) registrada. Reembolso $${created.refund_total}.`,
      );
      setPreviewTotal(null);
      setReason("");
      setSelections((current) =>
        Object.fromEntries(
          Object.entries(current).map(([id, value]) => [id, { ...value, quantity: "0" }]),
        ),
      );
      onCashChanged();
      const found = await api.findSaleForReturn(created.sale_folio);
      setSale(found);
      setHistory(await api.listSaleReturns());
    } catch (cause) {
      if (isUncertain(cause)) {
        setError(
          "No se pudo confirmar el resultado. Reintenta sin modificar los datos; se usará la misma operación.",
        );
      } else {
        onPendingAttemptChange(null);
        setError(messageOf(cause, "No fue posible registrar la devolución."));
      }
    } finally {
      setBusy(false);
    }
  }

  async function confirm(event: React.FormEvent) {
    event.preventDefault();
    if (locked || !sale || !shift) return;
    const lines = buildLines();
    if (lines.length === 0) {
      setError("Indica al menos una cantidad a devolver.");
      return;
    }
    if (!reason.trim()) {
      setError("Indica el motivo de la devolución.");
      return;
    }
    const payload: SaleReturnDraft = {
      sale_id: sale.id,
      shift_id: shift.id,
      reason: reason.trim(),
      idempotency_key: crypto.randomUUID(),
      lines: [...lines].sort((left, right) =>
        left.sale_item_id.localeCompare(right.sale_item_id),
      ),
    };
    await send({
      fingerprint: JSON.stringify({
        sale_id: payload.sale_id,
        shift_id: payload.shift_id,
        reason: payload.reason,
        lines: payload.lines,
      }),
      payload,
    });
  }

  return (
    <div className="screen returns-screen">
      <header className="screen-header">
        <div>
          <h1>Devoluciones</h1>
          <p className="screen-subtitle">
            Reembolso en efectivo desde un turno abierto. La venta original no se modifica.
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
        <section className="surface attention" aria-label="Devolución pendiente de confirmar">
          <header className="surface-header">
            <div>
              <h2>Devolución pendiente de confirmar</h2>
              <p className="muted">
                Se reintentará la misma operación: no se reembolsará dos veces ni se duplicará el
                inventario.
              </p>
            </div>
          </header>
          <dl className="mini-grid">
            <div><dt>Motivo</dt><dd>{pendingAttempt.payload.reason}</dd></div>
            <div><dt>Partidas</dt><dd className="mono">{pendingAttempt.payload.lines.length}</dd></div>
          </dl>
          <button
            type="button"
            className="button button-primary"
            onClick={() => void send(pendingAttempt)}
            disabled={busy || readOnly}
          >
            {busy ? "Reintentando…" : "Reintentar devolución"}
          </button>
        </section>
      )}

      <section className="surface">
        <header className="surface-header">
          <div>
            <h2>Buscar venta</h2>
            <p className="muted">Localiza el ticket por folio para devolver partidas.</p>
          </div>
        </header>
        <form className="inline-form" onSubmit={search}>
          <label className="field">
            <span>Folio de venta</span>
            <input
              value={folio}
              onChange={(event) => setFolio(event.target.value)}
              placeholder="V-XXXXXXXXXX"
              disabled={locked}
            />
          </label>
          <button className="button button-quiet" disabled={busy || !folio.trim()}>
            Buscar venta
          </button>
        </form>
      </section>

      {sale && (
        <form className="surface" onSubmit={confirm} aria-label="Devolución de la venta">
          <header className="surface-header">
            <div>
              <h2>Venta {sale.folio}</h2>
              <p className="muted">
                {formatTimestamp(sale.occurred_at)} · total original ${sale.total}
              </p>
            </div>
          </header>
          <div className="table-scroll">
            <table className="data-table">
              <thead>
                <tr>
                  <th>Partida</th>
                  <th>Vendido</th>
                  <th>Devuelto</th>
                  <th>Disponible</th>
                  <th>Reembolso unitario</th>
                  <th>Cantidad</th>
                  <th>Destino</th>
                </tr>
              </thead>
              <tbody>
                {sale.items.map((item) => (
                  <tr key={item.id}>
                    <td>
                      {item.product_name}
                      {item.variant_name ? ` · ${item.variant_name}` : ""}
                      {item.modifiers_summary ? (
                        <span className="muted"> · {item.modifiers_summary}</span>
                      ) : null}
                    </td>
                    <td className="mono">{item.quantity}</td>
                    <td className="mono">{item.returned_quantity}</td>
                    <td className="mono">{item.returnable_quantity}</td>
                    <td className="mono">${item.unit_refund}</td>
                    <td>
                      <input
                        className="quantity-input"
                        inputMode="decimal"
                        aria-label={`Cantidad a devolver ${item.product_name}`}
                        value={selections[item.id]?.quantity ?? "0"}
                        onChange={(event) =>
                          setSelections((current) => ({
                            ...current,
                            [item.id]: {
                              quantity: event.target.value,
                              disposition: current[item.id]?.disposition ?? "restock",
                            },
                          }))
                        }
                        disabled={locked || Number(item.returnable_quantity) === 0}
                      />
                    </td>
                    <td>
                      <select
                        aria-label={`Destino ${item.product_name}`}
                        value={selections[item.id]?.disposition ?? "restock"}
                        onChange={(event) =>
                          setSelections((current) => ({
                            ...current,
                            [item.id]: {
                              quantity: current[item.id]?.quantity ?? "0",
                              disposition: event.target.value as SaleReturnDisposition,
                            },
                          }))
                        }
                        disabled={locked || Number(item.returnable_quantity) === 0}
                      >
                        <option value="restock">Reintegrar a inventario</option>
                        <option value="discard">No reintegrable</option>
                      </select>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          <label className="field">
            <span>Motivo</span>
            <input
              value={reason}
              onChange={(event) => setReason(event.target.value)}
              disabled={locked}
              placeholder="Producto en buen estado, cambio, etc."
            />
          </label>

          <div className="return-summary">
            <div>
              <p className="muted">
                {shift
                  ? `Reembolso desde el turno abierto · saldo esperado $${shift.expected_amount}`
                  : "Abre un turno de caja para entregar el reembolso."}
              </p>
              <p className="metric-strong">
                Reembolso calculado por Rust:{" "}
                <span className="mono">
                  {previewTotal === null ? "—" : `$${previewTotal}`}
                </span>
              </p>
            </div>
            <div className="form-actions">
              <button
                type="button"
                className="button button-quiet"
                onClick={() => void preview()}
                disabled={busy || locked}
              >
                Calcular reembolso
              </button>
              <button
                type="submit"
                className="button button-primary"
                disabled={busy || readOnly || locked || !shift}
              >
                {busy ? "Registrando…" : "Confirmar devolución"}
              </button>
            </div>
          </div>
        </form>
      )}

      <section className="surface">
        <header className="surface-header">
          <div>
            <h2>Historial</h2>
            <p className="muted">Devoluciones registradas en la sucursal.</p>
          </div>
          <span className="badge badge-neutral">{history.length}</span>
        </header>
        {history.length === 0 ? (
          <div className="empty-state">
            <p>Sin devoluciones registradas.</p>
          </div>
        ) : (
          <div className="table-scroll">
            <table className="data-table">
              <thead>
                <tr>
                  <th>Venta</th>
                  <th>Reembolso</th>
                  <th>Partidas</th>
                  <th>Motivo</th>
                  <th className="align-end">Fecha</th>
                </tr>
              </thead>
              <tbody>
                {history.map((item) => (
                  <tr key={item.id}>
                    <td className="mono">{item.sale_folio}</td>
                    <td className="mono">${item.refund_total}</td>
                    <td>
                      {item.lines
                        .map(
                          (line) =>
                            `${line.product_name} × ${line.quantity} (${
                              line.disposition === "restock" ? "reintegrada" : "no reintegrable"
                            })`,
                        )
                        .join(", ")}
                    </td>
                    <td>{item.reason}</td>
                    <td className="align-end">{formatTimestamp(item.occurred_at)}</td>
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
