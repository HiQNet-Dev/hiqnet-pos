import { useCallback, useEffect, useState } from "react";
import { api, IpcError } from "../api";
import {
  productDraftSchema,
  type Actor,
  type AuditEvent,
  type Category,
  type CommercialState,
  type Product,
} from "../contracts";
import { InventoryPanel } from "./InventoryPanel";
import { ModifierConfigurator } from "./ModifierConfigurator";
import { CatalogPanel } from "./CatalogPanel";

type Props = {
  actor: Actor;
  commercial: CommercialState | null;
  productVersion: string;
};

const emptyDraft = {
  sku: "",
  name: "",
  price: "",
  taxPercent: "16",
  categoryId: "",
};

export function WorkspaceScreen({ actor, commercial, productVersion }: Props) {
  const [products, setProducts] = useState<Product[]>([]);
  const [audit, setAudit] = useState<AuditEvent[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [draft, setDraft] = useState(emptyDraft);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const canManageCatalog = actor.permissions.includes("catalog.manage");
  const canBackup = actor.permissions.includes("backups.manage");
  const readOnly = commercial?.read_only ?? false;

  const refresh = useCallback(async () => {
    try {
      const [list, events] = await Promise.all([
        api.listProducts(),
        canManageCatalog ? api.listAuditEvents(25) : Promise.resolve([]),
      ]);
      setProducts(list);
      setAudit(events);
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible cargar el catálogo.",
      );
    } finally {
      setLoading(false);
    }
  }, [canManageCatalog]);

  useEffect(() => {
    let cancelled = false;
    api
      .listCategories()
      .then((list) => {
        if (!cancelled) setCategories(list);
      })
      .catch(() => {
        // El catálogo sigue siendo usable sin categorías.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const [list, events] = await Promise.all([
          api.listProducts(),
          canManageCatalog ? api.listAuditEvents(25) : Promise.resolve([]),
        ]);
        if (cancelled) return;
        setProducts(list);
        setAudit(events);
      } catch (cause) {
        if (cancelled) return;
        setError(
          cause instanceof IpcError
            ? cause.message
            : "No fue posible cargar el catálogo.",
        );
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, [canManageCatalog]);

  async function handleCreateProduct(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setNotice(null);

    const taxRate = Number.parseFloat(draft.taxPercent);
    const parsed = productDraftSchema.safeParse({
      sku: draft.sku,
      name: draft.name,
      price: draft.price,
      tax_rate_bp: Number.isFinite(taxRate)
        ? Math.round(taxRate * 100)
        : Number.NaN,
      category_id: draft.categoryId || undefined,
    });
    if (!parsed.success) {
      setError(
        parsed.error.issues[0]?.message ?? "Revisa los datos del producto.",
      );
      return;
    }

    setBusy(true);
    try {
      await api.createProduct(parsed.data);
      setDraft(emptyDraft);
      setNotice(`Producto ${parsed.data.sku.toUpperCase()} creado.`);
      await refresh();
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible crear el producto.",
      );
    } finally {
      setBusy(false);
    }
  }

  async function handleBackup() {
    setError(null);
    setNotice(null);
    setBusy(true);
    try {
      const path = await api.backupDatabase();
      setNotice(`Respaldo creado en ${path}`);
      await refresh();
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible crear el respaldo.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="workspace">
      {readOnly && (
        <p className="alert alert-warning" role="status">
          La licencia está vencida. Puedes consultar, exportar y respaldar, pero
          no registrar nuevas operaciones hasta renovar.
        </p>
      )}

      {(error || notice) && (
        <div aria-live="polite">
          {error && (
            <p className="alert alert-error" role="alert">
              {error}
            </p>
          )}
          {notice && <p className="alert alert-success">{notice}</p>}
        </div>
      )}

      <div className="workspace-grid">
        <section className="panel" aria-labelledby="catalog-title">
          <header className="panel-header">
            <div>
              <p className="eyebrow">Catálogo</p>
              <h1 id="catalog-title">Productos</h1>
            </div>
            <span className="badge badge-neutral">
              {products.length} activos
            </span>
          </header>

          {canManageCatalog && (
            <form
              className="form-grid form-compact"
              onSubmit={handleCreateProduct}
              noValidate
            >
              <div className="field-row">
                <label className="field field-short">
                  <span>SKU</span>
                  <input
                    value={draft.sku}
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        sku: event.target.value,
                      }))
                    }
                    spellCheck={false}
                    maxLength={32}
                    required
                  />
                </label>
                <label className="field">
                  <span>Nombre</span>
                  <input
                    value={draft.name}
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        name: event.target.value,
                      }))
                    }
                    required
                  />
                </label>
              </div>
              <div className="field-row">
                <label className="field field-short">
                  <span>Precio (MXN)</span>
                  <input
                    inputMode="decimal"
                    value={draft.price}
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        price: event.target.value,
                      }))
                    }
                    placeholder="0.00"
                    required
                  />
                </label>
                <label className="field field-short">
                  <span>IVA (%)</span>
                  <input
                    inputMode="decimal"
                    value={draft.taxPercent}
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        taxPercent: event.target.value,
                      }))
                    }
                    required
                  />
                </label>
                <label className="field field-short">
                  <span>Categoría (opcional)</span>
                  <select
                    value={draft.categoryId}
                    onChange={(event) =>
                      setDraft((current) => ({
                        ...current,
                        categoryId: event.target.value,
                      }))
                    }
                  >
                    <option value="">Sin categoría</option>
                    {categories.map((category) => (
                      <option key={category.id} value={category.id}>
                        {category.name}
                      </option>
                    ))}
                  </select>
                </label>
                <div className="field field-action">
                  <button
                    type="submit"
                    className="button button-primary"
                    disabled={busy || readOnly}
                  >
                    Agregar producto
                  </button>
                </div>
              </div>
            </form>
          )}

          {loading ? (
            <p className="muted">Cargando catálogo…</p>
          ) : products.length === 0 ? (
            <div className="empty-state">
              <p>Todavía no hay productos.</p>
              <p className="muted">
                {canManageCatalog
                  ? "Registra el primer producto para comenzar a operar la caja."
                  : "Pide al propietario que registre productos."}
              </p>
            </div>
          ) : (
            <table className="data-table">
              <caption className="visually-hidden">
                Productos registrados
              </caption>
              <thead>
                <tr>
                  <th scope="col">SKU</th>
                  <th scope="col">Producto</th>
                  <th scope="col" className="align-end">
                    Precio
                  </th>
                  <th scope="col" className="align-end">
                    IVA
                  </th>
                </tr>
              </thead>
              <tbody>
                {products.map((product) => (
                  <tr key={product.id}>
                    <td className="mono">{product.sku}</td>
                    <td>{product.name}</td>
                    <td className="align-end mono">${product.unit_price}</td>
                    <td className="align-end mono">
                      {(product.tax_rate_bp / 100).toFixed(2)}%
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </section>

<CatalogPanel
          actor={actor}
          products={products}
          readOnly={readOnly}
          onProductsChanged={refresh}
        />
        {actor.permissions.includes("inventory.view") && (
          <>
                    <CatalogPanel actor={actor} products={products} readOnly={readOnly} />
                    <ModifierConfigurator actor={actor} readOnly={readOnly} />
                    {actor.permissions.includes("inventory.view") && (
                      <InventoryPanel actor={actor} products={products} readOnly={readOnly} />
                    )}
                  </>
        )}
        <aside className="workspace-side">
          <section className="panel" aria-labelledby="license-title">
            <header className="panel-header">
              <div>
                <p className="eyebrow">Licencia</p>
                <h2 id="license-title">Estado comercial</h2>
              </div>
            </header>
            {commercial ? (
              <dl className="definition-list">
                <div>
                  <dt>Plan</dt>
                  <dd className="capitalize">{commercial.plan}</dd>
                </div>
                <div>
                  <dt>Estado</dt>
                  <dd className="capitalize">
                    {commercial.status.replace("_", " ")}
                  </dd>
                </div>
                <div>
                  <dt>Vigente hasta</dt>
                  <dd>{formatDate(commercial.valid_until)}</dd>
                </div>
                <div>
                  <dt>Versión</dt>
                  <dd>{productVersion}</dd>
                </div>
              </dl>
            ) : (
              <p className="muted">Sin información comercial.</p>
            )}

            {canBackup && (
              <button
                type="button"
                className="button button-secondary"
                onClick={() => void handleBackup()}
                disabled={busy}
              >
                Crear respaldo
              </button>
            )}
          </section>

          {canManageCatalog && (
            <section className="panel" aria-labelledby="audit-title">
              <header className="panel-header">
                <div>
                  <p className="eyebrow">Auditoría</p>
                  <h2 id="audit-title">Últimos eventos</h2>
                </div>
              </header>
              {audit.length === 0 ? (
                <p className="muted">Sin eventos registrados.</p>
              ) : (
                <ol className="timeline">
                  {audit.map((event) => (
                    <li key={event.id}>
                      <span className="timeline-action">{event.action}</span>
                      <span className="timeline-meta">
                        {event.entity_type} · {formatDate(event.occurred_at)}
                      </span>
                    </li>
                  ))}
                </ol>
              )}
            </section>
          )}
        </aside>
      </div>
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
