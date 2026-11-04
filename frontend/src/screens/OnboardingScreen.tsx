import { useState } from "react";
import { api, IpcError } from "../api";
import { bootstrapRequestSchema, type LoginRequest } from "../contracts";

type Props = {
  onDone: (credentials: LoginRequest) => Promise<void>;
};

const initialForm = {
  organizationName: "",
  branchName: "Matriz",
  branchCode: "MX-01",
  ownerDisplayName: "",
  ownerUsername: "",
  ownerPassword: "",
  ownerPasswordConfirm: "",
};

export function OnboardingScreen({ onDone }: Props) {
  const [form, setForm] = useState(initialForm);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  function update<K extends keyof typeof form>(key: K, value: string) {
    setForm((current) => ({ ...current, [key]: value }));
  }

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    if (form.ownerPassword !== form.ownerPasswordConfirm) {
      setError("Las contraseñas no coinciden.");
      return;
    }

    const parsed = bootstrapRequestSchema.safeParse({
      organization_name: form.organizationName,
      branch_name: form.branchName,
      branch_code: form.branchCode,
      owner_username: form.ownerUsername,
      owner_display_name: form.ownerDisplayName,
      owner_password: form.ownerPassword,
    });
    if (!parsed.success) {
      setError(
        parsed.error.issues[0]?.message ?? "Revisa los datos del formulario.",
      );
      return;
    }

    setBusy(true);
    try {
      await api.bootstrap(parsed.data);
      await onDone({
        username: form.ownerUsername,
        password: form.ownerPassword,
      });
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible inicializar la instalación.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <section
      className="panel panel-onboarding"
      aria-labelledby="onboarding-title"
    >
      <h1 id="onboarding-title">Configura tu negocio</h1>
      <p className="lede">
        HiQNet POS crea los datos en este equipo. Dispones de 30 días de prueba
        con las funciones de la edición Local.
      </p>

      <form className="form-grid" onSubmit={handleSubmit} noValidate>
        <label className="field">
          <span>Nombre del negocio</span>
          <input
            value={form.organizationName}
            onChange={(event) => update("organizationName", event.target.value)}
            autoComplete="organization"
            required
          />
        </label>

        <div className="field-row">
          <label className="field">
            <span>Nombre de la sucursal</span>
            <input
              value={form.branchName}
              onChange={(event) => update("branchName", event.target.value)}
              required
            />
          </label>
          <label className="field field-short">
            <span>Código</span>
            <input
              value={form.branchCode}
              onChange={(event) => update("branchCode", event.target.value)}
              maxLength={16}
              required
            />
          </label>
        </div>

        <label className="field">
          <span>Nombre del propietario</span>
          <input
            value={form.ownerDisplayName}
            onChange={(event) => update("ownerDisplayName", event.target.value)}
            autoComplete="name"
            required
          />
        </label>

        <div className="field-row">
          <label className="field">
            <span>Usuario</span>
            <input
              value={form.ownerUsername}
              onChange={(event) => update("ownerUsername", event.target.value)}
              autoComplete="username"
              spellCheck={false}
              required
            />
          </label>
          <label className="field">
            <span>Contraseña</span>
            <input
              type="password"
              value={form.ownerPassword}
              onChange={(event) => update("ownerPassword", event.target.value)}
              autoComplete="new-password"
              minLength={10}
              required
            />
          </label>
        </div>

        <label className="field">
          <span>Confirmar contraseña</span>
          <input
            type="password"
            value={form.ownerPasswordConfirm}
            onChange={(event) =>
              update("ownerPasswordConfirm", event.target.value)
            }
            autoComplete="new-password"
            minLength={10}
            required
          />
        </label>

        {error && (
          <p className="alert alert-error" role="alert">
            {error}
          </p>
        )}

        <div className="form-actions form-actions-stack">
          <button
            type="submit"
            className="button button-primary button-block"
            disabled={busy}
          >
            {busy ? "Creando instalación…" : "Crear instalación y entrar"}
          </button>
          <p className="hint">
            Mínimo 10 caracteres. La contraseña se guarda con Argon2id.
          </p>
        </div>
      </form>
    </section>
  );
}
