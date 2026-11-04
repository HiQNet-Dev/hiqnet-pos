import { useState } from "react";
import { api, IpcError } from "../api";
import type { ModifierGroup, ModifierOption, Product, ProductVariant } from "../contracts";
import { lineFromConfiguration, type CartLine } from "../lib/cart";
import { Modal } from "../components/Modal";

export interface ConfigTarget {
  product: Product;
  variants: ProductVariant[];
  groups: ModifierGroup[];
  optionsByGroup: Record<string, ModifierOption[]>;
}

interface Props {
  target: ConfigTarget;
  onClose: () => void;
  onAdded: (line: CartLine) => void;
}

/**
 * Configuración de una partida: variante, opciones y cantidad. El precio de la
 * línea lo devuelve Rust; aquí solo se muestra el desglose.
 */
export function ProductConfigDialog({ target, onClose, onAdded }: Props) {
  const { product, variants, groups, optionsByGroup } = target;
  const [variantId, setVariantId] = useState(variants[0]?.id ?? "");
  const [selected, setSelected] = useState<string[]>([]);
  const [quantity, setQuantity] = useState(1);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function selectedInGroup(groupId: string): number {
    const ids = optionsByGroup[groupId] ?? [];
    return selected.filter((id) => ids.some((option) => option.id === id)).length;
  }

  function toggle(group: ModifierGroup, option: ModifierOption) {
    setError(null);
    const ids = optionsByGroup[group.id] ?? [];
    setSelected((current) => {
      const inGroup = current.filter((id) => ids.some((item) => item.id === id));
      const withoutGroup = current.filter((id) => !ids.some((item) => item.id === id));
      if (current.includes(option.id)) {
        return current.filter((id) => id !== option.id);
      }
      if (group.max === 1) return [...withoutGroup, option.id];
      if (inGroup.length >= group.max) return current;
      return [...current, option.id];
    });
  }

  async function handleAdd() {
    setBusy(true);
    setError(null);
    try {
      const configuration = await api.configureProduct({
        product_id: product.id,
        variant_id: variantId || null,
        selected_option_ids: selected,
      });
      onAdded(lineFromConfiguration(configuration, quantity));
      onClose();
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible configurar el producto.",
      );
    } finally {
      setBusy(false);
    }
  }

  const canAdd = groups.every((group) => selectedInGroup(group.id) >= group.min);

  return (
    <Modal
      title={product.name}
      subtitle={`Precio de lista $${product.unit_price} · impuesto calculado por el sistema`}
      onClose={onClose}
      footer={
        <>
          <button type="button" className="button" onClick={onClose} disabled={busy}>
            Cancelar
          </button>
          <button
            type="button"
            className="button button-primary"
            onClick={() => void handleAdd()}
            disabled={busy || !canAdd}
          >
            {busy ? "Agregando…" : "Agregar al carrito"}
          </button>
        </>
      }
    >
      {variants.length > 0 && (
        <fieldset className="config-group">
          <legend>Presentación</legend>
          <div className="option-grid">
            {variants.map((variant) => (
              <label
                key={variant.id}
                className={
                  variantId === variant.id ? "option option-selected" : "option"
                }
              >
                <input
                  type="radio"
                  name="variant"
                  value={variant.id}
                  checked={variantId === variant.id}
                  onChange={() => setVariantId(variant.id)}
                />
                <span className="option-name">{variant.name}</span>
                <span className="option-price">${variant.unit_price}</span>
              </label>
            ))}
          </div>
        </fieldset>
      )}

      {groups.map((group) => {
        const options = optionsByGroup[group.id] ?? [];
        const count = selectedInGroup(group.id);
        const missing = count < group.min;
        return (
          <fieldset className="config-group" key={group.id}>
            <legend>
              {group.name}
              <span className={missing ? "config-rule config-rule-missing" : "config-rule"}>
                {group.min > 0
                  ? `${count}/${group.max} · obligatorio`
                  : `hasta ${group.max}`}
              </span>
            </legend>
            <div className="option-grid">
              {options.map((option) => {
                const active = selected.includes(option.id);
                return (
                  <label
                    key={option.id}
                    className={active ? "option option-selected" : "option"}
                  >
                    <input
                      type={group.max === 1 ? "radio" : "checkbox"}
                      name={`group-${group.id}`}
                      checked={active}
                      disabled={!option.active}
                      onChange={() => toggle(group, option)}
                    />
                    <span className="option-name">{option.name}</span>
                    {option.kind === "extra" && option.price_delta !== "0.00" ? (
                      <span className="option-price">+${option.price_delta}</span>
                    ) : (
                      <span className="option-price option-price-muted">
                        {option.active ? "sin costo" : "no disponible"}
                      </span>
                    )}
                  </label>
                );
              })}
            </div>
          </fieldset>
        );
      })}

      {error && (
        <p className="alert alert-error" role="alert">
          {error}
        </p>
      )}

      <div className="config-quantity">
        <span className="field-label">Cantidad</span>
        <div className="stepper">
          <button
            type="button"
            className="stepper-button"
            onClick={() => setQuantity((value) => Math.max(1, value - 1))}
            aria-label="Disminuir cantidad"
            disabled={quantity <= 1}
          >
            −
          </button>
          <span className="stepper-value">{quantity}</span>
          <button
            type="button"
            className="stepper-button"
            onClick={() => setQuantity((value) => Math.min(99, value + 1))}
            aria-label="Aumentar cantidad"
          >
            +
          </button>
        </div>
      </div>
    </Modal>
  );
}
