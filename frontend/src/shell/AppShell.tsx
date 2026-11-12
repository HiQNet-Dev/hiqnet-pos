import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, IpcError } from "../api";
import type {
  Actor,
  BranchSummary,
  CashRegister,
  CashShift,
  Category,
  CommercialState,
  Product,
  SaleView,
} from "../contracts";
import type { CartLine } from "../lib/cart";
import {
  loadPendingAttempt,
  pendingAttemptStorageKey,
  savePendingAttempt,
  type PendingExpenseAttempt,
  type PendingReturnAttempt,
  type PendingSaleAttempt,
  type PendingWasteAttempt,
} from "../lib/pendingAttempt";
import { AdminScreen } from "../screens/AdminScreen";
import { CashScreen } from "../screens/CashScreen";
import { CatalogScreen } from "../screens/CatalogScreen";
import { InventoryScreen } from "../screens/InventoryScreen";
import { ExpensesScreen } from "../screens/ExpensesScreen";
import { PurchasingScreen, type PendingReceiptAttempt } from "../screens/PurchasingScreen";
import { SalesScreen } from "../screens/SalesScreen";
import { ReportsScreen } from "../screens/ReportsScreen";
import { ReturnsScreen } from "../screens/ReturnsScreen";
import { SideNav } from "./SideNav";
import { TopBar } from "./TopBar";
import { ChangePasswordDialog } from "./ChangePasswordDialog";
import { Toast } from "../components/Toast";
import { availableSections, defaultSection, type SectionId } from "./navigation";

interface Props {
  actor: Actor;
  commercial: CommercialState | null;
  productVersion: string;
  onLogout: () => void;
}

function messageOf(cause: unknown, fallback: string): string {
  return cause instanceof IpcError ? cause.message : fallback;
}

/** Preferencia de navegación por instalación. No contiene datos personales. */
const NAV_COLLAPSED_KEY = "hiqnet.nav.collapsed";

function loadNavPreference(): boolean {
  try {
    return window.localStorage.getItem(NAV_COLLAPSED_KEY) === "true";
  } catch {
    return false;
  }
}

function saveNavPreference(collapsed: boolean): void {
  try {
    window.localStorage.setItem(NAV_COLLAPSED_KEY, collapsed ? "true" : "false");
  } catch {
    // Sin almacenamiento la preferencia vive solo en memoria.
  }
}

function receiptAttemptStorageKey(actor: Actor): string {
  return `hiqnet.pending-receipt.${actor.organization_id}.${actor.branch_id}.${actor.user_id}`;
}

function loadPendingReceipt(key: string): PendingReceiptAttempt | null {
  try {
    const value: unknown = JSON.parse(window.localStorage.getItem(key) ?? "null");
    if (
      value && typeof value === "object" &&
      typeof (value as PendingReceiptAttempt).orderId === "string" &&
      typeof (value as PendingReceiptAttempt).fingerprint === "string" &&
      typeof (value as PendingReceiptAttempt).key === "string" &&
      typeof (value as PendingReceiptAttempt).quantities === "object"
    ) {
      return value as PendingReceiptAttempt;
    }
  } catch {
    // El dato local es solo una ayuda de recuperación y nunca es autoridad.
  }
  return null;
}

/**
 * Shell de escritorio: barra superior, navegación lateral y una sola pantalla
 * activa. Mantiene el estado compartido (catálogo, caja y ticket) para que
 * cambiar de sección no pierda trabajo en curso.
 */
