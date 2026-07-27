import React from "react"; // React.StrictMode 需要
import ReactDOM from "react-dom/client"; // React 18 客户端渲染入口
import { MonitorApp } from "./MonitorApp"; // 应用根组件
import "./styles.css"; // 全局样式

// 应用唯一入口：直接挂载 MonitorApp，没有路由或全局 Provider——
// 服务端状态由 Tauri invoke/event 提供，客户端没有需要跨组件共享的额外状态。
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render( // 挂载到 index.html 中的 #root 节点
  <React.StrictMode>
    <MonitorApp />
  </React.StrictMode>,
);
