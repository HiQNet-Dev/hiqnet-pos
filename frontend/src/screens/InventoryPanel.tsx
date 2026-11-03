import { useCallback, useEffect, useRef, useState } from "react";
import { api, IpcError } from "../api";
import {
  inventoryMovementDraftSchema,
  type Actor,
  type InventoryMovement,
  type Product,
} from "../contracts";

interface Props {
  actor: Actor;
  products: Product[];
  readOnly: boolean;
}

export function InventoryPanel({ actor, products, readOnly }: Props) {
  const canManage = actor.permissions.includes("inventory.manage");
  const [productId, setProductId] = useState<string>("");
  const [kind, setKind] = useState<"inbound" | "outbound" | "adjustment_in" | "adjustment_out">("inbound");
  const [quantity, setQuantity] = useState<string>("");
  const [reason, setReason] = useState<string>("");
  const [stock, setStock] = useState<string | null>(null);
  const [movements, setMovements] = useState<InventoryMovement[]>([]);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const requestId = useRef(0);

  const selectedProduct = products.find((product) => product.id === productId) ?? null;

  const load = useCallback(async () => {
    const currentRequest = ++requestId.current;
    if (!productId) {
      setStock(null);
      setMovements([]);
      setLoading(false);
      return;
    }
    setLoading(true);
    setStock(null);
    setMovements([]);
    try {
      const [currentStock, history] = await Promise.all([
        api.currentStock(productId),
        api.listInventoryMovements(productId, 20),
      ]);
      if (currentRequest !== requestId.current) return;
      setStock(currentStock);
      setMovements(history);
    } catch (cause) {
      if (currentRequest !== requestId.current) return;
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible consultar el inventario.",
      );
    } finally {
      if (currentRequest === requestId.current) setLoading(false);
    }
  }, [productId]);

  useEffect(() => {
    void load();
  }, [load]);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
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
    if (quantity.trim() === "0" || quantity.trim() === "0.0") {
      setError("La cantidad debe ser mayor que cero.");
      return;
    }
    if (
      (kind === "adjustment_in" || kind === "adjustment_out") &&
      !reason.trim()
    ) {
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
      await load();
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible registrar el movimiento.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel" aria-labelledby="inventory-title">
      <header className="panel-header">
        <div>
          <p className="eyebrow">Inventario</p>
          <h2 id="inventory-title">Existencias y movimientos</h2>
        </div>
        {stock !== null && selectedProduct && (
          <span className="badge badge-neutral">
            {selectedProduct.name}: {stock}
          </span>
        )}
      </header>

      <div className="field-row">
        <label className="field">
          <span>Producto</span>
          <select
            value={productId}
            onChange={(event) => setProductId(event.target.value)}
          >
            <option value="">Selecciona un producto</option>
            {products.map((product) => (
              <option key={product.id} value={product.id}>
                {product.sku} · {product.name}
              </option>
            ))}
          </select>
        </label>
      </div>

      {(error || notice) && (
        <div aria-live="polite">
          {error && <p className="alert alert-error">{error}</p>}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      {canManage && !readOnly && (
        <form className="form-grid form-compact" onSubmit={handleSubmit} noValidate>
          <div className="field-row">
            <label className="field field-short">
              <span>Tipo</span>
              <select
                value={kind}
                onChange={(event) =>
                  setKind(event.target.value as typeof kind)
                }
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
              <input
                value={reason}
                onChange={(event) => setReason(event.target.value)}
              />
            </label>
            <div className="field field-action">
              <button type="submit" className="button button-primary" disabled={busy || !productId}>
                Registrar
              </button>
            </div>
          </div>
        </form>
      )}

      <div>
        <p className="eyebrow">Historial reciente</p>
        {loading ? (
          <p className="muted">Consultando inventario…</p>
        ) : movements.length === 0 ? (
          <p className="muted">Sin movimientos registrados.</p>
        ) : (
          <ol className="timeline">
            {movements.map((movement) => (
              <li key={movement.id}>
                <span className="timeline-action">
                  {movement.kind.replace("_", " ")}
                </span>
                <span className="timeline-meta">
                  {movement.quantity} · {movement.reason ?? "sin motivo"} · {formatDate(movement.occurred_at)}
                </span>
              </li>
            ))}
          </ol>
        )}
      </div>
    </section>
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