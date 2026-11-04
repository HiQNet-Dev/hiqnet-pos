import { useEffect, useMemo, useRef, useState } from "react";
import { api, IpcError } from "../api";
import type {
  Actor,
  Category,
  CheckoutDraft,
  ModifierGroup,
  ModifierOption,
  Product,
  ProductVariant,
  SaleView,
} from "../contracts";

interface Props {
  actor: Actor;
  readOnly: boolean;
}

interface CartLine {
  key: string;
  productId: string;
  variantId: string | null;
  selectedOptionIds: string[];
  name: string;
  quantity: number;
  unitTotal: string;
}

export function SalesPanel({ actor, readOnly }: Props) {
  const canSell = actor.permissions.includes("sales.sell");
  const [products, setProducts] = useState<Product[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [selectedCategory, setSelectedCategory] = useState("");
  const [query, setQuery] = useState("");
  const [selectedProduct, setSelectedProduct] = useState<Product | null>(null);
  const [variants, setVariants] = useState<ProductVariant[]>([]);
  const [variantId, setVariantId] = useState("");
  const [groups, setGroups] = useState<ModifierGroup[]>([]);
  const [optionsByGroup, setOptionsByGroup] = useState<Record<string, ModifierOption[]>>({});
  const [selectedOptionIds, setSelectedOptionIds] = useState<string[]>([]);
  const [cart, setCart] = useState<CartLine[]>([]);
  const [shiftId, setShiftId] = useState("");
  const [cashReceived, setCashReceived] = useState("");
  const [sale, setSale] = useState<SaleView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const idempotencyKey = useRef<string>("");

  useEffect(() => {
    if (!canSell) return;
    let cancelled = false;
    Promise.all([api.listProducts(), api.listCategories()])
      .then(([productList, categoryList]) => {
        if (cancelled) return;
        setProducts(productList.filter((product) => product.active));
        setCategories(categoryList);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(cause instanceof IpcError ? cause.message : "No fue posible cargar el catálogo.");
        }
      });
    return () => {
      cancelled = true;
    };
  }, [canSell]);

  useEffect(() => {
    if (!canSell) return;
    let cancelled = false;
    api
      .listCashRegisters()
      .then(async (registers) => {
        if (cancelled || registers.length === 0) return;
        const register = registers[0];
        if (!register) return;
        const active = await api.activeCashShift(register.id);
        if (!cancelled) setShiftId(active?.id ?? "");
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [canSell]);

  useEffect(() => {
    if (!selectedProduct) return;
    let cancelled = false;
    api
      .listProductVariants(selectedProduct.id)
      .then((list) => {
        if (!cancelled) {
          setVariants(list);
          setVariantId(list[0]?.id ?? "");
        }
      })
      .catch(() => {});
    api
      .listModifierGroups(selectedProduct.id)
      .then(async (list) => {
        if (cancelled) return;
        setGroups(list);
        const map: Record<string, ModifierOption[]> = {};
        for (const group of list) {
          map[group.id] = await api.listModifierOptions(group.id);
        }
        if (!cancelled) setOptionsByGroup(map);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [selectedProduct]);

  const visibleProducts = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return products.filter((product) => {
      const matchesCategory = !selectedCategory || product.category_id === selectedCategory;
      const matchesQuery =
        !normalized ||
        product.name.toLowerCase().includes(normalized) ||
        product.sku.toLowerCase().includes(normalized);
      return matchesCategory && matchesQuery;
    });
  }, [products, selectedCategory, query]);

  function toggleOption(optionId: string) {
    setSelectedOptionIds((current) =>
      current.includes(optionId)
        ? current.filter((id) => id !== optionId)
        : [...current, optionId],
    );
  }

  async function addToCart() {
    if (!selectedProduct) return;
    setBusy(true);
    setError(null);
    try {
      const configuration = await api.configureProduct({
        product_id: selectedProduct.id,
        variant_id: variantId || null,
        selected_option_ids: selectedOptionIds,
      });
      const modifiers = configuration.selected_options.map((item) => item.name).join(", ");
      const name = configuration.variant_name
        ? `${configuration.product_name} · ${configuration.variant_name}`
        : configuration.product_name;
      setCart((current) => [
        ...current,
        {
          key: `${Date.now()}-${Math.random()}`,
          productId: selectedProduct.id,
          variantId: variantId || null,
          selectedOptionIds: [...selectedOptionIds],
          name: modifiers ? `${name} (${modifiers})` : name,
          quantity: 1,
          unitTotal: configuration.unit_price,
        },
      ]);
      setNotice("Producto agregado al carrito.");
    } catch (cause) {
      setError(cause instanceof IpcError ? cause.message : "No fue posible configurar el producto.");
    } finally {
      setBusy(false);
    }
  }

  function updateQuantity(key: string, quantity: number) {
    setCart((current) =>
      current
        .map((line) => (line.key === key ? { ...line, quantity } : line))
        .filter((line) => line.quantity > 0),
    );
  }

  async function checkout(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSell || readOnly || cart.length === 0 || !shiftId) return;
    setBusy(true);
    setError(null);
    setSale(null);
    try {
      const draft: CheckoutDraft = {
        shift_id: shiftId,
        idempotency_key: idempotencyKey.current || crypto.randomUUID(),
        items: cart.map((line) => ({
          product_id: line.productId,
          variant_id: line.variantId,
          quantity: String(line.quantity),
          selected_option_ids: line.selectedOptionIds,
        })),
        cash_received: cashReceived,
      };
      idempotencyKey.current = draft.idempotency_key;
      const result = await api.checkoutSale(draft);
      setSale(result);
      setCart([]);
      setCashReceived("");
      idempotencyKey.current = "";
      setNotice(`Venta ${result.folio} confirmada.`);
    } catch (cause) {
      setError(cause instanceof IpcError ? cause.message : "No fue posible confirmar la venta.");
    } finally {
      setBusy(false);
    }
  }

  if (!canSell) return null;

  const total = cart.reduce((sum, line) => sum + Number.parseFloat(line.unitTotal) * line.quantity, 0);
  const change = Math.max(0, Number.parseFloat(cashReceived || "0") - total);

  return (
    <section className="panel" aria-labelledby="sales-title">
      <header className="panel-header">
        <div>
          <p className="eyebrow">Venta</p>
          <h2 id="sales-title">Punto de venta</h2>
        </div>
        <span className="badge badge-neutral">{cart.length} partidas</span>
      </header>

      {(error || notice) && (
        <div aria-live="polite">
          {error && <p className="alert alert-error">{error}</p>}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      <div className="sales-layout">
        <div>
          <div className="field-row">
            <label className="field field-short">
              <span>Categoría</span>
              <select value={selectedCategory} onChange={(event) => setSelectedCategory(event.target.value)}>
                <option value="">Todas</option>
                {categories.map((category) => (
                  <option key={category.id} value={category.id}>{category.name}</option>
                ))}
              </select>
            </label>
            <label className="field">
              <span>Buscar</span>
              <input value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Nombre o SKU" />
            </label>
          </div>

          <div className="card-grid" role="list" aria-label="Productos para venta">
            {visibleProducts.map((product) => (
              <button
                key={product.id}
                type="button"
                className={selectedProduct?.id === product.id ? "product-card product-card-active" : "product-card"}
                onClick={() => {
                  setSelectedProduct(product);
                  setSelectedOptionIds([]);
                }}
              >
                <span className="product-card-sku">{product.sku}</span>
                <span className="product-card-name">{product.name}</span>
                <span className="product-card-price">${product.unit_price}</span>
              </button>
            ))}
          </div>

          {selectedProduct && (
            <div className="catalog-detail">
              <h3>{selectedProduct.name}</h3>
              {variants.length > 0 && (
                <label className="field field-short">
                  <span>Variante</span>
                  <select value={variantId} onChange={(event) => setVariantId(event.target.value)}>
                    {variants.map((variant) => (
                      <option key={variant.id} value={variant.id}>{variant.name} · ${variant.unit_price}</option>
                    ))}
                  </select>
                </label>
              )}
              {groups.map((group) => (
                <fieldset key={group.id} style={{ border: 0, padding: 0 }}>
                  <legend>{group.name} (mín {group.min}, máx {group.max})</legend>
                  {(optionsByGroup[group.id] ?? []).map((option) => (
                    <label key={option.id} className="field field-short">
                      <input
                        type="checkbox"
                        checked={selectedOptionIds.includes(option.id)}
                        onChange={() => toggleOption(option.id)}
                      />
                      <span>{option.name}{option.kind === "extra" ? ` (+${option.price_delta})` : ""}</span>
                    </label>
                  ))}
                </fieldset>
              ))}
              <button type="button" className="button button-primary" disabled={busy} onClick={() => void addToCart()}>
                Agregar al carrito
              </button>
            </div>
          )}
        </div>

        <aside className="sales-cart" aria-labelledby="cart-title">
          <h3 id="cart-title">Carrito</h3>
          {cart.length === 0 ? (
            <p className="muted">Carrito vacío.</p>
          ) : (
            <ul className="timeline">
              {cart.map((line) => (
                <li key={line.key}>
                  <span className="timeline-action">{line.name}</span>
                  <span className="timeline-meta">
                    ${line.unitTotal} × {line.quantity}
                    <button type="button" className="button button-quiet" onClick={() => updateQuantity(line.key, line.quantity - 1)}>-</button>
                    <button type="button" className="button button-quiet" onClick={() => updateQuantity(line.key, line.quantity + 1)}>+</button>
                    <button type="button" className="button button-quiet" onClick={() => updateQuantity(line.key, 0)}>Quitar</button>
                  </span>
                </li>
              ))}
            </ul>
          )}

          <div className="cart-total">
            <span>Total</span>
            <strong>${total.toFixed(2)}</strong>
          </div>

          {!shiftId && <p className="alert alert-warning">Abre un turno de caja para cobrar.</p>}

          <form className="form-grid form-compact" onSubmit={checkout} noValidate>
            <label className="field field-short">
              <span>Efectivo recibido</span>
              <input
                inputMode="decimal"
                value={cashReceived}
                onChange={(event) => setCashReceived(event.target.value)}
                required
              />
            </label>
            <p className="muted">Cambio: ${change.toFixed(2)}</p>
            <button
              type="submit"
              className="button button-primary"
              disabled={busy || cart.length === 0 || !shiftId || readOnly}
            >
              Cobrar
            </button>
          </form>

          {sale && (
            <div className="alert alert-success" role="status">
              Folio {sale.folio}. Total ${sale.total}. Cambio ${sale.payment?.change ?? "0.00"}.
            </div>
          )}
        </aside>
      </div>
    </section>
  );
}