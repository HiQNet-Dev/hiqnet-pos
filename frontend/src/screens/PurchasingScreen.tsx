import { useCallback, useEffect, useRef, useState } from "react";
import { api, IpcError } from "../api";
import {
  purchaseOrderDraftSchema,
  purchaseReceiptDraftSchema,
  supplierDraftSchema,
  type Actor,
  type Product,
  type PurchaseOrder,
  type Supplier,
} from "../contracts";
import { Modal } from "../components/Modal";
import { displayMoney, formatQuantity } from "../lib/money";

export interface PendingReceiptAttempt {
  orderId: string;
  fingerprint: string;
  key: string;
  quantities: Record<string, string>;
}

interface Props {
  actor: Actor;
  readOnly: boolean;
  products: Product[];
  onInventoryChanged: () => void;
  pendingReceipt: PendingReceiptAttempt | null;
  onPendingReceiptChange: (attempt: PendingReceiptAttempt | null) => void;
}

const STATUS: Record<PurchaseOrder["status"], string> = {
  draft: "Borrador",
  ordered: "Confirmada",
  partially_received: "Recepción parcial",
  received: "Recibida",
};

function messageOf(cause: unknown, fallback: string) {
  return cause instanceof IpcError ? cause.message : fallback;
}

export function PurchasingScreen({
  actor,
  readOnly,
  products,
  onInventoryChanged,
  pendingReceipt,
  onPendingReceiptChange,
}: Props) {
  const canManage = actor.permissions.includes("purchasing.manage");
  const [suppliers, setSuppliers] = useState<Supplier[]>([]);
  const [orders, setOrders] = useState<PurchaseOrder[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [supplierOpen, setSupplierOpen] = useState(false);
  const [orderOpen, setOrderOpen] = useState(false);
  const [receiptOrder, setReceiptOrder] = useState<PurchaseOrder | null>(null);
  const [busy, setBusy] = useState(false);
  const requestRef = useRef(0);

  const refresh = useCallback(async () => {
    const request = ++requestRef.current;
    setLoading(true);
    try {
      const [nextSuppliers, nextOrders] = await Promise.all([api.listSuppliers(), api.listPurchaseOrders()]);
      if (request !== requestRef.current) return;
      setSuppliers(nextSuppliers);
      setOrders(nextOrders);
      setError(null);
    } catch (cause) {
      if (request === requestRef.current) setError(messageOf(cause, "No fue posible cargar las compras."));
    } finally {
      if (request === requestRef.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    const timer = window.setTimeout(() => void refresh(), 0);
    return () => window.clearTimeout(timer);
  }, [refresh]);

  async function confirm(order: PurchaseOrder) {
    setBusy(true); setError(null);
    try {
      await api.confirmPurchaseOrder(order.id);
      setNotice(`Orden ${order.folio} confirmada.`);
      await refresh();
    } catch (cause) { setError(messageOf(cause, "No fue posible confirmar la orden.")); }
    finally { setBusy(false); }
  }

  if (!canManage) return <div className="empty-state"><p>No tienes permiso para administrar compras.</p></div>;

  return <div className="screen purchasing-screen">
    <header className="screen-header">
      <div><h1>Compras</h1><p className="screen-subtitle">Las existencias aumentan solo al confirmar una recepción.</p></div>
      <div className="screen-actions">
        <button type="button" className="button button-quiet" onClick={() => setSupplierOpen(true)} disabled={readOnly}>Nuevo proveedor</button>
        <button type="button" className="button button-primary" onClick={() => setOrderOpen(true)} disabled={readOnly || suppliers.filter((item) => item.active).length === 0}>Nueva orden</button>
      </div>
    </header>
    {(error || notice) && <div className="screen-inline-alert" aria-live="polite">{error && <p className="alert alert-error">{error}</p>}{notice && <p className="alert alert-success">{notice}</p>}</div>}
    <div className="purchasing-layout">
      <section className="surface">
        <header className="surface-header"><div><h2>Proveedores</h2><p className="muted">Disponibles para nuevas órdenes.</p></div><span className="badge badge-neutral">{suppliers.length}</span></header>
        {loading ? <p className="muted">Cargando proveedores…</p> : suppliers.length === 0 ? <div className="empty-state"><p>Aún no hay proveedores.</p><p className="muted">Registra el primero para preparar una orden.</p></div> : <div className="table-scroll"><table className="data-table"><thead><tr><th>Proveedor</th><th>Código</th><th>Contacto</th><th>Estado</th></tr></thead><tbody>{suppliers.map((supplier) => <tr key={supplier.id}><td>{supplier.name}</td><td className="mono">{supplier.code ?? "—"}</td><td>{supplier.contact ?? "—"}</td><td><span className={supplier.active ? "badge badge-success" : "badge badge-neutral"}>{supplier.active ? "Activo" : "Inactivo"}</span></td></tr>)}</tbody></table></div>}
      </section>
      <section className="surface purchase-orders">
        <header className="surface-header"><div><h2>Órdenes de compra</h2><p className="muted">Crear una orden no afecta el inventario.</p></div><button type="button" className="button button-quiet" onClick={() => void refresh()} disabled={loading}>Actualizar</button></header>
        {loading ? <p className="muted">Cargando órdenes…</p> : orders.length === 0 ? <div className="empty-state"><p>Sin órdenes registradas.</p><p className="muted">Crea una orden y confírmala antes de recibir mercancía.</p></div> : <div className="table-scroll"><table className="data-table"><thead><tr><th>Folio</th><th>Estado</th><th>Partidas</th><th className="align-end">Acciones</th></tr></thead><tbody>{orders.map((order) => <tr key={order.id}><td className="mono">{order.folio}</td><td><span className="badge badge-neutral">{STATUS[order.status]}</span></td><td>{order.lines.length}</td><td className="align-end">{order.status === "draft" && <button type="button" className="button button-quiet" onClick={() => void confirm(order)} disabled={busy || readOnly}>Confirmar</button>}{(order.status === "ordered" || order.status === "partially_received") && <button type="button" className="button button-primary" onClick={() => setReceiptOrder(order)} disabled={busy || readOnly || (pendingReceipt !== null && pendingReceipt.orderId !== order.id)}>Recibir</button>}</td></tr>)}</tbody></table></div>}
      </section>
    </div>
    {supplierOpen && <SupplierDialog onClose={() => setSupplierOpen(false)} onSaved={async () => { setSupplierOpen(false); setNotice("Proveedor creado."); await refresh(); }} />}
    {orderOpen && <OrderDialog suppliers={suppliers.filter((item) => item.active)} products={products.filter((item) => item.active)} onClose={() => setOrderOpen(false)} onSaved={async () => { setOrderOpen(false); setNotice("Orden creada como borrador."); await refresh(); }} />}
    {pendingReceipt && <p className="banner banner-warning" role="status">Hay una recepción pendiente de confirmar. Ábrela para reintentar sin duplicar inventario.</p>}
    {receiptOrder && <ReceiptDialog order={receiptOrder} products={products} pendingAttempt={pendingReceipt?.orderId === receiptOrder.id ? pendingReceipt : null} onPending={onPendingReceiptChange} onClose={() => setReceiptOrder(null)} onSaved={async () => { onPendingReceiptChange(null); setReceiptOrder(null); setNotice("Recepción registrada e inventario actualizado."); onInventoryChanged(); await refresh(); }} />}
  </div>;
}

function SupplierDialog({ onClose, onSaved }: { onClose: () => void; onSaved: () => Promise<void> }) {
  const [name, setName] = useState(""); const [code, setCode] = useState(""); const [contact, setContact] = useState(""); const [error, setError] = useState<string | null>(null); const [busy, setBusy] = useState(false);
  async function submit(event: React.FormEvent) { event.preventDefault(); const parsed = supplierDraftSchema.safeParse({ name, code: code || undefined, contact: contact || undefined }); if (!parsed.success) { setError(parsed.error.issues[0]?.message ?? "Revisa los datos."); return; } setBusy(true); try { await api.createSupplier(parsed.data); await onSaved(); } catch (cause) { setError(messageOf(cause, "No fue posible crear el proveedor.")); } finally { setBusy(false); } }
  return <Modal title="Nuevo proveedor" onClose={onClose}><form className="form-grid" onSubmit={submit} noValidate>{error && <p className="alert alert-error" role="alert">{error}</p>}<label className="field"><span>Nombre</span><input autoFocus value={name} onChange={(event) => setName(event.target.value)} required /></label><label className="field"><span>Código</span><input value={code} onChange={(event) => setCode(event.target.value)} /></label><label className="field"><span>Contacto</span><input value={contact} onChange={(event) => setContact(event.target.value)} /></label><div className="dialog-actions"><button type="button" className="button button-quiet" onClick={onClose}>Cancelar</button><button type="submit" className="button button-primary" disabled={busy}>{busy ? "Guardando…" : "Crear proveedor"}</button></div></form></Modal>;
}

function OrderDialog({ suppliers, products, onClose, onSaved }: { suppliers: Supplier[]; products: Product[]; onClose: () => void; onSaved: () => Promise<void> }) {
  const [supplierId, setSupplierId] = useState(""); const [productId, setProductId] = useState(""); const [quantity, setQuantity] = useState("1"); const [unitCost, setUnitCost] = useState("0.00"); const [lines, setLines] = useState<{ product_id: string; quantity: string; unit_cost: string }[]>([]); const [error, setError] = useState<string | null>(null); const [busy, setBusy] = useState(false);
  function addLine() { if (!productId) return; if (lines.some((line) => line.product_id === productId)) { setError("El producto ya está en la orden."); return; } setLines((current) => [...current, { product_id: productId, quantity, unit_cost: unitCost }]); setProductId(""); setError(null); }
  async function submit(event: React.FormEvent) { event.preventDefault(); const parsed = purchaseOrderDraftSchema.safeParse({ supplier_id: supplierId, lines }); if (!parsed.success) { setError(parsed.error.issues[0]?.message ?? "Revisa las partidas."); return; } setBusy(true); try { await api.createPurchaseOrder(parsed.data); await onSaved(); } catch (cause) { setError(messageOf(cause, "No fue posible crear la orden.")); } finally { setBusy(false); } }
  return <Modal title="Nueva orden de compra" onClose={onClose}><form className="form-grid" onSubmit={submit} noValidate>{error && <p className="alert alert-error" role="alert">{error}</p>}<label className="field"><span>Proveedor</span><select value={supplierId} onChange={(event) => setSupplierId(event.target.value)} required><option value="">Selecciona un proveedor</option>{suppliers.map((supplier) => <option key={supplier.id} value={supplier.id}>{supplier.name}</option>)}</select></label><div className="field-row"><label className="field"><span>Producto</span><select value={productId} onChange={(event) => setProductId(event.target.value)}><option value="">Selecciona un producto</option>{products.map((product) => <option key={product.id} value={product.id}>{product.sku} · {product.name}</option>)}</select></label><label className="field field-short"><span>Cantidad</span><input inputMode="decimal" value={quantity} onChange={(event) => setQuantity(event.target.value)} /></label><label className="field field-short"><span>Costo neto</span><input inputMode="decimal" value={unitCost} onChange={(event) => setUnitCost(event.target.value)} /></label><button type="button" className="button button-quiet" onClick={addLine}>Agregar</button></div>{lines.length > 0 && <ul className="compact-list">{lines.map((line) => <li key={line.product_id}>{products.find((product) => product.id === line.product_id)?.name ?? line.product_id} · {formatQuantity(line.quantity)} × {displayMoney(line.unit_cost)}<button type="button" className="button-link" onClick={() => setLines((current) => current.filter((item) => item.product_id !== line.product_id))}>Quitar</button></li>)}</ul>}<div className="dialog-actions"><button type="button" className="button button-quiet" onClick={onClose}>Cancelar</button><button type="submit" className="button button-primary" disabled={busy || lines.length === 0}>{busy ? "Guardando…" : "Crear borrador"}</button></div></form></Modal>;
}

function ReceiptDialog({ order, products, pendingAttempt, onPending, onClose, onSaved }: { order: PurchaseOrder; products: Product[]; pendingAttempt: PendingReceiptAttempt | null; onPending: (attempt: PendingReceiptAttempt | null) => void; onClose: () => void; onSaved: () => Promise<void> }) {
  const [quantities, setQuantities] = useState<Record<string, string>>(() => pendingAttempt?.quantities ?? Object.fromEntries(order.lines.map((line) => [line.id, "0"])));
  const [attempt, setAttempt] = useState<{ fingerprint: string; key: string } | null>(() => pendingAttempt ? { fingerprint: pendingAttempt.fingerprint, key: pendingAttempt.key } : null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const locked = attempt !== null;

  async function submit(event: React.FormEvent) {
    event.preventDefault();
    const lines = order.lines.map((line) => ({ order_line_id: line.id, quantity: quantities[line.id] ?? "0" })).filter((line) => Number(line.quantity) > 0);
    const fingerprint = JSON.stringify({ order_id: order.id, lines });
    const operation = attempt?.fingerprint === fingerprint ? attempt : { fingerprint, key: crypto.randomUUID() };
    const parsed = purchaseReceiptDraftSchema.safeParse({ order_id: order.id, idempotency_key: operation.key, lines });
    if (!parsed.success) { setError(parsed.error.issues[0]?.message ?? "Indica una cantidad válida."); return; }
    setAttempt(operation); setBusy(true); setError(null);
    try { await api.receivePurchaseOrder(parsed.data); await onSaved(); }
    catch (cause) {
      const uncertain = cause instanceof IpcError && (cause.code === "unknown_error" || cause.code === "storage_error");
      if (uncertain) {
        onPending({ orderId: order.id, fingerprint: operation.fingerprint, key: operation.key, quantities });
        setError("No se confirmó el resultado. Reintenta sin modificar las cantidades; se usará la misma operación.");
      } else {
        setAttempt(null);
        onPending(null);
        setError(messageOf(cause, "No fue posible registrar la recepción."));
      }
    } finally { setBusy(false); }
  }
  return <Modal title={`Recibir ${order.folio}`} onClose={onClose}><form className="form-grid" onSubmit={submit} noValidate>{error && <p className="alert alert-error" role="alert">{error}</p>}<p className="muted">Solo las cantidades confirmadas incrementan existencias.</p><div className="receipt-lines">{order.lines.map((line) => { const product = products.find((item) => item.id === line.product_id); const pending = Math.max(0, Number(line.quantity) - Number(line.received_quantity)); return <div className="receipt-line" key={line.id}><div><strong>{product?.name ?? line.product_id}</strong><small>Solicitado {formatQuantity(line.quantity)} · Pendiente {formatQuantity(String(pending))}</small></div><label className="field field-short"><span>Recibir</span><input inputMode="decimal" value={quantities[line.id] ?? "0"} disabled={locked} onChange={(event) => setQuantities((current) => ({ ...current, [line.id]: event.target.value }))} /></label></div>; })}</div><div className="dialog-actions"><button type="button" className="button button-quiet" onClick={onClose} disabled={busy}>Cancelar</button><button type="submit" className="button button-primary" disabled={busy}>{busy ? "Registrando…" : locked ? "Reintentar recepción" : "Confirmar recepción"}</button></div></form></Modal>;
}
