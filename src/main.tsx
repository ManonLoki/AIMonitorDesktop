import React from "react"; // React.StrictMode 需要
import ReactDOM from "react-dom/client"; // React 19 客户端渲染入口
import { MonitorApp } from "./MonitorApp"; // 应用根组件
import { PetApp } from "./PetApp";
import { PetSettingsApp } from "./PetSettingsApp";
import "./pet.css";
import "./styles.css"; // 全局样式

// 应用唯一入口：直接挂载 MonitorApp，没有路由或全局 Provider——
// 服务端状态由 Tauri invoke/event 提供，客户端没有需要跨组件共享的额外状态。
const view = new URLSearchParams(window.location.search).get("view");
const isPetWindow = view === "pet";
if (isPetWindow) document.documentElement.classList.add("pet-window");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render( // 挂载到 index.html 中的 #root 节点
  <React.StrictMode>
    {isPetWindow ? <PetApp /> : view === "pet-settings" ? <PetSettingsApp /> : <MonitorApp />}
  </React.StrictMode>,
);
