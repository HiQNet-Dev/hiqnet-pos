import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import {
  type Actor,
  type Category,
  type ModifierGroup,
  type ModifierOption,
  type Product,
  type ProductVariant,
} from "../contracts";
import { PlusIcon, SearchIcon } from "../shell/icons";
import { messageOf } from "./catalog/messages";
import { VariantEditor } from "./catalog/VariantEditor";
import { GroupEditor } from "./catalog/GroupEditor";
import { OptionEditor } from "./catalog/OptionEditor";
import { ConfigurationPreview } from "./catalog/ConfigurationPreview";
import { ProductCreateDialog } from "./catalog/ProductCreateDialog";
import { CategoryCreateDialog } from "./catalog/CategoryCreateDialog";

interface Props {
  actor: Actor;
  readOnly: boolean;
  products: Product[];
  categories: Category[];
  loading: boolean;
  onDataChanged: () => void;
}

interface DetailData {
  variants: ProductVariant[];
  groups: ModifierGroup[];
  optionsByGroup: Record<string, ModifierOption[]>;
}

/** Lectura pura de la configuración de un producto (sin estado de React). */
async function fetchDetail(productId: string): Promise<DetailData> {
  const [variants, groups] = await Promise.all([
    api.listProductVariants(productId),
    api.listModifierGroups(productId),
  ]);
  const optionsByGroup: Record<string, ModifierOption[]> = {};
  for (const group of groups) {
    optionsByGroup[group.id] = await api.listModifierOptions(group.id);
  }
  return { variants, groups, optionsByGroup };
}

