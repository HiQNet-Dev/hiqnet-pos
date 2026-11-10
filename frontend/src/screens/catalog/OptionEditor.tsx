import { useState } from "react";
import { modifierOptionDraftSchema } from "../../contracts";
import { PlusIcon } from "../../shell/icons";
import type { OptionDraft } from "./types";

interface OptionEditorProps {
  disabled: boolean;
  onCreate: (draft: OptionDraft) => Promise<boolean>;
}

export function OptionEditor({ disabled, onCreate }: OptionEditorProps) {
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

