import { IpcError } from "../../api";

/** Traduce un error de IPC al mensaje que ve el usuario. */
export function messageOf(cause: unknown, fallback: string): string {
  return cause instanceof IpcError ? cause.message : fallback;
}
