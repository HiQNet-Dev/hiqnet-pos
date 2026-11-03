import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import {
  actorSchema,
  appStatusSchema,
  auditEventSchema,
  bootstrapResultSchema,
  commercialStateSchema,
  ipcErrorSchema,
  productSchema,
  type Actor,
  type AppStatus,
  type AuditEvent,
  type BootstrapRequest,
  type BootstrapResult,
  type CommercialState,
  type LoginRequest,
  type Product,
  type ProductDraft,
} from "./contracts";

/** Error estable proveniente de Rust (`code`) con mensaje en español. */
export class IpcError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "IpcError";
    this.code = code;
  }
}

function normalizeError(raw: unknown): IpcError {
  const parsed = ipcErrorSchema.safeParse(raw);
  if (parsed.success) {
    return new IpcError(parsed.data.code, parsed.data.message);
  }
  const message =
    raw instanceof Error
      ? raw.message
      : "No fue posible completar la operación. Intenta de nuevo.";
  return new IpcError("unknown_error", message);
}

async function call<T>(
  command: string,
  args: Record<string, unknown> | undefined,
  schema: z.ZodType<T>,
): Promise<T> {
  let raw: unknown;
  try {
    raw = await invoke(command, args);
  } catch (error) {
    throw normalizeError(error);
  }

  const parsed = schema.safeParse(raw);
  if (!parsed.success) {
    throw new IpcError(
      "contract_mismatch",
      "La respuesta del sistema no coincide con el contrato esperado.",
    );
  }
  return parsed.data;
}

export const api = {
  status: () => call<AppStatus>("app_status", undefined, appStatusSchema),

  bootstrap: (request: BootstrapRequest) =>
    call<BootstrapResult>("bootstrap", { request }, bootstrapResultSchema),

  login: (request: LoginRequest) =>
    call<Actor>("login", { request }, actorSchema),

  logout: () => call<null>("logout", undefined, z.null()),

  currentSession: () =>
    call<Actor | null>("current_session", undefined, actorSchema.nullable()),

  createProduct: (draft: ProductDraft) =>
    call<Product>("create_product", { draft }, productSchema),

  listProducts: () =>
    call<Product[]>("list_products", undefined, productSchema.array()),

  listAuditEvents: (limit = 50) =>
    call<AuditEvent[]>(
      "list_audit_events",
      { limit },
      auditEventSchema.array(),
    ),

  commercialState: () =>
    call<CommercialState>("commercial_state", undefined, commercialStateSchema),

  backupDatabase: () => call<string>("backup_database", undefined, z.string()),
};
