# 贡献指南

感谢你考虑为「休一下（Pause）」做贡献！

## 开发环境

- macOS 13+
- Xcode Command Line Tools（Swift 5.9+）
- 无第三方依赖，克隆后可直接构建

```bash
git clone <repo-url>
cd pause
swift build          # 开发构建
swift test           # 运行全部单元测试
./build.sh           # 组装 Release Pause.app
open build/Pause.app
```

## 项目约定

- **架构**：MVVM、单向数据流。View 只观察 ViewModel 并发送用户意图；业务逻辑放在 Service；不引入第三方依赖（保持零依赖）
- **语言**：代码注释与提交信息使用英文或中文均可，界面文案必须同时维护 `Localization.swift` 中的中英双语表
- **测试**：涉及状态机、设置、缓存等逻辑的改动请补充对应单元测试，保持 `swift test` 全绿
- **版本**：遵循语义化版本，变更记录写入 `CHANGELOG.md`

## 提交 Pull Request

1. Fork 并创建特性分支：`git checkout -b feature/my-feature`
2. 保证 `swift build` 与 `swift test` 通过
3. 提交 PR，描述清楚动机与实现方式

## 反馈问题

提 Issue 时请附上 macOS 版本、应用版本（设置 → 通用 → 版本）与复现步骤；如涉及崩溃请附崩溃报告。

## 行为准则

保持友善与尊重，讨论对事不对人。
