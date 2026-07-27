import { gsap } from "gsap"; // 动画引擎
import { ScrollTrigger } from "gsap/ScrollTrigger"; // 基于滚动位置触发动画的 GSAP 插件
import { useEffect, useRef, type HTMLAttributes, type ReactNode } from "react";

gsap.registerPlugin(ScrollTrigger); // 注册插件，模块加载时执行一次即可

interface AnimatedContentProps extends HTMLAttributes<HTMLDivElement> {
  children: ReactNode; // 被包裹的内容
  container?: Element | string | null; // 滚动容器（选择器字符串或元素），缺省用 window
  distance?: number; // 入场时的初始偏移距离（像素）
  direction?: "vertical" | "horizontal"; // 偏移方向
  reverse?: boolean; // 是否反转偏移方向
  duration?: number; // 动画时长（秒）
  ease?: string; // GSAP 缓动函数名称
  initialOpacity?: number; // 初始透明度（仅在 animateOpacity 为 true 时生效）
  animateOpacity?: boolean; // 是否同时做透明度渐入动画
  scale?: number; // 初始缩放比例
  threshold?: number; // 元素进入视口多少比例时触发动画（0-1）
  delay?: number; // 动画开始前的延迟（秒）
}

/**
 * Adapted from React Bits AnimatedContent (TypeScript + CSS variant).
 * https://reactbits.dev/animations/animated-content
 */
export function AnimatedContent({
  children,
  container, // 滚动容器，未传时使用 window
  distance = 28, // 默认偏移 28px
  direction = "vertical", // 默认纵向偏移
  reverse = false, // 默认不反转方向
  duration = 0.55, // 默认动画时长 0.55s
  ease = "power3.out", // 默认缓动曲线
  initialOpacity = 0, // 默认从完全透明开始
  animateOpacity = true, // 默认启用透明度动画
  scale = 0.985, // 默认初始略微缩小
  threshold = 0.08, // 默认进入视口 8% 即触发
  delay = 0, // 默认无额外延迟
  className = "",
  style,
  ...props // 其余原生 div 属性透传
}: AnimatedContentProps) {
  const ref = useRef<HTMLDivElement>(null); // 指向被动画的 div

  useEffect(() => {
    const element = ref.current;
    if (!element) return; // 理论上挂载后必然存在，防御性判断

    // 系统开启"减少动态效果"时直接清除动画属性并显示，不做任何入场动效。
    const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches; // 读取操作系统的无障碍偏好
    if (reducedMotion) {
      gsap.set(element, { clearProps: "all", visibility: "visible" }); // 清除内联样式并直接可见
      return; // 不再创建时间线和 ScrollTrigger
    }

    // container 可传入选择器字符串，此处解析为实际的滚动容器元素。
    let scroller: Element | string | null = container ?? null;
    if (typeof scroller === "string") scroller = document.querySelector(scroller); // 字符串选择器转为真实 DOM 元素

    const axis = direction === "horizontal" ? "x" : "y"; // 决定操作 GSAP 的哪个位移属性
    const offset = (reverse ? -1 : 1) * distance; // reverse 时偏移方向取反
    // 动画先建成暂停状态，实际播放时机交给下面的 ScrollTrigger 触发。
    const timeline = gsap.timeline({ paused: true, delay });

    // 起始帧：元素偏移 offset、可能透明、略微缩小，并标记 visibility 以避免闪烁。
    gsap.set(element, {
      [axis]: offset, // 沿 x 或 y 轴设置初始偏移
      opacity: animateOpacity ? initialOpacity : 1, // 不启用透明度动画时直接保持完全不透明
      scale, // 初始缩放
      visibility: "visible", // 从 CSS 默认的 hidden 切到 visible（配合下方 return 的初始样式）
      willChange: "transform, opacity", // 提示浏览器提前做渲染优化
    });

    // 结束帧：回到原位、完全不透明、原始缩放；完成后清掉内联样式，避免污染布局。
    timeline.to(element, {
      [axis]: 0, // 回到无偏移的原始位置
      opacity: 1, // 完全不透明
      scale: 1, // 恢复原始缩放
      duration,
      ease,
      clearProps: "transform,opacity,willChange", // 动画结束后移除这些内联样式，恢复由 CSS 类控制
    });

    // 元素进入视口（滚动容器缺省为 window）达到 threshold 比例时播放一次。
    const trigger = ScrollTrigger.create({
      trigger: element, // 被观察是否进入视口的元素
      scroller: scroller || window, // 滚动容器，缺省监听整个窗口滚动
      start: `top ${(1 - threshold) * 100}%`, // 元素顶部到达视口 (1-threshold) 高度处时触发
      once: true, // 只播放一次，避免反复滚动重复触发
      onEnter: () => timeline.play(), // 触发时才真正播放之前建好的时间线
    });

    // 卸载或依赖变化时清理 ScrollTrigger、时间线与残留的补间，避免内存泄漏。
    return () => {
      trigger.kill(); // 销毁滚动监听
      timeline.kill(); // 销毁时间线
      gsap.killTweensOf(element); // 保险起见清理该元素上所有残留补间
    };
  }, [animateOpacity, container, delay, direction, distance, duration, ease, initialOpacity, reverse, scale, threshold]); // 任一动画参数变化都需要重建动画

  return (
    // 初始 inline style 设为 hidden，避免动画脚本执行前出现一帧未处理的原始内容（防止闪烁）
    <div ref={ref} className={className} style={{ visibility: "hidden", ...style }} {...props}>
      {children}
    </div>
  );
}
