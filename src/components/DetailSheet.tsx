import { createPortal } from "react-dom";
import { X } from "lucide-react";
import type { ReactNode } from "react";

interface DetailSheetProps {
  open: boolean;
  title: ReactNode;
  description?: ReactNode;
  meta?: ReactNode;
  onClose: () => void;
  children: ReactNode;
}

export function DetailSheet({
  open,
  title,
  description,
  meta,
  onClose,
  children,
}: DetailSheetProps) {
  if (!open) return null;

  return createPortal(
    <div className="fixed top-[28px] right-0 bottom-0 left-[220px] z-40 flex">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative flex h-full min-h-0 w-full flex-col overflow-hidden border-l border-border-subtle bg-bg-secondary shadow-2xl animate-in slide-in-from-right duration-200">
        <button
          onClick={onClose}
          className="absolute top-4 right-5 z-10 shrink-0 rounded-[4px] p-1.5 text-muted transition-colors outline-none hover:bg-surface-hover hover:text-secondary"
        >
          <X className="h-4 w-4" />
        </button>
        <div className="min-h-0 flex-1 overflow-y-auto px-6 pt-5 pb-6 scrollbar-hide animate-in fade-in duration-300">
          <h2 className="mb-3 min-w-0 pr-10 text-[28px] font-semibold leading-tight tracking-tight text-primary animate-in slide-in-from-left-2 duration-300">
            <span className="block">{title}</span>
          </h2>
          {description ? (
            <div className="text-[15px] leading-7 text-secondary">{description}</div>
          ) : null}
          {meta ? <div className="mt-4">{meta}</div> : null}
          <div className="mt-5">{children}</div>
        </div>
      </div>
    </div>,
    document.body
  );
}
