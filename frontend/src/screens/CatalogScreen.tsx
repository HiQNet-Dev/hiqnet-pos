import { useCallback, useEffect, useRef, useState } from "react";
import { api, IpcError } from "../api";
import {
  modifierGroupDraftSchema,
  modifierOptionDraftSchema,
  productDraftSchema,
  type Actor,
  type Category,
  type ModifierConfiguration,
  type ModifierGroup,
  type ModifierOption,
  type Product,
  type ProductVariant,
} from "../contracts";
import { Modal } from "../components/Modal";
import { PlusIcon, SearchIcon } from "../shell/icons";

interface Props {
  actor: Actor;
  readOnly: boolean;
  products: Product[];
  categories: Category[];
  loading: boolean;
  onDataChanged: () => void;
}

function messageOf(cause: unknown, fallback: string): string {
  return cause instanceof IpcError ? cause.message : fallback;
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
            Precios netos; el impuesto se calcula al vender.
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
                    <th scope="col">SKU</th>
                    <th scope="col">Producto</th>
                    <th scope="col">Categoría</th>
                    <th scope="col" className="align-end">
                      Precio
                    </th>
                    <th scope="col" className="align-end">
                      IVA
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
                        <td className="mono">
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
                        <td className="muted">{category?.name ?? "Sin categoría"}</td>
                        <td className="align-end mono">${product.unit_price}</td>
                        <td className="align-end mono">
                          {(product.tax_rate_bp / 100).toFixed(2)}%
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
                  <p className="eyebrow">{selected.sku}</p>
                  <h2>{selected.name}</h2>
                </div>
                <span className="badge badge-neutral">${selected.unit_price}</span>
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
                              {group.min > 0 ? `obligatorio · ${group.min} a ${group.max}` : `hasta ${group.max}`}
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

interface VariantEditorProps {
  disabled: boolean;
  onCreate: (name: string, price: string) => Promise<boolean>;
}

function VariantEditor({ disabled, onCreate }: VariantEditorProps) {
  const [name, setName] = useState("");
  const [price, setPrice] = useState("");
  const [busy, setBusy] = useState(false);

  return (
    <form
      className="inline-form"
      onSubmit={async (event) => {
        event.preventDefault();
        if (disabled || busy || !name.trim() || !price.trim()) return;
        setBusy(true);
        const created = await onCreate(name.trim(), price.trim());
        setBusy(false);
        if (created) {
          setName("");
          setPrice("");
        }
      }}
    >
      <label className="field">
        <span>Nueva variante</span>
        <input value={name} onChange={(event) => setName(event.target.value)} />
      </label>
      <label className="field field-short">
        <span>Precio</span>
        <input
          inputMode="decimal"
          value={price}
          onChange={(event) => setPrice(event.target.value)}
          placeholder="0.00"
        />
      </label>
      <button type="submit" className="button" disabled={disabled || busy}>
        Crear variante
      </button>
    </form>
  );
}

interface GroupDraft {
  name: string;
  min: number;
  max: number;
}

interface OptionDraft {
  name: string;
  kind: "choice" | "extra" | "omission";
  price_delta: string;
}

interface GroupEditorProps {
  disabled: boolean;
  onCreate: (draft: GroupDraft) => Promise<boolean>;
}

function GroupEditor({ disabled, onCreate }: GroupEditorProps) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [min, setMin] = useState("0");
  const [max, setMax] = useState("1");
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  if (!open) {
    return (
      <button type="button" className="button button-quiet" onClick={() => setOpen(true)} disabled={disabled}>
        <PlusIcon size={15} /> Nuevo grupo
      </button>
    );
  }

  return (
    <form
      className="inline-form inline-form-wrap"
      onSubmit={async (event) => {
        event.preventDefault();
        const parsed = modifierGroupDraftSchema.safeParse({
          product_id: "pending",
          name,
          min: Number(min),
          max: Number(max),
          sort_order: 0,
        });
        if (!parsed.success) {
          setLocalError(parsed.error.issues[0]?.message ?? "Revisa el grupo.");
          return;
        }
        setBusy(true);
        const created = await onCreate({ name: name.trim(), min: Number(min), max: Number(max) });
        setBusy(false);
        if (created) {
          setOpen(false);
          setName("");
          setMin("0");
          setMax("1");
          setLocalError(null);
        }
      }}
    >
      <label className="field">
        <span>Grupo</span>
        <input value={name} onChange={(event) => setName(event.target.value)} required />
      </label>
      <label className="field field-tiny">
        <span>Mín</span>
        <input
          inputMode="numeric"
          value={min}
          onChange={(event) => setMin(event.target.value.replace(/\D/g, ""))}
        />
      </label>
      <label className="field field-tiny">
        <span>Máx</span>
        <input
          inputMode="numeric"
          value={max}
          onChange={(event) => setMax(event.target.value.replace(/\D/g, ""))}
        />
      </label>
      <button type="submit" className="button button-primary" disabled={busy}>
        Crear grupo
      </button>
      <button
        type="button"
        className="button button-quiet"
        onClick={() => setOpen(false)}
        disabled={busy}
      >
        Cancelar
      </button>
      {localError && <p className="alert alert-error">{localError}</p>}
    </form>
  );
}

