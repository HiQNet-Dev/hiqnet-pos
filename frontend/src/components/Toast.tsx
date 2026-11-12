import { createPortal } from "react-dom";
import { CloseIcon } from "../shell/icons";

interface Props {
  /** `null` mantiene la región viva para que los lectores anuncien el siguiente mensaje. */
  message: string | null;
  tone?: "success" | "warning" | "error";
  className?: string;
  actionLabel?: string;
  onAction?: () => void;
  onDismiss: () => void;
}

/**
 * Aviso flotante para resultados globales. No participa del layout: el shell no
 * se desplaza cuando aparece una venta confirmada. La región `aria-live` existe
 * siempre y el contenido se anuncia al cambiar.
 */
export function Toast({
  message,
  tone = "success",
  className,
  actionLabel,
  onAction,
  onDismiss,
}: Props) {
  return createPortal(
    <div
      className={`toast-region${className ? ` ${className}` : ""}`}
      aria-live={tone === "error" ? "assertive" : "polite"}
      aria-atomic="true"
    >
      {message && (
        <div
          className={`toast toast-${tone}`}
          role={tone === "error" ? "alert" : "status"}
        >
          <span className="toast-message">{message}</span>
          {actionLabel && onAction && (
            <button
              type="button"
              className="button button-small toast-action"
              onClick={onAction}
            >
              {actionLabel}
            </button>
          )}
          <button
            type="button"
            className="icon-button toast-dismiss"
            onClick={onDismiss}
            aria-label="Descartar mensaje"
          >
            <CloseIcon size={16} />
          </button>
        </div>
      )}
    </div>,
    document.body,
  );
}
