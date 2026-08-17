import { useCallback, useRef, useState, type MouseEvent, type ReactNode } from "react";
import "./ClickRipple.css";

interface Ripple {
  id: number;
  x: number;
  y: number;
}

export default function ClickRipple({ children }: { children: ReactNode }) {
  const [ripples, setRipples] = useState<Ripple[]>([]);
  const nextId = useRef(0);

  const onClick = useCallback((event: MouseEvent<HTMLDivElement>) => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const target = event.target as HTMLElement | null;
    if (target?.closest(".caption-btn")) return;

    const rect = event.currentTarget.getBoundingClientRect();
    const id = nextId.current++;
    setRipples((current) => [
      ...current,
      { id, x: event.clientX - rect.left, y: event.clientY - rect.top },
    ]);
  }, []);

  function dismiss(id: number) {
    setRipples((current) => current.filter((ripple) => ripple.id !== id));
  }

  return (
    <div className="click-ripple-host" onClick={onClick}>
      {children}
      {ripples.map((ripple) => (
        <span
          key={ripple.id}
          className="click-ripple"
          style={{ left: ripple.x, top: ripple.y }}
        >
          <span className="click-ripple-fill" />
          <span className="click-ripple-ring" onAnimationEnd={() => dismiss(ripple.id)} />
        </span>
      ))}
    </div>
  );
}
