import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import {
  actorSchema,
  appStatusSchema,
  auditEventSchema,
  bootstrapResultSchema,
  branchSummarySchema,
  commercialStateSchema,
  categorySchema,
  cashRegisterSchema,
  cashShiftSchema,
  ipcErrorSchema,
  inventoryMovementSchema,
  productSchema,
  modifierConfigurationSchema,
  modifierGroupSchema,
  modifierOptionSchema,
  productVariantSchema,
  saleViewSchema,
  stockLevelSchema,
  userSchema,
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
  type CashCloseShiftDraft,
  type CashMovementDraft,
  type CashOpenShiftDraft,
  type CashRegister,
  type CashRegisterDraft,
  type CashShift,
  type Category,
  type CategoryDraft,
  type BranchSummary,
  type ChangePasswordDraft,
  type StockLevel,
  type User,
  type UserDraft,
  type ModifierConfiguration,
  type ModifierConfigurationDraft,
  type ModifierGroup,
  type ModifierGroupDraft,
  type ModifierOption,
  type ModifierOptionDraft,
  type CheckoutDraft,
  type ProductVariant,
  type ProductVariantDraft,
  type SaleView,
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

  createCashRegister: (draft: CashRegisterDraft) =>
    call<CashRegister>("create_cash_register", { draft }, cashRegisterSchema),

  listCashRegisters: () =>
    call<CashRegister[]>("list_cash_registers", undefined, cashRegisterSchema.array()),

  branchSummary: () =>
    call<BranchSummary>("branch_summary", undefined, branchSummarySchema),

  listStockLevels: () =>
    call<StockLevel[]>("list_stock_levels", undefined, stockLevelSchema.array()),

  listUsers: () => call<User[]>("list_users", undefined, userSchema.array()),

  createUser: (draft: UserDraft) => call<User>("create_user", { draft }, userSchema),

  setUserActive: (userId: string, active: boolean) =>
    call<User>(
      "set_user_active",
      { draft: { user_id: userId, active } },
      userSchema,
    ),

  resetUserPassword: (userId: string, password: string) =>
    call<null>(
      "reset_user_password",
      { draft: { user_id: userId, password } },
      z.null(),
    ),

  changePassword: (draft: ChangePasswordDraft) =>
    call<null>("change_password", { draft }, z.null()),

  openCashShift: (draft: CashOpenShiftDraft) =>
    call<CashShift>("open_cash_shift", { draft }, cashShiftSchema),

  activeCashShift: (registerId: string) =>
    call<CashShift | null>(
      "active_cash_shift",
      { register_id: registerId },
      cashShiftSchema.nullable(),
    ),

  registerCashMovement: (draft: CashMovementDraft) =>
    call<unknown>("register_cash_movement", { draft }, z.unknown()),

  closeCashShift: (draft: CashCloseShiftDraft) =>
    call<CashShift>("close_cash_shift", { draft }, cashShiftSchema),

  checkoutSale: (draft: CheckoutDraft) =>
    call<SaleView>("checkout_sale", { draft }, saleViewSchema),

  getSale: (saleId: string) =>
    call<SaleView>("get_sale", { sale_id: saleId }, saleViewSchema),

  listCashShiftHistory: () =>
    call<CashShift[]>("list_cash_shift_history", undefined, cashShiftSchema.array()),

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
