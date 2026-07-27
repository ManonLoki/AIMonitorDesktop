import { useRef, type CSSProperties, type HTMLAttributes, type MouseEventHandler } from "react";
import "./reactbits.css"; // 光晕效果的实际 CSS 实现（径向渐变）

interface SpotlightCardProps extends HTMLAttributes<HTMLDivElement> {
  spotlightColor?: string; // 光晕颜色，需为带透明度的颜色值
}

// 扩展 CSSProperties，允许写入自定义 CSS 变量（TS 默认不认识 "--xxx" 这类属性名）
type SpotlightStyle = CSSProperties & {
  "--spotlight-color"?: string; // 光晕颜色变量
  "--mouse-x"?: string; // 鼠标当前横坐标（相对卡片）
  "--mouse-y"?: string; // 鼠标当前纵坐标（相对卡片）
};

/**
 * Adapted from React Bits SpotlightCard (TypeScript + CSS variant).
 * https://reactbits.dev/components/spotlight-card
 */
export function SpotlightCard({
  children,
  className = "",
  spotlightColor = "rgba(67, 109, 246, 0.14)", // 默认蓝色半透明光晕
  onMouseMove, // 外部传入的鼠标移动回调，内部处理完后继续转发
  style,
  ...props // 其余原生 div 属性透传
}: SpotlightCardProps) {
  const cardRef = useRef<HTMLDivElement>(null); // 指向卡片根节点，用于读取其位置和直接写内联样式

  // 直接写 CSS 自定义属性而非 setState，避免鼠标移动时触发 React 重渲染；
  // 光晕效果完全交给 reactbits.css 里基于 --mouse-x/--mouse-y 的径向渐变绘制。
  const handleMouseMove: MouseEventHandler<HTMLDivElement> = (event) => {
    const card = cardRef.current;
    if (card) {
      const rect = card.getBoundingClientRect(); // 卡片在视口中的位置和尺寸
      card.style.setProperty("--mouse-x", `${event.clientX - rect.left}px`); // 换算成相对卡片左边的横坐标
      card.style.setProperty("--mouse-y", `${event.clientY - rect.top}px`); // 换算成相对卡片顶部的纵坐标
    }
    onMouseMove?.(event); // 继续调用外部传入的回调（若有）
  };

  const spotlightStyle: SpotlightStyle = {
    "--mouse-x": "50%", // 初始光晕居中显示，鼠标移动前不会偏向一侧
    "--mouse-y": "50%",
    "--spotlight-color": spotlightColor,
    ...style, // 允许外部 style 覆盖以上默认值
  };

  return (
    <div
      ref={cardRef}
      className={`reactbits-spotlight ${className}`.trim()} // 拼接基础类名与外部传入的类名
      onMouseMove={handleMouseMove}
      style={spotlightStyle}
      {...props}
    >
      {children}
    </div>
  );
}
