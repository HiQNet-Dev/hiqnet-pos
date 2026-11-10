/** Datos de demostracion que alimentan el puente simulado. */

export const actor = {
  user_id: "u-1",
  organization_id: "o-1",
  branch_id: "b-1",
  display_name: "Ana Martínez",
  role: "owner",
  permissions: [
    "catalog.manage",
    "sales.sell",
    "inventory.view",
    "inventory.manage",
    "cash.shift.view",
    "cash.shift.open",
    "cash.shift.close",
    "cash.movement.record",
    "cash.register.manage",
    "audit.view",
    "backups.manage",
  ],
};

export const categories = [
  { id: "c-1", name: "Bebidas calientes", sort_order: 1, active: true },
  { id: "c-2", name: "Panadería", sort_order: 2, active: true },
  { id: "c-3", name: "Refrigerados", sort_order: 3, active: true },
  { id: "c-4", name: "Combos", sort_order: 4, active: true },
];

export const products = [
  {
    id: "p-1",
    sku: "CAF-001",
    name: "Café americano",
    unit_price: "12.50",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-1",
  },
  {
    id: "p-2",
    sku: "CAF-002",
    name: "Capuchino",
    unit_price: "18.00",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-1",
  },
  {
    id: "p-3",
    sku: "PAN-010",
    name: "Croissant de mantequilla",
    unit_price: "9.80",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-2",
  },
  {
    id: "p-4",
    sku: "PAN-011",
    name: "Concha de vainilla",
    unit_price: "7.50",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-2",
  },
  {
    id: "p-5",
    sku: "REF-020",
    name: "Agua mineral 600 ml",
    unit_price: "6.00",
    tax_rate_bp: 0,
    active: true,
    category_id: "c-3",
  },
  {
    id: "p-6",
    sku: "REF-021",
    name: "Jugo de naranja 355 ml",
    unit_price: "11.00",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-3",
  },
  {
    id: "p-7",
    sku: "COM-001",
    name: "Combo desayuno",
    unit_price: "39.00",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-4",
  },
  {
    id: "p-8",
    sku: "CAF-003",
    name: "Latte grande de temporada con leche de avena",
    unit_price: "24.00",
    tax_rate_bp: 1600,
    active: true,
    category_id: "c-1",
  },
];

export const variantsByProduct: Record<string, unknown[]> = {
  "p-2": [
    {
      id: "v-1",
      product_id: "p-2",
      name: "Chico",
      sku: "CAF-002-CH",
      unit_price: "18.00",
      active: true,
      sort_order: 1,
    },
    {
      id: "v-2",
      product_id: "p-2",
      name: "Grande",
      sku: "CAF-002-GR",
      unit_price: "22.00",
      active: true,
      sort_order: 2,
    },
  ],
  "p-7": [
    {
      id: "v-3",
      product_id: "p-7",
      name: "Para aquí",
      sku: "COM-001-AQ",
      unit_price: "39.00",
      active: true,
      sort_order: 1,
    },
    {
      id: "v-4",
      product_id: "p-7",
      name: "Para llevar",
      sku: "COM-001-LL",
      unit_price: "39.00",
      active: true,
      sort_order: 2,
    },
  ],
};

export const groupsByProduct: Record<string, unknown[]> = {
  "p-2": [
    { id: "g-1", name: "Leche", min: 1, max: 1, sort_order: 1, active: true },
    { id: "g-2", name: "Extras", min: 0, max: 3, sort_order: 2, active: true },
  ],
  "p-7": [
    { id: "g-3", name: "Bebida", min: 1, max: 1, sort_order: 1, active: true },
  ],
};

