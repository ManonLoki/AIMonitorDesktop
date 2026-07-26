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

    // 系统开启"减少动态效果"时直接清除动画属性并显示，不做任何入场动效。
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reducedMotion) {
      gsap.set(element, { clearProps: "all", visibility: "visible" });
      return;
    }

    // container 可传入选择器字符串，此处解析为实际的滚动容器元素。
    let scroller: Element | string | null = container ?? null;
    if (typeof scroller === "string") scroller = document.querySelector(scroller);

    const axis = direction === "horizontal" ? "x" : "y";
    const offset = (reverse ? -1 : 1) * distance;
    // 动画先建成暂停状态，实际播放时机交给下面的 ScrollTrigger 触发。
    const timeline = gsap.timeline({ paused: true, delay });

    // 起始帧：元素偏移 offset、可能透明、略微缩小，并标记 visibility 以避免闪烁。
    gsap.set(element, {
      [axis]: offset,
      opacity: animateOpacity ? initialOpacity : 1,
      scale,
      visibility: "visible",
      willChange: "transform, opacity",
    });

    // 结束帧：回到原位、完全不透明、原始缩放；完成后清掉内联样式，避免污染布局。
    timeline.to(element, {
      [axis]: 0,
      opacity: 1,
      scale: 1,
      duration,
      ease,
      clearProps: "transform,opacity,willChange",
    });

    // 元素进入视口（滚动容器缺省为 window）达到 threshold 比例时播放一次。
    const trigger = ScrollTrigger.create({
      trigger: element,
      scroller: scroller || window,
      start: `top ${(1 - threshold) * 100}%`,
      once: true,
      onEnter: () => timeline.play(),
    });

    // 卸载或依赖变化时清理 ScrollTrigger、时间线与残留的补间，避免内存泄漏。
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
