// Prevents additional console window on Windows in release, DO NOT REMOVE!!
// 仅在 release 构建时生效：把子系统设为 "windows"，避免 Windows 上弹出多余的控制台黑窗口。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// 可执行文件入口，实际逻辑都在 lib.rs 的 run() 里（Tauri 移动端入口也复用同一个 run）。
fn main() {
    aimonitor_desktop_lib::run() // 转发到库 crate 的 run()，本文件不包含任何业务逻辑
}