export const optionsByGroup: Record<string, unknown[]> = {
  "g-1": [
    {
      id: "o-1",
      group_id: "g-1",
      name: "Entera",
      kind: "choice",
      price_delta: "0.00",
      sort_order: 1,
      active: true,
    },
    {
      id: "o-2",
      group_id: "g-1",
      name: "Deslactosada",
      kind: "choice",
      price_delta: "0.00",
      sort_order: 2,
      active: true,
    },
    {
      id: "o-3",
      group_id: "g-1",
      name: "Avena",
      kind: "choice",
      price_delta: "5.00",
      sort_order: 3,
      active: true,
    },
  ],
  "g-2": [
    {
      id: "o-4",
      group_id: "g-2",
      name: "Shot extra",
      kind: "extra",
      price_delta: "8.00",
      sort_order: 1,
      active: true,
    },
    {
      id: "o-5",
      group_id: "g-2",
      name: "Sin azúcar",
      kind: "omission",
      price_delta: "0.00",
      sort_order: 2,
      active: true,
    },
    {
      id: "o-6",
      group_id: "g-2",
      name: "Canela",
      kind: "extra",
      price_delta: "2.00",
      sort_order: 3,
      active: true,
    },
  ],
  "g-3": [
    {
      id: "o-7",
      group_id: "g-3",
      name: "Café americano",
      kind: "choice",
      price_delta: "0.00",
      sort_order: 1,
      active: true,
    },
    {
      id: "o-8",
      group_id: "g-3",
      name: "Jugo de naranja",
      kind: "choice",
      price_delta: "4.00",
      sort_order: 2,
      active: true,
    },
  ],
};

export const shift = {
  id: "s-1",
  register_id: "r-1",
  status: "open",
  opened_by: "u-1",
  opened_at: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
  closed_by: null,
  closed_at: null,
  opening_amount: "500.00",
  expected_amount: "1842.80",
  entry_total: "1342.80",
  exit_total: "0.00",
  counted_amount: null,
  difference_minor: null,
  movements: [
    {
      id: "m-1",
      kind: "in",
      amount: "200.00",
      reason: "Cambio adicional",
      occurred_at: new Date(Date.now() - 2 * 60 * 60 * 1000).toISOString(),
    },
    {
      id: "m-2",
      kind: "out",
      amount: "50.00",
      reason: "Compra de hielo",
      occurred_at: new Date(Date.now() - 60 * 60 * 1000).toISOString(),
    },
  ],
};

export const closedShift = {
  ...shift,
  id: "s-0",
  status: "closed",
  closed_by: "u-1",
  closed_at: new Date(Date.now() - 24 * 60 * 60 * 1000).toISOString(),
  counted_amount: "1490.00",
  difference_minor: -20,
};

export const movements = [
  {
    id: "im-1",
    product_id: "p-1",
    kind: "outbound",
    quantity: "2.000",
    reason: "Venta F-000128",
    occurred_at: new Date(Date.now() - 40 * 60 * 1000).toISOString(),
  },
  {
    id: "im-2",
    product_id: "p-1",
    kind: "inbound",
    quantity: "24.000",
    reason: "Compra a proveedor",
    occurred_at: new Date(Date.now() - 5 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: "im-3",
    product_id: "p-1",
    kind: "adjustment_out",
    quantity: "1.000",
    reason: "Producto derramado",
    occurred_at: new Date(Date.now() - 26 * 60 * 60 * 1000).toISOString(),
  },
];

export const auditEvents = [
  {
    id: "a-1",
    action: "sales.completed",
    entity_type: "sale",
    entity_id: "sale-128",
    actor_user_id: "u-1",
    occurred_at: new Date(Date.now() - 12 * 60 * 1000).toISOString(),
  },
  {
    id: "a-2",
    action: "inventory.movement_registered",
    entity_type: "inventory_movement",
    entity_id: "im-1",
    actor_user_id: "u-1",
    occurred_at: new Date(Date.now() - 40 * 60 * 1000).toISOString(),
  },
  {
    id: "a-3",
    action: "cash.shift_opened",
    entity_type: "cash_shift",
    entity_id: "s-1",
    actor_user_id: "u-1",
    occurred_at: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
  },
  {
    id: "a-4",
    action: "session.logged_in",
    entity_type: "session",
    entity_id: "sess-1",
    actor_user_id: "u-1",
    occurred_at: new Date(Date.now() - 3 * 60 * 60 * 1000).toISOString(),
  },
];
