import { useCallback, useEffect, useState } from "react";
import { api, IpcError } from "./api";
import type { Actor, AppStatus } from "./contracts";
import { LoginScreen } from "./screens/LoginScreen";
import { OnboardingScreen } from "./screens/OnboardingScreen";
import { WorkspaceScreen } from "./screens/WorkspaceScreen";

export default function App() {
  const [status, setStatus] = useState<AppStatus | null>(null);
  const [actor, setActor] = useState<Actor | null>(null);
  const [loading, setLoading] = useState(true);
  const [fatal, setFatal] = useState<string | null>(null);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await api.status());
    } catch (error) {
      setFatal(
        error instanceof IpcError
          ? error.message
          : "No fue posible consultar el estado de la instalación.",
      );
    }
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const current = await api.status();
        if (cancelled) return;
        setStatus(current);
        if (current.initialized) {
          const session = await api.currentSession();
          if (!cancelled && session) setActor(session);
        }
      } catch (error) {
        if (!cancelled) {
          setFatal(
            error instanceof IpcError
              ? error.message
              : "No fue posible consultar el estado de la instalación.",
          );
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  async function handleLogout() {
    try {
      await api.logout();
    } finally {
      setActor(null);
    }
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            HQ
          </span>
          <div>
            <p className="brand-name">HiQNet POS</p>
            <p className="brand-subtitle">Punto de venta local-first</p>
          </div>
        </div>
        <div className="header-meta">
          {status?.commercial && (
            <span
              className={
                status.commercial.read_only
                  ? "badge badge-warning"
                  : "badge badge-neutral"
              }
            >
              {status.commercial.read_only
                ? "Solo lectura"
                : `Plan ${planLabel(status.commercial.plan)}`}
            </span>
          )}
          {actor && (
            <div className="session">
              <span className="session-name">{actor.display_name}</span>
              <button
                type="button"
                className="button button-quiet"
                onClick={handleLogout}
              >
                Salir
              </button>
            </div>
          )}
        </div>
      </header>

      <main className="app-main">
        {loading && <p className="screen-status">Iniciando HiQNet POS…</p>}

        {!loading && fatal && (
          <section className="panel panel-error" role="alert">
            <h1>No se pudo iniciar la aplicación</h1>
            <p>{fatal}</p>
            <button
              type="button"
              className="button"
              onClick={() => void refreshStatus()}
            >
              Reintentar
            </button>
          </section>
        )}

        {!loading && !fatal && status && !status.initialized && (
          <OnboardingScreen
            onDone={async (credentials) => {
              const session = await api.login(credentials);
              await refreshStatus();
              setActor(session);
            }}
          />
        )}

        {!loading && !fatal && status?.initialized && !actor && (
          <LoginScreen onAuthenticated={setActor} />
        )}

        {!loading && !fatal && status?.initialized && actor && (
          <WorkspaceScreen
            actor={actor}
            commercial={status.commercial}
            productVersion={status.product_version}
          />
        )}
      </main>

      <footer className="app-footer">
        <span>Versión {status?.product_version ?? "0.1.0"}</span>
        <span>Datos locales en este equipo</span>
      </footer>
    </div>
  );
}

function planLabel(plan: string): string {
  switch (plan) {
    case "trial":
      return "Trial";
    case "local":
      return "Local";
    case "pro":
      return "Pro";
    case "enterprise":
      return "Enterprise";
    default:
      return plan;
  }
}
