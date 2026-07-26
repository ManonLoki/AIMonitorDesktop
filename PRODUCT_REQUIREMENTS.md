# AIMonitorDesktop 产品要求

## 目标

基于 `/Users/manonloki/Documents/my-work/ai/AiMonitorAndroid` 已实现的业务逻辑、HTTP API 与界面文案，1:1 复刻可运行于 Windows 和 macOS 的桌面版。

## 强制要求

1. 界面保持简洁，不引入与监控画布无关的仪表盘或装饰信息。
2. 应用初始化时窗口立即最大化。

## 功能基线

- 监控画布支持 1–5 行、1–5 列，共 25 个可寻址宫格。
- 宫格显示 AI 名称、用户名、图片、内容与更新时间。
- 图片支持 JPG、PNG、GIF，以及“等比缩放”和“填满裁剪”两种显示模式。
- 提供与 Android 版兼容的 `/health`、`/api/device`、`/api/config`、`/api/images`、`/api/slots/{slot}` HTTP API。
- HTTP 端口从 10241 起自动选择，支持 `_aimonitor._tcp.` mDNS/DNS-SD 和 UDP 8080 广播发现。
- 设置持久化：设备名称、宫格行列、图片显示模式、开机自启与窗口位置/大小。
- 页面文案以 Android 版现有中文文案为准。
