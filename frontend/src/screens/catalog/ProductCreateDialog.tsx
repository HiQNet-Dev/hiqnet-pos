import { useEffect, useState } from "react";
import { api } from "../../api";
import { productDraftSchema, type Category, type ProductPricePreview } from "../../contracts";
import { Modal } from "../../components/Modal";
import { messageOf } from "./messages";

interface ProductDialogProps {
  categories: Category[];
  onClose: () => void;
  onCreated: (name: string) => Promise<void> | void;
  onError: (message: string) => void;
}

export function ProductCreateDialog({ categories, onClose, onCreated, onError }: ProductDialogProps) {
  const [sku, setSku] = useState("");
  const [name, setName] = useState("");
  const [price, setPrice] = useState("");
  const [tax, setTax] = useState("16");
  const [categoryId, setCategoryId] = useState("");
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);
  const [preview, setPreview] = useState<{
    key: string;
    result: ProductPricePreview | null;
    error: string | null;
  } | null>(null);

  const taxRateBp = Math.round(Number(tax) * 100);
  const trimmedPrice = price.trim();
  const priceValid =
    trimmedPrice !== "" && Number.isFinite(taxRateBp) && taxRateBp >= 0 && taxRateBp <= 10_000;
  const previewKey = `${trimmedPrice}|${taxRateBp}`;

  // El precio final lo calcula Rust con la misma política que la venta; aquí
  // solo se muestra, con un retardo corto para no llamar en cada tecla.
  useEffect(() => {
    if (!priceValid) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      api
        .previewProductPrice(trimmedPrice, taxRateBp)
        .then((result) => {
          if (!cancelled) setPreview({ key: previewKey, result, error: null });
        })
        .catch(() => {
          if (!cancelled)
            setPreview({
              key: previewKey,
              result: null,
              error: "Escribe un precio válido para ver el precio final.",
            });
        });
    }, 250);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [previewKey, priceValid, trimmedPrice, taxRateBp]);

  const currentPreview = priceValid && preview?.key === previewKey ? preview : null;

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
          <label className="field field-price">
            <span>Precio base, sin IVA</span>
            <input
              inputMode="decimal"
              value={price}
              onChange={(event) => setPrice(event.target.value)}
              placeholder="0.00"
              aria-describedby="product-price-preview"
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
        </div>
        <div className="price-preview" id="product-price-preview" aria-live="polite">
          <span className="price-preview-label">Precio final al público</span>
          <strong className="price-preview-value mono">
            {currentPreview?.result ? `$${currentPreview.result.public_unit_total}` : "—"}
          </strong>
          <span className="price-preview-tax">
            {!priceValid
              ? "Se calcula con el precio base"
              : currentPreview?.error
                ? currentPreview.error
                : currentPreview?.result
                  ? `IVA ${currentPreview.result.tax_rate_bp / 100}%: $${currentPreview.result.tax}`
                  : "Calculando…"}
          </span>
        </div>
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
        {localError && (
          <p className="alert alert-error" role="alert">
            {localError}
          </p>
        )}
      </form>
    </Modal>
  );
}

