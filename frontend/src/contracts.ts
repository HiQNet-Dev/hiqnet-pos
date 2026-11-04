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

export const branchSummarySchema = z.object({
  id: z.string(),
  name: z.string(),
  code: z.string(),
});

export const userSchema = z.object({
  id: z.string(),
  username: z.string(),
  display_name: z.string(),
  role: z.enum(["owner", "cashier"]),
  active: z.boolean(),
  created_at: z.string(),
});

export const userDraftSchema = z.object({
  username: z
    .string()
    .min(3, "El usuario debe tener al menos 3 caracteres")
    .max(40, "El usuario admite hasta 40 caracteres"),
  display_name: z.string().min(2, "Escribe el nombre del empleado"),
  role: z.enum(["owner", "cashier"]),
  password: z.string().min(10, "La contraseña debe tener al menos 10 caracteres"),
});

export const changePasswordDraftSchema = z.object({
  current_password: z.string().min(1, "Escribe tu contraseña actual"),
  new_password: z
    .string()
    .min(10, "La contraseña debe tener al menos 10 caracteres"),
});

export type User = z.infer<typeof userSchema>;
export type UserDraft = z.infer<typeof userDraftSchema>;
export type ChangePasswordDraft = z.infer<typeof changePasswordDraftSchema>;

export const stockLevelSchema = z.object({
  product_id: z.string(),
  quantity: z.string(),
});

export type BranchSummary = z.infer<typeof branchSummarySchema>;
export type StockLevel = z.infer<typeof stockLevelSchema>;

export const expenseCategorySchema = z.object({ id: z.string(), name: z.string(), active: z.boolean() });
export const expenseCategoryDraftSchema = z.object({ name: z.string().min(1, "Escribe el nombre de la categoría") });
export const expenseSchema = z.object({ id: z.string(), category_id: z.string(), amount: z.string(), concept: z.string(), paid_from_cash: z.boolean(), occurred_at: z.string() });
export const expenseDraftSchema = z.object({ category_id: z.string().min(1, "Selecciona una categoría"), amount: z.string().regex(/^\d+(\.\d{1,2})?$/, "Usa un importe válido"), concept: z.string().min(1, "Escribe el concepto"), paid_from_cash: z.boolean(), shift_id: z.string().nullable().optional(), idempotency_key: z.string().min(1) });
export const wasteSchema = z.object({ id: z.string(), product_id: z.string(), quantity: z.string(), reason: z.string(), occurred_at: z.string() });
export const wasteDraftSchema = z.object({ product_id: z.string().min(1, "Selecciona un producto"), quantity: z.string().regex(/^\d+(\.\d{1,3})?$/, "Usa una cantidad válida"), reason: z.string().min(1, "Indica el motivo"), idempotency_key: z.string().min(1) });
export type ExpenseCategory = z.infer<typeof expenseCategorySchema>;
export type ExpenseCategoryDraft = z.infer<typeof expenseCategoryDraftSchema>;
export type Expense = z.infer<typeof expenseSchema>;
export type ExpenseDraft = z.infer<typeof expenseDraftSchema>;
export type Waste = z.infer<typeof wasteSchema>;
export type WasteDraft = z.infer<typeof wasteDraftSchema>;

export const saleReturnDispositionSchema = z.enum(["restock", "discard"]);

export const saleReturnLineViewSchema = z.object({
  sale_item_id: z.string(),
  product_name: z.string(),
  quantity: z.string(),
  disposition: saleReturnDispositionSchema,
  refund_net: z.string(),
  refund_tax: z.string(),
  refund_total: z.string(),
});

export const saleReturnSchema = z.object({
  id: z.string(),
  sale_id: z.string(),
  sale_folio: z.string(),
  reason: z.string(),
  refund_total: z.string(),
  occurred_at: z.string(),
  lines: z.array(saleReturnLineViewSchema),
});

