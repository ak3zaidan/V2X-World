/**
 * A button that opens a small menu, with the keyboard behaviour a menu is expected to have: the
 * first item takes focus when it opens, the arrow keys move between items, Escape closes it and
 * puts focus back on the button, and a click anywhere else closes it.
 */

import { useCallback, useEffect, useRef, useState } from "react";

const ITEM = '[role="menuitem"]:not([disabled]), [role="menuitemcheckbox"]:not([disabled])';

export function MenuButton({
  label,
  title,
  testId,
  menuTestId,
  className,
  align = "right",
  children,
}: {
  label: React.ReactNode;
  title: string;
  testId: string;
  menuTestId: string;
  className?: string;
  align?: "left" | "right";
  /** The menu's items; `close` shuts the menu and returns focus to the button. */
  children: (close: () => void) => React.ReactNode;
}): React.JSX.Element {
  const [open, setOpen] = useState(false);
  const host = useRef<HTMLDivElement | null>(null);
  const button = useRef<HTMLButtonElement | null>(null);
  const pop = useRef<HTMLDivElement | null>(null);

  const close = useCallback(() => {
    setOpen(false);
    button.current?.focus();
  }, []);

  useEffect(() => {
    if (!open) return;
    pop.current?.querySelector<HTMLElement>(ITEM)?.focus();
    const onDown = (ev: MouseEvent): void => {
      if (host.current && !host.current.contains(ev.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  const onKeyDown = useCallback(
    (ev: React.KeyboardEvent<HTMLDivElement>) => {
      if (ev.key === "Escape") {
        ev.preventDefault();
        ev.stopPropagation();
        close();
        return;
      }
      if (ev.key === "Tab") {
        setOpen(false);
        return;
      }
      if (ev.key !== "ArrowDown" && ev.key !== "ArrowUp" && ev.key !== "Home" && ev.key !== "End") return;
      const items = Array.from(pop.current?.querySelectorAll<HTMLElement>(ITEM) ?? []);
      if (items.length === 0) return;
      ev.preventDefault();
      const at = items.indexOf(document.activeElement as HTMLElement);
      const next =
        ev.key === "Home" ? 0 : ev.key === "End" ? items.length - 1 : ev.key === "ArrowDown" ? (at + 1) % items.length : (at - 1 + items.length) % items.length;
      items[next]?.focus();
    },
    [close],
  );

  return (
    <div className="menu" ref={host}>
      <button
        ref={button}
        type="button"
        className={className}
        aria-haspopup="menu"
        aria-expanded={open}
        title={title}
        aria-label={title}
        data-testid={testId}
        onClick={() => setOpen((v) => !v)}
        onKeyDown={(ev) => {
          if (ev.key === "ArrowDown" && !open) {
            ev.preventDefault();
            setOpen(true);
          }
        }}
      >
        {label}
      </button>
      {open ? (
        <div
          ref={pop}
          className={`menu-pop app-menu${align === "right" ? " anchor-right" : ""}`}
          role="menu"
          data-testid={menuTestId}
          onKeyDown={onKeyDown}
        >
          {children(close)}
        </div>
      ) : null}
    </div>
  );
}
