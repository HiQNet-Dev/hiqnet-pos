import { useEffect, useRef, useState } from "react";
import { api, IpcError } from "../api";
import type { Actor, ModifierConfiguration, ModifierGroup, ModifierOption, Product, ProductVariant } from "../contracts";

interface Props {
  actor: Actor;
  readOnly: boolean;
}

export function ModifierConfigurator({ actor, readOnly }: Props) {
  const [products, setProducts] = useState<Product[]>([]);
  const [productId, setProductId] = useState("");
  const [variants, setVariants] = useState<ProductVariant[]>([]);
  const [variantId, setVariantId] = useState<string>("");
  const [groups, setGroups] = useState<ModifierGroup[]>([]);
  const [optionsByGroup, setOptionsByGroup] = useState<Record<string, ModifierOption[]>>({});
  const [selectedOptionIds, setSelectedOptionIds] = useState<string[]>([]);
  const [configuration, setConfiguration] = useState<ModifierConfiguration | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestId = useRef(0);

  useEffect(() => {
    let cancelled = false;
    api
      .listProducts()
      .then((list) => {
        if (!cancelled) setProducts(list);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (!productId) return;
    const currentRequest = ++requestId.current;
    let cancelled = false;
    Promise.all([api.listProductVariants(productId), api.listModifierGroups(productId)])
      .then(async ([variantList, groupList]) => {
        if (cancelled || currentRequest !== requestId.current) return;
        setVariants(variantList);
        setVariantId(variantList[0]?.id ?? "");
        setGroups(groupList);
        const map: Record<string, ModifierOption[]> = {};
        for (const group of groupList) {
          map[group.id] = await api.listModifierOptions(group.id);
        }
        if (!cancelled && currentRequest === requestId.current) {
          setOptionsByGroup(map);
          setSelectedOptionIds([]);
          setConfiguration(null);
        }
      })
      .catch((cause: unknown) => {
        if (!cancelled) {
          setError(
            cause instanceof IpcError ? cause.message : "No fue posible cargar los modificadores.",
          );
        }
      });
    return () => {
      cancelled = true;
    };
  }, [productId]);

  function toggleOption(optionId: string) {
    setConfiguration(null);
    setSelectedOptionIds((current) =>
      current.includes(optionId)
        ? current.filter((id) => id !== optionId)
        : [...current, optionId],
    );
  }

  async function handleValidate() {
    if (!productId) return;
    setBusy(true);
    setError(null);
    try {
      const result = await api.configureProduct({
        product_id: productId,
        variant_id: variantId || null,
        selected_option_ids: selectedOptionIds,
      });
      setConfiguration(result);
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible validar la configuración.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel" aria-labelledby="modifier-config-title">
      <header className="panel-header">
        <div>
          <p className="eyebrow">Configurador</p>
          <h2 id="modifier-config-title">Productos con modificadores</h2>
        </div>
      </header>

      {error && <p className="alert alert-error">{error}</p>}

      <div className="field-row">
        <label className="field">
          <span>Producto</span>
          <select
            value={productId}
            onChange={(event) => {
              const value = event.target.value;
              requestId.current += 1;
              setProductId(value);
              setVariants([]);
              setVariantId("");
              setGroups([]);
              setOptionsByGroup({});
              setSelectedOptionIds([]);
              setConfiguration(null);
              setError(null);
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
      </div>

      {productId && variants.length > 0 && (
        <div className="field-row">
          <label className="field field-short">
            <span>Variante</span>
            <select
                          value={variantId}
                          onChange={(event) => {
                            setVariantId(event.target.value);
                            setConfiguration(null);
                          }}
                        >
              {variants.map((variant) => (
                <option key={variant.id} value={variant.id}>
                  {variant.name} · ${variant.unit_price}
                </option>
              ))}
            </select>
          </label>
        </div>
      )}

      {groups.length === 0 && productId && (
        <p className="muted">Este producto no tiene grupos de modificadores.</p>
      )}

      {groups.map((group) => (
        <fieldset key={group.id} className="field-row" style={{ border: 0, padding: 0 }}>
          <legend>
            {group.name} {group.min > 0 ? `(obligatorio: ${group.min} a ${group.max})` : `(hasta ${group.max})`}
          </legend>
          <div>
            {(optionsByGroup[group.id] ?? []).map((option) => (
              <label key={option.id} className="field field-short">
                <input
                  type="checkbox"
                  checked={selectedOptionIds.includes(option.id)}
                  disabled={!option.active || readOnly}
                  onChange={() => toggleOption(option.id)}
                />
                <span>
                  {option.name}{" "}
                  {option.kind === "extra" && `(+${option.price_delta})`}
                </span>
              </label>
            ))}
          </div>
        </fieldset>
      ))}

      {!readOnly && actor.permissions.includes("sales.sell") && (
        <div className="field-row">
          <button
            type="button"
            className="button button-primary"
            disabled={busy || !productId}
            onClick={() => void handleValidate()}
          >
            Validar configuración
          </button>
        </div>
      )}

      {configuration && (
        <div className="catalog-detail">
          <p>
            <strong>Desglose:</strong> base ${configuration.base_price}, modificadores $
            {configuration.modifiers_total}, impuesto ${configuration.tax}, total $
            {configuration.unit_price}
          </p>
          <ul className="timeline">
            {configuration.selected_options.map((option) => (
              <li key={option.selected_option_id}>
                <span className="timeline-action">{option.name}</span>
                <span className="timeline-meta">
                  {option.kind} · ${option.price_delta}
                </span>
              </li>
            ))}
          </ul>
          {configuration.selected_options.length === 0 && (
            <p className="muted">Sin modificadores seleccionados.</p>
          )}
        </div>
      )}
    </section>
  );
}