export const returnableSaleItemSchema = z.object({
  id: z.string(),
  product_name: z.string(),
  variant_name: z.string().nullable(),
  modifiers_summary: z.string(),
  quantity: z.string(),
  returned_quantity: z.string(),
  returnable_quantity: z.string(),
  unit_refund: z.string(),
});

export const returnableSaleSchema = z.object({
  id: z.string(),
  folio: z.string(),
  occurred_at: z.string(),
  total: z.string(),
  items: z.array(returnableSaleItemSchema),
});

export const saleReturnLineDraftSchema = z.object({
  sale_item_id: z.string().min(1, "Selecciona una partida"),
  quantity: z.string().regex(/^\d+(\.\d{1,3})?$/, "Usa una cantidad con hasta 3 decimales"),
  disposition: saleReturnDispositionSchema,
});

export const saleReturnDraftSchema = z.object({
  sale_id: z.string().min(1),
  shift_id: z.string().optional(),
  reason: z.string().min(1, "Indica el motivo de la devolución"),
  idempotency_key: z.string().min(1),
  lines: z.array(saleReturnLineDraftSchema).min(1, "Selecciona al menos una partida"),
});

export const saleReturnPreviewDraftSchema = z.object({
  sale_id: z.string().min(1),
  lines: z.array(saleReturnLineDraftSchema).min(1, "Selecciona al menos una partida"),
});

export const saleReturnPreviewSchema = z.object({
  refund_total: z.string(),
  lines: z.array(
    z.object({
      sale_item_id: z.string(),
      quantity: z.string(),
      refund_total: z.string(),
    }),
  ),
});

export type SaleReturnDisposition = z.infer<typeof saleReturnDispositionSchema>;
export type SaleReturnLineView = z.infer<typeof saleReturnLineViewSchema>;
export type SaleReturn = z.infer<typeof saleReturnSchema>;
export type ReturnableSaleItem = z.infer<typeof returnableSaleItemSchema>;
export type ReturnableSale = z.infer<typeof returnableSaleSchema>;
export type SaleReturnLineDraft = z.infer<typeof saleReturnLineDraftSchema>;
export type SaleReturnDraft = z.infer<typeof saleReturnDraftSchema>;
export type SaleReturnPreviewDraft = z.infer<typeof saleReturnPreviewDraftSchema>;
export type SaleReturnPreview = z.infer<typeof saleReturnPreviewSchema>;

export const reportsQuerySchema = z.object({
  from: z.string().min(1, "Indica la fecha inicial"),
  to: z.string().min(1, "Indica la fecha final"),
});

export const reportsViewSchema = z.object({
  from: z.string(),
  to: z.string(),
  sales: z.object({
    count: z.number().int(),
    subtotal: z.string(),
    tax: z.string(),
    total: z.string(),
    returned_count: z.number().int(),
    returned_total: z.string(),
    net_total: z.string(),
    products: z.array(
      z.object({
        product_id: z.string(),
        product_name: z.string(),
        quantity: z.string(),
        net: z.string(),
        tax: z.string(),
        total: z.string(),
      }),
    ),
  }),
  cash: z.object({
    shifts_opened: z.number().int(),
    shifts_closed: z.number().int(),
    entries: z.string(),
    exits: z.string(),
    expenses_from_cash: z.string(),
    refunds: z.string(),
    difference: z.string(),
  }),
  purchases: z.object({
    orders_created: z.number().int(),
    pending_orders: z.number().int(),
    received_quantity: z.string(),
    received_cost: z.string(),
  }),
  expenses: z.object({
    total: z.string(),
    from_cash: z.string(),
    external: z.string(),
    categories: z.array(z.object({ name: z.string(), total: z.string() })),
  }),
  inventory: z.object({
    stock_quantity: z.string(),
    inbound: z.string(),
    outbound: z.string(),
    waste: z.string(),
    adjustment_in: z.string(),
    adjustment_out: z.string(),
  }),
});

