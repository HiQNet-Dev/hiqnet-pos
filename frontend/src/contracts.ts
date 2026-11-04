import { z } from "zod";

/**
 * Contratos IPC. Se validan en la frontera para detectar desajustes entre Rust
 * y React sin duplicar reglas de negocio: el backend sigue siendo la autoridad.
 */

export const commercialStateSchema = z.object({
  plan: z.enum(["trial", "local", "pro", "enterprise"]),
  status: z.enum(["trial", "active", "grace", "read_only", "expired"]),
  valid_until: z.string(),
  features: z.array(z.string()),
  read_only: z.boolean(),
});

export const appStatusSchema = z.object({
  initialized: z.boolean(),
  product_version: z.string(),
  commercial: commercialStateSchema.nullable(),
  user_count: z.number().int().nonnegative(),
});

export const actorSchema = z.object({
  user_id: z.string(),
  organization_id: z.string(),
  branch_id: z.string().nullable(),
  display_name: z.string(),
  role: z.enum(["owner", "cashier"]),
  permissions: z.array(z.string()),
});

export const bootstrapResultSchema = z.object({
  organization_id: z.string(),
  branch_id: z.string(),
  user_id: z.string(),
  trial_ends_at: z.string(),
});

export const productSchema = z.object({
  id: z.string(),
  sku: z.string(),
  name: z.string(),
  unit_price: z.string(),
  tax_rate_bp: z.number().int().min(0).max(10_000),
  active: z.boolean(),
  category_id: z.string().nullable().optional(),
});

export const categorySchema = z.object({
  id: z.string(),
  name: z.string(),
  sort_order: z.number().int(),
  active: z.boolean(),
});

export const categoryDraftSchema = z.object({
  name: z.string().min(1, "El nombre de la categoría es obligatorio"),
  sort_order: z.number().int(),
});

export const productVariantSchema = z.object({
  id: z.string(),
  product_id: z.string(),
  name: z.string(),
  sku: z.string().nullable(),
  unit_price: z.string(),
  active: z.boolean(),
  sort_order: z.number().int(),
});

export const productVariantDraftSchema = z.object({
  product_id: z.string().min(1, "Selecciona un producto"),
  name: z.string().min(1, "El nombre de la variante es obligatorio"),
  sku: z.string().max(32).optional(),
  price: z
    .string()
    .regex(/^\d+(\.\d{1,2})?$/, "Usa un precio con hasta dos decimales"),
  sort_order: z.number().int(),
});

export const auditEventSchema = z.object({
  id: z.string(),
  action: z.string(),
  entity_type: z.string(),
  entity_id: z.string(),
  actor_user_id: z.string().nullable(),
  occurred_at: z.string(),
});

export const ipcErrorSchema = z.object({
  code: z.string(),
  message: z.string(),
});

export type CommercialState = z.infer<typeof commercialStateSchema>;
export type AppStatus = z.infer<typeof appStatusSchema>;
export type Actor = z.infer<typeof actorSchema>;
export type BootstrapResult = z.infer<typeof bootstrapResultSchema>;
export type Product = z.infer<typeof productSchema>;
export type Category = z.infer<typeof categorySchema>;
export type CategoryDraft = z.infer<typeof categoryDraftSchema>;

export const cashRegisterSchema = z.object({
  id: z.string(),
  name: z.string(),
  code: z.string(),
  active: z.boolean(),
});

export const cashRegisterDraftSchema = z.object({
  name: z.string().min(1, "Escribe el nombre de la caja"),
  code: z.string().min(2).max(16),
});

export const cashShiftSchema = z.object({
  id: z.string(),
  register_id: z.string(),
  status: z.string(),
  opened_by: z.string(),
  opened_at: z.string(),
  closed_by: z.string().nullable(),
  closed_at: z.string().nullable(),
  opening_amount: z.string(),
  expected_amount: z.string(),
  entry_total: z.string(),
  exit_total: z.string(),
  counted_amount: z.string().nullable(),
  difference_minor: z.number().int().nullable(),
  movements: z.array(
    z.object({
      id: z.string(),
      kind: z.string(),
      amount: z.string(),
      reason: z.string(),
      occurred_at: z.string(),
    }),
  ),
});

export const cashMovementDraftSchema = z.object({
  shift_id: z.string().min(1),
  kind: z.enum(["in", "out"]),
  amount: z.string(),
  reason: z.string().min(1),
});

export const cashOpenShiftDraftSchema = z.object({
  register_id: z.string().min(1),
  opening_amount: z.string(),
});

export const cashCloseShiftDraftSchema = z.object({
  shift_id: z.string().min(1),
  counted_amount: z.string(),
});

export type CashRegister = z.infer<typeof cashRegisterSchema>;
export type CashRegisterDraft = z.infer<typeof cashRegisterDraftSchema>;
export type CashShift = z.infer<typeof cashShiftSchema>;
export type CashMovementDraft = z.infer<typeof cashMovementDraftSchema>;
export type CashOpenShiftDraft = z.infer<typeof cashOpenShiftDraftSchema>;
export type CashCloseShiftDraft = z.infer<typeof cashCloseShiftDraftSchema>;

