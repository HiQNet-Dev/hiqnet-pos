import { useCallback, useEffect, useState } from "react";
import { api, IpcError } from "./api";
import type { Actor, AppStatus } from "./contracts";
import { LoginScreen } from "./screens/LoginScreen";
import { OnboardingScreen } from "./screens/OnboardingScreen";
import { AppShell } from "./shell/AppShell";

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

  if (loading) {
    return (
      <div className="auth-layout">
        <p className="screen-status">Iniciando HiQNet POS…</p>
      </div>
    );
  }

  if (fatal) {
    return (
      <div className="auth-layout">
        <section className="auth-card panel-error" role="alert">
          <h1>No se pudo iniciar la aplicación</h1>
          <p>{fatal}</p>
          <button type="button" className="button" onClick={() => void refreshStatus()}>
            Reintentar
          </button>
        </section>
      </div>
    );
  }

  if (status && !status.initialized) {
    return (
      <div className="auth-layout">
        <div className="auth-brand">
          <span className="brand-mark" aria-hidden="true">
            HQ
          </span>
          <div>
            <p className="brand-name">HiQNet POS</p>
            <p className="brand-branch">Configuración inicial</p>
          </div>
        </div>
        <OnboardingScreen
          onDone={async (credentials) => {
            const session = await api.login(credentials);
            await refreshStatus();
            setActor(session);
          }}
        />
      </div>
    );
  }

  if (status?.initialized && !actor) {
    return (
      <div className="auth-layout">
        <div className="auth-brand">
          <span className="brand-mark" aria-hidden="true">
            HQ
          </span>
          <div>
            <p className="brand-name">HiQNet POS</p>
            <p className="brand-branch">Punto de venta local-first</p>
          </div>
        </div>
        <LoginScreen onAuthenticated={setActor} />
      </div>
    );
  }

  if (actor && status) {
    return (
      <AppShell
        actor={actor}
        commercial={status.commercial}
        productVersion={status.product_version}
        onLogout={() => void handleLogout()}
      />
    );
  }

  return null;
}