interface OptionEditorProps {
  disabled: boolean;
  onCreate: (draft: OptionDraft) => Promise<boolean>;
}

function OptionEditor({ disabled, onCreate }: OptionEditorProps) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");
  const [kind, setKind] = useState<OptionDraft["kind"]>("extra");
  const [delta, setDelta] = useState("0.00");
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  if (!open) {
    return (
      <button
        type="button"
        className="button button-quiet button-small"
        onClick={() => setOpen(true)}
        disabled={disabled}
      >
        <PlusIcon size={14} /> Opción
      </button>
    );
  }

  return (
    <form
      className="inline-form inline-form-wrap"
      onSubmit={async (event) => {
        event.preventDefault();
        const parsed = modifierOptionDraftSchema.safeParse({
          group_id: "pending",
          name,
          kind,
          price_delta: kind === "omission" ? "0.00" : delta,
          sort_order: 0,
        });
        if (!parsed.success) {
          setLocalError(parsed.error.issues[0]?.message ?? "Revisa la opción.");
          return;
        }
        setBusy(true);
        const created = await onCreate({
          name: name.trim(),
          kind,
          price_delta: kind === "omission" ? "0.00" : delta.trim(),
        });
        setBusy(false);
        if (created) {
          setOpen(false);
          setName("");
          setDelta("0.00");
          setLocalError(null);
        }
      }}
    >
      <label className="field">
        <span>Opción</span>
        <input value={name} onChange={(event) => setName(event.target.value)} required />
      </label>
      <label className="field field-short">
        <span>Tipo</span>
        <select value={kind} onChange={(event) => setKind(event.target.value as OptionDraft["kind"])}>
          <option value="extra">Extra</option>
          <option value="choice">Elección</option>
          <option value="omission">Omisión</option>
        </select>
      </label>
      <label className="field field-tiny">
        <span>Delta</span>
        <input
          inputMode="decimal"
          value={kind === "omission" ? "0.00" : delta}
          onChange={(event) => setDelta(event.target.value)}
          disabled={kind === "omission"}
        />
      </label>
      <button type="submit" className="button button-primary button-small" disabled={busy}>
        Agregar
      </button>
      <button type="button" className="button button-quiet" onClick={() => setOpen(false)}>
        Cancelar
      </button>
      {localError && <p className="alert alert-error">{localError}</p>}
    </form>
  );
}

interface PreviewProps {
  productId: string;
  variants: ProductVariant[];
  groups: ModifierGroup[];
  optionsByGroup: Record<string, ModifierOption[]>;
}

/** Validación de una configuración con el motor autoritativo de Rust.
 *
 * Se monta con `key={product.id}`, de modo que cambiar de producto reinicia el
 * formulario sin necesidad de sincronizar estado en un efecto.
 */
