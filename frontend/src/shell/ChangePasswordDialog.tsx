import { useState } from "react";
import { api, IpcError } from "../api";
import { Modal } from "../components/Modal";

interface Props {
  onClose: () => void;
  onChanged: () => void;
}

/** Cambio de contraseña de la cuenta autenticada. Requiere la contraseña actual. */
export function ChangePasswordDialog({ onClose, onChanged }: Props) {
  const [current, setCurrent] = useState("");
  const [next, setNext] = useState("");
  const [repeat, setRepeat] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const mismatch = repeat.length > 0 && next !== repeat;
  const canSubmit = current.length > 0 && next.length >= 10 && next === repeat;

  async function submit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!canSubmit) return;
    setBusy(true);
    setError(null);
    try {
      await api.changePassword({ current_password: current, new_password: next });
      onChanged();
      onClose();
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible cambiar la contraseña.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal
      title="Cambiar contraseña"
      subtitle="Se aplica sólo a tu cuenta."
      onClose={onClose}
      footer={
        <>
          <button type="button" className="button" onClick={onClose} disabled={busy}>
            Cancelar
          </button>
          <button
            type="submit"
            form="change-password-form"
            className="button button-primary"
            disabled={busy || !canSubmit}
          >
            {busy ? "Guardando…" : "Cambiar contraseña"}
          </button>
        </>
      }
    >
      <form
        id="change-password-form"
        className="form-grid"
        onSubmit={submit}
        noValidate
      >
        <label className="field">
          <span>Contraseña actual</span>
          <input
            type="password"
            value={current}
            onChange={(event) => setCurrent(event.target.value)}
            autoComplete="current-password"
            autoFocus
            required
          />
        </label>
        <label className="field">
          <span>Nueva contraseña</span>
          <input
            type="password"
            value={next}
            onChange={(event) => setNext(event.target.value)}
            autoComplete="new-password"
            required
          />
        </label>
        <label className="field">
          <span>Repetir contraseña</span>
          <input
            type="password"
            value={repeat}
            onChange={(event) => setRepeat(event.target.value)}
            autoComplete="new-password"
            required
          />
        </label>
        <p className="hint">
          Mínimo 10 caracteres. La contraseña actual deja de servir al guardar.
        </p>
        {mismatch && <p className="alert alert-error">Las contraseñas no coinciden.</p>}
        {error && (
          <p className="alert alert-error" role="alert">
            {error}
          </p>
        )}
      </form>
    </Modal>
  );
}