export function AppShell({ actor, commercial, productVersion, onLogout }: Props) {
  const sections = useMemo(() => availableSections(actor), [actor]);
  const initialSection = defaultSection(actor);
  const [section, setSection] = useState<SectionId>(initialSection ?? "sales");
  const [collapsed, setCollapsed] = useState<boolean>(loadNavPreference);

  useEffect(() => {
    saveNavPreference(collapsed);
  }, [collapsed]);

  const canSell = actor.permissions.includes("sales.sell");
  const canViewCash =
    actor.permissions.includes("cash.shift.view") ||
    actor.permissions.includes("cash.register.manage");
  const readOnly = commercial?.read_only ?? false;

  const [products, setProducts] = useState<Product[]>([]);
  const [categories, setCategories] = useState<Category[]>([]);
  const [stockLevels, setStockLevels] = useState<Record<string, string>>({});
  const [catalogLoading, setCatalogLoading] = useState(true);
  const [catalogError, setCatalogError] = useState<string | null>(null);
  const [catalogVersion, setCatalogVersion] = useState(0);

  const [branch, setBranch] = useState<BranchSummary | null>(null);
  const [registers, setRegisters] = useState<CashRegister[]>([]);
  const [registerId, setRegisterId] = useState("");
  const [shiftState, setShiftState] = useState<{
    registerId: string;
    shift: CashShift | null;
  } | null>(null);
  const [cashVersion, setCashVersion] = useState(0);
  const shiftRequest = useRef(0);

  const [cart, setCart] = useState<CartLine[]>([]);
  const saleAttemptKey = pendingAttemptStorageKey("sale", actor);
  const [pendingSale, setPendingSale] = useState<PendingSaleAttempt | null>(() =>
    loadPendingAttempt<PendingSaleAttempt["payload"]>(saleAttemptKey),
  );
  const receiptAttemptKey = receiptAttemptStorageKey(actor);
  const [pendingReceipt, setPendingReceipt] = useState<PendingReceiptAttempt | null>(() =>
    loadPendingReceipt(receiptAttemptKey),
  );
  const expenseAttemptKey = pendingAttemptStorageKey("expense", actor);
  const wasteAttemptKey = pendingAttemptStorageKey("waste", actor);
  const returnAttemptKey = pendingAttemptStorageKey("return", actor);
  const [pendingExpense, setPendingExpense] = useState<PendingExpenseAttempt | null>(() =>
    loadPendingAttempt<PendingExpenseAttempt["payload"]>(expenseAttemptKey),
  );
  const [pendingWaste, setPendingWaste] = useState<PendingWasteAttempt | null>(() =>
    loadPendingAttempt<PendingWasteAttempt["payload"]>(wasteAttemptKey),
  );
  const [pendingReturn, setPendingReturn] = useState<PendingReturnAttempt | null>(() =>
    loadPendingAttempt<PendingReturnAttempt["payload"]>(returnAttemptKey),
  );
  const [passwordOpen, setPasswordOpen] = useState(false);
  const [notice, setNotice] = useState<{ tone: "success" | "error"; text: string } | null>(
    null,
  );

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(() => setNotice(null), 6_000);
    return () => window.clearTimeout(timer);
  }, [notice]);

  const refreshCatalog = useCallback(() => setCatalogVersion((value) => value + 1), []);
  const refreshCash = useCallback(() => setCashVersion((value) => value + 1), []);
  const updatePendingReceipt = useCallback((attempt: PendingReceiptAttempt | null) => {
    setPendingReceipt(attempt);
    try {
      if (attempt) window.localStorage.setItem(receiptAttemptKey, JSON.stringify(attempt));
      else window.localStorage.removeItem(receiptAttemptKey);
    } catch {
      // La recuperación en memoria sigue siendo segura si el almacenamiento local falla.
    }
  }, [receiptAttemptKey]);

  const updatePendingSale = useCallback((attempt: PendingSaleAttempt | null) => {
    setPendingSale(attempt);
    savePendingAttempt(saleAttemptKey, attempt);
  }, [saleAttemptKey]);

  const updatePendingExpense = useCallback((attempt: PendingExpenseAttempt | null) => {
    setPendingExpense(attempt);
    savePendingAttempt(expenseAttemptKey, attempt);
  }, [expenseAttemptKey]);

  const updatePendingWaste = useCallback((attempt: PendingWasteAttempt | null) => {
    setPendingWaste(attempt);
    savePendingAttempt(wasteAttemptKey, attempt);
  }, [wasteAttemptKey]);

  const updatePendingReturn = useCallback((attempt: PendingReturnAttempt | null) => {
    setPendingReturn(attempt);
    savePendingAttempt(returnAttemptKey, attempt);
  }, [returnAttemptKey]);


  const availableIds = useMemo(() => sections.map((item) => item.id), [sections]);
  // Sección efectiva derivada: si el rol pierde acceso, se usa la primera disponible.
  const activeSection: SectionId = availableIds.includes(section)
    ? section
    : (availableIds[0] ?? "sales");

  useEffect(() => {
    let cancelled = false;
    api
      .branchSummary()
      .then((value) => {
        if (!cancelled) setBranch(value);
      })
      .catch(() => {
        // La sucursal se muestra como "Sucursal activa" si no está disponible.
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      setCatalogLoading(true);
      try {
        const [productList, categoryList] = await Promise.all([
          api.listProducts(),
          api.listCategories(),
        ]);
        if (cancelled) return;
        setProducts(productList);
        setCategories(categoryList);
        setCatalogError(null);
        if (canSell) {
          const levels = await api.listStockLevels();
          if (cancelled) return;
          setStockLevels(
            Object.fromEntries(levels.map((level) => [level.product_id, level.quantity])),
          );
        }
      } catch (cause) {
        if (!cancelled) {
          setCatalogError(messageOf(cause, "No fue posible cargar el catálogo."));
        }
      } finally {
        if (!cancelled) setCatalogLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [catalogVersion, canSell]);

  useEffect(() => {
    if (!canViewCash) return;
    let cancelled = false;
    api
      .listCashRegisters()
      .then((list) => {
        if (cancelled) return;
        setRegisters(list);
        setRegisterId((current) =>
          current && list.some((register) => register.id === current)
            ? current
            : (list[0]?.id ?? ""),
        );
      })
      .catch(() => {
        if (!cancelled) setRegisters([]);
      });
    return () => {
      cancelled = true;
    };
  }, [canViewCash, cashVersion]);

  useEffect(() => {
    if (!canViewCash || !registerId) return;
    let cancelled = false;
    const request = ++shiftRequest.current;
    api
      .activeCashShift(registerId)
      .then((value) => {
        if (!cancelled && request === shiftRequest.current) {
          setShiftState({ registerId, shift: value });
        }
      })
      .catch(() => {
        if (!cancelled && request === shiftRequest.current) {
          setShiftState({ registerId, shift: null });
        }
      });
    return () => {
      cancelled = true;
    };
  }, [canViewCash, registerId, cashVersion]);

  // El turno se deriva de la caja seleccionada: nunca se muestra uno ajeno.
  const shift =
    canViewCash && shiftState && shiftState.registerId === registerId ? shiftState.shift : null;

  const activeRegister = registers.find((register) => register.id === registerId) ?? null;

  const handleSaleCompleted = useCallback(
    (sale: SaleView) => {
      setCart([]);
      setNotice({ tone: "success", text: `Venta ${sale.folio} confirmada.` });
      refreshCash();
      refreshCatalog();
    },
    [refreshCash, refreshCatalog],
  );

  const showSection = (next: SectionId) => {
    if (!availableIds.includes(next)) return;
    setSection(next);
    setNotice(null);
  };

  return (
    <div className="app">
      <a className="skip-link" href="#contenido">
        Saltar al contenido
      </a>

      <TopBar
        actor={actor}
        branch={branch}
        register={activeRegister}
        shift={shift}
        commercial={commercial}
        onLogout={onLogout}
        onOpenCash={() => showSection("cash")}
        canOpenCash={canViewCash}
        onChangePassword={() => setPasswordOpen(true)}
      />

      <div className="app-body" data-collapsed={collapsed}>
        <SideNav
          sections={sections}
          active={activeSection}
          collapsed={collapsed}
          onSelect={showSection}
          onToggle={() => setCollapsed((value) => !value)}
          cartCount={cart.reduce((total, line) => total + line.quantity, 0)}
          version={productVersion}
        />

        <main className="app-content" id="contenido">
          {readOnly && (
            <p className="banner banner-warning" role="status">
              La licencia está vencida. Puedes consultar, exportar y respaldar, pero no
              registrar nuevas operaciones hasta renovar.
            </p>
          )}

          {catalogError && (
            <div className="banner-row" role="alert">
              <p className="banner banner-error">
                {catalogError}
                <button type="button" className="button button-quiet" onClick={refreshCatalog}>
                  Reintentar
                </button>
              </p>
            </div>
          )}

          <div className="screen-host">
            {activeSection === "sales" && canSell && (
              <SalesScreen
                actor={actor}
                readOnly={readOnly}
                products={products}
                categories={categories}
                stockLevels={stockLevels}
                registers={registers}
                registerId={registerId}
                onRegisterChange={setRegisterId}
                onRegisterRequired={() => showSection("cash")}
                shift={shift}
                cart={cart}
                onCartChange={setCart}
                onSaleCompleted={handleSaleCompleted}
                pendingAttempt={pendingSale}
                onPendingAttemptChange={updatePendingSale}
                onOpenCatalog={() => showSection("catalog")}
                loading={catalogLoading}
              />
            )}

            {activeSection === "catalog" && (
              <CatalogScreen
                actor={actor}
                readOnly={readOnly}
                products={products}
                categories={categories}
                loading={catalogLoading}
                onDataChanged={refreshCatalog}
              />
            )}

            {activeSection === "inventory" && (
              <InventoryScreen
                actor={actor}
                readOnly={readOnly}
                products={products}
                loading={catalogLoading}
                onStockChanged={refreshCatalog}
                pendingWaste={pendingWaste}
                onPendingWasteChange={updatePendingWaste}
              />
            )}

            {activeSection === "purchasing" && (
              <PurchasingScreen
                actor={actor}
                readOnly={readOnly}
                products={products}
                onInventoryChanged={refreshCatalog}
                pendingReceipt={pendingReceipt}
                onPendingReceiptChange={updatePendingReceipt}
              />
            )}

            {activeSection === "expenses" && (
              <ExpensesScreen
                actor={actor}
                readOnly={readOnly}
                shift={shift}
                onCashChanged={refreshCash}
                pendingAttempt={pendingExpense}
                onPendingAttemptChange={updatePendingExpense}
              />
            )}

            {activeSection === "returns" && (
              <ReturnsScreen
                actor={actor}
                readOnly={readOnly}
                shift={shift}
                onCashChanged={refreshCash}
                pendingAttempt={pendingReturn}
                onPendingAttemptChange={updatePendingReturn}
              />
            )}

            {activeSection === "reports" && (
              <ReportsScreen actor={actor} readOnly={readOnly} />
            )}

            {activeSection === "cash" && (
              <CashScreen
                actor={actor}
                readOnly={readOnly}
                registers={registers}
                registerId={registerId}
                onRegisterChange={setRegisterId}
                shift={shift}
                onShiftChanged={refreshCash}
                onRegisterCreated={refreshCash}
              />
            )}

            {activeSection === "admin" && (
              <AdminScreen actor={actor} commercial={commercial} readOnly={readOnly} />
            )}

            {availableIds.length === 0 && (
              <div className="empty-state">
                <p>Tu usuario no tiene secciones habilitadas.</p>
                <p className="muted">Pide al propietario que revise tus permisos.</p>
              </div>
            )}
          </div>
        </main>
      </div>

      {passwordOpen && (
        <ChangePasswordDialog
          onClose={() => setPasswordOpen(false)}
          onChanged={() =>
            setNotice({ tone: "success", text: "Contraseña actualizada." })
          }
        />
      )}

      <Toast
        message={notice?.text ?? null}
        tone={notice?.tone ?? "success"}
        onDismiss={() => setNotice(null)}
      />
    </div>
  );
}
