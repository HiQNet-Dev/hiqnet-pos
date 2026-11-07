import { onBackButtonPress } from "@tauri-apps/api/app";

/**
 * Botón y gesto Back de Android.
 *
 * Tauri registra su propio `OnBackPressedCallback` en `AppPlugin`: si no hay
 * ningún listener de `back-button`, cuando la WebView no puede retroceder llama
 * a `activity.onBackPressed()` y **termina la Activity**. Como la aplicación es
 * una SPA de una sola entrada, eso ocurría en cada pulsación.
 *
 * Registramos un único listener nativo para toda la aplicación y resolvemos el
 * gesto por capas, de la más superficial a la más profunda. Ninguna capa cierra
 * la Activity: la salida del sistema queda en manos del usuario (Recientes).
 */

type BackHandler = () => boolean;

const handlers: BackHandler[] = [];

/**
 * Registra una capa que puede consumir Back. Las capas se apilan: la última
 * registrada tiene prioridad. Devuelve una función de baja idempotente.
 */
export function registerBackHandler(handler: BackHandler): () => void {
  handlers.push(handler);
  let released = false;
  return () => {
    if (released) return;
    released = true;
    const index = handlers.lastIndexOf(handler);
    if (index >= 0) handlers.splice(index, 1);
  };
}

/**
 * Ejecuta la capa superior que consuma el gesto. Devuelve `true` si alguna lo
 * hizo; `false` significa que el gesto no tiene efecto (pantalla raíz).
 */
export function handleBackButton(): boolean {
  for (let index = handlers.length - 1; index >= 0; index -= 1) {
    const handler = handlers[index];
    if (handler?.()) return true;
  }
  return false;
}

/** Solo en Android dentro de Tauri; en escritorio y en el navegador no aplica. */
export function isAndroidTauri(): boolean {
  return (
    typeof window !== "undefined" &&
    "__TAURI_INTERNALS__" in window &&
    typeof navigator !== "undefined" &&
    /Android/i.test(navigator.userAgent)
  );
}

let references = 0;
let teardown: (() => void) | null = null;

/**
 * Instala el listener nativo una sola vez. Tolera el doble montaje de
 * StrictMode con un contador de referencias y desregistra si la promesa de
 * registro se resuelve después del desmontaje.
 */
export function installAndroidBackGuard(): () => void {
  references += 1;
  if (references === 1) teardown = startBackListener();

  let released = false;
  return () => {
    if (released) return;
    released = true;
    references -= 1;
    if (references === 0) {
      teardown?.();
      teardown = null;
    }
  };
}

function startBackListener(): () => void {
  if (!isAndroidTauri()) return () => {};

  let disposed = false;
  let unlisten: (() => void) | null = null;

  void onBackButtonPress(() => {
    if (!disposed) handleBackButton();
  })
    .then((listener) => {
      if (disposed) {
        void listener.unregister();
        return;
      }
      unlisten = () => void listener.unregister();
    })
    .catch((cause: unknown) => {
      // Sin el plugin `app` disponible se conserva el comportamiento nativo.
      // Se registra porque, si ocurre en Android, Back volvería a cerrar la
      // aplicación y hay que detectarlo.
      console.warn("HiQNet POS: no fue posible registrar el botón Back", cause);
    });

  return () => {
    disposed = true;
    unlisten?.();
    unlisten = null;
  };
}