function ConfigurationPreview({ productId, variants, groups, optionsByGroup }: PreviewProps) {
  const [variantId, setVariantId] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const [result, setResult] = useState<ModifierConfiguration | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const effectiveVariant = variants.some((variant) => variant.id === variantId)
    ? variantId
    : (variants[0]?.id ?? "");

  async function validate() {
    setBusy(true);
    setError(null);
    try {
      setResult(
        await api.configureProduct({
          product_id: productId,
          variant_id: effectiveVariant || null,
          selected_option_ids: selected,
        }),
      );
    } catch (cause) {
      setResult(null);
      setError(messageOf(cause, "La configuración no es válida."));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="detail-block">
      <h3>Probar configuración</h3>
      <p className="muted">
        Comprueba reglas y desglose con el mismo motor que usa la venta.
      </p>

      {variants.length > 0 && (
        <label className="field">
          <span>Variante</span>
          <select value={effectiveVariant} onChange={(event) => setVariantId(event.target.value)}>
            {variants.map((variant) => (
              <option key={variant.id} value={variant.id}>
                {variant.name} · ${variant.unit_price}
              </option>
            ))}
          </select>
        </label>
      )}

      {groups.map((group) => (
        <fieldset key={group.id} className="preview-group">
          <legend>
            {group.name} {group.min > 0 ? `(obligatorio ${group.min})` : `(hasta ${group.max})`}
          </legend>
          {(optionsByGroup[group.id] ?? []).map((option) => (
            <label key={option.id} className="check-row">
              <input
                type={group.max === 1 ? "radio" : "checkbox"}
                name={`preview-${group.id}`}
                checked={selected.includes(option.id)}
                disabled={!option.active}
                onChange={() =>
                  setSelected((current) => {
                    const ids = (optionsByGroup[group.id] ?? []).map((item) => item.id);
                    const without = current.filter((id) => !ids.includes(id));
                    if (current.includes(option.id)) {
                      return current.filter((id) => id !== option.id);
                    }
                    return group.max === 1 ? [...without, option.id] : [...current, option.id];
                  })
                }
              />
              <span>{option.name}</span>
              {option.kind === "extra" && <span className="mono">+${option.price_delta}</span>}
            </label>
          ))}
        </fieldset>
      ))}

      <button
        type="button"
        className="button"
        onClick={() => void validate()}
        disabled={busy}
      >
        {busy ? "Validando…" : "Validar configuración"}
      </button>

      {error && <p className="alert alert-error">{error}</p>}

      {result && (
        <dl className="receipt">
          <div>
            <dt>Precio base</dt>
            <dd>${result.base_price}</dd>
          </div>
          <div>
            <dt>Modificadores</dt>
            <dd>${result.modifiers_total}</dd>
          </div>
          <div>
            <dt>Impuesto</dt>
            <dd>${result.tax}</dd>
          </div>
          <div className="receipt-total">
            <dt>Total unitario</dt>
            <dd>${result.unit_price}</dd>
          </div>
        </dl>
      )}
    </section>
  );
}

interface ProductDialogProps {
  categories: Category[];
  onClose: () => void;
  onCreated: (name: string) => Promise<void> | void;
  onError: (message: string) => void;
}

function ProductCreateDialog({ categories, onClose, onCreated, onError }: ProductDialogProps) {
  const [sku, setSku] = useState("");
  const [name, setName] = useState("");
  const [price, setPrice] = useState("");
  const [tax, setTax] = useState("16");
  const [categoryId, setCategoryId] = useState("");
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const parsed = productDraftSchema.safeParse({
      sku,
      name,
      price,
      tax_rate_bp: Math.round(Number(tax) * 100),
      category_id: categoryId || undefined,
    });
    if (!parsed.success) {
      setLocalError(parsed.error.issues[0]?.message ?? "Revisa los datos.");
      return;
    }
    setBusy(true);
    try {
      await api.createProduct(parsed.data);
      await onCreated(parsed.data.sku.toUpperCase());
    } catch (cause) {
      onError(messageOf(cause, "No fue posible crear el producto."));
      onClose();
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      title="Nuevo producto"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="button" onClick={onClose} disabled={busy}>
            Cancelar
          </button>
          <button
            type="submit"
            form="product-form"
            className="button button-primary"
            disabled={busy}
          >
            Agregar producto
          </button>
        </>
      }
    >
      <form id="product-form" className="form-grid" onSubmit={submit} noValidate>
        <label className="field">
          <span>SKU</span>
          <input
            value={sku}
            onChange={(event) => setSku(event.target.value)}
            maxLength={32}
            spellCheck={false}
            required
          />
        </label>
        <label className="field">
          <span>Nombre</span>
          <input value={name} onChange={(event) => setName(event.target.value)} required />
        </label>
        <div className="field-row">
          <label className="field field-short">
            <span>Precio (MXN)</span>
            <input
              inputMode="decimal"
              value={price}
              onChange={(event) => setPrice(event.target.value)}
              placeholder="0.00"
              required
            />
          </label>
          <label className="field field-short">
            <span>IVA (%)</span>
            <input
              inputMode="decimal"
              value={tax}
              onChange={(event) => setTax(event.target.value)}
              required
            />
          </label>
          <label className="field">
            <span>Categoría (opcional)</span>
            <select value={categoryId} onChange={(event) => setCategoryId(event.target.value)}>
              <option value="">Sin categoría</option>
              {categories.map((category) => (
                <option key={category.id} value={category.id}>
                  {category.name}
                </option>
              ))}
            </select>
          </label>
        </div>
        {localError && (
          <p className="alert alert-error" role="alert">
            {localError}
          </p>
        )}
      </form>
    </Modal>
  );
}

interface CategoryDialogProps {
  nextOrder: number;
  onClose: () => void;
  onCreated: (name: string) => Promise<void> | void;
  onError: (message: string) => void;
}

function CategoryCreateDialog({ nextOrder, onClose, onCreated, onError }: CategoryDialogProps) {
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);

  return (
    <Modal
      title="Nueva categoría"
      onClose={onClose}
      footer={
        <>
          <button type="button" className="button" onClick={onClose} disabled={busy}>
            Cancelar
          </button>
          <button
            type="submit"
            form="category-form"
            className="button button-primary"
            disabled={busy || !name.trim()}
          >
            Crear categoría
          </button>
        </>
      }
    >
      <form
        id="category-form"
        className="form-grid"
        onSubmit={async (event) => {
          event.preventDefault();
          if (!name.trim()) return;
          setBusy(true);
          try {
            await api.createCategory({ name: name.trim(), sort_order: nextOrder });
            await onCreated(name.trim());
          } catch (cause) {
            onError(messageOf(cause, "No fue posible crear la categoría."));
            onClose();
          } finally {
            setBusy(false);
          }
        }}
      >
        <label className="field">
          <span>Nombre</span>
          <input
            value={name}
            onChange={(event) => setName(event.target.value)}
            required
            autoFocus
          />
        </label>
      </form>
    </Modal>
  );
}
