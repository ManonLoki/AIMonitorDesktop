import { useRef, type CSSProperties, type HTMLAttributes, type MouseEventHandler } from "react";
import "./reactbits.css";

interface SpotlightCardProps extends HTMLAttributes<HTMLDivElement> {
  spotlightColor?: string;
}

type SpotlightStyle = CSSProperties & {
  "--spotlight-color"?: string;
  "--mouse-x"?: string;
  "--mouse-y"?: string;
};

/**
 * Adapted from React Bits SpotlightCard (TypeScript + CSS variant).
 * https://reactbits.dev/components/spotlight-card
 */
export function SpotlightCard({
  children,
  className = "",
  spotlightColor = "rgba(67, 109, 246, 0.14)",
  onMouseMove,
  style,
  ...props
}: SpotlightCardProps) {
  const cardRef = useRef<HTMLDivElement>(null);

  const handleMouseMove: MouseEventHandler<HTMLDivElement> = (event) => {
    const card = cardRef.current;
    if (card) {
      const rect = card.getBoundingClientRect();
      card.style.setProperty("--mouse-x", `${event.clientX - rect.left}px`);
      card.style.setProperty("--mouse-y", `${event.clientY - rect.top}px`);
    }
    onMouseMove?.(event);
  };

  const spotlightStyle: SpotlightStyle = {
    "--mouse-x": "50%",
    "--mouse-y": "50%",
    "--spotlight-color": spotlightColor,
    ...style,
  };

  return (
    <div
      ref={cardRef}
      className={`reactbits-spotlight ${className}`.trim()}
      onMouseMove={handleMouseMove}
      style={spotlightStyle}
      {...props}
    >
      {children}
    </div>
  );
}
