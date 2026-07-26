// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// 可执行文件入口，实际逻辑都在 lib.rs 的 run() 里（Tauri 移动端入口也复用同一个 run）。
fn main() {
    aimonitor_desktop_lib::run()
}
