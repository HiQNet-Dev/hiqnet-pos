import type { Actor } from "../contracts";

/**
 * Intento de una mutación cuyo resultado aún no se confirmó. La clave
 * idempotente vive dentro de `payload`, junto al contenido exacto enviado a
 * Rust: reintentar nunca cambia ni la clave ni el contenido.
 */
export interface PendingAttempt<T> {
  fingerprint: string;
  payload: T;
}

export type PendingSaleAttempt = PendingAttempt<import("../contracts").CheckoutDraft>;
export type PendingExpenseAttempt = PendingAttempt<import("../contracts").ExpenseDraft>;
export type PendingWasteAttempt = PendingAttempt<import("../contracts").WasteDraft>;
export type PendingReturnAttempt = PendingAttempt<import("../contracts").SaleReturnDraft>;

/**
 * La identidad forma parte de la clave de almacenamiento: cambiar de usuario,
 * organización o sucursal no recupera intentos de otra identidad.
 */
export function pendingAttemptStorageKey(
  kind: "sale" | "expense" | "waste" | "return",
  actor: Actor,
): string {
  return `hiqnet.pending-${kind}.${actor.organization_id}.${actor.branch_id ?? "sin-sucursal"}.${actor.user_id}`;
}

/**
 * Dato local de recuperación únicamente: la autoridad del hecho confirmado es
 * SQLite, y el backend valida la clave y el contenido.
 */
export function loadPendingAttempt<T>(storageKey: string): PendingAttempt<T> | null {
  try {
    const value: unknown = JSON.parse(window.localStorage.getItem(storageKey) ?? "null");
    if (
      value &&
      typeof value === "object" &&
      typeof (value as PendingAttempt<T>).fingerprint === "string" &&
      (value as PendingAttempt<T>).payload &&
      typeof (value as PendingAttempt<T>).payload === "object"
    ) {
      return value as PendingAttempt<T>;
    }
  } catch {
    // Un dato local corrupto no debe impedir operar: se ignora.
  }
  return null;
}

export function savePendingAttempt<T>(
  storageKey: string,
  attempt: PendingAttempt<T> | null,
): void {
  try {
    if (attempt) window.localStorage.setItem(storageKey, JSON.stringify(attempt));
    else window.localStorage.removeItem(storageKey);
  } catch {
    // La recuperación en memoria sigue siendo segura si el almacenamiento falla.
  }
}
