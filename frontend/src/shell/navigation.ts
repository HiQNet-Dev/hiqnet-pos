import type { ComponentType } from "react";
import type { Actor } from "../contracts";
import {
  CartIcon,
  CatalogIcon,
  CashIcon,
  InventoryIcon,
  ShieldIcon,
  type IconProps,
} from "./icons";

export type SectionId = "sales" | "catalog" | "inventory" | "purchasing" | "expenses" | "cash" | "admin";

export interface SectionDefinition {
  id: SectionId;
  label: string;
  /** Etiqueta de la barra estrecha (tablet y ventanas pequeñas). */
  shortLabel: string;
  title: string;
  description: string;
  /** Permisos que habilitan la sección (basta uno). */
  permissions: string[];
  Icon: ComponentType<IconProps>;
}

/**
 * Orden de navegación del escritorio. La visibilidad es solo presentación:
 * cada caso de uso vuelve a exigir su permiso en Rust.
 */
export const SECTIONS: SectionDefinition[] = [
  {
    id: "sales",
    label: "Ventas",
    shortLabel: "Ventas",
    title: "Punto de venta",
    description: "Catálogo, ticket y cobro en efectivo.",
    permissions: ["sales.sell"],
    Icon: CartIcon,
  },
  {
    id: "catalog",
    label: "Productos",
    shortLabel: "Productos",
    title: "Productos y catálogo",
    description: "Productos, categorías, variantes y modificadores.",
    permissions: ["catalog.manage"],
    Icon: CatalogIcon,
  },
  {
    id: "inventory",
    label: "Inventario",
    shortLabel: "Inventario",
    title: "Existencias y movimientos",
    description: "Existencias por producto y movimientos de inventario.",
    permissions: ["inventory.view"],
    Icon: InventoryIcon,
  },
  {
    id: "purchasing",
    label: "Compras",
    shortLabel: "Compras",
    title: "Compras y recepción",
    description: "Proveedores, órdenes de compra y recepción de mercancía.",
    permissions: ["purchasing.manage"],
    Icon: CatalogIcon,
  },
  {
    id: "expenses",
    label: "Gastos",
    shortLabel: "Gastos",
    title: "Gastos operativos",
    description: "Gastos externos y pagos desde caja.",
    permissions: ["expenses.manage"],
    Icon: CashIcon,
  },
  {
    id: "cash",
    label: "Cajas y turnos",
    shortLabel: "Caja",
    title: "Caja y turnos",
    description: "Apertura, movimientos, arqueo y cierre.",
    permissions: ["cash.shift.view", "cash.register.manage"],
    Icon: CashIcon,
  },
  {
    id: "admin",
    label: "Gestión",
    shortLabel: "Gestión",
    title: "Gestión y auditoría",
    description: "Licencia, respaldos y eventos del sistema.",
    permissions: ["audit.view", "backups.manage"],
    Icon: ShieldIcon,
  },
];

export function availableSections(actor: Actor): SectionDefinition[] {
  return SECTIONS.filter((section) =>
    section.permissions.some((permission) => actor.permissions.includes(permission)),
  );
}

export function defaultSection(actor: Actor): SectionId | null {
  const [first] = availableSections(actor);
  return first?.id ?? null;
}

export function sectionDefinition(id: SectionId): SectionDefinition {
  return SECTIONS.find((section) => section.id === id) ?? SECTIONS[0]!;
}
