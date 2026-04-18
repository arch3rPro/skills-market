
import { useState, useRef, useEffect } from "react";
import { ChevronDown, Check } from "lucide-react";
import { cn } from "../utils";

interface Option {
  value: string;
  label: string;
}

interface CustomSelectProps {
  value: string;
  onChange: (value: string) => void;
  options: Option[];
  placeholder?: string;
  className?: string;
  disabled?: boolean;
}

export function CustomSelect({
  value,
  onChange,
  options,
  placeholder = "—",
  className,
  disabled = false,
}: CustomSelectProps) {
  const [isOpen, setIsOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const selectedOption = options.find((opt) => opt.value === value);
  const displayLabel = selectedOption?.label || placeholder;

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        containerRef.current &&
        !containerRef.current.contains(event.target as Node)
      ) {
        setIsOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        setIsOpen(false);
      }
    };

    if (isOpen) {
      document.addEventListener("keydown", handleKeyDown);
    }
    return () => document.removeEventListener("keydown", handleKeyDown);
  }, [isOpen]);

  const handleSelect = (optionValue: string) => {
    onChange(optionValue);
    setIsOpen(false);
    buttonRef.current?.focus();
  };

  return (
    <div className={cn("relative", className)} ref={containerRef}>
      <button
        ref={buttonRef}
        type="button"
        onClick={() => !disabled && setIsOpen(!isOpen)}
        disabled={disabled}
        className={cn(
          "h-8 rounded-[4px] border border-border-subtle bg-background px-2.5 text-[13px] text-secondary outline-none transition-colors",
          "focus:border-border",
          "flex items-center justify-between gap-2",
          disabled && "opacity-50 cursor-not-allowed"
        )}
      >
        <span className="truncate">{displayLabel}</span>
        <ChevronDown
          className={cn(
            "w-3.5 h-3.5 text-muted transition-transform",
            isOpen && "rotate-180"
          )}
        />
      </button>

      {isOpen && (
        <div className="absolute z-50 mt-1 w-full min-w-[160px] max-h-[240px] overflow-y-auto rounded-[6px] border border-border bg-surface shadow-lg">
          <div className="py-1">
            {options.map((option) => (
              <button
                key={option.value}
                type="button"
                onClick={() => handleSelect(option.value)}
                className={cn(
                  "w-full px-2.5 py-1.5 text-left text-[13px] transition-colors outline-none",
                  "flex items-center justify-between gap-2",
                  option.value === value
                    ? "bg-surface-active text-secondary"
                    : "text-tertiary hover:bg-surface-hover hover:text-secondary"
                )}
              >
                <span className="truncate">{option.label}</span>
                {option.value === value && (
                  <Check className="w-3.5 h-3.5 text-accent flex-shrink-0" />
                )}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

