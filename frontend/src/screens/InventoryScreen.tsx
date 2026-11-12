import { useEffect, useRef, useState } from "react";
import { api, IpcError } from "../api";
import {
  inventoryMovementDraftSchema,
  type Actor,
  type InventoryMovement,
  type Product,
  type WasteDraft,
} from "../contracts";
import { formatQuantity } from "../lib/money";
import { parseQuantity, subtractQuantity } from "../lib/quantity";
import type { PendingWasteAttempt } from "../lib/pendingAttempt";
import { Modal } from "../components/Modal";

interface Props {
  actor: Actor;
  readOnly: boolean;
  products: Product[];
  loading: boolean;
  onStockChanged: () => void;
  pendingWaste: PendingWasteAttempt | null;
  onPendingWasteChange: (attempt: PendingWasteAttempt | null) => void;
}

const KIND_LABELS: Record<string, string> = {
  inbound: "Entrada",
  outbound: "Salida",
  adjustment_in: "Ajuste +",
  adjustment_out: "Ajuste −",
};

const WASTE_REASON_LABELS: Record<string, string> = {
  caducidad: "Caducidad",
  daño: "Daño",
  pérdida: "Pérdida",
  otro: "Otro",
};

export function InventoryScreen({ actor, readOnly, products, loading, onStockChanged, pendingWaste, onPendingWasteChange }: Props) {
  const canManage = actor.permissions.includes("inventory.manage");
  const canWaste = actor.permissions.includes("waste.manage");
  const [productId, setProductId] = useState("");
  const [kind, setKind] = useState<"inbound" | "outbound" | "adjustment_in" | "adjustment_out">(
    "inbound",
  );
  const [quantity, setQuantity] = useState("");
  const [reason, setReason] = useState("");
  const [wasteQuantity, setWasteQuantity] = useState("");
  const [wasteReason, setWasteReason] = useState("caducidad");
  const [wasteReview, setWasteReview] = useState<{
    quantity: string;
    reason: string;
    currentStock: string;
    resultingStock: string;
  } | null>(null);
  const [wasteError, setWasteError] = useState<string | null>(null);
  const [stock, setStock] = useState<string | null>(null);
  const [movements, setMovements] = useState<InventoryMovement[]>([]);
  const [busy, setBusy] = useState(false);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const requestRef = useRef(0);
  const wasteLocked = pendingWaste !== null;

  const selectedProduct = products.find((product) => product.id === productId) ?? null;

  useEffect(() => {
    if (!productId) return;
    let cancelled = false;
    const request = ++requestRef.current;
    Promise.all([api.currentStock(productId), api.listInventoryMovements(productId, 50)])
      .then(([currentStock, history]) => {
        if (cancelled || request !== requestRef.current) return;
        setStock(currentStock);
        setMovements(history);
      })
      .catch((cause: unknown) => {
        if (cancelled || request !== requestRef.current) return;
        setError(
          cause instanceof IpcError ? cause.message : "No fue posible consultar el inventario.",
        );
      })
      .finally(() => {
        if (!cancelled && request === requestRef.current) setLoadingDetail(false);
      });
    return () => {
      cancelled = true;
    };
  }, [productId]);

  async function refreshDetail() {
    if (!productId) return;
    const request = ++requestRef.current;
    try {
      const [currentStock, history] = await Promise.all([
        api.currentStock(productId),
        api.listInventoryMovements(productId, 50),
      ]);
      if (request !== requestRef.current) return;
      setStock(currentStock);
      setMovements(history);
    } catch (cause) {
      setError(
        cause instanceof IpcError ? cause.message : "No fue posible consultar el inventario.",
      );
    }
  }

  /** Registra el intento antes de enviar y lo elimina sólo con resultado definitivo. */
  async function sendWaste(attempt: PendingWasteAttempt): Promise<"ok" | "error" | "uncertain"> {
    setBusy(true); setError(null); setNotice(null);
    onPendingWasteChange(attempt);
    try {
      await api.recordWaste(attempt.payload);
      onPendingWasteChange(null);
      setWasteQuantity("");
      setNotice("Merma registrada y existencias actualizadas.");
      if (productId === attempt.payload.product_id) await refreshDetail();
      onStockChanged();
      return "ok";
    } catch (cause) {
      const uncertain = !(cause instanceof IpcError) || cause.code === "storage_error" || cause.code === "unknown_error";
      if (uncertain) {
        setError("No se pudo confirmar el resultado. Reintenta sin modificar los datos; se usará la misma operación.");
        return "uncertain";
      }
      onPendingWasteChange(null);
      const message = cause instanceof IpcError ? cause.message : "No fue posible registrar la merma.";
      setError(message);
      setWasteError(message);
      return "error";
    } finally { setBusy(false); }
  }

  /** Abre la revisión de impacto; todavía no modifica existencias. */
  function reviewWaste(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setWasteError(null);
    if (!canWaste || readOnly || wasteLocked || !productId || !wasteQuantity || !wasteReason.trim()) return;
    const quantity = parseQuantity(wasteQuantity);
    if (quantity === null || quantity <= 0) {
      setWasteError("Escribe una cantidad mayor que cero, con hasta tres decimales.");
      return;
    }
    if (stock === null) {
      setWasteError("Consulta las existencias del producto antes de registrar la merma.");
      return;
    }
    const resultingStock = subtractQuantity(stock, wasteQuantity);
    if (resultingStock === null) {
      setWasteError("La merma no puede superar el stock actual.");
      return;
    }
    setWasteReview({
      quantity: wasteQuantity,
      reason: wasteReason.trim(),
      currentStock: formatQuantity(stock),
      resultingStock,
    });
  }

  async function confirmWaste() {
    if (!wasteReview) return;
    const payload: WasteDraft = {
      product_id: productId,
      quantity: wasteReview.quantity,
      reason: wasteReview.reason,
      idempotency_key: crypto.randomUUID(),
    };
    const result = await sendWaste({
      fingerprint: [payload.product_id, payload.quantity, payload.reason].join("|"),
      payload,
    });
    if (result === "ok" || result === "uncertain") setWasteReview(null);
  }

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canManage || readOnly) return;
    setError(null);
    setNotice(null);

    const parsed = inventoryMovementDraftSchema.safeParse({
      product_id: productId,
      kind,
      quantity,
      reason: reason.trim() ? reason.trim() : undefined,
    });
    if (!parsed.success) {
      setError(parsed.error.issues[0]?.message ?? "Revisa los datos.");
      return;
    }
    if (Number(quantity) <= 0) {
      setError("La cantidad debe ser mayor que cero.");
      return;
    }
    if ((kind === "adjustment_in" || kind === "adjustment_out") && !reason.trim()) {
      setError("Los ajustes requieren un motivo.");
      return;
    }
    if (
      (kind === "outbound" || kind === "adjustment_out") &&
      !window.confirm("¿Confirmas este movimiento que reduce existencias?")
    ) {
      return;
    }

    setBusy(true);
    try {
      await api.recordInventoryMovement(parsed.data);
      setNotice("Movimiento registrado.");
      setQuantity("");
      setReason("");
      await refreshDetail();
      onStockChanged();
    } catch (cause) {
      setError(
        cause instanceof IpcError ? cause.message : "No fue posible registrar el movimiento.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="screen inventory-screen">
      <header className="screen-header">
        <div>
          <h1>Inventario</h1>
          <p className="screen-subtitle">
            Existencias por sucursal y movimientos de inventario.
          </p>
        </div>
        <label className="field field-inline">
          <span>Producto</span>
          <select
            value={productId}
            onChange={(event) => {
              const next = event.target.value;
              // Repetir la misma opción no cambia la selección: sin esta guarda el
              // panel se quedaba mostrando "…" porque no se disparaba la lectura.
              if (next === productId) return;
              requestRef.current += 1;
              setProductId(next);
              setStock(null);
              setMovements([]);
              setError(null);
              setNotice(null);
              setLoadingDetail(Boolean(next));
            }}
          >
            <option value="">Selecciona un producto</option>
            {products.map((product) => (
              <option key={product.id} value={product.id}>
                {product.sku} · {product.name}
              </option>
            ))}
          </select>
        </label>
      </header>

      {(error || notice) && (
        <div aria-live="polite" className="screen-inline-alert">
          {error && <p className="alert alert-error">{error}</p>}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      {pendingWaste && (
        <section className="surface attention" aria-label="Merma pendiente de confirmar">
          <header className="surface-header">
            <div>
              <h2>Merma pendiente de confirmar</h2>
              <p className="muted">
                Se reintentará la misma operación: no se descontará dos veces el inventario.
              </p>
            </div>
          </header>
          <dl className="mini-grid">
            <div>
              <dt>Producto</dt>
              <dd>
                {products.find((product) => product.id === pendingWaste.payload.product_id)?.name ??
                  pendingWaste.payload.product_id}
              </dd>
            </div>
            <div>
              <dt>Cantidad</dt>
              <dd className="mono">{formatQuantity(pendingWaste.payload.quantity)}</dd>
            </div>
            <div>
              <dt>Motivo</dt>
              <dd>{pendingWaste.payload.reason}</dd>
            </div>
          </dl>
          <button
            type="button"
            className="button button-primary"
            onClick={() => void sendWaste(pendingWaste)}
            disabled={busy || readOnly}
          >
            {busy ? "Reintentando…" : "Reintentar merma"}
          </button>
        </section>
      )}

      {loading ? (
        <p className="muted">Cargando inventario…</p>
      ) : !selectedProduct ? (
        <div className="empty-state">
          <p>Selecciona un producto para ver sus existencias.</p>
          <p className="muted">
            El historial muestra cada entrada, salida y ajuste con su motivo.
          </p>
        </div>
      ) : (
        <div className="inventory-layout">
          <section className="surface" aria-label="Existencias del producto">
            <header className="surface-header">
              <div>
                <h2>{selectedProduct.name}</h2>
                <p className="muted">Existencias de la sucursal activa</p>
              </div>
              <span className="stock-figure">
                {loadingDetail ? "…" : formatQuantity(stock ?? "0")}
              </span>
            </header>

            <dl className="mini-grid">
              <div>
                <dt>SKU</dt>
                <dd className="mono">{selectedProduct.sku}</dd>
              </div>
              <div>
                <dt>Al público</dt>
                <dd className="mono">${selectedProduct.public_unit_total}</dd>
              </div>
              <div>
                <dt>IVA</dt>
                <dd className="mono">{(selectedProduct.tax_rate_bp / 100).toFixed(2)}%</dd>
              </div>
            </dl>

            {canManage && !readOnly && (
              <form className="form-grid form-compact movement-form" onSubmit={submit} noValidate>
                <h3>Registrar movimiento</h3>
                <div className="field-row">
                  <label className="field field-short">
                    <span>Tipo</span>
                    <select
                      value={kind}
                      onChange={(event) => setKind(event.target.value as typeof kind)}
                    >
                      <option value="inbound">Entrada</option>
                      <option value="outbound">Salida</option>
                      <option value="adjustment_in">Ajuste +</option>
                      <option value="adjustment_out">Ajuste -</option>
                    </select>
                  </label>
                  <label className="field field-short">
                    <span>Cantidad</span>
                    <input
                      inputMode="decimal"
                      value={quantity}
                      onChange={(event) => setQuantity(event.target.value)}
                      placeholder="0"
                      required
                    />
                  </label>
                </div>
                <label className="field">
                  <span>Motivo (solo ajustes)</span>
                  <input value={reason} onChange={(event) => setReason(event.target.value)} />
                </label>
                <button type="submit" className="button button-primary button-block" disabled={busy}>
                  Registrar movimiento
                </button>
              </form>
            )}
            {actor.permissions.includes("waste.manage") && !readOnly && !pendingWaste && (
              <form className="form-grid form-compact movement-form" onSubmit={reviewWaste} noValidate>
                <h3>Registrar merma</h3>
                <p className="muted">Reduce existencias; no genera gasto ni salida de caja.</p>
                <div className="field-row"><label className="field field-short"><span>Cantidad</span><input inputMode="decimal" value={wasteQuantity} onChange={(event) => { setWasteQuantity(event.target.value); setWasteError(null); }} placeholder="0" required /></label><label className="field"><span>Motivo</span><select value={wasteReason} onChange={(event) => setWasteReason(event.target.value)}><option value="caducidad">Caducidad</option><option value="daño">Daño</option><option value="pérdida">Pérdida</option><option value="otro">Otro</option></select></label></div>
                <button type="submit" className="button button-quiet" disabled={busy || !wasteQuantity || wasteLocked}>Revisar merma</button>
                {wasteError && !wasteReview && (
                  <p className="alert alert-error" role="alert">{wasteError}</p>
                )}
              </form>
            )}
          </section>

          <section className="surface" aria-label="Historial de movimientos">
            <header className="surface-header">
              <div>
                <h2>Movimientos recientes</h2>
                <p className="muted">Cada entrada, salida y ajuste con su motivo</p>
              </div>
              <span className="badge badge-neutral">{movements.length}</span>
            </header>

            {loadingDetail ? (
              <p className="muted empty-cell">Consultando inventario…</p>
            ) : movements.length === 0 ? (
              <div className="empty-state">
                <p>Sin movimientos registrados.</p>
                <p className="muted">Registra una entrada para comenzar a vender.</p>
              </div>
            ) : (
              <div className="table-scroll">
                <table className="data-table ledger-table">
                  <thead>
                    <tr>
                      <th>Tipo</th>
                      <th>Cantidad</th>
                      <th>Motivo</th>
                      <th className="align-end">Fecha</th>
                    </tr>
                  </thead>
                  <tbody>
                    {movements.map((movement) => (
                      <tr key={movement.id}>
                        <td>
                          <span
                            className={
                              movement.kind === "inbound" || movement.kind === "adjustment_in"
                                ? "ledger-kind ledger-in"
                                : "ledger-kind ledger-out"
                            }
                          >
                            {KIND_LABELS[movement.kind] ?? movement.kind}
                          </span>
                        </td>
                        <td className="mono">{movement.quantity}</td>
                        <td>{movement.reason ?? "Sin motivo"}</td>
                        <td className="align-end ledger-date">
                          {formatDate(movement.occurred_at)}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>
        </div>
      )}

      {wasteReview && selectedProduct && (
        <Modal
          title="Registrar merma"
          subtitle={`${selectedProduct.name} · ${WASTE_REASON_LABELS[wasteReview.reason] ?? wasteReview.reason}`}
          onClose={() => {
            if (!busy) setWasteReview(null);
          }}
          footer={
            <>
              <button
                type="button"
                className="button"
                onClick={() => setWasteReview(null)}
                disabled={busy}
              >
                Cancelar
              </button>
              <button
                type="button"
                className="button button-danger"
                onClick={() => void confirmWaste()}
                disabled={busy}
              >
                {busy ? "Registrando…" : "Registrar merma"}
              </button>
            </>
          }
        >
          <dl className="receipt">
            <div>
              <dt>Producto</dt>
              <dd>{selectedProduct.name}</dd>
            </div>
            <div>
              <dt>Stock actual</dt>
              <dd className="mono">{wasteReview.currentStock}</dd>
            </div>
            <div>
              <dt>Merma</dt>
              <dd className="mono">−{wasteReview.quantity}</dd>
            </div>
            <div className="receipt-total">
              <dt>Stock resultante</dt>
              <dd className="mono">
                {wasteReview.currentStock} → {wasteReview.resultingStock}
              </dd>
            </div>
            <div>
              <dt>Motivo</dt>
              <dd>{WASTE_REASON_LABELS[wasteReview.reason] ?? wasteReview.reason}</dd>
            </div>
          </dl>
          <p className="alert alert-warning" role="status">
            Se descontará {wasteReview.quantity} de {selectedProduct.name} del inventario.
            Esta acción no se puede deshacer.
          </p>
          {wasteError && (
            <p className="alert alert-error" role="alert">
              {wasteError}
            </p>
          )}
        </Modal>
      )}
    </div>
  );
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("es-MX", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}