export const modifierGroupSchema = z.object({
  id: z.string(),
  name: z.string(),
  min: z.number().int().nonnegative(),
  max: z.number().int().nonnegative(),
  sort_order: z.number().int(),
  active: z.boolean(),
});

export const modifierGroupDraftSchema = z.object({
  product_id: z.string().min(1),
  name: z.string().min(1),
  min: z.number().int().nonnegative(),
  max: z.number().int().nonnegative(),
  sort_order: z.number().int(),
});

export const modifierOptionSchema = z.object({
  id: z.string(),
  group_id: z.string(),
  name: z.string(),
  kind: z.enum(["choice", "extra", "omission"]),
  price_delta: z.string(),
  sort_order: z.number().int(),
  active: z.boolean(),
});

export const modifierOptionDraftSchema = z.object({
  group_id: z.string().min(1),
  name: z.string().min(1),
  kind: z.enum(["choice", "extra", "omission"]),
  price_delta: z.string(),
  sort_order: z.number().int(),
});

export const modifierSelectionSchema = z.object({
  selected_option_id: z.string(),
  name: z.string(),
  kind: z.string(),
  price_delta: z.string(),
});

export const modifierConfigurationSchema = z.object({
  product_id: z.string(),
  product_name: z.string(),
  variant_id: z.string().nullable(),
  variant_name: z.string().nullable(),
  base_price: z.string(),
  modifiers_total: z.string(),
  tax: z.string(),
  unit_price: z.string(),
  tax_rate_bp: z.number().int(),
  selected_options: z.array(modifierSelectionSchema),
});

export const modifierConfigurationDraftSchema = z.object({
  product_id: z.string().min(1),
  variant_id: z.string().nullable().optional(),
  selected_option_ids: z.array(z.string()),
});

export type ModifierGroup = z.infer<typeof modifierGroupSchema>;
export type ModifierGroupDraft = z.infer<typeof modifierGroupDraftSchema>;
export type ModifierOption = z.infer<typeof modifierOptionSchema>;
export type ModifierOptionDraft = z.infer<typeof modifierOptionDraftSchema>;
export type ModifierSelection = z.infer<typeof modifierSelectionSchema>;
export type ModifierConfiguration = z.infer<typeof modifierConfigurationSchema>;
export type ModifierConfigurationDraft = z.infer<
  typeof modifierConfigurationDraftSchema
>;
export type ProductVariant = z.infer<typeof productVariantSchema>;
export type ProductVariantDraft = z.infer<typeof productVariantDraftSchema>;
export type AuditEvent = z.infer<typeof auditEventSchema>;

export const bootstrapRequestSchema = z.object({
  organization_name: z.string().min(2, "Escribe el nombre del negocio"),
  branch_name: z.string().min(2, "Escribe el nombre de la sucursal"),
  branch_code: z
    .string()
    .min(2, "El código debe tener al menos 2 caracteres")
    .max(16, "El código admite hasta 16 caracteres"),
  owner_username: z
    .string()
    .min(3, "El usuario debe tener al menos 3 caracteres")
    .max(40, "El usuario admite hasta 40 caracteres"),
  owner_display_name: z.string().min(2, "Escribe el nombre de la persona"),
  owner_password: z
    .string()
    .min(10, "La contraseña debe tener al menos 10 caracteres"),
});

export const loginRequestSchema = z.object({
  username: z.string().min(1, "Escribe tu usuario"),
  password: z.string().min(1, "Escribe tu contraseña"),
});

export const productDraftSchema = z.object({
  sku: z
    .string()
    .min(1, "El SKU es obligatorio")
    .max(32, "El SKU admite hasta 32 caracteres"),
  name: z.string().min(1, "El nombre es obligatorio"),
  price: z
    .string()
    .regex(
      /^\d+(\.\d{1,2})?$/,
      "Usa un precio con hasta dos decimales, por ejemplo 12.50",
    ),
  tax_rate_bp: z
    .number()
    .int()
    .min(0, "El impuesto no puede ser negativo")
    .max(10_000, "El impuesto máximo es 100%"),
  category_id: z.string().optional(),
});

export type BootstrapRequest = z.infer<typeof bootstrapRequestSchema>;
export type LoginRequest = z.infer<typeof loginRequestSchema>;
export type ProductDraft = z.infer<typeof productDraftSchema>;

export const inventoryMovementSchema = z.object({
  id: z.string(),
  product_id: z.string(),
  kind: z.enum(["inbound", "outbound", "adjustment_in", "adjustment_out"]),
  quantity: z.string(),
  reason: z.string().nullable(),
  occurred_at: z.string(),
});

export const inventoryMovementDraftSchema = z.object({
  product_id: z.string().min(1, "Selecciona un producto"),
  kind: z.enum(["inbound", "outbound", "adjustment_in", "adjustment_out"]),
  quantity: z
    .string()
    .regex(/^\d+(\.\d{1,3})?$/, "Usa una cantidad con hasta 3 decimales"),
  reason: z.string().optional(),
});

export type InventoryMovement = z.infer<typeof inventoryMovementSchema>;
export type InventoryMovementDraft = z.infer<typeof inventoryMovementDraftSchema>;