export type ReportsQuery = z.infer<typeof reportsQuerySchema>;
export type ReportsView = z.infer<typeof reportsViewSchema>;

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

export const saleItemDraftSchema = z.object({
  product_id: z.string().min(1),
  variant_id: z.string().nullable().optional(),
  quantity: z.string().regex(/^\d+(\.\d{1,3})?$/),
  selected_option_ids: z.array(z.string()),
});

export const checkoutDraftSchema = z.object({
  shift_id: z.string().min(1),
  idempotency_key: z.string().min(1),
  items: z.array(saleItemDraftSchema).min(1),
  cash_received: z.string(),
});

export const saleViewSchema = z.object({
  id: z.string(),
  folio: z.string(),
  status: z.string(),
  subtotal: z.string(),
  tax: z.string(),
  total: z.string(),
  items: z.array(
    z.object({
      id: z.string(),
      product_id: z.string(),
      product_name: z.string(),
      variant_name: z.string().nullable(),
      quantity: z.string(),
      unit_price: z.string(),
      unit_tax: z.string(),
      line_total: z.string(),
      modifiers_summary: z.string(),
    }),
  ),
  payment: z
    .object({ method: z.string(), amount: z.string(), change: z.string() })
    .nullable(),
});

export type SaleItemDraft = z.infer<typeof saleItemDraftSchema>;
export type CheckoutDraft = z.infer<typeof checkoutDraftSchema>;
export type SaleView = z.infer<typeof saleViewSchema>;

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

export const supplierSchema = z.object({
  id: z.string(),
  code: z.string().nullable(),
  name: z.string(),
  contact: z.string().nullable(),
  rfc: z.string().nullable(),
  active: z.boolean(),
});

export const supplierDraftSchema = z.object({
  code: z.string().max(32).optional(),
  name: z.string().min(1, "El nombre del proveedor es obligatorio"),
  contact: z.string().max(120).optional(),
  rfc: z.string().max(16).optional(),
});

export const purchaseOrderLineSchema = z.object({
  id: z.string(),
  product_id: z.string(),
  quantity: z.string(),
  received_quantity: z.string(),
  unit_cost: z.string(),
});

export const purchaseOrderSchema = z.object({
  id: z.string(),
  supplier_id: z.string(),
  folio: z.string(),
  status: z.enum(["draft", "ordered", "partially_received", "received"]),
  created_at: z.string(),
  lines: z.array(purchaseOrderLineSchema),
});

export const purchaseOrderLineDraftSchema = z.object({
  product_id: z.string().min(1, "Selecciona un producto"),
  quantity: z.string().regex(/^\d+(\.\d{1,3})?$/, "Usa una cantidad con hasta 3 decimales"),
  unit_cost: z.string().regex(/^\d+(\.\d{1,2})?$/, "Usa un costo con hasta dos decimales"),
});

export const purchaseOrderDraftSchema = z.object({
  supplier_id: z.string().min(1, "Selecciona un proveedor"),
  lines: z.array(purchaseOrderLineDraftSchema).min(1, "Agrega al menos un producto"),
});

export const purchaseReceiptLineDraftSchema = z.object({
  order_line_id: z.string().min(1),
  quantity: z.string().regex(/^\d+(\.\d{1,3})?$/, "Usa una cantidad con hasta 3 decimales"),
});

export const purchaseReceiptDraftSchema = z.object({
  order_id: z.string().min(1),
  idempotency_key: z.string().min(1),
  lines: z.array(purchaseReceiptLineDraftSchema).min(1),
});

export type Supplier = z.infer<typeof supplierSchema>;
export type SupplierDraft = z.infer<typeof supplierDraftSchema>;
export type PurchaseOrder = z.infer<typeof purchaseOrderSchema>;
export type PurchaseOrderDraft = z.infer<typeof purchaseOrderDraftSchema>;
export type PurchaseReceiptDraft = z.infer<typeof purchaseReceiptDraftSchema>;

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
