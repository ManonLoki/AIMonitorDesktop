import { gsap } from "gsap";
import { ScrollTrigger } from "gsap/ScrollTrigger";
import { useEffect, useRef, type HTMLAttributes, type ReactNode } from "react";

gsap.registerPlugin(ScrollTrigger);

interface AnimatedContentProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode;
  container?: Element | string | null;
  distance?: number;
  direction?: "vertical" | "horizontal";
  reverse?: boolean;
  duration?: number;
  ease?: string;
  initialOpacity?: number;
  animateOpacity?: boolean;
  scale?: number;
  threshold?: number;
  delay?: number;
}

/**
 * Adapted from React Bits AnimatedContent (TypeScript + CSS variant).
 * https://reactbits.dev/animations/animated-content
 */
export function AnimatedContent({
  children,
  container,
  distance = 28,
  direction = "vertical",
  reverse = false,
  duration = 0.55,
  ease = "power3.out",
  initialOpacity = 0,
  animateOpacity = true,
  scale = 0.985,
  threshold = 0.08,
  delay = 0,
  className = "",
  style,
  ...props
}: AnimatedContentProps) {
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const element = ref.current;
    if (!element) return;

    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reducedMotion) {
      gsap.set(element, { clearProps: "all", visibility: "visible" });
      return;
    }

    let scroller: Element | string | null = container ?? null;
    if (typeof scroller === "string") scroller = document.querySelector(scroller);

    const axis = direction === "horizontal" ? "x" : "y";
    const offset = (reverse ? -1 : 1) * distance;
    const timeline = gsap.timeline({ paused: true, delay });

    gsap.set(element, {
      [axis]: offset,
      opacity: animateOpacity ? initialOpacity : 1,
      scale,
      visibility: "visible",
      willChange: "transform, opacity",
    });

    timeline.to(element, {
      [axis]: 0,
      opacity: 1,
      scale: 1,
      duration,
      ease,
      clearProps: "transform,opacity,willChange",
    });

    const trigger = ScrollTrigger.create({
      trigger: element,
      scroller: scroller || window,
      start: `top ${(1 - threshold) * 100}%`,
      once: true,
      onEnter: () => timeline.play(),
    });

    return () => {
      trigger.kill();
      timeline.kill();
      gsap.killTweensOf(element);
    };
  }, [animateOpacity, container, delay, direction, distance, duration, ease, initialOpacity, reverse, scale, threshold]);

  return (
    <div ref={ref} className={className} style={{ visibility: "hidden", ...style }} {...props}>
      {children}
    </div>
  );
}
