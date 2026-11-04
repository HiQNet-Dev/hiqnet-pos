import { useState } from "react";
import { api, IpcError } from "../api";
import { loginRequestSchema, type Actor } from "../contracts";

type Props = {
  onAuthenticated: (actor: Actor) => void;
};

export function LoginScreen({ onAuthenticated }: Props) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);

    const parsed = loginRequestSchema.safeParse({ username, password });
    if (!parsed.success) {
      setError(parsed.error.issues[0]?.message ?? "Revisa tus datos.");
      return;
    }

    setBusy(true);
    try {
      const actor = await api.login(parsed.data);
      onAuthenticated(actor);
    } catch (cause) {
      setError(
        cause instanceof IpcError
          ? cause.message
          : "No fue posible iniciar sesión.",
      );
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="panel panel-narrow" aria-labelledby="login-title">
      <h1 id="login-title">Entrar a la caja</h1>
      <p className="lede">Usa tu usuario local de HiQNet POS.</p>

      <form className="form-grid" onSubmit={handleSubmit} noValidate>
        <label className="field">
          <span>Usuario</span>
          <input
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            autoComplete="username"
            autoFocus
            spellCheck={false}
            required
          />
        </label>
        <label className="field">
          <span>Contraseña</span>
          <input
            type="password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            autoComplete="current-password"
            required
          />
        </label>

        {error && (
          <p className="alert alert-error" role="alert">
            {error}
          </p>
        )}

        <div className="form-actions">
          <button
            type="submit"
            className="button button-primary button-block"
            disabled={busy}
          >
            {busy ? "Verificando…" : "Entrar"}
          </button>
        </div>
      </form>
    </section>
  );
}
