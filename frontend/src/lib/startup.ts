import { api, IpcError } from "../api";
import type { AppStatus } from "../contracts";

/**
 * En Android, el WebView puede ejecutar la primera llamada IPC antes de que el
 * puente nativo termine de enlazarse: la respuesta se pierde o el comando queda
 * pendiente. Se reintenta solo esta consulta inicial, con límite y espera
 * acotada. Un error determinista (SQLite, permisos, migraciones) se propaga de
 * inmediato y nunca se oculta.
 */
const MAX_ATTEMPTS = 4;
const ATTEMPT_TIMEOUT_MS = 2_500;
const BACKOFF_MS = 200;

function isTransient(error: unknown): boolean {
  if (error instanceof IpcError) {
    return error.code === "startup_timeout" || error.code === "unknown_error";
  }
  // Rechazo sin estructura: el puente IPC aún no está listo.
  return true;
}

function withTimeout(promise: Promise<AppStatus>): Promise<AppStatus> {
  return new Promise<AppStatus>((resolve, reject) => {
    const timer = setTimeout(
      () =>
        reject(
          new IpcError(
            "startup_timeout",
            "El sistema todavía está iniciando. Intenta de nuevo.",
          ),
        ),
      ATTEMPT_TIMEOUT_MS,
    );
    promise.then(
      (value) => {
        clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
}

export async function loadInitialStatus(): Promise<AppStatus> {
  let lastError: unknown;
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    try {
      return await withTimeout(api.status());
    } catch (error) {
      lastError = error;
      if (!isTransient(error) || attempt === MAX_ATTEMPTS) break;
      console.warn(`HiQNet POS: reintento de arranque ${attempt}/${MAX_ATTEMPTS}`);
      await new Promise((resolve) => setTimeout(resolve, BACKOFF_MS * attempt));
    }
  }
  throw lastError;
}
