import { useEffect, useMemo, useState } from "react";
import { api, IpcError } from "../api";
import type {
  Actor,
  Category,
  Product,
  ProductVariant,
} from "../contracts";

interface Props {
  actor: Actor;
  products: Product[];
  readOnly: boolean;
  onProductsChanged?: () => Promise<void>;
}

export function CatalogPanel({
  actor,
  products,
  readOnly,
  onProductsChanged,
}: Props) {
  const canManage = actor.permissions.includes("catalog.manage");
  const [categories, setCategories] = useState<Category[]>([]);
  const [selectedCategory, setSelectedCategory] = useState<string>("");
  const [query, setQuery] = useState("");
  const [selectedProduct, setSelectedProduct] = useState<Product | null>(null);
  const [selectedVariantId, setSelectedVariantId] = useState<string | null>(null);
  const [variants, setVariants] = useState<ProductVariant[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [categoryName, setCategoryName] = useState("");
  const [variantName, setVariantName] = useState("");
  const [variantPrice, setVariantPrice] = useState("");
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .listCategories()
      .then((list) => {
        if (!cancelled) setCategories(list);
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(
            cause instanceof IpcError
              ? cause.message
              : "No fue posible cargar las categorías.",
          );
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!selectedProduct) return;
    let cancelled = false;
    api
      .listProductVariants(selectedProduct.id)
      .then((list) => {
        if (!cancelled) {
          setVariants(list);
          setSelectedVariantId(list[0]?.id ?? null);
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(
            cause instanceof IpcError
              ? cause.message
              : "No fue posible cargar las variantes.",
          );
        }
      });
    return () => {
      cancelled = true;
    };
  }, [selectedProduct]);

  const visibleProducts = useMemo(() => {
    const normalized = query.trim().toLowerCase();
    return products.filter((product) => {
      const matchesCategory =
        !selectedCategory || product.category_id === selectedCategory;
      const matchesQuery =
        !normalized ||
        product.name.toLowerCase().includes(normalized) ||
        product.sku.toLowerCase().includes(normalized);
      return matchesCategory && matchesQuery;
    });
  }, [products, selectedCategory, query]);

  async function handleAssignCategory(value: string) {
    if (!canManage || readOnly || !selectedProduct) return;
    setBusy(true);
    setError(null);
    try {
      const updated = await api.assignProductCategory(
        selectedProduct.id,
        value || null,
      );
      setSelectedProduct(updated);
      setNotice("Categoría del producto actualizada.");
      await onProductsChanged?.();
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible cambiar la categoría.",
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleCreateCategory(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canManage || readOnly) return;
    setBusy(true);
    setError(null);
    try {
      const category = await api.createCategory({
        name: categoryName,
        sort_order: categories.length,
      });
      setCategories((current) => [...current, category]);
      setCategoryName("");
      setNotice(`Categoría ${category.name} creada.`);
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible crear la categoría.",
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleCreateVariant(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canManage || readOnly || !selectedProduct) return;
    setBusy(true);
    setError(null);
    try {
      const variant = await api.createProductVariant({
        product_id: selectedProduct.id,
        name: variantName,
        price: variantPrice,
        sort_order: variants.length,
      });
      setVariants((current) => [...current, variant]);
      setVariantName("");
      setVariantPrice("");
      setNotice(`Variante ${variant.name} creada.`);
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible crear la variante.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel" aria-labelledby="catalog-panel-title">
      <header className="panel-header">
        <div>
          <p className="eyebrow">Catálogo</p>
          <h2 id="catalog-panel-title">Categorías, productos y variantes</h2>
        </div>
        <span className="badge badge-neutral">{visibleProducts.length} productos</span>
      </header>

      {(error || notice) && (
        <div aria-live="polite">
          {error && <p className="alert alert-error">{error}</p>}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      <div className="field-row">
        <label className="field field-short">
          <span>Categoría</span>
          <select
            value={selectedCategory}
            onChange={(event) => setSelectedCategory(event.target.value)}
          >
            <option value="">Todas</option>
            {categories.map((category) => (
              <option key={category.id} value={category.id}>
                {category.name}
              </option>
            ))}
          </select>
        </label>
        <label className="field">
          <span>Buscar</span>
          <input
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="Nombre o SKU"
          />
        </label>
      </div>

      {canManage && readOnly === false && (
        <form className="form-grid form-compact" onSubmit={handleCreateCategory}>
          <div className="field-row">
            <label className="field">
              <span>Nueva categoría</span>
              <input
                value={categoryName}
                onChange={(event) => setCategoryName(event.target.value)}
                required
              />
            </label>
            <div className="field field-action">
              <button type="submit" className="button button-secondary" disabled={busy}>
                Crear categoría
              </button>
            </div>
          </div>
        </form>
      )}

      <div className="card-grid" role="list" aria-label="Productos del catálogo">
        {visibleProducts.length === 0 ? (
          <p className="muted">No hay productos que coincidan.</p>
        ) : (
          visibleProducts.map((product) => (
            <button
              key={product.id}
              type="button"
              className={
                selectedProduct?.id === product.id
                  ? "product-card product-card-active"
                  : "product-card"
              }
              onClick={() => {
                setSelectedProduct(product);
                setSelectedVariantId(null);
              }}
              aria-pressed={selectedProduct?.id === product.id}
            >
              <span className="product-card-sku">{product.sku}</span>
              <span className="product-card-name">{product.name}</span>
              <span className="product-card-price">${product.unit_price}</span>
              {!product.active && <span className="badge badge-warning">Inactivo</span>}
            </button>
          ))
        )}
      </div>

      {selectedProduct && (
        <div className="catalog-detail">
          <h3>{selectedProduct.name}</h3>
          {canManage && !readOnly && (
            <div className="field-row">
              <label className="field field-short">
                <span>Categoría del producto</span>
                <select
                  value={selectedProduct.category_id ?? ""}
                  onChange={(event) => void handleAssignCategory(event.target.value)}
                  disabled={busy}
                >
                  <option value="">Sin categoría</option>
                  {categories.map((category) => (
                    <option key={category.id} value={category.id}>
                      {category.name}
                    </option>
                  ))}
                </select>
              </label>
            </div>
          )}
          {variants.length === 0 ? (
            <p className="muted">Este producto no tiene variantes.</p>
          ) : (
            <ul className="variant-list" aria-label="Variantes disponibles">
              {variants.map((variant) => (
                <li key={variant.id}>
                  <button
                    type="button"
                    className={
                      selectedVariantId === variant.id
                        ? "variant-button variant-button-active"
                        : "variant-button"
                    }
                    onClick={() => setSelectedVariantId(variant.id)}
                    aria-pressed={selectedVariantId === variant.id}
                    disabled={!variant.active}
                  >
                    <span>{variant.name}</span>
                    <span>${variant.unit_price}</span>
                    {!variant.active && <span className="badge badge-warning">Inactiva</span>}
                  </button>
                </li>
              ))}
            </ul>
          )}

          {canManage && !readOnly && (
            <form className="form-grid form-compact" onSubmit={handleCreateVariant}>
              <div className="field-row">
                <label className="field">
                  <span>Nueva variante</span>
                  <input
                    value={variantName}
                    onChange={(event) => setVariantName(event.target.value)}
                    required
                  />
                </label>
                <label className="field field-short">
                  <span>Precio</span>
                  <input
                    inputMode="decimal"
                    value={variantPrice}
                    onChange={(event) => setVariantPrice(event.target.value)}
                    placeholder="0.00"
                    required
                  />
                </label>
                <div className="field field-action">
                  <button type="submit" className="button button-primary" disabled={busy}>
                    Crear variante
                  </button>
                </div>
              </div>
            </form>
          )}
        </div>
      )}
    </section>
  );
}