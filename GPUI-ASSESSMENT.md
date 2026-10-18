# GPUI 选型评估

调研日期：2026-10-15。对象：Windows 只读 Git 审阅器，以内存和 IntelliJ Git Log / Diff 体验为核心。本文为资料分析，尚未执行 Windows 基准；不修改 PLAN.md 的技术决策。

## 结论

**GPUI 值得作为首选验证方案，但目前没有证据认定它是内存最低的方案。** 它的自绘能力适合复现 IntelliJ 的紧凑布局、graph 和双栏差异视图；目前可复用的 Diff 组件明显降低了实现成本。建议维持 Rust + GPUI 的候选地位，先用真实 Windows 原型检验现有 80 / 150 / 250 MiB 预算，再决定是否正式采用。Qt Widgets 是需要保留的对照候选。

## 已核实事实

- GPUI 是 Rust 自绘 UI 框架，不依赖浏览器或系统原生控件；Windows 后端使用 DirectX 11。布局、样式和按焦点上下文派发快捷键都有框架支持。[GPUI 官方示例](https://gpui.rs/examples/)
- Zed 于 2025-10-15 发布 Windows 稳定版，官方说明使用 DirectX 11 和 DirectWrite，且有持续维护 Windows 的团队。这能证明平台已用于实际产品，不能证明本项目的内存、中文和 DPI 测试已通过。[Zed Windows 公告](https://zed.dev/blog/zed-for-windows-is-here)
- gpui-component 现在属于 **GPUI Kit**；项目提供虚拟列表、表格、可调整面板及主题支持，并允许使用基础行为层自行实现视觉样式。软件源码采用 Apache-2.0；应以选定版本实际依赖及许可为准。[GPUI Kit 仓库](https://github.com/longbridge/gpui-kit)
- 0.7.3 发布说明包含新的只读 Diff 组件和 `examples/tig` Git 历史浏览器。这是最近新增的能力，不能把功能清单等同于经过本项目边界案例验证。[发布说明](https://github.com/longbridge/gpui-kit/releases)
- Diff 支持单栏和双栏、虚拟化、文本拖选与复制、折叠、变更跳转及行内强调。它接收应用提供的 patch，不负责读取 Git 或计算差异；不支持 combined diff，不能凭 patch 恢复未提供的上下文。快捷键可由应用配置。[Diff 官方文档](https://gpui-kit.com/component/diff/)

因此，“GPUI 只有基础绘图，整个双栏文本选择引擎都要自己写”已不是准确的当前判断。但 IntelliJ 的 graph、分支筛选、焦点切换、预览和展开状态、特定差异连接绘制等仍需应用自行实现及验证。

## 内存判断：架构优势，不是数字保证

以下属于工程推断：使用原生 Rust UI，有机会省掉 JVM 或 WebView 的运行时基线，且容易让 Git 数据读取、UI 状态和缓存共用同一进程中的数据。不过 Rust 解决的内存安全问题不等同于低内存占用；GPU 加速首先服务绘制速度，也不等同于节省 RAM。

在本次查阅的官方资料中，未发现同一台 Windows 机器、同一 Git 数据集、同一 UI 功能下 GPUI 与 Qt Widgets 的可比内存基准。因此不应承诺“GPUI 空载几十 MiB”或“比 Qt 省若干倍”，也不能用完整 Zed 与 IntelliJ IDEA 的占用差异来推导 UI 框架的净差异。

本项目应分别控制：历史元数据与 graph 轨道、当前文件与 patch 解析副本、排版和高亮缓存、后台 Git 输出、图形资源。列表虚拟化限制可见元素的绘制成本，并不会自动减少已经装载的全部 Git 数据。GPU 内存应另行报告；尤其在集显机器上，不能把转移到共享图形内存当作消除了系统内存成本。

建议每次只加载选中文件的 patch；只保留有限历史窗口和按字节计费的 diff 缓存；终止过期 Git 请求；限制并发和输出尺寸。初版不引入 LSP、全仓库索引、JS 扩展运行时。以上是本项目设计建议，不是框架默认保证。

## IntelliJ 还原度

**视觉还原：适合。** 自绘意味着布局、行高、颜色、边框、refs 标签、graph 曲线都可以由应用控制，不必迁就 Windows 默认控件外观。现成组件默认风格不等于 IntelliJ，仍需统一主题和尺寸；GPUI Kit README 明确其设计参考包含 shadcn/ui。[GPUI Kit](https://github.com/longbridge/gpui-kit)

**操作还原：可行，取决于实现。** 单击提交更新文件、单击文件预览、Ctrl+D 展开、Esc 返回、F7 跳转、上下键选择，以及返回后保留滚动位置，都是状态和焦点模型问题。框架能表达这些行为，但不会自动提供 IntelliJ 的操作语义。应先把这些行为写成验收用例，再实现。

**精细文本体验：优先实测。** 对双栏错位行、长行、中文与 emoji、跨虚拟行拖选、复制、折叠、左右侧选择边界、缩放和 DPI 进行验证。复用 Diff 可以减少基础工作；若对齐效果、连接区样式或选择规则不能达到目标，再决定局部扩展，而非预先重写整个组件。

## 与 Qt 的简要比较

| 候选 | 本项目的主要优势 | 本项目的主要代价 | 内存判断 |
| --- | --- | --- | --- |
| GPUI + GPUI Kit | Rust 同栈、自绘自由；已有只读 Diff 和虚拟化组件 | 当前 Diff 较新，需要 Windows 和行为验收；跟随依赖变更需谨慎 | 有合理低内存潜力，具体预算未验证 |
| Qt Widgets | 已有文本、树表、菜单等桌面控件；可用样式表及自绘调整外观 | 精确模仿 IntelliJ 需要定制；双栏 diff 语义仍需组合实现；Rust 栈还涉及桥接选择 | 值得作低基线对照，不能未测就宣布比 GPUI 小 |
| Qt Quick | 可声明式定制界面，GPU scene graph 适合复杂视觉 | 增加 QML 和 scene graph 层；当前需求未显示值得换栈的独有收益 | GPU 加速同样不能证明低内存，需要同场景测量 |

Qt 的 `QPlainTextEdit` 支持只读、复制、查找、语法高亮和面向大文本的优化；但这不是现成的 IntelliJ 双栏 diff。[Qt 文本控件](https://doc.qt.io/qt-6/qplaintextedit.html) Qt Widgets 可用样式表自定义外观。[Qt 样式表](https://doc.qt.io/qt-6/stylesheet.html) Qt Quick 使用独立 scene graph，并可在单独线程通过图形 API 渲染。[Qt Quick Scene Graph](https://doc.qt.io/qt-6/qtquick-visualcanvas-scenegraph.html)

## 决策实验

用锁定依赖的 Release 构建，复用 Diff 和虚拟列表，制作接近最终窗口尺寸及信息密度的 Windows 原型。依次测量：空窗口；一万条历史及 graph；选中一个不超过 1 MiB 的文本 diff；连续切换一百个文件；返回初始状态。

记录应用及 Git 子进程 Private Bytes 的稳定值与峰值，同时记录 Working Set、GPU 独立及共享内存。保留现有 PLAN.md 中 80 / 150 / 250 MiB 的预算作为待测门槛，记录机器、DPI、窗口尺寸、Git 版本和文件数据。不要只测 Hello World，也不要用 Debug 构建决定框架优劣。

若 GPUI 原型满足内存与核心交互，继续该路线。若仅启动和基本控件就占去大部分预算，或关键文本交互修复成本过高，再用 Qt Widgets 实现相同工作负载做对照。若增长来自 patch 多份复制和无界缓存，先修正数据生命周期，换框架未必有效。
