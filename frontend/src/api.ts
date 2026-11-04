import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import {
  actorSchema,
  appStatusSchema,
  auditEventSchema,
  bootstrapResultSchema,
  commercialStateSchema,
  categorySchema,
  ipcErrorSchema,
  inventoryMovementSchema,
  productSchema,
  modifierConfigurationSchema,
  modifierGroupSchema,
  modifierOptionSchema,
  productVariantSchema,
  type Actor,
  type AppStatus,
  type AuditEvent,
  type BootstrapRequest,
  type BootstrapResult,
  type CommercialState,
  type LoginRequest,
  type Product,
  type ProductDraft,
  type InventoryMovement,
  type InventoryMovementDraft,
  type Category,
  type CategoryDraft,
  type ModifierConfiguration,
  type ModifierConfigurationDraft,
  type ModifierGroup,
  type ModifierGroupDraft,
  type ModifierOption,
  type ModifierOptionDraft,
  type ProductVariant,
  type ProductVariantDraft,
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

  assignProductCategory: (productId: string, categoryId: string | null) =>
    call<Product>(
      "assign_product_category",
      { product_id: productId, category_id: categoryId },
      productSchema,
    ),

  createCategory: (draft: CategoryDraft) =>
    call<Category>("create_category", { draft }, categorySchema),

  listCategories: () =>
    call<Category[]>("list_categories", undefined, categorySchema.array()),

  createProductVariant: (draft: ProductVariantDraft) =>
    call<ProductVariant>("create_product_variant", { draft }, productVariantSchema),

  listProductVariants: (productId: string) =>
    call<ProductVariant[]>(
      "list_product_variants",
      { product_id: productId },
      productVariantSchema.array(),
    ),

  createModifierGroup: (draft: ModifierGroupDraft) =>
    call<ModifierGroup>("create_modifier_group", { draft }, modifierGroupSchema),

  listModifierGroups: (productId: string) =>
    call<ModifierGroup[]>(
      "list_modifier_groups",
      { product_id: productId },
      modifierGroupSchema.array(),
    ),

  createModifierOption: (draft: ModifierOptionDraft) =>
    call<ModifierOption>("create_modifier_option", { draft }, modifierOptionSchema),

  listModifierOptions: (groupId: string) =>
    call<ModifierOption[]>(
      "list_modifier_options",
      { group_id: groupId },
      modifierOptionSchema.array(),
    ),

  configureProduct: (draft: ModifierConfigurationDraft) =>
    call<ModifierConfiguration>("configure_product", { draft }, modifierConfigurationSchema),

  currentStock: (productId: string, branchId?: string) =>
    call<string>("current_stock", { product_id: productId, branch_id: branchId }, z.string()),

  recordInventoryMovement: (draft: InventoryMovementDraft) =>
    call<InventoryMovement>("record_inventory_movement", { draft }, inventoryMovementSchema),

  listInventoryMovements: (productId?: string, limit = 50) =>
    call<InventoryMovement[]>(
      "list_inventory_movements",
      { product_id: productId, limit },
      inventoryMovementSchema.array(),
    ),

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
