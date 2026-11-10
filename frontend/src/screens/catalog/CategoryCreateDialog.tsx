import { useState } from "react";
import { api } from "../../api";
import { Modal } from "../../components/Modal";
import { messageOf } from "./messages";

interface CategoryDialogProps {
  nextOrder: number;
  onClose: () => void;
  onCreated: (name: string) => Promise<void> | void;
  onError: (message: string) => void;
}

export function CategoryCreateDialog({ nextOrder, onClose, onCreated, onError }: CategoryDialogProps) {
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
