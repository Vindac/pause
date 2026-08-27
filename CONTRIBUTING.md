# 贡献指南

感谢你考虑为「休一下（Pause）」做贡献！

## 开发环境

- Node.js ≥ 18 与 npm
- Rust stable（[rustup](https://rustup.rs)）
- macOS 需 Xcode Command Line Tools；Windows 需 MSVC Build Tools

```bash
git clone <repo-url>
cd pause
npm install            # 安装前端依赖
npm run tauri dev      # 开发模式（vite HMR + Rust debug 构建）
```

## 测试与检查

```bash
cd src-tauri
cargo test             # 全部单元测试（状态机 / 设置语义 / 壁纸管线 / 布局缩放…）
cargo clippy           # 静态检查
```

```bash
npm run build          # 前端构建校验
```

## 项目约定

- **业务逻辑只写在 Rust 侧**：状态机、设置语义、壁纸管线、平台桥都位于 `src-tauri/src/`；前端 `src/` 保持纯视图层（收事件、发意图），不承载决策逻辑
- **文案**：新增界面文案一律先加入 `src-tauri/src/l10n.rs` 的中英内建表（带参数用 `{name}` 模板占位符），前端经 `t(key, params)` 取词——不要在前端硬编码双语字符串
- **设置项**：新字段需同时覆盖写入钳制与读取回退两种语义（见 `settings.rs` 及其测试）
- **平台代码**：所有原生调用集中在 `src-tauri/src/platform/{macos,windows}.rs` 并以 trait 探针隔离；任何涉及空闲查询的调用不得放到主线程/每秒心跳路径上执行（macOS 主线程直查 CGEventSource 会死锁，详见 README 架构说明）
- 提交前请确保：`cargo test` 全绿、`npm run build` 通过

## 演示模式

```bash
PAUSE_DEMO=1 npm run tauri dev              # 自动演示完整提醒流程后退出
PAUSE_DEMO_SETTINGS=1 npm run tauri dev     # 打开设置窗数秒后退出
PAUSE_DEBUG=1                               # 输出诊断日志到 /tmp/pause_debug.log
```

## 版本与发布

- 遵循语义化版本，变更记录写入 `CHANGELOG.md`
- 推送 `v*` 标签后，GitHub Actions 会自动在 macOS / Windows 上跑测试并产出安装包发布到 Release

## 提交 Pull Request

1. Fork 并创建特性分支：`git checkout -b feature/my-feature`
2. 保证测试与构建通过
3. 提交 PR 并附上清晰的改动说明

## 许可证

提交即代表你同意代码以 [MIT](LICENSE) 协议发布。
