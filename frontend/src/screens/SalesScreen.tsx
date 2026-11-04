import { useMemo, useRef, useState } from "react";
import { api, IpcError } from "../api";
import type {
  Actor,
  CashRegister,
  CashShift,
  Category,
  ModifierOption,
  Product,
  SaleView,
} from "../contracts";
import {
  cartTotals,
  lineFromConfiguration,
  lineTotalMinor,
  mergeLine,
  type CartLine,
} from "../lib/cart";
import { formatMinor, formatQuantity } from "../lib/money";
import { CheckoutDialog } from "./CheckoutDialog";
import { ProductConfigDialog, type ConfigTarget } from "./ProductConfigDialog";
import { MinusIcon, PlusIcon, SearchIcon, TrashIcon } from "../shell/icons";

interface Props {
  actor: Actor;
  readOnly: boolean;
  products: Product[];
  categories: Category[];
  stockLevels: Record<string, string>;
  registers: CashRegister[];
  registerId: string;
  onRegisterChange: (registerId: string) => void;
  onRegisterRequired: () => void;
  shift: CashShift | null;
  cart: CartLine[];
  onCartChange: (cart: CartLine[]) => void;
  onSaleCompleted: (sale: SaleView) => void;
  onOpenCatalog: () => void;
  loading: boolean;
}

function stockOf(levels: Record<string, string>, productId: string): number | null {
  const raw = levels[productId];
  if (raw === undefined) return null;
  const value = Number(raw);
  return Number.isFinite(value) ? value : null;
}

