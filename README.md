<p align="center">
  <img src="assets/branding/plico.svg" width="80" height="80" alt="Plico logo">
</p>

# Plico

轻量的 Windows Git 客户端，以审阅代码为主，使用 Rust 和 GPUI 构建。

Plico 面向想快速看提交、同事的改动和 AI 生成代码的人：参考 JetBrains / Rebased 的交互，重点控制内存占用，同时提供日常需要的基础 Git 操作和简单编辑。

[下载预览版](https://github.com/lying200/plico/releases) · [反馈问题](https://github.com/lying200/plico/issues) · [构建状态](https://github.com/lying200/plico/actions)

> 当前处于 Preview 阶段。它不是完整 IDE，也不包含代码补全、项目索引或插件系统。

## 功能

- **提交审阅**：Git Log / Graph、分支与条件过滤、提交详情和版本比较。
- **查看差异**：单栏 / 双栏 Diff，独立 Tab 或底部预览，跳转到工作区源码。
- **文件浏览**：文件树、代码高亮、文件历史和选中范围历史。
- **基础操作**：Fetch、Pull、Push、Checkout、Commit / Amend、Stash，以及多选 Squash。
- **简单编辑**：工作区源码与 Commit Diff 的工作区内容可编辑，支持自动保存、撤销和重做。历史及暂存快照保持只读。
- **中英文界面**：默认跟随系统语言，也可在设置中切换。

## 开始使用

1. 安装 [Git for Windows](https://gitforwindows.org/)，确保 `git --version` 可用。
2. 在 [Releases](https://github.com/lying200/plico/releases) 下载 Windows x64 ZIP。
3. 解压**整个目录**，运行 `Plico.exe`，选择本地仓库。

也可以从 PowerShell 指定仓库：

```powershell
.\Plico.exe "C:\code\my-repository"
```

自动保存只写工作区文件，不会自动暂存或提交。请保留包内的辅助程序；字体和图标已内嵌，Git 需自行安装。

常用快捷键：`Ctrl+O` 打开仓库，`Ctrl+S` 保存，`Ctrl+F5` 刷新，`F4` 跳转源码，`F7` / `Shift+F7` 切换差异，`Ctrl+F4` 关闭文件 Tab。

更多操作说明见 [Windows 使用说明](docs/WINDOWS-PREVIEW.txt)。

## 预览版边界

- 当前提供 Windows x64 便携包，尚未代码签名，不含自动更新。
- 单栏编辑 Diff 的行号、差异底色及部分界面细节仍待完善。
- 低内存是设计目标；已有开发阶段测量不代表所有仓库、硬件或长时间使用场景的保证。

遇到问题请提交 [Issue](https://github.com/lying200/plico/issues)，附版本、复现步骤和必要截图；请勿包含仓库凭据或私有代码。

## 开发与构建

依赖 Rust、Windows SDK / MSVC 和 LLVM。GitHub Actions 使用 Windows runner 构建；也提供 Nix / devenv 配置，供 WSL 开发环境使用。

```sh
# 核心测试（不启动图形界面）
cargo test --locked --no-default-features

# Windows 原生构建
cargo build --locked --release --target x86_64-pc-windows-msvc --bins
```

WSL 中可使用 `devenv shell` 后运行 `cargo xwin build --locked --release --target x86_64-pc-windows-msvc --bins`。当前 WSL shader 编译需要宿主机 PowerShell。

主程序产物为 `target/x86_64-pc-windows-msvc/release/plico.exe`。打包脚本会连同辅助程序、许可证与校验信息生成便携包：

```sh
python scripts/package-windows.py dist/Plico-windows-x64.zip
```

[发布流程](docs/RELEASING.md) · [开发计划](PLAN.md) · [上游参考](docs/REBASED-REFERENCE.md) · [早期开发记录](docs/DEVELOPMENT-HISTORY.md)

## 许可证与致谢

Plico 原创源码采用 [Apache-2.0](LICENSE)。第三方代码、图标和字体保留各自许可证，见 [NOTICE](NOTICE) 和 [许可证说明](docs/LICENSING.md)。项目 logo 不包含在源码许可证授权内。

感谢 [Rebased](https://github.com/DetachHead/rebased)、[IntelliJ Platform](https://github.com/JetBrains/intellij-community) 以及 GPUI 生态。Plico 是独立项目，不代表这些项目的官方产品或获得其背书。