export function CatalogScreen({
  actor,
  readOnly,
  products,
  categories,
  loading,
  onDataChanged,
}: Props) {
  const canManage = actor.permissions.includes("catalog.manage");
  const [query, setQuery] = useState("");
  const [categoryFilter, setCategoryFilter] = useState("");
  const [selectedId, setSelectedId] = useState("");
  const [createProduct, setCreateProduct] = useState(false);
  const [createCategory, setCreateCategory] = useState(false);

  const [variants, setVariants] = useState<ProductVariant[]>([]);
  const [groups, setGroups] = useState<ModifierGroup[]>([]);
  const [optionsByGroup, setOptionsByGroup] = useState<Record<string, ModifierOption[]>>({});
  const [detailFor, setDetailFor] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const requestRef = useRef(0);

  // Selección derivada: si el producto elegido desaparece se usa el primero.
  const selected = products.find((product) => product.id === selectedId) ?? products[0] ?? null;
  const effectiveId = selected?.id ?? "";
  const detailLoading = Boolean(effectiveId) && detailFor !== effectiveId;

  const visible = products.filter((product) => {
    const normalized = query.trim().toLowerCase();
    const matchesQuery =
      !normalized ||
      product.name.toLowerCase().includes(normalized) ||
      product.sku.toLowerCase().includes(normalized);
    const matchesCategory = !categoryFilter || product.category_id === categoryFilter;
    return matchesQuery && matchesCategory;
  });

  const loadDetail = useCallback(async (productId: string) => {
    if (!productId) return;
    const request = ++requestRef.current;
    try {
      const detail = await fetchDetail(productId);
      if (request !== requestRef.current) return;
      setVariants(detail.variants);
      setGroups(detail.groups);
      setOptionsByGroup(detail.optionsByGroup);
      setDetailFor(productId);
    } catch (cause) {
      if (request === requestRef.current) {
        setError(messageOf(cause, "No fue posible cargar la configuración del producto."));
      }
    }
  }, []);

  useEffect(() => {
    if (!effectiveId) return;
    let cancelled = false;
    const request = ++requestRef.current;
    fetchDetail(effectiveId)
      .then((detail) => {
        if (cancelled || request !== requestRef.current) return;
        setVariants(detail.variants);
        setGroups(detail.groups);
        setOptionsByGroup(detail.optionsByGroup);
        setDetailFor(effectiveId);
      })
      .catch((cause: unknown) => {
        if (cancelled || request !== requestRef.current) return;
        setError(messageOf(cause, "No fue posible cargar la configuración del producto."));
      });
    return () => {
      cancelled = true;
    };
  }, [effectiveId]);

  async function refreshAll(message?: string) {
    onDataChanged();
    await loadDetail(effectiveId);
    if (message) setNotice(message);
  }

  if (!canManage) {
    return (
      <div className="empty-state">
        <p>Tu usuario no administra el catálogo.</p>
        <p className="muted">Puedes vender desde la sección Ventas.</p>
      </div>
    );
  }

  return (
    <div className="screen catalog-screen">
      <header className="screen-header">
        <div>
          <h1>Productos y catálogo</h1>
          <p className="screen-subtitle">
            Precios netos y precio al público con el impuesto ya incluido.
          </p>
        </div>
        <div className="screen-actions">
          <button
            type="button"
            className="button"
            onClick={() => setCreateCategory(true)}
            disabled={readOnly}
          >
            Nueva categoría
          </button>
          <button
            type="button"
            className="button button-primary"
            onClick={() => setCreateProduct(true)}
            disabled={readOnly}
          >
            <PlusIcon size={16} /> Nuevo producto
          </button>
        </div>
      </header>

      {(error || notice) && (
        <div aria-live="polite" className="screen-inline-alert">
          {error && <p className="alert alert-error">{error}</p>}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      <div className="catalog-layout">
        <section className="surface" aria-label="Listado de productos">
          <div className="surface-toolbar">
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
            <label className="field field-inline">
              <span>Categoría</span>
              <select
                value={categoryFilter}
                onChange={(event) => setCategoryFilter(event.target.value)}
              >
                <option value="">Todas</option>
                {categories.map((category) => (
                  <option key={category.id} value={category.id}>
                    {category.name}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {loading ? (
            <p className="muted empty-cell">Cargando productos…</p>
          ) : visible.length === 0 ? (
            <div className="empty-state">
              <p>Sin productos que coincidan.</p>
              <p className="muted">Crea el primero con “Nuevo producto”.</p>
            </div>
          ) : (
            <div className="table-scroll">
              <table className="data-table">
                <caption className="visually-hidden">Productos registrados</caption>
                <thead>
                  <tr>
                    <th scope="col" className="col-sku">SKU</th>
                    <th scope="col">Producto</th>
                    <th scope="col" className="col-category">Categoría</th>
                    <th scope="col" className="align-end">
                      Precio neto
                    </th>
                    <th scope="col" className="align-end">
                      IVA
                    </th>
                    <th scope="col" className="align-end">
                      Al público
                    </th>
                  </tr>
                </thead>
                <tbody>
                  {visible.map((product) => {
                    const category = categories.find(
                      (item) => item.id === product.category_id,
                    );
                    return (
                      <tr
                        key={product.id}
                        className={
                          product.id === effectiveId ? "data-row data-row-active" : "data-row"
                        }
                        onClick={() => setSelectedId(product.id)}
                      >
                        <td className="mono col-sku">
                          <button
                            type="button"
                            className="row-button"
                            onClick={() => setSelectedId(product.id)}
                            aria-current={product.id === effectiveId}
                          >
                            {product.sku}
                          </button>
                        </td>
                        <td>{product.name}</td>
                        <td className="muted col-category">
                          {category?.name ?? "Sin categoría"}
                        </td>
                        <td className="align-end mono">${product.unit_price}</td>
                        <td className="align-end mono">
                          {(product.tax_rate_bp / 100).toFixed(2)}%
                        </td>
                        <td className="align-end mono product-public-price">
                          ${product.public_unit_total}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}
        </section>

        <aside className="surface detail-panel" aria-label="Detalle del producto">
          {!selected ? (
            <div className="empty-state">
              <p>Sin producto seleccionado.</p>
              <p className="muted">
                Crea un producto con “Nuevo producto” para configurar variantes y
                modificadores.
              </p>
            </div>
          ) : (
            <>
              <header className="detail-header">
                <div>
                  <p className="sku-label mono">{selected.sku}</p>
                  <h2>{selected.name}</h2>
                </div>
                <span className="badge badge-neutral">${selected.public_unit_total} IVA incluido</span>
              </header>

              <div className="detail-scroll">
                <section className="detail-block">
                  <h3>Categoría</h3>
                  <select
                    className="input"
                    value={selected.category_id ?? ""}
                    disabled={readOnly}
                    aria-label="Categoría del producto"
                    onChange={(event) => void assignCategory(selected.id, event.target.value)}
                  >
                    <option value="">Sin categoría</option>
                    {categories.map((category) => (
                      <option key={category.id} value={category.id}>
                        {category.name}
                      </option>
                    ))}
                  </select>
                </section>

                <section className="detail-block">
                  <h3>Variantes</h3>
                  {detailLoading ? (
                    <p className="muted">Cargando variantes…</p>
                  ) : variants.length === 0 ? (
                    <p className="muted">Sin variantes: se vende con el precio base.</p>
                  ) : (
                    <ul className="tag-list">
                      {variants.map((variant) => (
                        <li key={variant.id} className="tag">
                          <span>{variant.name}</span>
                          <span className="mono">${variant.unit_price}</span>
                        </li>
                      ))}
                    </ul>
                  )}
                  <VariantEditor
                    disabled={readOnly || detailLoading}
                    onCreate={async (name, price) => {
                      try {
                        await api.createProductVariant({
                          product_id: selected.id,
                          name,
                          price,
                          sort_order: variants.length,
                        });
                        await refreshAll(`Variante ${name} creada.`);
                        setError(null);
                        return true;
                      } catch (cause) {
                        setError(messageOf(cause, "No fue posible crear la variante."));
                        return false;
                      }
                    }}
                  />
                </section>

                <section className="detail-block">
                  <h3>Modificadores</h3>
                  {detailLoading ? (
                    <p className="muted">Cargando grupos…</p>
                  ) : groups.length === 0 ? (
                    <p className="muted">Sin grupos de opciones.</p>
                  ) : (
                    <div className="group-stack">
                      {groups.map((group) => (
                        <article key={group.id} className="modifier-group">
                          <header>
                            <span className="modifier-group-name">{group.name}</span>
                            <span className="muted">
                              {group.min > 0
                                ? group.min === group.max
                                  ? `Obligatorio · elige ${group.min}`
                                  : `Obligatorio · entre ${group.min} y ${group.max}`
                                : `Opcional · hasta ${group.max}`}
                            </span>
                          </header>
                          <ul className="tag-list">
                            {(optionsByGroup[group.id] ?? []).map((option) => (
                              <li key={option.id} className="tag">
                                <span>{option.name}</span>
                                <span className="mono">
                                  {option.kind === "extra"
                                    ? `+$${option.price_delta}`
                                    : option.kind === "omission"
                                      ? "omisión"
                                      : "sin costo"}
                                </span>
                              </li>
                            ))}
                          </ul>
                          <OptionEditor
                            disabled={readOnly}
                            onCreate={async (draft) => {
                              try {
                                await api.createModifierOption({
                                  group_id: group.id,
                                  name: draft.name,
                                  kind: draft.kind,
                                  price_delta: draft.price_delta,
                                  sort_order: (optionsByGroup[group.id] ?? []).length,
                                });
                                await refreshAll(`Opción ${draft.name} creada.`);
                                setError(null);
                                return true;
                              } catch (cause) {
                                setError(messageOf(cause, "No fue posible crear la opción."));
                                return false;
                              }
                            }}
                          />
                        </article>
                      ))}
                    </div>
                  )}

                  <GroupEditor
                    disabled={readOnly}
                    onCreate={async (draft) => {
                      try {
                        await api.createModifierGroup({
                          product_id: selected.id,
                          name: draft.name,
                          min: draft.min,
                          max: draft.max,
                          sort_order: groups.length,
                        });
                        await refreshAll(`Grupo ${draft.name} creado.`);
                        setError(null);
                        return true;
                      } catch (cause) {
                        setError(messageOf(cause, "No fue posible crear el grupo."));
                        return false;
                      }
                    }}
                  />
                </section>

                <ConfigurationPreview
                  key={selected.id}
                  productId={selected.id}
                  variants={variants}
                  groups={groups}
                  optionsByGroup={optionsByGroup}
                />
              </div>
            </>
          )}
        </aside>
      </div>

      {createProduct && (
        <ProductCreateDialog
          categories={categories}
          onClose={() => setCreateProduct(false)}
          onCreated={async (name) => {
            setCreateProduct(false);
            await refreshAll(`Producto ${name} creado.`);
            setError(null);
          }}
          onError={(message) => setError(message)}
        />
      )}

      {createCategory && (
        <CategoryCreateDialog
          nextOrder={categories.length}
          onClose={() => setCreateCategory(false)}
          onCreated={async (name) => {
            setCreateCategory(false);
            await refreshAll(`Categoría ${name} creada.`);
            setError(null);
          }}
          onError={(message) => setError(message)}
        />
      )}
    </div>
  );

  async function assignCategory(productId: string, categoryId: string) {
    try {
      await api.assignProductCategory(productId, categoryId || null);
      await refreshAll("Categoría actualizada.");
      setError(null);
    } catch (cause) {
      setError(messageOf(cause, "No fue posible cambiar la categoría."));
    }
  }
}