export function SalesScreen({
  actor,
  readOnly,
  products,
  categories,
  stockLevels,
  registers,
  registerId,
  onRegisterChange,
  onRegisterRequired,
  shift,
  cart,
  onCartChange,
  onSaleCompleted,
  onOpenCatalog,
  loading,
}: Props) {
  const canSell = actor.permissions.includes("sales.sell");
  const [query, setQuery] = useState("");
  const [categoryId, setCategoryId] = useState("");
  const [configTarget, setConfigTarget] = useState<ConfigTarget | null>(null);
  const [checkoutOpen, setCheckoutOpen] = useState(false);
  const [selectingId, setSelectingId] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<"catalog" | "ticket">("catalog");
  const requestRef = useRef(0);

  const totals = cartTotals(cart);

  const visibleProducts = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return products.filter((product) => {
      if (!product.active) return false;
      const matchesCategory = !categoryId || product.category_id === categoryId;
      const matchesQuery =
        !normalized ||
        product.name.toLowerCase().includes(normalized) ||
        product.sku.toLowerCase().includes(normalized);
      return matchesCategory && matchesQuery;
    });
  }, [products, categoryId, query]);

  function addLine(line: CartLine) {
    onCartChange(mergeLine(cart, line));
    setError(null);
  }

  async function handleProduct(product: Product) {
    if (readOnly) return;
    const request = ++requestRef.current;
    setSelectingId(product.id);
    setError(null);
    try {
      const [variants, groups] = await Promise.all([
        api.listProductVariants(product.id),
        api.listModifierGroups(product.id),
      ]);
      if (request !== requestRef.current) return;

      if (variants.length === 0 && groups.length === 0) {
        const configuration = await api.configureProduct({
          product_id: product.id,
          variant_id: null,
          selected_option_ids: [],
        });
        if (request !== requestRef.current) return;
        addLine(lineFromConfiguration(configuration, 1));
        return;
      }

      const optionsByGroup: Record<string, ModifierOption[]> = {};
      for (const group of groups) {
        optionsByGroup[group.id] = await api.listModifierOptions(group.id);
      }
      if (request !== requestRef.current) return;
      setConfigTarget({ product, variants, groups, optionsByGroup });
    } catch (cause) {
      if (request === requestRef.current) {
        setError(
          cause instanceof IpcError
            ? cause.message
            : "No fue posible preparar el producto.",
        );
      }
    } finally {
      if (request === requestRef.current) setSelectingId(null);
    }
  }

  function updateQuantity(key: string, delta: number) {
    onCartChange(
      cart
        .map((line) =>
          line.key === key ? { ...line, quantity: line.quantity + delta } : line,
        )
        .filter((line) => line.quantity > 0),
    );
  }

  function removeLine(key: string) {
    onCartChange(cart.filter((line) => line.key !== key));
  }

  if (!canSell) {
    return (
      <div className="empty-state">
        <p>Tu usuario no puede registrar ventas.</p>
        <p className="muted">Solicita el permiso de venta al propietario.</p>
      </div>
    );
  }

  return (
    <div className="sales" data-view={view}>
      <section className="sales-catalog" aria-labelledby="sales-title">
        <header className="screen-header">
          <div>
            <h1 id="sales-title">Ventas</h1>
            <p className="screen-subtitle">Selecciona productos y cobra el ticket actual.</p>
          </div>
          <div className="screen-actions">
            <label className="field field-inline">
              <span>Caja</span>
              <select
                value={registerId}
                onChange={(event) => onRegisterChange(event.target.value)}
                disabled={registers.length === 0}
              >
                {registers.length === 0 && <option value="">Sin cajas</option>}
                {registers.map((register) => (
                  <option key={register.id} value={register.id}>
                    {register.name} · {register.code}
                  </option>
                ))}
              </select>
            </label>
          </div>
        </header>

        <div className="catalog-toolbar">
          <label className="search-field">
            <SearchIcon className="search-icon" />
            <span className="visually-hidden">Buscar producto</span>
            <input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Buscar por nombre o SKU"
              aria-label="Buscar producto"
            />
          </label>

          <div className="chips" role="group" aria-label="Filtrar por categoría">
            <button
              type="button"
              className={categoryId === "" ? "chip chip-active" : "chip"}
              onClick={() => setCategoryId("")}
              aria-pressed={categoryId === ""}
            >
              Todas
            </button>
            {categories.map((category) => (
              <button
                key={category.id}
                type="button"
                className={categoryId === category.id ? "chip chip-active" : "chip"}
                onClick={() => setCategoryId(category.id)}
                aria-pressed={categoryId === category.id}
              >
                {category.name}
              </button>
            ))}
          </div>
        </div>

        {error && (
          <div className="screen-inline-alert" role="alert">
            <p className="alert alert-error">{error}</p>
          </div>
        )}

        <div className="product-scroll">
          {loading ? (
            <p className="muted">Cargando catálogo…</p>
          ) : visibleProducts.length === 0 ? (
            <div className="empty-state">
              <p>No hay productos que coincidan.</p>
              <p className="muted">
                Ajusta la búsqueda o registra productos en la sección Productos.
              </p>
              <button type="button" className="button" onClick={onOpenCatalog}>
                Ir a Productos
              </button>
            </div>
          ) : (
            <div className="product-grid" aria-label="Productos disponibles">
              {visibleProducts.map((product) => {
                const stock = stockOf(stockLevels, product.id);
                const soldOut = stock !== null && stock <= 0;
                return (
                  <button
                    key={product.id}
                    type="button"
                    className="product-card"
                    onClick={() => void handleProduct(product)}
                    disabled={readOnly || soldOut || selectingId === product.id}
                    aria-label={`${product.name}, ${product.sku}`}
                  >
                    <span className="product-card-name">{product.name}</span>
                    <span className="product-card-sku">{product.sku}</span>
                    <span className="product-card-footer">
                      <span className="product-card-price">${product.unit_price}</span>
                      {product.tax_rate_bp > 0 && (
                        <span className="product-card-tax">+ IVA</span>
                      )}
                    </span>
                    <span
                      className={soldOut ? "product-card-stock stock-empty" : "product-card-stock"}
                    >
                      {stock === null
                        ? "Sin existencias registradas"
                        : soldOut
                          ? "Agotado"
                          : `${formatQuantity(String(stock))} disp.`}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </section>

      <aside className="ticket" aria-labelledby="ticket-title">
        <header className="ticket-header">
          <div>
            <h2 id="ticket-title">Ticket actual</h2>
            <p className="ticket-subtitle">
              {totals.items === 0
                ? "Sin artículos"
                : `${totals.items} ${totals.items === 1 ? "artículo" : "artículos"}`}
            </p>
          </div>
          {cart.length > 0 && !readOnly && (
            <button
              type="button"
              className="button button-quiet"
              onClick={() => onCartChange([])}
            >
              Vaciar
            </button>
          )}
        </header>

        <div className="ticket-lines">
          {cart.length === 0 ? (
            <p className="muted ticket-empty">
              Selecciona un producto del catálogo para comenzar la venta.
            </p>
          ) : (
            <ul className="ticket-list">
              {cart.map((line) => (
                <li key={line.key} className="ticket-line">
                  <div className="ticket-line-main">
                    <span className="ticket-line-name">{line.name}</span>
                    <span className="ticket-line-meta">
                      {line.quantity} × ${line.unitTotal}
                      {line.variantName ? ` · ${line.variantName}` : ""}
                    </span>
                    {line.modifiers.length > 0 && (
                      <span className="ticket-line-meta">{line.modifiers.join(", ")}</span>
                    )}
                  </div>
                  <div className="ticket-line-side">
                    <div className="stepper stepper-small">
                      <button
                        type="button"
                        className="stepper-button"
                        onClick={() => updateQuantity(line.key, -1)}
                        aria-label={`Disminuir ${line.name}`}
                      >
                        <MinusIcon size={14} />
                      </button>
                      <span className="stepper-value">{line.quantity}</span>
                      <button
                        type="button"
                        className="stepper-button"
                        onClick={() => updateQuantity(line.key, 1)}
                        aria-label={`Aumentar ${line.name}`}
                      >
                        <PlusIcon size={14} />
                      </button>
                    </div>
                    <span className="ticket-line-total mono">
                      ${formatMinor(lineTotalMinor(line))}
                    </span>
                    <button
                      type="button"
                      className="icon-button icon-button-danger"
                      onClick={() => removeLine(line.key)}
                      aria-label={`Quitar ${line.name}`}
                    >
                      <TrashIcon size={15} />
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </div>

        <footer className="ticket-footer">
          <dl className="ticket-totals">
            <div>
              <dt>Subtotal</dt>
              <dd>${totals.subtotal}</dd>
            </div>
            <div>
              <dt>Impuestos</dt>
              <dd>${totals.tax}</dd>
            </div>
            <div className="ticket-total">
              <dt>Total</dt>
              <dd>${totals.total}</dd>
            </div>
          </dl>

          {!shift && (
            <p className="ticket-hint">
              Abre un turno de caja para cobrar.{" "}
              <button type="button" className="link-button" onClick={onRegisterRequired}>
                Ir a caja
              </button>
            </p>
          )}

          <button
            type="button"
            className="button button-primary button-lg"
            onClick={() => setCheckoutOpen(true)}
            disabled={cart.length === 0 || !shift || readOnly}
          >
            Cobrar ${totals.total}
          </button>
        </footer>
      </aside>

      <div className="sales-mobile-bar">
        <button
          type="button"
          className="button button-quiet"
          onClick={() => setView(view === "catalog" ? "ticket" : "catalog")}
        >
          {view === "catalog"
            ? `Ver ticket (${totals.items})`
            : "Volver al catálogo"}
        </button>
        <strong className="mono">${totals.total}</strong>
        <button
          type="button"
          className="button button-primary"
          onClick={() => setCheckoutOpen(true)}
          disabled={cart.length === 0 || !shift || readOnly}
        >
          Cobrar
        </button>
      </div>

      {configTarget && (
        <ProductConfigDialog
          target={configTarget}
          onClose={() => setConfigTarget(null)}
          onAdded={addLine}
        />
      )}

      {checkoutOpen && shift && (
        <CheckoutDialog
          cart={cart}
          shift={shift}
          readOnly={readOnly}
          onClose={() => setCheckoutOpen(false)}
          onCompleted={(sale) => {
            onSaleCompleted(sale);
            setView("catalog");
          }}
        />
      )}
    </div>
  );
}
