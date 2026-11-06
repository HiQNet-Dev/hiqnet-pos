import { useEffect, useId, useRef, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { CloseIcon } from "../shell/icons";

interface Props {
  title: string;
  subtitle?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  size?: "md" | "lg";
}

const FOCUSABLE =
  'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/**
 * Diálogo modal accesible: bloquea el fondo, atrapa el foco, cierra con Escape
 * y devuelve el foco al elemento que lo abrió.
 */
export function Modal({ title, subtitle, onClose, children, footer, size = "md" }: Props) {
  const titleId = useId();
  const container = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  // Se captura en el primer render, no en el efecto: un campo con `autoFocus`
  // mueve el foco durante el montaje y el efecto ya no vería el control que
  // abrió el diálogo, dejando el foco en el `body` al cerrar.
  const opener = useRef<HTMLElement | null>(null);
  if (opener.current === null) {
    opener.current = document.activeElement as HTMLElement | null;
  }

  useEffect(() => {
    onCloseRef.current = onClose;
  }, [onClose]);

  useEffect(() => {
    const node = container.current;
    const first = node?.querySelector<HTMLElement>(FOCUSABLE);
    (first ?? node)?.focus();

    function handleKey(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.stopPropagation();
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab" || !node) return;
      const focusable = Array.from(node.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
        (element) => element.offsetParent !== null || element === document.activeElement,
      );
      if (focusable.length === 0) return;
      const firstElement = focusable[0]!;
      const lastElement = focusable[focusable.length - 1]!;
      if (event.shiftKey && document.activeElement === firstElement) {
        event.preventDefault();
        lastElement.focus();
      } else if (!event.shiftKey && document.activeElement === lastElement) {
        event.preventDefault();
        firstElement.focus();
      }
    }

    document.addEventListener("keydown", handleKey, true);
    // No se toca `document.body.style.overflow`: el shell ya recorta el documento
    // (ver `styles.css`). Cambiarlo aquí convertía la raíz en un contenedor
    // desplazable mientras el diálogo estaba abierto, y el foco podía desplazar
    // la interfaz completa al abrir y cerrar diálogos repetidamente.
    return () => {
      document.removeEventListener("keydown", handleKey, true);
      // Devolver el foco sin arrastrar la pantalla de fondo.
      opener.current?.focus?.({ preventScroll: true });
    };
  }, []);

  return createPortal(
    <div
      className="modal-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        className={size === "lg" ? "modal modal-lg" : "modal"}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        ref={container}
        tabIndex={-1}
      >
        <header className="modal-header">
          <div>
            <h2 id={titleId}>{title}</h2>
            {subtitle && <p className="modal-subtitle">{subtitle}</p>}
          </div>
          <button
            type="button"
            className="icon-button"
            onClick={onClose}
            aria-label="Cerrar"
          >
            <CloseIcon />
          </button>
        </header>
        <div className="modal-body">{children}</div>
        {footer && <footer className="modal-footer">{footer}</footer>}
      </div>
    </div>,
    document.body,
  );
}
