import React from "react";
import ReactDOM from "react-dom/client";
import { MonitorApp } from "./MonitorApp";
import "./styles.css";

// 应用唯一入口：直接挂载 MonitorApp，没有路由或全局 Provider——
// 服务端状态由 Tauri invoke/event 提供，客户端没有需要跨组件共享的额外状态。
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <MonitorApp />
  </React.StrictMode>,
);
