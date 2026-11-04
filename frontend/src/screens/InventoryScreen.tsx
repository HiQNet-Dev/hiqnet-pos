import { useEffect, useRef, useState } from "react";
import { api, IpcError } from "../api";
import {
  inventoryMovementDraftSchema,
  type Actor,
  type InventoryMovement,
  type Product,
} from "../contracts";
import { formatQuantity } from "../lib/money";

interface Props {
  actor: Actor;
  readOnly: boolean;
  products: Product[];
  loading: boolean;
  onStockChanged: () => void;
}

const KIND_LABELS: Record<string, string> = {
  inbound: "Entrada",
  outbound: "Salida",
  adjustment_in: "Ajuste +",
  adjustment_out: "Ajuste −",
};

export function InventoryScreen({ actor, readOnly, products, loading, onStockChanged }: Props) {
  const canManage = actor.permissions.includes("inventory.manage");
  const [productId, setProductId] = useState("");
  const [kind, setKind] = useState<"inbound" | "outbound" | "adjustment_in" | "adjustment_out">(
    "inbound",
  );
  const [quantity, setQuantity] = useState("");
  const [reason, setReason] = useState("");
  const [stock, setStock] = useState<string | null>(null);
  const [movements, setMovements] = useState<InventoryMovement[]>([]);
  const [busy, setBusy] = useState(false);
  const [loadingDetail, setLoadingDetail] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const requestRef = useRef(0);

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
            Existencias por sucursal y ledger de movimientos.
          </p>
        </div>
        <label className="field field-inline">
          <span>Producto</span>
          <select
            value={productId}
            onChange={(event) => {
              requestRef.current += 1;
              setProductId(event.target.value);
              setStock(null);
              setMovements([]);
              setError(null);
              setNotice(null);
              setLoadingDetail(Boolean(event.target.value));
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

      {loading ? (
        <p className="muted">Cargando catálogo…</p>
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
                <p className="eyebrow">Existencia actual</p>
                <h2>{selectedProduct.name}</h2>
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
                <dt>Precio neto</dt>
                <dd className="mono">${selectedProduct.unit_price}</dd>
              </div>
              <div>
                <dt>Impuesto</dt>
                <dd className="mono">{(selectedProduct.tax_rate_bp / 100).toFixed(2)}%</dd>
              </div>
            </dl>

            {canManage && !readOnly && (
              <form className="form-grid form-compact" onSubmit={submit} noValidate>
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
                  <label className="field">
                    <span>Motivo (ajustes)</span>
                    <input value={reason} onChange={(event) => setReason(event.target.value)} />
                  </label>
                </div>
                <div className="form-actions">
                  <button type="submit" className="button button-primary" disabled={busy}>
                    Registrar
                  </button>
                </div>
              </form>
            )}
          </section>

          <section className="surface" aria-label="Historial de movimientos">
            <header className="surface-header">
              <div>
                <p className="eyebrow">Ledger</p>
                <h2>Historial reciente</h2>
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
              <ol className="ledger">
                {movements.map((movement) => (
                  <li key={movement.id}>
                    <span
                      className={
                        movement.kind === "inbound" || movement.kind === "adjustment_in"
                          ? "ledger-kind ledger-in"
                          : "ledger-kind ledger-out"
                      }
                    >
                      {KIND_LABELS[movement.kind] ?? movement.kind}
                    </span>
                    <span className="ledger-quantity mono">{movement.quantity}</span>
                    <span className="ledger-reason">{movement.reason ?? "sin motivo"}</span>
                    <span className="ledger-date">{formatDate(movement.occurred_at)}</span>
                  </li>
                ))}
              </ol>
            )}
          </section>
        </div>
      )}
    </div>
  );
}

function formatDate(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("es-MX", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(date);
}
