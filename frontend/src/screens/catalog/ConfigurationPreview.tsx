import { useState } from "react";
import { api } from "../../api";
import {
  type ModifierConfiguration,
  type ModifierGroup,
  type ModifierOption,
  type ProductVariant,
} from "../../contracts";
import { messageOf } from "./messages";

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
export function ConfigurationPreview({ productId, variants, groups, optionsByGroup }: PreviewProps) {
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

