import { useState } from "react";
import { modifierGroupDraftSchema } from "../../contracts";
import { PlusIcon } from "../../shell/icons";
import type { GroupDraft } from "./types";

interface GroupEditorProps {
  disabled: boolean;
  onCreate: (draft: GroupDraft) => Promise<boolean>;
}

export function GroupEditor({ disabled, onCreate }: GroupEditorProps) {
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

