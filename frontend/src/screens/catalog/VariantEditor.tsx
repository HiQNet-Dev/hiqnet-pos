import { useState } from "react";

interface VariantEditorProps {
  disabled: boolean;
  onCreate: (name: string, price: string) => Promise<boolean>;
}

export function VariantEditor({ disabled, onCreate }: VariantEditorProps) {
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
        <span>Precio neto</span>
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

