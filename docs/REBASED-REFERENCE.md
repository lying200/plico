# Rebased 源码对照入口

仓库：https://github.com/detachhead/rebased

本次已读取的参考 commit：`4aec35256d765c217aa0d662b82ac4e860c396fd`。这是源码快照，不表示与宿主机安装版本一致。现有 checkout `/tmp/gitui-rebased-reference` 只作缓存；可用 `git show <commit>:<path>` 读取 sparse checkout 外文件。

## 过滤与对齐

已读 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/MainFrame.java`：构造时调用 `myChangesBrowser.setToolbarHeightReferent(myToolbar)`，文件工具栏参照 Log 工具栏高度；工具栏和表格位于同一左右拆分结构。这支持共享几何约束，而非分别调两个固定高度。

已读 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/filter/VcsLogClassicFilterUi.kt`：分别维护 branch、user、date、structure、text、parent model；模型变化汇总 filter collection；`createActionGroup` 包括 Branch、User、Date、Paths 和 graph 控件。具体匹配及组合语义仍需继续读各 FilterModel 和 provider，不能直接从按钮名推断。

本地入口：`src/ui.rs` 的 `branches`、历史列表和文件面板；`src/git.rs`、`src/history.rs` 的快照与分页。现有 loaded-history 查找不能充当完整历史过滤。

## 已包含历史高亮

已读 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/highlighters/CurrentBranchHighlighter.java`：

- `getContainedInCurrentBranchCondition(root)` 判断提交是否被当前分支包含。
- 已选中行返回默认样式，由选中态覆盖高亮。
- 使用主题键 `VersionControl.Log.Commit.currentBranchBackground`；Java fallback 不是 Islands 最终主题颜色，需进一步查主题覆盖和截图。
- 当单独过滤 HEAD、过滤当前分支或无法取得当前分支时抑制该高亮。本项目用户要求为“当前所在 commit 已包含历史”，detached HEAD 的最终行为需明确记录，不能假定源码自动满足。

后续入口：`platform/vcs-log/impl/src/com/intellij/vcs/log/data/ContainingBranchesGetter.kt` 及它的 condition cache；Islands theme 中相应颜色键。当前仅定位到 getter 委托，未完成算法对照。

本地入口：`src/git.rs` 快照固定 HEAD、`src/graph.rs` 图模型、`src/ui.rs` 提交行背景。当前金色分支图标只是当前位置提示，不是可达历史高亮。

## Diff Tab 与底部预览

已读 MainFrame：根据 `withEditorDiffPreview` 为 changes browser 安装 `VcsLogEditorDiffPreview`，同时保留 `FrameDiffPreview` 的布局机制。

已读 `platform/diff-impl/src/com/intellij/openapi/vcs/changes/EditorTabDiffPreview.kt`：preview 有独立虚拟文件身份；`openPreview` 经 `DiffEditorTabFilesManager.showDiffFile` 打开；`performDiffAction` 触发预览，Esc handler 关闭预览；生命周期与 disposable 关联。这是编辑器 Tab 对象，不是仅放大内嵌面板。

已读 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/actions/ChangeDiffPreviewLocationActions.kt`：底部／右侧预览位置通过 UI properties 控制，仅在预览开启时显示对应操作。

后续入口（尚未完整读取）：`VcsLogEditorDiffPreview`、`DiffEditorTabFilesManagerImpl.kt`、changes browser 的双击及快捷键 action。当前源码证据不能单独证明已安装 Rebased 的默认打开方式、单击行为及 Tab 固定策略，实施前需原生操作验证。

本地入口：`src/ui.rs` 的 `load_file`、`diff_panel`、`expanded` 和主 render；配置持久化沿用用户应用数据目录。新增独立 Tab 生命周期，底部模式保留为设置选项。

## 记录要求

每阶段补充已读取源码路径与规则、原生截图、Rust 实现位置和验证结果。对照的是用户需要的交互和状态语义；为内存优化采用轻量状态或按需重载时，明确记录行为是否发生变化。具体任务和验收只维护在 PLAN.md 的 R1–R3，避免两份清单漂移。

修改 Branch／User／Date 过滤菜单或 Diff Tab 时，先查看[用户截图索引](../artifacts/rebased-user-reference/README.md)。四张原图保存于仓库，包含级联菜单、预设项及独立 Diff Tab；索引区分截图可见事实、已确认源码语义和仍待核对的行为。此前 Branch 平铺搜索弹窗与此结构不符，仍需替换，不能视为视觉验收通过。

## R1 单一查询入口（2026-10-15）

用户补充的 single-search.png 已加入上述索引。`src/ui.rs` 删除已加载历史专用 search 状态、输入框和 Find 按钮，移除 Filter 按钮；消息输入采用 prefix 搜索图标和 suffix 模式开关。Enter 提交，Ctrl+F 聚焦此输入；切仓清空输入及查询条件。Windows release 构建及原生启动通过，`artifacts/single-search` 确认单入口布局、输入 Normalize 并按 Enter 从六条过滤为一条。SendKeys 因前台保护拒绝后，验证改用限定专用进程 HWND 的字符与 Enter 消息。测试窗口已关闭。

已进一步读取同一参考 commit 的 VcsLogFilters.kt::fromHash／HashSeparatorCharFilter、VcsLogUtil.GIT_HASH_REGEX、VcsLogFiltererImpl.kt::filter／applyHashFilter：以空白、逗号或分号分割，所有项均为 7～64 位十六进制时构造 hash filter；有匹配哈希时优先显示，不受其他过滤器约束；无匹配时回到文本过滤。Registry 的消息包含哈希扩展路径还需继续核对。当前仅完成单输入 UI，哈希语义尚未接入，故暂保留 Filter messages 占位文案，不将其宣称为完整 Text or hash。聚焦边框目前仍是 GPUI 默认灰色，需改为参考图蓝色；Branch 菜单结构、输入尺寸及其余过滤器仍待还原。便携包尚未更新。

## R1 对齐实施记录（2026-10-15）

`src/ui.rs` 中分支搜索、Log 过滤栏、文件标题栏共用 `LOG_TOOLBAR_HEIGHT = 36`，文件标题增加 `flex_shrink_0`。此前文件栏为 30，导致首行提前 6 个逻辑像素。遵循上述 MainFrame 的同高参照规则；当前固定单行高度，过滤控件及窄窗溢出仍待后续实现。Windows release 构建通过，原生截图 `artifacts/toolbar-alignment/window.png` 已确认工具栏下沿和第一行对齐；同目录 reference.png 为仍运行的 Rebased 实际状态，未关闭用户进程。当前只验证默认窗口，不代表所有 DPI／窄窗通过。

## R1 文本过滤查询（2026-10-15）

已读同一参考 commit 下 `plugins/git4idea/backend/src/log/GitLogProvider.kt` 的 `appendTextFilterParameters`，以及 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/filter/TextFilterModel.kt`：消息过滤使用 `--grep=<value>`，明确选择 extended-regexp 或 fixed-strings，忽略大小写时添加 regexp-ignore-case；空白查询取消过滤。哈希模型与文本模型分别存在，不能以 grep 哈希替代哈希定位。

本地 `src/git.rs` 新增 `TextFilter` 和 `snapshot_filtered_at`，普通 `snapshot_at` 委托到无过滤路径。过滤作用于完整可达历史、先过滤再分页；保留原始 parent IDs，不冒充已完成过滤图。当前仅有消息文本查询数据层，尚未接入 UI；哈希、User／Date／Paths、隐藏祖先连接仍待实现。真实 Git 集成测试覆盖普通文本中正则符号、大小写、正则、错误正则、空白、参数外形文本、固定 roots 分页及隐藏父节点。Linux 核心 25 项通过，Windows 原生新增用例通过。现有便携包保持前一版工具栏对齐构建。

## R1 过滤图连接算法（2026-10-15）

已读同一参考 commit 的 `platform/vcs-log/graph/src/com/intellij/vcs/log/graph/impl/facade/FilteredController.kt` 和 `platform/vcs-log/graph/src/com/intellij/vcs/log/graph/collapsing/DottedFilterEdgesGenerator.kt`。FilteredController 建立可见节点集合，生成器分别 downWalk／upWalk，传播最近可见节点编号并生成 DOTTED 边；范围外连接另有 DOTTED_ARROW 处理。

`src/filtered_graph.rs` 翻译了完整有界 DAG 范围内的双向遍历，输出带 dotted 标记的边；真实可见父边保留。输入为子先父后的节点索引，原始 Commit.parents 不被修改。当前保留差异：仅接收完整图，无窗口外节点，缺失父节点拒绝输入；边界箭头、窗口装配、渲染接入尚未实现。相同端点的真实父边优先，避免重复实虚线。临时节点最多 100,000，输入和输出边均受调用方预算限制；这只是分配保护，不能作为大仓库过滤已完成的依据。后续必须给超出完整图上限的仓库提供有界分页装配，而非永久缩小产品历史范围。

Linux 和 Windows 原生均通过 4 个新测试：隐藏链、不相连提交、隐藏 merge 的双侧连接、非法／超预算输入，以及 40 组生成 DAG 的输出边真实祖先关系。当前算法尚未在 UI 使用，没有本次界面变化或新便携包。

## R1 虚线布局与绘制接线（2026-10-15）

已读 `platform/vcs-log/impl/src/com/intellij/vcs/log/paint/SimpleGraphCellPainter.kt` 的 paintLine／getDashLength：非普通边使用虚线，垂直线一行一个 dash/space，间隔 rowHeight/2−2，偏移半个 dash。`src/ui.rs::graph_cell` 将此规则适配到现有半行 PathBuilder 分段；GPUI 手工分段与 Java BasicStroke 的圆端点、斜线跨行相位尚未做像素核对。

`src/graph.rs` 的 Segment 与 lane frontier 携带 dotted，`append_projected` 接收独立投影父边，普通 Commit.parents 入口仍生成实线。同目标 lane 合流时真实父边的延续优先。新增测试覆盖跨无关行、状态克隆恢复和实虚线共用目标 lane。Linux 全量核心 30 项先通过，合流补充后 graph 相关 10 项通过；包含渲染改动的 Windows release 已编译，最后纯布局合流补充尚未重新构建 Windows。过滤图装配尚未调用此入口，故未宣称原生虚线视觉验证或过滤 UI 完成，便携包未更新。

## R1 过滤结果到布局装配（2026-10-15）

沿用已读 FilteredController 的“原图＋可见节点集合→折叠图”分离规则，`src/filtered_graph.rs::window` 将完整快照提交 ID 映射为索引，投影后逐行交给 `Layout::append_projected_bounded`。只保留所请求窗口的行；重建前置行时释放旧几何，当前窗口最多 HISTORY_MAX，另有边、512 lane 和几何预算。返回源提交索引而非改写 Commit，确保 Diff 的父提交不随可视边改变。拒绝重复 ID、跨快照匹配、缺失父历史，未将不完整图伪装为完整图。

真实 Git 测试 `filtered_graph_windows_keep_real_diff_base_and_project_hidden_parent` 在 Linux／Windows 通过：三提交隐藏中间项后输出虚线，两个单行页与整页几何一致，Diff 仍包含中间版本的删除行。覆盖未知匹配、缺父和几何预算失败。此步骤可处理完整有界输入的输出分页；大仓库输入流式读取、浅边界箭头、取消和 UI 接入仍未完成。便携包保持上次工具栏对齐版本。

## R1 轻量图读取（2026-10-15）

进一步读取同一 GitLogProvider.kt 的 `readAllHashes`／`readFullDetails`：图历史与详细提交信息分开读取，图使用 date-order。`src/git.rs::topology_page` 采用同样的数据分层，仅读取 `%H %P`，固定已解析的 OID roots，每页最多 1,000 条、额外一条检测 more；复用已有子进程取消／超时／输出限额，解析 ID／父 ID 时检查取消。读取顺序与当前 snapshot 的 date-order 一致。

`TopologyCommit` 不含作者、标题、refs；`filtered_graph::GraphNode` 使装配函数直接接受轻量节点，无需转换成完整 Commit。Linux／Windows 新增真实 Git 测试通过：包含 merge 的两条一页读取等于原快照；读取前 refs 前移仍保持固定 roots；轻量和完整元数据产生相同图；非法 roots、空 roots、结尾、提前取消明确处理。未测量实际内存收益。当前仍为 skip 分页，完整图装配仍有总节点上限，尚非无界仓库的流式投影；图 CPU 遍历取消仍待补齐，UI 未接入。

## R1 文本过滤纵向流程（2026-10-15）

`history::read_filtered` 将固定 roots 的过滤快照、轻量拓扑及匹配 ID 查询与投影装配连接；`ui.rs` 增加消息过滤输入和 Filter 按钮，回车事件也提交条件。清空后提交恢复普通历史；原“已加载历史查找”暂时独立保留。遵循前述 TextFilterModel／GitLogProvider 的消息语义，当前 UI 固定普通文本、忽略大小写，正则／大小写开关及统一文本哈希字段待接入。查询失败清除旧过滤列表，避免旧结果冒充新结果。

Windows release 原生截图 `artifacts/message-filter-ui`：filtered-button.png 确认三条变两条，跨隐藏父节点为虚线；Diff 左侧仍为 hidden middle；cleared.png 恢复三条；empty-second-enter.png 显示无结果说明。输入文本后的首次 Enter 在本次自动输入中未立即提交，第二次 Enter 才显示结果；可能与输入法组合态有关，尚未归因，不能声称所有键盘路径已通过。Filter 按钮已验证。测试窗口已关闭，用户 Rebased 保留。

Linux 核心 18 单元＋15 集成测试通过，包含 read_filtered 的真实 Git 分页／父基准用例。已更新便携包。临时总图上限 100,000 节点／500,000 边，超限显式报错；skip 分页和重复查询性能、图 CPU 内部取消、斜线虚线视觉、窄窗以及浅边界箭头仍待完善。大仓库和完整过滤工具栏均未验收完成。

## R1 正则与大小写开关（2026-10-15）

依据已读 TextFilterModel 的 TEXT_FILTER_REGEX／TEXT_FILTER_MATCH_CASE 变化重新触发过滤，以及 VcsLogClassicFilterUi 的 createTextActionsToolbar，`ui.rs` 增加 `.*`／`Cc` 按钮、选中态与立即重新查询。模式状态独立于非空查询保存，空查询仍取消过滤。临时差异：按钮放在输入框右侧而非输入框内部，模式未持久化，整体仍需与统一文本／哈希入口及窄窗布局一并收敛。

Windows release 构建通过；`artifacts/filter-options` 原生截图确认 `Match` 默认匹配 Match root＋match tip，Cc 后只留 Match root；`[Mm]atch` 正则重新匹配两条；`[Mm]atch[` 显示 Invalid regular expression，关闭正则后恢复普通文本无结果状态。第一次 SendKeys 因目标不在前台被脚本拒绝，后续使用仅发往专用测试 HWND 的字符／按键消息，未向其他应用发送文本。已关闭测试进程并更新便携包。数据层本次未修改，沿用此前已通过的真实 Git 正则／大小写测试。

## R1 哈希查询接入（2026-10-15）

沿用上文 TextFilterModel／VcsLogFilters／VcsLogFiltererImpl 的参考 commit，新增 `TextFilter::hashes` 和 `history::read_hashes`。7～64 位十六进制项以空白、逗号或分号分割，统一小写；在固定全仓库 roots 的轻量拓扑中匹配前缀，命中时忽略分支／消息条件，未命中回退消息过滤。保留真实父关系，通过现有投影图显示匹配结果。`Repository::snapshot_commits` 以每批 64 个已验证完整 OID 读取所需元数据，恢复图顺序并保留 refs／浅边界元数据，避免 Windows 命令行过长。UI 入口文案现为 Text or hash。

Linux 原有 33 项核心测试通过；新增真实 Git 集成用例在 Linux／Windows 均通过，覆盖大小写短哈希、跨分支匹配、多个哈希和去重、分页、未命中消息回退、非法文本识别和取消。Windows release 原生截图 `artifacts/unified-query/hash.png` 确认 main 仍选中时输入 272274d 找到 feature/ai-review 的提交并显示真实父版本 Diff；专用测试进程已关闭。

保留差异：仍需完整且有界的拓扑，临时 100,000 节点／500,000 边上限与浅仓库投影限制未解除；未实现 Rebased Registry 开启后的“更多结果包含消息中的哈希”扩展阶段，当前实现 hash-first 首阶段与无匹配回退。Registry 默认值尚未确认（远端按需取对象失败），不宣称完整等价。无持久索引；图 CPU 内部取消与大仓库内存／性能仍待验证。蓝色焦点边框及 Branch 级联菜单仍待还原。

## R1 分支级联结构（2026-10-15）

进一步读取同一参考 commit 的 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/filter/BranchPopupBuilder.java`：构建 singleton／recent／favorite 项，分隔线后为分组子菜单；结合用户 branch-menu.png 确认 Local 和远端入口的级联结构。

`src/ui.rs::branch_menu` 已替换旧固定 360px 高度平铺搜索弹窗，使用 GPUI DropdownMenu／PopupMenu 子菜单；本地和各远端独立分组，完整 ref 传入共用 select_branch，保留 All branches 清除入口。触发器箭头移到文字右侧，菜单背景采用现有 Islands shell 色、选择采用蓝色。删除旧弹窗搜索实体与订阅。Windows release 原生截图 `artifacts/branch-cascade` 确认 Local 子菜单展开，选择 main 后两级菜单关闭、左侧 main 同步选中、历史从六条变五条。

这只是级联结构接入：Select… 多选、HEAD、Favorites／星标、recent、speed search、自然排序仍待补齐；菜单文字目前使用组件 text_sm，比参考偏小，圆角、边框与图标留白尚需精确校准。远端子菜单和键盘导航仍需单独验证；大分支数下当前构建所有菜单项的内存成本待优化。整体不作为像素级验收通过，便携包仍为上一轮统一查询构建。

## R1 HEAD 过滤入口（2026-10-15）

已读同一参考 commit 的 `plugins/git4idea/backend/src/log/GitRefManager.kt::groupForBranchFilter`：HEAD 作为 SingletonRefGroup 放在 Local／remote 分组之前。`Snapshot` 新增 head_id，复用现有一次 HEAD 解析；分支过滤值 HEAD 使用该完整 OID 为根，固定 roots 的分页优先于实时 HEAD。未诞生 HEAD 返回空历史，UI 仅在已有 HEAD 时显示入口；不执行 checkout 或修改 refs。

`ui.rs::branch_menu` 已接入 HEAD，Windows release 原生菜单显示与点击关闭截图在 `artifacts/head-filter`。Linux 18 单元＋17 集成测试通过；新增 head_filter_tracks_detached_head_and_pins_paging_roots 在 Windows 宿主机 Git 同样通过，覆盖未诞生 HEAD、新提交后原快照分页、detached 历史范围及读取不移动 HEAD。该入口并不等于 R2 当前 HEAD 可达性背景已实现。菜单收藏星标及其余视觉差异仍保留，便携包尚未更新。

## R2 当前 HEAD 已包含背景（2026-10-15）

再次核对同一参考 commit 的 CurrentBranchHighlighter.java：包含关系决定背景，选中行优先；源码在 HEAD／当前分支单分支过滤和无当前分支时抑制。按用户要求以当前 HEAD 已包含为语义，本项目明确保留这些过滤状态及 detached HEAD 的背景，不以是否有分支名改变祖先判断。颜色 #1D2336 来自用户 single-search.png 的背景像素 (1100,200)，尚未确认该安装版本主题资源值。

`git.rs::contained_in_head` 固定已解析 HEAD OID，每次读取最多 1,000 个轻量拓扑节点，只保存当前提交窗口的匹配 ID，找到全部候选即结束；不为每行启动 Git，也不常驻整图。Snapshot.included_in_head 以 Arc 共享给 ui.rs，选中行颜色后应用。匹配包含所有父路径，浅仓库仅覆盖本地已知历史。

Linux 18 单元＋18 集成测试通过；新增 head_membership_includes_merged_sides_but_not_unmerged_branches 在 Windows 同样通过，覆盖已合并侧枝、未合并分支、浏览其他分支不改变 HEAD、detached 和窗口内集合上限。Windows release 原生 `artifacts/head-highlight/window.png` 确认六提交样例中五条非选中行深蓝、选中行仍为选择蓝。

限制：存在不在 HEAD 内的候选时会遍历 HEAD 全部历史，目前 skip 分页会重复 Git 遍历；读取在后台但可能增加加载延迟，尚需大仓库性能优化和内存复测。浅仓库及并发 HEAD 刷新仍需完整验收；当前属于 R2 初步接入，不勾选整体通过。

## R3 独立 Diff 目标（2026-10-15）

再次读取同一参考 commit 的 EditorTabDiffPreview.kt：previewFile 独立身份，通过 showDiffFile 打开；hasContent 为打开前提；Escape 关闭预览；虚拟文件关联 disposable。当前先落实查询对象身份，不声称已复制其 Tab 复用策略。

新增 `src/diff_target.rs`，Target 捕获仓库、commit 和 Change，Identity 包含仓库路径、提交 ID、父版本、area、新旧路径；不持有文件内容。现有 ui.rs::load_file 已通过 Target 读取 Diff，保留原取消及 generation 检查。提交目标与本地区域不匹配时拒绝创建；本地目标仍是可刷新工作区／index 视图，不伪装成不可变文件快照。

真实 Git 用例 captured_diff_target_keeps_revision_after_head_moves 在 Linux／Windows 通过：HEAD 前进后旧目标仍读原版本，新提交目标身份不同，staged／unstaged 身份不同。Windows release 编译通过。本次无可见布局变化，未更新便携包；标题目前仍依赖 Log 选择，独立 Tab 实体、活动 viewer 生命周期、关闭／返回和持久设置仍需接入。

## R3 Tab 接线与完整上下文（2026-10-15，进行中）

`src/diff_tab.rs` 新增独立目标／viewer／取消令牌及 generation，非活动时释放 viewer，重新进入按目标读取；Review 保留多 Tab 身份，同目标复用，支持关闭、Esc、文件双击和 Ctrl+D／文件列表 Enter。默认 Log 不显示底部 Diff，设置页面可保存底部模式至 diff-location。原生首次截图位于宿主机验证目录 diff-tabs/output，确认默认 Log 占满内容区；Tab 打开／返回、多 Tab、设置持久化和竞态尚未完成原生验收，不能以此宣称 R3 完成。

用户指出 Tab 应显示完整文件后，已读参考 commit 的 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/actions/diff/ChangeDiffRequestProducer.java`：分别 createContent 读取 before／after 完整内容，再构建 SimpleDiffRequest；ByteBackedContentRevision 或 getContent 提供全文。

本地 Tab 改用 Target::read_full → Repository::full_patch，以完整 unified 上下文提供两侧文本，同时 DiffState.with_context_lines(None) 关闭默认未修改区域折叠；底部预览保留原 20 行上下文。本地实现仍经 Git patch 传输，未等价于完整 revision content 模型：纯重命名／仅权限变化的零 hunk 文件仍需补全文，二进制与输出限额仍明确处理。Linux 真实 Git 300 行、中间一行变化用例确认输出从首行到末行，紧凑模式不含首尾。Windows release 编译通过，最新完整上下文的原生 Tab 尚待验证，便携包未更新。

### 原生 Tab 与设置验证（2026-10-15）

`artifacts/diff-tabs` 已记录原生流程：选择 cache.rs 后 Enter 打开独立 Tab，完整显示样例文件 1～20 行，无默认上下文折叠；返回 Log，改选 merge 提交，再激活原 Tab 仍显示 272274d7 的原内容；关闭 Tab 后回到 merge 选择。设置切换底部预览后布局恢复，关闭程序重启仍为底部模式；测试结束通过 UI 恢复 tab 偏好并关闭专用窗口。设置按钮宽度从铺满改为 260 逻辑像素。刷新入口已路由到活动 Diff Tab 自身重新加载，Log 状态单独保留。

尚未验收：多 Tab 溢出、快速关闭／切换竞态、快捷键完整组合、Diff 视口恢复及内存压力。当前 Tab 导航工具栏、两侧版本标题、文件切换、Islands Tab 外形和设置页面仍需还原；首尾超过一屏的大文件仅有数据层测试，滚动交互仍由用户手测。上述截图验证不替代这些完整验收。

R3 验证补充（2026-10-15）：Linux 核心 18 单元＋20 集成测试通过；Windows 宿主机 Git 的 300 行完整上下文用例通过，确认中间一行改动时仍包含首尾。活动 Tab 刷新路由通过 Windows release 编译。便携预览包已更新并校验 ZIP CRC 与程序 SHA-256（471e86a682b2eb01c8d7fa8c6c1d6002413d9261d265050b4bb7444cedbcfc0a）。纯重命名／仅权限变化的全文、Tab 视觉和生命周期压力测试仍待补齐。

## R3 无文本变更片段的全文（2026-10-15）

重新读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `ChangeDiffRequestProducer.java::createContent`，确认读取 revision 全文不依赖变更片段。`git.rs::patch_context` 对 100% 相似重命名及仅普通文件权限变化的无 hunk 输出，使用 `cat-file blob` 读取提交版本或 index，补为两侧未修改上下文；保留 metadata，处理 UTF-8 路径、缺少末尾换行、二进制拒绝、取消和输出限额。底部紧凑模式不变。

Linux 18 单元＋21 集成测试通过；Windows 两项真实 Git 测试通过，覆盖 staged／committed chmod、中文重命名、缺少末尾换行和提交后工作区改动不泄漏。Windows release 原生截图 `artifacts/diff-tabs/rename-full-file.png` 确认 before.txt → after.txt 两侧显示全部两行，增删数 0，正确保留无末尾换行提示。仅关闭本轮测试窗口。完整视图的 @@ 标题和多余文件头、版本标题、Tab／工具栏像素差异仍待处理；此项不代表 R3 整体完成。

本轮预览包已更新，ZIP CRC 和 release 程序一致性校验通过；程序 SHA-256：`2b9272e5ed236d8fb61c4e51e2d4a55d60319f26802349d5b552d55f8bdc978a`。

## R3 版本标题及单侧布局（2026-10-15）

已读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 ChangeDiffRequestProducer.java（前后 revision 独立标题）、TwosideTextDiffViewer.java::createTitles、OnesideTextDiffViewer.java::createTitle 和 OnesideDiffViewer.java::canShowRequest：空侧采用单侧 viewer，标题选择有内容的一侧。

`src/diff_tab.rs` 为左右栏提供独立版本／路径标题，merge 标注第一父提交，本地区分 HEAD／Index／Working tree；新增／删除使用单栏模式。关闭重复文件头，以 LineInfo 分隔策略隐藏完整上下文中零缺失行的 @@ 标题；单独保留 old/new mode 信息。上下差异按钮复用已归档 JetBrains SVG。

Windows release 编译通过，原生 `artifacts/diff-tabs/titles-split.png` 确认纯重命名双栏标题／正文对齐，`titles-added.png` 确认根提交新增文件单栏。未单独验证删除和仅权限变化的新布局。与用户 diff-tab.png 相比，单栏仍有 + 标记，字体／行号留白、绿底色、右侧变化条、锁图标、Tab 外形和工具栏文件导航待还原；当前截图不同数据及窗口条件，不能作为像素级验收通过。未重复滚动测试，已关闭本轮专用窗口。

本轮标题布局预览包 CRC／release 一致性通过；程序 SHA-256：`cb60267fad2aee1c033dc90229209a1e1187a047a74ba80ac3a047f92b2379e5`。

## R3 文件列表导航（2026-10-16）

已读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/impl/CacheDiffRequestChainProcessor.java`：当前 index 选择 request provider，前后导航受列表边界约束，切换更新 request；另有 GoToChangePopupBuilder 文件选择入口。

`diff_target::Chain` 保存一份 commit／repository 和 Arc 文件元数据列表；导航生成当前 Target，不加载其他文件内容。当前按选中文件 area 分组，避免混淆 staged／unstaged 版本；独立打开的多个 Tab 尚各自复制文件列表，跨 Tab 共享与总体规模仍需优化。`DiffTab::move_file` 取消旧请求、释放 viewer 后按需加载新文件，Review 观察 Tab 更新标题。工具栏接入左右 JetBrains SVG、文件序号和边界禁用状态，资源许可来源已记录。

Linux／Windows 真实 Git diff_chain_keeps_revision_order_and_stops_at_boundaries 通过：固定提交、HEAD 后续前进不影响目标、顺序／边界及返回原目标。Windows release 原生 `artifacts/diff-tabs/chain-{first,second,previous}.png` 确认 a→b→a、序号及 Tab 标题更新；中间回 Log 选择 Base 再回 Tab，仍浏览原 b63e0066。仅关闭专用测试窗口。

仍缺少点击序号打开文件选择弹窗、导航快捷键、跨文件差异跳转及精确尺寸；当前蓝色序号为静态状态文字。多 Tab 大文件列表内存与快速切换竞态未验收，不据此勾选整体 R3。用户截图和当前样例数据／窗口条件不同，尚未达到像素级验收。

文件导航预览包 CRC／release 一致性通过；程序 SHA-256：`8aaa12316f1c48709b0dd9a98b705a7b860603ceb8125779c87babac8e673688`。

## R3 跨 Tab 元数据共享（2026-10-16）

再次读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 CacheDiffRequestChainProcessor.java：chain 与 processor 的 index 分离。这里据此保持独立游标，并按项目低内存目标实施共享；不声称 Rebased 内部使用相同缓存机制。

`diff_target::Chain` 改为 Arc<ChainData>＋独立 index，ChainData 包含 repository、commit 和文件列表。`fork_for` 仅对同仓库／同提交／同父版本／同 shallow 状态和匹配文件身份共享；拒绝工作区／index 的可变列表。`Review::open_diff_tab` 优先复用已打开 Tab 的 chain。活动 Target 仍保留少量自身 metadata；无全局常驻缓存，最后一个持有者释放全部 chain 数据。

Linux 20 单元＋22 集成通过；Windows 两项新增测试通过：10,000 文件／100 个 chain 游标引用相同分配，游标独立、最后释放后 Weak 无法升级；不同仓库／提交和本地列表拒绝共享。该测试证明结构和生命周期，不是 100 个原生 Tab 的工作集压力实测。Windows release 编译通过，无可见 UI 改动，未重复原生截图或滚动测试。总体 Tab 数限制、跨提交压力和大文件活动 viewer 内存仍待验收。

预览包已更新并通过 ZIP CRC／release 一致性验证；程序 SHA-256：`7d3b90abfb0675a88ddae3f0bcbf89c8d6e26e31720d80f5d11f03190516c8b5`。

## R3 文件选择弹窗初版（2026-10-16）

已读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GoToChangePopupBuilder.kt、ChangeDiffRequestChain.java、PresentableGoToChangePopupAction.kt 和 GoToChangePopupUtil.kt：通用链提供 speed search 列表，Git change chain 覆盖为树形单选，预选当前文件；序号链接在多文件时可用，鼠标触发显示在控件下方，选择后先关闭弹窗再导航。Git 树 onDoubleClick 确认文件。

新增 `src/diff_picker.rs`，以共享 Chain 元数据和已有 file_tree 构建目录行，GPUI List 虚拟渲染，支持搜索、目录展开收起、当前项预选及关闭。DiffTab 仅打开弹窗时创建 ListState，关闭／切文件／离开 Tab 时释放；Chain::select 按原始索引生成目标，不读取额外文件。当前搜索为大小写不敏感路径包含匹配。

Windows release 编译和原生 `artifacts/diff-tabs/picker-{opened,search,chosen,escape}.png` 验证：当前 b.txt 预选，搜索 c.txt，选择后切换到 3/3 的 src/c.txt 完整内容；重新打开后 Esc 仅关闭弹窗，保留 Tab。只关闭测试进程。

待对齐：当前 GPUI 列表单击确认，源码 Git 树双击确认；目录图标暂为文字、无文件类型图标／状态色；序号应恢复蓝色链接，搜索控件、树分组工具栏、尺寸及键盘左右展开仍需原生参照；搜索后首选目录，需改善定位。当前 Esc 焦点显示回到序号触发器，尚未完整验证焦点组合。未做大文件列表 CPU／内存压力或滚动自动化；此项不算 R3／像素验收完成。

文件选择弹窗预览包 CRC／release 一致性通过；程序 SHA-256：`2ca4053569d9a8bc3357ec90fa27dc919978ed585d0d0f1e1b040acc1498346b`。

## R3 文件选择交互／图标（2026-10-16）

再次读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GoToChangePopupUtil.kt::onDoubleClick／GenericChangesBrowserNode::render：双击确认文件，文件状态颜色、文件类型图标及目录计数。`diff_picker.rs` 改为单击选中、双击或 Enter 确认；独立 chevron 点击展开收起目录，复用 ui.rs 的文件类型资源与 Islands 状态色，文件序号改为 link variant。

为避免改变其他 GPUI 列表，vendor/gpui-component ListDelegate 新增默认 true 的 confirm_on_single_click，仅 FilePicker 返回 false；List 点击先同步 selection，再按策略确认，键盘确认不变。Windows release 编译通过。原生 `artifacts/diff-tabs/picker-style-{opened,single,enter,collapsed,double}.png` 验证单击留在原 b.txt Tab，Enter 打开 c.txt；目录收起／展开；双击重新打开 b.txt。仅关闭自己的测试窗口。

待完成：link variant 当前主题仍渲染白色及悬停下划线，需对齐 Rebased 蓝色；收起包含选中项的目录后应明确回选目录并校准键盘状态；搜索后首选项、左右键、分组工具栏、弹窗尺寸、颜色／字体和同条件原生参照仍待验收。本轮未重新做大规模内存或滚动测试。

文件选择器交互预览包 CRC／release 一致性通过；程序 SHA-256：`abc6f973393f5f30343a1850795e68397e928df2b2c840e0c56ecd1f065a50ea`。

## R3 文件选择器选中状态（2026-10-16）

重新核对参考 GoToChangePopupUtil.kt 的单选／初始文件索引／节点确认路径。本地修正 row index 在目录折叠后错位的问题：点击 chevron 重建后按目录路径定位并选中目录；搜索完成在 GPUI List 默认重置之后选中第一个匹配文件，空查询优先当前文件，异步回调检查 query 防止旧查询覆盖。搜索首选文件为本地交互决策，尚未等价验证 JetBrains speed search 的完整算法。文件序号子元素显式使用蓝色。

Windows release 编译通过，原生 `artifacts/diff-tabs/picker-selection-{collapsed,enter-folder,search-file,enter-file}.png` 确认收起后目录选中、Enter 只展开；搜索 c.txt 选中文件、Enter 打开 3/3 正确内容。蓝色序号显示正确。首次在树获得焦点后直接输入字符没有进入搜索，随后显式聚焦输入框验证通过；树 speed search 和左右键仍待实现。未测试空查询恢复全部组合，也未做大规模搜索压力，整体 UI／R3 保持进行中。测试窗口已关闭。

文件选择状态预览包 CRC／release 一致性通过；程序 SHA-256：`4c311ba31f496181d9853d28f9a2d399b83be80a11a25c625939d737cc2b021c`。

## R1 Date 查询与预设（2026-10-16）

已读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 DateFilterPopupComponent.java（Calendar 当前日期减 1／7 日、Select…、选中范围文案）、VcsLogDateFilterImpl.java，以及 GitLogProvider.kt 的 DATE_FILTER 参数生成：after／before 交给 Git。将原 TextFilter 命名扩展为 `LogFilter`，增加固定 Unix 秒上下界，metadata 和 topology_filtered_page 共用参数；文本与日期组合，已有哈希匹配优先逻辑保留。UI Date 菜单接 Last 24 hours／Last 7 days 和清除，切仓清除日期，分页保留已捕获边界。

Linux 20 单元＋23 集成测试通过；Windows 新增 date_filter_uses_commit_time_combines_text_and_keeps_paging 通过，覆盖作者时间相同但提交时间不同、上下界、文本组合、分页、哈希优先及 HEAD 不变。Windows release 原生 `artifacts/date-filter` 验证最近一天 1 条、一周 2 条、清除恢复 3 条；工具栏和文件首行仍对齐。仅关闭本轮测试窗口。

差异／待办：当前预设在点击时减固定 86400 秒，而参考为打开菜单时 Calendar 日运算，夏令时地区可能不同；Select… 自定义日期 UI 尚未接入（后端 before 已支持），暂使用 All dates 清除；选中日期仅 tooltip／selected 态，不是参考 Since／Until 文案；User／Paths 未完成。日期过滤沿用完整临时图投影上限和性能限制，不能称为任意大仓库验收通过。字体／菜单几何仍需同条件对比。

Date 预览包 CRC／release 一致性通过；程序 SHA-256：`21df1946a3f6872f6a5386cb916c8254ec9dee051e951db977297706987755fd`。

## R1 Date 日历与当前条件文案（2026-10-16）

重新读取同一参考 commit 的 DateFilterPopupComponent.java：createActionGroup 在打开菜单时捕获日期，先减一天再减六天；getText 按 after／before 显示 Since／Until／范围。新增 `src/date_filter.rs`，以 chrono 本地日历运算生成菜单固定时间戳；`ui.rs` 保存固定 after，按钮显示 Since YYYY-MM-DD，清除恢复 Date。chrono-tz 仅为测试依赖，不进入发布程序。

Linux 22 单元＋23 集成通过；Windows 两项新增日期测试通过，覆盖纽约春秋切换的 23／25 小时日历日。Windows release 编译通过；原生 `artifacts/date-calendar/day.png` 验证 Since 日期和 1 条结果，clear.png 验证清除恢复 Date／3 条。只关闭本轮专用进程 12496，没有滚动自动化。

仍有差异：目标日期落在 DST 重复或缺失时刻时，当前明确报错并保留旧条件，没有实现 Java Calendar 的消歧规则；任一预设无法计算时本次菜单两个预设均报错。文案暂用 ISO 日期，尚非 DateFormatUtil 的系统地区格式。Select… 自定义范围、Until／双边范围 UI、User／Paths 仍待完成；同条件像素验收尚未通过。

预览包 CRC／release 一致性通过；程序 SHA-256：`25bc10672fa1669c2ad98fc55ba35911c21584458d29e12ef0b2d98bf919838f`。


## R1 自定义日期范围（2026-10-16）

已读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 DateFilterPopupComponent.java::SelectAction、platform/vcs-impl/src/com/intellij/openapi/vcs/versionBrowser/DateFilterComponent.java 及 .form，以及 VcsLogBundle／VcsBundle 文案：Select Period 弹窗同一行 After／Before 复选框与日期选择器；按现有条件初始化、仅 OK 应用；双方均未启用时不替换现有条件。

新增 src/date_dialog.rs 保存独立草稿，复选框控制日期控件可用性，日历按需显示；确认转换本地时间戳，错误留在弹窗；取消不改变 Log。src/date_filter.rs::Bounds 支持单边／双边范围和 Since／Until／范围文案，ui.rs 将上下界交给原有 LogFilter 查询。现有时间部分在打开及改日期时保留；Java Microba 日期控件改日后的具体时间归一化尚未实测一致性。

Linux 22 单元＋23 集成通过，Windows release 编译通过。原生 artifacts/date-period/calendar.png／applied.png 确认 Before 日历选 2026-10-14、OK 后 Until 文案与 2 条结果；reopened／draft-change／cancelled 确认当前值初始化、修改后取消保留原条件；final-dialog.png 为紧凑右下角按钮修正后的重新启动截图。双边范围 UI 和键盘组合尚未逐项验收。只关闭本轮自启进程 23144／31972，用户 Rebased 1168 保留；对 Rebased 顶层窗口发送日期点击没有打开菜单，因此归档的 rebased-menu.png 不能作为菜单／弹窗参考。

差异待办：GPUI 日历外观、字体、主按钮色、弹窗位置和尺寸、系统地区格式、DST 消歧、日期键盘编辑；菜单目前另有 All dates 清除。尚未完成同条件像素验收，也没有新内存压力结论。预览包 CRC／release 一致性通过，exe SHA-256 `82f14269c8afd7272176f4cd87106018799a140e0c4e8ae06a79ea943199d1fb`。


## R1 原生日期弹窗对照与输入（2026-10-16）

以已读 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 DateFilterComponent.java／.form 和 DateFilterPopupComponent.java 为行为依据，实际操作用户已打开的 Rebased 1168，取得 `artifacts/rebased-user-reference/date-period/{menu,dialog,dialog-enabled,calendar}.png`。GetDpiForWindow=144；主窗口截图 2100×1500。实际弹窗居中、只有蓝色 OK（取消用关闭／Esc）、After／Before 各有独立文本框及日历按钮；原生日历是 Microba 风格，中文星期、周一首列、前后年／月按钮及 today／none。已取消参考弹窗，未应用其草稿，保留用户进程。安装版与参考 checkout 的精确对应版本仍未知。

本地 `ui.rs` 调整弹窗宽 514、最小高 132 logical px，随窗口居中，移除额外 Cancel，主题 primary／button_primary／ring 对齐蓝色；`date_dialog.rs` 增加独立可输入 YYYY/MM/DD 文本框和日历双向同步，确认时严格检查有效日期，错误不提交。订阅随对话框释放。vendor DatePicker 增加默认 false 的 icon_only，仅此处启用。实测发现输入框留白点击只聚焦框架，已在日期字段外层转交实际文本焦点；没有改变其他输入框事件行为。

Windows release 编译通过。`artifacts/date-native`：final-dialog 显示居中单 OK／独立输入控件；invalid-confirmed 显示无效日期错误且保留旧过滤；最终构建 valid-applied 验证点击字段右侧留白后可输入 2026/10/14，OK 得 Until／2 条结果；calendar-synced 预选输入日期 14；escape-calendar 仅关闭日历，escape-dialog 再关闭弹窗并保留条件。早期 invalid／typed-target 等截图属于未送入文字的测试，不是错误校验成功证据。全局 SendKeys 曾因非前台拒绝，未绕过键盘保护；真实鼠标激活仅在指针所在窗口 PID 校验匹配时进行。测试窗口 9516／32236 均已关闭。

剩余：Microba 日历结构／星期语言／年导航／today-none、日历图标、独立原生标题栏及应用图标、字段底色／字体／焦点边框细节、地区日期格式、DST 消歧；当前不同主窗口尺寸，未通过完整像素验收。输入法可能把 slash 转为标点，当前要求明确格式并提示错误；日历回填流程在本次新文本组件上仍需补一轮完整选择确认。没有新内存压力验收。包 CRC／release 一致，exe SHA-256 `cecfbc4d34ffb1da8dc18a1ba22ccc9b35e1e7b4dc67fe81a39abdab6ae41e41`。


## R1 User 作者查询层（2026-10-16）

已读参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`：UserFilterPopupComponent.java（Select…／me／近期项与 speed search）、VcsLogUserFilterImpl.kt、VcsLogUserResolver.kt、platform/vcs-impl/shared/src/com/intellij/vcs/log/util/VcsUserUtil.java、UserNameRegex.java，以及 plugins/git4idea/backend/src/log/GitLogProvider.kt 的 USER_FILTER 参数生成、GitUserRegistry.java 的配置读取。规则：UI 名称先解析为原始作者身份；me 根据 user.name／email 和历史同名身份解析，并排除仅邮箱前缀相同但无关联邮箱的其他人；Git 查询禁用 mailmap，多个作者取并集，与日期和文本条件组合。regexp/case 选项与文本共享，未设置文本条件时默认扩展正则且不区分大小写。

`src/git.rs` 增加 Author 和 LogFilter.authors；None 无过滤，Some(empty) 为无身份匹配，不允许退化为全部历史。metadata／topology 共用参数；正则模式转义并锚定完整 name <email>，固定字符串模式保留原文。两类 log 查询显式 -c log.mailmap=false。UI 暂传 None，尚未增加 User 菜单；本轮为查询层实现，不宣称 User 功能完成。

新增真实 Git author_filter_uses_raw_identities_and_composes_before_paging：特殊正则／反斜杠姓名、前缀姓名区分、多作者 OR、文本大小写和 regexp 组合、日期、分页和真实 parent／投影图、空身份、哈希优先、启用 .mailmap 后仍匹配原始身份、HEAD／index 不变。Linux 全量 22 单元＋24 集成通过；Windows 新增测试通过（11.83 秒），Windows release 构建通过。本轮无可见 UI 改动，未重复窗口截图或滚动测试。

已明确记录的 Git／参考行为：固定字符串模式完整身份仍按子串匹配，因此前缀姓名可能命中；正则模式首尾锚定不会命中。测试分别断言这两种结果。待办：作者目录按需加载和规模预算、当前用户配置读取、名称标准化及 me 解析、Select… 多选／speed search／近期项、菜单 UI／原生验收、大量作者参数在 Windows 命令行长度限制内的处理。过滤图暂存上限和原有性能待办仍保留。

作者查询层预览包 CRC／release 一致性通过；exe SHA-256 `4e6931f99483feccfdd9e7b9cf35883125e919329b7ecd25c4016627a9d38cc6`。


## R1 User 身份解析与当前用户（2026-10-16）

重新核对参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/vcs-log/impl/src/com/intellij/vcs/log/data/VcsLogUserResolver.kt`、`platform/vcs-impl/shared/src/com/intellij/vcs/log/util/VcsUserUtil.java`、`plugins/git4idea/backend/src/GitUserRegistry.java`。`src/authors.rs` 新增去重身份目录与名称／邮箱前缀索引，索引保存下标，避免每个别名重复拥有身份字符串。标准化遵循 ASCII 两段姓名规则（含贪婪下划线及 Java 的六种空白字符），不对 Unicode 姓名随意转小写。me 先解析当前姓名，再补入具有已关联完整邮箱的前缀匹配身份；没有姓名匹配时不擅自退化为全邮箱匹配。

`Repository::current_author` 通过 Git 有效配置读取 user.name／user.email，支持 include；缺少／空姓名为 None，空邮箱保留，损坏配置、取消等读取错误向上传递。`Author::exact_name` 对空姓名或空邮箱按参考规则返回单独一项。配置不从提交 author 环境变量推断。目录保留原始邮箱大小写，关联比较时转小写；这与参考 createUser 在创建时小写邮箱的存储方式不同，但保持原始 Git 身份供查询使用。

验证：Linux 核心 25 单元＋25 集成通过；随后补齐 vertical-tab 标准化并重跑 3 项作者单元测试通过。Windows 原生 3 项作者单元＋1 项真实 Git 配置集成通过；集成覆盖 include 优先级、空字段、损坏配置、取消及 HEAD／配置文件不变。本轮未修改可见 UI，未启动窗口或测试滚动；未更新便携包。

尚未接入完整作者目录加载、User 菜单／Select…／me UI。目录必须覆盖历史作者，不能只取当前可见提交；加载取消、唯一身份数量／字符串预算、Windows 查询参数长度仍须落实。上述测试不是 User 功能或内存／像素验收完成的证明。


## R1 作者目录流式加载（2026-10-16）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GitLogProvider.kt::readAllHashes（userRegistry）、plugins/git4idea/backend/src/history/GitLogUtil.java::readTimedCommits／LOG_ALL、UserFilterPopupComponent.java::collectUsers：完整 Log 的作者集合来自流式历史记录，不限可见提交；LOG_ALL 为 HEAD／branches／remotes／tags，不含 stash。只提取 author，不混入 committer。

本地 `Repository::author_catalog` 接受完整、固定的 Log root IDs，调用无 mailmap／notes／signature 的 NUL 分隔 Git log，仅输出原始姓名与邮箱。`authors::read_catalog` 逐字段读取并去重，无整份历史输出缓冲；限制字段 8 KiB（含 NUL）、16,384 个不同身份、4 MiB 原始身份文本。任一超限／格式错误整体失败，不返回部分目录。这些是加载预算，不是整个索引或进程内存上限。索引建立仍有集合／索引构建峰值，未测真实峰值。

Git 执行器增加共享 read_stream 路径，保留取消／30 秒超时／stderr 限额；流解析出错会通知控制线程终止 Git。无论 stdout 是否失败，都 join 两个 reader。既有普通读取仍保持 8 MiB 输出限额。

Linux 27 单元＋26 集成通过；随后增加 4 MiB 唯一身份文本超限用例，Windows 原生全量 27 单元＋26 集成通过（集成 17.38 秒）。新增用例覆盖超过 8 MiB 的重复记录流、身份数／文本／字段超限、截断和无效 UTF-8；真实 Git 验证只加载一条提交时跨分支作者完整、mailmap 不改变原始身份、固定 roots 不受后续提交影响、取消以及 HEAD／index 不变。Windows release 构建通过；没有可见 UI 变更，未启动窗口或做滚动自动化。

尚未接入 User 菜单。调用者必须使用未按分支／文本／日期过滤的全部 Log roots；作者加载的任务生命周期、刷新失效、Select… 多选／speed search／近期项、Windows roots／author 参数长度、大仓库实际 CPU／内存及中途取消压力仍待验收。

便携包已更新，ZIP CRC 和程序字节一致性通过；exe SHA-256 `d6d3af78b3ebfd187903766cbf89cbab92b64ecea13c0f4aff92f47b944866df`。


## R1 User Select 原生参照与选择模型（2026-10-16）

重新读参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 UserFilterPopupComponent.java、MultipleValueFilterPopupComponent.java、MultilinePopupBuilder.java、VcsLogUserFilterImpl.kt 和 GitLogProvider.kt::USER_FILTER。Select… 不是复选列表，而是有补全的多行文本 popup；只用 pipe／newline 分隔、Java trim、移除空项，Ctrl+Enter closeOk；空值确认清除条件，点击外部取消。初始值每行一个；普通 User 菜单 Select…／me／近期项，作者候选属于 speed search。源码最小尺寸 200×90 logical px，支持 resize，不含普通 OK／Cancel 按钮。

实际操作用户保留的 Rebased 1168／HWND 397638，保存 `artifacts/rebased-user-reference/user-select/user-reference-{start,menu,select,dismissed}.png`，2100×1500（此前同窗口 DPI=144，当前本轮未重测 DPI）。原生 Select 确认为贴在 User 下方的黑底多行框，下方一行灰字说明 pipe／new lines／Ctrl+Enter；输入区与 footer 分隔，无额外确认按钮。只打开并在空白区点击取消，未应用作者条件，截图确认返回原 Log，保留用户进程。没有滚动测试。脚本输出的单次内存值未用于性能结论。

`src/user_filter.rs` 新增 Selection：保留提交值和多行 draft 的转换，严格按参考分隔符／trim 规则处理，普通逗号是姓名内容；空选择为 None，无法解析的非空选择仍为 Some(empty)，多名称／me 解析取去重并集。输入预算 64 KiB／256 项，超限报错、不截断；这是本地内存约束。Directory::read 明确从未过滤的全部 Log roots 读取目录和当前配置，避免误用当前分支／可见行的作者。该模块尚未接 UI，不宣称切换仓库／刷新生命周期已完成。

Linux 与 Windows 均通过新增 2 项选择模型测试和更新的真实 Git 作者目录测试：涵盖 CRLF、pipe、逗号姓名、空值、Java trim 与 NBSP、预算、me／名字并集、未知名字不清除过滤，以及 Directory 完整 roots／配置。没有可见实现变更，本轮未更新 release 或便携包。

下一步：以该原生 popup 形式接 GPUI Textarea、Ctrl+Enter 与取消，接目录加载状态／任务取消／刷新失效、补全与近期项，随后做 Windows 同条件交互和像素对照。不能用通用复选弹窗代替这一行为。


## R1 User 菜单与多行弹窗接入（2026-10-16）

依据上一节已读 Rebased commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 UserFilterPopupComponent／MultipleValueFilterPopupComponent／MultilinePopupBuilder 和已保存原生截图，实现 `src/user_menu.rs`，并在 ui.rs 的 Branch 与 Date 之间接入。首次打开异步加载全历史目录；菜单含 Select…、配置存在时的 me、本仓库会话内最近 10 次选择。多行草稿默认回填当前值，Ctrl+Enter 应用、空值确认清除、Esc／点击外部取消。选择解析为原始身份后与现有 Text／Date／Branch 条件组合，哈希优先行为沿用现有 history 层。

目录任务有独立 Cancel／generation；取消初次加载、切换仓库和实体释放会取消旧任务，过时结果不写回。关闭弹窗释放编辑实体；目录按仓库持有至刷新或切换。刷新重读身份／配置，有活动条件时解析完成后重新查询；该路径尚未做变更配置／并发刷新原生压力验证。当前目录失败信息在 popup 中，活动过滤的后台刷新错误还需向主界面明确传达。

Windows release 编译通过，原生 `artifacts/user-filter-ui`：me-initial 确认当前 Alice 的两条提交；editor 显示黑底多行框、footer；bob 为文字 Bob Smith 经 Ctrl+Enter 后唯一提交；cancel-confirmed 为修改草稿再 Esc，保留 Bob／1 条；cleared 为 Ctrl+A／Backspace／Ctrl+Enter，恢复 User／3 条。初版 keydown 未拦截输入组件 Enter action，已改为 capture_action<Enter> 检查 secondary，避免确认时插入换行。早期 cancel.png 只是 IME 取消了组合文字、popup 仍开，不算完成取消；bob.png 已被最终成功结果覆盖。输入文字最终使用目标窗口 WM_CHAR，快捷键用前台 PID 校验后的真实 SendKeys。

测试窗口 8208／22784 已关闭，用户 Rebased 1168 保留。没有滚动测试，也没有新总体内存结论。不同窗口尺寸下的截图用于确认结构与交互，不等于逐像素验收。

差异／待办：speed search、作者补全、菜单初始蓝色选中／上下键导航、popup resize、字体／间距细节、近期项持久化及参考排序规则、失焦后的主窗口焦点恢复、目录刷新错误反馈、切换仓库和在途竞态的原生验证、输入过程与 Windows 参数长度预算。初始加载期间增加 Loading users… 并禁用 Select，此为本地按需加载状态。User 尚未整体完成。

预览包 ZIP CRC／release 字节一致性通过；exe SHA-256 `8ecde352671f3599acb764ea8edc1d340247610178e2fa2fc69a8347e8d7ec48`。


## R1 User 菜单键盘与刷新反馈（2026-10-16）

继续核对参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 UserFilterPopupComponent.java，并读取 platform/platform-impl/src/com/intellij/ui/popup/list/ListPopupImpl.java（首个可选项、Enter）与 platform/platform-api/src/com/intellij/ui/ScrollingUtil.java（上下键依赖 UISettings.cycleScrolling）。本地 UserMenu 新增独立菜单焦点／键上下文与游标；打开选择 Select…，蓝色 #2E4D89，Up／Down 跳过 Recent 标题，Enter 按当前项执行。当前边界停留，不支持 JetBrains 可配置的循环导航；hover 跟随、Home／End、补全／speed search 仍待实现。

UserMenu 事件区分 Applied 和 Failed，后台刷新失败反馈到 Review。检查发现既有 Review.error 仅位于底部 Diff／设置区域，独立 Tab 默认布局不可见，现于主 Git 工具栏下方显示错误文字；成功查询清除。已查阅 VcsLogErrorHandler／LoggingErrorHandler 但它们没有证明 JetBrains 的可见错误布局，当前文字条是本地可见性修复，不宣称按参考像素还原通知。底部模式仍可能出现重复错误文本，后续需要统一展示。

Windows release 构建通过。原生证据 `artifacts/user-filter-keys`：selected 首项蓝色；down 下键切换到 me；me-enter 选择 me；identity-real-refresh 仅修改测试仓库当前配置为 Bob 后真实点击刷新，me 重新解析为 Bob／1 条（identity-refreshed 等早期截图仍为 Alice，不算成功证据）。本轮未单独截图 Up，反向路径仅代码实现。`artifacts/user-filter-errors/failed.png` 确认损坏配置时主布局可见 Could not refresh User filter，recovered.png 确认恢复有效配置、刷新完成后提示消失并恢复 Alice／2 条。早期瞬时恢复截图是查询未完成，已被最终截图覆盖。仅对专用 fixture 配置做故障注入，已从备份恢复。

测试窗口 11140、3168、27576 已关闭，Rebased 1168 保留；3168 因旧进程退出时 exe 暂时占用导致复制失败，未作为新构建证据，后续等待退出并重新复制后以 27576 验证。未做滚动测试或总体内存验收。

待办：菜单剩余交互／补全、切换仓库与并发请求压力验证、目录加载可见状态、主错误条与 JetBrains 通知的视觉对齐、Windows 参数长度与输入预算。预览包已更新，CRC／release 一致；exe SHA-256 `074be56562a8ed70be5980603e6e25cd6613fcfa72310cfb6af95ace54938a61`。


## R1 作者补全的候选与范围规则（2026-10-16）

继续对照 `4aec35256d765c217aa0d662b82ac4e860c396fd`：UserFilterPopupComponent.collectUsers、VcsUserUtil（已读），本轮读取 ValuesCompletionProvider.java、TextCompletionProviderBase.java、DefaultTextCompletionValueDescriptor.java、platform/analysis-api/src/com/intellij/codeInsight/completion/PlainPrefixMatcher.java。候选使用短名称；ASCII 双段姓名将分隔符变为空格，仅首字母大写、保留其余大小写；无姓名用邮箱前缀，没有 @ 时用完整邮箱；显示项 TreeSet 去重。补全 prefix 为最后一个 pipe／newline 之后直到 caret 的原文，不 trim。PlainPrefixMatcher 默认 prefixMatches 实际为 containsIgnoreCase，isStartMatch 另行判断开头，不能误实现成只按 startsWith 匹配。lookup 的排序／匹配与菜单 speed search 仍须区分。

`authors.rs` 抽出共用 first_last，新增 Catalog::presentations，按 Java UTF-16 顺序排序、显示名去重，原始身份索引保持完整。`user_filter.rs::completion_range` 提供 UTF-8 caret 对应前缀范围，非法字符中间／超界 offset 返回 None；保留其他作者、空格及逗号。此 API 仅描述前缀范围，不宣称已实现 JetBrains lookup 的后缀替换／提交字符行为。候选目前供后续弹窗接线使用，未接入可见 UI。

Linux／Windows 原生库测试各 31 项通过。新用例包含多邮箱同名、姓名大小写保留、邮箱回退、Unicode 与 supplementary 字符排序、中文光标边界、多行／pipe 输入范围及既有作者不受替换影响。无可见变更，没有启动窗口、滚动测试或更新便携包。下一步接候选匹配／输入补全与原生行为验证，不能把当前数据层视为补全功能完成。


## R1 补全匹配与输入接口检查（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 StringUtil.java::containsIgnoreCase、platform/util/base/src/com/intellij/openapi/util/text/Strings.java::indexOfIgnoreCase／compare(char, char, true)、StringUtilRt.java::toUpperCase／toLowerCase。PlainPrefixMatcher 的默认 contains 匹配最终逐 UTF-16 char 比较，先 uppercase，再比较 uppercase 结果的 lowercase；不是把字符串整体展开后比较。

`user_filter.rs::completion_matches` 增加无字符串折叠缓存的包含匹配：逐候选起点及 UTF-16 单元比对，保留多字符展开边界，处理 İ 的 simple lowercase，不把 ß 与 ss 混为一谈；不做模糊缩写匹配。当前大小写映射使用 Rust Unicode 数据，未穷举对比安装版 JDK 的所有 Unicode／simple-uppercase 特例，不能宣称完整 Unicode 表等价。搜索最坏为候选长度乘查询长度，后续 UI 接入时须控制每帧扫描及候选数量。

检查 GPUI 0.7.3 输入实现：TextareaState.cursor 是 UTF-8 byte offset，set_selected_range 接受字节范围，补全 LSP 能力属于 Editor 的 extras；普通 Textarea 需要自行管理候选与应用范围。尚未改变可见输入控件，未引入完整代码编辑器。

Linux 和 Windows 原生各 4 项 user_filter 测试通过，覆盖中间子串、混合大小写、重音、中文、土耳其 I、sigma、空查询、前导空格保留、非模糊匹配、ß 不展开，以及 supplementary case pair 的 Java char 行为。没有 UI 改动、窗口／滚动测试或预览包更新。待接入候选显示、Enter／Tab 接受、首个 Esc 仅关闭候选、Ctrl+Enter 应用草稿，并验证实际光标替换与生命周期。


## R1 Paths 查询入口（2026-10-16）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，已读取 GitLogProvider.kt 的 STRUCTURE_FILTER 分支（`--full-history --simplify-merges -- <relative paths>`）、VcsLogStructureFilterImpl.java（before/after 的目录祖先匹配、多路径并集）、StructureFilterPopupComponent.java 的 EditPathsAction／SelectPathsInTreeAction（换行编辑、空值清除、树选择）。本地入口为 `LogFilter.paths`、`Repository::snapshot_filtered_at` 和 `history::read_filtered`。特别注意 Git 简化路径历史会改写 `%P`，本地必须从未过滤 metadata 恢复真实父关系；三项集成测试及验证范围见 PLAN。菜单及选择器尚未实现。


## Paths 树形选择入口（2026-10-16）

已读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 VcsStructureChooser.java、SelectionManager.java 和 FileChooserDescriptor.java；选择状态、目录／文件过滤、Space 与空选择规则翻译到 `src/path_tree.rs` 和 `src/path_chooser.rs`，入口在 `src/paths_menu.rs`。源码规则、Windows 行为证据、边界与未完成视觉差异详见 PLAN 的“R1 Paths 树形选择器”。


## Branches 展开／收起与单搜索（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，入口在 `plugins/git4idea/backend/src/ui/branch/dashboard/BranchesInGitLogUiFactoryProvider.kt`：ExpandablePanelController 隐藏 treePanel，ExpandStripeButton 保留竖条。BranchesDashboardTreeComponent.kt 在展开树中安装分支搜索；因此用户单框截图体现收起状态，不意味着源码没有分支搜索。ExpandStripeButton.kt 逆时针旋转文字。对应 `src/ui.rs::branches_rail` 及独立收起分栏状态，截图／默认值取舍／字体轮廓和持久化限制见 PLAN 最新记录。


## Islands Tab painter（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`：`platform/platform-impl/src/com/intellij/openapi/application/impl/islands/IslandsTabPainter.kt` 的 paintTab／paintUnderline／getColors，与 `platform/platform-resources/src/themes/islands/ManyIslandsDark.theme.json` 的 EditorTabs 颜色键。普通模式 28 px 圆角填充与边框，不画下划线。已映射到 `src/ui.rs::islands_tab`；活动／非活动状态和全部 hover 规则不能混为一谈，本地目前仅完成活动选中和普通 hover，验证与差异见 PLAN 最新记录。


## 文件工具栏（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`：VcsLogChangesBrowser.kt → AsyncChangesBrowserBase.kt → ChangesBrowserBase.java；基础左侧 Show Diff，TreeActionsToolbarPanel.java 将 Expand All／Collapse All 工具条放右侧。Log 的额外动作通过 intellij.platform.vcs.log.impl.xml 的 Vcs.Log.ChangesBrowser.Toolbar 引入。已接入 `src/ui.rs::files_toolbar`，展开状态由 set_files_expanded 修改，Windows 证据与剩余差异见 PLAN。

## 侧栏、提交与 Squash（2026-10-17）

用户新范围与验收以PLAN.md“侧栏、提交与历史整理增量”为准，截图位于artifacts/rebased-user-reference/{commit-project-toolwindows,log-multiselect-menu}.png。继续使用参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；本次未确认安装版本与快照一致性。

本次实际读取及可确认规则：

- `plugins/git4idea/backend/src/rebase/log/squash/GitSquashLogAction.kt`：少于两项不显示/启用Squash；按选中提交完整消息生成prettySquash草稿，消息弹窗确认后后台执行。优先InMemoryRebaseOperations.squash，使用executeInMemoryWithFallback；成功定位结果并提供含Undo的成功通知。
- 同目录 `GitSquashOperation.kt`：Git fallback走GitCommitEditingOperation.rebase；编辑器在todo模型unite选中索引，再对合并根reword。不是直接reset --soft。具体unite排序及in-memory/fallback切换原因尚未读取。
- `plugins/git4idea/backend/src/rebase/log/GitMultipleCommitEditingAction.kt`及`GitCommitEditingActionBase.kt`相关检查：单仓库选择，HEAD可达性、merge元数据和protected remote branch约束；执行前checkHeadLinearHistory重新检查HEAD路径并查询最旧选择所属保护分支。checkHeadLinearHistory把遇到父数不等于1的节点作为错误；另有根提交确认代码，因此根提交在不同调用链/后端的实际可达行为需进一步测试，不能仅凭弹窗代码宣称允许。基础类默认Allow的rebase-during-rebase策略也不能未经下游检查就解释为任意进行中状态安全。
- `plugins/git4idea/tests/git4idea/rebase/log/squash/GitSquashOperationTest.kt`前180行：相同测试参数化Git与In-Memory两后端；覆盖最近若干、历史中间连续段、间隔选择（测试命名non-linear，实际构造为线性历史上的不连续选择），未选between提交保留在结果后方；还读到Undo不连续选择的首段断言。后续测试与完整Undo条件仍未读取，禁止把名称non-linear当作支持任意merge DAG。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/CommitToolWindowUtil.kt`：有内容才打开Diff，一般交给DiffEditorTabFilesManager；浮动/独立Commit窗口与registry组合可能强制新窗口。本地计划使用左侧停靠窗口和中央只读Tab，浮动窗口不在本次范围。
- `platform/vcs-impl/src/com/intellij/vcs/commit/NonModalCommitWorkflow.kt`：检查通过后等待actions-on-save再performCommit；取消/异常结束执行。轻量客户端没有IDE保存动作系统，仍须遵循确认/执行/结束状态边界。
- `plugins/git4idea/backend/src/checkin/GitCheckinEnvironment.kt`的commit、doCommit、createCommitOptions段（165–290）：按repo收集选择的changes，创建包含CommitToAmend的options，调用commitRepository；仅无exceptions且isPushAfterCommit才进入GitPushAfterCommitDialog。**本次未读完commitRepository及index/partial处理，不能据此决定具体Git命令。**

下一步必读：GitCheckinEnvironment的commitRepository及暂存保留/部分提交实现、GitCommitOptions与Amend handler、非模态Commit UI选中模型及测试；ProjectViewImpl、工具窗口停靠/上下分区与Saved Changes入口；GitRebaseTodoModel.unite、GitCommitEditingOperation、InMemoryRebaseOperations.squash、完整Squash测试及消息/Undo处理。ProjectViewImpl等仅确认路径存在，尚无行为结论。实现映射暂定src/ui.rs的顶层布局/本地视图/Log选择，提交与Squash使用独立核心模块并接入现有执行器；文件名及接口需在读取现有模块后确定，本轮未新增产品代码。

## Project 文件浏览读取层（2026-10-17）

参考仍为4aec35256d765c217aa0d662b82ac4e860c396fd。本轮读取ProjectViewImpl.java中AutoScrollToSourceHandler（620–658）及pane切换安装树处理器（1085–1100）：单击打开取决于autoscroll/preview选项，目录toggleClickCount可设1或2，切pane恢复展开路径。因此后续UI不能把任意单击无条件当成固定Tab打开；本轮未实现这些UI行为。

读取platform/lang-impl/src/com/intellij/ide/projectView/impl/nodes/ProjectViewDirectoryHelper.java的getDirectoryChildren、directoryChildrenInProject、processPsiDirectoryChildren相关段：普通模式枚举当前目录，依据project/view settings过滤，文件/目录分别建node；flatten package及hide empty middle packages会走额外递归。单仓库轻量浏览不复制PSI/project索引与语言包折叠；当前仅形成底层真实文件系统枚举API，UI隐藏项策略仍待。

本地src/project_files.rs提供Project::children/read：只读取指定目录或文件，包含未跟踪文件，.git管理路径排除；目录优先并复用现有自然排序。PathBuf保留系统路径身份。读取上限5MiB，目录4096节点/512KiB相对路径字节，有取消检查；超限明确报错，不返回伪完整列表。符号链接及Windows reparse节点标为Link且暂不展开/读取，作为首步适配差异记录；不是声称上游也拒绝所有链接。UTF-8（含BOM）原行尾及末行保留，NUL判二进制，UTF-16 BOM/非UTF-8明确UnsupportedEncoding；后续编码支持仍待。元数据前后复核用于发现常见读取期间改变，不是原子快照，不对恶意并发目录替换提供沙箱保证。

tests/project_files.rs覆盖真实目录和文件，不替代原生UI验收；Windows junction通过mklink /J构造并在末尾移除。Project工具窗口、上下分区、文件Tab、UI错误映射及总缓存生命周期尚未接线。

## Project 工具窗口与只读 Tab 接线（2026-10-17）

沿用上节已读的参考 4aec35256d765c217aa0d662b82ac4e860c396fd，复核 ProjectViewImpl.java 的树安装、toggleClickCount 与恢复展开路径段。src/project_view.rs 实现单击选择、箭头展开、双击/Enter 打开文件；不把单击无条件绑定打开。src/ui.rs 左侧 Project 入口与中央文件 Tab 共存，关闭 Tab 返回原 Log/本地视图。展开仅请求当前目录，整体最多 16384 节点；文件正文只保留活动 Tab，离开取消请求并释放 viewer，再次激活重读。

差异：暂不实现自动滚动到源码、单击 preview Tab、语法分析/高亮、浮动/可调整宽度工具窗口、文件 Tab 滚动位置恢复；Project 固定宽度，尚未迁移本地/已保存更改及上下分区。沿用既有文件读取限制，无编辑和保存功能。安装版与参考快照版本一致性本次未确认。Windows 验证记录见 PLAN 的同名段。

## Saved Changes 停靠与双工具窗口（2026-10-17）

参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd，本轮实际读取：
- platform/vcs-impl/src/com/intellij/openapi/vcs/changes/savedPatches/SavedPatchesUi.kt：TwoKeySplitter 依据 isVertical/preview 决定记录树与 changes browser 上下/左右组织；竖向默认比例 0.5；选择 patch 更新 changes browser；双击/Enter 打开 editor diff preview，底部操作属于 changes browser。split preview 按选项创建和销毁。
- platform/platform-impl/src/com/intellij/toolWindow/ToolWindowPane.kt 的 addDecorator/removeDecorator（225–288）及 addAndSplitDockedComponentCmd（611–676）：同侧 docked 工具窗口用 splitter 分区，移除一个时保留另一组件并保存 split proportion；方向受 leftHorizontalSplit/rightHorizontalSplit 控制。

本地映射 src/ui.rs::left_tool_windows 和独立 tool_windows_split/tool_windows_width 状态；src/stash_view.rs 从中央横向排列改为侧栏上下排列。移除 Stashes 中央主 Tab，打开/关闭左侧窗口不切换中央 Tab，打开其 Diff 也不隐藏侧栏。已有 apply/pop 仍使用同一执行器，未修改 Git 命令。图标 shelve_dark.svg 原样来自 platform/icons/src/expui/vcs，来源与 Apache-2.0 已追加 assets/icons/SOURCE.txt。

保留差异：目前左侧上区为 Saved Changes、下区为 Project，Commit 独立状态迁移尚待；不支持任意停靠位置/方向切换、浮动窗口及跨重启布局持久化。上游初始分割比例 0.5，本地先给记录列表 150 logical px，支持拖动；安装版本与参考快照一致性未重新确认。

### 用户调整：独占侧栏（2026-10-17）

用户明确取消工具窗口上下分区，采用一次显示一个左侧工具窗口。此要求覆盖上节源自 ToolWindowPane 的双窗口布局：src/ui.rs 删除 tool_windows_split，Project/Saved Changes 开启时互斥，保留实体及选择状态；重复点击当前入口收起，中央 Tab 不变。保留侧栏与中央区域之间的宽度拖动。Saved Changes 内部记录列表/文件列表仍按 SavedPatchesUi 竖向组织，这属于一个工具窗口内部的内容，不是多个工具窗口同时显示。后续 Commit 迁移也遵循同一独占规则。

## 本地变更独立树模型（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd：读取 ChangesViewPanel.kt 全文，确认 changes tree 为中央内容、横向 toolbar 位于顶部；NonModalCommitPanel.kt 前180行确认消息编辑器、status/toolbar、author 与 commitActions 的组织，inclusion dispatcher 独立于普通树选择。读取 LocalChangesListView.java 全文：其 collapseAll 保留首层展开。读取 LocalChangesListViewSelectFileTest.kt 三项测试：按 FilePath 选择 unversioned/普通变更，同文件处于多个 change list 时选择 Default 列表。本次未读取完整提交 inclusion/default check 流程，不能由 review selection 推导提交勾选。

src/local_changes.rs 是独立 metadata-only Model：按 Area/path/old_path 维护审阅身份，刷新不按旧下标选中另一个文件；复用 file_tree 生成可见行并清理消失目录的折叠记录，键盘只遍历可见文件。没有后台任务、正文缓存、提交勾选或 Git 写命令。本地尚保留 Conflict/Unstaged/Staged/Untracked 四组，这是现有比较目标语义；与上游 ChangeList 分组不同，后续 Commit UI 合并分组时必须同时处理比较目标与 inclusion，不能把当前模型当作已还原的 Commit 数据模型。普通刷新中已消失的审阅对象清除选择，不自动改为同路径另一 Area。

模型目前未接入 UI，原中央 Local Changes 未移除。Linux/Windows 模型测试证据见 PLAN；安装版版本差异本次未重新确认。

## 本地更改独占侧栏接线（2026-10-17）

沿用上一节已读同一 commit 的 ChangesViewPanel（顶部横向工具栏+树）、LocalChangesListView（保留首层的 Collapse All）及 NonModalCommitPanel（提交消息/操作独立组织）。src/local_view.rs 接入独立 Model、取消 token/generation、目录虚拟列表、独立焦点；src/ui.rs::open_local 将本地 Diff 事件路由到既有 Target/Chain/DiffTab，以完整 target identity 复用 Tab。读取只调用 Repository::local_changes，不修改索引。工具窗口切换/收起不修改中央 Log 选择或关闭文件/Diff Tab。

移除中央 Local Changes 入口，Saved Changes 的 ViewLocal 动作指向新侧栏；Git操作后的 refresh_impl 同步刷新本地元数据但不切换中央模式。切仓清空本地实体，Drop 取消任务。现阶段标题仍为 Local Changes，没有伪 Commit/Amend 按钮；后续提交分组和 inclusion 继续按上游深入。

保留差异：原四种 Area 组尚未整理为 Rebased Commit change list；当前从侧栏显式打开的比较统一进入中央 Tab，原本地更改底部预览偏好尚待重新接线，不能勾选完整迁移。旧 Review.local 分支暂保留但没有可见启用入口，后续清理不与提交核心混改。键盘、tooltip、焦点选中颜色及圆角还有验收/细化待办。

## 本地侧栏底部预览恢复（2026-10-17）

参考仍为 4aec35256d765c217aa0d662b82ac4e860c396fd。本次读取 ChangesViewDiffPreviewProcessor.kt 的构造/ChangesViewDiffPreviewHandler：通过 TreeHandlerChangesTreeTracker 跟随树，isShowAllChangesForEmptySelection=false，选择变更与未跟踪文件分别包装，同组导航由 ChangeList/Unversioned 节点确定。读取 ChangesViewManager.kt 的 ChangesViewToolWindowPanel init/updatePanelLayout/ChangesViewSplitterDiffPreview 段：根据方向及预览设置创建或dispose splitter processor，openPreview 接受 requestFocus。复核 CommitToolWindowUtil.kt：无内容不打开，通常交给 DiffEditorTabFilesManager，浮动窗口有独立规则。

本地 src/diff_tab.rs 增加 embedded 预览构造，复用渲染/Changes偏好、差异导航和有界读取；embedded 不主动focus且使用片段patch，独立Tab继续full_patch。src/local_view.rs::preview_target 输出元数据target/chain，busy/error/无选择不产生目标。src/ui.rs 只在本地工具窗口可见、bottom设置开启且中央无活动文件/Diff Tab时创建local_preview；侧栏通知使旧预览失效、重新从当前选择生成，隐藏/切Tab/切仓drop并取消请求。F7/Shift+F7优先路由当前可见预览。

差异：按用户保留的底部布局偏好，预览放中央Log下方而非窄左侧；embedded隐藏文件链切换器，由侧栏负责文件选择，避免预览换文件后树选择不一致。未复制PSI、line status tracker或提交排除功能。安装版和快照一致性本次未确认。

## Commit／Amend 的暂存区保留（2026-10-17）

参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，未据此推断已安装 Rebased 的 registry 覆盖值。实际读取：

- `plugins/git4idea/backend/src/checkin/GitCheckinEnvironment.kt::commitRepository/stageAndCommit`：部分提交／大小写重命名先处理，prepare 排除未选路径，stage 所选路径，执行 commit，use/close 恢复排除的暂存项。不是整体 index 回滚。`createCommitMessageFile/runWithMessageFile` 用 Git commit encoding 写临时消息文件，finally 删除。
- 同目录 `GitStagingAreaStateManager.kt` 两种实现选择；`GitIndexInfoStagingAreaStateManager.kt` 全文：porcelain v2/no-renames 读取 HEAD/index 身份，拒绝冲突；排除不在 toAdd∪toRemove 的 staged 路径；update-index --index-info 临时恢复 HEAD，再恢复保存的 index blob。原实现准备／恢复异常只记 warn。
- `GitResetAddStagingAreaStateManager.kt` 全文：旧后端 reset 后重新 stage 文件，不能保证恢复原始 staged blob，特别是工作区有额外修改或 added/deleted。`backend/resources/intellij.vcs.git.backend.xml` registry `git.commit.staged.saver.use.index.info` 默认 true，当前源码默认使用新后端。
- `GitRepositoryCommitter.kt` 全文：消息文件 -F，Last 加 --amend；signoff/skipHooks/author/date/cleanup 仅显式选项启用；GPG 失败与空提交单独识别，不自动禁用签名重试。`GitCommitOptions.kt` UI 选项及 `GitAmendCommitService.kt` 全文：Last message 通过 %B 读取；Specific amend 是另一个能力，不能混称 Last。
- `backend/src/util/GitFileUtils.kt::stageForCommit`：删除用 rm --ignore-unmatch --cached -r，新增／修改强制 add 指定路径，分别收集异常。
- `tests/git4idea/tests/GitCommitTest.kt` 已读 409–500、664–740、770–801、1026–1055：原 index 与工作区不同的排除项、大小写重命名、AD/DA、未解决冲突及排除 staged submodule。测试明确验证 `show :c.java` 与磁盘内容分别保持 staged/unstaged。

本地首个落点 `src/commit_index.rs::ExcludedIndex`，只构造准备／恢复计划，不执行 Git、不接 UI。输入使用 NUL 分隔的 porcelain v2，8MiB 上限，未选文件保存原 mode/OID/path，无文件正文；输出 `update-index -z --index-info` 输入。保留完整 mode（包括 symlink），先移除全部排除端点再添加目标项，支持文件／目录互换；路径保留原始字节，支持空格/Unicode/换行。与上游换行字符串和 executable bool 的实现差异是避免路径转义丢失和 symlink mode 丢失。异常记录/格式错误/冲突拒绝生成部分计划。

后续执行器必须持有仓库串行 guard，从读取到恢复覆盖整个事务，准备失败停止、取消后仍恢复并明确报告恢复错误。当前测试手动驱动 Git 以验证计划，尚未实现这些执行器保证；没有承诺恢复完整 index 文件字节或所有扩展标志。提交勾选与 Area 审阅选择仍需分离，部分行、大小写 rename、hooks/GPG、编码及 Amend 消息草稿生命周期继续待实现/验证。

### Commit 输入传输接线（2026-10-17）

同一参考 commit 进一步读取 `plugins/git4idea/backend/src/commands/GitHandlerInputProcessorUtil.java` 全文：writeLines 写入分隔记录并 flush/close，redirectStream 复制后关闭，empty 直接关闭。该文件不证明上游线程或取消机制；本地并发策略沿用既有 ProcessTree 所有权。

`src/operation_process.rs::run_with_input` 接收借用字节切片，不额外复制；scope 内 writer 与 stdout/stderr reader 并行，EOF 在写入结束时关闭stdin产生。等待循环继续处理取消/超时，先终止自有进程树再join所有线程；写失败也结束自有进程树。新增 Outcome.input_error，保留Git退出状态、stderr、停止原因，不以BrokenPipe覆盖取消。调用方必须同时检查三者；原run仍为null stdin，现有调用点不产生input_error。预算由构造输入的调用方负责，本层不将输入日志化。

`tests/commit_index.rs` 的真实Git prepare/restore已改走run_with_input，仍非生产Commit事务。`tests/operation_process.rs` 新增输入/双输出管道饱和、空输入EOF、拒绝输入、阻塞写入取消/超时、预取消；两路输出继续64KiB尾部预算。未实现消息编码、勾选、完整提交事务或恢复失败的UI呈现。

### Commit／Amend Last 执行器首版（2026-10-17）

再次读取同一 commit 的 `GitRepositoryCommitter.kt` 全文，确认 -F、Last 的 --amend、可选hooks/signoff/cleanup、GPG与空提交识别；未启用的选项不能强行加入。沿用前述 `GitCheckinEnvironment.stageAndCommit` 和 `GitFileUtils.stageForCommit` 的 prepare→rm/add→commit→restore 顺序。

本地 `src/commit_operation.rs`：Request 显式 add/remove 路径、消息、amend、expected_head（None表示unborn，不是跳过校验）；串行queue guard贯穿读取、prepare、暂存、提交、恢复和结果读取。临时消息文件位于工作树Git目录、随机名称create_new、Unix 0600、RAII删除，1MiB消息上限。rm --ignore-unmatch --cached -r/add --force 使用literal pathspec及NUL stdin；commit -F [--amend] --，不跳hooks/签名、不自动重试。prepare一旦尝试后即无条件以新Cancel令牌恢复排除项；保存分阶段命令结果与真实HEAD/operation state，恢复失败返回原始index-info恢复数据，不能把已成功commit但恢复失败报告为整体成功。

差异／未完：当前仅UTF-8消息，其他i18n.commitEncoding在任何暂存修改前明确拒绝；目录/所选submodule、部分行提交尚未接入；普通选文件提交不接管进行中的merge/rebase（已有独立完成流程）。排除的submodule仍由上一轮metadata方案保留。用户明确勾选的rename要传入两端。恢复数据目前只由Report持有，GUI接入前须补持久恢复记录/交互，不能让关闭UI丢失恢复能力；不自动删除Git锁、不声称对外部客户端并发或进程崩溃提供原子回滚。GPG/空提交专门分类、非UTF8编码、作者选项、Amend消息草稿、Commit and Push仍待。保持hooks配置并不等于已覆盖所有自定义hook修改index的行为。

### 提交结果流式识别（2026-10-17）

再次读取上述commit的GitRepositoryCommitter.kt中GitGpgProblemDetector/GitEmptyCommitProblemDetector；结合已读Pinentry detector：GPG stderr包含区分大小写的`gpg failed to sign the data`，pinentry stderr包含`No pinentry`/`pinentry`/`signing failed`或GPG提示；空提交stdout行首不区分大小写匹配No changes/no changes added to commit/nothing added to commit/nothing to commit。

本地src/commit_result.rs按流区分检测，固定40字节行首/后缀缓存，跨read碎片匹配、CR/LF断开，EOF无需换行，信号不因输出tail截断丢失。operation_process::Outcome新增commit_signals；识别证据不等于提交失败，commit_operation::Report::status先处理恢复失败/取消/超时，再结合commit阶段、退出状态和实际state读取结果分类Success/SigningFailed/Empty/Failed。pinentry保留独立信号供未来提示，不自动配置GPG、不自动关闭签名或重试。用户在阶段之间取消导致run尚未启动时，由Report.cancelled保留取消状态，避免仅字符串错误降成泛化失败。

新测试使用真实Git空提交与不存在的GPG程序，并断言HEAD未变化和排除index恢复；不依赖用户密钥/签名代理。检测单测覆盖分片、错误流、非行首、跨行和超长后续日志，进程测试验证输出截断后仍保留信号。尚不覆盖真实pinentry GUI，也未加入非英语Git输出的本地化检测（对应上游英文模式差异仍需验证）。

### 提交恢复记录持久化（2026-10-17）

继续沿用已读GitIndexInfoStagingAreaStateManager保存排除项mode/OID并restore的规则；持久记录是本地补充，上游该类仅内存保存且restore异常记warn，不将本地崩溃恢复宣称为上游现成功能。

src/commit_recovery.rs 在每工作树Git目录的gitui-commit-index-recovery独占create_new，格式带版本、长度、提交前HEAD、prepare输入与restore输入；单份输入16MiB上限，读取限制各字段长度并拒绝未知格式、截断、附加内容与非普通文件。生成端数据来自ExcludedIndex；读取API仅检查记录格式，不代表允许重放。文件写完sync_all之后才允许prepare，Unix同步所在目录；Unix权限0600，Windows沿用Git目录ACL。恢复成功删除；删除失败单独RecoveryRecordFailed，不冒充index未恢复。恢复失败保留文件和内存数据。任何已有标记（含损坏文件/目录）在新Commit开始时阻止覆盖，需后续显式恢复流程处理。

生产恢复入口仍待：必须检查现有index受影响路径是否仍为prepared/原始身份，不能把磁盘record盲目传给update-index；还需恢复成功后清理记录及相应UI。本轮测试内手动回放仅限所有writer均由fixture拥有的仓库。记录持久化并不保证Git对象免于未来GC，也未实现对象保活引用、断电/磁盘硬件故障保证或对外部篡改的安全边界；崩溃可能遗留消息临时文件及Git锁，产品仍不自动删除未知锁。

### 显式恢复前的暂存身份核验（2026-10-17）

commit_recovery::restore现在提供worker API，仍不在启动时自动执行。沿用前述Rebased mode/OID恢复规则，增加本地中断恢复的前置条件：持有仓库queue guard，解析记录中NUL index-info的删除/添加顺序、模式、OID和相同路径集合；读取ls-files --stage -z，拒绝冲突；每个受影响路径必须仍为prepared或original身份，否则停止。不相关的新暂存项保留，若其祖先/后代与恢复路径重叠则停止，避免update-index文件/目录替换误移除新项。前缀检查使用有序查找和路径分段，避免全仓库项×恢复项的二次遍历。

执行前再次检查记录未变；恢复使用已有带stdin的进程执行器，失败/取消保留记录且不删除锁。成功后重新读取index，核对受影响身份均为original，再确认记录未变并删除。HEAD与工作区不由此API修改。读取仍受已有Git输出8MiB限制，大index超限明确失败而非不完整校验。此API不提供其他客户端并发事务锁；检查与Git写入之间仍有外部并发窗口，不承诺原子隔离。磁盘记录的结构检查不是对恶意篡改的认证；before_head用于后续展示，不能要求它等于当前HEAD，因为原commit可能已经成功。

生产恢复界面/确认仍未接入；测试是显式调用API。对象长期保活、更多编码/所选submodule等差异继续保留，不将这些差异混作完成。

下一步侧栏Amend入口准备：已读同一commit的platform/vcs-api/src/com/intellij/vcs/commit/AmendCommitHandler.kt全文。它只定义None、Last.Known/Unknown、Specific和toggle listener，不能据此推断消息草稿替换/恢复；NonModalAmendCommitHandler及消息policy仍需深入读取后实现。

### Amend 消息草稿生命周期（2026-10-17）

同一参考commit实际读取NonModalAmendCommitHandler.kt全文：异步load details、loading影响提交可用性；关闭取消请求并恢复message/author；Last可无文件变化提交。AmendCommitHandlerImpl.kt全文：自动替换前记录before/amend消息；关闭仅当当前消息精确等于自动消息时恢复before，否则保留用户修改。setAmendMessage在忽略空白相同时不替换。追到Strings.java::equalsIgnoreWhitespaces委托与StringsKmp.kt::equalsIgnoreWhitespaces/isSpaceEnterOrTab，实际只忽略SP/TAB/LF，不含CR和全部Unicode空白。

读取ChangesViewCommitMessagePolicy.kt、AbstractCommitMessagePolicy.kt与DefaultCommitMessagePolicy.kt：before commit保存消息历史，成功后按CLEAR_INITIAL_COMMIT_MESSAGE清理或取新消息；VcsConfiguration.java该字段源码默认true。ChangeList policy按changelist保存comment；这不表示本项目已有消息历史/changelist持久化。

本地src/commit_draft.rs独立状态Off/Loading/Ready/Failed、消息与自动替换前值。关闭Amend只精确恢复未编辑自动消息；成功调用committed才清理，失败/取消不清理。允许Ready且零勾选的仅消息Amend，Loading/Failed禁用提交。generation阻止旧请求回写；loading期间手动编辑优先于自动消息是本地明确补充，上游setAmendDetails当前文本仍可被替换，本地避免异步覆盖。load_amend_message固定OID读取UTF-8完整%B，前后检查HEAD；沿用1MiB消息限制，不修改index。作者手动编辑/消息历史/持久草稿/勾选UI尚未实现，当前模型未接LocalView。

### 提交文件勾选模型（2026-10-17）

同一commit实际读取ChangesViewCommitWorkflowHandler.kt的synchronizeInclusion/setInclusion/setSelection（90–192）：刷新retain到仍可包含的集合；setCommitState根据激活/force决定重置或增量包含默认changelist；树选择与included集合分开。PartialCommitInclusionModel.kt全文：独立inclusion API，底层支持部分行tracker与三态。仅此模型定义不能证明初始所有场景都默认全选。

本地src/commit_selection.rs对现有Git Area审阅数据按路径合并整文件提交项，同一路径暂存/未暂存只共享一个inclusion身份；rename来源参与身份，刷新清理消失/变换来源的勾选；冲突不允许包含并阻止生成请求。终态以Untracked重新创建优先，其次Unstaged、Staged，生成具体add/remove集合，rename两端纳入，copy不删除来源，同时待add路径从remove剔除，映射前述stageAndCommit规则。

保留差异：本地仅whole-file，无命名changelist和partial-line tracker；新出现文件不自动纳入，初始/组全选由UI显式调用set/set_all，不把Rebased的setCommitState激活策略硬塞进refresh。没有以本地树高亮/打开Diff自动勾选。未接UI；现有四Area审阅树不因该模型自动变成Changes/Unversioned树。

### Commit 侧栏首轮可见接线（2026-10-17）

再次读取NonModalCommitPanel.kt前185行：中心message区域、status/author/actions底部组织、最小高度120及输入边框层次。GitCheckinEnvironment.commit中仅exceptions为空时调用GitPushAfterCommitDialog.showOrPush；GitPushAfterCommitDialog.java全文确认按preview/protected偏好决定展示或直接push(false)。本地当前始终打开已有Push预览，不实现跳过预览偏好；未重写已有Push执行逻辑。

src/local_view.rs接入Selection、Draft、TextareaState、独立amend/write token/task、HEAD与恢复标记。树高亮仍负责审阅，Checkbox按文件路径同步whole-file包含；消息/Amend/Commit/Commit and Push位于下方，输入只由控件获得焦点，移除父容器强制抢焦点。后台Commit失败保留草稿/勾选，成功清理；所有结果发Changed刷新Log和本地状态。src/ui.rs在成功且用户选择Commit and Push时打开现有Push对话框。恢复标记阻止提交并提供显式恢复按钮；取消按钮绑定操作令牌。新增静态标签中英文，Git详细错误保持原文。

结构差异仍保留：文件树暂保留原四Area审阅分组，因此同路径可能出现两次但共享一个勾选；不是最终Changes/Unversioned分组，Checkbox提示明确包含未暂存内容。未实现组/目录三态、消息历史、持久草稿、partial-line、作者选项、输入区拖动、Commit快捷键；状态长文本截断显示，完整诊断交互待补。恢复/取消UI路径和完整多语言/窄窗验收仍需继续，不把首轮可见接线宣称全部Rebased Commit工作流完成。

### 已打开 Rebased 的 Commit 交互实测（2026-10-17）

实际控制用户已打开的 rebased64（PID 1168，GitReview-fixture）；安装 product-info.json 为 Rebased 1.1.20 / 262.10968.SNAPSHOT，与参考源码 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 分别记录，不假设构建一致。截图原图位于 `artifacts/rebased-commit-interaction/`（2100×1500）。执行的是可逆 UI 操作，没有点击 Commit/Push，也没有关闭用户程序。

实测：`amend-loaded.png` 开启 Amend 后增加原提交及其文件组、填入并选中完整消息、聚焦消息框；主操作变为 Amend Commit，旁边推送操作也改为 Amend 文案（窄侧栏原生按钮自身有截断）。`amend-off.png` 恢复原草稿。`group-checked.png` 更改组复选框联动两个文件，Log 原选择不变；随后取消勾选。`file-selected.png` 单击只选文件；Ctrl+D 后 `file-diff.png` 打开 Commit: cache.rs Tab，具有版本标题、差异/包含计数和包含复选框。关闭本次打开的 Diff Tab，恢复关闭 Amend、未勾选状态，保留用户窗口。此实测仅确认当前安装设置，不能推断所有预览配置默认值。

本次重读 NonModalAmendCommitHandler.kt、AmendCommitHandlerImpl.kt（消息替换/恢复时 focus）、AbstractCommitWorkflowHandler.kt::getDefaultCommitActionName/getActionTextWithoutEllipsis、GitCommitAndPushExecutor.kt 和 platform/dvcs-impl/src/com/intellij/dvcs/commit/CommitAndPushExecutor.kt（普通/Amend 两颗操作同步命名）。本地 src/local_view.rs 的 sync_amend_input 仅在消息实际改变时更新、聚焦、全选；加载期间已手动编辑且消息未被替换时保留光标，继续遵循已有异步编辑保护差异。普通成功清理不触发此聚焦。两颗按钮随 Amend 模式切换，含 Loading 状态的命名；新增中英文文案在 src/i18n.rs。

Windows release 构建通过；本地隔离配置/既有测试仓库运行 PID 18588，`local/amend.png` 确认焦点蓝框、全选消息、两颗中文 Amend 按钮，`local/restored.png` 确认关闭恢复；本轮没有 Git 写操作。仍未还原的结构：Changes/Unversioned 合并、组三态、Amend 原提交文件展示，以及 Commit Diff 中的包含控制。不能把这些现象标记为已实现；下一步需深入对应树节点和 Diff inclusion 源码。

### Commit 组与目录包含状态（2026-10-17）

参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`。本次实际读取 shared/src/com/intellij/openapi/vcs/changes/ui/ChangesTree.java::toggleChanges、computeNodeStatus（位于 platform/vcs-impl），以及 src/com/intellij/vcs/commit/PartialCommitInclusionModel.kt：遍历节点下全部对象，包含与未包含并存返回 DONT_CARE；任何未包含项存在时 toggle 全部包含，否则全部排除。原生组勾选对照沿用 artifacts/rebased-commit-interaction/group-checked.png；安装版本 1.1.20 与源码快照分开记录。

本地 local_changes::Model::inclusion_paths 基于实际文件而非可见行收集组/目录范围，目录使用路径组件前缀；commit_selection::Selection::scope_counts/set_scope 按逻辑文件去重、过滤冲突，不影响范围外包含集合或审阅选中项。LocalView 为组与目录增加复选框，部分状态显示横线，点击补齐；展开箭头仍独立于复选框。vendor/gpui-component Checkbox 接入已有 base 的 indeterminate 语义，保留键盘激活、禁用和无障碍 Mixed 状态，不用普通文本模拟按钮。

本轮继续保留四Area分组；同路径不同版本共用一个整文件勾选身份。Changes/Unversioned 合并还需同步处理 HEAD→工作区完整Diff，不能只重标 Area 造成比较基准错误。未实现部分行提交。包含范围只存路径元数据、不读取文件正文；聚合当前按可见节点扫描元数据，未做大树缓存性能验收。

Windows release 首次因 Rems/Pixels 混算编译失败，改为相对中心加像素偏移后构建通过（1m00s）。Linux local_changes 6项、独立group_inclusion测试通过；此前selection原3项也通过。Windows自有PID36124原生验证：mixed.png为单文件勾选后的组横线，all.png为点击混合组后全选，none.png为再次点击清空；collapsed-included.png确认折叠目录勾选仍作用到隐藏文件，并同步同路径另一Area行，Log/预览未改变。artifacts/commit-group-inclusion/finish.ps1核对HEAD/index树/工作区文件hash与启动前一致，assertions.json落盘，随后正常关闭自有窗口。未做滚动测试，未关闭用户Rebased。

### 默认预览与选择保留：统一核对入口（2026-10-17）

用户要求本地变更、Log、已保存更改、推送前预览统一核对默认展示逻辑，避免有文件却停留在“选择文件以查看差异”。本次读取参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ChangeViewDiffRequestProcessor.java::refresh：无显式选择且 showAllChangesForEmptySelection=true（基类默认）时使用全部候选；当前文件仍在候选中则保留，否则取首个候选；无候选才清空。模型刷新还有 forceKeepCurrentFileWhileFocused 与窗口/查看器焦点保护分支，不应一律跳首项。预览回退不等同于树必须选中首项，更不等同于提交自动勾选。

已读 platform/dvcs-impl/src/com/intellij/dvcs/push/ui/PushLogChangesBrowser.kt 全文：继承 AsyncChangesBrowserBase，setCommitsToDisplay 设置 commitNodes 后 rebuildTree，树从 PushLog.collectAllChanges 构造。因此推送预览应继续追踪异步树、Diff processor 接线与提交节点初始选择；此文件本身不能证明 Push 首次进入自动选择哪条提交。TreeHandlerDiffRequestProcessor.kt 的相关搜索命中尚不替代完整生命周期核对。

实施待办：共享默认预览候选/当前身份保留规则，分别核对首次进入、切换提交/仓库、刷新增删、空组、冲突/二进制、异步过期结果及焦点保护；不能因自动展示而自动包含文件或打开/抢占独立Diff Tab。现有WorkingTree比较改动尚在开发，未完成测试，不提前视为已接入新分组。

### 完整工作区比较基础接口（2026-10-17）

已读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 plugins/git4idea/backend/src/status/GitChangesCollector.java::collectChanges/collectStagedUnstagedModifications/reportModified/reportAdded/reportDeleted/reportRename：普通修改 before=HEAD、after=工作区；MM 通过净diff过滤已还原修改；AD不报告；RD报告原路径删除；冲突单列。已读 tests/git4idea/tests/GitChangeProviderVersionedTest.kt 的 staged modification、staged unstaged modification、reverted staged modification、reverted staged addition 测试段。

src/git.rs新增独立WorkingTreeChange元数据及Repository::working_tree_changes/working_tree_patch。它是HEAD→工作区净diff基础接口，尚不是完整GitChangesCollector状态表翻译，不替换现有local_changes。固定本次读取HEAD，前后核对，未出生分支使用不写对象的hash-object -t tree --stdin计算仓库格式空树。复用有界、可取消、literal-pathspec、禁用外部diff/textconv的读取通道。完整模式保留全文上下文，纯重命名/权限元数据从HEAD原路径补全文，不误读暂存区。WorkingTreeChange独立于现有Area，避免把新的比较冒充Unstaged版本供UI使用。

差异/后续：该列表不包含未跟踪文件，冲突可能带Git U状态，预览明确拒绝U；最终收集器仍需从status合并冲突/未跟踪、处理删除后重建和重命名边界，不能直接把此API结果当完整Commit树。Git全局rename检测与上游按status翻译的保留规则可能不同，尚未完成对应边界用例。工作区不是原子快照，外部编辑的并发窗口仍存在；HEAD复验不能防止所有ABA变化。尚未接入Diff Target/侧栏/默认预览。

真实Git测试net_working_tree_*覆盖：暂存+未暂存同一文件净比较；MM还原和AD不显示；index读取前后不变；unborn完整新增；纯rename全文；方括号路径不扩展；取消不写index。Linux3项通过。Windows结果见artifacts/working-tree-review。

Windows原生宿主Git运行3项全部通过（2.19s），stdout/stderr已保存，无用户仓库写入。

### Commit 状态收集器：净变更与索引身份分离（2026-10-17）

再次实际读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GitChangesCollector.java::collectChanges 中 M/A/C/D/U/R/T 分支；GitChangeProviderVersionedTest.kt 的 simultaneous operations、twice renamed 段。上游的逻辑不是直接展示 git diff HEAD 的全部重命名猜测：RD 展示原路径删除，MM才借净diff检查还原，AD省略；TT按冲突处理。twice-renamed测试依赖较新Git工作区重命名支持，不将当前Git未实际产生的状态宣称为已验证。

新增 src/commit_changes.rs：Snapshot::load 固定读取前后HEAD，保存原始 review_versions；File 分为 Tracked(WorkingTreeChange)、Unversioned(Change)、Conflict(Change)。collect按实际status的暂存/工作区组合翻译规则，只有MM用净diff判断是否省略；保留rename原路径、RD映射旧路径、冲突优先，未知组合明确报错。未跟踪和已暂存删除即使路径相同也保留不同对象，未来UI/提交请求不能用单一路径勾选把它们强行合并。保留原始版本是因为删除重命名旧路径时可能还需清理其暂存目标。

此模型尚未接UI与commit_selection；不得将已有按路径共享勾选模型直接接入两个同路径不同类型项。下一步应定义可提交项身份和原始index端点映射，再替换Commit树。原始status和net查询之间仍可能遭外部文件编辑；HEAD复验不提供完整快照隔离。净diff查询当前覆盖整个有界结果，后续可仅对MM候选查询以减少开销。

真实Git commit_collector三项在Linux通过：MM合并、RD旧路径与原始index目标保留、未跟踪分类；已暂存删除+同路径未跟踪对象分开；MM还原/AD省略；真实merge冲突仅进入Conflict，读取前后index保持不变。Windows结果保存到artifacts/commit-collector。

Windows宿主Git的commit_collector三项全部通过（2.71s），stdout/stderr位于artifacts/commit-collector；测试只写自有临时仓库，本轮无GUI变化。

### 新 Commit 项身份与提交请求（2026-10-17）

继续对照同一参考commit，实际读取 GitCheckinEnvironment.kt::stageAndCommit（beforePath/afterPath集合，toRemove排除toAdd）和 GitIndexInfoStagingAreaStateManager.kt::prepareStagingArea/listStagedChanges/restore：staged saver用--no-renames逐路径，未选路径保存并恢复。由此纠正前一节对RD端点处理的推测：RD仅显示/提交原路径删除时，不应擅自清理暂存rename目标；目标不在所选before/after集合，属于需恢复的未选暂存内容。src/commit_changes.rs的误导注释已改正；review_versions保留为来源元数据，不作为自动扩大操作范围的依据。

新增src/commit_inclusion.rs：Identity以Kind(Tracked/Unversioned/Conflict)、path、before/after区分，删除项与同路径未跟踪项互不勾选。刷新仅保留相同身份，HEAD变化清空勾选以免沿用旧提交基准（本地更保守的补充规则）。支持范围计数/包含，冲突不可包含并阻止生成请求。request直接从选中项before/after生成remove/add，add覆盖同路径remove，使用快照HEAD给既有执行器复验；不新增Git写执行流程。

真实Git commit_inclusion三项在Linux通过：RD只提交旧路径删除、rename目标暂存blob恢复且工作区仍缺失；删除项与重建未跟踪项分别提交；两项同时包含add胜出、同HEAD刷新保留身份、新HEAD清空、范围外staged blob与工作区内容都保留。Windows日志在artifacts/commit-inclusion。尚未接LocalView，旧commit_selection仍服务旧UI，待迁移完成后收敛，不能宣称界面已统一。

Windows宿主Git的commit_inclusion三项全部通过（4.71s），输出已保存；仅操作独立临时仓库，本轮没有GUI变化。

### Commit 统一侧栏与默认底部预览接线（2026-10-17）

本轮再次读取参考commit 4aec35256d765c217aa0d662b82ac4e860c396fd 的GitChangesCollector报告before/after版本方法，以及ChangeViewDiffRequestProcessor.refresh的候选回退与保留分支。原生参考沿用artifacts/rebased-commit-interaction，安装1.1.20与源码版本不混同。

LocalView刷新改用commit_changes::Snapshot和commit_inclusion::Inclusion；文件转换到新增Area::WorkingTree，在Model中显示“更改”，Untracked在此侧栏显示“未版本化文件”，冲突保持独立组。范围勾选同时匹配Area与路径，防止同路径删除/重建跨组联动。勾选请求使用快照HEAD，刷新/提交成功行为沿用既有保护。空态改为“没有可提交的更改”，避免净变更为空时谎称index也干净。

Area::WorkingTree接Repository.patch/full_patch及现有Target/Chain，明确走HEAD→工作区，旧Staged/Unstaged API仍保留给其他审阅入口。DiffTab版本标签加入HEAD/工作区。LocalView.preview_target无显式选择时回退首候选，已有有效树选择优先；不自动勾选，不自动打开独立Tab，仅在已有底部预览设置/可见侧栏条件下加载。

Windows release通过（25.77s）。Linux桥接集成测试确认两组、单文件项、WorkingTree身份、全文含staged+unstaged；local_changes六项回归通过。Windows自有PID22908：artifacts/commit-unified-ui/preview-ready.png确认首次打开自动预览、一个sample.rs且含两行修改；included.png/commit.ps1勾选更改组后实际提交，commit-assertions.json断言只提交sample.rs且含未暂存行，notes未纳入。committed.png确认清理后默认展示剩余首项binary的不可预览提示，而非静默跳过文件。再在fixture添加Next review并刷新，选中文件Enter打开full-tab.png，确认全文和新增行；普通SendKeys因前台保护拒绝，改用核验PID/路径后的定向HWND消息，无全局输入。

剩余差异：默认预览尚未统一到Log/Stash/Push；现有LocalView任意notify会使父视图重建预览，刷新焦点保护/滚动位置保留仍待统一处理。新链当前仍按Area分组；独立Tab标题仍为Repository Diff而非参考Commit，统一视图版本标题布局、文件图标/目录缩进等仍有视觉差异。Amend原提交文件组、消息历史、partial-line等未实现。旧commit_selection暂留为既有测试/旧API，后续清理；本轮不宣称完整UI/UX或像素级还原完成，未更新便携包。

### Commit 预览复用与刷新失效（2026-10-17）

再次读取参考4aec352的ChangeViewDiffRequestProcessor.refresh：当前候选有效时保持，同一候选不因无关UI事件切换；模型刷新另有focused viewer保护。本地src/ui.rs去掉LocalView每次notify直接清空local_preview，改按刷新代次、Area、path、old_path、status判断复用。刷新进行中暂保留已有viewer，完成后按新代次读取；错误/无候选清空，隐藏侧栏或切独立Tab仍释放。LocalView提供只借用的preview_change及代次查询，未变化时不再构造Target/克隆Chain列表。DiffTab携带snapshot_revision用于预览失效判断。

Windows release两次构建通过（19.23/18.94s，第二次加入借用检查避免元数据克隆）。artifacts/commit-preview-lifecycle使用隔离fixture、自有PID10864与GIT_TRACE日志：check.ps1先自动首项预览，再组勾选、折叠目录、逐字符输入草稿；实际diff调用数1→1。随后向fixture文件追加内容并点击刷新，调用数1→2且refreshed.png可见新增行、草稿与包含集合保留。switch-file.png验证显式选择notes后预览切换，未因复用停留旧文件。未做滚动自动化。本轮没有执行Commit/Push。

仍待统一：刷新完成时目前重建viewer，未实现参考focused viewer保护与视口保留；无显式树选择时的候选回退尚未抽成Log/Stash/Push共享策略。当前修复确保普通编辑/勾选不触发Git或viewer重建，不扩大宣称所有刷新/焦点状态已还原。内存仅生命周期改进，没有新峰值预算测量。

### Push 默认选中仓库与合并变更（2026-10-17）

已实际读取参考4aec352的platform/dvcs-impl/src/com/intellij/dvcs/push/PushController.java初始化片段：无预选仓库时highlightNodeOrFirst(currentRepository)。PushLog.java::highlightNodeOrFirst选仓库节点或首节点；collectSelectedCommitNodes将仓库节点展开为子CommitNode并逆序收集，collectAllChanges调用CommittedChangesTreeBrowser.zipChanges；onSelectionChanges同时更新文件与详情，详情只在单个commit且未选择仓库节点时启用。因此Push默认不应自动选第一条commit来冒充仓库整体变化。

进一步实际读取CommittedChangesTreeBrowser.java::zipChanges全文与ZipChangesTest.kt的trivial/simple/movement/tricky场景：输入必须从旧到新；按after-path linked multimap累积，遇before-path只弹出该桶第一项，将旧before与新after串联；两端均空时删除（新增再删除抵消）。删除后新增保持独立；重复路径保留碰撞，末尾将删除按before-path重放。上游明确此算法不理解提交拓扑，不等同于任意两个分支tip的净diff。

新增src/combined_changes.rs翻译该规则，保存revision/path元数据，用HashMap索引+FIFO桶保持插入与碰撞顺序；输入100,000变更上限是本地低内存限制，即使抵消后结果很小也按输入计数。没有加载文件正文；调用方后续应按需读before/after版本。来源与Apache-2.0记录加入docs/THIRD-PARTY.txt。Linux四项测试通过，覆盖非连续版本修改、重命名链、增删抵消、删除重建、重复路径碰撞与预算，场景来自已读上游测试。

尚未接Push：当前UI仍要求用户点选commit；需要继续接仓库默认范围、分页/总范围完整性、单commit切换和复合版本Diff Target。不能用当前100条分页的聚合结果冒充所有待推送提交，也不能把此算法改成简单target-tip→source-tip比较。Windows测试日志在artifacts/combined-changes。本轮无GUI变化。

Windows原生combined_changes四项全部通过（0.08s），输出保存于artifacts/combined-changes；本轮未操作用户GUI或仓库。

### Push 合并变更的版本端点（2026-10-17）

实际读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/history/GitChangesParser.java 全文：NEW 没有 before；DELETED 没有 after；MODIFICATION 使用父提交同路径与当前提交同路径；MOVED 使用旧路径的父版本与新路径的当前版本。TYPE_CHANGED 映射 MODIFICATION，COPIED/RENAMED 映射 MOVED。

本地 combined_changes::from_file 将已提交文件状态转换成上述端点，供 zip_changes 合并后按需加载内容。父提交由调用方明确传入，不隐式选择 merge parent；根提交新增允许无父版本，缺失必要父版本、重命名旧路径、非 commit 区域或非完整 OID 均报错，避免把错误端点当空文件。暂不接 UI，也未实现合并提交文件收集策略；不能据此宣称 Push 默认预览完成。Linux 五项测试通过，新增端点测试覆盖根提交新增→重命名→删除的完整折叠和无效输入。

本轮 Windows 原生同组五项测试亦通过（0.08s），日志：artifacts/combined-changes/gitui_review.txt。

### Push 全范围读取与 merge 语义核对（2026-10-17）

本轮实际读取同一参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd 的 GitOutgoingCommitsProvider.java、GitCommit.java、GitCommitRequirements.kt，及 GitHistoryUtils.history、GitLogUtil.readFullDetails、VcsChangesLazilyParsedDetails 的解析/合并片段。路径分别位于 plugins/git4idea/backend/src/{push,history} 与 platform/vcs-log/impl/src/com/intellij/vcs/log/impl。Push 普通目标查询 destination..source 的全部记录；新分支排除指定 remote 可达提交并 max-count=1000。history→readFullDetails 使用 GitCommitRequirements.DEFAULT，即 root changes、-M 和 combined diff -c。getChanges 合并状态后用 parent index 0 构建普通端点，merge 额外包装 source changes。因此不能用当前 Repository::details 的 first-parent 文件集合替代 Push 的 merge 文件集合；combined 状态解析仍需进一步接通。

本地 Repository::outgoing_preview_commits 独立于 UI 页码遍历 outgoing_commits_checked，固定 source OID，并在每页前后检查 remote 引用快照。保留上游新分支 1000 上限及 capped 标志；调用方提供元数据数量预算，超限明确失败而非返回看似完整的部分结果。仅收集提交元数据，不预载正文；尚未接 UI/文件合并。新增真实临时仓库测试生成 105 条提交，验证跨 100 条分页的完整顺序、恰好预算成功、超限拒绝、空范围零预算、引用失效和取消。Linux 通过（0.99s）。源代码版本与已安装 Rebased 的差异沿用文档既有记录。

Windows 原生相同跨页集成测试通过（7.39s）；日志 artifacts/outgoing-preview/repository.txt。仅操作隔离临时仓库，未执行网络推送。

### Push combined 文件收集（2026-10-17）

补读参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/history/GitLogParser.java::PathsParser、GitChangeType.java 全文与 GitLogParserTest.kt 的状态样例。上游从 combined 状态首字符映射变化类型；MOVED 无第二路径时 GitChangesParser 回退到第一路径。普通 rename 则保留旧/新两路径。

本地 Repository::push_commit_changes 用 diff-tree --root -c --name-status -z -M 获取单个提交的元数据（不读文件正文），普通提交沿用 NUL 名称解析，merge 按父数验证 combined 状态并使用首字符，明确不启用 combined-all-paths。再接上一轮 from_file 版本端点。保留上游 merge rename 单路径回退行为，未擅自把它替换为 first-parent rename；复杂 merge rename 的实际 Diff 展示尚待专项验证。已标记 shallow boundary 时拒绝完整预览；调用方仍须确保浅边界元数据准确，不能把缺失父对象伪装成根提交。

真实临时仓库测试覆盖根新增、两父 merge 的双方独有文件排除、双方都修改的解决结果保留、采用某父结果时为空、普通重命名旧/新路径、已知浅边界拒绝。Linux 通过 0.22s。此 API 尚未接 Push UI；仓库范围 zipChanges 与完整文件读取仍待串联。

本轮 Windows 原生 combined 文件集成测试通过（1.72s），日志 artifacts/push-combined-files/repository.txt；没有界面改动。

### Push 仓库选择的聚合查询接线（2026-10-17）

本轮重新读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/dvcs-impl/src/com/intellij/dvcs/push/ui/PushLog.java::collectSelectedCommitNodes/collectChanges/collectAllChanges/getChildNodesByType：仓库子提交逆序传入 zipChanges。按既有已核对的 GitOutgoingCommitsProvider 和 GitCommitRequirements 规则，新增 src/push_preview.rs::load 串联全范围 metadata、逆序逐提交 push_commit_changes 和 zip_changes。返回合并文件端点、提交数、capped 与 remote 引用快照；不保留正文。

浅仓库边界读取从历史 snapshot 提取为共享 Repository::shallow_boundaries；Push 收集前加载，遇到边界拒绝误作根提交，完成后再比对边界集合。收集完成还复验 remote 引用，取消和预算失败不返回部分“完整”结果。保留差异：100,000 输入文件变化的本地内存保护、逐提交 Git 子进程暂未批量优化、查询前后检查无法杜绝外部 ABA 修改。上游新分支 1000 ceiling 不变；尚未接 UI 或复合文件正文读取。

真实临时仓库验证多提交 rename→edit 保留最早 before 与最终 after，add→delete 消失，提交数、预算拒绝、显式 shallow 源提交拒绝与取消。Linux 新测试通过；共享浅边界提取后的两项 shallow 回归亦通过（0.36s）。

本轮 Windows 原生两项聚合/浅边界集成测试通过（2.79s），日志 artifacts/push-preview-snapshot/repository.txt。无 GUI 变化。

### 复合版本全文读取（2026-10-17）

读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/GitContentRevision.java::getContent/getContentAsBytes/loadContent，以及 util/GitFileUtils.kt::getFileContent/addTextConvParameters。上游以文件路径+revision 作为内容身份，后台按需读取；完整 hash 可缓存，分支名不可作为固定缓存内容。GitFileUtils 使用 cat-file revision:path，并根据 AdvancedSettings 支持 filters/textconv。

本地 combined_changes::read_contents 按 before/after 独立 OID 与路径读取原始 blob，保留 None（无该版本）与 Some(空字符串) 的差异。仅接受完整 OID/仓库相对路径，文件名为 literal revision path；正文单次读取受已有 8MiB 限制，最终两侧文本总长也限制为 8MiB，正文不写入持久缓存。二进制、非 UTF-8、丢失 blob、非 blob 对象与取消报错。区别：沿用产品现有禁用外部 filters/textconv 的读取策略，暂不支持上游按文件编码解码；两次读取期间临时字节分配峰值尚需实际测量，最终总长限制不是峰值内存保证。

Linux 真实仓库测试通过（0.14s）：300 行文件跨提交重命名、全文与无末尾换行精确保留、工作区改动不混入、空新增/删除的版本缺失区分、literal 方括号路径及二进制/非 UTF-8/缺失路径拒绝。当前为数据 API，尚未接 DiffTarget/Chain/viewer，不能据此宣称 UI 已支持复合预览。

Windows 原生复合正文集成测试通过（1.13s），日志 artifacts/combined-contents/repository.txt。

### 复合 Diff Target 与共享 Chain（2026-10-17）

沿用已读参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 GitContentRevision 路径+revision 身份及 PushLog zipChanges 端点，本轮重读 ChangeViewDiffRequestProcessor.java 的选择/导航与刷新保留逻辑。src/diff_target.rs 新增 Target::combined/read_combined 和 Chain::combined：完整 before/after 端点参与 Identity；删除和新增即使路径相同也不混为同一个目标。Chain 共享不含正文的端点集合，切换文件重建对应目标，各 Tab 游标独立。禁止复合目标误走旧单提交 patch API/Chain，避免在接线未完成时悄悄展示错误范围。

Linux 三项导航/共享测试通过，涵盖同路径不同端点、跨仓库拒绝、游标独立、越界不变更及旧链回归。真实全文集成测试已改为经 Target::combined/read_combined 读取。尚未改 viewer：DiffTab 仍需按两侧正文构造 DiffFile，版本标题也待核对，现有界面尚无复合入口；不宣称 GUI 已完成。

Windows 三项 Target/Chain 单测通过（0.01s）；Linux 经 Target 读取全文的集成测试通过（0.14s）。

### 复合全文进入 Diff viewer（2026-10-17）

补读参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/vcs-impl/src/com/intellij/openapi/vcs/changes/actions/diff/ChangeDiffRequestProducer.java 的 createSimpleRequest/createContent/getRevisionTitleOrEmpty 相关片段：独立加载两个 ContentRevision；缺失版本为 empty content，实际存在但读取失败不能当空；getRevisionTitleOrEmpty 对缺失版本返回空串、固定版本返回短 revision，但 createSimpleRequest 会通过 getRevisionTitle 回退到 Base/Your version 默认标题。本地继续沿用单侧模式隐藏不存在侧标题的既有设计，未宣称所有标题状态等价。沿用前轮 GitContentRevision 已核对的正文生命周期。

vendor/gpui-component 的 DiffFile::from_sources 新增双侧全文构造，直接创建行映射/配对/全文 hunk，不生成临时 patch。按版本存在性保留 Added/Deleted，路径变化为 Renamed；相同正文保留两侧全文，不伪造改动。src/diff_tab.rs 在后台优先读取复合正文并创建该模型，取消/过期代次仍沿用现有保护；复合标题分别使用真实 before/after hash，缺失侧为空。

差异：使用现有 similar 依赖做行差异，500ms 算法 deadline 降级可能产生更粗的有效差异，未复刻 JetBrains 的差异对齐算法；该时间预算不是整个读取/构造的总耗时保证。正文存储和行索引的峰值内存尚待实际规模测量。当前仍没有 Push UI 入口，因此本轮不能提供复合预览 GUI 截图验收，也不宣称其已可从 Push 默认打开。

Windows 原生 tests/combined_document.rs 通过：CRLF、无末尾换行、行增删计数、纯重命名零文本变化及全文、空新增/删除存在性、无两侧拒绝。日志 artifacts/combined-document/combined_document.txt。release 构建另行记录。

本轮 Windows release 构建通过（1m02s）。原生窗口截图与复合 Push 交互仍待入口接线后验收。

### Push 默认仓库合并预览 UI 接通（2026-10-17）

本轮再次核对参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 PushController.java::processRepositories 初始 highlightNodeOrFirst 与 platform/dvcs-impl/src/com/intellij/dvcs/push/ui/PushLog.java::onSelectionChanges/updateChangesView/updateDetailsPanel：仓库选择驱动合并文件，只有选择具体单提交时才启用提交详情；文件列表加载状态与默认 empty text 分开。

src/push_dialog.rs 默认查询完整 outgoing 聚合结果并选中仓库行；右侧直接出现合并文件，首个文件可直接通过 Show Diff 打开，无需先选提交或文件。单提交改用 push_commit_changes 的 combined 语义，同时显示消息/作者；返回仓库行清除单提交详情，复用已读合并元数据，取消可能过期的单提交加载。所有文件打开使用复合 Chain/DiffTab，下一文件沿用该范围。新增“个提交/没有变更”中英文文本。

Windows release 通过（最终 26.88s）。自有 PID37528，隔离仓库两条 outgoing、本地 bare remote。artifacts/push-aggregate-ui/ready.png：初始仓库选中，仅 renamed.txt，temporary 的新增后删除抵消；diff.png：无文件选择直接 Show Diff，正确比较 89101711 old.txt → 199df99d renamed.txt，含完整三行。single.png：选择旧提交出现 renamed/temporary 与消息作者；rename.png：纯重命名两侧全文、正确 hash/path；added.png：下一文件为新增 temporary、单栏、无末尾换行提示；repository-restored.png：回到合并范围与隐藏提交详情。finish.ps1/assertions.json 验证 HEAD/remote 未变、工作区干净并正常关闭自有进程，用户 Rebased 未关闭。

保留差异及待验收：当前仓库行是现有布局内的独立行，没有完整还原 Rebased 仓库 checkbox/tree/目标行；文件仍是平面列表，缺少源客户端目录与状态图标、行配色；未新增多提交选择。没有执行本轮同条件 Rebased Push 原生对照，只有源码规则与本程序截图；不称像素级或完整 UX 验收完成。当前加载失败仍使用既有统一错误提示，未区分预算/浅边界等具体错误；分页会重收集聚合范围，逐提交子进程与元数据复制仍待优化。10,000 commit 本地聚合预算显式失败，上游新分支 1000 cap 保留；大仓库性能/内存、快速切目标和跨页 GUI 仍待验收。本轮无真实推送，仅 fixture 初始化推送至自有 bare remote。

### 已保存更改刷新保留文件（2026-10-17）

本轮实际读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/vcs-impl/src/com/intellij/openapi/vcs/changes/savedPatches/SavedPatchesChangesBrowser.kt 与 SavedPatchesDiffProcessor.kt：同 patch object 重选不重新加载，异步结果按 currentPatchObject 校验；文件模型按 tag 分组；SavedPatchesDiffPreviewHandler 通过 userObject+tag 选择，空选择的 VCS changes 数据回退所有候选。结合前述 ChangeViewDiffRequestProcessor::refresh 的有效当前项保留/首项回退规则。

本地 src/stash.rs::FileIdentity 使用 group、固定 comparison commit OID、path/old_path；src/stash_view.rs 在 refresh 清理前保存该身份，仅当同 stash OID 刷新后仍存在时恢复对应文件，否则首文件。不会因为 stash@{n} 下标变化而切错 stash，也不会因同名路径把 Index/Working tree 混淆。已加载同一 stash 的重复点击不再清空文件选择/重启 Git 查询。

Windows release 通过（26.09s），自有 PID9824 原生验证：artifacts/stash-selection-refresh/ready.png 首次选择 Working tree a.txt；refreshed.png 选择 Index b.txt 后刷新仍保留；shifted.png 外部脚本仅在隔离 fixture 新增 stash 后，原记录变为 stash@{1}，仍保留 Index b.txt；diff.png 显示 stash@{1} 暂存区 b.txt，仅 index b 增量，不含 working b。finish.ps1/assertions.json 确认 HEAD/原 stash 不变与工作区干净，正常关闭测试进程，未操作用户 Rebased。

保留差异：只修复文件选择，不表示已完成底部预览/焦点保护的统一；加载中再次刷新时仍可能丢失待恢复偏好，快速连续刷新和当前 stash 消失尚待专项验收；同条件 Rebased 原生刷新操作未在本轮验证。Log 现有加载已做首文件及 path+area 恢复，本轮未修改；其更完整的身份与焦点/视口生命周期继续待办。

### Stash 连续刷新与失效回退（2026-10-17）

重新读取同一参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd 的 SavedPatchesChangesBrowser.kt::selectPatchObject（currentPatchObject/currentChangesFuture、异步结果对象复验与空态）并沿用上节 tag+固定身份规则。src/stash_view.rs 增加独立 pending_selection，避免刷新中 entries/rows 清空后第二次刷新覆盖掉原偏好。有效当前选择优先；同 OID 的待恢复文件可跨列表/正文两阶段加载保留；用户显式 select_with_file 替换旧 pending；空列表和成功正文加载清理 pending，过期 generation 不写回。

Windows release 通过（18.93s）。自有 PID11576 原生验证，脚本仅向已核验路径的 HWND 连续发送 8 次刷新点击、间隔 25ms；artifacts/stash-refresh-burst/burst-retained.png 仍选择 Index b.txt。随后仅在隔离 fixture 新增 stash 并删除原选择：fallback.png 回退到剩余 stash 的首个 Working tree a.txt；再移除最后 stash：empty.png 文件清空、Show Diff/Apply/Pop 禁用。check.ps1 断言 HEAD 不变、工作区干净并正常关闭测试进程。此 burst 是实际快速点击验证，不宣称覆盖所有线程调度；显式新选项的竞态仍需进一步验证。

尚未完成默认预览整体专项：Log/Commit 的焦点和视口保护、Stash 底部模式与同条件 Rebased 原生对照仍待。此改动没有自动打开或抢占 Diff Tab，没有滚动自动化。

### Log 默认预览与跨提交文件保留（2026-10-17）

参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd。本轮读取 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/VcsLogChangesBrowser.kt、VcsLogChangeProcessor.kt、VcsLogEditorDiffPreview.kt，以及 platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ChangeViewDiffRequestProcessor.java 的 ChangeWrapper/refresh；进一步读取 platform/vcs-api/vcs-api-core/src/com/intellij/openapi/vcs/changes/Change.java::equals 与 platform/vcs-api/shared/src/com/intellij/openapi/vcs/changes/ChangeListChange.kt::HASHING_STRATEGY。规则：可用当前项优先，失效才首项回退；普通 Change 以可缺失的 before/after 文件路径比较，revision 不参与选择身份，wrapper 另含分组 tag。选择身份不能直接复用包含 hash 的正文缓存身份。

src/preview_selection.rs 集中实现 before/after 路径与 area 匹配、首项及空列表回退；src/ui.rs 的 select_commit 保留原文件偏好，提交详情与本地原始刷新使用该规则。新增/删除的单侧缺失、重命名旧路径及分组均参与匹配，不仅比较展示文件名。该模块不读取或缓存正文。

Linux 定向单测通过，覆盖同路径修改/类型变化、重命名相似度变化、不同旧路径、同名新增/删除及不同 area；Windows release 通过（26.03s）。自有 PID35892 原生截图 artifacts/log-preview-selection/b-current.png、b-older.png 证明跨两个修改提交保留 b.txt 并更新比较内容；root-fallback.png 证明原变更身份不存在时回退首文件 a.txt。assertions.json 确认 HEAD 未变、工作区干净，测试窗口已关闭。

保留差异：area 是本地分组代理，尚非所有上游 tag 的完整模型；加载中再次切提交/刷新时的待恢复偏好、焦点和视口生命周期仍待统一。本轮没有同条件 Rebased 原生窗口对照。底部旧预览的 hunk/header 视觉结构仍与新版独立 Diff Tab 存在差异，未宣称整体默认预览或像素验收完成。

### Log 加载期间保留文件偏好（2026-10-17）

继续对照 4aec35256d765c217aa0d662b82ac4e860c396fd。本轮重读 VcsLogChangeProcessor.kt 的 model-change tracker、VcsLogChangesBrowser.kt 的 model listener/requestRefresh，并读取同目录 VcsLogAsyncChangesTreeModel.kt::updateSelectionState/buildTreeModelSync：待处理 selection state 与已构建 changesState 分开；取消时仅在没有更新状态的情况下放回待处理项，不覆盖新的选择。沿用前节 ChangeWrapper 的路径匹配语义。

本地 src/ui.rs 增加 pending_preview_file：提交详情或历史刷新暂时清空文件模型前保存偏好，下一次切提交优先取有效当前项、其次取待恢复项；成功文件模型消费偏好，实际文件选择覆盖旧偏好，空历史和切换仓库清理。异步结果仍先检查 generation，过期请求不能清理新请求的偏好。只保存一个 Change 元数据，不保留额外正文。

区别：本地单提交状态与 generation 机制不是上游 AtomicReference/树重建实现的直接复制；刷新按钮加载期间仍沿用忽略重复触发的既有行为。本轮处理加载窗口中的偏好丢失，不包含焦点/滚动视口或多提交选择。Windows release 通过（19.09s）；原生快速切换验证结果另记。

原生验证：自有 PID19372，artifacts/log-preview-burst/check.ps1 向已核验的测试窗口连续发送 12 次提交切换，间隔 15ms；burst-retained.png 显示最终 First edit both files 仍预览 b.txt，内容为 base b → first b；refresh-retained.png 刷新后保持同项。assertions.json 确认 HEAD 未变、工作区干净，进程正常关闭。此点击压力检查不证明覆盖所有线程调度；本轮无同条件 Rebased 原生对照，也未做滚动自动化。

### Stash 底部自动预览接线（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd，本轮读取 platform/vcs-impl/src/com/intellij/openapi/vcs/changes/savedPatches/SavedPatchesDiffProcessor.kt：非 editor 使用 TreeHandlerChangesTreeTracker 跟随树变化；handler 提供 selected/all wrappers、userObject/tag 选择恢复，普通比较与 diff-with-local 选择不同 producer。沿用已核对的首项回退规则。

src/stash_view.rs 抽取 preview_target，底部与显式 Show Diff 使用同一个固定 revision/group 的 Target/Chain；src/ui.rs 观察 StashView，底部模式下自动加载当前默认/用户选择文件。复用既有 local_preview 的单一可见 viewer 槽位，切换侧栏或进入独立 Tab 后释放；加载列表时保留 viewer，同固定目标不重复 activate，stash 下标变更只更新展示来源。DiffTab::preview 已有 embedded 分支不主动 focus，因此预览不抢文件树焦点。

保留差异：目前只支持原始 stash 比较，未新增上游 diff-with-local 开关；preview_target 每次构造链仍有元数据复制，后续需随列表共享化优化。底部布局沿用本地统一中央底部区域，尚未做 Rebased 同条件原生布局对照；焦点/视口跨其他入口专项也未全部完成。Windows release 通过（20.06s），原生验收结果另记。

原生验收：自有 PID9364，artifacts/stash-bottom-preview/default-preview.png 证明打开侧栏即默认显示 Working tree a.txt；index-preview.png 单击 Index b.txt 即切换为仅包含 index b 的比较，未混入 working b；refreshed-preview.png 刷新后仍保持该文件与内容，无额外 Diff Tab。check.ps1 断言 HEAD 未变、工作区干净并关闭测试进程；没有执行 Apply/Pop，未操作用户 Rebased。跨分组键盘操作与具体视口位置尚未专项验收。

### Stash 预览共享导航元数据（2026-10-17）

本轮读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/stash/GitStashCache.kt：缓存按 StashId（hash、parentHashes、root）组织，取消的 future 重新加载，失效 stash 清理；GitStashUtils.kt::loadStashChanges 指定 hash 与 parentHashes 查询、使用 FIRST_PARENT 和根提交变化。核对本地 snapshot_query 的 --no-walk=unsorted 与既有 stash 集成测试，父提交查询保留传入顺序，Index/Untracked 分组不需要另行排序。

src/stash_view.rs 添加每个当前分组的按需 Chain 缓存。首次打开该分组创建一次 metadata，后续 preview_target 通过 fork_for 共享 Arc 数据和独立游标；begin 清理当前列表缓存，新的 contents 重建空槽。底部预览与 Show Diff 共同复用。src/ui.rs 通过实体 update 调用该按需入口，不为缓存更新发送 notify，避免渲染通知循环。

与上游差异：只缓存当前已加载 stash 的分组导航，不采用全 stash 预加载；无正文缓存。现有 contents 文件列表与首次导航仍各有一份元数据，本轮消除的是每次 render 的整组复制，不宣称全程序零复制或完成内存验收。Linux 3 项 Chain 测试通过（包含 10,000 文件/100 游标共享、独立游标、释放、跨仓库/版本拒绝）；stash 含 Index/Untracked 的真实仓库集成测试通过（0.36s）。Windows release 通过（19.92s）。

Windows 自有 PID29656 原生回归完成：artifacts/stash-shared-preview 保存默认文件、切换 Index b.txt 及刷新截图；refreshed-preview.png 内容为 base b → index b、版本标题正确。HEAD/工作区断言通过，测试进程已关闭。未开展大规模进程内存测量，本轮启动样本不用于证明内存收益；同条件 Rebased 原生对照仍待。

### Log 多选前置：逐父状态合并（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd，本轮读取 VcsLogGraphTable.java 选择相关片段、SelectionSnapshot.kt、CommitSelectionImpl.kt、VcsLogCommitSelectionListenerForDiff.kt：选择以 commit identity 恢复、详情按需读取、空选择/加载/错误与成功状态分离。Ctrl/Shift 的 Swing 默认实现尚未深入，不能将此当成完整手势验收。

读取 platform/vcs-log/impl/src/com/intellij/vcs/log/util/VcsLogUtil.java::collectChanges：选中详情逆序取 getChanges 后 zipChanges。进一步读取 plugins/git4idea/backend/src/log/GitLogProvider.kt::readFullDetails，实际为 DIFF_TO_PARENTS，区别于 Push 默认 combined。读取 VcsChangesLazilyParsedDetails.java::parseMergedChanges/getMergedStatusInfo：逐父状态通过 VcsStatusMerger 合并，再按 parent 0 转换端点。VcsStatusMerger.kt 完整规则：路径取修改/新增/删除的 first、移动的 second，只保留每个父比较共有的路径；同类型保留首项，重命名来源不同退化为修改；混合类型含删除则删除，否则修改。该规则不是直接取 combined status[0]。读取 VcsStatusMergerTest.kt 全文；其中 renamed helper 的参数次序与命名不直观，本地按实际 first/second 规则构造显式 old_path/path 案例，不照抄误导性的测试名字。

新增 src/log_changes.rs::merge_parents 翻译上述元数据合并，保留第一父路径顺序；类型变化按 modification，移动要求 old_path，有输入总数上限且不缓存正文。来源和许可证记入 THIRD-PARTY。4 项 Linux 测试通过：三父交集、空父列表、混合状态、同源/异源 rename、不同目的路径、无效移动与空输入。

本轮仍为数据层前置，尚未接逐父 Git 查询、Log 多选模型/右侧多详情/合并 Diff，更未实现 Squash。不能用既有 Push 的 first-character parser 代替此模块。下一步接真实 merge Git 查询与多选聚合，再接 Ctrl/Shift、右键集合及对应详情。没有可见 UI 变化，因此本轮不提供新窗口截图。

Windows 原生 4 项 log_changes 测试通过（0.01s），日志 artifacts/log-parent-status/gitui_review.txt。未运行写操作或新 GUI 功能。

### Log 多选真实 Git 查询与聚合（2026-10-17）

继续参考 4aec35256d765c217aa0d662b82ac4e860c396fd，本轮重读 GitLogProvider.kt::readFullDetails（DIFF_TO_PARENTS）及 VcsChangesLazilyParsedDetails.java::parseMergedChanges（合并状态按 parent 0 转换端点）；沿用前节 VcsLogUtil.collectChanges 逆序 zip 规则。

src/git.rs::log_commit_changes 对每个父 OID 单独 diff-tree -r --name-status -z --find-renames，根使用 --root；禁用 ext-diff/textconv，逐父元数据总量上限 100,000。调用 log_changes::merge_parents 后，以第一个父版本转换 before、提交版本转换 after。src/log_changes.rs::load 接受显示顺序的完整 OID 集合，拒绝重复/无效/超 HISTORY_MAX 选择，重新读取固定提交与父链，逆序收集并 zip；空选择返回空，取消返回错误。前后复验 shallow 状态，选中边界拒绝而不作为根提交；不依赖可移动 ref，因此普通 HEAD/ref 移动不使固定选择失效。

真实临时 Git 测试覆盖根新增、普通 rename、merge 冲突解决的共有文件、只改一个父比较的路径排除及干净 merge 无合并变更；不连续选中 rename/add 与 edit/delete，确认未选 gap 提交独有文件未加入，temporary 新增后删除抵消、端点 base old → latest renamed 正确且读取完整正文。覆盖空/重复/非法 OID、浅边界、取消及 HEAD/工作区未改变。Linux 两项新集成测试通过（log_ 过滤还运行既有 author catalog 测试，共 3 项，0.35s）。

保留差异：逐父逐提交子进程尚未批处理；元数据数量预算与输出上限不是最终进程峰值承诺。未保留上游 MergedChange 的所有 parent source changes，当前输出服务于聚合 Diff，未来单 merge 的逐父分组要另接；根变化当前总显示，上游 shouldIncludeRootChanges 设置未复刻。消息多详情尚未接，复杂合并重命名还需真实 Git 专项，GUI/多选手势/Squash 均未完成。

Windows 原生上述 3 项集成测试通过（2.99s），日志 artifacts/log-selection-query/repository.txt。本轮没有修改可见 UI，未更新便携包。

### Log 的 OID 多选状态模型（2026-10-17）

继续参考 4aec35256d765c217aa0d662b82ac4e860c396fd，本轮读取 VcsLogGraphTable.java::changeSelection/shouldChangeSelect 与 JBTable.java 相关鼠标处理；表格在 cell controller 允许选择时委托 superclass 的 toggle/extend。结合已读 SelectionSnapshot.kt 按 commit identity 在可见图恢复规则。补查 OpenJDK JTable.changeSelection 的四种公开契约（https://raw.githubusercontent.com/openjdk/jdk21u/master/src/java.desktop/share/classes/javax/swing/JTable.java）：替换、单项切换、锚点范围替换，以及将锚点选中状态应用到范围。此处仅按 API 行为独立实现 OID 模型，未拷贝 JDK 实现代码；JDK master 来源不是已安装 Rebased JBR 的精确版本。

src/log_selection.rs::Selection 用 OID 集合保存选择，anchor/lead 分开；按当前显示顺序导出集合供 log_changes::load。reconcile 对刷新/重排/过滤/换页只保留仍可见身份，空集合不会自动选择首行，初次默认首项由调用层明确处理。键盘 step 共享范围操作；context 在已有选中行保留集合、未选中行替换为单项，符合本项目要求，但右键实际平台 event 路由仍需原生核对。

4 项 Linux 单测通过：不连续切换、范围缩小、Ctrl+Shift 在未选锚点上移除范围、键盘 lead、右键保留/替换，以及重排/过滤/分页/空态/越界。当前模型尚未接 Review，避免只画多选而右侧仍展示最后一项；下一步一次性接选择模型、聚合文件/详情、Diff 和命令可用性。无 UI 新行为，本轮没有窗口验收。

Windows 原生 4 项选择模型测试通过（0.01s），日志 artifacts/log-selection-model/gitui_review.txt。尚无真实 Ctrl/Shift/右键窗口验收。

### Log 多选 UI 与合并 Diff 首次接线（2026-10-17）

本轮在 4aec35256d765c217aa0d662b82ac4e860c396fd 继续读取 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/VcsLogCommitSelectionListenerForDetails.kt::onDetailsLoaded：重建多提交详情，最多 50 条，超过显示提示，随后加载 refs/包含分支等。读取 platform/vcs-impl/src/com/intellij/openapi/vcs/history/actions/CopyRevisionNumberAction.java 与 VcsLogPanel.kt 相关数据入口：选择按显示顺序导出，复制操作逆序，以空格连接完整 hash。此前猜测的换行分隔已在本轮接线时修正。

src/ui.rs 将 log_selection 与 log_changes 接入鼠标 Ctrl/Shift、Shift+上下、右键集合和刷新身份恢复；选择变化清空旧文件/详情，generation 检查阻止过期结果覆盖。单选继续既有详情路径，多选后台读取聚合端点，首 50 条提交消息/作者独立呈现，最终展示文本总长限制 8MiB。文件列表和底部/独立 Tab 均使用同一复合 Chain；首项默认预览、切文件沿该集合导航，旧 Tab 保留固定端点。多选时 selected 单提交字段为空，Checkout 禁用；菜单新增真实 Copy Revision Number，快捷键同步采用反向空格格式。中英文新增提示同步。

首轮 Windows release（28.57s）与自有 PID37708 验证 Shift+Down、复制完整旧→新 hash、刷新集合保持、选择 b.txt 后底部/独立 Tab 为 base→最新两次增量。截图 artifacts/log-multiselect-ui；HEAD/工作区断言通过且关闭窗口。首次键盘尝试因非前台被脚本保护拒绝，随后仅在已核验窗口可见位置实际点击取得焦点，未向其他程序发送按键。该轮截图发现旧的 selected.is_none 早返回导致多详情区域不显示；本轮随后修正并再次 release 构建（20.11s），补充详情/Ctrl/右键验证结果另记。

保留差异及待验收：多详情暂未逐条接 refs、包含分支、外部状态和父链接；50 条以外只提示，不加载正文；消息查询目前复用 details 会额外查询单提交文件，需分离 metadata API。连续多选圆角仍逐行绘制；未完成复杂过滤/跨页/快速查询竞态、空选择所有路径和超上限实际窗口测试。Ctrl+Shift/键盘边界与同条件 Rebased 原生对照仍待。Squash、Compare Versions/Parent/Child 后续仍在计划内，不把此轮标为整体功能完成。

修正后的自有 PID35848 原生验证通过，artifacts/log-multiselect-details/two-selected.png 显示多详情区域与合并预览；single-after-toggle/two-after-toggle 及剪贴板断言验证 Ctrl 点击移除/加入；context-two.png 显示保留两行、复制入口与禁用 Checkout，关闭菜单后复制集合未变。refresh-two、b-preview、combined-tab 验证刷新、文件切换与完整复合 Diff 端点。check.ps1 断言 HEAD/工作区未变，测试窗口正常关闭，未操作用户 Rebased。没有滚动自动化；详情第二项在当前底部模式高度下部分出屏，未宣称每条卡片全部原生可见验收。

### 多选详情消息查询与 refs 复用（2026-10-17）

本轮重读参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 GitLogProvider.kt::readMetadata/readFullDetails：metadata 通过 collectMetadata，文件变更另走 full details；VcsLogCommitSelectionListenerForDetails.kt::onDetailsLoaded 对每条卡片按 commit ID 取 refs 并 setRefs。

src/git.rs 抽出 commit_metadata，仅 git show --no-patch 读取作者/提交者/日期/完整消息；为兼容现有 presentation 返回 Details，但 files 明确为“未加载”，不能当作没有文件变化。details 复用消息结果再查询文件，浅边界仍仅消息。src/ui.rs 的多详情改用 metadata，不再为首 50 条每条多跑一次文件比较；每项保存本次固定提交的 refs。抽出 commit_refs，单/多选共享分支与 tag 分组、叠放图标、100 项显示上限及转义，不复制一套 UI。多选 metadata 链接沿用原详情蓝色。顺带补齐 move_commit 的 history_loading guard，防止普通上下键在历史模型刷新时启动并发选择查询。

新增真实仓库测试：metadata 与完整 details 的消息/身份/时间/base 一致且不读取文件；构造缺失比较父对象时 metadata 仍可读取已有提交文本，而完整 details 正确失败；浅边界、取消和只读状态断言通过。Linux 通过（0.18s），Windows 原生通过（1.50s），日志 artifacts/log-multiselect-refs/repository.txt。Windows release 通过（21.18s）。

剩余：多详情包含分支与父链接仍未接，metadata 目前逐提交调用、未批处理；共享 Details 类型的“files 未加载”靠明确 API 约定，后续可独立类型以进一步避免误用。未做大规模 CPU/内存数据，不宣称量化收益。原生 refs 界面验收结果另记。

原生验收：首次自有 PID4060 截图 artifacts/log-multiselect-refs/two-selected.png 验证 refs 已按提交分组，但发现 base TextView 默认前景黑色；随后补齐与单选相同的 foreground/link/selection，最终 release 通过（20.88s）。自有 PID31096 的 artifacts/log-multiselect-refs-final/two-selected.png：最新卡片 HEAD/main/review-tag，旧卡片 review-base，各自 hash/消息/作者日期及蓝色邮箱清晰正确。fixture profile 将 files-details-ratio 设为 0.5，完整看到两条卡片，不进行滚动自动化；默认比例未修改。两次脚本均通过复制顺序和 HEAD/工作区只读断言并关闭测试进程。未操作用户 Rebased，仍无同条件原生参考对照。

### 默认预览统一规则复核（2026-10-17）

针对用户要求“先结合源码了解，后续统一”，本轮重读参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ChangeViewDiffRequestProcessor.java::refresh、ChangesViewDiffPreviewProcessor.kt，以及 platform/dvcs-impl/src/com/intellij/dvcs/push/ui/PushLog.java::highlightNodeOrFirst/collectSelectedCommitNodes/collectAllChanges/updateDetailsPanel。规则：先保留候选中的当前文件，找不到才取候选首项，无候选才清空；空选择是否退回全部候选由场景决定。Local Changes 明确关闭空选择取全部，并在 viewer 获焦时保护当前文件，不能把“全部场景无条件选第一项”当作上游逻辑。Push 默认仓库节点对应其提交合并变化；具体单提交选择才显示该提交详情。默认预览不等于自动创建独立 Diff Tab。

本地对应 src/preview_selection.rs、src/ui.rs、src/commit_view.rs、src/stash_view.rs 与 src/push_dialog.rs。现有 Push show_file_diff 已在无显式文件时回退索引 0，但这只是打开 Diff 动作的回退，不等价于文件列表已默认高亮或内嵌正文已自动展示；后续需分别验收。统一时保留各入口范围、分组、用户选择和焦点语义，按需读取当前正文，不预加载全部文件。该轮只核对源码与记录验收规则，未修改 UI、未新增原生验证；源码版本与已安装 Rebased 1.1.20 / 262.10968.SNAPSHOT 的差异仍保留。

### 多选提交的包含分支详情（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd；本轮实际读取 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/VcsLogCommitSelectionListenerForDetails.kt::ContainingBranchesAsyncLoader 与 platform/vcs-log/impl/src/com/intellij/vcs/log/data/ContainingBranchesGetter.kt。上游按 CommitId 将异步分支结果分发到各卡片，使用 SequentialLimitedLifoExecutor（容量 10）和最大 2000 项缓存，分支 checksum 变化清缓存并防止旧结果写入。另读 ui/details/commit/CommitDetailsPanel.kt::ContainingBranchesPanel：展开显示逐行分支文本，收起清空展开正文，宽度变化重算摘要。

src/containing_branches.rs 增加 pending 卡片和 Batch；src/ui.rs 多选详情每项带独立实体，消息就绪后由一个串行 worker 查询包含分支，逐项通知。切换选择/仓库释放 Batch 并取消，弱实体防止旧结果写入替代卡片；最多与既有 50 条详情相同范围。分支结果按 Vec/String capacity 总计受 8 MiB 保留预算约束，超限后不继续新查询。该预算不是整个程序峰值（单次查询临时分配、Git/GPU 不计入）。

保留差异：使用当前选择 FIFO 串行及随选择释放，不复刻全局 2000 项缓存/LIFO；暂未实现上游分支 checksum 在查询中变化时的整批复验，外部 refs 变化仍依赖刷新；父链接导航仍待。现有包含分支查询排除 tag/远端 symref，按真实 ancestry 判断。

Windows release 增量检查通过（0.44s）。自有 PID36740 原生截图 artifacts/log-containing-selection/two-selected.png：最新卡片为 HEAD/main 共 2 个，旧卡片 HEAD/main/review-base 共 3 个；review-tag 仅在 refs 装饰中。trace.jsonl 的多选两次 --contains 分别为 10:54:23.289591–.324414Z、10:54:23.395249–.429858Z，没有重叠；不把两项 fixture 视为大规模性能验收。复制顺序、HEAD 和工作区只读断言通过，测试窗口已关闭。

补充展开验收：同 fixture 自有 PID29600，expanded.png 显示旧提交的 HEAD、main、review-base 逐行完整文本，深色主题前景正常；collapsed.png 确认收起恢复截断摘要和“显示全部”，另一卡片不受影响。复制顺序和只读断言再次通过，进程已关闭。query-assertions.json 保存首轮 trace 的非重叠断言；此补测没有重跑 trace，不作更大并发范围推断。

### Squash 重放计划与消息草稿（2026-10-17）

参考仍为 4aec35256d765c217aa0d662b82ac4e860c396fd。本轮完整读取 GitSquashOperation.kt、GitCommitEditingEditorHandler.kt、GitCommitEditingOperation.kt、GitCommitEditingOperationResult.kt、GitSquashedCommitsMessage.kt 与 GitSquashOperationTest.kt/GitSquashedCommitsMessageTest.kt；深入 GitRebaseTodoModel.kt::unite/moveElements/Type、GitRebaseTodoModelConverter.kt、GitInteractiveRebaseEditorHandler.java::processModel、InMemoryRebaseOperations.kt::squash/executeInMemoryCommitModification，以及 GitRebaseParams.kt::editCommits。路径均位于 plugins/git4idea/backend/src 或对应 tests/git4idea，沿用上文入口。

确认：选中集合先按 todo 原序匹配；unite 以最旧选择为根，把其余选中项移到根后，未选项保留相对顺序并在合并组之后重放。根 reword，子项 fixup，不是 Git squash 命令逐次询问消息；编辑器保存旧消息到新消息映射。In-Memory 使用相同 model，且要求匹配数量完整。完整操作测试包括最近连续、中间连续、不连续选择和 Undo；Undo 要求 HEAD 未移动、首个重写结果不在受保护远端分支内，执行 reset --keep 而非 hard。根/merge/detached 的 UI 可用性、fallback 选择和进行中状态规则仍需继续核对，不据本次计划生成声明已支持。

src/squash_plan.rs 新增有上限的纯计划：完整 OID、线性重放序列（旧→新）及无重复的选择集合；按上游顺序生成 reword/fixup/pick，todo 不嵌入用户消息或 subject。draft 按输入顺序去重完整消息，仅当普通目标 subject 同时被选中时剥离 fixup!/squash!/amend! subject，三换行连接；不加载正文，8 MiB 输入/输出限制。Plan 不是可执行授权：拓扑、HEAD/protected refs 复验、恢复记录和操作队列尚未接。当前针对普通 pick 序列，没有复刻已有 autosquash 分组/update-ref 的通用 todo 编辑；未来接真实 Git sequence editor 时必须核对实际 todo 与完整快照，不可直接覆盖未知指令。本地仅规划 Git fallback，未实现 in-memory 对象重写；保留差异必须在最终交付说明。许可证记录在 THIRD-PARTY.txt。

Linux 两项模型/消息测试通过；tests/editor.rs 新增真实 Git 四场景，复用现有 Server/Document 编辑器桥接：最近连续、中间连续、不连续选择，逐项核对重写后消息顺序、Squash 单提交的实际变更文件、最终 tree、工作区状态；冲突场景确认 unmerged index 与 rebase 状态存在，并验证 abort 后 HEAD/index/tree/worktree 恢复。通过（1.21s）。测试明确关闭 autosquash/update-refs，以隔离纯计划规则；这不代表产品已决定忽略用户配置。没有新增 GUI 按钮，也没有执行用户仓库写操作。

Windows 原生同一真实 Git 四场景通过（8.99s），两项模型/消息测试通过（0.01s）；日志与启动脚本在 artifacts/squash-plan。测试仅使用独立临时仓库，编辑器 helper 路径包含空格/中文；没有启动 GUI 或更新便携包。

### Squash 只读准备：HEAD 路径与保护分支（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd。本轮读取 plugins/git4idea/backend/src/rebase/log/GitMultipleCommitEditingAction.kt，GitCommitEditingActionBase.kt 的 checkHeadLinearHistory、checkCommitsCanBeEdited、lastCheckCommitsEditingAvailability、checkNotRebaseDuringRebase/getProhibitedStateMessage；重读 squash/GitSquashLogAction.kt，并读取 backend/src/GitProtectedBranches.kt、GitRebaseProcess.java 的 published commit gate。确认从 HEAD 向最旧选择行走时，先检查父数为 1，再移除待匹配选择；因此该 Log action 路径遇到选中根也会失败，不能仅凭后面的根确认弹窗推断可用。最旧选择的包含分支足以判断整组选中是否进入保护远端分支；保护匹配针对去掉 remote 名称的分支名称。基础 action 的默认 rebase-during-rebase 策略是 Allow，此事实不等价于 Git 执行层支持任意进行中状态。

src/squash_check.rs::prepare 以完整 expected HEAD 和选中 OID 获取有上限的第一父路径，拒绝路径内 merge/root、浅边界、缺失选择、过期 HEAD 与保护远端祖先。返回 base、按旧到新排列的选中项、squash_plan、完整 HEAD branch（detached 为 None）和当前 operation state。最大路径 10,000；仅在到最旧选择前检查，因此更早的 merge 可作为 base，不无条件禁止整个仓库有 merge。保护策略复用 published_commits::Protection：默认 main/master 或调用者已解析的 Explicit 分支集；查询失败不作为安全结论。读取前后复验 HEAD、symbolic branch、heads/remotes refs 与 shallow 状态。

该 Prepared 只是消息弹窗准备数据，不是持久写入许可。执行器尚未接入，必须在 operation_guard 内重新准备、比较原快照并处理操作状态/本地修改。此处保留 detached 信息且不自行禁止，不改变工作区/index；正在进行的操作只报告，不宣称已决定完整允许规则。使用 Git first-parent 查询替代上游已加载图；未实现自定义 Java 正则设置 UI，Explicit 由现有设置层调用者提供。跨进程检查仍非原子，外部 ref 改动的极短竞争窗口无法由本应用队列消除。

Linux 两项真实仓库测试通过（1.15s）：不连续选择计划与 base，过期 HEAD，选中根，保护远端包含选择、普通远端允许、自定义保护集合、保护 tip 早于最旧选择允许，detached 保留、浅边界与取消；另覆盖选中不在 HEAD、路径内 merge 拒绝、merge 只在更早 base 允许。核对原 HEAD/index/已暂存与未暂存修改不受只读查询影响。没有新增 GUI 或用户仓库写操作。

Windows 原生两项 Squash 准备集成测试通过（9.18s），日志 artifacts/squash-check/repository.txt。仍为只读后端准备，无新增可见 UI，未更新便携包。

### Squash 实际 todo 与准备快照对照（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd。本轮读取 plugins/git4idea/backend/src/rebase/GitRebaseEntry.kt、GitInteractiveRebaseFile.java、GitInteractiveRebaseEditorHandler.java::editCommits/handleInteractiveEditor、GitRebaseEditorService.kt，以及 interactive/GitRewordedCommitMessageProvider.kt。确认 pick/p 等别名；文件加载跳过空行/注释，未知 action 转入普通编辑器而非默默删除；取消通过编辑器返回失败。reword 映射另存，按 rebase onto 与原消息前缀匹配，成功匹配才消耗，未匹配交给普通消息编辑器。后者的持久化与继续流程本轮尚未实现。

src/squash_plan.rs::rewrite_todo 现在是唯一公开的 todo 生成入口：Plan 用索引排列保存原始顺序，避免再拷贝一份完整 OID 字符串；解析实际 Git todo 时验证每一项对应准备范围且顺序/数量完整。允许 pick/p、唯一十六进制缩写、CRLF 与调用者传入的实际注释串；用排序后二分定位/相邻检查判定缩写唯一性，不做每项全表扫描。输入与 editor Document 的 1 MiB 上限一致。subject 不作为指令执行或匹配依据，输出只含 action/full OID。

保留差异：目前仅接受普通 pick 序列，autosquash 既有 unite 分组、update-ref、exec/label 等显式拒绝，尚未接上游通用 todo 模型/普通编辑器 fallback，不宣称这些配置已支持。实际 comment string 的配置解析仍需由执行器接入，不能直接把 auto 当作注释串。该校验不能替代执行前 HEAD/protected refs/state 检查。消息映射需按上游 onto/原消息身份和生命周期后续接入，不可对任何 commit editor 请求一律写 Squash 消息。

Linux 三项模型测试通过；真实 Git 测试改为通过 rewrite_todo 再 Document.save，最近/中间/不连续/冲突中止回归通过。另新增实际 sequence editor 拒绝测试，分别构造缺提交、顺序交换、额外 exec，拒绝后不写 todo，Document Drop 回复 exit 1，Git 返回失败且不残留 rebase-merge；原 HEAD/index/worktree/文件内容保持，操作取消 token 未被误触发。两项集成测试共 1.17s。未新增 GUI。

Windows 原生同组验证通过：两个真实 Git 集成测试（含四种重放/恢复与三种拒绝场景）13.23s，三项模型测试 0.01s；日志 artifacts/squash-todo。未更新便携包，无新增 GUI 验收。

### Squash reword 消息持久映射（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd。本轮重读 plugins/git4idea/backend/src/rebase/interactive/GitRewordedCommitMessageProvider.kt 的 save/getRewordedCommitMessage/checkRebaseOnto 以及 GitInteractiveRebaseEditorHandler.java 已读接线：映射按 onto 匹配，原消息前缀一致才消费，未匹配进入普通消息编辑器。上游使用项目 workspace 持久状态，保留 currentCommit 游标。

新增 src/squash_message.rs::install/apply：当前 Squash 单一 reword 映射持久化于 worktree gitdir/rebase-merge/gitui-squash-message；按 onto、orig-head 和原消息前缀匹配，sequence 请求永不按消息处理。Document 保存 canonical gitdir，apply 从 document 所属仓库获取记录，调用方不能传另一个仓库记录覆盖当前编辑器。两个消息各限 1 MiB，二进制长度头限制分配，拒绝未知版本/损坏/额外字节/NUL；create_new 不覆盖已存在记录。替换内容与普通 Document.save 共用外部修改和取消检查。成功/中止由 Git 清理 rebase 目录，同时清理映射。

一次性消费标记在回应 editor 前写入并 sync；保存失败尽力撤回标记。标记已存在时不再自动应用，新请求仍可手工保存。与上游差异：本地绑定 orig-head 更严格；不用项目全局 workspace，而随单 worktree 的 rebase 生命周期存储；空原消息不进行自动前缀匹配以免任意消息都命中，交普通编辑器。若进程在占用标记后、响应 editor 前崩溃，保留标记并回退手工编辑，不推测是否已完成；这不是跨文件事务或断电原子性承诺。持久层错误需在执行器/UI 接线时展示，不能静默当作成功。

Linux 三项 editor 集成测试通过（1.21s）：正常四种 Squash 重放/冲突中止场景现使用磁盘消息映射，而非直接写死新消息；额外覆盖重复安装拒绝、原消息/onto/orig-head 不匹配、已消费、损坏记录、外部编辑冲突；新 helper/document 再次请求同原消息不重复消费，仍可手工完成。此处验证磁盘重读，不宣称完整应用重启/真实冲突 continue UI 已接通。执行器、既有 Continue 路由及持久错误反馈仍待；没有新增可见 UI。

Windows 原生三项 editor 集成测试通过（15.62s），三项计划测试通过（0.01s），证据 artifacts/squash-message。额外断言 sequence 请求不消费消息映射，以及 Git 完成/中止后映射所在目录确实被清理。未新增 GUI 或更新便携包。

### Squash Git fallback 执行器（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd。本轮重读 plugins/git4idea/backend/src/rebase/log/GitCommitEditingOperation.kt，读取 rebase/GitRebaseProcess.java::rebase/doRebase（saveDirtyRootsInitially、逐仓库重放、成功/未开始时加载保存内容、状态保存）、rebase/GitRebaser.java 的 rebase/continue/abort 接线及 commands/GitImpl.java::rebase/REBASE_CONFIG_PARAMS。上游在 Git fallback 使用 interactive rebase 与 editor handler，配置 core.commentChar 为 SOH；结束后刷新 repository 并以旧/新 HEAD 构造结果，不使用 reset --soft 代替重放。

src/squash_operation.rs::execute 串联 prepare、实际 todo 校验与消息映射：在 common-dir operation_guard 内重新 prepare，比较弹窗原 HEAD/branch/base/plan，检查进行中状态、冲突与未解决 index recovery；使用经验证的 helper 路径和本次独立 Server/Requests。一个 editor worker 处理 sequence 后安装持久消息，再回应 Git；commit editor 优先匹配映射，未匹配可转交调用者的普通编辑器 Requests。src/editor.rs::forward 只转发请求及响应，不将先前读入的旧文本写回覆盖 UI 的保存。进程结束关闭路由，等待 worker，重新读取真实 HEAD/state，即使取消也不跳过恢复状态读取。Report 保留 old_head/base/branch、命令结果和独立 editor_error，succeeded 同时要求命令成功、无中断、无 editor_error、实际操作结束且无冲突。

阶段性差异仍明确保留：尚未接上游 saveDirtyRootsInitially 的本地修改保存/恢复，因此当前后端明确拒绝 tracked dirty state，不能视为最终能力；没有把它改为强制丢弃或自动提交。已有进行中操作须由 Continue/Abort 路由处理，不新起 Squash。使用纯 Git fallback，尚无 in-memory 后端；autosquash/update-ref 等 todo 动作仍由校验明确拒绝，未通过覆盖配置隐藏差异。普通 editor 转发已接接口，尚未完成实际 GUI 路由验收。Undo、focus commit、持久 stash ownership、继续操作时的自动映射和 UI 入口仍待。

新增真实临时仓库 executor 测试：最近连续/中间连续/不连续成功顺序与最终 tree；冲突返回真实 RebaseMerge/unmerged 状态，记录保留，测试再显式 abort 恢复；准备后 HEAD 移动、同 HEAD 切分支、暂存修改、更新保护远端的拒绝保持 HEAD/index/worktree。首轮 Linux 八场景通过（4.79s）；随后补 pre-rebase hook 失败与预取消，最终结果另记。此前测试手工启动 Git 的路径保留，新增测试直接调用产品 execute。没有打开 GUI 或用户仓库写操作。

最终 Linux executor 十场景通过（5.89s）。Windows 原生四项 editor 集成回归通过（59.77s，包含 executor 十场景、消息映射、重放恢复和 todo 拒绝），日志 artifacts/squash-executor。仅临时仓库，无可见 UI 改动，便携包未更新。

### Squash Undo 后端（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd，本轮重读 plugins/git4idea/backend/src/rebase/log/GitCommitEditingOperationResult.kt::Complete：以 base..newHead 最旧项作为 firstChangedHash，HEAD 未移动且 firstChangedHash 不在保护远端分支才允许 Undo；执行 GitResetMode.KEEP 并刷新 repository。

src/squash_undo.rs 新增 Undo::capture/check/execute：只从成功 Report 捕获 old/new HEAD、base 所定范围的首个重写提交、原分支及 worktree gitdir；通知可调用只读 check，execute 在 common-dir guard 内重复核对。保护分支查询复用从 squash_check 抽出的 protected_containing，避免 Squash 与 Undo 两套默认/显式策略漂移；HEAD/branch/ref inventory 在检查前后复验。执行 reset --keep，失败/取消后也重新读取真实 HEAD/state；只有命令成功且实际回到 oldHead、操作无冲突才报告成功。

本地保留差异：额外绑定 worktree 与原分支，避免同 OID 切到另一分支/仓库后移动错误引用；进行中操作与未解决 index recovery 明确拒绝。上游 Undo 本身不是跨重启持久操作，本地同样只创建内存 action，没有在启动时自动执行或重建撤销。通知按钮/定位尚未接。reset --keep 仅遵循 Git 的本地内容保留语义，不能宣称保持 staged/unstaged 划分；测试实证 staged 修改变为 unstaged，而文件内容保留。外部 Git 写入不受本应用队列约束，前后复验不是跨进程原子事务。

Linux 真实 Git Undo 八场景通过（7.40s）：先执行不连续 Squash，再撤销并精确比较原始全部提交 OID 顺序及 tree；本地 unstaged/staged/未跟踪内容保留，已消费 Undo 再调用拒绝；保护远端包含首个重写结果、HEAD 移动、同 OID 切分支、其他仓库拒绝；index.lock 导致 reset 失败，返回未成功且 HEAD 保持。保护检查提取后，既有两项 Squash preparation 测试回归通过（1.16s）。没有可见 UI 改动。

Windows 原生 Undo 八场景通过（55.86s），共享保护查询的两项 preparation 回归通过（12.22s），日志 artifacts/squash-undo。测试仅使用临时仓库，没有 GUI 变更或便携包更新。

### Continue 路由接入持久 reword 映射（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd，本轮读取 plugins/git4idea/backend/src/rebase/GitRebaser.java::continueRebase/handleRebaseContinueFailure，重读 GitInteractiveRebaseEditorHandler.java::editCommits：继续操作重新建立 editor handler，匹配持久消息时自动替换，否则进入普通编辑器；取消由 Git 失败结果保留继续/中止能力。

src/rebase_editor.rs::Router 新增操作级桥接：独立内部 Requests/Server，commit 请求先匹配 squash_message，未匹配转发现有 UI Requests；普通 sequence 请求也保留普通编辑器路径。停止时取消转发等待、关闭 Server 并等待 worker，不保留跨操作 helper。src/rebase_operation.rs::continue_with_recovery 使用该 Router，既有 UI 的 Continue 和 update_session 继续路径共享生效；requests=None 的外部 editor 路径保留。持久映射损坏等内部错误返回诊断，最终 repository state 仍由原执行器刷新。

回归发现并修正：最初普通编辑器拒绝被 forward 的 Interrupted 转成内部错误，破坏原 update_session 调用者对 Git 失败 Outcome 的处理。Router 现在把用户取消保留为 Git 的 exit 1，而不替换 command 结果；不会取消整体 token 或丢失已解决内容。外部保存仍由 Document 的文件原内容复验保护，转发者不写回旧文本。

Linux 验证：新鲜桥接恢复磁盘映射、原消息不匹配、已消费、用户取消和损坏记录五场景，匹配自动完成零 UI prompt；不匹配/已消费走普通消息，取消/损坏保持 rebase 并能显式 abort 恢复原历史。已有普通 continuation 拒绝后重试测试修复后通过，两项共 1.44s。另真实不连续 Squash 构造两次冲突，逐次外部解决并调用已有 Continue，已消费映射不覆盖后续消息，最终 base/squashed/middle 与原 tree 一致，0.92s。这里没有把解决冲突文件写入产品：测试模拟用户外部解决。

剩余：Squash 初始入口尚未接 UI；跨继续阶段的统一成功 Report/Undo/focus、local saver 的持久 ownership 与提示仍待。虽已有 Continue UI 会经过该后端，但本轮只做原生核心/桥接测试，没有新增截图或宣称完整 UI 验收。

Windows 原生验证全部通过：消息恢复/普通取消重试两项 14.04s，真实两轮 Squash 冲突 Continue 6.55s，原有独立 Continue/Abort 回归 8.77s；证据 artifacts/squash-continue。没有新增 GUI 截图或更新便携包。

### Squash 本地修改保存与首次恢复（2026-10-17）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd。本轮读取 plugins/git4idea/backend/src/rebase/GitRebaseProcess.java::saveDirtyRootsInitially/doRebase：首次重写前保存 dirty roots，继续已有 rebase 时不重复保存；SUCCESS 或 NOT_STARTED 时 load，未完成保留。读取 backend/src/stash/GitStashChangesSaver.java::save/load：记录保存的 stash hash，恢复用 pop，staging area 可用时加 --index；读取 backend/src/commands/GitImpl.java::stashSave 确认 push -m，无 --include-untracked。仍未证明该源码与已安装版本完全一致。

src/squash_operation.rs 复用 update_session::Session 的 common-dir 队列、save 与 restore_saved，去掉 tracked dirty 拒绝。保存后再次复验 HEAD/分支/选择/操作状态，成功或未启动且实际状态允许时恢复暂存划分。保存失败不开始 rebase；失败/取消报告同时返回保存结果、恢复结果、残留 stash OID 与恢复记录错误。保留原有 stash，未跟踪文件不纳入保存。

src/squash_saved.rs 用 worktree gitdir 中有界、create_new 的 ownership 记录保存 head/base/branch/stash；成功恢复后仅删除仍匹配的记录，冲突和取消保留。与上游内存 saver 不同，此记录为后续跨会话恢复准备；当前 Continue/Abort 尚未消费它，不能宣称自动恢复链路完成。恢复前要求 owned stash 仍位于栈顶，避免误 pop 其他应用新增的 stash；这是检查，不是跨进程原子保证。持久化失败后不重写历史，返回残留 OID；不承诺断电原子性。

Linux editor 七项 Squash 集成测试通过（9.00s）。新增四场景验证混合暂存/未暂存内容、已有 stash 保留、pre-rebase hook 失败后的恢复、保存遇 index.lock 不重写、冲突时 owned stash 内容与记录保留；原 executor dirty 场景改为成功且 index/status 不变。尚需实现继续/中止后的恢复、残留记录处理与 UI 提示，原生结果另记。

Windows 原生验证通过：本地保存/恢复四场景 25.84s，执行器十场景回归 57.30s；日志与运行脚本在 artifacts/squash-save。仅临时仓库测试，无可见 UI 改动或便携包更新。

### Squash Continue／Abort 后恢复保存内容（2026-10-17）

继续使用 4aec35256d765c217aa0d662b82ac4e860c396fd。本轮读取 plugins/git4idea/backend/src/rebase/GitAbortRebaseProcess.java::doAbort：rebaseAbort 成功及所需 rollback 完成后才调用 saver.load；失败保留 stash 并提示。Continue 依据前节已读 GitRebaseProcess::doRebase 的 SUCCESS/NOT_STARTED 恢复条件，未完成不加载。来源版本与安装版本差异仍未消除。

src/squash_saved.rs::for_rebase 在 Git 清理元数据前核对 orig-head、onto、head-name，绑定当前 worktree 的保存记录。src/rebase_operation.rs::execute 在成功 Continue/Abort、无进行中操作/冲突且未取消时调用 restore；restore 复验记录、分支、栈顶 OID，中止额外要求原 HEAD 已恢复，然后 pop --index。成功才删除记录；失败保留并重新读取实际状态。保存记录不匹配不会阻止用户 Abort，但不会恢复其他操作的内容。原 update_session 内的 Continue 仍由其自身 saver 管理，不重复恢复。

ResultState 增加 recovery_error，diagnostic 纳入现有有界诊断。src/ui.rs 不再把恢复失败称作 rebase 完成，而使用现有失败状态并保留详细诊断。没有新弹窗布局；此提示分支的原生可见验收尚待。保留差异：上游持有内存 saver，本地从磁盘绑定恢复；跨进程竞争不具备原子保护；外部 Git 已完成/中止后的显式恢复入口、pop 冲突处理和残留记录清理仍待，不自动重复 pop。

Linux 七项 Squash 回归通过（8.91s）；扩大保存测试至六场景后通过（5.64s），包含错配记录和外部栈顶变更时允许 Abort 但拒绝恢复。两轮实际 Squash 冲突继续后验证原 index 与未暂存内容恢复、记录清除；Abort 验证原 HEAD/index/文件和原有 stash 恢复。全部写操作限定临时仓库。

Windows release 构建通过（32.63s）。原生测试通过：编辑器继续/取消重试两项 20.34s，两轮 Squash 冲突继续与恢复 13.61s，保存/恢复六场景 43.08s，原有独立 Continue/Abort 9.84s；证据 artifacts/squash-resume-save。Linux 诊断两项亦通过。未更新便携包；现有失败提示分支未做窗口截图验收。

### 手动恢复 stash 后清理 Squash ownership（2026-10-17）

本轮读取同一 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/stash/ui/GitStashActions.kt：Apply/Pop 均显式选择单个 stash，普通入口 reinstateIndex=false，Unstash As 才使用对话框选项。读取 backend/src/stash/GitStashUtils.kt::unstash/createUnstashHandler：按选择执行 apply/pop，可选 --index；index conflict、merge conflict、本地/未跟踪覆盖及其他失败各自处理，成功才报告已应用。

src/stash_operation.rs 在显式恢复前读取当前 worktree 的 Squash ownership。成功且选中 stash OID 与记录一致才清理原记录；Apply 保留 stash 但也结束该次自动恢复责任，避免以后重复 pop。其他 stash、失败、冲突、取消均不清除。清理前再次比较记录，防止覆盖外部变动。损坏记录不阻止用户恢复 Git 内容，单独返回 recovery_error。src/ui.rs/src/i18n.rs 区分“已恢复保存的更改；恢复记录仍需处理”和真正的恢复失败，保留诊断。

这项清理是本地持久 ownership 的生命周期适配，上游没有相同磁盘文件；不是从源码声称上游存在同名清理。普通 Apply 保持不恢复 index 的行为，自动恢复仍用 --index。终端已恢复/删除 stash 后的显式确认、冲突解决后清理及损坏记录修复入口仍待，不能仅凭 stash 消失推断内容已恢复。

Linux 保存/恢复十场景通过（9.74s），新增外部 Abort 后普通 Apply、带 --index 的 Pop、操作其他 stash 和本地修改阻止四场景；标准 Apply/Pop 九场景回归通过（1.78s）。均使用隔离仓库，保留原有 stash 与内容断言。Windows 与窗口提示验证另记。

Windows release 通过（32.00s）；原生保存/恢复十场景 85.08s、普通 stash 九场景 13.52s，日志 artifacts/squash-manual-restore。自有 PID18696 原生窗口点击 Apply 后，artifacts/squash-manual-restore-ui/restored-warning.png 确认中文“已恢复保存的更改；恢复记录仍需处理”及损坏记录诊断可见；脚本断言文件内容恢复、HEAD/stash 不变、index 保持未暂存和异常记录保留，测试窗口正常关闭。首次截图显示被缩放，修正原始像素点击坐标后验收通过；无滚动自动化，未操作用户窗口。没有更新便携包或宣称整体 Squash UI 已完成。

### Squash Log 入口与消息弹窗接线（2026-10-17）

本轮读取 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/rebase/log/squash/GitSquashLogAction.kt、GitSquashOperation.kt、同级上层 GitNewCommitMessageActionDialog.kt：至少两项选择，action checks 后读取所选完整消息并 prettySquash；消息编辑器优先聚焦，空白消息禁用 OK；确认前复验 HEAD/保护分支，确认后后台执行，完成后 focusCommitWhenReady/通知 Undo。上游对话框 isModal=false，并保存修改过的提交消息历史。

本地 src/ui.rs::prepare_squash/start_squash 接入 Log 右键选择、后台 prepare/message、确认后已有执行器及普通 editor 请求桥接；src/squash_check.rs::message 按 Log 选择顺序载入完整消息，总量有界，匹配执行器的 1MiB 编辑限制。src/squash_dialog.rs 提供标签、多行消息、初始聚焦、OK/Cancel 与空白校验，中英文在 i18n。执行器在队列内重新检查快照，输出通过已有 Rebase details 诊断显示。

保留差异/待完成：当前 GPUI 使用应用已有居中弹窗，不是上游非模态独立窗口；右键一项时保留禁用 Squash，上游隐藏；没有提交消息历史/工具栏；动作准备失败仍通过状态和详细诊断，弹窗内动态 repository validation 尚未接；成功 Undo/精确提交定位与跨继续结果汇总未接。本地已有 bounded Git fallback，未实现上游 in-memory 优先。不能把入口接通视为完整 Squash 或像素验收完成。

Windows release 通过（23.63s）。原生 artifacts/squash-ui 验证：多选右键入口、预填完整消息、输入聚焦、空消息禁用/不改历史、确认后两条提交压成一条、tree 不变及工作区干净。首轮 SendKeys 被当前中文输入法消费空格，屏幕与实际提交一致；改用粘贴后消息精确断言通过。第二次自有 PID34956 等待刷新完成，completed.png 显示 Reviewed squash/base 两行，测试窗口正常关闭，未操作用户仓库/窗口。

原生发现的待办：初次 4 秒截图尚处异步刷新，15 秒等待后列表更新；没有将延迟视为性能验收。重写后原 OID 选择清空，详情区为空，必须继续接 focusCommitWhenReady 等效行为及 Undo，尚未满足完整功能交互验收。取消/保护分支变化/冲突继续等 UI 路径仍待，现有后端用例不代替窗口验收。便携包未更新。

### Squash 完成定位与 Undo UI 接线（2026-10-17）

本轮读取 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/rebase/log/GitCommitEditingUiUtils.kt::focusCommitWhenReady（等待新 hash 进入 Log 后 showCommit），GitCommitEditingNotifications.kt::notifySuccess/undoInBackground（通知 Undo，后台复验；不可撤销或成功后过期；成功可定位原提交），重读 GitCommitEditingOperationResult.kt 的范围/保护分支/HEAD 检查与 reset --keep。读取 inMemory/rebase/log/InMemoryRebaseOperations.kt 与 GitInMemoryCommitEditingOperation.kt，确认内存操作携带 focus 信息；Git fallback 的 GitCommitEditingOperation.kt 创建 Complete 时没有指定 focus hash。

本地 squash_undo::Undo 增加新旧范围首提交定位数据，按固定 base..HEAD 有界读取；这是 Git fallback 补齐用户期待的定位，不宣称上游 fallback 已传入相同 hash。src/ui.rs 成功时后台捕获 Undo，导航复用 locate/locate_filtered，但重新读取当前 refs，避免使用旧 tips 查不到新提交；定位后复用单选文件/默认预览。保留过滤器，目标隐藏时沿用已有清除过滤提示。状态栏提供 Undo Squash，后台执行既有复验及 reset --keep，成功后定位原范围首提交并消耗入口；检查拒绝移除入口，命令失败可保留重试。

保留差异：Undo 在本地状态栏而非 IDE 通知气泡；HEAD 变化后刷新可移除入口，未接上游仓库事件的即时远端保护变更过期，但点击始终重新查询；此轮只覆盖直接成功，冲突继续后统一结果/Undo 仍待。导航对隐藏目标与跨页已有能力，但本次专项尚未全量原生验收。reset --keep 的暂存划分语义沿用前节，不保证保留 staging。便携包未更新。

Windows release 通过（21.60s）；Undo 八场景 Linux 回归 9.36s、Windows 原生回归 75.21s，增加新/旧定位目标断言。自有 PID6668 在 artifacts/squash-ui-undo/completed.png 中自动选中 Reviewed squash，右侧消息与 a.txt 默认预览正确；undone.png 显示原历史三行、选中 First edit both files、默认 Diff 恢复且 Undo 入口消失。脚本通过复制 OID 验证新旧定位，断言原 HEAD/完整 rev-list 恢复、工作区干净，并正常关闭测试窗口。未操作用户 Rebased，未做滚动自动化；15 秒等待仅为测试稳定等待，不代表性能结论。

### Squash 冲突继续后的结果与 Undo（2026-10-17）

本轮读取 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/rebase/GitRebaseSpec.java：新 rebase 保存 initial HEAD/branch、params/status/saver；独立恢复路径 forResumeInSingleRepository 使用空 initial maps、null params。结合前节 GitCommitEditingOperationResult/Notifications 的完成与 Undo 规则。本地不能用任意原生 rebase 的 orig-head 猜测它是本产品发起的 Squash。

src/squash_session.rs 新增 rebase-merge/gitui-squash-origin，初次 sequence editor 放行前记录初始 HEAD/base/branch。create_new、有界读取、格式校验、与 orig-head/onto/head-name 匹配；Git 完成或 Abort 清理所属目录。src/rebase_operation.rs 在 Continue 前读取身份，成功且无冲突/进行中操作、未取消、保存内容恢复无错时调用 Undo::after_continue；没有有效来源的普通 rebase 不提供 Squash Undo。坏记录不阻止 Continue/Abort，但完成后无法汇总 Undo 时返回诊断；Abort 永不生成 Undo。

src/squash_undo.rs 复用直接成功路径的有界新旧范围与 worktree/HEAD/分支检查，不伪造初次执行结果。src/ui.rs 消费 Continue 的可选 Undo，复用新 refs 导航、默认文件预览和状态栏 Undo。保存来源跨桥接/应用会话可读取；与上游独立恢复时空 initial maps 的差异是本地只为有自身来源文件的操作保留此能力。外部 Git 已完成且删除 rebase 目录后不能重建该会话；不扩展为任意历史重写 Undo。

Linux 实际两轮冲突的四变体通过（6.00s）：合法来源最终提供可执行 Undo 且原历史/未暂存内容恢复，中间冲突无 Undo；来源缺失、损坏、身份不匹配都不产生 Undo，损坏/错配有诊断。底层已有保存/恢复与普通 Continue 回归继续保留。窗口验证另记。

Windows release 通过（31.01s）；原生编辑器继续/取消重试两项 16.22s、两轮 Squash 冲突及来源四变体 41.74s、独立 Continue/Abort 回归 11.09s，日志 artifacts/squash-continue-undo。上述为后端真实 Git/桥接验证，窗口交互结果另记。

原生 UI 验证 artifacts/squash-continue-undo-ui：自有 PID14896 从 Log 选择 first/last 发起 Squash，发生首轮冲突；正常关闭后以 PID21088 重新启动。测试脚本在隔离 fixture 外部解决/暂存 shared，应用两次 Continue 与普通消息编辑确认后完成，completed-after-restart.png 自动选中非 HEAD 的 squashed、显示详情/默认 Diff/Undo。点击 Undo 后 undone-after-restart.png 选中原 first，脚本断言原 HEAD/完整 rev-list 恢复、工作区干净且 rebase 目录清理，测试窗口正常关闭。没有内置冲突编辑器的完成声明。

自动化两次因窗口遮挡/前台保护拒绝操作，未向其他窗口发送点击或按键；最终定位用截图核对，Undo 历史用 Git 断言，不把被拦截的剪贴板检查计为通过。用户 Rebased/IDEA 未关闭，未进行滚动自动化。便携包未更新。

共享 Undo capture 重构后的直接成功八场景 Linux 回归也通过（9.24s）。

### Squash 确认校验与错误提示（2026-10-17）

读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/rebase/log/GitNewCommitMessageActionDialog.kt::doValidate/validateCommitsEditable/updateOkButtonState、plugins/git4idea/shared/resources/messages/GitBundle.properties 中 rebase.log.reword.dialog.failed 两条文案，以及 platform/platform-api/src/com/intellij/openapi/ui/DialogWrapper.java::updateErrorText/doUpdateErrorText/ErrorText 与验证后的 OK 状态更新。上游校验 HEAD 和受保护分支；无 component 的 ValidationInfo 在按钮上方用单条错误色文本显示，阻止确认；不标红提交消息输入框，也不同时展示通用错误标题和底层技术诊断。

本地 src/squash_check.rs::revalidate 提供只读复验，src/squash_operation.rs 在 common-dir 队列内复用，保存本地修改之后原有复验保持不变。src/squash_dialog.rs 异步确认，等待期间禁用编辑/确认，取消时取消查询，失败保持草稿并禁用确认；src/i18n.rs 提供中文文案。按用户反馈移除“已保留消息”标题及直接拼接英文诊断，改为上游式单条校验消息。保留差异：仍是应用内模态窗口；校验按确认触发，不采用 DialogWrapper 的周期校验，所以错误后需关闭重开；本地额外检查进行中的操作和待恢复状态，未知查询失败用可操作的通用提示。安装版 1.1.20 / 262.10968.SNAPSHOT 与参考源码快照不同，尚未做该错误态的安装版同条件操作对照。

### Log 父／子提交导航查询（2026-10-17）

读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/actions/GoToParentOrChildAction.kt 全文，以及 platform/vcs-log/graph/src/com/intellij/vcs/log/graph/impl/facade/VisibleGraphImpl.kt::getAdjacentRows：仅单选提供邻接目标；目标来自 compiled visible graph 的上下节点，按可见行排序，单目标直接 jumpToGraphRow，多目标使用带标题的动作选择 popup，缓存元数据可用于 hash/subject/author/date 标签。键盘入口只在表格焦点下启用，无目标不动作。

本地先完成查询：src/filtered_graph.rs::adjacent 与 window 共用 snapshot_edges，确保跨隐藏节点的虚线邻接与图形投影一致，不篡改真实 Commit.parents。src/history.rs::adjacent 固定 roots、沿用 hash 优先／普通过滤回退和路径规则，返回排序后的完整 OID；仅读取有界拓扑，不读取 Diff 或提交正文。保留现有 100,000 节点／500,000 边预算，拒绝不完整图；在读取和投影前后检查取消。实际 UI 的单选门控、禁用状态、键盘入口、多候选 popup 和原生窗口验收尚未接通，不能宣称菜单功能已可用。源码快照仍与已安装 1.1.20 / 262.10968.SNAPSHOT 区分。

验证：filtered_graph 五项测试全部通过，含 merge/fork 选择顺序、根和 tip、边预算及部分图拒绝；真实临时仓库 graph_navigation_uses_filtered_adjacency_and_fixed_roots 通过（0.22s），覆盖隐藏中间提交、双向虚线邻接、隐藏选择无候选、固定 roots 排除后续提交、hash 查询覆盖路径过滤、取消与只读断言。本轮无 UI 变化，未新增 Windows 截图。

### Log 父／子导航 UI 接线（2026-10-17）

继续使用上节已读 GoToParentOrChildAction.kt；本轮重读 getRowsToJump，并核对 platform/vcs-log/impl/resources/intellij.platform.vcs.log.impl.xml 的 Vcs.Log.GoToParent/GoToChild：默认 RIGHT 到父、LEFT 到子。src/ui.rs 接单选菜单入口及 History 焦点下的左右键；后台查询固定 roots/过滤范围，回写同时检查取消、generation 和选择 OID，模型重置／窗口销毁取消查询。单候选复用现有 navigate_commit，多个候选使用 PopupMenu，缓存元数据生成 hash/subject/author/date，未缓存只显示短 hash；选择时再次核对原选择/代次。弹窗默认最高 popup 层，不另建对话框或文件正文缓存。

保留差异：候选按需查询而非上游常驻 compiled graph，因此当前单选菜单在候选未知时启用，查询为空才不动作，尚未实现无邻接行时菜单预先禁用；多候选暂未加数字助记键，键盘触发位置目前沿用鼠标位置；标题/标签排版与上游简化 subject 格式还需继续对齐。查询沿用完整有界拓扑，尚未测大仓库调用延迟与峰值内存。当前仅基础路径，未将完整菜单/键盘验收标完成。

### 导航数字选择与键盘锚点（2026-10-17）

补读参考 4aec352 的 platform/platform-impl/src/com/intellij/ui/popup/ActionStepBuilder.java::appendAction：NUMBERING 前九项为1–9，第十项为0，后续仅 ALPHA_NUMBERING 才使用字母。vendor/gpui-component/src/menu/popup_menu.rs 新增显式 numbered_navigation（其他菜单默认关闭），启用首个可操作项选择和无修饰数字激活；src/ui.rs 为候选提供对应编号，键盘锚点依据当前 Log 行与 viewport 计算，鼠标仍使用操作位置。Windows release 通过（1m04s），原生数字2与Enter选择分别精确复制对应OID，截图 artifacts/log-adjacent-ui/numbered-popup.png、断言 numbered-assertions.json；context-child-hash.png 验证右键入口，context-two.png 确认多选禁用导航。初次前台保护拒绝键盘输入，随后使用目标窗口命中检查激活重跑，不将被拒绝动作计为成功。无候选菜单预先禁用、超过十候选和跨页窗口验收仍待。

### Compare Versions 两版本语义与读取（2026-10-17）

读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/actions/CompareRevisionsFromLogAction.kt 全文：仅恰好两个同仓库提交显示并启用，传入 commits[1] 为左、commits[0] 为右。GitLogDiffHandler.java::showDiffForPaths 调用 GitChangeUtils.getDiff；读取 plugins/git4idea/backend/src/changes/GitChangeUtils.java 的 getDiff 重载及内部实现：detectRenames=true，threeDots=false，解析双方固定引用后构造两个版本的变化，不做 merge-base 或逐提交 zip。

继续读取 VcsLogUtil.java::getAffectedPaths、VcsLogUiPropertiesImpl.kt：SHOW_ONLY_AFFECTED_CHANGES 默认 false，仅显式启用时使用结构过滤路径。读取 platform/vcs-impl/src/com/intellij/vcs/CompareWithLocalDialog.java 的 showChanges/showAsTab/showDialog/createPanel/MyChangesBrowser：LocalContent.NONE，不保存工作区文档；默认经 Changes 工具窗口显示文件浏览器与 DiffPreview，非模态弹窗是 registry/模态场景回退。AbstractVcsHelperImpl.showCommittedChangesAsTab 与 registry.properties 确认 vcs.show.affected.files.as.tab 默认 true。文件树使用 KEEP_NON_EMPTY，选择逻辑不能复用多提交累计变化的范围。

本地 src/git.rs::revision_changes 固定两个完整OID，用有界name-status读取及rename检测，literal路径参数避免特殊字符扩展，复用 combined_changes::from_file 创建前后正文端点；只读、不预加载正文。临时分叉仓库测试通过（0.22s），覆盖非祖先直接比较、反向比较、同版本空变化、rename、增删、中文方括号路径、工作区修改不影响固定版本、非法输入和取消。尚未接菜单/Changes浏览器，不声称 Compare Versions 已可见可用；默认UI应比较全范围，受影响路径显示另行决定。已安装Rebased与参考源码版本差异照旧保留。

### Compare Versions Changes 工具窗口接线（2026-10-17）

补读同一参考 commit 的 platform/vcs-impl/src/com/intellij/openapi/vcs/impl/ChangesBrowserToolWindow.kt 全文：注册 anchor=LEFT，一次 showTab 先 removeAllContents(true) 再添加新内容，使用 TreeHandlerEditorDiffPreview。读取 changes/ui/TreeHandlerDiffRequestProcessor.kt::TreeHandlerEditorDiffPreview：双击/Enter 执行 Diff 动作，Escape 关闭预览并返回树。因此本地使用独占左侧 Changes 窗口，不增加底部面板；不会把默认首文件选择等同自动创建 Diff Tab。

src/compare_view.rs 新增固定两版本、异步变化查询、目录树与首文件选择；刷新保持前后路径相同的选择，无匹配回退首项。文件正文仅显式查看时由共享 Chain/Target 按需读取。src/ui.rs 在恰好双选时显示 Compare Versions，按 Log 下→上固定 before/after，替换旧比较窗口，保留其他工具窗口状态；关闭释放查询/列表，切仓清理，独立 Diff Tab 按其固定目标继续持有轻量链。比较窗口暂时收起可从左侧图标恢复。沿用中英文资源。

保留差异：文件状态颜色、目录/文件类型图标、重命名信息与总数等尚未完整对齐；目前显式打开的是现有固定 Diff Tab，未接上游可复用预览Tab及树与已打开预览的自动联动/Escape回树；尚未提供 Show Only Affected Changes 设置，默认全范围与上游默认值一致。仅一组比较窗口符合 removeAllContents 行为；横向宽度复用现有侧栏，但详细快捷键/焦点/空态/错误和快速关闭竞态、大文件/多目录原生验收仍待。没有声称像素级还原或完整比较交互完成。

## Compare Versions 预览身份与 Escape（2026-10-17）

读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/openapi/vcs/changes/EditorTabDiffPreview.kt`：previewFile 是每个 preview 固定的虚拟文件，openPreview 重复打开同一身份；dispose 令文件无效并关联 viewer 生命周期。读取 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/TreeHandlerDiffRequestProcessor.kt::TreeHandlerEditorDiffPreview`：Enter/双击触发 performDiffAction，Escape 关闭预览后 returnFocusToTree。

本地 `ui.rs::compare_versions` 以 WeakEntity 记录比较所属 DiffTab，显式打开另一文件复用该 Tab，不按文件身份制造多个 Tab，也不接管其他来源的固定 Tab。`DiffTab::replace_target` 先取消并释放旧 viewer；比较窗口替换/关闭清理所属预览。Escape 关闭该预览后展开比较侧栏并聚焦文件树。保留差异：当前只接显式打开复用，树单击自动更新与 Diff 导航反向同步、固定预览及关闭按钮焦点策略尚待核对接线。源码快照不等同已安装 Rebased 1.1.20 / 262.10968.SNAPSHOT；原生本地验证另记 PLAN。

## Compare Versions 树与 Diff 双向选择（2026-10-17）

继续读取同一 `4aec352` 的 `TreeHandlerDiffRequestProcessor.kt`：`TreeHandlerChangesTreeTracker::track` 监听树选择及模型更新，`refreshPreview` 调用 processor.refresh；队列合并更新并以 100ms 防抖。`TreeHandlerDiffRequestProcessor::selectChange` 委托 handler 选择树中对应 change。与前述固定 previewFile 身份结合，文件树变更更新已打开预览而非新增编辑器。

本地 CompareView 增加 SelectionChanged，只在明确文件选择/上下键移动时发送，Review 仅更新该比较拥有的预览；未打开预览不新建，非活动 Tab 仅更换轻量目标。DiffTab 的文件选择器/前后文件导航发 FileChanged，CompareView 通过完整 Target/Chain 身份匹配同步选中并展开祖先；回写不重复发送选择事件，防止循环。activate_with_focus(false) 允许树选择加载预览但不抢树焦点。当前为立即取消/替换旧查询，尚未采用上游 100ms 合并队列；刷新失败/空列表清理预览、键盘自动定位可见行与更大规模性能仍待处理。

## 比较刷新失败与恢复（2026-10-17）

实际读取 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `CompareWithLocalDialog.java::MyLoadingChangesPanel.applyResult`（null 转空变更集合）和 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/browser/LoadingChangesPanel.java`：替换请求取消前一请求；失败停止进度、在北部有界面板显示可复制错误并 applyResult(null)；取消不应用失败结果。结合前述 tracker，模型变化会刷新既有预览。

本地 CompareView 在后台查询完成时发送 Reloaded，成功重新绑定选中文件；失败或空模型使所属 DiffTab 失效，取消读取、释放 viewer、禁止文件导航并隐藏旧版本标题，保留 Tab 身份以便刷新后恢复。非活动失效预览再次激活也不重新读取过期目标。界面用本地化摘要与“复制错误详情”取代整段 Git 输出，刷新按钮作为重试；这是按用户提示风格要求作出的呈现差异，上游直接显示可复制原始消息。源码快照与已安装版本差异仍沿用文首记录。

## 外部恢复后的明确确认：记录清理基础（2026-10-17）

重新读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/stash/GitStashChangesSaver.java::load/showSavedChanges` 和 `notifyUnresolvedRemain`：load 通过 unstash/pop 恢复，Staging Area 可用时带 --index；未解决冲突提示提供查看 stash 和解决冲突动作。上游 saver 持有内存身份，没有本地 gitui-squash-saved 文件或对应“已在外部处理”记录清理 API；不声称该本地动作直接照搬上游。

本地 src/squash_saved.rs 新增 Pending 有界字节快照和 acknowledge_external_recovery：显式确认后，在公共操作队列内复验 worktree gitdir、无进行中操作/冲突及记录字节未变，只删除应用记录；不执行 Git 写命令、不推断 stash 消失表示已恢复，也不应用/删除 stash。允许确认已人工处理的损坏记录，但拒绝非普通文件、Windows reparse point 和超过8KiB文件。私有快照字段防止调用方伪造另一 worktree 的确认。外部进程仍不受应用队列约束，最后检查与删除不是跨进程原子事务；不宣称抗任意并发替换。

真实 Git 测试覆盖合法/损坏记录明确确认、确认后记录变更、进行中 merge、取消及另一仓库误用；均核对 HEAD、stash、index、工作区不变。本轮为后端基础，尚未添加可见确认入口或自动清理行为；需继续接已保存更改面板、确认说明/默认取消、执行后刷新和原生交互验收。

## 外部恢复确认入口（2026-10-17）

继续核对 `4aec352` 的 GitStashChangesSaver::notifyUnresolvedRemain/showSavedChanges（查看stash/解决冲突入口），以及 DialogWrapper.java::createActions（确认、取消的标准顺序）。上游没有本项目磁盘ownership清理动作，因此新增“已在外部处理”明确作为本地持久化适配：StashView 刷新时后台读取Pending，显示待处理提示及真实入口；无法读取非普通/超限记录时显示摘要和可复制详情。

Review::confirm_external_recovery 展示仓库及“只清记录，不恢复/删除stash、不改文件”说明，默认取消；对话框Enter只关闭，显式按钮才发起清理。后台执行复用fetch_token/fetch_task及公共队列，绑定仓库与记录快照；完成后刷新stash面板，失败保留记录并提供本地化下一步。该确认不是上游原生同名窗口的像素复刻；采用现有标准居中弹窗与内容宽度按钮。原生发现提示占高使底部动作被挤出，文件分区改为填充剩余空间；具体验证见PLAN。

外部恢复确认补充验证：重新读取同一 4aec352 的 GitStashChangesSaver::notifyUnresolvedRemain，未解决 stash 冲突继续保留查看/解决入口。tests/repository.rs 的 squash_external_recovery_* 新增 linked worktree 同字节隔离、真实 stash apply 未合并 index（无 MERGE_HEAD）和异常记录替换测试；这是本地持久化适配的约束验证，未新增上游不存在的自动恢复推断。非普通/超限记录仍要求外部人工检查，本轮无产品界面变更。

## 普通 Diff 失败与重新加载（2026-10-17）

实际读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/diff-impl/src/com/intellij/diff/tools/ErrorDiffTool.java::MyViewer/createReloadMessagePanel、platform/diff-api/src/com/intellij/diff/requests/ErrorDiffRequest.java、platform/diff-impl/src/com/intellij/diff/impl/CacheDiffRequestProcessor.java::reloadRequest/doLoadRequest：错误 request 独立显示，内容区居中放置消息及 reload 链接；reload 强制重新读取；异常保留在 request，取消单独处理。

本地 src/diff_tab.rs 在失败后释放 viewer，居中显示本地化摘要、重新加载和复制错误详情；重载复用固定 Target/现有 generation 与取消，不能改为读取当前 Log 选择。文件导航继续可用，差异块导航禁用，失败页隐藏版本标题；同一组件覆盖独立 Tab 和使用 DiffTab 的嵌入预览（例如 Stash）；旧 Log 底部预览由 Review 另行绘制，仍待统一。偏好保存失败改为本地化摘要及复制详情。按用户错误提示要求，将上游直接显示异常 message 改为短摘要+主动复制原始详情；未冒充逐像素一致。普通取消仍通过代次丢弃，不新增错误弹窗。具体原生验证见 PLAN。

## Log 底部预览与全局错误归属（2026-10-17）

继续读取同一 4aec352 的 ErrorDiffTool::createReloadMessagePanel，复用居中消息/重载规则；另读 platform/vcs-impl/src/com/intellij/openapi/vcs/VcsNotifier.java::importantNotification/notifyError：操作错误用独立通知，可附详情动作，不作为主内容顶部行。上游 important 为 sticky balloon，本地按用户此前指定的右下信息位置，暂用既有状态栏摘要、复制详情和关闭；尚未实现通知历史/浮层，明确保留结构差异。

src/ui.rs 将文件读取失败拆为 preview_error，reset_job 清理，代次校验阻止旧结果；底部预览显示本地化摘要+重载当前文件+复制详情，隐藏失败版本标题、禁用差异导航。非文件错误保留在全局 error，去掉顶部原文行，在右下摘要呈现；底部数据失败提供只读刷新，不自动重试 Git 写操作。偏好失败同样不直接展示长诊断。原生验收与剩余范围见 PLAN。

## Project 大目录分页读取基础（2026-10-17）

重新读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/lang-impl/src/com/intellij/ide/projectView/impl/nodes/ProjectViewDirectoryHelper.java::getDirectoryChildren/processPsiDirectoryChildren：普通目录枚举当前层并构造文件/目录节点，flatten/hide-empty 另有递归路径；上游使用 PSI/VFS 索引，不提供与本地对应的有界分页游标。因此分页是低内存适配，不宣称上游也有“加载更多”同名动作。

src/project_files.rs 新增 children_page/Page/Cursor，按既有目录优先/自然排序选取下一页，最多256项及512KiB路径内容。扫描期间有界最大堆仅保留排序前缀（入堆瞬间额外一项），无需持有整个目录或跨请求 OS 枚举句柄；每页重新扫描当前层，不递归。游标私有字段绑定canonical根/相对目录/目录修改时间/末项，拒绝跨根、跨目录和检测到目录变化的继续请求；分页期间变化同样报错，不伪造完整列表。修改时间不是原子快照或敌对文件系统保证，时间粒度隐藏的修改仍是限制。

现有 children 完整有界API保留且共用排序规则。UI尚未切换分页，下一步接目录内继续加载和刷新重置，继续保留整个树16384节点预算与折叠释放；不将后端通过记成大目录可见功能交付。

## Project 分页接入文件树（2026-10-17）

复核同一 4aec352 的 ProjectViewImpl.java 树安装/toggleClickCount/restoreExpandedPaths 段，沿用单击选择、双击或Enter打开文件及目录展开关系。上游当前源码未提供本地有界分页协议，本地目录末尾“加载更多文件”是明确的低内存适配，保留原树层级/文件身份而不引入额外主面板。

src/project_view.rs 每目录保存轻量Cursor；读取第一页/追加页调用children_page，只有代次匹配才应用。目录末尾真实动作行可单击/Enter/Right，加载后选中新页首项，已有文件选择按路径及行类型恢复；Home/End定位当前已加载树的首末项。折叠删除该子树数据/游标并取消未完成任务；刷新根重置；目录变化清理该子树旧分页，提示刷新。不沿用旧4096项整目录失败；整树16384项预算仍保留，超过预算不追加半页，原游标保留供折叠其他目录后重试。未使用PSI/VFS常驻索引，精确视觉/大目录时延和内存验收另记。

## Project 文件 Tab 的默认排列与关闭（2026-10-17）

读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/editor-ui-api/src/com/intellij/ide/ui/UISettingsState.kt`：openInPreviewTabIfPossible、openTabsAtTheEnd、activeMruEditorOnClose 默认false；activeRightEditorOnClose仅AppCode默认true。读取 `platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/EditorWindow.kt::addComposite/shouldReservePreview/closeFile/getTabToSelect/computeIndexToSelect`：新Tab默认插在选中Tab之后；预览须设置启用且满足打开选项/焦点条件；关闭优先仍存在的previousSelection，其次按配置选择，默认左邻；关闭非活动Tab保留当前选择。参考源码不等同已安装Rebased 1.1.20 / 262.10968.SNAPSHOT的用户设置。

本地 `src/ui.rs::toggle_project/close_file_tab` 先落实文件Tab组内部的插入和默认左邻关闭规则。关闭非活动文件只修正索引，不重新加载活动正文；关闭活动文件释放正文/任务，再按需读取左邻。Log在文件组左侧，关闭首个文件回Log。保留单击选择、双击/Enter打开普通文件Tab，不能把预览默认关闭误译成无条件单击预览。差异：尚未统一Log/Diff/File的Tab身份与previousSelection历史，文件仍是独立排列组；可选预览模式、Esc规则和光标/视口恢复继续待办，不宣称完整EditorWindow行为已还原。

## 普通文件 Esc 与键盘关闭（2026-10-17）

同一参考commit，已读 `platform/platform-impl/src/com/intellij/openapi/editor/actions/EscapeAction.java`：清除选择、退出sticky selection并收敛多光标，不关闭普通编辑器；`platform/diff-impl/src/com/intellij/diff/editor/DefaultDiffFileEditorCustomizer.kt` 为Diff专门注册关闭/自定义Escape动作，不能应用到Project文件。另读 `platform/platform-resources/src/keymaps/$default.xml` 的CloseContent=Ctrl+F4、`platform/platform-impl/src/com/intellij/ide/actions/CloseAction.java` 和 `platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/EditorTabbedContainer.kt::uiDataSnapshot/close`：通过当前上下文CloseTarget关闭选中编辑器。

本地 `src/ui.rs` 的Back不再移除普通文件Tab；委托 `src/project_view.rs::FileTab::escape`，仅当正文聚焦时清除文本选择，保留Tab及焦点。新增ProjectFileTab键盘上下文内Ctrl+F4，调用已有close_file_tab，避免在Project树或其他面板焦点下误关文件。Diff原Esc逻辑保留。GPUI输入组件先处理输入法/菜单/多光标，传播后执行本地清选；上游保留最老光标与GPUI保留活动光标的细节仍有差异，当前不宣称多光标完整一致。普通文件仍只读，无编辑保存行为。

## 文件与 Diff 统一 Tab 顺序（2026-10-17）

继续读取同一 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/platform-api/src/com/intellij/ui/tabs/impl/TabLabel.kt::mousePressed/mouseReleased`：previousSelection在按下时临时赋值，释放时清空；不是永久“打开来源”或MRU历史。结合已读EditorWindow::addComposite/getTabToSelect/computeIndexToSelect，普通关闭默认依据统一Tab排列的左邻。此前文档将previousSelection列为未实现时未核对赋值生命周期，此处补充限定，不能据此实现永久opener返回。

本地 `src/ui.rs::editor_tab_order/insert_editor_tab` 用轻量EntityId排列普通文件和Diff，不再将两种Tab强制分组；Log保持首位。Log、Commit、Stash、Compare Versions新建Diff及Project新建文件统一插在活动Tab之后，重复打开原Tab不改变顺序。关闭文件及鼠标关闭Diff时选择实际左邻（可以跨类型），无左邻回Log；关闭非活动Tab不转移当前焦点。程序关闭Diff/Compare及切仓同步清理顺序身份，身份列表不持有正文或Entity引用。保留Diff Esc的来源面板返回规则，与标题关闭不同；任务取消/正文按需加载沿用现有路径。尚未实现鼠标按下释放期间的临时previousSelection细节、拖拽排列、MRU配置与可选文件预览，不宣称整套编辑器Tab完全一致。已安装版本仍为1.1.20 / 262.10968.SNAPSHOT，源码快照不等同该版本用户设置。

## 普通文件阅读状态与正文释放（2026-10-17）

已读同一 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/text/TextEditorProvider.kt::getStateImpl/setStateImpl/scrollToCaret` 与 `TextEditorState.java`：保存光标、选择起止逻辑行列及光标相对视口距离；异步编辑器显示后恢复位置，不把状态等同正文副本。

本地 `src/project_view.rs::ReadingPosition/FileTab::deactivate/render` 在释放EditorState前保存当前活动选择的行列、方向及视口偏移，激活仍重新读取文件；新EditorState先恢复选择，布局时使用已有set_scroll_offset延后应用/限制有效范围。读取失败保留位置供重试，不保留旧正文。`vendor/gpui-base/src/input/base/state.rs::set_selected_range_with_direction`复用现有UTF-8边界限制并恢复活动端，避免反向选择恢复后Shift方向颠倒。该适配没有复制上游代码。

明确差异：目前保存单个活动选择，不保留全部多光标/折叠；视口使用GPUI逻辑像素偏移，而非上游光标相对视口距离，字体/DPI变化的相对位置仍待进一步对齐。当前会话保留，关闭文件或切仓释放，不新增磁盘会话恢复。原生验证不包含滚动自动化，手动视口体验与长文件/缩放另待验收。源码快照与已安装Rebased版本区别沿用前述记录。

## 可选 Project 文件预览 Tab（2026-10-17）

参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，已读 `platform/platform-api/src/com/intellij/ui/AutoScrollToSourceHandler.kt`：鼠标单击立即导航、选择变化默认100ms合并延迟且检查树焦点；`platform/platform-impl/src/com/intellij/ui/AutoScrollToSourceTaskManagerImpl.kt` 使用requestFocus(false)/preserveCaret(true)。`platform/lang-impl/src/com/intellij/ide/projectView/impl/ProjectViewImpl.java` 的auto-scroll模式包含openInPreviewTab。结合已读EditorWindow::addComposite/shouldReservePreview/trimToSize：已有普通Tab不降级，预览复用/替换，显式requestFocus打开转普通；`EditorTabbedContainer.kt::doProcessDoubleClick` 双击预览Tab转普通；`EditorsSplitters.kt` 预览文字使用italic。默认false见此前UISettingsState记录。

本地 src/layout_preferences.rs 保存有界读取的file-preview偏好，src/ui.rs设置“编辑器标签页”提供中英文开关，默认关闭。ProjectView单击发预览，Up/Down/End延迟100ms且保留树焦点，离焦/隐藏不应用旧导航；Home/折叠/明确打开/关闭开关取消待处理任务。FileTab::activate_with_focus区分预览与明确打开，正文完成不抢预览焦点；已有普通文件保持普通。新文件替换唯一旧预览并释放旧任务/正文，普通Tab不受影响；Enter/文件双击或Tab双击转普通，斜体标识预览。关闭开关将当前预览转普通以保留正在看的文件，这是本地显式选定的适配，未声称上游关闭开关必然同样处理。

差异：无PSI文件类型注册机制，二进制/未知编码/超大文件沿用有界只读说明，不调用外部应用；一个旧预览存在时新文件在其槽位替换，即使明确打开时旧预览并非活动Tab也保留该位置，上游普通新Tab插入后再trim的排列还需补齐。尚未提供Project工具菜单的重复开关、任意tab pin保护或跨会话Tab恢复；“固定打开”指转普通Tab，不等同禁止自动关闭的pinned状态。源版本与已安装Rebased版本不一致的限制继续保留。

### 明确打开时的预览清理顺序补齐（2026-10-17）

再次核对同commit EditorWindow::addComposite先根据isPreviewMode决定插入点，再由trimToSize关闭其他预览。src/ui.rs::open_project_file现区分：新预览沿用旧预览槽位；明确打开普通文件先在当前活动Tab后插入身份，再移除旧预览身份。替换存储槽不决定视觉排列，避免非活动预览在远处时新普通文件跑到错误位置。此修正消除上一节明确记录的该项排列差异，其余类型/设置适配仍保留。

### Project 文件读取错误的短摘要与动作（2026-10-17）

参考commit仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，本轮读取 `platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/FailedEditorPane.kt`：failedEditorPane要求消息简短，不直接呈现多页诊断；FailedEditorBuilder.link将动作放在消息下方，draw以水平50%/垂直47%布局，长消息可以换行。该源码快照与已安装Rebased 1.1.20 / 262.10968.SNAPSHOT不同，未将源码构造器视为安装版每种文件失败都使用同一页面的证明。

本地 `src/project_view.rs` 的Notice将I/O错误摘要与原始details分开，FileTab失败页居中呈现摘要、重新加载和复制错误详情链接。重新加载复用固定root/path及generation取消流程，先释放旧正文；二进制、不支持编码和超大文件保留各自原因，不显示无意义的读取重试。Project树底部错误也增加刷新/诊断链接，重试重新读取根目录。原始系统诊断不翻译，摘要沿用集中中英文资源。

保留差异：本地垂直居中而非上游47%，未添加错误图标；复制诊断是本地沿用Diff错误页的排查动作，不声称来自FailedEditorPane默认动作。目录错误仍显示在树底部而非独立通知；其原生失败/刷新组合待验收。按需创建链接和读取，未增加常驻文件正文或全局通知历史。

### Project 目录错误文案与刷新验证（2026-10-17）

继续沿用上一节FailedEditorPane的摘要/动作分离规则，并读取同commit `platform/lang-impl/src/com/intellij/ide/projectView/impl/ProjectViewImpl.java::refresh(ProjectViewUpdateCause)`：只更新当前pane，清图标缓存并调用updateFromRoot(false,cause)。本轮未深入pane内部恢复算法，不将该调用等同已确认完整树状态恢复规则。

本地src/project_view.rs树错误对I/O使用独立“Could not read project folder”摘要，src/i18n.rs增加对应中文，原始诊断复制保留。错误区Refresh复用当前ProjectView根目录重读，不销毁中央文件Tab；英文Windows已核对错误、诊断复制及Tab到达刷新链接后Enter恢复。保留差异：本地当前刷新清空展开/选择并回根节点，后续需按pane实现对齐；不是上游完整Project树状态恢复。链接点击后焦点保留原控件，不能依据鼠标点击推断键盘焦点，验收以实际焦点截图为准。

### Project 刷新保留展开与选择（2026-10-17）

参考commit仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`。本轮读取：
- `platform/lang-impl/src/com/intellij/ide/projectView/impl/AbstractProjectViewPane.java::createTreeState/saveExpandedPaths/restoreExpandedPaths` 保存/应用树展开状态；无可恢复状态且无选择时选第一项。
- 同目录 `ProjectViewPane.java` 使用AbstractProjectViewPaneWithAsyncSupport；后者updateFromRoot调用当前support.updateAll。`ProjectViewPaneSupportService.kt` 根据ide.project.view.coroutines选择AsyncProjectViewSupport或CoroutineProjectViewSupport。
- `AsyncProjectViewSupport.java::updateAll/setModelTo` 通过invalidateAsync更新现有模型并注册RestoreSelectionListener；`CoroutineProjectViewSupport.kt::updateAll/updateImpl/setModelTo` 同样invalidate/awaitUpdates并注册选择监听器。不是刷新按钮主动清空所有展开节点。
- `platform/platform-impl/src/com/intellij/ui/tree/RestoreSelectionListener.java::valueChanged` 在选择失去后延迟恢复，只在没有新选择时执行；沿旧路径寻找仍可见节点，完全消失时回第一项。

本地 `src/project_files.rs::refresh_expanded` 接收已展开路径及每个目录最后已加载Entry作为排序边界，父目录优先后台分页读取；仅重读旧展开范围，新目录/关闭目录不递归。每页沿用256项，整个新快照最多DISPLAY_NODE_LIMIT=16384项；预算不足保留上一个cursor，不能跳过未接纳的页。目录删除/变成文件或链接后不恢复旧子树；失败子树省略并反馈，兄弟目录继续；取消返回Cancelled。`src/project_view.rs::refresh` 取消旧读取与待发预览，应用结果校验generation；刷新期间保留当前树，完成后按当时选中路径重建，路径不存在回最近可见祖先，不抢正文焦点或重新打开Tab。折叠会取消刷新；已折叠根正常刷新后仍折叠。目录读取期间变化用目录专用摘要。

差异/边界：没有VFS/PSI常驻模型，使用有界新快照替换；读取期间可能同时持有旧/新两份有界目录元数据，文件正文不复制。排序边界恢复可能多读一页；超过预算显示限制，未承诺任意外部插入后一定保持远处选择。根读取失败会清掉旧树，因此失败后重试不保证还原此前所有展开路径；未做跨会话树状态持久化。选择恢复沿相对路径，不追踪外部重命名的文件身份。尚未完成慢读交互竞态、全部视口恢复或像素验收，源版本与安装版本差异继续保留。

### Project 失败重试保留树上下文（2026-10-17）

继续依据同commit的AbstractProjectViewPane::saveExpandedPaths/restoreExpandedPaths及RestoreSelectionListener::valueChanged（上一节已读）：恢复以路径状态为依据，不能覆盖后续主动选择。本轮是文件系统快照实现的失败恢复适配，不声称上游也使用同名失败记录。

src/project_view.rs新增refresh_restore/restore_selection，保存每个展开目录的最后加载边界与选择身份。根失败仍移除旧列表并呈现错误，轻量记录跨连续重试保留，恢复成功后释放；子树失败/超限时同样保留供重试。普通选择统一经过select，清除旧选择恢复意图；实际展开/分页/折叠清除旧展开计划，避免稍后恢复重新打开用户已收起的目录。刷新完成优先尊重这期间的新选择。刷新遇到尚未加载完的首次展开，会保留该请求路径，不能因尚无entries误判为用户已折叠。切仓随ProjectView实体销毁释放，不持久化磁盘。

仍保持256项分页与16384节点上限；新增记录只持有边界Entry/路径，无完整目录副本、文件正文或Editor实体。全部视口状态、外部重命名身份追踪及慢读取中的原生竞态仍待核对，未扩大为VFS模型或后台监听。

### Project 延迟预览的主动取消与来源身份（2026-10-17）

同commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，本轮重读 `platform/platform-api/src/com/intellij/ui/AutoScrollToSourceHandler.kt`：SingleAlarm默认100ms，触发时检查component.isShowing与焦点，选择变化cancelAndRequest；鼠标点击先cancelAllRequests再立即导航。cancelAllRequests是显式取消入口，不能单靠某一时刻可见性来表示请求仍有效。

本地src/project_view.rs新增preview_generation和cancel_preview，取消时释放延迟任务并递增请求序号；延迟/即时Open携带序号。src/ui.rs订阅同时验证事件来自当前ProjectView实体、当前请求序号及Project可见性，避免切仓后来源碰撞或迟到事件。切换/关闭中央Tab、Project收起/展开、切换Commit/Stash/Compare与打开设置均主动取消；普通键盘预览仍100ms合并且不抢树焦点。已有普通Tab保持普通、显式Enter打开等语义不变。

本地取消入口覆盖是GPUI生命周期适配，不声称上游每个相同入口直接调用cancelAllRequests；依据是上游可见/焦点约束和取消语义。旧本地版本仅在计时结束检查焦点/可见，Windows快速收起再展开已实证旧请求错误打开文件；新版本同条件已阻止，并验证正常预览仍有效。对切仓/模态/所有Diff入口的完整原生竞态仍待专项；来源EntityId加序号仅是代码层保护，不把代码检查替代全部场景验收。


### 设置模态与待发 Project 预览验证（2026-10-17）

重读参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/platform-api/src/com/intellij/ui/AutoScrollToSourceHandler.kt::getOrCreateAutoScrollAlarm/cancelAllRequests，除100ms与焦点约束外，SingleAlarm还使用ModalityState.defaultModalityState。本地没有Swing模态调度；src/ui.rs::open_settings通过cancel_project_preview显式取消既有延迟请求，是轻量适配而非同一调度器实现。原生artifacts/project-preview-modal证实打开设置时不替换当前文件，关闭后正常预览仍可用。本轮未改变源码；其他业务弹窗不能由此推定通过。参考源码与已安装Rebased 1.1.20 / 262.10968.SNAPSHOT的版本区别不变。


### Project 默认文件名过滤（2026-10-17）

本轮实际读取参考4aec35256d765c217aa0d662b82ac4e860c396fd：
- platform/lang-impl/src/com/intellij/ide/projectView/impl/nodes/ProjectViewDirectoryHelper.java::directoryChildrenInProject/shouldBeShown/processPsiDirectoryChildren：枚举当前目录、按项目索引和视图设置过滤，普通文件/目录分别建节点。
- platform/lang-impl/src/com/intellij/ide/projectView/ProjectViewSettings.java：isShowExcludedFiles默认true。
- platform/projectModel-impl/src/com/intellij/openapi/roots/impl/ProjectFileIndexImpl.java::isInProjectOrExcluded：接纳项目内/EXCLUDED，未将IGNORED视为EXCLUDED。
- platform/platform-impl/src/com/intellij/openapi/fileTypes/impl/FileTypeManagerImpl.java::DEFAULT_IGNORED/isFileIgnored与impl/copy1/IgnoredPatternSet.java：独立的文件类型忽略名单，不是Git .gitignore匹配器；名称包括缓存/备份/版本控制管理目录。
- jps/model-impl/src/org/jetbrains/jps/model/fileTypes/impl/FileNameMatcherFactoryImpl.java与jps/model-api/src/com/intellij/openapi/fileTypes/{ExactFileNameMatcher,ExtensionFileNameMatcher}.java：精确名称默认大小写敏感，*.扩展名不区分大小写。
- platform/core-impl/src/com/intellij/psi/impl/file/PsiDirectoryImpl.java::processChildren：当前目录子项转PSI，不是通用点文件过滤。

本地src/project_files.rs::hidden_in_project在两个枚举入口、分页预算计算前应用默认名单；刷新复用相同分页规则。普通.env/.hidden/.gitignore保留，Git忽略文件不由这份名单排除。无Git子进程/全仓索引，固定名称匹配不增加常驻文件集合。presentation过滤不改变read的权限或使已打开正文失效；.git仍保留已有路径拒绝规则，在Windows大小写不敏感（相对上游默认名称匹配的刻意平台适配）。名单适用于文件和目录，不只文件扩展名。

保留差异：不复制IDE模块/PSI索引、项目排除目录、可编辑全局忽略名单与语言插件规则；不把OS hidden属性一概排除。本轮补齐默认名称名单，不宣称完整IDE项目模型一致。安装Rebased 1.1.20 / 262.10968.SNAPSHOT与源码快照不同。来源声明加入docs/THIRD-PARTY.txt，Apache-2.0全文加入assets/themes/APACHE-2.0.txt以沿用现有打包许可收集。


### UI/UX 集中修复：图标、筛选、菜单与欢迎页（2026-10-17）

参考commit仍为4aec35256d765c217aa0d662b82ac4e860c396fd；已安装版1.1.20 / 262.10968.SNAPSHOT，二者不能等同。用户本轮六张截图保存在artifacts/uiux-round-reference/1.png～6.png。

本轮实际读取与提取规则：
- platform/icons/src/expui/toolwindows/{commit,project,changes}@20x20_dark.svg、expui/fileTypes/{c,cpp,shell}_dark.svg、language/{rust,python}_dark.svg：采用上游资源，保持路径/颜色/viewBox，仅放大固有尺寸以供缓存HiDPI光栅；来源和Apache-2.0声明追加assets/icons/SOURCE.txt。gpui-component Button源码确认会用自身icon_size覆盖传入Icon.size，因此在按钮显式设置20px。Commit/Project使用对应工具窗口图标；Stash沿用上游Shelve图标并增大，Changes工具窗口使用Changes图标。
- FilterPopupComponent.java::createResetAction将单个model设null；VcsLogPopupComponent.kt提供重置动作及快捷键。src/ui.rs与user_menu.rs/paths_menu.rs为Branch/User/Date/Paths添加独立×，各自清除并走原查询刷新，不清其他筛选，不打开下拉。选中分支显示名称，用户显示User:值；路径沿用现有摘要。保留差异：使用相邻GPUI按钮实现命中区域，未照搬Swing组合控件。
- plugins/git4idea/backend/src/actions/GitCheckoutActionGroup.kt：取提交上的本地分支、排除当前分支，有分支则Checkout子菜单列分支与Checkout Revision，否则直接提交检出。src/ui.rs历史右键复用start_local_checkout/start_checkout；单选和操作状态控制可用性，执行器仍复验HEAD/目标，不新增另一套写操作。
- platform/platform-impl/src/com/intellij/ide/actions/{CloseAllEditorsAction,CloseAllEditorsButActiveAction,CloseAllEditorsToTheRightAction,CloseAllReadonlyEditorsAction}.java：基于上下文Tab/实际顺序关闭，Other保留上下文Tab、Right按其右侧，readonly依据编辑器属性。src/ui.rs::editor_tab_menu按EntityId快照后重新解析当前索引，复用现有关闭/取消/释放；先关非活动Tab，活动关闭后定位幸存Tab。Log保留且Close禁用；本产品普通文件和Diff均只读，因此Close All Read-Only关闭全部编辑器Tab。没有加入未实现的Pin/Split/Reopen占位。
- CommitDetailsListPanel.kt将详情放入JBScrollPane，垂直AS_NEEDED/水平NEVER。src/ui.rs实际详情滚动容器绑定details_scroll并显示溢出时常见的垂直Scrollbar；未进行滚动自动化。
- RecentProjectsManagerBase.kt::markPathRecent通过remove/put重排路径；RecentProjectListActionProvider.kt以路径为身份组织列表；ReopenProjectAction.kt在打开时检查缺失路径并提供移除。src/recent_projects.rs保存最多20项/64KiB的独立用户偏好；仅成功读取仓库后记入，最近在前、规范路径去重，不在欢迎页扫描仓库或加载Diff。欢迎页列名称/路径、打开/移除，复用统一open_repository清理和加载。差异：单窗口客户端原窗口打开，无项目分组、搜索、新建/克隆或后台分支查询；移除只删偏好、不删项目。
- intellij.vcs.git.backend.xml::Git.Log.Branches.Change.Branch.Filter绑定双击与Enter；BranchesDashboardActions.kt::UpdateBranchFilterInLogAction调用controller；BranchesTreeSelection.kt::selectedBranchFilters将Head节点映射VcsLogUtil.HEAD；BranchesInGitLogUiFactoryProvider.kt::filterBy只替换branch-like过滤。当前src/ui.rs HEAD双击入口已走select_branch("HEAD")，本轮原生复核，不将既有行为冒充新增实现。
- EditorHighlighterFactoryImpl.kt按文件类型创建Lexer/highlighter，未知类型回PlainSyntaxHighlighter。src/project_view.rs::FileTab按扩展名给现有只读EditorState设置语言，复用tree-sitter与已安装Islands配色，未知类型plaintext；仍仅持有活动正文与高亮状态，非活动Tab释放。该轻量实现不复制IDE语义索引/检查/LSP；语言支持受当前grammar集合约束。

文件图标统一入口仍为src/ui.rs::file_icon，Log/Commit/Stash/Project/Compare与普通文件Tab均使用相同映射和彩色img绘制，补Rust/Python/C/C++/Shell；避免把同一彩色SVG交给单色Icon mask导致形状/颜色不一致。原生验证和当前包状态见PLAN本轮记录；未宣称全部DPI或像素还原完成。

## 关闭编辑器的内存保留与 Git 所有权错误（2026-10-17）

参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版本为 Rebased 1.1.20 / 262.10968.SNAPSHOT，不能将源码快照等同于安装版。

已读 `platform/platform-impl/src/com/intellij/openapi/editor/impl/EditorFactoryImpl.kt::releaseEditor`：释放编辑器，通知生命周期监听器，并从会话编辑器登记移除。本地 `src/project_view.rs` 已在关闭／停用时卸载 viewer，但 `vendor/gpui-component/src/root.rs` 的 focused_input 强引用会延长其生命。改为 `WeakAnyInputState`，读取或打开菜单时临时 upgrade；公开 focused_input API 不变。`tests/focused_input_release.rs` 在无 getter 清理的情况下移除焦点编辑器，修复前确实失败、修复后通过。测试特性 `desktop-tests` 只直接启用组件 test-support，避免上游 Kit 的平台测试依赖冲突。

已读 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/VcsLogMainGraphTable.java::updateEmptyText` 与 `ui/table/VcsLogGraphTable.java::setErrorEmptyText`：加载失败进入 Log 空态，错误文本截短至 150 字符、换行变空格，附刷新入口；过滤失败另有清除过滤动作。Git 插件对应目录搜索未发现 `dubious ownership` 专门处理器；`GitCommandNotTrustedException` 是 IDE 项目信任，不能混同 Git 的 safe.directory。故目前只能确认通用 Log 错误路径，未宣称安装版专门错误弹窗已实测。

本地 `src/git_error.rs` 识别 Git fatal ownership 诊断，`src/ui.rs::history/show_error_details` 在空 Log 内显示本地化短说明、刷新与详情；完整输出保留在有关闭入口的详情弹窗及复制按钮。仓库未加载成功时隐藏无意义的底部 Diff 错误副本。底部状态栏保留简短“仓库访问被阻止”。差异：本地使用语义摘要而非上游原始错误前 150 字符，以支持中文和较窄窗口；不自动修改 safe.directory，也不把所有权错误映射为普通无提交仓库。

## 文件／选区历史与 Jump to Source（2026-10-18，实施中）

commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，与安装版 Rebased 1.1.20 / 262.10968.SNAPSHOT 有版本区别。截图索引 `artifacts/file-history-reference`。

已读：
- `platform/vcs-impl/src/com/intellij/openapi/vcs/actions/TabbedShowHistoryAction.kt`：编辑器文件可作为历史查询目标，新版 provider 打开文件历史；旧版 fallback provider 单文件历史。
- `platform/vcs-log/impl/src/com/intellij/vcs/log/history/FileHistoryPanel.java`：文件历史使用独立 Log／详情／Diff 结构；本地历史 Tab 先以版本表和预览实现，尚保留 graph、详情分栏的结构差异，不能称像素还原完成。
- `plugins/git4idea/backend/src/history/GitFileHistory.kt`：先按当前路径 log，遇创建边界后逐父检查重命名再查询旧路径，去重 revision；不以整段 `log --follow` 替代，避免合并内重命名丢历史。`tests/git4idea/history/GitFileHistoryTest.kt` 已读 cyclic rename、appendable history、merged rename 用例；本地真实Git复现合并内rename最初失败，补父差异后通过。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/actions/SelectedBlockHistoryAction.java`、`com/intellij/vcsUtil/VcsSelectionUtil.java`、`platform/vcs-api/src/com/intellij/vcsUtil/VcsSelection.java`：有选区采用起止行（终止所在行也包括），没有选区退到光标所在行；打开独立历史框架窗口。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/history/impl/VcsSelectionHistoryDialog.java`：本地版本先于已提交版本，逐版本创建 previousBlock；Changes only 根据块内容差异过滤；默认选首项，单选与下一版本块比较，上方Diff、版本/日期/作者/说明表、底部完整提交说明。
- `platform/vcs-impl/src/com/intellij/diff/Block.java` 与 `platform/diff-impl/src/com/intellij/diff/util/DiffUtil.java::updateRangeOnModification`：忽略空白匹配行，非空块使用 greedy 损坏边界映射，空块不重新扩张。翻译到 `src/file_history.rs`；本地 Patience 行匹配与上游 ByLine 有算法差异，比较超时明确中止选区查询，不将超大比较的近似结果当完整历史。
- `platform/diff-impl/src/com/intellij/diff/actions/impl/OpenInEditorAction.kt`、`tools/fragmented/UnifiedDiffViewer.java::getNavigatable`、`contents/DocumentContentBase.java::MyNavigatable`：无可导航文件时禁用；按当前侧选择目标、将旧文档行映射到当前文档，并请求聚焦编辑器。本地 `src/diff_tab.rs`／`src/ui.rs::jump_to_source` 翻译为安全只读Project文件Tab，`src/project_view.rs` 应用行定位；不增加编辑权限。

本地 `src/history_view.rs` 承载版本表与选区窗口，`src/project_view.rs` 的编辑器菜单发出历史请求，`src/ui.rs` 复用现有Tab生命周期。差异／限制：历史单选比较，尚无上游多选版本比较；文件历史表尚未复刻graph；选区Diff行号为块内相对行，标题明确列出原文件行范围；首批100／上限1000条，有更多历史时明确提示；只缓存有界选区块，不缓存所有完整文件。功能及Windows原生验收尚在实施，精确像素对照未完成。

后续接入：历史预览及选区窗口也提供 Jump to Source／F4；选区先加回该版本的块起始行，再映射到工作区。`src/history_view.rs::SourceRequest` 携带当前路径、历史revision/path与源行；选区独立窗口经弱引用和原主窗口句柄导航，不把历史内容当工作区源码。`src/project_view.rs` 保存轻量历史版本ID／行／模式／加载上限，停用时释放重型viewer，回来重新按ID恢复选择。

Diff 按钮和键盘动作经 `src/diff_tab.rs::SourceCallback` 传递该 Diff 的不可变 target；`src/push_dialog.rs` 的预览也采用该回调，避免解析底层 Log 的无关选择。源码跳转关闭只读预览/Push草稿弹窗后回到文件Tab，不执行Push；此关闭组织是本地模态栈适配差异。编辑器菜单用既有 GPUI PopupMenu，保留选区与复制行为；关闭默认Win32菜单，避免出现与Rebased不一致的OS菜单。新增界面尚未宣称像素级验收完成。

补读 `FileHistoryUiFactory.java`、`FileHistoryFilterUi.kt` 与 `FileHistoryFilterer.kt::createFilters`：未指定revision的现代文件历史默认过滤HEAD（指定revision则从该commit开始），不是默认全分支；本地HEAD起点与此一致。现代面板另有独立Branch过滤入口和graph，本轮版本表尚不具备这两项，保留为后续界面功能差异，不宣称完整现代文件历史UI已复刻。

原生检查发现工具栏指针焦点先清除Diff视觉选区，造成铅笔按钮回退首行，F4正常。`DiffTab`、`HistoryView` 和内嵌预览分别订阅 `DiffEvent::SelectionChanged(Some(...))`，只保存最后明确源侧／行号；切文件或重建viewer时清空。R2原生重测原版本侧铅笔按钮、历史预览跳转、历史版本恢复及选区窗口导航；空选区清理不会把明确行丢掉。旧失败截图保留为诊断，未计为通过。

### 文件历史 graph 与独立 Branch 条件（2026-10-18）

同一参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版仍为 1.1.20 / 262.10968.SNAPSHOT。本轮实读 `FileHistoryFilterUi.kt`：独立 BranchFilterModel，初始条件与主 Log 无关，clearFilters 设 null；`FileHistoryFilterer.kt::createFilters` 默认 HEAD；`FileHistoryPanel.java` 使用 VcsLogGraphTable，工具栏在表上方，详情/预览分区。复用已对照过的 `DottedFilterEdgesGenerator.kt` → 本地 `filtered_graph::project`，跨未匹配节点的边使用虚线，不能直接将文件历史按相邻行串联。

`src/file_history.rs::load_scoped` 冻结 HEAD／本地／远程／commit 类型 tag 的 OID，逐起点沿现有 GitFileHistory 重命名规则获取有界元数据，再按真实拓扑排序／投影。`Revision.parents` 不被投影边重写。`src/history_view.rs` 复用主 Log 的 graph_cell，提供独立 Branch 下拉与清除 ×；轻量 ReadingState 保存条件及选中的 revision，停用时仍释放重型 viewer。选区历史继续使用原版本表和 Changes only，不受文件历史 Branch 条件影响。

补读 `FileHistoryModel.java::getSelectedChange` 与 `FileHistoryUtil.kt::createChangeToParents`：单选使用 graph 的父行，多选取末行；多父 change 合并最终以前面的父版本构造 before。本地单选按真实第一父版本读取，逐父差异检查其重命名前路径，避免误比同列表中相邻的另一分支。保留差异：尚未复制多选版本比较及上游多父 changes merger；与单父、跨无关提交的内容比较等效，合并提交使用第一父语义。文件历史详情仍为底部提交说明，没有复制完整 IDE 详情组件。精确间距／颜色／多 DPI 验收仍延后。

低内存边界：显示首批100／最多1000；引用最多4096、去重起点最多128、元数据最多16000／16MiB；临时祖先拓扑最多100000、边最多400000、lane 最多512，查询完成即释放拓扑。超过预算明确加载失败，不伪造直线图或将截断节点当根。此首版在大于100000祖先的仓库可能触发预算，尚未做逐页祖先索引；完整文件正文仅在选中版本时按需读取。引用与历史读取保持只读。

### 文件历史打开后的焦点与关闭快捷键（2026-10-18）

仍对照 `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版版本区别不变。新读 `platform/platform-impl/src/com/intellij/ide/actions/CloseEditorAction.java`：事件没有 EditorWindow 时回退 manager.getCurrentWindow，再关闭当前上下文文件；不要求正文获得焦点。补读 `FileHistoryPanel.java` 的 ComponentsListFocusTraversalPolicy：graph table、Diff 首选控件、toolbar 的遍历顺序。

本地性能脚本发现：从源码右键打开历史后立即 Ctrl+F4 无效，点击历史行再按却成功。小仓库原生脚本 `artifacts/file-history-close/repro.ps1` 失败（关闭后复制不到源文件全文），`focus-probe.ps1` 点击列表后通过，排除文件规模及发键无效；原因是绑定仅存在于 ProjectFileTab，而历史激活未将焦点转入其列表。`src/ui.rs` 改在 Review 上下文关闭当前活动文件／Diff，复用原有释放和邻近 Tab 返回逻辑，dialog/sheet 打开时不关闭背后 Tab。`src/project_view.rs`／`src/history_view.rs` 激活历史时一次性聚焦列表，不在每次渲染抢焦点。独立选区窗口继续以 Esc 关闭，未引入其关闭与主窗口联动。

### 文件历史多选比较（2026-10-18）

参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 / 262.10968.SNAPSHOT 与该快照不同。本轮重读 `FileHistoryModel.java::getSelectedChange`：单选沿 graph 父行，多选使用 rows[0] 与 rows[rows.length-1]，不按鼠标最后点击的 lead 决定左右端点。重读 `FileHistoryPanel.java` 的 VcsLogTableCommitSelectionListener，详情随所选 commits 加载；新读 `ui/frame/VcsLogCommitSelectionListenerForDetails.kt` 的 onDetailsLoaded／MAX_COMMITS_TO_LOAD，详情只展示前50条并显示 overflow 提示。

`src/history_view.rs` 文件历史复用 `log_selection::Selection` 的OID集合／anchor／lead，支持Ctrl切换、Shift范围和Shift上下扩展；选区历史保留原单选表。`src/file_history.rs::comparison_rows` 按当前显示顺序计算端点；两端完整文件按需读取，rename分别使用各版本真实路径。原／新侧Jump to Source分别携带对应版本，不能误用多选lead。ReadingState只保存轻量选择模型，Tab停用释放viewer，刷新／重新激活按OID与现有图交集恢复；筛选改变仍从首项开始。详情前50条提交说明，溢出使用既有本地化提示。

保留差异：本地单选继续真实第一父语义，尚未复制上游多父changes merger；详情仍是底部简化区域，未完整复刻CommitDetailsPanel。多选是在现有文件历史内比较，不增加另一套历史窗口／缓存，也不增加编辑／Git写操作；显示上限和祖先预算不变。精确像素及复杂合并的完整验收没有因本轮功能加入而自动通过。

补读 `CommitDetailsListPanel.kt` 的 JBScrollPane 配置：垂直 AS_NEEDED、水平 NEVER。历史提交说明区域复用主Log的 ScrollHandle／Scrollbar，溢出有可见滚动条；未进行滚动自动化。多选全部取消时清空正文及源码导航；有版本但两个端点都无文件内容时显示“没有差异”，不把完成后的空预览写成仍在读取。

### 阅读面板错误反馈统一（2026-10-18）

参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版仍为1.1.20 / 262.10968.SNAPSHOT。本轮重读 `VcsLogGraphTable.java::setErrorEmptyText`（原文截至150字符、换行压平）及 `VcsLogMainGraphTable.java::updateEmptyText`（错误空态附刷新／清过滤动作）；重读 `SavedPatchesChangesBrowser.kt` 的 LoadingResult.Error 分支，错误属于变更浏览区域而非整窗长诊断。补读 `MessageDialog.java::createActions` 与 `DialogWrapper.java::createSouthPanel`：默认动作关闭对话框，主要操作在底部右侧按钮组。精确主题尺寸尚未验收。

`src/error_ui.rs` 复用既有Log详情弹窗并提供短说明／Refresh／Details；诊断仅在用户打开时进入有界高度弹窗，可复制完整原始输出，有明确Close、X、Esc／Enter关闭入口。复制与关闭使用底部内容宽度按钮组，避免正文内复制按钮拉伸成整行。现有所有权解释保留，不自动修改safe.directory。

映射：`src/stash_view.rs` 的列表失败放在记录区域、正文失败放在变更区域；原来直接铺满的error及100px原始操作诊断改为短说明与详情；恢复记录失败也复用入口。`src/history_view.rs`／`src/compare_view.rs` 添加同位置刷新和详情。`src/project_view.rs` 保留已有语义／重试规则，IO诊断从仅复制改为详情弹窗；`src/ui.rs::show_error_details` 复用同实现。工作区操作、查询、选择恢复和取消规则未改。

保留差异：上游Log原文短截取，本地采用面板语义摘要并单独保存完整诊断，以支持中英文且避免UNC路径／命令铺开；Details弹窗是本地轻量适配，不宣称上游所有面板有完全相同的Details动作。不新增通知历史、不自动信任目录／执行恢复操作。本文未将完整通知机制或所有错误种类验收记为已完成。

Stash列表读取与恢复记录检查可能返回同一底层Git诊断。渲染时仅在两条诊断完全相同时隐藏恢复检查的重复提示，原诊断状态保留；不同错误仍分别可见。该规则为本地双读取流程适配，未声称上游存在同名去重逻辑。恢复配置与记录内容均未改变。

### Diff 正文导航位置与首次源码加载（2026-10-18）

继续实读 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/tools/util/side/TwosideTextDiffViewer.java::getNavigatable`：从当前侧编辑器的 `LineCol.fromCaret` 导航，当前侧不可导航时映射到另一侧。重读 `tools/fragmented/UnifiedDiffViewer.java::getNavigatable`：以单栏光标逻辑行转为原／新侧行号，优先有效导航目标，否则按当前行所属侧选择。没有文本选区也应能导航。安装版仍是1.1.20 / 262.10968.SNAPSHOT，不能将源码快照视为完全同版。

本地 `vendor/gpui-component/src/diff/{mod,state}.rs` 为源行正文按下记录独立位置，文本拖选按选择游标的稳定文档offset更新；行号选择同步更新。位置保留跨工具栏焦点，替换文件清空，不强制把正文点击变成整行选区。`src/diff_tab.rs`、`history_view.rs` 和 `ui.rs` 的嵌入预览统一读取该位置；Push等已复用DiffTab的预览随之接入。`src/project_view.rs` 等编辑器创建后才消费待定位行，避免异步加载期间丢失。版本至工作区的行映射沿用已实现逻辑，不改文件内容。

保留差异：只读Diff尚无完整可编辑光标系统，记录源行而不记录列；源码定位到对应行首，键盘普通光标移动也不宣称与JetBrains编辑器完全一致。当前侧无导航目标时，既有按钮可用性规则仍适用，没有扩展全部另一侧回退场景。新增测试覆盖正文点击、单／双栏来源、替换文件丢弃旧位置以及编辑器异步创建前后的待跳转生命周期；Windows实测及失败跑次见PLAN，不宣称完整像素还原。

### 文件历史详情区域与提交卡片（2026-10-18）

实读同commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/vcs-log/impl/src/com/intellij/vcs/log/history/FileHistoryPanel.java` 构造：CommitDetailsListPanel由所选提交加载，table和details构成纵向OnePixelSplitter（0.7），该区域再作为FrameDiffPreview主区域；详情并非主Log那样直接放右侧。此前本地固定90px提交说明位于Diff之后，与此区域关系不同。安装版仍为1.1.20 / 262.10968.SNAPSHOT，未断言此源码等同全部安装版设置。

进一步实读 `ui/details/CommitDetailsListPanel.kt` 的构造／rebuildPanel：多提交纵向卡片，卡片间分隔线，整体垂直滚动、无横向滚动，并有加载／空态。`ui/details/commit/CommitDetailsPanel.kt` 的构造、setCommit、setRefs、setBranches：先说明，再hash／作者元信息，分支与Tag分开，最后包含分支；已有本地commit_presentation／commit_refs／ContainingBranches承担这些格式。`ui/frame/VcsLogCommitSelectionListenerForDetails.kt::onSelection/onDetailsLoaded/onEmptySelection/onError`：选择变更取消旧查询，最多50条，refs随对应OID，包含分支独立异步加载。

本地 `src/history_view.rs`：普通文件历史将列表／详情组织为可调整纵向区域，再与下方Diff拆分；选区历史保留原独立窗口的Diff→版本表→提交说明布局。文件历史详情以不可变OID查询snapshot及commit_metadata，复用主Log提交说明／作者提交者本地化／refs图标与串行ContainingBranches::Batch，详情前50条并提示overflow；加载和错误在详情区域，切选择清空旧卡片并取消metadata和包含分支工作，generation拒绝迟到结果；Tab停用仍释放HistoryView，只保留原轻量ReadingState。正文按原Diff规则按需加载，不为每张卡片缓存文件全文。详情格式和refs渲染复用现有许可证来源。

保留差异：暂无外部检查状态／签名徽章、从提交说明内hash跳回主Log、显示详情开关和分割比例跨重启持久化；复用既有ContainingBranches错误样式，并非所有诊断均已统一。精确间距／配色／字体与多DPI验收继续延后；分割结构和真实信息已经确定，后续不以替换整页为实现像素细化方式。

### 文件历史图父版本、状态合并与 Diff 请求模型（2026-10-18）

同commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版仍为1.1.20 / 262.10968.SNAPSHOT。重读 `history/FileHistoryModel.java::getSelectedChange`：单选取当前可见graph父行，多选取显示顺序首末；`FileHistoryUtil.kt::createChangeToParents/MyVcsChangesMerger`：deleted对应无ContentRevision，双侧都不存在时跳过该父比较；多个有效比较交给mergedChange，合成状态的新before用第一个graph父版本ID，并非真实第一父ID。

新读 `impl/VcsStatusMerger.kt::merge(path,statuses)/VcsChangesMerger`：同类型直接保留第一个；同为rename但来源路径不同转modification；混合类型deleted优先，否则modification。文件历史调用单路径merge，不使用主Log的多路径交集入口。实读 `impl/test/com/intellij/vcs/log/impl/VcsStatusMergerTest.kt` 的simpleMerge、renamedModified、addedModified、deletedRenamed；测试中多路径交集行为与单路径merge须区分。补读 `history/FileHistoryTest.kt::historyWithUndetectedNonTrivialRename`，该测试说明图父遍历和路径跟踪需要分开，不能用邻近列表行或真实第一父冒充。

沿请求链实读 `history/FileHistoryDiffProcessor.kt::getCurrentRequestProvider`、`ui/frame/VcsLogAsyncChangesTreeModel.kt::createDiffRequestProducer`、`impl/MergedChange.java::SimpleMergedChange/getSourceChanges` 与 `impl/MergedChangeDiffRequestProvider.java::MyProducer/createRequest`：恰有两个有效sourceChanges时，优先构造left-parent / merge-result / right-parent三栏请求；left为空改center/right，right为空改left/center，center为空改left/right。其他父数使用合并后的普通两栏请求。不是所有合并提交都固定比较同一个父版本。

本地 `src/file_history.rs::selected_comparison/comparison_to_parents` 按已加载graph边选择端点，逐父应用缺失／rename／合成状态规则；保留source_before供请求层使用。`preview_revisions`翻译上游三栏／缺失侧降为两栏的顺序。返回值只携带OID／历史路径，清空非内容元数据，不为每个父版本复制长提交说明或完整文件。已知边指向未保留的元数据时明确要求加载更多，不能把cache缺项当文件新增；额外已读older项可以作为边界端点。history查询本身仍有分页／祖先预算，当前图不代表无限历史。

`src/history_view.rs` 保留ScopedHistory.links，使用请求模型替换固定真实第一父的diff-tree查找，原／新侧导航随真实端点；旧previous_revision接口只保留既有第一父查询与测试，不再作为普通文件历史UI默认路径。选区块历史仍沿原块追踪机制，不随这次改变。

未完成差异：三栏GPUI渲染仍需接入；当前三侧请求暂以明确标注“父版本”的left-parent→merge-result两栏呈现，不能宣称完整合并Diff还原。查询层和请求顺序已保留全部两父身份，后续接三栏不得重新从真实第一父取内容。混合状态合成路径可能在第一graph父版本中不存在；读 `plugins/git4idea/backend/src/GitContentRevision.java::getContentAsBytes/loadContent` 确认其也是按OID／路径交给Git读取，不能通过任意挑选另一个有效blob掩盖。两父请求保留各自来源，三栏接入后无需依赖该合成路径；更多父数沿上游普通合成比较。精确视觉、完整复杂合并矩阵及新的性能基线未完成。

### 三栏合并结果 Diff 渲染（2026-10-18）

参考commit仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，与已安装Rebased 1.1.20 / 262.10968.SNAPSHOT不同。实读 `platform/diff-impl/src/com/intellij/diff/tools/util/text/SimpleThreesideTextDiffProvider.java::compare/getConflictType/invertConflictType`：比较left/result/right，MERGE_RESULT翻转INSERTED／DELETED，MODIFIED／CONFLICT保留；三侧共同未变化行是对齐依据。新读 `platform/util/diff/src/com/intellij/diff/comparison/ComparisonMergeUtil.kt::FairMergeBuilder/ChangeBuilder`：相交两组center→side的Equal区间，joint Equal之间的gap构成一个三侧片段；相邻但没有共同Equal的修改不擅自拆成独立片段。新读 `platform/util/diff/src/com/intellij/diff/util/MergeRangeUtil.kt::getMergeType/getLineThreeWayDiffType` 及 `MergeConflictType.kt::isChange`：center在变更片段中总高亮，左右仅高亮相对center有变化的一侧；空区间、相同父修改与不同父修改有不同分类。实读 `platform/diff-impl/tests/testSrc/com/intellij/diff/comparison/LineComparisonMergeUtilTest.kt::testSimpleCases` 作为空文本、相同文本及插入块用例入口。

渲染与导航继续读 `tools/simple/SimpleThreesideDiffViewer.java::performRediff/apply`、`SimpleThreesideDiffChange.kt`、`ThreesideDiffChangeBase.kt`：按三侧片段创建高亮、安装折叠并恢复导航。`tools/util/side/ThreesideTextDiffViewer.java::getNavigatable/MyOpenInEditorWithMouseAction` 从当前编辑器及其光标取内容，不固定center。结合此前已读DocumentContentBase的历史→当前行映射。

本地：`src/three_way.rs` 翻译共同Equal拼接、片段分类与MERGE_RESULT颜色方向；使用Patience匹配替代上游ByLine，这是记录在案的算法差异，重复行的具体片段边界不宣称全等。保留原始文本／换行，行索引零起点，不为padding生成源行。每来源5MiB，最终最多100000对齐行；收集行时先take上限，避免先为海量换行分配无界索引；比较超时／取消明确失败，不静默用粗略全文件替换当精确比较。

`vendor/gpui-component/src/diff/{three_way,state,mod,selection,horizontal}.rs` 增加三列布局投影，复用虚拟List、代码着色、窗口复制和滚动条；共享纵向行列表与横向offset，每列独立裁剪。ThreeWayRow输入必须每列按真实顺序包含全部源行一次，否则拒绝；padding不参与选择或复制。内部列身份仅用于索引和复制，标题／导航使用真实历史路径与OID。跨栏文本拖选锁定起始列，Ctrl+A使用最后活动列；换文件清除三栏布局，关闭不持有已移除viewer。F7切换变更时清选择、导航位置优先center对应真实行。`src/history_view.rs` 消费全部三侧请求，显示parent/result/parent及真实标题，三侧各自可F4／铅笔导航；多选／缺侧继续普通两栏，保留原用户单／双栏偏好。无编辑／提交动作；Git读取仍按需且可取消。

保留差异：当前三栏按行高亮，尚无BY_WORD词内强调、JetBrains分隔连接曲线、独立横向滚动／同步开关及三栏专用显示设置；精确颜色与间距未完成同条件像素验收。通过共同虚拟行对齐替代三个独立Swing编辑器的同步／对齐inlay，避免复制padding进入源码。文件／行预算与只读范围保持；binary／不支持编码仍使用既有不可预览提示。完全空列无可选择源行，未扩展完整编辑器caret行为；默认导航合并结果。不得将本轮功能和单一内存场景外推为完整发布验收。

### 三栏合并结果的词内强调（2026-10-18）

同一参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版Rebased仍为1.1.20 / 262.10968.SNAPSHOT，未将源码版本等同安装版。

本轮实读 `platform/diff-impl/src/com/intellij/diff/tools/util/text/SimpleThreesideTextDiffProvider.java::compare/getChunks`：fine highlighting对整段MergeLineFragment取三侧内容，未变化侧与空侧传null，不按对齐后的显示行单独配对。`platform/diff-impl/src/com/intellij/diff/util/DiffUtil.java::compareThreesideInner`：三侧都有内容时比较center→left、center→right，center收集两组范围；只有两侧时比较这两侧；少于两侧不做词内强调。`tools/util/text/MergeInnerDifferences.java`说明同一侧范围可重叠且无序。`tools/simple/SimpleThreesideDiffViewer.java::performRediff/apply`与`SimpleDiffViewerHighlighters.kt`将inner范围交给高亮层，不修改文档或行对齐。`comparison/ByWord.java`实际委托ByWordRt；未读其完整算法，不能宣称本地分词等同ByWordRt。

本地 `src/three_way.rs::inner_ranges`按连续三侧变更块取真实源文本，执行相同两／三侧选择和center范围并集规则，合并相交范围后拆为真实行内UTF-8字节偏移。插入行不会导致后续词错误地和画面同一行比较。`src/history_view.rs`在已有后台请求中计算，沿用token／generation／关闭生命周期；不新增常驻全文副本。`vendor/gpui-component/src/diff/state.rs::set_three_inner`验证来源、变化状态、UTF-8边界及范围预算，换源／换三栏布局清理；`mod.rs::line_highlights`转换为Tab展开后的display范围，覆盖行级底色并保留语法／选区优先级。仅Word模式使用提供的词级范围，关闭强调不绘制，Character不冒充Word。

适配差异：本地用similar Unicode word分词，未翻译完整ByWordRt的空白／标点整理算法；不声称精确区间全面一致。每块每侧≤32KiB、处理源累计约512KiB、100000个行内范围、比较每次20ms／总250ms，接近超时丢弃该块的词级结果；超限块保留完整行级颜色和源码，不拒绝整个文件历史、也不输出超时后的粗略词范围。取消仍结束整个过期任务。行级三栏对齐沿用已有预算；连接曲线、独立横向滚动与三栏设置仍待办，精确颜色／间距仍未像素验收。

### 宽图垂直边存储／绘制合并（2026-10-18）

参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版仍1.1.20 / 262.10968.SNAPSHOT。本轮实读 `platform/vcs-log/graph/src/com/intellij/vcs/log/graph/impl/print/PrintElementGeneratorImpl.kt::getPrintElements`：相邻上下端点分别产生UP／DOWN print element，节点在边之后绘制；`platform/vcs-log/impl/src/com/intellij/vcs/log/paint/SimpleGraphCellPainter.kt::paintEdge/paintLine`：同列边是节点中心到上下行界的垂直半线，虚线的edgeLength按完整rowHeight计，非垂直边分别计算上下位置。上游是两个print element，不据此声称它也合并存储。

本地保留相同半边拓扑／节点覆盖及虚实线规则，仅对不经过当前节点、from==to且上下dotted样式相同的连续垂直边，在`src/graph.rs::Layout::append_edges`中存为`Segment.full`，相当于原有upper/lower两条；换列、节点接入／父边、样式变化均保留半边。`src/ui.rs::graph_cell`完整垂直线从行顶画到行底，沿原虚线周期与上半段相位起画，不增加或删除父边。Vec预分配按通常压缩后的宽度和父数估算，不改变500000条存储线段／512轨道上限。

实际触发：32条并行分支、10000提交的真实Git窗口，旧638107半段超出500000预算导致全Log错误页；新329099存储段展开后仍638107半段，最大32轨道。该适配用于低内存及降低重复绘制，不能将增加预算或省略分支当优化。随机DAG父边／上下连续样式、分页及虚线测试继续约束拓扑；Windows原生和内存证据见PLAN同名条目，不以核心布局通过冒充所有DPI／滚动帧率验收。

### Commit／Amend 的短错误与详情（2026-10-18）

参考仍为4aec35256d765c217aa0d662b82ac4e860c396fd，与安装版1.1.20 / 262.10968.SNAPSHOT不同。本轮实际读取 `platform/vcs-impl/src/com/intellij/vcs/commit/ShowNotificationCommitResultHandler.kt::reportResult`：根据提交异常生成Failed／SuccessfulWithWarnings等结果，标题是结果摘要，异常有独立Show Details动作；取消是独立警告，不因失败自动Push。`plugins/git4idea/backend/src/checkin/GitRepositoryCommitter.kt::performCommit`保持runCommand.throwOnError及GPG／pinentry分类，不自动略过hooks或签名。

本地 `src/local_view.rs`将写操作结果分成静态本地化summary与原始details，失败在消息框下提供短结果、Details、Dismiss，Commit／Amend按钮仍是用户显式重试入口，保留消息与包含选择。详情复用既有error_ui对话框／复制，合并有界stdout/stderr及index恢复／记录／HEAD/state读取失败诊断，展示副本最多32000字符并明确截断；不再仅保留2000字符stderr并塞入72px裁切区。提交执行、hook／签名设置、恢复事务和Push成功门槛不变。读取失败使用共享Refresh／Details，Amend消息失败使用Retry／Details重新请求原加载动作。

保留差异：Rebased为通知体系，本地仍在Commit工具窗口显示短结果并打开现有详情弹窗；通知浮层／历史按PLAN暂缓，未凭这次错误结构改进宣称完全一致。提交详情仅承诺受输出预算约束的诊断，超出process tail或展示预算的原始输出不是完整日志。动态语言只本地化summary及控件，不翻译Git／hook输出；Windows常用提交／失败重试及Amend验证见PLAN对应记录。

### 文件历史详情显示开关（2026-10-18）

参考commit仍4aec35256d765c217aa0d662b82ac4e860c396fd，安装版仍1.1.20 / 262.10968.SNAPSHOT。本轮实读 `FileHistoryUiProperties.kt::State/get/set`：SHOW_DETAILS默认false，为项目workspace持久属性；`FileHistoryPanel.java::createActionsToolbar/showDetails`：工具栏经显示选项动作组调用布局开关，列表／详情内部splitter第二组件设null或详情；外部列表／Diff分隔独立，默认比例0.7不因隐藏详情改变。`ShowDetailsAction.java`及`BooleanPropertyToggleAction.java`仅切换存在的布尔属性，并以该属性显示checked状态。`intellij.platform.vcs.log.impl.xml`的Vcs.FileHistory.PresentationSettings包含Vcs.Log.LayoutConfiguration→ShowDetails；`VcsLogToolbarPopupActionGroup.java`将popup锚定动作按钮。

图标实际映射已读`platform/icons/src/PlatformIconMappings.json`：AllIcons.Actions.GroupBy（actions/groupBy.svg）映射expui/general/show.svg；导入同commit的general/show_dark.svg为history-show_dark.svg，未修改图形，许可证与来源见assets/icons/SOURCE.txt。

本地`src/history_view.rs`普通文件历史增加显示选项按钮／Show Details勾选项，默认不创建详情区，外层0.7列表／Diff比例保留；仅替换内部分区，版本选择、Diff实体及来源不变。选区历史独立窗口不出现此开关。显示时加载当前所选OID的metadata，隐藏取消details token／task、递增generation、释放卡片及包含分支实体；迟到详情不能恢复已隐藏的内容。上游hidden组件仍保留并可收到selection listener；本地按需销毁是低内存内部差异，不改变重新显示后的可见规则。

`src/layout_preferences.rs`按项目根路径在应用配置目录存一个有界偏好文件，Windows路径分隔／大小写归一，文件名为稳定FNV64索引且正文验证完整项目key，防止碰撞误用其他项目的值；缺失／无效／超过64KiB回默认false。不写仓库或.idea，区别于上游workspace.xml位置。已有单主窗口FileTab停用释放HistoryView，重建时读最新项目偏好，不把旧Tab状态当单独覆盖；分支过滤／刷新不改变此值。路径别名／非UTF8项目名、配置写入异常及多窗口实时同步不是完整IDE配置系统的承诺。菜单checked、刷新、切Tab、重启／项目隔离与原生截图证据见PLAN对应条目；其余显示选项不添加空占位，像素细节仍未完成。


## Windows 剪贴板短暂占用（2026-10-18）

- 参考仍为`4aec35256d765c217aa0d662b82ac4e860c396fd`；已实读`platform/platform-impl/src/com/intellij/ide/ClipboardSynchronizer.java`的ClipboardHandler::setContent／WindowsClipboardHandler::wrapClipboardAccess／getRetries，以及`LocalCopyPasteManager.java`的doSetSystemClipboardContent。安装的Rebased1.1.20／262.10968.SNAPSHOT与参考源码快照分开，不宣称已操作安装版本复现本缺陷。
- 上游：Windows读写在syncRoot内串行；setContent捕获IllegalStateException，Windows getRetries返回5，全部失败后发System Clipboard warning。源码明确指出另一个窗口打开剪贴板时OpenClipboard可失败。API交叉确认：https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard 。源码测试未找到对应实读回归，不冒称移植了上游测试。
- 本地：`vendor/gpui-pre-windows/src/clipboard.rs`的ClipboardGuard::open只对带有效owner的写入最多获取5次，间隔5ms，成功才EmptyClipboard；读仍单次。复用现有平台隐藏HWND；无需JDK同步对象、常驻任务或正文缓存；耗尽后保留既有内容并使用原错误日志，不排队延迟写入。5ms间隔是本地适配（累计请求sleep最多20ms，实际调度时间不作硬时限承诺）。上游warning通知未映射到当前GPUI void写接口，是明确保留差异。
- 原生真实接缝：`platform.rs::tests::test_clipboard_busy_write`使用另一个线程的真实STATIC消息窗口持有系统剪贴板，短占用5ms时单次写应成功；持续占用时有界返回且原内容未清空。旧实现短占用断言失败、保留sentinel，修复后与所有权／Unicode／metadata／NUL往返共2项通过。fixture获取锁允许最多500ms等待，避免环境中的剪贴板监视器导致测试布置失败；这不是重复用户复制。
- 偶发原生流程与受控缺陷区分：无诊断构建曾出现蓝色选区但复制后仍为哨兵；临时边界日志构建的101次同文件及229次完整串联均通过，故没有捕获此前自然失败的根因。受控实验确认了同症状的底层缺陷，但不把它声称为所有偶发失败的已证实原因。最终交付必须移除临时日志并重新验证。

## 主工具栏原生对照（2026-10-18）

应用户要求，实际操作用户已打开的 Rebased（PID 1168）的项目、分支、齿轮菜单，并截图后按 Esc 恢复主界面；保留其 History for Selection 窗口，未触发 Git 操作或更新。原图见 `artifacts/rebased-live-toolbar/project-popup-real.png`、`branch-popup.png`、`settings-menu-correct.png`、`restored-main.png`。安装版本既有记录为 1.1.20；本次菜单显示可下载 1.1.21，不代表已升级。源码仍是入口记录的 4aec35256d765c217aa0d662b82ac4e860c396fd 快照。

本次已读源码：

- `platform/platform-impl/src/com/intellij/openapi/wm/impl/headertoolbar/ProjectToolbarWidgetAction.kt` 的项目 widget、菜单组及 ProjectWidgetRenderer：项目图标、名称、下拉箭头；已打开／最近项目分组；行中名称与较小的路径分两行显示，路径缩写用户目录。
- 同目录系的 `platform/platform-impl/src/com/intellij/ide/RecentProjectIconHelper.kt` 和 `platform/util/ui/src/com/intellij/util/ui/AvatarUtils.kt`：名字首字母、渐变色、圆角比例和图标字体比例。
- `plugins/git4idea/frontend/src/com/intellij/vcs/git/frontend/widget/GitToolbarWidgetAction.kt`、`plugins/git4idea/backend/src/remoteApi/GitWidgetApiImpl.kt`、`plugins/git4idea/backend/src/ui/branch/GitCurrentBranchPresenter.kt`、`plugins/git4idea/backend/src/branch/GitBranchUtil.java`：正常分支使用 Vcs 图标；detached 且精确位于 tag 时显示 tag 名和 BranchLabel，其余非正常状态使用 Warning。
- `platform/platform-impl/src/com/intellij/ide/ui/laf/darcula/ui/ToolbarComboButtonUI.kt`、`platform/platform-impl/src/com/intellij/openapi/wm/impl/ToolbarComboButton.kt`：工具栏组合按钮的文字、图标和箭头组织；`platform/icons/src/PlatformIconMappings.json` 确认 Warning 资源映射。

本地映射：`src/ui.rs` 的 project_menu、project_avatar、project_identity、project_display_path 和 repository_branch_menu；`src/i18n.rs` 增加已打开项目分组翻译；`src/assets.rs` 嵌入原版 warning_dark.svg（出处见 assets/icons/SOURCE.txt）。项目菜单只在打开时克隆有界最近列表，弱引用窗口，不建立额外项目服务／图像缓存。Windows verbatim 与普通路径统一身份用于菜单去重和图标配色，不修改持久化路径。

保留差异：单窗口只列一个已打开项目，最近列表最多 20；未提供 Clone 功能所以不放无行为菜单项；使用路径哈希映射上游 9 组深色渐变，不实现项目自定义颜色／图标服务；未位于 tag 的 detached 状态保留本地“游离 HEAD”说明，进行中操作仍使用既有操作入口。完整分支弹窗、欢迎页布局、菜单分隔线／字号渲染／精确间距仍有视觉差异，本轮不声称像素级完成。

### 顶部分支菜单 Commit 入口（2026-10-18）

同一源码 commit，本次完整读取 `plugins/git4idea/shared/src/com/intellij/vcs/git/branch/popup/GitDefaultBranchesPopupStep.kt`、`GitBranchesPopupStepBase.kt`、`GitBranchesPopupActions.kt`；读取 backend/resources/intellij.vcs.git.backend.xml 的 Git.Experimental.Branch.Popup.Actions 与 Git.Branches.List 分组。Rebased 明确保留 Commit 工具窗口入口，把 Pull／Push 放到主工具栏；New UI 首组动作与后续分支动作之间有分隔。原生 branch-popup.png 同样显示 Commit 在 Checkout Tag or Revision 上方。

进一步读取 `platform/vcs-impl/src/com/intellij/openapi/vcs/actions/commit/AbstractCommitChangesAction.kt`、`CheckinActionUtil.kt`、`platform/vcs-impl/src/com/intellij/vcs/commit/ChangesViewCommitPanel.kt` 的 activate：调用提交 workflow 的激活流程、选择 LOCAL_CHANGES 工具窗口、聚焦 commit message，入口本身不直接执行提交。

本地 `src/ui.rs` 用 repository_actions 列表替代单个 revision 可见布尔值；Commit／revision 共享上游已翻译的搜索、选择及上下／Home／End导航，左键或 Enter 都经 activate_repository_action 关闭菜单后激活已有入口。`src/local_view.rs::focus_message` 聚焦既有说明输入框，不重建侧栏或清空草稿。保留后台操作／设置／欢迎页的既有防重入限制。菜单将 Commit 置于第一组，复用已带来源的工具窗口图标；中英文文案在 src/i18n.rs。保留差异：不加入尚未实现的 New Branch，不增加第二套 staging workflow，不改变现有文件级 Commit 语义；快捷键提示与精确视觉仍待统一验收。

### 菜单与弹窗共用边界（2026-10-18）

本次读取同一4aec快照的 `platform/platform-resources/src/themes/islands/ManyIslandsDark.theme.json`：Menu.separatorColor／Menu.borderColor／Popup.borderColor 共用 popup-border → layer-1-border → gray-40（#33353B）；界面内容层分隔与浮层边界不是同一个色值。读取 `platform/platform-impl/src/com/intellij/ide/ui/laf/darcula/ui/DarculaMenuSeparatorUI.java`：菜单条纹默认1px，颜色取 Menu.separatorColor。已安装 Rebased 的实际项目／分支菜单见上轮 artifacts/rebased-live-toolbar；源码快照与安装版区别仍保留，不假定主题所有颜色一致。

本地排查发现 `Review::new` 将 GPUI theme.border 设成页面分隔色 #26282C，PopupMenu 分隔宽度2px；框架 popover_ring 又单独采用文字颜色10%透明度，故菜单、分支自绘面板及弹窗轮廓各自不同。`src/ui.rs` 增加 POPUP_BORDER，页面 BORDER 保持原有分隔语义；标准控件 theme.border 与分支弹窗外缘／分组线统一为上游浮层色。`vendor/gpui-component/src/menu/popup_menu.rs` 分隔线改1px；`styled.rs::popover_ring` 读取 theme.border，让菜单／Select／Popover 共用既有单层轮廓，不额外增加边框导致双线。只改变共用样式，不增加实体、缓存、查询或事件。来源补入 assets/themes/SOURCE.txt。

保留差异：沿用 GPUI 阴影模型与圆角，分隔周围留白尚未逐像素对齐；当前 Inter UI 字体与用户运行中的 Rebased 系统字体仍需单独核对。本轮不调整字体，不声称全界面或全部 DPI 已完成。

### Windows UI 系统字体（2026-10-18）

本次完整读取参考4aec35256d765c217aa0d662b82ac4e860c396fd 的 `platform/util/ui/src/com/intellij/ui/scale/JBUIScale.kt::computeSystemFontData`：Windows 优先取 Toolkit 的 win.messagebox.font；`platform/platform-impl/src/com/intellij/ide/ui/laf/IdeaLaf.kt::initComponentDefaults` 将该字体以 PLAIN 与 fallback 应用于控件。另读 AppearanceConfigurable.kt::getFontFamily：未设置自定义字体时使用已初始化的系统字体。安装版截图中自定义字体未启用、字体显示 Microsoft YaHei UI（artifacts/rebased-user-reference 的用户设置截图索引），与本次宿主机消息框字体采样一致；不把此主机名称硬编码为所有 Windows 的默认值。

本地此前 theme、主窗口、欢迎页、提交 metadata 与 containing-branches 的测量均硬编码 Inter。`src/ui_font.rs` 通过 SPI_GETNONCLIENTMETRICS 的 lfMessageFont 读取系统消息字体，进程内 OnceLock 只缓存一个有界名称；API失败／无效名称回到GPUI .SystemUIFont，非Windows保留Inter。Win32字段语义核对 [NONCLIENTMETRICSW 文档](https://learn.microsoft.com/en-us/windows/win32/api/winuser/ns-winuser-nonclientmetricsw)。只读 GET，不修改系统字体。`src/ui.rs` 和 `src/containing_branches.rs` 的绘制与TextRun宽度测量改用同一入口，`src/main.rs` 注册模块；Cargo.toml 仅补已有 windows-sys 的UI／GDI功能，不新增字体包或常驻服务。

保留差异：本轮字体族跟随系统，字号仍保留现有13逻辑像素，未翻译JBUIScale整个缩放系统或提供自定义UI字体设置；系统字体在运行中改变需重启应用。Inter仍内嵌用于非Windows，代码／Diff正文仍用JetBrains Mono。DirectWrite与Java字体fallback／字重／抗锯齿及其他DPI未宣称逐像素一致。

### 最近项目路径身份回归（2026-10-18）

字体欢迎页原生验证发现同一项目两行：artifacts/system-font/welcome/window.png。专用profile先保存普通Windows路径，打开仓库后 canonicalize 返回 verbatim 路径；最近模块原来的 raw PathBuf 比较未合并二者。工具栏的独立UI去重曾掩盖此问题，欢迎页直接展示持久列表因而复现。

本次读取同一4aec快照的 `platform/platform-impl/src/com/intellij/ide/RecentProjectsManagerBase.kt::loadState`，确认上游载入时将recentPaths及additionalInfo键标准化为system-independent路径。Rust实现仍保留原始可打开路径，但 `src/recent_projects.rs::identity` 统一Windows普通／verbatim／UNC前缀、斜杠及大小写，在decode、opened和remove共用；`src/ui.rs::project_identity`委托该入口，欢迎页、菜单及图标配色身份不再各自不同。加载时保留最先出现的表示与最近顺序，不为识别去访问离线仓库；列表20项、文件64KiB预算保持。UTF-8／相对路径／截断／超限校验保留，不把key用于Git命令。

真实Windows回归 `windows_recent_file_coalesces_normal_and_verbatim_paths`：同一批记录修复前读出5项而非2项，修复后2项；native红／绿输出 artifacts/system-font/recent-{red,green}.txt。Linux原有读写／Unicode／截断测试继续覆盖非Windows行为。保留差异：不实现整个IDE项目管理器的迁移、分组及项目服务；Windows按默认不区分大小写身份，不对路径符号链接或大小写敏感目录做启动时I/O探测。


### 窄窗分栏、筛选溢出与子弹层（2026-10-18）

参考仍为4aec35256d765c217aa0d662b82ac4e860c396fd。本轮已读 MainFrame.java（platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/）的工具栏高度引用及Log／详情分割；platform/util/ui/src/com/intellij/openapi/ui/Splitter.java 的doLayout／computeFirstComponentSize：最小宽度之和超出可用宽度时按最小宽度比例分配，组件留在可用区域内。读取platform/editor-ui-api/src/com/intellij/openapi/actionSystem/toolbarLayout/AutoLayoutStrategy.kt 的溢出按钮空间预留、隐藏不合适控件，以及CompressingLayoutStrategy.kt的首段压缩规则，未声称移植全部压缩算法。完整读取platform/platform-impl/src/com/intellij/openapi/actionSystem/impl/ActionToolbarImpl.kt 的calculateAutoPopupRect／showAutoPopup：弹出PopupToolbar，采用NOWRAP；允许作为子弹层父级，action popup仍在栈中时不关闭工具栏弹层。

本地src/ui.rs的observe_review_width用on_prepaint读取实际分配宽度，弱引用+defer在宽度变化时更新。中央分栏最小值不足时同比收缩，避免右侧详情超出窗口；历史搜索可缩，过滤项按宽度进入filter_overflow，复用原筛选实体与清除事件。UserMenu／PathsMenu记录自身及补全弹层bounds；vendor/gpui-component/src/menu/popup_menu.rs添加可选child_popup_hit_test，点击子编辑器不关闭父菜单，外部点击仍关闭，未配置此钩子的菜单行为不变。没有新增后台轮询、Git查询类型或正文缓存。

保留差异：本地650／360逻辑px阈值和分组是轻量适配；溢出中列出被隐藏的筛选项，采用竖向列表，上游水平工具栏可弹出完整水平动作组。尚未翻译完整toolbar布局系统、所有键盘导航和分栏跨重启状态；本次原生验证覆盖现有筛选事件与嵌套弹层，并非全部DPI／像素一致验收。

### 项目字母图标的独立字体与留白（2026-10-18）

同一参考commit，实读platform/util/ui/src/com/intellij/util/ui/AvatarUtils.kt 的generateColoredAvatar／getFont／getNewUiFont：New UI使用JetBrains Mono、WEIGHT_DEMIBOLD，字号为整数13*size/20，白字与渐变底色；不是系统UI字体。进一步读取同目录UIUtil.java::drawCenteredString：按文字宽度与ascent计算居中基线，并限制绘制在图标矩形内。安装版依据沿用前述用户截图／原生工具栏截图，未把源码快照等同安装版本。

src/ui.rs::project_avatar此前继承系统字体和行高，造成双字母贴边；现在显式Mono SEMIBOLD、整数比例字号、80%高度文字行框、四周10%内边距及禁止换行／越界。20px工具栏／菜单与24px欢迎页复用同一入口。src/main.rs注册从上述快照platform/jewel/int-ui/int-ui-standalone/src/main/resources/fonts/jetbrains-mono/JetBrainsMono-SemiBold.ttf取出的原版字体，不依赖用户安装或系统合成粗体；来源、SHA256和OFL见assets/fonts/SOURCE.txt及JetBrainsMono-OFL.txt。只增加一个字体资源，没有图标位图缓存。

保留差异：GPUI行框居中与AWT的ascent基线算法不同，10%留白为本地显式约束，上游采用等宽字体的自然留白及绘制裁剪；特殊Unicode缩写的字体fallback和其他DPI仍待核对。渐变仍采用既有路径哈希9色规则，没有实现项目自定义图标／颜色服务。不宣称像素级还原全部完成。


### 256轨道压力及原生Rebased反证（2026-10-18）

参考commit仍为4aec35256d765c217aa0d662b82ac4e860c396fd，安装版仍按前述1.1.20记录，本轮没有更新安装版本。已读取PrintElementGeneratorImpl.kt的getPrintElements／getSortedVisibleElementsInRow及cache常量：按行生成UP／DOWN、最后绘节点，缓存SLRU为100／200；SimpleGraphCellPainter.kt的paintEdge按端点绘制。上游不是本地几何共享算法：src/graph.rs::GeometryPool在单次窗口构建内对完全相同的Segment切片做hash及完整相等复核，Row改成Arc切片；重复行共享存储，唯一几何总数仍限500000，离开函数后只剩行的拥有引用，关闭窗口释放，不跨仓缓存。过滤窗口也复用同池，UI绘制规则此阶段未改。

真实10000提交／256并行分支、256父merge、975000字节Diff在旧581228构建出现几何预算错误；graph回归红／绿及原生同仓库截图见artifacts/wide-256。本地共享修复使全部10000行能够装入500000存储段预算，随机DAG逐行几何与不共享结果完全对比；小预算拒绝、同切片共享及最后窗口释放有回归。不提升轨道512上限或150／250MiB门槛。

用户指出原生宽图显示不合适，本轮实际用其Rebased进程1168打开相同Windows临时仓库，收起Project后截图artifacts/wide-256/rebased-default.png；仅打开／切换只读视图，未触发Git写操作或关闭用户进程。Rebased显示顶部线性提交文字、作者／日期列，而本地全局256lane宽度使所有文字挪到屏外。此为明确视觉反证，不将内存及完整加载成功宣称为宽图UI验收通过。

进一步完整读取platform/vcs-log/impl/src/com/intellij/vcs/log/ui/render/GraphCommitCellUtil.kt::getGraphWidth：取当前行printElements实际位置以及边端点中点，再与min(6,recommendedWidth)比较；MAX_GRAPH_WIDTH=6只限制推荐留白下限，不能误当所有图形的硬上限。GraphCommitCellRenderer.kt的customize／paintComponent：每行graphWidth作为提交文字padding，refs在Commit列内，作者／日期为独立列，绘制受表格单元格边界限制。PaintParameters.java确认元素宽16、行高22、文本gap2随rowHeight缩放。

实读PrintElementGeneratorImpl.kt::getArrowType／isEdgeVisibleInRow及常量：默认LONG_EDGE_SIZE=30、LONG_EDGE_PART_SIZE=1，长边中间不绘制，以端点相邻行箭头表示连接；Show Long Edges使用1000／250，并可在相邻行加箭头。它改变可见print elements及位置排序，不能仅隐藏本地固定lane的线条来仿制，亦不能删掉底层父关系。

待实现的结构修复：按当前行图形计算文本留白、Commit单元格裁剪与独立作者／日期；长边显示投影及箭头、显示长边开关；底层拓扑／分页／过滤保持真实完整。当前src/ui.rs仍取整个window的最大Row.width并设整行最小宽，因此此差异尚未解决。本轮未重新打包未完成的宽图版本。


### 按行Commit单元格与长边显示投影（2026-10-18）

参考commit4aec35256d765c217aa0d662b82ac4e860c396fd；仍与安装Rebased版本分开记录。沿用前节实际同仓库截图，进一步实读GraphLayoutBuilder.kt（platform/vcs-log/graph/src/com/intellij/vcs/log/graph/impl/permanent/）：有序heads、迭代first-parent DFS分配layoutIndex。完整读取impl/print/GraphElementComparatorByLayoutIndex.java：按layoutIndex及端点位置比较node／edge，特殊边用已知端点排序；PrintElementGeneratorImpl.kt的相邻行position映射、getPrintElements、getArrowType、isEdgeVisibleInRow和SLRU行缓存规则。SimpleGraphCellPainter.kt的斜线跨相邻行中心后在单元格裁剪，箭头长度=.3*rowHeight、cos²=.7。

src/graph.rs::text_offset_columns翻译GraphCommitCellUtil的当前行node／edge中点宽度规则，min(6,recommendedWidth)只用于推荐留白，未硬裁成6轨道。src/ui.rs::history将graph＋说明／refs放在有裁剪的Commit单元格内；作者／日期继续独立固定宽列，整张表的最小宽度不再随全窗最大轨道数增长。graph_cell在自身行内裁剪相邻中心连线，按方向绘箭头。当前推荐留白用本地窗口最大显示宽度再min6，未翻译上游全永久图的采样加权宽度；节点密度、字号和16逻辑px步距保留本地值，不称像素完全一致。

src/graph_print.rs为后台显示投影：从已有完整lane几何反向恢复实际显示端点，因此过滤虚线关系保留，未把未修改的Commit.parents误当过滤后的父边。默认30／1，保留短边和长边端点箭头，隐藏中间print元素后重排位置；内部也实现1000／250＋30的Show Long Edges策略，尚未增加可见切换入口。原始Layout start／end继续分页，原始Git父关系、Diff基准、OID选择与导航不修改。构建只保留相邻三行的元素／position映射，事件、邻接及几何池都随本次任务释放；每行检查取消，500000边／存储段、512位置与10000历史上限保留。输出沿用不可变共享几何，不为每行缓存一份永久图。

边界适配差异：有单个已知端点的窗口外连接用箭头；两端都在窗口外的贯穿frontier没有足够信息判断全局边长，保留并排在已加载拓扑后，不猜测或静默丢弃。布局从当前有界窗口heads重建，未引入IDE全永久图或其branch-head优先级服务；跨窗的具体左右位置可能不同。箭头目前为连接提示，点击跳转与Show Long Edges可见入口仍待接入；文件历史自身graph暂保留原始布局，主Log默认显示投影已接入，两者不混称全部完成。

本次已读取上游PrintElementGeneratorTest.kt及原版manyNodes／longEdges输入输出，原样放tests/fixtures并留REBASED-SOURCE.txt。Rust回归采用该测试的7／2／10策略，逐节点位置、逐边端点、虚线和箭头全量一致，忽略上游测试presentation-manager专用颜色及输出排列顺序。其他用例覆盖默认／展开策略、短边、窗口外提示、取消、随机DAG端点恢复，以及39组图的分页／过滤位置；256轨道完整10000行保留，端点箭头行仍可很宽，不能用全局硬压宽度替代源行为。


### Long Edges可见开关与重绘模型（2026-10-18）

参考仍为4aec35256d765c217aa0d662b82ac4e860c396fd。本轮实读platform/vcs-log/impl/src/com/intellij/vcs/log/ui/actions/{ShowLongEdgesAction,BooleanPropertyToggleAction,VcsLogToolbarPopupActionGroup}.java，impl/VcsLogUiPropertiesImpl.kt及ui/CommonVcsLogUiImpl.java的onShowLongEdgesChanged，以及resources/intellij.platform.vcs.log.impl.xml的Vcs.Log.PresentationSettings：属性默认false，空数据禁用，显示选项中为Long Edges；属性改变只设置可见图控制器并repaint，不查询Git。原生操作安装Rebased同一256分支fixture，确认切换勾选后弹层保持打开，恢复默认false后保留用户窗口。原图artifacts/rebased-live-toolbar/long-edges-real-menu.png、long-edges-reference-expanded.png及long-edges-final-restored.png。最初误点Search的long-edges-view-menu.png不是菜单依据，已Esc取消；Popup HWND的鼠标保护拒绝后改用该进程已聚焦菜单的键盘，未绕过点位保护。

本地src/ui.rs新增Log眼睛显示选项按钮，只列已实现的Long Edges／长连线，不放上游其余无实现项。PopupMenu.close_on_confirm(false)及观察者set_item_state刷新勾选／禁用态，鼠标及Enter切换后保持菜单，Esc关闭。独立后台任务只使用graph_print::Model，完成并保存偏好后替换rows；不触发refresh、load_log_selection或Git子进程，选择、Diff和历史窗口保持。窗口刷新／切仓取消任务，generation＋Model Arc身份防止迟到结果跨窗；空结果／在途任务禁用该项，保存失败保留原显示与值并使用已有错误入口。

src/graph_print.rs提取Model，持有当前有界窗口的端点、颜色、layoutIndex及邻接；不持有原始行矩阵，渲染仍只保留相邻三行位置映射。新增回归证明原raw geometry在Model创建后可释放、重复切换恢复完全相同几何。src/layout_preferences.rs按规范化项目身份保存独立布尔文件，最多64KiB读预算及原身份复核，默认false；不写.idea。路径身份委托recent_projects，普通／verbatim Windows路径共用键。布局偏好回归覆盖读写、默认／损坏／超限和其他项目键拒绝，不把低概率哈希碰撞静默应用到别的项目。

展开压力曾出现D1候选无Diff采样141.62–152.69MiB，不记为150MiB预算通过。graph_cell现在用Window当前content mask剔除整段在屏外的路径，保留穿越裁剪区的线及9px箭头余量；这是GPUI内部优化，上游Java Graphics单元格裁剪的可见规则不变，不称上游也有同一AABB代码。最终B312候选展开图139.29MiB（10次），另有展开＋完整Diff原生预算记录。此为有限宿主场景，不外推所有轨道／窗口／长会话。

保留差异：只有一个主Log，按项目单布尔值保存，没有IDE多Log ID服务；后台重绘短暂禁用，保存是同步小文件写入。菜单目前仅含一项，无上游完整Show／Highlight分组；保留入口、选中及关闭行为，精确几何待细化。箭头点击导航与文件历史显示投影仍未接入。


补充生命周期复核：GPUI Context::observe 的弱订阅在 detach 后可能等 observed owner 下次 notify 才移除回调。vendor/gpui-component 的 PopupMenu::with_subscription 改由菜单自己的 Subscription 集合拥有，菜单销毁立即取消观察；src/ui.rs::log_view_options 不再 detach。测试 tests/menu_subscription_release.rs 保持 owner 存活且不 notify，验证关闭菜单后回调捕获资源释放。此为本地 GPUI 生命周期适配，不声称来自 Swing 的同名实现。

同仓库截图再次核对：artifacts/rebased-live-toolbar/long-edges-final-restored.png 中默认模式的分支头较快向左收拢并露出文字；本地 native-r3/collapsed.png 在可见下半区仍有更多贯穿线，分支头位置／排序及可读性不能视作完全等同。窗口、列宽及首屏行数不同，尚未做同尺寸逐行对照；线粗、配色、箭头密度及轨道位置差异继续保留为待视觉验收，不能用同 fixture 的单元测试或内存通过覆盖这一差异。


### 长边状态证据更正与紧凑几何（2026-10-18）

同一4aec352快照继续作为规则依据。本轮进一步实读 GraphLayoutBuilder.kt、LinearGraphUtils.java::getDownNodes、PermanentLinearGraphImpl.java::getAdjacentEdges、PermanentLinearGraphBuilder.java::fixUnderdoneEdge/doStep 及 GraphElementComparatorByLayoutIndex.java。确认构建器保留 Git parent 顺序，不能仅因首屏文字不同就改成按当前行排序。本轮没有修改 DFS／轨道排序或显示投影策略。

**更正前节截图判断**：native-r3/unchecked.png 的 Long Edges 实际仍有勾选，collapsed.png 仍处于展开模式，不能用于默认模式与 Rebased 的比较，也不能证明关闭切换成功。后来 raw Enter 再启用的旧记录同样不足以证明一次完整双向操作；保留原文件避免掩盖历史，状态结论以本条为准。state-audit/native-v2 使用无重新聚焦的原始截图、分离 Down/Enter、等待项目保存值，初始 false→true→false 均成功；取消勾选后的分支标题按行收拢，未复现之前声称的默认布局退化。新 verified.json 确认没有新增子进程。最初 HOME 无菜单绑定导致未选择条目，保存值断言拒绝该次尝试；v2使用真实支持的Down。精确间距、线宽、配色和全图布局仍不是完整视觉验收。

src/graph.rs::Segment 改为内部 u16 坐标、一个样式位字节及完整 usize color，32→16字节；所有调用改用返回原usize／bool的只读访问器。产品各入口仍先按512轨道预算拒绝超限，构造坐标另用 checked conversion、不截断；未将颜色缩成调色板编号，窗口间颜色身份和几何hash／完整相等判定保留。不声称上游Swing使用这种Rust存储格式；它只是内部存储优化，不改变上述上游规则。src/graph_print.rs／src/ui.rs 与 repository 测试使用访问器；上下半段、完整竖线、虚线、相邻端点和箭头五项标志均保留。

压力fixture测出展开模式276484条独立存储段，8847488→4423744字节（8.44→4.22MiB）；默认6166条，197312→98656字节。新增存储预算红／绿、32种样式与usize::MAX颜色身份回归；原版Rebased print fixtures、随机DAG、过滤／分页与真实Git聚合测试继续覆盖实际几何，不用单纯sizeof代替功能验证。完整核心386项通过，Windows release D72CB27E；内存／原生截图及包记录见PLAN最新条目。


### 主Log长边箭头点击（2026-10-18）

同一4aec35256d765c217aa0d662b82ac4e860c396fd。已实读 platform/vcs-log/graph/src/com/intellij/vcs/log/graph/impl/facade/VisibleGraphImpl.kt::performArrowAction：仅hasArrow边，DOWN→downNodeIndex、UP→upNodeIndex，NOT_LOAD_COMMIT保留targetId；MOUSE_OVER给手形／目标元数据而不跳，MOUSE_CLICK给doJump并清图形hover选择。已读 platform/vcs-log/impl/src/com/intellij/vcs/log/paint/SimpleGraphCellPainter.kt::getElementUnderCursor/isOverNode/isOverEdge：先节点圆命中，再按元素顺序用两端距离和椭圆判定边，不能以矩形覆盖斜线。GraphCommitCellController.java 的shouldSelectCell/performGraphAction/handleGraphAnswer/getArrowTooltipText：graph元素点击不先选所在行；只有目标在当前visible graph才jumpToGraphRow，图外目标显示tooltip。VcsLogGraphTable.java::jumpToGraphRow/scrollToRowIndex 选择单行并聚焦，默认只滚到可见而非强制居中。

本地 src/graph_print.rs 生成箭头时保留visible row目标，src/graph.rs::Row用稀疏(segmentIndex,targetRow)表单独拥有导航身份；不能把目标并入共享Segment的hash／Eq。最初将目标放进Segment空隙，256lane压力测试因共享失效超过500000段而失败，现已移除；16字节Segment和共享预算均保留。src/graph.rs::arrow_at翻译节点优先及椭圆边命中，Row::arrow_target只接受本窗口的索引，不用原始Commit.parents索引冒充过滤结果。src/ui.rs::interactive_graph_cell只装饰有箭头的当前可见行，hover显示目标摘要／作者／日期与手形；mousedown拦住源行选择，click以Arc窗口身份＋在途状态防旧回调，选目标／聚焦History／Nearest定位，复用select_commit后的文件及Diff按需加载，不清筛选，不改分支。普通行、节点及其他边沿用现有选择行为。

保留差异：本地有界窗口的单端／未知frontier没有永久图targetId，因此图外箭头仅提示“提交不在已加载的历史窗口中”，不假造目标、不自动扩大历史或清筛选；上游能显示hash／元数据。hover尚未翻译上游整条边的黑色轮廓高亮，只提供命中、光标和说明。文件历史仍用原始图，箭头交互仅主Log；精确DPI／像素细节不声称完成。导航稀疏索引使每行增加一个可空Arc描述符，不保存源文件正文或无界永久图。

本轮验证补充：GPUI测量canvas原先静态位置低一行，明确inset_0后native-v5往返的完整OID核对通过；中文boundary-native验证图外提示和不改变选择。最终2330E04B的核心389／Windows graph26及压力预算通过，详见PLAN。tooltip目前用完整subject而非上游50字符省略，排版及hover黑轮廓仍有保留差异；未另行操作用户Rebased的本轮箭头，行为来源为上述已读源码，原生截图是本地程序验证。

## 文件历史图与 Plico 品牌／日期／窗口（2026-10-18）

参考版本仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`；宿主机此前确认的安装版本为 Rebased 1.1.20 / 262.10968.SNAPSHOT，未更新或假定两者一致。

文件历史已读 `platform/vcs-log/impl/src/com/intellij/vcs/log/history/FileHistoryPanel.java`、`FileHistoryFilterer.kt`、`FileHistoryUiProperties.kt`、`FileHistoryUi.java` 和 graph `PermanentGraphImpl.kt`／`VisibleGraphImpl.kt`：文件历史复用 GraphTableModel／VcsLogGraphTable，默认隐藏长边；文件历史 properties 不提供 Main Log 的 SHOW_LONG_EDGES。Rust `file_history::load_scoped` 复用 `graph_print::project(false)`，先做有界几何共享，再产生打印图；`history_view.rs` 与 `ui.rs::graph_arrow_cell` 共享箭头命中、提示和跳转，保留独立窗口身份／任务取消。预览比较继续使用原始 parent links，不使用裁剪后的画线关系。候选历史最多 1000 条、512 lanes、500000 几何预算；候选范围没有包含的旧文件 revision 不会被虚构为可跳转目标。保留差异：Commit／Author／Date 的现有列顺序、跨候选范围的完整 permanent graph 和精确视觉验收尚未齐平。

日期已读 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/table/column/VcsLogDefaultColumn.kt::Date` 与 `platform/platform-api/src/com/intellij/util/text/DateFormatUtil.java::doFormatPretty`、`platform/util/resources/messages/UtilBundle.properties`：默认作者时间；最近 61 分钟采用四舍六入五成双的分钟数与 Moments／A minute／minutes／1 hour 消息分段，其余按本地日历判断今天／昨天，跨年也适用，较早日期显示日期和时间。Rust `git.rs` 的两条历史查询改用 iso-strict 作者日期，`commit_date.rs` 在展示层转换为本地时区，主 Log 与文件历史共用。较早日期采用固定 `yyyy/MM/dd HH:mm`，没有复刻任意 OS 日期格式／12 小时制偏好；目前没有作者／提交者日期偏好设置。

项目窗口已读 `platform/platform-impl/src/com/intellij/ide/impl/ProjectUtil.kt::confirmOpenOrAttachProject`：已有项目时询问 This Window／New Window／Cancel。Rust `ui.rs::choose_project_window` 用同一入口处理选择器和最近项目；欢迎页直接使用当前空窗口；`main.rs::open_project_window` 在同一 GPUI App 内创建独立 Review 和窗口，避免重启整个运行时。保留差异：不支持 Attach、多窗口焦点列表和 Don't ask again 偏好，未增加无行为控件。新窗口不会改动原窗口的仓库／Tab，内存预算按每个窗口分别计。

标题区域已读 `platform/platform-impl/src/com/intellij/openapi/wm/impl/customFrameDecorations/header/toolbar/ToolbarFrameHeader.kt::fillContent`／`productIcon.border` 与 `headertoolbar/MainToolbar.kt`：product icon 在 header content 左侧独立区域，工具栏组布局和产品 icon 分开。结合用户 `c9bbb011…png` 截图，Rust 顶栏采用固定 48 逻辑像素 logo 区，其中心与 `8 + 32/2` 的左侧栏按钮中心一致；logo 20 px，在 40 px 高标题栏居中。项目徽标继续独立使用 JetBrains Mono 字母。Plico artwork 来自用户文件，透明外角与裁切见 assets/branding/SOURCE.txt；Windows PE resource 1 为多尺寸 icon，与 GPUI Windows load_icon 的资源入口一致。界面、任务栏窗口标题、PE 产品元数据与便携包入口均使用 Plico；Cargo crate／构建目标和原 GitReview 偏好目录保留，以兼容原有设置与辅助进程约定。

按用户要求移除主 Log 常驻 Loaded commits／offset／End of history 文案。有更多历史或 shallow 边界时，左下状态栏显示“历史尚未完整加载”；加载更多／前后窗口等真实导航操作继续可用。未将内部偏移改成面向用户的信息。

本轮证据：Plico 双窗口／英文日期在 artifacts/plico-branding/native-verified；最终版中文欢迎页与日期在 native-zh-verified；解压包图标／元数据与部分历史状态栏在 packaged-native／package-check.json。文件历史表头、上下箭头真实预览内容与 Main Log 完整 OID 复验在 artifacts/file-history-graph-print/native-verified。最终 exe `a9d986e1…`，功能与构建通过，像素级验收仍按 PLAN 原门槛保留。

## Graph 箭头悬停轮廓（2026-10-18）

实读同一 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/vcs-log/impl/src/com/intellij/vcs/log/paint/SimpleGraphCellPainter.kt::paint`、`PaintParameters.java`、`ui/table/GraphCommitCellController.java::performMouseMove`，以及 `platform/vcs-log/graph/src/com/intellij/vcs/log/graph/impl/facade/VisibleGraphImpl.kt::performArrowAction`、`PrintElementPresentationManagerImpl.kt::Selection.FromPrintElement`。上游先绘制未选元素，再绘制所选元素的较粗 MARK_COLOR 轮廓，最后覆盖原有 branch color；选择的是该 row 的 print element，不是整条 Git 分支。移动到空白会取消选择；悬停与提交行选中态独立。

更正旧记录里的“黑轮廓”：`MARK_COLOR = JBColor.BLACK` 并不意味着深色主题使用黑色。实读 `platform/util/ui/src/com/intellij/ui/JBColor.java`：BLACK 的 dark variant 为 foreground()，后者来自 Label.foreground。本地 Islands 深色主题使用现有 FG，画线和箭头翼使用同一颜色／粗细规则。

Rust 映射：`ui.rs::graph_arrow_cell` 将已有的可空 hovered segment 交给共享 `graph_cell`；后者不分配新的选区表或几何缓存，迭代器按未选／轮廓／彩色三个顺序绘制。主 Log 和文件历史自然复用。保留差异：普通线的既有 1.5 逻辑 px 与 28px 行几何不在本轮整体改动；轮廓取 max(2.5, 1.5 + 2 / scale) 以保证每侧至少一个设备像素，未完整照搬 Java2D 奇数像素对齐／round-cap。无已加载端点的 frontier 沿用既有通用说明，尚不能恢复 permanent graph 元数据。普通非箭头图元素的其他控制器悬停语义未增加。

Windows `artifacts/graph-hover-outline/native` 确认主 Log／文件历史 hover→leave；限定箭头区域的亮色像素计数分别为 40→0、38→0，不将区域计数冒充全屏像素验收。文件历史 hover-only 两阶段检查无新进程；双向导航的实际预览文本与 Main Log 完整 OID 均通过，readonly.json 确认仓库不变与自有 Job 清理。源码构建 D132EF… 已通过 Windows release 编译。本轮没有改变历史数据或缓存，不重复已通过的完整核心／大 Diff 内存压力；既有 393 项核心证据仍绑定上一构建，不冒充本轮重跑。

## 真正 HEAD 的 graph 节点（2026-10-18）

实读同一 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/table/GraphTableModel.kt::withPossibleHeadNode`、`HeadPrintElements.kt`、`paint/HeadNodePainter.kt`，以及 `platform/vcs-log/graph-api/src/com/intellij/vcs/log/graph/NodePrintElement.java`。规则：只有 getRefsAtRow 中带 HEAD 引用的提交才包为 OUTLINE_AND_FILL；不是当前过滤结果第一条，也不是所有 branch tip。HeadNodePainter 画彩色外圈→commitStyle.background 中圈→彩色中心，RADIUS_DELTA 为2；普通点继续实心。

Rust `ui.rs::GraphNodeStyle` 是绘制参数，既不更改 Row，也不污染共享 Segment 几何。Main Log 用 snapshot.head_id 与完整 OID 比较，中圈使用该行实际失焦／聚焦 selection_color、included 背景或 BG。`file_history::ScopedHistory.head_id` 复用查询开始时原本已解析的 HEAD；`history_view.rs` 随有界查询结果更新，完整 OID 匹配可见 revision，再传给同一个 painter。文件最后一次修改早于 HEAD 时不擅自标记首条；切到游离 HEAD 后可以标记历史中的较早行。选区历史不显示 graph，不添加无意义标记或额外查询。

保留差异：普通点既有4px半径不在本轮整体缩放，HEAD用6/4/2逻辑半径，未照搬 Java2D 奇数设备像素对齐。文件历史已有选中背景组织继续保留；本轮只保证中圈正确跟随实际行背景，不宣称所有表格焦点样式齐平。无新增图缓存或每行持久字段；每个文件历史结果多存一份HEAD OID，绘制时的临时标记按最多1000行有界。

原生参考 `artifacts/graph-head-marker/rebased-reference.png` 来自仍运行的用户 Rebased wide-256-fixture，PE ProductVersion 为262.10968.0.0-IC，之前UI记录1.1.20仍是另一种版本标识，未更新安装。截图仅确认HEAD圆环行为，不冒充与100提交Plico fixture同条件逐像素对比；原窗口保留，临时置顶状态已恢复。

Linux／Windows各14项真实Git文件历史测试通过，新增用例覆盖未修改文件的后续HEAD、游离HEAD位于其他branch历史较早行、HEAD过滤。Windows `native` 的marker-pixels.json限定区域检查中心／留白／外圈：Main失焦33353B、聚焦2A4371、included未选1D2336、文件历史选中2E4676均与各自行背景一致。双向预览文本／Main完整OID验证和无悬停Git查询仍通过；readonly.json确认HEAD/status不变与自有Job清理。本轮DPI精度及完整发布门槛仍未完成。

## 多窗口即时语言与退出验收（2026-10-18）

实读同一 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/platform-impl/src/com/intellij/ide/ui/LanguageAndRegionConfigurable.kt::LanguageAndRegionUi.createContent`／`apply`：选择语言持久化后提示重启。Plico按已明确的F1要求保留即时切换，不把上游重启要求搬进产品。设置入口／侧栏／按钮结构不变。

实测修复前BDBF8D候选：两个窗口中一个改中文，另一个Log/Branch/Date变中文而搜索框仍显示Text or hash，见 artifacts/multiwindow-review/baseline/window-9-*。原因是窗口保留的InputState提示仅在发起set_language的Review中更新。现 `ui.rs::sync_language` 在每个Review重绘时仅对实际语言变化同步三处保留输入框的提示；`local_view.rs` 同步提交说明Textarea。复用已有cx.refresh_windows，没有重建输入框，没有修改value/selection/focus，没有新增全局订阅、后台任务或Git请求。每个视图只增加一个语言标记bool；不改变历史与Diff缓存。

InputState::set_placeholder已实读 `vendor/gpui-base/src/input/base/state.rs`，只赋placeholder并notify，不发Change事件。Windows049297…候选的 verified 流程覆盖英→中→英、另一窗口已创建的Commit面板、切换前后完整复制提交草稿与筛选草稿、清空后检查两个方向placeholder；切换设置两次均断言无新增进程。仓库／提交／Tab没有重置。

验证边界：首次SendKeys输入Review draft后复制得到Reviewdraft，该输入路径未作为草稿保留通过证据；随后用目标窗口的WM_CHAR注入文本，并在切换前先复制确认包含空格，再在切换后逐字核验。未据此修改产品文本处理，也不把此用例当作物理键盘／IME完整验收。原始失败日志留native.log；有效记录为verified/actions.json及截图。仍未宣称每个保留错误状态／各类弹窗均完成中英文验收。

退出依据已读锁定GPUI-fast 0.1.8 `app.rs::QuitMode` 与窗口移除后的quit_on_empty分支：Windows默认LastWindowClosed，原产品无需增加强制退出逻辑。新增测试框架ExpectExit场景先等待自然退出、检查退出码和Job.ActiveProcesses=0，再Dispose Job，避免Job自动清理掩盖未退出进程。exit-verified验证2→1窗口仍可用，最后一个窗口WM_CLOSE后退出码0、所有Git子进程退出；退出句柄在关闭前取得，早期null ExitCode不冒充0。此为有界生命周期验收，未增加产品功能或重跑无关压力门槛。

## 首批历史可交互的验收依据（2026-10-18）

同一源码 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 已读取 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/table/GraphTableModel.kt::getRowCount`：数量来自 visiblePack.visibleGraph.visibleCommitCount，即已发布的可见图，不能从请求批大小推断已加载数。用户运行中的 Rebased PID1168／262.10968.0.0-IC 与源码快照不保证同版本，本轮未操作或关闭它。

本地 `src/startup_report.rs`／`main.rs`／`ui.rs::startup_frame` 提供显式 `--startup-report`：初始 snapshot 和 graph 发布且没有加载／错误状态后，在首次 prepaint 记录实际 commits、graph_rows、max_columns、offset、more 和 filtered；正常启动不启用，不新增 Git 查询或缓存，后续窗口不重复记录。文件 create_new，已存在时不覆盖。保留差异：这是独立性能诊断而非上游 UI；prepaint 不是 GPU 时间，不将其当作可交互证据。

`scripts/windows-startup-data.ps1` 另外观察实际屏幕中首个 graph 节点，限定自有 HWND 及前台，点击提交并复制完整 OID 与 Git HEAD 比较；10ms 轮询、输入与诊断开销计入从 Start-Process 前起算的保守上界。只测固定150%布局，无滚动。原生结果／图片和单位测试见 artifacts/startup-data；初次前台保护拒绝与后续报告汇总类型错误保留其目录，不算通过，runs-recorded 才是完整5次证据。

## 设置 Apply／Cancel／键盘路径集中核对（2026-10-18）

实读同一参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/platform-impl/src/com/intellij/openapi/options/newEditor/SettingsDialog.java::createActions/doOKAction/applyAndClose/doCancelAction` 与 `SettingsEditor.java::cancel/getPreferredFocusedComponent`：按钮顺序OK、Cancel、Apply，OK先apply成功再关闭；Cancel委托editor.cancel，未应用的configurable取消并重置；有搜索过滤时Esc先清搜索。本地 `ui.rs::open_settings/settings_panel/apply_settings/close_settings` 沿用这些保存／取消边界，用户要求不实现设置搜索，所以没有搜索清除层；语言按既有F1即时应用，不要求上游式重启。本轮没有产品代码变更。

C2CE1D…现有Windows程序，artifacts/settings-workflow/native-r3 完成29步交互与8次内联偏好文件断言，截图已查看：Tab到中文并Space选择，Esc丢弃；中文Apply保持弹窗，再选English后Esc保留已应用中文；Diff底部模式Apply，再选Tab后Cancel保留底部；重新打开选Tab并Enter确认关闭；最后English Enter确认。自然退出exit0和Job内0进程在Dispose前验证，HEAD/status不变。前两轮因前台保护与命令等待超时而中止，仅保留诊断，不算完整验收。r3预置整套命令不依赖人工接续。

Ctrl+Alt+S在SendKeys与限定自有窗口的原生虚拟键发送下均未打开；重新聚焦Log仍相同。独立RegisterHotKey短探测失败，GetLastError=1409（ERROR_HOTKEY_ALREADY_REGISTERED），证实宿主已有全局占用；没有注销或关闭占用者，未确定其身份。微软依据：[RegisterHotKey](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)、[错误代码1409](https://learn.microsoft.com/en-us/windows/win32/debug/system-error-codes--1300-1699-)。据此不误改GPUI键盘路径；无占用环境下此快捷键仍未验证，不能将鼠标入口和弹窗内键盘通过等同完整快捷键验收。成功探测本应立即Unregister，本次未注册成功。hotkey-check.json是首次脚本引号错误产生的无效结果；只有hotkey-check-verified.json可用。

## Push 准备失败保留诊断（2026-10-18）

实读同一 `4aec35256d765c217aa0d662b82ac4e860c396fd`：`platform/dvcs-impl/src/com/intellij/dvcs/push/VcsPushAction.java::actionPerformed/update` 打开项目范围Push对话框；`plugins/git4idea/backend/src/push/GitPushSupport.java::getSource/getDefaultTarget` 区分分支／detached revision／fresh状态；`platform/dvcs-impl/src/com/intellij/dvcs/push/PushController.java::loadCommits` 在OutgoingResult有errors时清空commits，将错误文本及处理链接放入PushLog，不把读取失败描述为无提交或无分支，并保留重载路径。

本地 `ui.rs::open_push` 原先将 `Ok(None)` 与 `Err` 都输出Could not read push source，中文甚至将Git配置损坏解释为需要本地分支，丢弃详情。现在 `Err` 保留原返回诊断到既有全局错误状态，由右下短摘要、Details／Dismiss与共享详情弹窗／复制显示；`Ok(None)` 单独提示先切换至已有提交的本地分支。`i18n.rs` 中英文统一原因，显式再次点击原Push入口可重试。没有改变Git命令、认证、默认目标或推送执行，也没有新增常驻面板／缓存／Git读取。

保留差异：准备阶段失败发生在本地Push窗口构造前，所以复用主窗口错误Details而不是上游PushLog错误节点；现有本地Push仍只支持attached branch，上游getSource支持detached revision，这一已有范围差异未扩大。安装版262.10968.0.0-IC与源码快照不保证一致，本轮未操作用户Rebased／IDEA。原生错误、详情复制、修复配置后重新打开Push，以及detached的独立说明验证见PLAN本轮记录。

## Push 面板与强制推送（2026-10-18）

继续对照快照 `4aec35256d765c217aa0d662b82ac4e860c396fd`，宿主安装版为 Rebased 1.1.20，两者不能视为同版。用户提供的原生 Push／Force Push 截图已保存到 artifacts/rebased-user-reference/push-force-{menu,confirmation}.png；不操作用户正在使用的 Rebased 确认框。

已读源码与提取规则：

- `platform/dvcs-impl/src/com/intellij/dvcs/push/ui/VcsPushDialog.java`：ComplexPushAction 的普通 Push 为默认动作，Force Push 位于 OptionAction 下拉；中心首选800×450。southPanel 为8／12逻辑像素边距，附加选项与右侧按钮同一行。
- 同目录 `ForcePushAction.kt`：禁止受保护目标；执行前确认，明确目标分支；“Don't warn about this target”仅确认成功后记忆。`platform/dvcs-api/src/com/intellij/dvcs/push/PushSettings.java`按项目存储 remote／branch 对；详情默认显示。
- 同目录 `PushLog.java`／`SingleRepositoryNode.java`／`PushLogChangesBrowser.kt`：单仓库根行为 source→target，不额外重复仓库名；左右对半分区，右侧为按目录分组的变更文件树、工具栏和可选详情。
- `plugins/git4idea/backend/src/push/GitPushOptionsPanel.java`：标签复选框与 ComboBox 同行，取消勾选时禁用组合框；默认All，标签模式来自 `plugins/git4idea/shared/src/git4idea/push/GitPushTagMode.kt` 的 All=`--tags`、Current Branch=`--follow-tags`。
- `platform/platform-resources/src/themes/islands/ManyIslandsDark.theme.json`：Button最小72×28、ComboBox最小49×28、组合框padding为1,9,1,6；控件禁用背景为dialog-bg，禁用边框gray-40，禁用文本gray-60；菜单选择背景使用selection-bg-active（blue-50）。`DarculaComboBoxUI.java` 保留标签左侧padding与独立箭头区。`DarculaOptionButtonUI.kt` 使默认动作与箭头共用品牌背景，并在上下内缩位置绘制半透明分隔线；`BasicOptionButtonUI.kt` 维护独立菜单与选择生命周期。
- `plugins/git4idea/backend/src/push/GitPushOperation.java`／`commands/GitImpl.java`及backend XML高级设置：默认使用force-with-lease，参数包含已知remote-tracking目标OID，没有已知引用时使用空期望值；force拒绝后不自动拉取／重试。`GitSharedSettings.java` 的默认受保护分支为main／master。
- `plugins/git4idea/tests/git4idea/push/GitPushOperationSingleRepoTest.kt`：已读force lease成功、stale拒绝、新目标成功、存在但未见目标拒绝等场景（实际checkout路径可用git ls-tree定位）。

本地映射：`src/push.rs` Spec携带显式force／tags，发送完整目的引用和显式lease；`push_flow.rs`沿用上游force不恢复重试规则。`src/ui.rs`构建split按钮、标签选项和嵌套确认；取消返回Push面板，确认才执行，目标记忆保存在`layout_preferences.rs`。`src/push_dialog.rs`单仓库根行、共享文件图标／目录树、展开折叠、Diff、只读源码跳转和详情。菜单初始选择入口位于vendor PopupMenu，split分隔线在DropdownButton；普通状态与展开／按下状态保留蓝色，修正此前继承到白色的未配置active背景。底栏各控件以28逻辑像素为公共高度，All固定132逻辑像素宽并显示箭头，选项和右侧按钮共享垂直中心。

有意保留差异：单仓库；仅main/master默认保护策略，尚无用户自定义正则或服务端保护策略配置；只支持默认安全lease，不提供plain force高级开关／旧Git降级；目标记忆按项目+remote+branch，无常驻缓存；标签选项暂不跨弹窗持久化，无skip hook选项。文件树工具栏只放已实现动作，未复制未实现的分组配置／眼睛子菜单。嵌套模态由同一GPUI窗口呈现，精确字体、按钮焦点描边和全部DPI还未像素级验收。

底栏对齐复查：Checkbox 组件内部为 items_start，给整项设置28px高度会令标签贴上沿；改为保持自然高度，由外部公共行居中，中文原生截图 footer-zh/push-open.png 已确认。另读 DarculaCheckBoxUI.java::drawCheckIcon：普通 focus 仅参与checkbox图标选择，不给整条标签画外框；本地仅在强推确认项关闭整项focus_ring，保留Tab／Space操作和勾选蓝色。当前未额外绘制上游图标级键盘焦点细节。

应用logo纵向对齐复查：已读同快照 MainToolbar.kt 的 HorizontalLayout 容器、MainMenuButton.kt 的图标按钮，以及 platform/platform-api/src/com/intellij/ui/components/panels/HorizontalLayout.kt 的FILL／CENTER布局规则。本地toolbar与logo容器共用h_full/items_center，无独立top偏移；plico.svg的tile viewBox为76,76,872,872，对称裁剪。Windows C3A5…构建 main-opened 原图不缩放提取像素，logo tile y14..43与菜单stroke y20..37均中心28.5；彩色symbol像素重心28.1，仅0.4物理像素视觉重心差，不将其报告为布局bug。结果 artifacts/force-push/logo-alignment.json；保持共同几何中心，未擅自移动品牌图形。

用户随后明确要求调SVG视觉重心：仅对assets/branding/plico.svg的彩色symbol组加translate(0 22)，相当于20px标题栏图标向下0.50逻辑像素；外框、viewBox、标题栏容器未动，用户原始SVG保留。通过generate-app-icon.py同步生成各尺寸ICO和PNG，不以挪动UI容器补偿品牌图形。

## 默认 Log 的变更加载反馈（2026-10-18）

源码快照仍为4aec35256d765c217aa0d662b82ac4e860c396fd，宿主参考安装1.1.20；本轮没有向用户正在操作的Rebased发送输入。已读MainFrame.java::constructor：JBLoadingPanel包装changes browser、与详情分区绑定；VcsLogCommitSelectionListenerForDiff.kt::onSelection清空旧选择，onLoadingStarted／Stopped启动／停止变更区提示，错误交给selection error。已读platform/util/ui/src/com/intellij/ui/progress/ProgressUIUtil.kt：上游可见progress默认延迟300ms。

本地src/ui.rs::files在busy期间于右侧文件区显示现有本地化Reading Git data…，覆盖已清空的旧树；busy由已有详情／历史任务控制，成功、失败或新一轮选择仍走原generation与取消路径。此前该提示仅在底部Diff区域，默认独立Tab模式切换提交只见空白。现新增文件区提示，不启动额外Git、不增加常驻缓存／计时器／动画；按照本项目明确的100ms反馈目标立即显示轻量文字，这是与上游300ms延迟spinner的有意差异。精确spinner形态不在本轮声明中。


## 基础编辑／自动保存：文档状态（2026-10-18）

参考源码 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版仍为 Rebased 1.1.20；本轮未操作安装版，不把源码条件当作已确认安装默认值。

已读 `platform/configuration-store-impl/src/SaveAndSyncHandlerImpl.kt::listenIdleAndActivate/executeOnIdle`：依据设置在应用失活时保存全部文档，依据 idle events 和 inactiveTimeout 去抖保存；不是每次键入立即写盘。已读 `platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/FileDocumentManagerImpl.java::doSaveDocument/maySaveDocument/doSaveDocumentInWriteAction`：保存前刷新并允许冲突 resolver 否决，保存时转换到文档换行符，写成功后移除 unsaved 状态；关闭路径尝试保存未保存文档并报告失败。

本地 `src/file_document.rs` 将文档文本、磁盘字节基线和正在保存快照分离；单文档只允许一个在途保存，成功确认仅更新磁盘基线，不把期间新输入标成已保存。失败不丢草稿。原字节 Arc 共享给保存任务，不复制基线；dirty 比较逐字节规范化换行，不额外分配全文。5MiB沿用现有文件预算，序列化大小也校验。纯UTF-8／BOM沿用现有范围；换行选择第一处换行是本地暂定规则，尚未读取上游混合换行检测算法，无编辑不改原字节。本轮未实现文件IO或原子外部变更保护，Save.expected 仅提供后续执行器必须比较的基线，不声明该比较已经执行。

六项测试覆盖 BOM/CRLF/无末尾换行、混合换行撤销不重写、保存期间继续编辑、失败重试、异文档确认和无效／超限输入。Windows 界面尚未变更，无本轮可见功能或新便携包。


## 基础编辑：本地 Diff 文档与实际保存（2026-10-18）

同一参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`、安装参考版本1.1.20。已读 `platform/diff-impl/src/com/intellij/diff/DiffContentFactoryImpl.java::createContentFromFile/createDocumentFromBytes/createDocument`：本地文件经FileDocumentManager取得文档，字节版本经readOnlyDocumentContent构建；不是依据侧栏标题决定只读。已读 `platform/diff-impl/src/com/intellij/diff/tools/simple/SimpleDiffViewer.java::isEditable/isDiffForLocalChanges`：编辑能力由各侧编辑器状态决定，本地变更情形检查左只读／右可编辑；变更应用动作也检查目标侧isEditable。

本地规划：源码Tab与Commit Diff工作区侧共享文档，历史／index／stash／push内容保持只读。不能简单删除所有Diff的readonly。尚未接入可编辑Diff，不声称已经复刻上游交互。

实际文件执行器 `src/file_save.rs` 依据前述 FileDocumentManager 保存前复查／冲突否决／成功后清除未保存状态的规则，接收文档Save快照并保留工作区文件属性。采用同目录完整临时写入＋flush＋复查＋替换；Windows ReplaceFileW保留目标安全属性，失败不删除目标再重试。不同于完整VFS，这里没有常驻磁盘监听；复查和替换不是原子CAS，竞争边界明确保留。现只读Project的path检查复用，原字节读取从read抽为read_bytes，不新增无界正文缓存或Git调用。

Windows原生11项／Linux文件相关16项通过；无界面更改、无新便携包。后续必须让保存结果回到同一文档、在UI中处理冲突并保留未保存文本，才构成完整功能。

## Tab 栏刷新与静默保存（2026-10-18）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`（安装版 Rebased 1.1.20，二者不等同）。本轮实际读取 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/MainFrame.java` 与同目录 `VcsLogComponents.kt::createActionsToolbar`：Log 的查询/动作组合属于 Log 内容工具栏；这些文件不单独证明所有 editor Tab 装饰的默认配置，Tab 右侧无重复刷新也依据用户原生截图。读取 `platform/configuration-store-impl/src/SaveAndSyncHandlerImpl.kt::listenIdleAndActivate/executeOnIdle`：失活/空闲触发文档保存，所读调用不创建常驻成功提示。

本地 `src/ui.rs` 移除 editor Tab 公共栏右侧刷新按钮，保留 Ctrl+F5、各面板及错误恢复入口；原按钮调用 refresh_current，既不是源码文件 Reload，也没有独立能力。`src/project_view.rs` 移除保存中/未保存/自动保存的行内状态文案；自动保存、dirty Tab 标记、保存失败反馈仍保留。源码文件行末 Reload 保留，负责显式重读磁盘并对未保存草稿确认。本轮没有更改保存时机或将本地 2 秒防抖说成上游默认值。

Windows release 构建通过；artifacts/quiet-editor-ui/window.png、saved.png 原生核对 Tab 栏无刷新与文件栏无保存提示，assertions.json 核实修改实际自动落盘。候选程序 SHA256 `7244f2fb7013bf7e5c0b93eea585db8f51c3e1bf4c1b78a87dde2dabbc5713b3`；隔离测试窗口正常关闭，未操作用户 Rebased。本轮只验收这两处 UI 调整，不代表 Commit Diff 编辑已完成。

## 工作区 Diff 文档来源与复用准备（2026-10-18）

实际读取同一 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/DiffContentFactoryImpl.java::createContentFromFile`、`createDocument`：真实文件通过 FileDocumentManager 取得文档；构造文本快照时显式设置 readonly。`platform/diff-impl/src/com/intellij/diff/tools/simple/SimpleDiffViewer.java::isEditable/isDiffForLocalChanges` 依据编辑器文档判断两侧编辑权限，不能以标题判断。

本地 `src/diff_target.rs::working_tree_document` 仅为 WorkingTree/Unstaged/Untracked 非删除端点提供候选路径，重命名用新路径；Staged、Conflict、Commit、组合 revision 快照不给权限。实际磁盘安全/编码/体积/权限仍须文档加载器验证，该方法本身不代表文件可写。冲突内容暂走现有专用冲突查看流程，不把它混入普通工作区 Diff 编辑。新增端点矩阵测试通过。

`src/editing.rs::Documents::get_or_create` 在 UI 线程安装文档时复查注册表，已有文档优先于异步读取返回的磁盘快照；新文档遵守全局操作锁。`src/project_view.rs::attach_document` 统一新建和复用文档的观察与保存事件订阅，使用持有的 Subscription 在换文档时释放，避免复用后不更新脏标志或反复注册事件。当前仅接好共享入口；Diff 的自定义虚拟行渲染器仍需接入实际编辑、对齐及重新计算，不能把本次后端准备宣称为右侧可编辑。

## 编辑 Diff 的完整端点读取（2026-10-18）

继续依据 DiffContentFactoryImpl 的真实文档/只读快照区分，并实际读取同一 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/GitContentRevision.java::getContentAsBytes/loadContent`：通过 revision/path 加载原字节，符号引用不作为不可变快照缓存；不是从显示 patch 反向重建文件。

新增 `src/working_copy.rs`：为已判定的本地编辑端点读取完整工作区字节并创建保留 BOM/换行规则的 Document。Unstaged 左侧解析 stage-0 index 对象，WorkingTree 左侧解析 HEAD tree 对象，随后通过不可变 blob ID 读取；新文件左侧为空。路径作为 literal 参数，重命名使用原路径查询基线、新路径读取工作区；拒绝不支持的只读区域、非普通文件基线、二进制/超限文档、缺失文件及取消。该层只读，不写 index，也不从 Git 的换行规范化 patch 构造保存基线。与上游完整 VFS/charset 缓存不同，沿用本项目 UTF-8 和 5 MiB 限额，且没有常驻版本正文缓存。

`tests/working_copy.rs` 的四项集成测试覆盖 HEAD/index 区分、BOM/CRLF 序列化、重命名和含方括号的字面路径、无尾换行、空未跟踪文件、无差异文件，以及只读区域/二进制/缺失文件拒绝。该模块尚待 DiffTab 接线，不能视为编辑器已交付。

## Commit Diff 双栏编辑首次接线（2026-10-18）

沿用已实际读取的 `DiffContentFactoryImpl::createContentFromFile`、`SimpleDiffViewer::isEditable/isDiffForLocalChanges`（commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 1.1.20）：工作区侧引用同一文档，基线侧保持只读。`src/editable_diff.rs` 使用独立只读基线 Editor 与 Documents 中共享的工作区 Editor；`src/diff_tab.rs` 在可编辑本地端点异步读取完整源文件，双栏接入 EditableDiff，历史/stash 等仍走原只读 Diff。输入按 100ms 防抖后台重算，generation 门禁阻止旧结果回写；`DiffFile::changed_ranges` 从差异配对映射到 UTF-8 源字节区间，独立 decorations 显示颜色并在离开时清理，不修改正文。

`src/editing.rs` 对 pending 文档额外持有引用，避免切换预览导致在途保存/失败草稿被丢弃，保存成功后解除；注册表其它条目仍为 weak。`src/ui.rs` 为各 DiffTab 传入同一仓库 Documents，Tab 关闭按钮等待保存；文件链换目标先保存旧文档。当前只是接线阶段：未实现增删行空白对齐/同步滚动、单栏编辑、编辑区域的差异导航和位置回传、完整外观选项、Diff 独立 Ctrl+S、所有关闭路径及保存后的变更列表刷新，不能宣称等同上游。共享文档 Reload 在多个视图已持有旧实体时的语义也需收口；装饰目前为文本区间而非整行背景。

Windows release 通过，候选 SHA256 `b588b95bbe018ccb9bf60b81ea1668a1a564afa8fbdc820581fc95bd4544581a`。`artifacts/editable-diff-first` 的原生截图 split/saved 与 assertions.json：从 Commit 文件双击进入 Diff、切双栏、在工作区侧粘贴修改，文件自动落盘；左侧尝试粘贴仍复制出原 HEAD 内容；HEAD/index 不变；Jump to Source 后复制得到相同修改正文。测试不证明未保存状态下多个窗口同步；这里只验证同窗口来源复用路径。测试窗口正常关闭，无滚动自动化，未操作用户窗口。未更新便携包。

## 编辑 Diff 的保存、导航与 Jump 生命周期（2026-10-18）

本轮重新读取同一参考 commit 的 `SimpleDiffViewer.java::scrollToChange/doScrollToContext/getSelectedChange`：变化定位使用各侧变化起始行并移动两侧 caret，基于当前侧显示；没有把只读 Diff 的 selection 状态当作编辑器光标。本地 `DiffFile::change_starts` 输出连续变化组的双侧行锚点（含空侧），`EditableDiff::navigate` 在真实编辑器移动光标并恢复用户最后使用的侧。Focus 事件记录当前侧，Jump 读取实际 caret；工作区侧已有共享文档时直接打开 Source 到对应行，不先拿尚未保存的正文与旧磁盘做错误映射。原始侧/历史映射仍沿用原流程，尚未完成所有“历史侧到含未保存改动文档”的语义验收。

Ctrl+S 绑定到 Review 上下文并调用共享文档保存；Documents 的统一 Saved 回调刷新 LocalView，移除 Source 专属重复刷新。底部相同 target 的刷新复用 DiffTab/编辑文档，独立 Tab 离开时保留已编辑文档以保留 Undo。`src/push_dialog.rs` 只适配新增 Window 参数，Push 内容仍只读。

原生验证发现新快速 Jump 同步回调 owner，owner 在该 DiffTab 正更新时调用 deactivate，发生 entity re-entry panic。按 diagnosing-bugs 流程缩小到打开双栏后点击 Jump，不需编辑即可复现；`src/diff_tab.rs` 的 GPUI 回归测试调用真实 open_source→owner 退出链，在修复前同样失败 `cannot update ... DiffTab while it is already being updated`，修复后通过。现在 `open_source` 用 window.defer 在当前点击更新结束后调用 owner；这是 GPUI 生命周期适配，不声称上游有相同 defer 实现。初版测试渲染完整工具栏触发了既有 debug hover assertion，随后用空 Root 移除无关渲染，仅保留真实调用链得到目标失败。不以该无关失败作为回归证据。未添加临时 DEBUG 日志。

`tests/diff_source_position.rs` 新锚点测试通过：纯删除、替换、末尾新增、UTF-8 范围及无变化。Windows 修复版 release SHA256 `fdba8759a2f0e26e8ff1d241db6d57e44171039dd765cf3a7967d19290ebc2d1`，artifacts/editable-diff-navigation-fixed：两次 F7 和 Shift+F7 复制得到预期行，Ctrl+S 后 118ms 观察到磁盘修改（单次功能验证，不是性能承诺），Undo/Redo 保持，Jump 后选中内容确认对应行。恢复基线保存后 Commit 列表为空，撤销后文件重新出现；两张截图人工确认，HEAD/index 不变，自有窗口正常关闭。前一候选 A047… 的崩溃日志留在 editable-diff-navigation，仅作诊断，不能算通过。仍待行对齐/同步滚动、单栏编辑、完整外观、历史侧映射和多视图 Reload；便携包未更新。

## 编辑 Diff 行对齐：坐标基础（2026-10-18）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/tools/simple/AlignedDiffModel.kt`：`initInlays/createAlignInlay` 在变更前后放置 block inlay，EOF 使用下方插入；`updateInlayHeights/calcEffectiveVisualLinesHeight` 按折叠及软换行后的高度补齐短侧；`realignChanges` 保存两个编辑器视口，重建后在禁用同步滚动的区间恢复；`clear` 释放 inlay；软换行监听触发重新对齐。源码快照与已安装 1.1.20 的区别沿用文首说明。

本地 `DiffFile::change_line_ranges` 提供完整来源的两侧半开变更区间，原 `change_starts` 导航复用边界；`vendor/gpui-base/src/input/editor/display_map/row_gaps.rs` 提供稀疏空白行投影、双向映射和方向相关的空白命中处理，空间随空白块数增长，未修改文档内容。测试通过 64 组文本组合核对区间外文字相同，以及 600 组投影与逐行展开 oracle 对比。差异：当前模型使用整数显示行，尚未接实际软换行高度、渲染与鼠标命中；投影仅由 `tests/editor_row_gaps.rs` 编译测试，未在生产 UI 启用，避免部分接线造成光标位置错误。接入时还须处理共享 Source/Diff 文档的视图隔离。此阶段无新增可见行为，无 Windows 界面验收或便携包更新。

### 行对齐接线与原生验证

继续按上述 AlignedDiffModel 规则，在 `src/editable_diff.rs::realign` 按变更前缀／变更正文的实际视觉行数填充短侧；换行布局变化事件重新计算。`DisplayMap` 在 wrap/fold 后接 RowGaps，`LastLayout` 保存可见真实行的位置；正文、背景、行号、选区、折叠图标、光标、鼠标命中和输入法定位共用坐标。上下键按方向越过空白，不生成源码偏移；离开 EditableDiff 清除共享工作区文档的对齐空白。

Linux 7 项相关测试通过（模型3、Diff区间/定位3、真实 Editor 交互1）；后者验证点击、跨空白上下键、输入撤销、清除投影后的普通布局。尝试单独运行 gpui-base 库测试被 Cargo 拒绝（非 workspace 成员且需要 dev-dependencies），不记为通过。Windows release `8779D563FF445839206C0DA7E5550DD2A225CBD680CA81EECE16CFA343404C0E` 构建通过；`artifacts/editable-diff-alignment/split.png` 确认删除、非等长替换、末尾插入均保持共同代码行对齐。原生断言确认点击 common 行、上下键跨空白、保存字节不含补齐空白、Undo/Redo、Jump 行位置；source.png 确认返回源码没有残留空白。HEAD/index 未变，stderr 空，自有测试进程正常关闭。第一次验证因系统剪贴板占用在写入前失败，增加有限重试后完成；未触碰用户 Rebased。

保留差异：同步滚动、重算后的视口锚点保留、完整共享视图销毁路径、折叠/软换行变化的原生验收、单栏编辑尚待；背景仍是字形范围。此记录仅证明本次双栏场景，不宣称完整编辑功能或像素级完成，便携包未更新。

### 编辑 Diff 同步视口与释放（2026-10-18）

读取同一 `4aec35256d765c217aa0d662b82ac4e860c396fd` 下 `platform/diff-impl/src/com/intellij/diff/tools/util/SyncScrollSupport.java` 的 TwosideSyncScrollSupport/ScrollHelper：监听 visible area、只同步改变的轴、同步期间禁止回传，接收侧限制有效位置；`SimpleDiffViewer.java::MySyncScrollable` 在行对齐启用时强制同步垂直像素位置，非对齐模式才需要区间插值。本地 `InputEvent::ScrollChanged` 在布局后发布真实位置，`set_peer_scroll_offset` 标识一次接收侧应用，限制位置后也不回传；`EditableDiff` 同步变化的轴。没有添加常驻轮询或计时器。`clear_view` 同时用于 deactivate 和 entity release；active 标记阻止已离开视图继续重算或同步，移除共享编辑器的空白与装饰。

验证：Editor 集成测试2项通过（原对齐输入/撤销，加程序设置视口的通知、接收侧不回传、EOF限制后不回传、普通导航恢复通知）；实际 EditableDiff 释放后保留 Source 文档的点击/文本回归1项通过。没有模拟滚轮或拖动滚动条，滚动交互体验仍待用户手测。Windows release `B9CD7BFB2943C3E19A092C1E7220FF52D4888765DBD6D47F6D91788E4888BBBF` 通过，artifacts/editable-diff-sync 的原生截图与断言确认双栏布局、点击、方向键、保存、撤销重做与 Jump 未回归，HEAD/index不变、stderr为空、自有进程正常关闭。该短文件场景不算同步滚动手感验收。

差异仍保留：暂不包含重算时的源码锚点恢复、非对齐模式的比例映射、单栏编辑或完整共享 Reload。当前同步以已对齐视图为前提；上游源码与安装1.1.20的区别同前。便携包未更新。

### 共享文档原地 Reload（2026-10-18）

读取 `4aec35256d765c217aa0d662b82ac4e860c396fd:platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/FileDocumentManagerImpl.java::reloadFromDisk/setNewText`：可重载文本在同一 Document 上 replaceText，保留原可写状态，重置文件编码/BOM信息，广播 fileContentReloaded 并移除未保存记录；不以新 Document 替换已经绑定的编辑器。上游重载在 write action/带 UndoConfirmationPolicy 的 command 中执行。源码版本与宿主机 1.1.20 差异沿用前述说明。

本地删除 FileTab 的 documents.remove/重建路径；`Editing::reload/finish_reload` 在共享 EditorState 中更新文本与磁盘基线，保持选区的逻辑位置和视口请求，通知 EditableDiff 重算和 Documents 的变更列表刷新回调。读取期间 save/关闭前等待共用 busy 状态；读取失败或文本发生变化时保留草稿，拒绝迟到内容。保持原有 dirty reload 确认。保留差异：后台读取使用文本快照复验而非 IDE write action；显式丢弃重载仍清空旧撤销栈，不提供上游重载撤销确认语义；没有实现自动 VFS 重载。

GPUI 回归通过，检查共享实体身份、新 BOM/CRLF 保存基线、迟到结果与读取失败保稿。Windows release `091AD32239A4A83D5B63ED4E4070BAB7E76BA41523C67D7F5517D64238F7BE2C` 通过；`artifacts/shared-document-reload` 原生验证先在 Diff 编辑保存，再跳源码、外部写磁盘、点击 Reload、切回之前编辑过的 Diff，两个视图均为新文本，继续编辑能按新磁盘基线保存。截图核对更新后的 Diff，HEAD/index 未改、stderr 空，自有窗口正常关闭。未操作用户 Rebased，便携包未更新。

### 编辑 Diff 重算时保持阅读位置（2026-10-18）

对照参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/tools/simple/SimpleDiffViewer.java::runPreservingScrollingPosition` 和 `platform/platform-impl/src/com/intellij/openapi/editor/ex/util/EditorScrollingPositionKeeper.java`：对齐模式只恢复当前侧，另一侧通过同步跟随；光标在视野内保持其屏幕高度，否则保持顶部源码位置。安装版仍为1.1.20，不把源码快照当作安装版逐行实现。

本地 `InputBaseState<EditorMode>::set_block_gaps_preserving_position` 在替换显示空白前记录 caret 或顶部真实 wrap row，按新旧显示行差调整 y，保留 x；`EditableDiff::realign` 仅对当前侧计算恢复，另一侧静默跟随。首次比较直接设置空白，避免把开头删除行滚出屏幕；离开 Diff 清除空白时也保留源码阅读位置。差异：本地只在同步替换 padding 期间保存稳定 wrap row，不是上游可跨任意文档变化的 RangeMarker，不能据此宣称已实现所有编辑/折叠位置恢复。

`cargo test --features desktop-tests --test editor_alignment` 三项通过，包括可见光标与不可见光标两条恢复路径，以及接收侧不回传。Windows release SHA256 `8A706C48C6872FC2BD64DA28DEF169B32CB63048B2FF1045EAFFF104C8204DA3`，`artifacts/editable-diff-position/split.png` 已核对首次打开的两行开头删除、共同代码行和末尾插入位置；保存/撤销/HEAD及index不变断言通过，stderr为空，自有PID25132正常退出。最初输入验证受到中文输入法标点转换影响失败，改用固定文本粘贴后通过；早期重叠执行的打开脚本未进入Diff，不作验收证据，最终split为逐步等待完成后的截图。没有滚轮/滚动条自动化，真实长文件滚动体验仍待用户手测。便携包未更新。

### 单栏编辑的来源映射基础（2026-10-18）

继续读取 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/tools/fragmented/UnifiedDiffViewer.java::installTypingSupport/MyOnesideDocumentListener`、重建 guarded blocks 的逻辑，以及 `UnifiedFragmentBuilder.kt` 和 `tests/testSrc/com/intellij/diff/tools/fragmented/UnifiedFragmentBuilderTest.kt::testSimple`。已确认：展示文档按删除在前/新增在后组装；只允许 master 文档对应区编辑；guard 阻止修改另一侧；严格映射行列后写 master；ORIGINAL_DOCUMENT 关联 master 撤销；事件互斥门禁防递归，输入后更新映射并安排重算。`DiffUtil.getLinesRange` 默认不含最后换行，上游 guard 两侧 greedy；文件末尾另有 fake newline guard。安装版仍为1.1.20，未宣称与参考快照完全一致。

本地新增 `vendor/gpui-component/src/diff/unified_edit.rs::UnifiedEditProjection`，基于完整 DiffFile 来源构建展示文本和分段 UTF-8 字节映射。删除行/补齐分隔符无可写来源，跨删除区选区拒绝；工作区 EOF 可逆定位，空文档仍保留可写锚点。调用方只能把映射后的修改交给共享文档，不能保存展示正文。当前为不可变快照模型，尚未接入输入事件、共享 Undo、动态重算或界面；不能接受文档变化后继续使用旧映射。

差异：映射使用 UTF-8 字节和稀疏片段，不直接照搬 UTF-16 Document/RangeMarker；末尾不额外添加全局 fake newline，删除的无换行末行仅为分隔展示添加不可写换行。边界校验按可写片段实现，仍需结合真实输入事务补齐上游 greedy guard/换行删除的交互一致性。测试覆盖删除区、跨区、Unicode、空文件、无尾换行，以及64组来源组合中所有选区的精确字节映射。此阶段无新增可见行为，未做Windows界面验收，便携包不变。

### 单栏连续输入的即时映射更新（2026-10-18）

重新读取同一参考 commit 的 `UnifiedDiffViewer.java::MyOnesideDocumentListener.beforeDocumentChange`，并读取 `LineNumberConvertor.java::handleMasterChange/Corrector` 和 `LineNumberConvertorCorrectorTest.kt`：master 写入后立即校正两侧映射，再安排后台 rediff；不能在后续输入中沿用上一份来源坐标。上游测试覆盖多次增删和已修改区内继续修改，严格双向转换仍需成立。

本地 `UnifiedEditProjection::apply_edit` 在合法工作区修改被接受后同步替换展示区、调整当前/后续来源片段及删除区位置；返回修改前工作区字节范围。非法选区在任何状态修改前拒绝。与上游累积 CorrectedChange 链不同，本地原地调整稀疏片段，不为每个输入永久追加映射记录；仍需调用方将工作区写入和映射更新置于同一事务，并对外部文档变化使映射失效，不能把本方法视为已解决共享文档并发。

`cargo test --features desktop-tests --test unified_edit_projection` 六项通过。新增连续增删/多行Unicode粘贴、清空可写区后再次输入、64对来源各24步混合操作；每一步核对删除区原文、可写光标双向映射以及拒绝后状态不变。没有启用单栏UI或共享Undo，因此无新增可见功能验收；下步仍是输入事务转写与Undo/Redo路由，而不是交付独立的单栏文档或保存展示正文。便携包未更新。

### 单栏输入保护、精确修改通知与共享历史接口（2026-10-18）

再次核对参考4aec352的 UnifiedDiffViewer::installTypingSupport/guarded blocks：删除区在修改前受保护，展示编辑器的 Undo 关联 master 文档。新增 EditorState 的 set_edit_guard、set_edit_observer、set_history_delegate/step_history：普通/批量输入和 IME 预编辑在文本修改前校验；接受后同步报告精确的原始字节范围与替换内容，IME 单独标识；显式程序替换临时绕过这些钩子，防止重建展示回传为用户修改。批量校验先于任何正文修改；自动括号补全被拒绝后不再记录括号对或移动光标。未安装钩子的现有编辑器不分配通知正文副本。

本地 tests/editor_guard.rs 使用真实 Editor 实例，验证只读区键入/全选删除/IME 拒绝、工作区粘贴与撤销、IME 接受和提交的字节范围、重建不回传。第二项将 UnifiedEditProjection 与独立工作区 Editor 接起来，验证连续输入只修改工作区、Undo/Redo 调用工作区历史、延迟重建避免 entity 重入，重建后删除区仍受保护。初版测试将逐字多行输入误当成单次粘贴，随后改用实际Ctrl+V；重做快捷键改为当前绑定Ctrl+Y。最终结果以最新运行记录为准。

这些是编辑器接口和集成测试，尚未在 EditableDiff 的单栏入口启用；不能据此宣称完整单栏功能可用。生产接线还须处理 IME 事务转写/保存时机、外部修改使映射失效、选择与阅读位置恢复，以及共享 Undo 的输入合并策略。测试桥使用原子 replace，不冒充上游完整输入合并。无新增可见UI，未启动Windows验收窗口，便携包未更新。

本轮最终验证：`cargo test --features desktop-tests --test editor_guard --test editor_alignment --test unified_edit_projection` 共11项通过（2+3+6），包含修正快捷键后的共享文档Undo/Redo测试。

### 单栏 Diff 编辑接线与原生验证（2026-10-18）

依据参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/tools/fragmented/UnifiedDiffViewer.java::installTypingSupport/MyOnesideDocumentListener.beforeDocumentChange`：展示文档以master为编辑及Undo来源，删除区域受guard保护，精确转写后即时更新映射并安排rediff。沿用前述 UnifiedFragmentBuilder 和 LineNumberConvertor 规则。安装版Rebased为1.1.20，参考快照不等同于安装版。

本地新增 `src/unified_editing.rs`，由 `src/editable_diff.rs` 管理生命周期，`src/diff_tab.rs` 将单／双栏编辑、导航、Jump路由至同一视图。EditorState::apply_projected_edit 将精确修改交给共享工作区输入接口，避免二次自动括号插入，并保留typing／IME历史；无变化的caret不重复设置，以免打断连续输入合并Undo。guard同时复验实际来源与快照，Documents锁定时拒绝输入；异步rediff结果必须匹配当前工作文本，composition期间暂缓展示重建。UnifiedEditProjection保留删除区原始来源，用于Jump定位；新增display_ranges避免绿色背景跨越删除区。

保留差异：使用UTF-8分段投影，后台快照校验替代IDE文档锁／RangeMarker；当前单栏仅投影行号，差异背景为字形范围，重算保持原始视口偏移，删除区选区刷新回退到工作区光标；纯删除导航可能落在删除块之后。多光标转写尚未独立验证为同一Undo批次，完整关闭／IME生命周期、Windows原生IME和内存验收仍待。不得将本轮描述为完整像素还原。

验证：编辑器／对齐／投影11项及生产接线1项测试通过。生产测试验证真实连续输入共用一次Undo、Redo、外部文本变化保护，以及预编辑／提交共用IME事务。原生Windows构建 `79D53C24A2E5259804FF9D0B6B56A6763C7BB1DB98220D3582D615685F3D2A7A`，`artifacts/unified-editing` 保存截图、startup.json、assertions.json与验证脚本；删除区保护、保存、撤销重做、Source／split内容一致和HEAD/index不变均通过，测试窗口正常退出。便携包未更新。

### 编辑区与工具面板圆角补齐（2026-10-18）

用户指出文件浏览底部方角。对照 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/platform-impl/src/com/intellij/openapi/application/impl/islands/IslandsUICustomization.kt::paintIslandAreaRaw`：以主窗口背景绘制岛屿外缘及四角，圆角不只是子控件背景属性；安装版仍为1.1.20，不将源码快照视为安装版逐行实现。

本地 `src/ui.rs::Review::render` 为 FileTab 外层补齐与独立Diff一致的底部 island_corners；该共享入口同时覆盖文件历史Tab。检查发现底部本地变更预览和左侧工具面板也仅设置rounded/矩形overflow，补齐外层遮罩，覆盖项目、提交、暂存及比较面板。保留现有10px半径和SVG遮罩，不复刻上游渐变背景／DPI对齐算法；GPUI矩形裁剪下，用绘制顺序保证子内容不填满外角。

Windows release `D9A641D65FC76ED466309FCB5A987C7E85F25BED74C6B9BEDE629F23E8EE717D` 构建通过；`artifacts/editor-corners/source.png` 与 `project-source.png` 已查看，确认文件编辑底角、提交／项目面板及页面切换。测试窗口PID29900正常关闭，stderr为空。底部预览、暂存及历史页本轮仅源码共用路径检查，未逐页截图验收；没有滚动自动化。便携包未更新。

### 关闭前识别未提交输入法草稿（2026-10-18）

保稿审计发现：Editor的IME预编辑先改变正文，普通InputEvent::Change在提交时才发出；原Editing::pending仅检查Document快照，可能将预编辑草稿误判为干净，绕过窗口关闭／切仓保存。现在pending接收App并在既有脏／保存／错误状态之外比较实际Editor与Document文本；以字符迭代比较，不额外复制整份正文。FileTab关闭／保留、Documents注册与Review保存门禁统一调用此判断。明确保存仍沿用原有读取当前Editor文本的路径，本轮未改变IME提交／取消策略。

源码对照：Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd`，`platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/FileDocumentManagerImpl.java::markDocumentUnsaved/saveDocumentsOnEdt/doSave`，保存入口围绕共享Document未保存集合执行；本地Editor与Document分离，因此增加正文复核弥补事件时序差异。安装版仍为1.1.20，未声称上游存在相同漏洞或完全相同IME行为。

GPUI真实Editor回归2项通过：预编辑未触发普通Change时已被pending识别，保留调用后释放Tab侧持有仍能取回草稿；共享Reload既有测试通过。UnifiedEditing生产输入／IME／Undo测试1项也通过。此次是内部保稿判断，无可见布局变化，未重跑Windows原生IME；Windows输入法真实候选窗、确认／取消后关闭的交互仍待验收，不能把GPUI测试当作其替代。便携包不变。

### Windows关闭保存冲突与重试验收（2026-10-18）

继续基础编辑保稿验收，未新增功能范围。最终Windows release SHA256 `B71CF4E6EF2A63F89BC59EC070DD7C3DA7A95D13D63D2DDDEBB5AC224C431176`，在独立 `Plico-validation/edit-close` 仓库中通过Source编辑后由外部写入冲突内容，再关闭窗口：窗口保持打开，磁盘外部字节未被覆盖，编辑器完整草稿保留；恢复测试文件原基线后再次关闭，窗口退出且磁盘精确等于草稿。HEAD/index不变、stderr为空，自有进程正常结束，用户Rebased未操作。

证据 `artifacts/edit-close/startup.json`、`assertions.json`、`close-conflict.png`（已查看）及verify/resume脚本。第一轮复制校验因错误行出现后编辑区域下移、脚本仍点击旧坐标失败；没有将失败断言当作通过。检查截图确认草稿仍在，resume在同一存活窗口以新坐标全选复制验证完整正文，然后执行保存重试和退出断言。此轮覆盖普通输入保存冲突与恢复，不覆盖原生IME候选窗或所有关闭入口；旧便携包未更新。

## Preview 修复：Tab 缩进、F7 边界、标题栏点击与欢迎页定位（2026-10-19）

源码快照仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`；宿主机参考为用户安装的 Rebased 1.1.20，未假定其构建与此 commit 完全一致。本轮实际读取：

- `platform/platform-impl/src/com/intellij/openapi/editor/impl/view/TabFragment.java`：Tab 占一个源字符，绘制为空、宽度到下一个 tab stop；点击映射到原始字符边界。对应 `vendor/gpui-base/src/input/editor/display_map/inline_line.rs`，以及 editor 绘制／换行测量。保留原文和 UTF-8 索引，不把文档中的 Tab 替换为空格；当前源码编辑器默认四列，尚不解析项目 `.editorconfig`。
- `platform/diff-impl/src/com/intellij/diff/actions/impl/DiffDifferenceNavigationAction.kt`、`DiffNextDifferenceAction.kt`、`tools/util/CrossFilePrevNextDifferenceIterableSupport.kt`、`PrevNextDifferenceIterableBase.java`、`impl/DiffRequestProcessor.java` 的 `DiffNavigator`：先导航文件内差异，边界首次提示、同方向再次触发才跨文件；没有相邻文件时停止。跨文件定位下一文件首个／上一文件末个差异。对应 `src/diff_tab.rs`、`src/editable_diff.rs`、vendored `diff/state.rs`。轻量提示放在 Diff 内，不实现上游提示气泡的全部位置与配置选项。
- `platform/platform-impl/src/com/intellij/ide/actions/SettingsEntryPointAction.java`：设置入口是普通 action button，`actionPerformed` 打开 popup，不是右键专属操作。本地保留直接打开设置的既有简化入口。Windows 实测物理左键失败而直接投递客户区点击消息成功，结合 GPUI native caption hit-test 定位到标题栏 Drag hitbox；`src/ui.rs` 的 `titlebar_button` 遮挡底层 caption 命中并阻止左键冒泡，应用于设置、更新、推送和标题栏菜单，保留空白区域拖动。
- `platform/platform-impl/src/com/intellij/openapi/wm/impl/welcomeScreen/FlatWelcomeFrame.kt`：欢迎页独立管理窗口尺寸／位置。该文件不证明项目窗口必然围绕欢迎页中心；本地按用户明确要求，以欢迎页中心创建项目窗口并限制在当前显示器内，成功后关闭欢迎页；普通项目切换不再强行调整窗口大小。对应 `src/main.rs`、`src/ui.rs`。

“加载更多”仍是本地有界历史分页入口，不声称是上游同款控件；固定 24 DIP 高并禁止 flex 拉伸，避免占用额外阅读空间。

## Log Reset（2026-10-19）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，已安装 Rebased 1.1.20，二者不声明完全一致。本轮实际读取：

- `plugins/git4idea/backend/src/reset/GitResetAction.java`：选中提交打开弹窗，默认/记忆模式，确认后后台执行。
- `plugins/git4idea/shared/src/git4idea/reset/GitResetMode.java`：Soft/Mixed/Hard/Keep；默认 Mixed。
- `plugins/git4idea/backend/src/reset/GitNewResetDialog.kt` 与 `shared/resources/messages/GitBundle.properties`：源分支（或 detached HEAD）、目标短 hash/说明/作者、纵向单选和模式解释，Reset/Cancel；Hard 在说明中警告丢失本地改动，不额外叠加确认框。
- `plugins/git4idea/backend/src/reset/GitResetOperation.java`：先保存文档，执行后刷新 VFS、changes、untracked；错误通知。读取 PROPOSE/FAIL 两种覆盖冲突处理策略。
- `plugins/git4idea/backend/src/commands/GitImpl.java` 的 reset overload/private implementation：`git reset <mode> <target>`。
- `plugins/git4idea/tests/git4idea/reset/GitResetTest.kt`：Hard 后文件内容刷新、Soft 后变更状态刷新。

本地映射：`src/reset_operation.rs`（Prepared 绑定工作树/HEAD/分支，在写锁下重验、限制输出、查询执行后状态）；`src/reset_dialog.rs`（四模式、说明、确认）；`src/layout_preferences.rs`（仅确认时记录模式）；`src/ui.rs`（单选右键入口、保存编辑器、异步执行、刷新文档/Log/本地变更与已有错误详情）；`tests/reset_operation.rs`（实际临时 Git 仓库）。

保留差异：单仓库；阻止已有 merge/rebase/conflict/index recovery 时执行 Reset，避免破坏现有恢复流程；采用上游 FAIL 策略，Keep 拒绝覆盖本地更改时显示失败，不实现 PROPOSE 的 Smart/Force/Cancel 暂存恢复选择器，不自动降级 Hard。自有锁防止应用内部并发，外部 Git 仍可能竞争。弹窗使用已有 GPUI 模态容器而非 Swing 独立窗口；未声称像素级一致。

## 换行符差异与 Reset 中途取消修复（2026-10-19）

仍对照 `4aec35256d765c217aa0d662b82ac4e860c396fd`（不等同安装版 Rebased 1.1.20）。本轮读取 `platform/diff-impl/src/com/intellij/diff/tools/util/DiffNotifications.kt::createEqualContents`、`tools/util/base/TextDiffViewerUtil.java::createEqualContentsNotification/areEqualLineSeparators`：JetBrains 分别保留 Document 文本与换行符属性，文本相同但 separator 不同时提供信息提示。Plico 的 working_copy 曾将两侧通过 Document::text 转成 LF，比较时丢失格式差异。现在 Document::comparison_text 保留未编辑的精确换行（含混合格式）；编辑后的比较使用与保存相同的格式。DiffFile::from_editor_sources 比较原始行，再生成 LF 编辑器坐标，避免 CRLF 字节偏移污染装饰和单栏映射。EditableDiff 增加“仅换行符不同”提示；按用户要求仍高亮实际格式变化的行，这是相比上游仅提示的有意差异。普通 LF baseline 共享 Arc，不新增全文件持久副本。

本轮重读 `plugins/git4idea/backend/src/reset/GitResetAction.java`、`reset/GitResetOperation.java`，并读取 `commands/GitTextHandler.java::waitForProcess/tryKillProcess/waitAndHardKillProcess`：上游任务可取消，取消时先 destroyProcess 后必要时 hard kill；Reset 操作未实现工作树/index/HEAD 的事务回滚。不能把上游取消等同于可恢复到原状。

Plico 原实现通过共享 Cancel 杀掉 Git 进程。可控 smudge filter 回归测试在 reset --hard 正在 checkout 时发出取消，旧实现 HEAD 未更新，留下中途状态。修复差异：准备/确认阶段仍可取消；真正执行 Reset 时隐藏底部取消并明确提示不可取消；执行器最后检查取消后使用独立 token 完成 Git 命令，再刷新结果，不做无法保证保留本地改动的反向 Reset。准备阶段完成与取消竞争时不再弹出迟到的确认框。此改动不宣称断电/外部强杀也有事务保障，也不会自动处理用户已有的中途状态。

回归：`tests/working_copy.rs` 的 EOL-only 高亮及 CRLF/LF/CR/混合换行/Unicode/末尾换行/保存字节测试；`tests/reset_operation.rs` 的慢 smudge 中途取消测试及既有准备前取消测试。复现失败日志 `/tmp/plico-eol-red.log`、`/tmp/plico-reset-cancel-fast.log`；修复后测试各 6 项通过。

### WSL 保存与 Git 输出误报（2026-10-19）

参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`（与安装版 Rebased 1.1.20 不保证一致）。已读 `platform/util/src/com/intellij/util/io/SafeFileOutputStream.java`：备份、同步写入、失败恢复；`PreemptiveSafeFileOutputStream.java`：完整临时文件、原子移动与失败恢复，不能直接截断原文件后忽略错误。本地 `src/file_save.rs` 保留完整临时文件、外部修改复验、保存前取消检查；Windows 的 WSL UNC 路径在 ReplaceFileW 返回 ERROR_NOT_SUPPORTED (50) 时改用同目录 MoveFileExW(REPLACE_EXISTING)，其他路径与错误不降级，不先删除目标。Windows 实际测试在 `\\wsl.localhost\legion-wsl\tmp\plico-wsl-save-test` 复现 code 50，定位为 replace 阶段，权限设置、write、sync 均成功。修复后原生 WSL 保存的 5 项测试通过（含外部修改、取消、文件删除、BOM/CRLF）；WSL redirector 可能接受但不回报 DOS readonly 属性，测试仅在属性受支持时断言 ReadOnly。当前权限复制仍限于 Windows 暴露的属性，尚未验证 POSIX executable/ACL 元数据保留。

已读 `plugins/git4idea/backend/src/repo/GitUntrackedFilesHolder.kt`：按 dirty scope 查询状态，未追踪项单独维护并保留目录语义；未追踪文件确实参与状态查询，但不能据此认定用户的真实仓库输出超限。本地 `local_changes` 使用 `status --porcelain=v1 -z --untracked-files=all`。已复现独立缺陷：标准错误仅 70,000 字节、标准输出仅 `ok`，也报 8 MiB 超限。`src/git.rs` 将诊断流与数据流限额分离，诊断持续排空但最多保留 64 KiB 前缀和截断标记；失败命令保留实际诊断，成功命令不因大量警告失败，数据仍限制 8 MiB。新增回归覆盖以上三种情况；Linux backend 194 项测试通过。用户确认仓库不在本机，真实输出大小无法在本环境测量，未擅自扩大限额或遗漏未追踪文件。

### 完整 Diff 预览与标题栏菜单命中（2026-10-19）

参考 commit 仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，与安装版 1.1.20 不保证一致。已读 `platform/diff-impl/src/com/intellij/diff/tools/simple/SimpleDiffViewer.java` 的 full immutable document text 比较流程、`tools/util/text/SimpleTextDiffProvider.java` 和 `comparison/ComparisonManagerImpl.java`：文件文本是完整输入；行内比较过大时保留外层行差异。Plico 的嵌入预览此前走 20 行上下文 patch，独立 Tab 才走完整 patch；现在 `src/diff_target.rs::read` 与 `src/diff_tab.rs` 统一完整文件加载，保留按文件加载与现有字节限额。300 行复现测试先失败后通过；8 项 repository diff 测试及 30,000 行真实 Git 文件的完整内容/高亮范围回归通过。未提高 5 MiB 编辑文件、8 MiB Git 输出限制，也未改变超长单行语法高亮策略；大文件原生 UI 尚待验收，不能据此宣称所有大文件场景已解决。

标题栏已读 `platform/platform-impl/src/com/intellij/openapi/wm/impl/headertoolbar/ProjectToolbarWidgetAction.kt::createPopup`、`plugins/git4idea/frontend/src/com/intellij/vcs/git/frontend/widget/GitToolbarWidgetAction.kt::createPopup`、`platform/platform-impl/src/com/intellij/openapi/wm/impl/ExpandableComboAction.kt::createCustomComponent` 和 `customFrameDecorations/header/toolbar/MainMenuButton.kt::ShowMenuAction`：正常按钮 action 打开对应 popup，不要求右键。Plico 根因为 `src/ui.rs::titlebar_button` 在子按钮上遮挡命中并停止冒泡，而 GPUI BasePopover 的开关监听在外层。只去掉停止冒泡仍失败；最终 `titlebar_menu` 在完整 popup trigger 外层隔离标题栏拖动，子按钮不再遮挡 popup 事件。设置/更新/推送仍保留直接按钮的命中处理。无意改变菜单布局和内容。

Windows release 构建通过；真实左键点击三个入口均正常打开，相互切换关闭前一个 popup。截图 `artifacts/eol-reset-20261019/main-menu-fixed.png`、`project-menu-fixed.png`、`branch-menu-fixed.png` 已查看。第一轮 `*-after.png` 为失败尝试，不作为通过证据。未进行像素级验收。

### 窗口缩放后提交详情继续选择（2026-10-19）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`（与安装版 Rebased 1.1.20 不保证一致）。已读 `platform/util/ui/src/com/intellij/openapi/ui/Splitter.java::DividerImpl.processMouseMotionEvent`：只在 MOUSE_DRAGGED 更新分隔比例；普通移动与拖拽有独立语义。已读 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/details/commit/CommitDetailsPanel.kt` 的详情布局及 ContainingBranchesPanel：只读文本组件保留文字操作，不以禁用文字选择规避问题。

Windows 原生复现：拖动窗口最右边缘，释放后仅移动鼠标到提交详情，文字仍被连续选中；内部面板分隔线的首次测试未复现。底层 `vendor/gpui-base/src/text_selection.rs` 的 mouse-move handler 仅检查 is_selecting，原生尺寸调整消费释放事件后状态可残留。现对非触摸文字选择核对 event.pressed_button：左键未按下时清理准备标记并结束手势，不更新选区；保留既有选区及主动拖选，触摸 handle 走原独立流程。此为 GPUI 事件生命周期修复，不复制 Swing 实现。

同一原生动作脚本 `artifacts/eol-reset-20261019/window-resize.ps1` 与 `check-selection.ps1`：修复前详情区域采样 4746 个蓝色像素并失败，修复后 478（链接文字等）并通过，截图也已人工核对没有选区。Windows release 构建通过；`window-resize-hover.png` 为修复后截图，`text-selection-preserved.png` 验证主动拖选标题后释放、移动到其他详情内容，选区仍只保留标题。没有禁用详情复制，没有新增滚动自动化。此原生回放覆盖实际窗口消息路径，尚未添加模拟该原生消息丢失的 headless 单元测试；不宣称所有平台事件路径均已验证。

### 文件编辑器获得焦点后光标不闪烁（2026-10-19）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 与快照可能不同。已读 `platform/platform-impl/src/com/intellij/openapi/editor/impl/EditorImpl.java::focusGained/updateCaretCursor/CaretCursor.setVisible/isCaretShown`：获得焦点显式显示光标、重置闪烁计时；绘制受编辑器输入焦点控制。Plico `src/project_view.rs::FileTab` 原来直接操作 FocusHandle，文件页挂载/激活路径出现 focus=true、show_cursor=false，连续帧不闪烁；临时绘制记录确认颜色为不透明近白色、光标几何数为 1，排除配色和缺失几何。

本地改为调用 EditorState::focus，使焦点与 BlinkCursor 启动同步；移除文件加载后及下次 render 的重复聚焦，保留异步加载完成再聚焦与预览不抢焦点的原规则。未改变文字内容、光标颜色或闪烁周期。没有把只读文本的 I 形鼠标指针与编辑插入光标混为一谈。

原生 Windows release 构建通过。`artifacts/eol-reset-20261019/caret-frames.ps1` 每 150 ms 捕获一次正文区域，共 12 帧；`check-caret.ps1` 比较帧像素。修复前 `caret-before.png` 变化为 0，断言失败；修复后 `caret-fixed.png` 与 Log 切换返回后的 `caret-tab-return.png` 均为 546 个变化像素，断言通过。已查看修复后帧序列，变化确为行尾插入光标亮灭。临时 DEBUG-caret/PLICO_CARET_DEBUG 代码已移除；原生脚本保留在忽略的 artifacts 内，不作为跨平台全覆盖声明。

### 只读 Diff 首次打开只有一屏滚动范围（2026-10-19）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，与安装版 Rebased 1.1.20 不保证一致。本轮已读 `platform/diff-impl/src/com/intellij/diff/tools/util/side/TwosideTextDiffViewer.java::createEditorHolders/installEditorListeners` 以及 `platform/diff-impl/src/com/intellij/diff/tools/util/SyncScrollSupport.java::TwosideSyncScrollSupport`：双栏各自持有完整编辑器及滚动模型，监听可见区域并同步另一侧；差异定位与文件内容的滚动范围独立。保留本地差异：GPUI 使用共享虚拟列表对齐双栏，仍只绘制可见行，不引入 Swing 编辑器或全文件预绘制。

失败复现为 500 行 TSX 的实际 Diff 布局：文件行数完整，但首次三帧后 viewport 高 1014px、最大滚动距离只有 15px。`gpui-pre 0.3.9/src/elements/list.rs::List::prepaint` 首次布局和宽度变化时把所有条目的 size_hint 清空；原 `DiffState::update_layout` 在 prepaint 前设置的估算因此失效，F7 跳转只会逐步积累已测行高。异步高亮仅更新 presentation，没有截断文件。

本地 `vendor/gpui-component/src/diff/state.rs::record_paint` 在列表完成 prepaint 后检测其实际宽度变化，恢复未测行高度估算并安排下一帧。每次宽度变化处理一次，不在普通绘制帧重建估算。`src/diff_tab.rs::readonly_tab_has_full_scroll_extent_without_f7` 使用与 Tab 正文相同布局，验证 500 行双栏首次显示、缩窄及切单栏都有完整滚动范围；连同 F7 边界和源码跳转测试共 4 项通过。测试不合成鼠标滚动；真实滚动体验仍由用户手测。完整 Tab 的 debug 测试会触发现有 ModeSelector 重复 hover 样式断言，因此渲染回归测试仅包裹真实 Diff 正文，工具栏另以 Windows release 原生验证。

Windows release 构建通过；自建 500 行 TSX 临时仓库在独立 Diff Tab 的修复前后截图为 `artifacts/diff-extent-20261019/before.png`、`after.png`。原生核对首次挂载、布局及内容显示，没有执行滚轮/拖动滚动自动化。默认滚动条为空闲自动隐藏，静态截图不证明完整滚动范围；该断言由上面的实际 GPUI 布局回归测试验证，用户真实仓库的鼠标滚动仍待手测。本次不宣称像素级验收或发布新包。

### 双栏 Diff 中线拖拽（2026-10-19）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 与快照可能不同。已读 `platform/diff-impl/src/com/intellij/diff/tools/util/DiffSplitter.java::createDivider`，以及 `platform/util/ui/src/com/intellij/openapi/ui/Splitter.java::DividerImpl.processMouseMotionEvent/processMouseEvent/getMinProportion`：分隔线使用左右调整光标，只在鼠标拖动时按相对位置更新左右比例，比例有边界；双击恢复 0.5。

本地 `src/diff_splitter.rs` 提供 8px 命中区，覆盖现有细线，捕获拖动并在释放/无左键时结束；`src/diff_tab.rs` 持有当前页面比例，标题与正文共用，退出页面或切模式清理拖拽状态。只读双栏通过 `vendor/gpui-component/src/diff/mod.rs` 的 split_ratio 联动列宽、裁切和底部横向滚动条；软换行将比例纳入 LayoutKey，重新测量行高。本地可编辑 Diff 的两个 Editor 容器在 `src/editable_diff.rs` 同步使用该比例。

保留差异：比例范围为 10%–90%，避免一侧消失；比例只保留于当前 DiffTab，不新增跨启动持久化。只读双栏保留既有共享横向偏移，以较窄一侧确定最大行程，不引入独立两侧横向状态。单栏/只读新增删除单列没有中线；三栏合并视图本轮不增加拖拽。功能阶段核对通过，不宣称像素级还原。

验证：分隔线比例边界与真实 GPUI 鼠标按下/移出中线/释放/双击测试 2 项通过；30:70 双栏完整滚动范围、窗口变窄、单栏切换以及 F7/源码跳转 4 项通过。Windows release 构建通过；自建 500 行 TSX 仓库原生历史 Diff 拖动与双击恢复等宽、本地变更可编辑 Diff 拖动已截图核对，标题与正文对齐、释放后无继续拖动/文本选区，证据 `artifacts/diff-splitter-20261019/dragged.png`、`centered.png`、`editable-dragged.png`。没有执行滚动自动化。较宽的 diff_ 测试筛选仍包含已记录的基线失败 `releasing_diff_clears_padding_from_retained_source`（实际 0，期望 2）；本轮未处理该独立生命周期测试，不宣称全套通过。

### 2026-10-19 Diff 高亮层次与独立字体设置

- 参考源码：`4aec35256d765c217aa0d662b82ac4e860c396fd`；现场 Rebased 1.1.20，截图 `artifacts/diff-fonts-20261019/rebased-before.png`。对照其已打开的 `force-push-native-en-keyboard/arrow-boundary-fixture/review.txt`（`rewritten version` → `rewritten versions`），不修改该仓库。
- 已读取：`SimpleDiffChange.java`、`SimpleDiffChangeUi.java`、`DiffUtil.java#getLineDiffType/getDiffType`、`TextDiffTypeFactory.java#getIgnoredColor`，以及 `DefaultColorSchemesManager.xml` 的 Darcula DIFF_* 和 `themes/islands/IslandSchemeDark.xml`。规则：删除灰 #484A4A、新增绿 #294436、替换蓝 #385570；行内变化用完整颜色，存在行内变化的行背景混入 60% 编辑器底色；背景覆盖整行。原实现只读视图使用主题 success/danger 的 12% 透明色，可编辑视图仅着色文字范围，明显弱于上游。
- 本地映射：`vendor/gpui-component/src/diff/{mod,document}.rs` 改用上述颜色和行内强调；`src/editable_diff.rs` 在后台比较时生成整行／行内装饰；`vendor/gpui-base/src/input/{editor/decorations,base/element}.rs` 分开绘制整行背景与字形背景。虚拟列表只绘制可见行，未引入完整文档 UI 节点。
- 字体源码：`AppearanceConfigurable.kt`（系统／自定义 UI 字体、字号、apply/reset）、`AppFontOptionsPanel.kt`（字体、字号、预览）、`AppEditorFontOptions.java`（独立持久化编辑器字体）及 `UISettingsState.kt`。本地 `src/ui_font.rs` 持久化 UI 与编辑器两套字体，设置草稿在 Apply/OK 才应用，Cancel 丢弃。`src/ui.rs` 外观／编辑器字体页提供已安装字体、字号、预览与恢复默认。项目编辑器、可编辑 Diff、只读 Diff 使用同一编辑器设置，去除只读 Diff 二次 rem 缩放。
- 保留差异：未复制 Java/Swing 框架；暂不提供字重、连字、备用字体与独立行距。UI 字号 9–32、编辑器 9–40；选择器使用本项目菜单。中央连接与块级还原见下条；内侧行号布局、曲线圆滑程度和部分菜单仍有差异，不宣称像素级还原。行内比较继续复用现有有长度限制的轻量比较器，不引入语义分析。

### 2026-10-19 Diff 中栏高亮与块级还原

- 同一参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，已读 `SimpleDiffChangeUi#createAcceptOperation/createApplyRenderer`、`SimpleDiffViewer#replaceChange`、`TextDiffViewerUtil#applyModification`、`DiffUtil#applyModification/deleteLines/insertLines/replaceLines`、`DiffDividerDrawUtil.DividerPolygon#paint`。只有目标侧可编辑才显示应用操作；左侧到本地工作区为 Revert；应用前验证变更有效性，使用一次可撤销的文档写命令。中栏根据两边真实显示位置连接变更范围，空区间以细边连接。
- 本地：`src/editable_diff.rs` 在双栏中预留 48px 中栏，按包装／折叠／对齐后的坐标绘制连接，箭头避开中央拖拽命中区；点击复验 generation、工作区内容和文档锁，再通过编辑器一次替换进入原有撤销及自动保存。`src/diff_revert.rs` 使用 UTF-8 字节范围和完整行分隔符，适配本地 DiffFile 不包含尾部空行的模型。`vendor/gpui-base/src/input/base/{state,element}.rs` 提供显示范围，整行背景延伸到行号区与编辑器边缘。只读双栏也加入中栏着色，不提供写入箭头。
- 保留差异：本地可编辑连接使用直线多边形；只读虚拟行使用对齐色带；暂未实现上游 Ctrl+应用的追加模式、应用快捷键与统一视图的块操作按钮。范围仅为右侧可编辑的工作区；不会修改历史 blob 或暂存区。
- 验证：块替换测试涵盖新增、删除、替换、中文及 EOF 分隔符；GPUI 测试确认单次撤销和拒绝过期比较。原有只读 F7 边界、完整文件滚动范围两项回归通过。Windows 独立 `Plico-validation/diff-fonts-20261019/revert-fixture` 实测箭头替换、Ctrl+Z、自动保存后磁盘内容；截图 `revert-diff.png`、`revert-applied.png`、`revert-undo.png`。未操作用户仓库的写入或用户的 Rebased 进程，滚动手感仍由用户手测。
- 字体原生验证：`editor-font.png`、`sizes.png`、`families.png`、`diff18.png`、`ui-applied.png`；编辑器 18px 与 UI 13px 独立，取消字体预览后持久化值仍为原值。设置 Apply 后保存 UI Arial / 编辑器 JetBrains Mono，重启独立测试程序仍保持。

- 最终原生截图 `artifacts/diff-fonts-20261019/final-connectors.png` 确认整行背景覆盖右侧内部留白，与中栏无断缝；同一测试仓库另实测恢复删除行、移除新增行，磁盘结果均只改变目标块。最终 Windows release 构建成功；本轮未创建 Git 提交或推送。

### 2026-10-19 还原箭头资源纠正

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20。已读 `DiffUtil#getArrowIcon`：LEFT 来源使用 `AllIcons.Diff.ArrowRight`；`platform/icons/src/PlatformIconMappings.json` 将旧 `diff/arrowRight.svg` 映射到现代 `expui/diff/arrowRight@14x14.svg`，暗色资源为双尖括号，14×14、#CED0D6。之前手绘单箭头不符，现原样引入 `assets/icons/diff-apply-right_dark.svg`，通过 `src/assets.rs` 注册，`src/editable_diff.rs` 按 14×14 居中绘制，保留原命中范围及还原行为。来源与 Apache-2.0 声明见 `assets/icons/SOURCE.txt`。

验证：引入 SVG 与上游逐字节一致；cargo check、Windows release 构建通过。Windows 原生截图 `artifacts/diff-fonts-20261019/rebased-apply-icon.png` 确认双尖括号正确显示。未改还原动作逻辑。

### 2026-10-19 主窗口职责拆分（保持现有交互）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版 Rebased 1.1.20 与该快照不保证一致。本轮对照 `platform/vcs-log/impl/src/com/intellij/vcs/log/paint/SimpleGraphCellPainter.kt` 的绘制／命中部分：非选中、选中外轮廓、选中本色按层绘制，节点命中优先于边；`platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ChangesViewPanel.kt` 的面板与工具栏组合入口。字体及设置行为沿用前述已记录规则；本轮未重新验证所有上游 Git 写操作实现。

本地映射：原 `src/ui.rs` 的共享 graph 绘制与命中转入 `src/ui_graph.rs`，调用者仍控制选择和查询；颜色、文件／引用图标、项目头像及 Tab 外观转入 `src/ui_style.rs`；快捷键动作声明转入 `src/ui_actions.rs`，动作命名空间保持不变。窗口内部按仓库生命周期、Log、设置、Tab、远端和历史操作等职责分入 `src/ui/`，原设置 UI 位置现为 `src/ui/settings.rs`，原 Log 行位置现为 `src/ui/log_history.rs`。设置草稿封装、页标识改用枚举，保留控件 ID、应用／取消顺序和持久化语义。

保留差异及理由：不翻译 Swing 组件层次；轻量 GPUI 绘制／异步任务设计不变。私有功能文件仍是同一 `Review` 的实现，共享状态尚未隔离；本次先移除其他视图依赖主窗口工具函数的结构问题，避免同时重写 Git 状态机。没有以拆分文件作为像素级还原或完整架构重构的证明。

验证：135 个搬移方法无缺失，规范化比较仅设置页枚举相关 5 个方法变化。194 项库测试、15 项桌面测试通过，已知基线失败 `releasing_diff_clears_padding_from_retained_source` 排除；cargo check 与 Windows release 构建成功。独立测试实例原生点击提交文件打开 Diff、打开设置字体页、取消并返回 Log，截图 `artifacts/diff-fonts-20261019/quality-{diff,settings,log}.png`。保留用户 Rebased／IDE 进程，未执行用户仓库写操作。


### 2026-10-19 操作生命周期与编辑器 Tab 状态

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版 Rebased 1.1.20 与该快照不保证一致。本轮已读：

- `plugins/git4idea/backend/src/fetch/GitRemoteOperationQueueImpl.kt`：按仓库／远端组织操作占用，等待允许取消，完成时通过 finally 释放占用。提取规则是占用随任务实际退出释放，取消请求不等于完成。
- `plugins/git4idea/backend/src/reset/GitResetOperation.java`：先保存文档，在工作树活动范围内执行 Reset，随后刷新并报告结果。
- `platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/EditorWindow.kt::addComposite/closeFile/computeIndexToSelect`：按稳定的 composite 身份关闭；非活动页关闭保留当前选中；新页可插入活动页后，预览替换保留位置；关闭活动页支持 MRU／右邻等配置，默认回到前邻。

本地映射：`src/active_operation.rs` 封装唯一运行操作、阶段、取消令牌和保存门禁；`src/ui/operation.rs` 将任务进入／退出与文档锁联动，保存失败不进入后续操作。所有相关调用方传递本次操作 ID，旧任务不能释放新任务占用。文档锁不再由绘制过程推导更新；本地提交状态也参与锁定。具体结果、工作树刷新、冲突和恢复流程仍由原功能模块负责。

`src/editor_tabs_state.rs` 封装有序 ID 与唯一活动 ID；`src/ui/editor_tabs.rs` 根据身份解析具体视图。`src/ui/chrome.rs` 点击／关闭事件在执行时解析身份；关闭其他／右侧页先取得身份快照。文件页与 Diff 页的类型化实体数组保留为视图存储，不再同时维护可变活动下标。预览替换先停用旧视图，再替换模型身份。

保留差异及理由：Plico 每个窗口只允许一个此类 UI 操作，不复制上游多仓库／多远端队列；Reset 执行阶段不可取消是既有本地安全策略，并非从上述 Java 类推导出的规则。Log 作为固定回退页；不新增 MRU 或关闭后选择方向设置。没有引入通用工作流框架，也没有把所有 Git 操作合并为一种结果处理。

验证：200 项库测试、18 项桌面测试和 46 项临时仓库 Git 集成测试通过；新增测试覆盖取消仍持锁、过期完成、Reset 阶段、保存失败保留草稿且阻止动作、不经过 render 的文档锁更新及混合 Tab 稳定身份。已知桌面基线失败 `releasing_diff_clears_padding_from_retained_source` 明确排除。cargo check、Windows release 构建通过。Windows 原生独立测试实例确认关闭前面的非活动 Diff 后源码页仍被选中、重新打开、菜单关闭全部、修改源码后立即关闭完成落盘；截图 `artifacts/diff-fonts-20261019/architecture-{close-inactive,tab-menu,close-all,saved-close}.png`。同 HEAD Mixed Reset 在临时仓库成功，未将原生测试记为取消验证；取消的边界由自动化测试覆盖。原生保存测试结束后已恢复临时文件内容；未改用户仓库或关闭用户 Rebased／IDE。


### 2026-10-19 Diff 空侧边界、曲线与内侧行号

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 与快照不保证一致。已读 `platform/diff-impl/src/com/intellij/diff/util/DiffDrawUtil.java::LineHighlighterBuilder.done/makeCurve/drawCurveTrapezium`：空范围用行分隔标记表示，首行取顶部、其他位置取前一行底部；曲线为三次贝塞尔，两控制点横向比例为 0.3／0.7，纵坐标分别与对应端点相同。`DiffDividerDrawUtil.java::DividerPolygon.paint` 将空端点处理为细边，非空端点保留块高度。已读 `tools/util/side/TwosideTextDiffViewer.java::createEditorHolders` 与 `DiffUtil.java::isMirrored`：左编辑器设置 LEFT 方向，Diff 以此识别镜像布局。用户截图明确要求两侧行号均靠近分隔区。

本地 `src/editable_diff.rs`：连接多边形改成上述曲线；新增／删除的空侧绘制横贯该侧的 1px 边界；原始版本使用独立内侧行号栏，行号取真实可见源行坐标，折行／对齐填充不增加行号，不随横向滚动位移。`vendor/gpui-base/src/input/base/state.rs::source_line_extent` 对空区间取前一真实行的末端，避免标记落在对齐填充之后；提供可见源行范围供行号栏使用。工作区共享文档的行号设置不变，因此返回文件编辑器不受影响。

`vendor/gpui-component/src/diff/mod.rs`：只读双栏原始版本行号从代码横向滚动容器中分离，放到该列右侧；空侧首个对齐行增加边界，中栏按变化块绘制曲线切片。单栏及三栏保留原布局。保留差异：只读 Diff 仍使用对齐虚拟行，未迁移为两个独立编辑器；跨多行的曲线以标准行高估算整体高度，软换行情况下尚未完成精确逐行几何还原。可编辑 Diff 使用实际显示行坐标。此条不宣称完整像素级还原。

验证：18 项现有桌面测试通过（明确排除已知基线失败 `releasing_diff_clears_padding_from_retained_source`）；新增空侧边界测试覆盖对齐填充、首行和 EOF，单独通过。Windows 原生验收及最终构建结果见 PLAN 同日条目。


### 2026-10-19 滚动时曲线变化的源码核对（仅分析）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 1.1.20 与快照不保证一致。本轮读取 `platform/diff-impl/src/com/intellij/diff/tools/util/SyncScrollSupport.java::TwosideSyncScrollSupport.visibleAreaChanged/ScrollHelper.syncVerticalScroll/transferVisualLine`：普通同步取视口高度约 1/3 处作为锚点，将其视觉行转换为逻辑行，经对应差异区间映射后回到另一编辑器视觉行，并保留行内像素相位；通过同步禁入计数避免递归。`BaseSyncScrollable.java::getRange/transferLine` 按差异边界映射，区间内按相对行数前进并在对侧区间末端截断，不是简单按总文件长度比例缩放。

已读 `tools/simple/SimpleDiffViewer.java::MySyncScrollable`：只有 needAlignChanges 时强制同步纵向像素偏移；差异范围包括各块起止及文件首尾。`tools/util/side/TwosideTextDiffViewer.java::MyVisibleAreaListener` 每次可见区域变化后重绘；`util/DiffDividerDrawUtil.java::createPolygon/getEditorTopOffset` 使用两侧各自的显示范围与滚动偏移计算端点。因此曲线变形是端点相对位置改变的绘制结果，不是独立的缓动动画。仅凭用户静态截图不能认定现场开启了哪个模式。

Plico 现状：`src/editable_diff.rs` 的 ScrollChanged 直接将变化轴偏移复制给另一侧，realign 插入显示空白对齐；中栏已从两侧实际 source_line_extent 重绘，但尚无普通模式的差异锚点滚动映射。`vendor/gpui-component/src/diff/mod.rs` 的只读双栏共用虚拟列表及纵向滚动状态，曲线切片随该列表移动，也不具备两侧独立纵向偏移。结论：具有基础同步及动态重绘，但未实现上游完整的普通同步模式。建议以后按同步映射实现真实位置变化，不添加脱离代码位置的装饰动画。本轮没有更改滚动行为，没有执行滚动自动化或宣称实际滚动手感已验收。

### 2026-10-19 差异锚点同步滚动实现

沿用上节已读的 `4aec35256d765c217aa0d662b82ac4e860c396fd` 中 `BaseSyncScrollable`、`SyncScrollSupport`、`SimpleDiffViewer::MySyncScrollable` 和 `DiffDividerDrawUtil`。`src/diff_scroll.rs` 以视口上方 1/3 为锚点，按差异起止与文档末尾寻找逻辑区间，转换为各自折行后的视觉边界，再按相对行数映射并保留行内像素偏移；一对多映射抑制不足一行的反向微调。水平偏移同步，目标滚动沿用 `set_peer_scroll_offset` 的事件抑制，避免循环反馈。布局事件只重绘，不重新反向驱动主编辑器。

`src/editable_diff.rs` 的双栏去掉显示空白对齐，曲线及空侧细线使用两侧实际几何。历史比较通过 `snapshot` 构造同一双栏视图：两侧均为独立只读 EditorState，不创建 Editing，不注册保存订阅，不开放还原箭头；`DiffTab` 停用时只有实际工作区文档才能走保存。F7、源位置、拖动分栏、折行、字体、行号及高亮粒度保留；单栏、整文件新增／删除和三方比较保留专用 DiffState 渲染。源行／视觉行转换由 `gpui-base` 的 display map 提供，避免手算软换行高度。

保留差异：不增加 Align Changes／同步开关；本轮使用普通映射模式，不实现上游导航动画专用的 anchor 插值或折叠批处理。文件首尾由编辑器自身滚动边界约束。仅活动文件保留双编辑器，历史文本额外建立编辑器 rope／布局；未进行内存基准，不宣称与旧虚拟列表开销相同。此前普通历史双栏曲线按标准行高估算的限制由实际编辑器几何替代，专用单栏／三方路径未扩展。

验证：24 项相关桌面／纯函数测试通过（仍跳过既有 `releasing_diff_clears_padding_from_retained_source` 基线失败）。新增映射用例覆盖新增／删除两方向、非等长替换、折行边界的行映射及文档首尾；只读快照用例验证无保存文档、F7 源位置和还原入口无效。没有滚动自动化；原生静态呈现与构建记录见 PLAN，滚动手感待用户手测。

补充：`DiffFile::editor_coordinates` 沿用既有差异配对，仅转换 CRLF 源字节范围到编辑器 LF 坐标，保留换行差异；新增回归测试覆盖范围合法及差异不丢失。工作区共享编辑器在退出 Diff 时恢复进入前的折行／行号设置，有生命周期回归验证。

### 2026-10-19 本地刷新、查询取消与 Diff 导航同步

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版 Rebased 1.1.20 与源码快照不保证一致。本轮实际读取：

- `plugins/git4idea/backend/src/status/GitChangesCollector.java::collectChanges/collectStagedUnstagedModifications`：用 status 收集各版本，只为同时暂存／未暂存修改（MM）的路径计算工作树相对 HEAD 的净变化。`src/commit_changes.rs::Snapshot::load` 与 `src/git.rs::working_tree_changes_for` 改为此规则，保留首尾 HEAD 复验；没有 MM 时不再执行整仓库 diff，也不写 index。
- `platform/diff-impl/src/com/intellij/diff/impl/DiffRequestProcessor.java::DiffNavigator`：第一次越界提示，第二次沿同方向跨文件；编辑器 hint 使用 HIDE_BY_ANY_KEY／TEXT_CHANGE／SCROLLING，timeout=0。`src/diff_tab.rs` 增加交互清除和退出取消，另加 5 秒自动消失，这是应用户要求的保留差异，不宣称是上游超时时间；提示失效同时解除跨文件确认状态。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ChangeViewDiffRequestProcessor.java::MyGoToChangePopupController/selection strategy`：改变文件时调用 selectChange，同步来源列表。`src/ui/editor_tabs.rs::subscribe_diff_navigation` 接收普通／嵌入 Diff 的 FileChanged，以 repository、revision、area、path／old_path 匹配来源；本地树展开祖先并定位，stash／Log 对应列表跟随。不能仅凭相同文件名改变另一个提交的选择，不重新打开 Diff，避免事件反馈。
- `plugins/git4idea/backend/src/log/GitLogProvider.kt::subscribeToRootRefreshEvents`、`checkin/GitCheckinEnvironment.kt`：提交后 repository.update，Log 订阅仓库更新。Plico 已有 LocalView::Changed → Review::refresh_impl；本轮补事件回归并在 Windows 测试仓库实际提交，确认 Log 已自动新增提交、HEAD 标记移动。保留当前选中历史提交，不强制抢选新提交。LocalView 正在刷新时将重复请求合并为一次后续查询，避免持续取消重启；完成后再处理排队请求，避免漏掉查询过程中发生的保存。
- `plugins/git4idea/backend/src/config/GitExecutable.kt::Wsl/Eel`：上游支持在 WSL／远程执行 Git，转换参数路径与返回路径；Plico 仍在 Windows 启动 Git 访问 UNC，完整 WSL 执行层本轮未替换（涉及写操作、路径、认证和取消的一致性），不能声称与 Rebased 的 WSL 性能等价。
- `platform/platform-util-io/src/com/intellij/execution/process/OSProcessHandler.java::doDestroyProcess/killProcessTree`：递归销毁进程树。压力测试发现只读 read_stream 原来只终止 git 启动进程，后代继续持有输出管道导致 join 等待。现在复用 `process_tree::ProcessTree`（Windows Job／Unix process group），在取消／超时以及 leader 退出后清理自己的后代再收回读线程。回归测试修前 3.01 秒失败，修后约 0.03 秒通过；不是放宽 30 秒限制。

性能测量、原生截图与限制见 PLAN 同日条目。测试只在独立 fixture 写入，已有 Spring／Rebased 仓库只读。没有运行滚动自动化，没有完成全部像素级验收。

### 2026-10-19 Diff 同帧连接与差异定位

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 与快照不保证一致。本轮实际读取 `platform/diff-impl/src/com/intellij/diff/tools/simple/SimpleDiffViewer.java::scrollToChange/MyInitialScrollHelper`：初始化的默认回退定位 FIRST_CHANGE，导航同时移动两侧光标并调用 makeVisible。读取 `tools/util/SyncScrollSupport.java::doMakeVisible/getTargetOffsets`：两侧共享顶部留白，优先视口 1/3；完整变化块能放下时尽量显示全块，过长时留两行；按文件首尾和最大滚动范围调整。普通滚动及 divider 规则沿用前节已经核对的源码。

根因：Plico 编辑器在 paint 才发布 last_bounds，中栏和左侧行号 canvas 在 prepaint 读到上一帧位置。不是刻意等滚动停止的降频。`vendor/gpui-base/src/input/base/{element,state}.rs` 现于 prepaint 发布本帧 source_geometry（文本坐标、行高、可见源行），外部行号和中栏读取同帧数据；不提前执行保存、输入或滚动事件，不增加轮询／延时。

`src/diff_scroll.rs` 将上述双侧定位规则翻译成目标偏移计算；`src/editable_diff.rs` 等首次布局后应用两侧偏移，抑制普通滚动映射覆盖明确的导航目标；`src/diff_tab.rs` 为激活后的文件设置首次差异定位，保留反向跨文件定位末处差异，自动定位不抢走已有焦点。`src/unified_editing.rs` 复用单侧定位；`vendor/gpui-component/src/diff/state.rs` 的专用虚拟列表增加上下文留白。

保留差异：未实现上游导航滚动动画；单栏投影以目标源行定位，虚拟列表按行高估算上下文（软换行／额外注释可能不同于精确像素高度）。双栏使用实际显示行映射及真实视口。没有改动原有按需加载范围或新增全文缓存。

验证：新增首帧 prepaint 回归修前失败、修后通过；新增实际编辑器导航测试覆盖首次布局、下一处／上一处均在上方 1/3，纯函数覆盖短块／长块／首尾。30 项桌面相关测试通过，仍明确跳过既有 `releasing_diff_clears_padding_from_retained_source` 基线失败。Windows release 构建通过；独立 400 行 navigation-fixture 的首处（121 行）与下处（241 行）原生截图见 `artifacts/diff-fonts-20261019/navigation-{current,next,previous}.png`。一次全黑截图被验收脚本拒绝，未计为成功，之后重新截图正常。未执行滚动自动化；持续滚动中的连接效果待用户手测，不以静态截图宣称动态体验已验收。

### 2026-10-19 Log 自动刷新与分支上下行标记

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版 Rebased 1.1.20 与源码快照不保证一致。本轮实际读取：

- `plugins/git4idea/backend/src/repo/GitRepositoryUpdater.kt::filesChanged`：监听 Git 管理目录的 HEAD、heads/remotes、packed-refs、config、reftable 等变化，更新 repository；index／工作树变更另行通知 changes。`backend/src/log/GitLogProvider.kt::subscribeToRootRefreshEvents/refreshOnRepoUpdate` 把仓库和 tag 更新转成 Log refresh。
- `platform/vcs-log/impl/src/com/intellij/vcs/log/data/VcsLogRefresherImpl.kt::handleRefreshRequests`：累积多个刷新请求，批量处理后更新数据包，不为每个事件重建一次 UI。
- `backend/src/branch/GitBranchIncomingOutgoingManager.java::activate/updateIncomingScheduling/requestRemoteInfo/calcBranchesWithIncoming/calculateBranchesWithOutgoing/getCommitsForBranch`：本地仓库事件重算上下行；远端策略支持 FETCH／LS_REMOTE／NONE。LS_REMOTE 比较远端和本地追踪引用 hash，未获取对象时也可显示 incoming，但不编造数量。incoming 比较 upstream，outgoing 比较 push target，二者独立。`shared/src/git4idea/config/GitVcsOptions.kt` 当前快照默认 FETCH，`backend/resources/intellij.vcs.git.backend.xml` 的检查周期默认 20 分钟。
- `plugins/git4idea/tests/git4idea/branch/GitBranchIncomingOutgoingManagerTest.kt` 已读 fetch／ls-remote／同步／手动获取用例；ls-remote 未获取更新时 incoming count 为 0、hasIncoming 为 true。
- `shared/src/com/intellij/vcs/git/ui/GitIncomingOutgoingUi.kt`：incoming 蓝色、outgoing 绿色，各自独立可见。`backend/src/ui/branch/GitBranchWidget.kt` 订阅仓库和 incoming/outgoing 事件，tooltip 使用计数。`platform/dvcs-impl/shared/resources/DvcsIconMappings.json` 把旧 incoming/outgoing 映射至 `icons/new/changesUpdate@12x12_dark.svg`／`changesPush@12x12_dark.svg`；本地原样引入现代 12px 资源，来源和许可证见 assets/icons/SOURCE.txt。

本地实现：`src/branch_updates.rs` 封装引用快照、HEAD／上游／推送目标、计数及只读远端检查；`src/ui/auto_refresh.rs` 持有仓库作用域的独立本地／远端任务及取消令牌。每 5 秒只查询引用和 HEAD，引用没有变化不重算 rev-list、不重载 Log、不遍历工作树。忙于查询或写操作时推迟，不中断用户操作，期间的多次外部变化由下一次完整引用快照合并。变化后复用 preserve 刷新，保留筛选、选中 OID 和列表位置。切换仓库／关闭窗口取消旧任务，远端结果还检查仓库路径和 upstream 身份，避免迟到结果污染。入口接入初次打开及欢迎页／项目切换。

远端独立每 5 分钟按当前分支 upstream 执行 ls-remote，30 秒超时、输出有界、取消沿用 ProcessTree；没有自动 fetch/pull，不改对象、引用或工作树。提示数量基于已获取引用，未 fetch 的更新单独显示箭头和说明。远端失败保持 unknown，不弹反复错误窗或假报“已同步”；普通 SSH 使用 BatchMode，GCM 禁止交互，已有自定义 SSH 命令不覆盖、该后台检查返回 unknown，手动 Git 操作不变。

保留差异：没有复制 IDE VFS，使用有界引用轮询以兼容 Windows→WSL UNC，不启动高频递归工作树扫描；不是整个文件系统的自动重载。只检查当前分支远端，采用 LS_REMOTE 而非上述快照的默认 FETCH，周期 5 分钟而非 20 分钟；本轮不新增策略设置。尚未获取的远端提交需显式 fetch 后才会出现在 Log。未进行真实跨机 WSL 性能测量，不宣称与上游事件驱动延迟／开销相同。

验证：3 项临时仓库集成测试覆盖外部提交、分叉双向计数、未获取远端更新、检查不修改 refs、fetch 后计数、游离 HEAD、无提交／无 upstream、tag 变化、取消及三角工作流的独立 push target；30 项现有桌面测试通过（仍排除已知 releasing_diff_clears_padding_from_retained_source 基线失败）。Windows release 构建通过。独立 auto-refresh-fixture + 本地 bare remote 原生截图 `artifacts/diff-fonts-20261019/auto-refresh-start.png` 展示双箭头；外部 fetch＋commit 后 `auto-refresh-external.png` 展示新提交、远端标记更新，原选中 Local change 保留。无用户仓库写入、无真实网络 push。最终现代图标截图记录见 PLAN。

### 2026-10-19 Reset 布局与编辑提交信息

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版 Rebased 1.1.20 与该快照不保证一致。本轮实际读取：

- `plugins/git4idea/backend/src/reset/GitNewResetDialog.kt`：单仓库目标行包含来源分支、短 hash、截断至 40 字符的 subject 与作者；接着说明 HEAD／工作树／index 的关系；Soft／Mixed／Hard／Keep 各自带缩进说明。`src/reset_operation.rs` 分别读取 subject/author 并缩短；`src/reset_dialog.rs` 使用一行摘要、四组 radio 与说明、右下 Reset/Cancel，Hard 的丢失本地更改提示单独换行；沿用既有模式记忆与执行阶段不可取消的规则。
- `backend/src/rebase/GitRewordAction.kt`、`GitSingleCommitEditingAction.kt`、`rebase/log/GitCommitEditingActionBase.kt`：仅单选；提交必须包含在 HEAD 历史中且没有发布到受保护远端分支；非 HEAD 的 merge 禁用，HEAD merge 可编辑；正常／detached 允许，其他操作进行中禁止，rebase 中的 HEAD 是例外；root 不禁用，先确认。`src/reword.rs::Prepared` 复验这些规则，额外阻止未解决冲突与应用恢复记录。`src/ui/reword.rs::Gate` 为当前选择异步读取资格，切换选择释放任务并取消；右键菜单禁用已知不符合资格的项目，F2 使用同一门禁；包含关系尚未返回时按上游保持可点击，点击后先异步完成检查再打开编辑器，避免第一次右键必须关闭重开。确认时及写入前再次复验 HEAD、分支、目录、包含关系和保护规则，旧弹窗不是持久写授权。
- `backend/src/rebase/log/GitNewCommitMessageActionDialog.kt`：预填完整消息、焦点/光标位于开头、空白消息禁用 OK，确认时检查仓库是否变化。`src/reword_dialog.rs` 对应标题、hash/作者标签、多行输入、OK/Cancel；异步复验期间禁用编辑，关闭时取消复验，不进入写操作。
- `backend/src/rebase/GitRewordOperation.kt`：旧流程对 HEAD 使用 `commit --amend --only --no-verify`，其他提交使用 reword rebase，不能带入暂存文件。`GitRewordService.kt` 当前快照优先 `GitInMemoryRewordOperation`，失败时才进入原生 Git fallback。本轮采用已读的 `backend/src/inMemory/rebase/log/reword/GitInMemoryRewordOperation.kt`、`GitInMemoryCommitEditingOperation.kt`、`inMemory/GitObjectRepository.kt::commitTree/commitTreeWithOverrides`：生成保留 tree/author/parents 的提交对象，更新引用，不更新工作区；保留配置的签名、复制 notes、执行 post-rewrite。`src/reword.rs` 顺序创建对象，每个对象输出有界；最后用带旧 OID 的 update-ref 发布，发布前允许取消，发布本身不中断。没有 reset/stash，没有临时工作区或中间 rebase 状态；取消时至多留下不可达对象，可由 Git 正常 GC。后续 notes/hook 失败明确报告“提交信息已修改，但后续处理失败”，不重复改写。
- 已读 `plugins/git4idea/tests/git4idea/rebase/GitRewordTest.kt` 与 `tests/git4idea/inMemory/rebase/log/reword/GitInMemoryRewordOperationTest.kt`：HEAD/旧提交/root、保留暂存内容、特殊字符、author/parent、notes/post-rewrite 映射等用例转成 `tests/reword.rs` 的独立 Git fixture 测试。

保留差异：只实现单仓库 UI，无 AI／消息历史工具栏及 IDE 帮助入口，没有 Reword 专属 Undo 按钮（Git reflog 保留）。对象重写不启动原生交互式 rebase fallback；对目标以上的 merge 使用拓扑排序映射父边，保留原 tree 和合并结构，避免原生 fallback 拉平历史；最多 10000 个改写提交，拒绝浅历史边界及非 UTF-8 提交编码配置。不同于上游仅依赖 IDE 状态，未解决冲突和本应用未完成的恢复记录也阻止执行。post-rewrite 通过 Git hook runner `--to-stdin` 传递映射，旧 Git 不支持时作为已完成后的警告。采用应用内弹窗和现有字体/主题，不宣称独立原生窗口或像素完全一致。原生验收与截图见 PLAN 同日记录。

## 2026-10-19 源码编辑器 Git 行变更标记

参考 commit：`4aec35256d765c217aa0d662b82ac4e860c396fd`；宿主已安装 Rebased 1.1.20，版本不保证相同。本轮对照用户提供的三张源码编辑器／标记弹层截图，不把源码快照当作已安装版的逐像素证据。

实际读取：
- `platform/vcs-impl/src/com/intellij/openapi/vcs/impl/LineStatusTrackerBaseContentUtil.java`：基线来自 VCS before revision，保存文件不把 Git 标记清零；ADDED／UNKNOWN／IGNORED 不提供普通已跟踪文件基线。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/ex/SimpleLocalLineStatusTracker.kt`、`LineStatusTracker.kt::LocalLineStatusMarkerRenderer`：源码编辑器标记排除 Diff；工具栏依次为上一个、下一个、回滚、Diff、复制原内容。
- `platform/diff-impl/src/com/intellij/openapi/vcs/ex/LineStatusTrackerMarkerRenderer.kt`：仅有原始行时创建带语法高亮的原文预览；纯新增只有工具栏。
- 同目录 `LineStatusMarkerRendererWithPopup.kt`、`LineStatusMarkerPopupActions.kt`：点击 gutter 打开锚定弹层，动作执行前重新找到 range，前后导航不循环，复制仅对原始行有效；滚动／失效后关闭提示。
- `platform/diff-impl/src/com/intellij/openapi/diff/LineStatusMarkerDrawUtil.java`：新增／修改绘制条形标记，零高度删除区间绘制边界标记；继续实读 `paintTriangle` 确认现代 New UI 实际绘制圆角胶囊而非旧版三角，按此实现；命中区比细线宽。
- `platform/vcs-tests/testSrc/com/intellij/openapi/vcs/LineStatusTrackerRollbackTest.kt`：首尾插入、删除、空行及原文恢复场景。

本地映射：`src/line_changes.rs` 提供固定 blob 基线、类型与行区间；`src/source_changes.rs` 负责活动源码 Tab 的后台计算、gutter、原文弹层及撤回；`src/project_view.rs` 接入并在离开 Tab 时释放。沿用共享 `Editing`、`diff_revert::replacement`、编辑器撤销栈和自动保存。撤回前重新检查基线 blob、文档内容及 Git 写操作锁，不修改 index。基线／当前内容只在活动源码视图持有；编辑事件 80ms 合并，差异计算在后台并限制算法时间；每 5 秒复验基线，blob 不变不重复读取全文。

保留差异：普通源码标记以当前路径的 HEAD blob 为基线，index 仅用于确认已跟踪且无冲突；回滚已暂存内容不会同步撤销 index。尚未追溯 staged rename 的旧路径；没有 HEAD 基线的新文件、冲突文件及不支持编码不提供标记，不把读取失败视为空原文。删除采用现代 UI 圆角边界标记。弹层提供前后导航、撤回和复制，暂不复制独立范围 Diff、词级强调开关；“提交此变更”依赖部分提交机制，依用户许可留待按块提交统一实现。宽度与预览高度有上限，超出原文可滚动；未宣称像素级还原。

验证：2 项行区间单元测试、3 项真实 Git 基线测试及 1 项编辑器撤回测试通过，覆盖修改／新增／删除／EOF、Unicode、保存与暂存不改变 HEAD 基线、提交后基线变化、冲突禁用、取消／非法路径，以及撤回保留其他块、Undo、锁定与过期草稿保护。Windows 原生结果另附于本节后续记录。

Windows 原生验证使用独立 `Plico-validation/diff-fonts-20261019/markers-fixture`：初次截图发现 gutter 横坐标重复加了左内边距，已修正；最终 `markers-final-source.png` 显示标记位于行号与正文之间且不压字，`markers-final-deleted.png` 显示现代删除短标记及删除原文预览，`markers-final-added.png` 显示纯新增只有工具栏、最后一块下一项禁用。`markers-popup.png`／`markers-reverted.png`／`markers-undo.png` 验证修改原文预览、单块撤回、Ctrl+Z；宿主 Git 与文件读取核对自动保存结果，其他新增／删除保留，index 未改变。前后导航另修正为下一帧按目标块的实际坐标重新锚定弹层，不沿用上一个块的弹层位置。持续滚动手感仍由用户手测，未投入滚动自动化。未提交、推送或发布。

最终构建补验：`markers-final-added.png` 确认导航弹层已跟随新增块位置；`markers-deleted-restored.png` 与 `markers-restore-deleted.ps1` 实际恢复删除的函数，磁盘断言同时确认原修改块和新增块仍保留、暂存区没有变化。最终 Windows release 构建通过；仅测试自建 fixture，未修改用户仓库或关闭用户 Rebased／IDE。

### 2026-10-19 外部文件修改自动同步

参考 commit：`4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 与此快照可能不同。本轮实际读取：

- `platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/FileDocumentManagerImpl.java::MyAsyncFileListener/contentsChanged/reloadFromDisk`：批量 VFS 内容／删除事件，自己保存的事件不重复重载；无未保存修改的共享文档可重载，文档更新后通知下游。
- 同目录 `MemoryDiskConflictResolver.java::beforeContentChange/processConflicts/askReloadFromDisk`：外部内容与未保存文档冲突时保留内存版本，由用户选择读取磁盘或保留内存，并提供差异比较入口。
- `platform/diff-impl/src/com/intellij/diff/tools/util/base/ListenerDiffViewerBase.java`：文档 Change／文件内容变化触发 scheduleRediff，监听随 viewer 生命周期释放，批量去重。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ChangeListManagerImpl.kt::scheduleUpdateImpl/invokeAfterUpdate`：通过更新队列调度，不在事件回调中直接完成所有扫描。
- `platform/platform-tests/testSrc/com/intellij/openapi/fileEditor/impl/FileDocumentManagerImplTest.java`：已读 `testContentChanged_DoNotReloadChangedDocument`、keepMemoryChanges／defaultAsks 用例，确认默认冲突不能擅自合并或覆盖。

本地映射：

- `src/worktree_events.rs`：notify 8.2 系统文件事件；不跟随符号链接，不响应访问事件，忽略 Git objects 等内部噪声和保存临时文件；单个队列最多 512 个路径，溢出变成一次整体失效。300ms 安静窗口、2s 最长合并窗口。
- `src/ui/worktree_sync.rs`：仓库作用域单一事件源，最多每 2s 消费一批；检查共享文档文件元数据，变化后才读内容，不在 UI 线程执行 IO。UNC／注册失败／worktree 的 Git 管理目录在外部时每 30s 补偿刷新。只有已建立的变更面板、已展开项目树、已打开本地 Diff 才加载对应数据；不因普通文件事件刷新整个 Log。切仓／关闭释放监听和任务，Git 写锁期间延后处理。
- `src/editing/external.rs`：观察状态与冲突 UI 从 Editing 主文件分离。比较磁盘原始字节而非仅规范化文本；同内容保存事件不重置 Undo。自动重载保留共享实体、行列选区、阅读偏移及 BOM／换行符；重载前复验原始 baseline 身份、保存状态、写锁与当前编辑文本（含 IME preedit）。迟到结果不能覆盖已保存／重载的新文档。
- 冲突、删除、不可读或不支持的新文件内容保留原文档并阻止自动保存；编辑不会抹掉冲突提示。可复制编辑器内容、确认从磁盘重载；取消保持双方内容。磁盘恢复为原 baseline 后允许恢复保存，干净文档重新出现时自动重载。不把删除解释为创建文件的许可。
- `src/source_changes.rs` 订阅 Reloaded，立即更新行变更标记。`src/editable_diff.rs` 沿用 Reloaded → recompare；`src/diff_tab.rs` 对 index／HEAD 失效单独更新原侧，不覆盖工作文档；普通本地只读 Diff 重读目标。重叠查询合并，不反复取消慢查询。`src/working_copy.rs::read_base` 不依赖工作文件存在。
- `src/project_view.rs` 外部树刷新保留路径选择和展开状态，并在正在加载时只排队一次；LocalView 复用已有合并刷新与选择保留。

保留差异：不复制 IDE VFS 和三方合并器。冲突用源码／Diff 上方的非模态提示与显式重载确认，避免 AI 连续写入反复弹框；暂未提供冲突双版本比较／强制覆盖磁盘按钮，可先复制草稿。外部重命名刷新列表，旧路径 Tab 保留内容并报告不可用，不推断身份后自动改路径。文档自动重载清空旧 Undo 历史，防止过期撤销记录套到新文本；自身同字节保存不重载。UNC 的全局列表补偿周期 30s，不保证所有文件系统通知都是实时的；元数据无法区分且通知丢失的同大小／同时间戳替换依赖补偿。历史 commit／stash 快照不跟随工作树修改。未验证跨机器 WSL 性能，不声称解决既有 Windows Git 全仓扫描耗时问题。

验证：文件事件队列单测 1 项、真实文件创建／替换／重命名／删除及读取不产生刷新循环的集成测试 2 项；working_copy 6 项（含 index 基线刷新不读写已删除的工作文件、新文件被外部提交后更新到新的 HEAD 基线）；桌面既有及新增 34 项通过（33 项一轮＋新增 baseline 更新用例一项，排除已记录的 `releasing_diff_clears_padding_from_retained_source` 基线失败）。新增桌面测试覆盖物理格式、光标、共享身份、迟到结果、写锁、草稿／preedit、删除与显式恢复，以及更新原侧不动当前文档。

Windows release 构建通过。独立 markers-fixture 原生截图位于 `artifacts/diff-fonts-20261019/`：`external-source.png` 显示 12 次外部连续写入后的最后版本和新增文件；`external-conflict-committed.png`／`external-reload-confirm.png` 展示保留草稿与确认；实际复制并断言草稿内容、取消后仍可复制；确认重载后磁盘内容显示正确；`external-deleted.png` 展示外部删除提示并断言文件没有被重新创建；`external-restored.png` 展示外部重建后自动恢复；`external-diff-after.png` 展示已打开 Diff 自动更新以及本地列表中的重命名文件。仅操作独立 fixture，不改用户仓库，不关闭用户 Rebased／IDE。连续滚动仍由用户手测；没有新增滚动自动化或声明像素级完全一致。

补充 Windows→WSL 原生验证：通过 `\\wsl.localhost\legion-wsl\tmp\plico-external-wsl-wwkm7c3_` 打开独立 Linux Git fixture，Linux Python 写入源码及新增文件后，`external-wsl-after.png` 显示 Windows 源码、行标记、项目树已同步，`external-wsl-local.png`／`external-wsl-diff.png` 验证本地列表与 Diff 可用。仅为该测试进程传入精确 fixture 路径的 safe.directory，没有更改宿主全局 Git 配置。该小 fixture 验证功能，不作为远程机器或大型 WSL 仓库性能结论。`working_copy::read_base` 同时补充动态 HEAD 解析：不能仅凭打开时的 A／untracked 状态永久使用空基线，外部首次提交后须读取新提交中的文件。

### 2026-10-19 源码变更标记随侧栏同帧定位

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版 Rebased 1.1.20 与快照不保证相同。本轮实读 `platform/diff-impl/src/com/intellij/openapi/vcs/ex/LineStatusMarkerRendererWithPopup.kt::LineStatusGutterMarkerRendererWithPopup.paint` 与 `platform/diff-impl/src/com/intellij/openapi/diff/LineStatusMarkerDrawUtil.java::paintChangedLines/getGutterArea`：标记作为 editor gutter renderer 绘制，绘制时取当前 gutter 区域；布局移动不依赖重新计算 Git ranges。

根因：本地 `SourceChanges` canvas 在 prepaint 生成绘制矩形及点击 hitbox，`source_gutter_right` 却读取到 paint 才更新的 input_bounds/last_layout，因此侧栏移动后横坐标落后一帧；没有后续重绘时会保持旧位置。这与 Git 查询或 80ms 文本变更合并无关。`vendor/gpui-base/src/input/base/{element,state}.rs` 将未水平滚动的 gutter 边界与已有 source_geometry 一起在编辑器 prepaint 发布；绘制与命中区域共用当前帧坐标，不增加定时器、额外重绘或 Git 查询。保留独立 GPUI canvas 的实现差异，不移植 Swing gutter 组件。

回归 `source_gutter_moves_with_sidebar_in_same_frame` 在真实 GPUI 编辑器布局后、paint 前采样标记实际使用的接口：修前收起 300px 仍得到 354.4px 而非 54.4px；修后收起、展开及宽度调整第一帧全部通过。35 项桌面测试通过，仍排除已记录的既有 `releasing_diff_clears_padding_from_retained_source` 基线失败。Windows release 构建通过；独立原生 fixture 截图 `artifacts/gutter-layout-20261019/source.png`、`collapsed.png` 确认侧栏两种布局下标记贴合正文左边，`marker-hit.png` 确认收起后在新位置点击可打开原文弹层。截图仅验证静态结果，同帧时序由回归测试验证；未执行滚动自动化，未改用户仓库或关闭用户窗口。未提交／推送。

## Project 右键与 Commit 刷新反馈（2026-10-20）

参考源码 commit：`4aec35256d765c217aa0d662b82ac4e860c396fd`，缓存 `/tmp/gitui-rebased-reference`。既有宿主记录为 Rebased 1.1.20，与此源码快照不视为同版本；本轮入口和行内状态的视觉依据为用户提供的三张截图。

本轮已读取：

- `platform/lang-impl/src/com/intellij/ide/actions/CopyPathProvider.kt` 的 `CopyAbsolutePathProvider`：复制所选实体的 presentableUrl。
- `platform/platform-impl/src/com/intellij/ide/actions/{CopyPathsAction,RevealFileAction,ShowFilePathAction}.java`：文件与目录均可定位；Explorer 打开父目录并选中目标，无父目录时打开根目录；Windows 优先使用 Shell API。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/{VcsDirtyScopeVfsListener.kt,UpdateRequestsQueue.java,ChangeListManagerRefreshHelper.kt}`：VFS 变更标记脏范围，合并更新请求，300ms 调度／去抖；同一时刻不启动重复更新。
- `platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/CommitChangesViewWithToolbarPanel.kt`：SingleTaskRunner 合并 UI 更新，100ms 延迟，后台准备模型后在 UI 线程替换并恢复树状态。
- 同目录 `ui/ChangesBrowserUnversionedFilesNode.kt`：更新状态追加在未版本化分组行内。`platform/platform-api/src/com/intellij/ui/treeStructure/Tree.java`：busy 图标不插入树上方布局行，普通树仅在有焦点时绘制。`ChangesViewManager.kt` 将刷新错误状态放在独立状态区。

本地映射：`src/project_view.rs` 仅提供“复制路径”“在文件浏览器中打开”两项；右键更新选择并取消待执行预览，菜单捕获点击时的完整路径，刷新重排不改变操作对象；复用 `ui_style::project_path_text` 保留大小写并隐藏 Windows verbatim 前缀，使用 GPUI 原生 clipboard／reveal_path，目录和根条目同样可用，分页占位行没有菜单。

`src/local_view.rs` 保留刷新前模型、选中和展开状态，删除树上方 loading 行；耗时超过 300ms 的刷新在既有分组行追加灰色 updating，首次空列表的提示使用固定高度工具栏，已加载的空态覆盖在固定树区域中。300ms 可见反馈延迟是本地抑制短查询闪烁的选择，不宣称是上游 Tree 的图标延迟常量。既有 single-flight／pending 刷新合并与写操作禁用继续保留。

`src/ui/worktree_sync.rs` 及时分发已有 300ms 合并后的文件事件，移除额外两秒门槛；100ms 检查待处理事件不运行 Git。两秒仅检查打开文档的磁盘 stamp；UNC、linked worktree、watcher 初始化失败仍保留 30 秒全量兜底。本地仍按事件执行一次有界 Git status 快照，不实现 IntelliJ 全套增量 VFS／ChangeListManager 缓存；refs 每五秒和远端每五分钟的既有观察不在本次范围。

回归：`refreshing_retains_tree_geometry` 在旧实现真实布局中失败（树顶 68px → 94px、高度减少 26px），修复后通过；慢刷新提示的延迟／结束取消、右键点击非选中目录后树重排仍复制原路径、已有刷新请求合并用例通过。完整桌面单元测试 38/39 通过；唯一失败 `editable_diff::lifecycle_tests::releasing_diff_clears_padding_from_retained_source` 在未修改的 HEAD 独立源码快照中也同样失败，未修改这一无关 Diff 行为。原生截图、操作结果与最终构建记录见 `artifacts/project-menu-refresh-20261020`。精确像素还原不作为本轮结论。

Windows 原生验收：独立本地 fixture 中，右键未选中目录后的剪贴板完整路径与 Explorer 选中路径均符合预期（clipboard.json、explorer.json）；受控 Git status 延迟 1200ms 时，changes-before/during/after.png 显示更新文字留在既有分组行，树顶和文件行位置保持不变；空闲 8 秒 status 次数 2→2，文件事件后 2→3（refresh.json）。测试进程与打包 EXE SHA-256 均为 `61a06f5511c208bac67fb5fd2154a848a9ba468c6f07ffacd5ef2af13a716986`。Windows 预览包为 `dist/Plico-windows-x64-preview-20261020-project-menu-refresh.zip`。

## 新增功能复审与刷新状态封装（2026-10-20）

复审范围 `dad3cb7...2a3371a`，参考仍为 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd`，已安装 1.1.20 不视为相同源码版本。本轮重新读取 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/UpdateRequestsQueue.java` 的 schedule／RefreshRunnable，以及同目录 `VcsDirtyScopeVfsListener.kt` 的受影响路径收集；另读取 `platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/FileDocumentManagerImpl.java` 的 afterVfsChange／contentsChanged：按变更文件查找缓存文档，保护未保存文档。提取规则是合并重复查询、按受影响文档分发变化，而非每个事件扫描全部打开文档。

发现并修复事件即时分发后的 IO 回归：`Documents::observe_disk` 原先即使路径不相关也会启动 stamp 读取，文件事件还会重置两秒检查计时。本地 `src/editing/external.rs` 改为命中事件路径或周期到期才检查；`src/worktree_events.rs::Schedule` 分别管理两秒文档检查与三十秒仓库兜底，普通文件事件不修改期限；`src/ui/worktree_sync.rs` 负责将两者接入同一事件循环。仍保留 GPUI／notify 轻量实现，不移植 IDE VFS；未测量异机 Windows→WSL 性能，不将该回归修复等同于全仓 Git status 提速。

`src/changes_refresh.rs::ChangesRefresh` 持有具体本地更改查询、取消令牌、合并请求和 300ms 延迟提示，向 LocalView 提供只读状态与结果事件。`src/local_view.rs` 保留树模型、提交范围、草稿、写操作及结果应用；控制器销毁取消查询，查询失败保留已加载列表。不把此模块推广成全局任务框架，300ms 可见反馈依然是本地抑制闪烁的选择。领域术语和架构评估分别记录于 CONTEXT.md、docs/ARCHITECTURE.md。

自动验证：新回归 `unrelated_events_do_not_schedule_open_document_io` 在修前失败、修后通过；独立时钟覆盖连续事件不推迟／加速兜底。控制器测试覆盖十次请求合并、300ms 延迟、失败保留加载状态、结束不残留提示和销毁取消；LocalView 测试通过生产订阅接线验证真实结果交付、旧列表保留与树区域尺寸不变。206 项库测试、2 项真实文件监听集成测试、38 项桌面测试通过；桌面测试仍排除上一轮已经在原始 HEAD 复现的 `releasing_diff_clears_padding_from_retained_source`，不宣称完整桌面测试全绿。

Windows release 在独立构建缓存中通过（旧缓存出现依赖 crate metadata 缺失，未改依赖规避）。原生独立 fixture 验证位于 `artifacts/refresh-architecture-20261020/`：`changes-during.png`／`changes-after.png` 确认 1200ms status 延迟时更新文字位于既有分组行，完成后消失，文件行位置不变。`refresh.json` 记录空闲八秒 Git status 次数 2→2，修改文件后 2→3。未操作用户仓库、未关闭用户窗口，未做滚动自动化；本轮未提交或推送。

## 提交反馈解释封装（2026-10-20）

参考 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd`，不等同于已安装 1.1.20。本轮实际重新读取 `plugins/git4idea/backend/src/checkin/GitRepositoryCommitter.kt` 全文，以及 `GitIndexInfoStagingAreaStateManager.kt::prepareStagingArea/restore`：提交失败才解释 GPG／空提交信号，不自动关闭签名重试；未选中暂存内容由 state manager 恢复，恢复异常上游记录 warn。本地仍沿用既有更严格的持久恢复记录与恢复失败状态，未修改 Git 命令、执行顺序或状态分类。

本地映射：`src/commit_feedback.rs` 将 `commit_operation::Report` 转成 UI 已有的标题键和详情，`src/local_view.rs` 只消费结果。修正旧先拼日志再截断导致恢复错误丢失的问题：恢复／记录／状态错误优先，进程输出随后；最多 32000 个 Unicode 字符（含截断标记），输入错误与两条输出流分别分隔，传输层 tail 已截断时也明确标记。执行前和显式恢复错误同样有界。不改提示入口、弹窗结构与翻译键，不复制上游通知／GPG 配置动作。

验证：3 项反馈单测覆盖 Unicode 截断、恢复优先级、记录清理与取消；33 项真实 Git 暂存／提交集成测试通过，其中成功、签名失败、空提交验证新的反馈 Interface，恢复失败用 70000 字符 post-commit 输出确认关键原因仍保留。38 项桌面测试通过，仍排除已记录的 Diff 生命周期基线失败。

Windows release 构建通过，原生启动截图在 `artifacts/commit-feedback-20261020/window.png`。本次自动化无法可靠获得前台焦点，键盘输入被保护检查拒绝，定向消息输入也未形成提交说明；未实际触发提交，故失败弹窗的原生交互验收仍待手工完成，不能把 `failed.png` 文件名当作通过证据。未改用户仓库或操作其窗口。长日志恢复原因保留已由上述真实 Git fixture 断言验证。

## Commit 面板 Rollback 与显示选项（2026-10-20）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；已安装 Rebased 1.1.20 仍不视为相同版本。视觉结构依据用户本轮两张 Commit 截图，实际读取：

- `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/actions/RollbackAction.java`、`RollbackFilesAction.kt`、`platform/util/resources/misc/registry.properties`：默认 `vcs.prefer.checkboxes.over.selection=false`，回滚初选来自树选择而非提交勾选；没有可用已跟踪选择则打开全部更改。动作先保存文档。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/RollbackChangesDialog.kt`：可勾选更改列表、数量信息、Delete locally added files，只有所选包含 NEW 才启用该选项；没有选中项时禁用确认。
- 同目录 `RollbackWorker.java`：不可取消的执行；完成（含失败）后更新工作树／列表；仅 Git 回滚无异常才按选项删除新增文件。
- `plugins/git4idea/backend/src/rollback/GitRollbackEnvironment.java`：NEW 执行 rm --cached -f，默认保留工作文件；MOVED 取消新路径跟踪、删除新路径并 checkout HEAD 恢复旧路径；MODIFICATION／DELETED checkout HEAD 同时恢复 index 与工作文件。
- `platform/vcs-impl/resources/META-INF/VcsActions.xml`、`platform/vcs-impl/shared/resources/intellij.platform.vcs.impl.shared.xml`：刷新、Rollback、Diff、显示选项、定位、展开／折叠的工具栏顺序；显示选项包含 Group By / Directory 和 Show / Ignored Files。
- `platform/vcs-impl/shared/src/com/intellij/platform/vcs/impl/shared/changes/{ChangesViewSettings.kt,ToggleShowIgnoredAction.kt}` 与 `platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/actions/ChangesGrouppingActions.kt`：默认仅仓库分组（单仓库对应平铺文件）、忽略文件默认隐藏；修改选项触发刷新并保存偏好。`ChangesGroupingSupport.kt`：Directory 开关独立于其他分组。

本地：`rollback_operation` 封装只读准备和确认后的写操作，采用 literal pathspec 与 NUL stdin，不执行全仓 reset/clean；确认前后复验 HEAD、相关 index 和文件内容指纹，拒绝过期确认。文件内容以固定 64KiB 缓冲读取，不保留正文；index 查询分块避免 Windows 命令行长度限制。`local_view/rollback.rs` 与 `rollback_dialog.rs` 管理保存门禁、确认、文档锁、失败提示及生命周期；复用既有 Changed 刷新，并立即通知打开文档／Diff。准备可取消，确认后的 Git 序列无取消按钮，失败可能已部分执行并刷新实际状态。

`local_view/options.rs`、`local_changes::Model`、`changes_refresh` 提供平铺／Directory 开关和可选 ignored 元数据查询；忽略目录用 ls-files --directory 表示，避免遍历产物，忽略条目不进入提交／Rollback／Diff。视图选项与删除新增文件偏好按项目记忆。工具栏动作靠左排列、增加 Rollback 与眼睛菜单，标题使用隐藏图标；平铺文件后显示父目录，提交勾选数放在 Amend 行，默认提交文本区高度按截图增加。图标 `hideToolWindow_dark.svg` 与 `locate_dark.svg` 来自上述 commit 的 `platform/icons/src/general/`，保留 Apache-2.0 来源。

保留差异：单仓库；未提供变更列表／Shelf／IDE Local History，未放置无行为的 Shelf 按钮；Rollback 对话框使用虚拟化平铺勾选列表而非 IDE 的多 changelist 树。无 IDE Local History Undo，因此确认框明确告知丢弃行为；删除新增文件偏好按项目而非 IDE 全局保存。暂不回滚进行中的 merge/rebase、冲突、目录／submodule 或已被重新创建旧路径的 rename，避免扩大删除范围；Windows 大小写专用 rename 尚未支持。最多 4096 条准备项，内容指纹用本次进程内散列侦测普通编辑，不宣称抵抗外部进程恶意竞争或跨文件事务原子性。精确像素间距与独立原生对话框未宣称完全一致。

验证：210 项核心单测、33 项真实 Git 提交／index 集成测试、7 项新增 rollback 集成测试通过；覆盖混合暂存、未选文件保护、新增保留／显式删除、unborn、rename/literal 路径、取消准备、文件／index／HEAD 过期、index.lock、符号链接边界。38 项桌面测试通过，仍排除此前已确认的 `releasing_diff_clears_padding_from_retained_source` 基线失败。Windows release 构建通过。

Windows 原生验收使用独立 `Plico-validation/rollback-20261020/fixture` 和独立 profile：确认平铺文件／父目录、工具栏／眼睛菜单结构、打开忽略项、全选／取消全选（空选择禁用 Rollback 和删除新增选项）、取消确认后 Git diff/status 不变；实际确认后 M/D/R 恢复，A 变为保留的 untracked，其他 untracked/ignored 保留，Changes 列表自动刷新。截图位于 `artifacts/rollback-20261020/{changes,menu,confirm-ready-final,none-final,cancel-final,rolled-back-final}.png`，原生文件断言见 `native-results.txt`。第一次执行脚本恰逢状态刷新，未打开确认框，断言正确失败；重开并确认后断言通过，未将首次脚本结果误记为通过。Directory 切换、快捷键、已打开编辑器内容同步与中英文布局的原生交互仍待验收；目录分组身份保持已由核心测试覆盖。未操作用户 Rebased/IDE 或用户项目；只关闭本轮自建测试进程。

## Log 刷新时跟随顶部（2026-10-20）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版 1.1.20 不视为同一源码。本轮实读 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/table/SelectionSnapshot.kt` 全文及 `VcsLogGraphTable.java::updateDataPack/getSelectionSnapshot`：更新模型前捕获位置和选择；`isOnTop && permanentGraphChanged` 时滚到第 0 行，其他情况按可见提交身份和 topGap 恢复；选择身份独立恢复，顶部跟随不意味着改选新提交。上游优先锚定可见选中提交，否则首个可见提交。

本地 `src/ui/repository.rs::refresh_impl` 原本无条件将旧首个可见提交滚回顶部，导致处于顶部时新增提交被移到视口上方。现在由 `src/ui/log_viewport.rs::restore` 在结果发布、旧列表替换之前读取当前位置：仓库首屏顶部且可见提交序列变化时保持第 0 行；向下浏览时按原首个可见提交恢复并保留行内像素偏移；仅引用标签变化不改变位置。读取时机从查询开始移到完成，避免使用异步等待前的旧位置。沿用原选择身份与详情刷新，不增加 Git 请求或自动 fetch。

保留差异：Plico 使用有界历史窗口；后续窗口的顶部不视为仓库顶部，锚点离开窗口时保持现有滚动句柄并由列表约束范围，不加载无限历史追踪。使用首个可见提交锚点，未照搬 Swing 的可见选中行优先；已有尚未布局的程序化行定位仍按行恢复。实际提交序列变化替代 IDE permanent graph 对象变化判断。没有把正在阅读旧历史的用户强制拉到顶部。

验证：先提取既有位置恢复逻辑运行 `cargo test --locked --features desktop-tests --bin plico ui::log_viewport::tests`，顶部新增提交用例失败（实际第 1 行，预期第 0 行），另两项旧历史／后续分页用例通过。修复后加上行内像素偏移、仅引用刷新用例，共 5 项通过；桌面测试合计 43 项通过，仍排除已记录的 `releasing_diff_clears_padding_from_retained_source` 基线失败。Windows release 构建通过。滚动手感与滚动期间刷新按项目约定留给用户手测，不做滚轮自动化。

Windows 原生验收：独立 80 条提交 fixture 与本地 bare remote，启动截图 `artifacts/log-viewport-20261020/window.png` 首行 History 79；向 bare remote 新增提交并执行 fetch，不发送任何滚动事件，自动刷新完成截图 `after-fetch-settled.png` 首行变为 Remote update visible at top，History 79 仍被选中，详情保留；`update.json` 记录新旧 hash。`after-fetch.png` 是查询中的旧快照，不作为完成证据。仅关闭自建 Plico 测试进程，未操作用户项目或 Rebased/IDE。

## 底部预览在本地变更与 Log 间切换（2026-10-20）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，不等同安装版 1.1.20。实读 `MainFrame.java` 的 selection listener／FrameDiffPreview 创建，`VcsLogTableCommitSelectionListener.kt`、`VcsLogCommitSelectionListenerForDiff.kt`、`VcsLogChangeProcessor.kt` 全文，以及 `VcsLogChangesBrowser.kt::createChangeProcessor`：Log 的预览 processor 属于 Log changes browser；提交选择更新详情及树模型，空选择清理；树模型变化触发 ON_MODEL_CHANGE，预览从该树的选中文件生成，不由 Commit 工具窗展开状态决定。

本地单个共享底部槽仍不同于上游各工具窗独立 preview，但来源不再由 `local_visible` 单独决定。`ui/preview.rs` 的来源判定／切换、`ui.rs` 的 viewer 创建与显示统一使用 `local_preview_requested`；显式打开 Commit 或 `LocalView::PreviewSelected`（点击文件、键盘导航、Diff 文件导航）请求本地来源。Log 行选择（包括同一行、右键、键盘和清空选择）、Log 文件选择切回 Log；保留 Commit 侧栏及其文件选择。后台列表刷新、输入草稿、提交勾选仍只发普通通知，不抢回来源。异步 Log 结果只更新数据，不改变来源意图。本轮不改变 Stash 独立面板行为，也不关闭已有 Diff 编辑器 Tab。

回归：抽取实际渲染使用的来源判定，旧行为在 `clicking_same_log_commit_reclaims_preview_without_closing_commit_sidebar` 失败；修复后同一测试经过真实 Review／LocalView 事件订阅，验证同一 Log 行切回、侧栏仍展开、普通通知不抢来源、再次显式本地选择切回、清空 Log 选择不暴露本地 Diff。桌面测试 44 项通过，仍排除已记录 Diff 生命周期基线失败。Windows release 构建通过。

Windows 原生验证使用独立两提交 fixture、独立 bottom-preview profile；`artifacts/preview-owner-20261020/log-same.png` 显示点击原来同一条 Log 提交后 BASE → COMMITTED，`local-again.png` 显示重新选择本地文件后 COMMITTED → LOCAL UNCOMMITTED，`log-other-settled.png` 显示选择另一条提交后 Empty tree → BASE。Commit 侧栏始终展开且本地选择保留。原生键盘与多选组合仍待手测；没有触发用户仓库写操作，关闭的仅自建测试进程。

### 2026-10-20 同行修改滚动时两侧错帧

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版 Rebased 1.1.20 与源码快照不保证一致。本轮重新实读 `platform/diff-impl/src/com/intellij/diff/tools/util/side/TwosideTextDiffViewer.java::MyVisibleAreaListener` 和 `tools/util/SyncScrollSupport.java::TwosideSyncScrollSupport.visibleAreaChanged/ScrollHelper.visibleAreaChanged/syncVerticalScroll/doScrollVertically`：可见区域事件内同步从侧，然后请求内容重绘；普通同步关闭从侧滚动动画，通过禁入区避免回传。曲线仍反映两侧实际显示位置，新增／删除、折行或文档边界导致的合理倾斜不应强行拉平。

根因与最小复现：两个 200 行、不折行、只在第 9 行替换文本、总行数不变的编辑器，主侧移动 20px 后首次 prepaint 的同一差异范围分别为 `(148,168)` 与 `(168,188)`。本地原先在 `InputBaseElement::paint` 末尾才 `publish_scroll_offset`，从侧收到事件后再排下一帧；上一轮仅验证“中栏读取的几何等于本帧各侧几何”，没有验证两侧是否已同步，因此漏掉此时序。

本地映射：`vendor/gpui-base/src/input/base/state.rs` 在滚轮事件内更新偏移后立即发布，在已有布局时的 `set_scroll_offset` 同样提前发布；保留初始布局及新尺寸校正的 paint 发布兜底和 peer 防回传。新滚轮输入清除尚未绘制的旧 deferred peer 请求，防止下一帧把新输入覆盖回去。未增加定时重绘、动画或 Git 查询，未更改差异行映射算法。保留差异：GPUI 的事件队列在绘制前分发，不移植 Swing 同步监听器；光标引起的布局内滚动及通用 scrollbar 直接修改 handle 的路径仍有 paint 兜底，本轮不宣称所有输入路径均已同帧化。

验证：`equal_line_changes_scroll_together_on_first_frame` 通过真实 EditableDiff 与每帧 prepaint probe 捕获所有中间帧，修前失败，修后左右作为主侧均通过；45 项桌面测试通过，继续明确跳过既有 `releasing_diff_clears_padding_from_retained_source` 基线失败。未合成鼠标滚轮事件做界面自动化；动态手感由用户手测。本次用户附件为 Commit 菜单，未将其作为 Diff 现象截图证据。

补充验收：独立 gpui-base 单元测试 `wheel_publishes_viewport_before_paint_and_supersedes_pending_peer` 通过，验证无编辑器绘制时也已发布事件、清除旧 peer 请求、随后 paint 发布不重复事件。Windows release 构建通过；独立 200 行仓库新开 Diff Tab 的原生截图 `artifacts/scroll-sync-20261020/tab.png` 确认第 9 行两侧及中栏连续对齐。静态截图不代表持续滚动已验收。测试进程关闭，未操作用户 Rebased／IDE。

### 2026-10-20 分支弹窗 Fetch 入口及加载状态

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 与此快照不保证相同。实读 `plugins/git4idea/backend/src/ui/branch/GitBranchPopupFetchAction.kt`：只在主分支弹窗显示，空闲使用 `AllIcons.Vcs.Fetch`；busy 时换 `AnimatedIcon.Default`、文案 fetching，保持动画工作但 actionPerformed 拒绝重复启动。`backend/src/actions/GitFetch.java` 后台 fetchAllRemotes，结束更新 action 并显示结果通知。`backend/src/fetch/GitFetchSupportImpl.kt` 用运行计数在 try/finally 发布开始／结束事件，逐仓库枚举全部 remotes；`GitFetchResult.kt` 定义成功／失败结果通知接口。`tests/git4idea/fetch/GitFetchTest.kt` 验证默认、指定、全部远端的引用更新。

本地：`src/ui/repository_popup.rs` 在搜索框右侧添加既有 Fetch SVG 按钮，调用 `start_fetch`，不关闭分支弹窗；复用 `Button::loading` 动画和禁止重复激活的实现。`src/active_operation.rs::Lifecycle::is_running` 从真实操作所有权读取状态，取消请求后仍显示加载，直到 worker 退出。允许 Fetch 期间重开弹窗观察状态，其他互斥操作期间仍禁用入口。执行复用现有认证、取消、全部远端查询和结果后的分支／Log 刷新，不新增 Git 执行链路。

保留差异：单窗口单仓库模型；沿用 Plico 既有“未配置远端”的执行后提示，没有为此按钮额外发起配置查询来提前禁用；加载动画使用 GPUI Spinner，未复制 Swing 动画资源。未添加尚无对应功能的设置齿轮。结构对齐截图中的搜索框右侧入口，未宣称像素级一致。

验证：45 项桌面测试通过（仍跳过既有 `releasing_diff_clears_padding_from_retained_source` 基线失败），4 项 operation 生命周期测试通过，含新增取消期间保持 Fetch 状态／禁止重复启动／完成恢复测试。Windows release 构建成功；仅在独立 fixture 与本地 bare remote 上用延迟 upload-pack 实测按钮，`artifacts/popup-fetch-20261020/{idle,loading,completed}.png` 分别记录空闲、加载及成功恢复，弹窗全程保留，状态栏显示 Fetch completed。认证及 Git 执行代码未改动。新入口及加载状态已原生核对，未宣称完整弹窗像素级还原。

### 2026-10-20 分支／Tag Checkout 入口缺口核对（分析，未修改行为）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 与该快照不保证相同。实读 `plugins/git4idea/backend/src/actions/ref/{GitCheckoutAction,GitSingleRefAction}.kt`：具体 Branch／Tag 均可 Checkout；单仓库当前本地分支、detached 状态下当前 Tag 不显示该动作；不能用目标 hash 等于 HEAD 代替当前引用判断。普通远端分支走专门的 `GitRemoteBranchesUtil.checkoutRemoteBranch`，不是 detached checkout。

继续实读 `backend/src/remote/hosting/GitRemoteBranchesUtil.kt::checkoutRemoteBranch/askNewBranchNameAndCheckout` 与 `backend/src/ui/branch/GitBranchCheckoutOperation.kt`：主菜单多仓库入口使用远端分支去掉 remote 后的名字建议本地分支名；同名本地分支跟踪冲突先要求新名字。已有本地分支独有提交时给出 checkout+rebase、保留本地分支 checkout、取消的选择（reset 模式则为丢弃本地提交）；无本地分支时创建并 checkout。另一单仓库工具入口还可优先使用已跟踪该远端的本地分支；不能混为主菜单始终复用任意跟踪分支的规则。

本地缺口：`src/ui/log_branches.rs` 的树右键菜单只提供 Log 过滤／定位，没有任何 Checkout 入口；`src/ui/repository_popup.rs` 的级联菜单使用 `!current && (local || tag)`，明确排除了远端分支，且入口为点击／右箭头展开而不是统一右键动作；`src/ui/log_history.rs` 的 Checkout 子菜单只枚举指向所选提交的本地分支，另有 detached revision 动作。`src/checkout.rs` 已有本地分支、提交号 checkout 及冲突恢复，但没有上述远端分支命名／跟踪冲突／独有提交流程。Tag 从顶部菜单走目标提交号，未按全名复验 Tag 是否移动；`repository_groups` 只标当前本地分支，没有 detached 当前 Tag 判断。

后续实现应统一具体引用的动作构造和目标解析（本地／远端／Tag），让顶部菜单、Log 分支树复用同一规则；远端必须增加完整准备与冲突选择流程，不能直接复用提交号 checkout 冒充功能对齐。本轮没有 Git 写操作、UI 改动或原生验收；以上是已读代码的行为分析，不宣称 Checkout 已修复。

### 2026-10-20 统一引用 Checkout 与远端分支决策

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版 Rebased 1.1.20 与该源码快照不保证一致。沿上节入口实读 `GitCheckoutAction.kt`、`GitSingleRefAction.kt`、`GitRemoteBranchesUtil.kt::checkoutRemoteBranch/askNewBranchNameAndCheckout`、`GitBranchActionsUtil.kt::hasTrackingConflicts`、`GitBranchCheckoutOperation.kt`、`GitNewBranchDialog.kt::GitNewBranchOptions` 以及 `GitRefUtil.java::getCurrentReference`、`GitCheckoutOperation.java`、`GitCheckoutNewBranchOperation.java`。补充校正上节分析：主分支菜单传入 `GitNewBranchOptions(name, true, true, false, repositories)`，第三参数为 reset，因此同名本地分支有独有提交时实际选择是 **Checkout and Rebase / Drop Local Commits / Cancel**，不是非 reset 模式的 Use Local Branch。

提取规则：当前本地分支按引用身份排除；同一 HEAD 上其他分支仍可切换。Tag 使用完整 ref 并进入 detached HEAD；当前 detached Tag 不重复显示动作。远端分支按去掉配置 remote 前缀的名字创建／切换本地分支；同名本地分支跟踪其他目标时要求新名字，未设置 upstream 本身不算冲突；有独有提交必须先显式选择变基或丢弃。主菜单不任意复用其他名字的跟踪分支。

本地 `src/ui/reference_checkout.rs` 统一目标解析及准备／命名／独有提交弹窗，顶部级联及右键、Log 引用树右键、提交 Checkout 子菜单共同使用；引用目标来自 snapshot 而不是当前有界提交列表。本地分支复用既有 checkout／Smart Checkout 流程。`src/reference_checkout.rs` 负责远端／Tag 准备、锁内复验及执行：校验 HEAD OID 和 symbolic identity、完整 ref（含 annotated tag 对象）、本地 tip 和 upstream；拒绝进行中的 Git 操作及过期确认。远端创建／替换绑定 upstream；只有明确选择 Drop Local Commits 才允许丢弃本地独有提交。执行后检查实际 HEAD、attached/detached 状态、变基后的祖先关系和冲突，退出码成功不足以判定成功。保存编辑文档后才写 Git，完成／失败／取消均刷新实际状态。

保留差异：Plico 单仓库，弹窗使用 GPUI 而非 Swing；同名冲突输入非法名字后当前以状态栏错误反馈，未复制 IDE 的内联名称校验。新增远端／Tag 路径遇到会覆盖未提交文件时保留 Git 拒绝和诊断，不自动 stash 或 force；既有本地分支 Smart Checkout 保持不变。远端变基冲突保留 Git rebase 状态，供已有继续／中止操作处理。尚未统一远端／Tag 的 Smart Checkout 弹窗，不能据此宣称完整 checkout 恢复体验完全一致。不复制 IDE 多仓库协调及所有分支操作菜单；无无效占位操作。

验证：11 项独立临时仓库集成测试通过，覆盖创建与跟踪、快进、独有提交显式决策、变基保留提交／冲突、命名跟踪冲突、过期 HEAD／ref／upstream、Tag detached／替换对象、取消前无写入、脏文件保护。46 项桌面测试通过（新增引用身份／历史窗口无关的菜单可用性测试），继续单独排除已记录的 `releasing_diff_clears_padding_from_retained_source` 基线失败。原生界面验收记录见后续补充。

Windows release 构建通过；独立 Windows fixture 与本地 bare remote 原生核对顶部远端 Checkout 级联、独有提交三选一弹窗、跟踪冲突新名字弹窗、Log 引用树 Tag 右键、Log 提交 Checkout 子菜单。实际完成 `review-other -> origin/other` 的创建／绑定、Tag detached 切换，并以宿主机 Git 断言；截图及断言保存在 `artifacts/reference-checkout-20261020/`（`remote-filter.png`、`local-commits-ready.png`、`naming-ready.png`、`name-completed.png`、`tag-menu.png`、`tag-completed.png`、`commit-checkout.png`、`native-results.txt`）。准备期间截图不是完成证据。验收中修复了准备结束后取消决策弹窗留下“正在检查”文案的问题。顶部右键与键盘组合、中英文弹窗布局仍待完整原生验收；未宣称逐像素复刻。仅操作独立 fixture、关闭自建进程，未操作用户项目或 Rebased／IDE。

补充原生断言：从 Log 提交 Checkout 子菜单切换到与当前 Tag 相同 OID 的 `same-head` 本地分支，宿主机 `symbolic-ref HEAD` 确认为 `refs/heads/same-head`；截图 `same-head-completed.png`。证明入口按引用身份判断，不因目标等于 HEAD 而遗漏本地分支。最终测试进程已关闭。

### 2026-10-20 Stash 归属 Commit 页签及记录右键操作

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 与源码快照不保证一致。实读 `plugins/git4idea/backend/resources/intellij.vcs.git.backend.xml` 的 `git.stash` 注册、`Git.Stash.Operations.ContextMenu`；前者明确 `isInCommitToolWindow="true"`。`ChangesViewContentManager.kt::getToolWindowId` 按 Commit 模式将内容映射到 Commit 工具窗口，而不是单独的 Stash 工具窗口。`GitStashContentProvider.kt` 将 Stash 注册为内容 Tab，并可配置与 Shelf 合并；`GitStashProvider.kt` 按保存记录身份提供单选操作；`SavedPatchesUi.kt` 在记录树安装右键菜单，在 changes browser 底部放置恢复动作。因此此前只参考 SavedPatches 内部 splitter 便创建独立左栏入口不充分，本轮纠正该结构。

本地 `ui/chrome.rs` 将 Commit／Stash 页签及隐藏按钮置于共同标题栏；`ui.rs` 去掉独立 Stash 工具栏按钮，Commit 图标在两个页签下均保持选中。`ui/editor_tabs.rs` 复用现有 LocalView／StashView 实体，切换不丢提交草稿、文件选择、记录选择；同一窗口隐藏／切换到 Project 后，Commit 入口恢复最后选择的页签。显式 Local Changes 动作仍进入 Commit 页签，恢复提示的 Stash 动作仍进入 Stash 页签。现有底部预览按活动来源切换；LocalView 不再重复绘制自己的标题栏，Stash 刷新放在页签内部工具栏。

实读 `stash/ui/GitStashActions.kt`：单条选择才启用 Apply／Pop／Drop；普通 Apply／Pop 均不恢复 index，Drop 需要确认。`stash/GitStashUtils.kt::dropStashWithConfirmation` 在确认中展示 selector 和 message，然后执行 `stash drop selector`；`tests/git4idea/stash/GitStashTest.kt` 验证记录 selector、分支、消息、日期；不是上游 Drop 写操作测试。本地 `stash_view.rs` 增加记录右键 Pop／Apply／Drop；右击先选中目标，异步加载文件内容不阻塞对记录本身的操作。Drop 由 `ui/saved_changes.rs` 展示确认并通过既有操作生命周期执行；`stash_operation.rs::drop_stash` 在仓库锁内复验 selector 对应的 OID，序号移动时拒绝删除。删除不改工作区／index，也不把删除保存数据当成已经恢复 Squash 更改，不清除 Squash recovery ownership。取消确认无 Git 写操作，执行后刷新记录和选择回退。

保留差异与范围：仅 Git Stash，尚未实现 Shelf，不将两者混称；Stash 页签允许空列表时进入，而不完全照搬 IDE 可见性 registry。页签选择仅在当前项目窗口生命周期保留，未跨启动持久化。此轮补齐工具窗口归属及基础记录菜单，并非完整 Stash 功能对齐：Unstash As（恢复 index／新分支）、Clear All、与本地比较开关，以及创建 Stash 的完整界面仍需分别对照实现；未放置无行为菜单项。精确像素样式与 IDE 独立原生对话框不宣称一致。

验证：新增 3 项临时仓库 Drop 测试通过，覆盖只删除确认记录、HEAD／index／工作区不变、序号漂移拒绝、取消与非法身份无写入；既有 Apply／Pop 的多场景集成回归通过。47 项桌面测试通过（仍单独排除已记录的 Diff 生命周期基线失败），其中新增真实 Review 页签测试验证实体复用、隐藏后恢复 Stash、Project 切换后恢复页签。Windows release 构建通过。原生验证中确认草稿跨 Commit／Stash 切换保留，并发现首次右键切换记录时 loading 提示挤动列表导致菜单丢失；已将 loading 放入固定高度工具栏，避免鼠标按下与抬起之间改变行位置。

原生验收：独立 `Plico-validation/stash-tabs-20261020/fixture`，`artifacts/stash-tabs-20261020/{stash,draft-retained}.png` 确认页签结构、底部预览及草稿保留；最终构建 `final/context.png` 确认首次右键选中另一个 stash，内容加载中菜单仍出现且记录位置稳定；`final/confirm.png` 显示正确的 `stash@{1}` 和消息。`finish.ps1` 断言取消后完整 stack 不变，确认后只移除旧记录，HEAD／index／工作区不变；`final/dropped.png`、`final/reopened.png` 确认刷新后回退到剩余记录及隐藏重开仍为 Stash。结果见 `native-results.txt`。仅关闭自建测试进程；未操作用户仓库或 Rebased/IDE。键盘菜单、中文长文案和恢复冲突下页签交互仍待原生专项验收；未重新发布安装包。

## 2026-10-20 编辑器变更标记的单块提交

参考 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd`；宿主机 1.1.20 不保证与该源码快照一致。本轮重新实际读取：

- `platform/vcs-impl/src/com/intellij/openapi/vcs/ex/commit/CommitChunkComponent.kt` 的 Panel、Workflow、Handler、Component、Spec 和 ChunkCommitMessagePolicy：标记弹层内输入提交信息，空消息禁止执行、Ctrl+Enter 提交；只包含当前 range；执行时关闭弹层，成功清理消息，关闭弹层保留临时消息。上游还支持 inline Amend 和自适应输入高度。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/ex/PartialLocalLineStatusTracker.kt::handlePartialCommit/applyPartialCommitChanges`：从基线合成选中范围内容，成功后更新 tracker 的 VCS 基线，不把未选工作区改动撤掉。
- `plugins/git4idea/backend/src/checkin/GitCheckinEnvironment.kt::commitRepository/addPartialChangesToIndex/stageAndCommit/convertDocumentContentToBytesWithBOM`：先把部分文件内容写入 index，普通 add 跳过已经处理的路径；处理换行与 BOM、保留 executable 模式；未选文件的 index 另行保护。选中的同一文件原 index 不属于 excluded-index 恢复范围，提交失败可保留已准备的选中块 index。
- `plugins/git4idea/backend/src/index/GitIndexUtil.java::hashObject/updateIndex`：`hash-object -w --stdin --path` 应用 Git filter，再写指定 mode/OID/path 到 index。
- `plugins/git4idea/tests/git4idea/tests/GitPartialCommitTest.kt` 全文：指定 changelist／排除 range、多列表组合、保留未选文件、autocrlf 开／关。命名列表本轮不实现，相关组合测试不宣称已覆盖。

本地映射：`partial_commit::Chunk` 保存路径、基线 blob、当前文档快照、range 和消息；`prepare` 在现有提交 operation guard 内验证基线、保存后的正文、精确 range，合成目标 blob（保留当前文件换行/BOM），执行 clean filter 后再次验证磁盘内容。`commit_operation` 重用原有 prepare／commit／restore 与持久恢复记录，跳过选中文件的普通 add，不写工作区。没有临时 checkout/reset，不调用外部 diff 或 textconv；新 blob 在失败时可能不可达，由 Git GC 管理。失败后的选中文件 index 语义与上游一致，不声称恢复该文件原暂存版本。

`source_changes` 标记弹层加入输入和上游 CommitInline 图标、Ctrl+Enter；消息保留至成功。`project_view` 只转发显式事件，`ui/operation` 通过既有保存门禁和文档锁运行，成功／失败均同步变更、文档与 Log。只提交此块，不使用 Commit 面板勾选范围或清理其独立草稿。

阶段边界：首个入口是已跟踪普通文本文件的单块提交；尚未提供 Diff 多块勾选／块内选行、inline Amend、命名列表归属、未跟踪文件及 rename 旧路径基线。未引入 Shelf。临时消息仅当前源文件视图内保留，尚无跨重启消息历史；输入栏固定单行高度，与上游自适应布局保留差异，未宣称像素一致。沿用源码编辑器 5 MiB 正文预算，不进行全仓正文预加载。

验证：6 项真实 Git 集成测试通过，覆盖分离改动、其他文件 staged/unstaged 保留、同文件已有 staged 内容、增删／EOF、Unicode 与 literal 路径、BOM／CRLF／autocrlf、clean filter／executable、正文／基线／范围过期、预取消和提交拒绝。33 项现有提交／index 集成测试通过。3 项 source_changes 桌面测试通过，验证空消息／锁定禁止触发、所选范围准确、提交事件不修改文档或清理失败前草稿，以及真实编辑器标记点击和输入框聚焦后的 Ctrl+Enter。Windows release 构建通过；原生交互验收见后续记录。

Windows 原生验收：独立 `Plico-validation/chunk-commit-20261020/fixture` 中，通过鼠标提交修改块、Ctrl+Enter 提交删除块；宿主机 Git 断言每次只提交所选块，首块修改保持未提交，另一文件原 staged／unstaged 内容均保留。验收发现 textarea 的 Enter 处理吞掉 Ctrl+Enter，已用弹层 capture_action 处理 secondary Enter，并以失败后通过的焦点输入回归测试验证。成功后消息清空、只剩未提交标记、Log 自动出现两个新提交；截图 `artifacts/chunk-commit-20261020/{final-draft-cleared,final-log}.png`，断言 `native-results.txt`。Windows release 构建通过，测试进程已关闭；未操作用户仓库。新增块由 Git 集成测试覆盖，中文长消息、自适应输入和 inline Amend 尚未原生验收／实现；不宣称完整部分提交或像素级对齐。

## 2026-10-20 Commit Diff 多块／部分行选择

参考仍为 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 1.1.20 与此快照分别记录。本轮重新实读 `SimpleLocalChangeListDiffViewer.kt` 全文、`LocalTrackerDiffUtil.kt` 的 `toggleBlockExclusion`／`toggleLinePartialExclusion`／`PartiallyExcludeSelectedLinesFromCommitAction`／`ExcludeAllCheckboxPanel`，`PartialLocalLineStatusTracker.kt` 的 `setPartiallyExcludedFromCommit`／`handlePartialCommit`，以及 `DocumentTracker.kt::getContentWithPartiallyAppliedBlocks`、`RangeExclusionState.Partial` 和 `DiffUtil.java::applyModification`。提交端重新读取 `GitCheckinEnvironment.kt::commitRepository/addPartialChangesToIndex/stageAndCommit`。测试参考实读 `PartiallyExcludedChangesTest.kt` 的 inclusion 缓存、无 tracker／全选／部分选择场景，`PartialLineStatusTrackerTest.kt::testPartialCommit1/2/3`（多块排除、提交后工作文档不变和基线更新）。未将未读测试视为已验证。

规则：普通块在新侧有整体 checkbox；拆分后旧侧删除行和新侧新增行各有独立 mask。部分行操作将整块拆分时，对侧初始排除；不能按左右行号配对提交。选择覆盖完整块时使用整块包含／排除，只有覆盖部分块才进入逐行操作。原内容保留未选删除行，再在块尾插入所选新增行，段之间补必要换行。文件／组使用三态并与 tracker 同步；浏览选中与提交 inclusion 分开。部分路径先写合成 blob，普通 add 必须跳过这些路径，但仍处理同次选择的其他整文件。

本地：`partial_selection.rs` 实现整块／独立增删行状态和内容合成；`commit_scope.rs` 是 Commit 树与 Diff 共享的 GPUI entity；`editable_diff/partial.rs` 提供并排模式中栏 checkbox、文件总 checkbox 与两侧“包含／排除所选行”菜单；拆分后逐行 checkbox 可直接切换。`local_view` 的整文件／目录勾选会清除该路径 partial 状态；`diff_tab` 仅对 Commit 的 HEAD—Working tree 普通修改路径启用，不对历史、Stash、index 比较启用。独立 Tab、底部预览和文件导航共享 scope。`commit_operation::execute_partial` 复用现有 operation guard、保存门禁和 index 恢复，支持多个部分文件与整文件混合。选择计算随比较任务生成，不在每帧重新执行行 diff。

边界与保留差异：基线或正文改变时不猜测重定位旧 mask；活动视图清除该路径部分选择并排除整文件，关闭视图中保留的快照在提交前复验失败，不静默扩大为整文件。上游 tracker 有更细的编辑映射，本地采用保守失效。当前新增细粒度入口为并排模式；Unified 保留共享选择及文件总开关，尚无逐块／逐行控件。rename、全文件新增／删除继续走整文件提交，不伪装为行选择。当前未复制鼠标悬停才显示的临时单行 checkbox、排除区域的 gutter 淡化／部分块拆色、菜单动态文案与快捷键；整文件开关位于正文上方工具行，尚未并入版本标题。精确 checkbox 样式和位置待视觉细化，不宣称完整 Rebased 像素还原。命名列表与跨启动选择持久化不在此增量。

验证：部分提交 Git 集成测试扩至 8 项，通过多块＋部分行＋整文件混合、未选 index／工作区保留、过期正文和非法路径拒绝；既有 33 项提交 index 测试通过。2 项选择模型测试覆盖拆分初始化、增删、空内容和无末尾换行；1 项 GPUI scope 测试覆盖共享三态和过期选择不扩大范围。Windows release 构建通过。独立 `Plico-validation/multi-commit-20261020/fixture` 原生验证两个整块与两侧部分行同时勾选、三态联动、右键菜单、逐行切换和真实提交；宿主机 Git 断言只提交所选内容、工作区不变、其他文件 staged/unstaged 均保留。截图与断言见 `artifacts/multi-commit-20261020/{blocks-selected,line-menu,line-included,selected-lines-and-blocks,committed}.png` 和 `native-results.txt`。不操作用户仓库；未发布新包。

收尾回归：50 项桌面测试通过，仍只排除已记录的 `releasing_diff_clears_padding_from_retained_source` 基线失败。补充 scope 测试验证最后一行取消后，文件不进入提交但逐行控件状态保留。最终 Windows 构建再次打开独立 fixture，`final/arrows.png` 确认普通块回退箭头与提交 checkbox 共存，只有部分行块隐藏整块回退箭头；已关闭自建进程。原生完整提交流程已在此前同一增量构建验证，收尾改动是上述控件／空选择状态，不把截图当作滚动交互验证。

## 2026-10-21 Windows 操作延迟排查

参考源码仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，与已安装 Rebased 1.1.20 不保证一致。本轮实际读取：
- `platform/vcs-log/impl/src/com/intellij/vcs/log/data/AbstractDataGetter.kt`：全部命中时直接交付；未命中按 repository 分组在 Backgroundable task 中查询，保留取消和完成通知。
- `CommitDetailsGetter.kt`：Caffeine 上限 10,000，低内存和 dispose 时清理；详情缓存独立于 placeholder。
- `plugins/git4idea/backend/src/log/GitLogProvider.kt::readFullDetails/readMetadata`：metadata 与完整变更数据分层，完整数据经 GitLogUtil 批量读。
- `plugins/git4idea/backend/src/reset/GitResetAction.java`、`GitResetOperation.java::execute`：对话框使用已取得的详情，确认后进入后台任务，执行完刷新仓库/VFS/dirty scope。并不在 UI 线程直接执行 reset。
- `plugins/git4idea/backend/src/status/GitChangesCollector.java::collect/collectChanges`：使用已有 repository HEAD 与 status；仅 MM 等必要情况计算净差异。
- `platform/platform-api/src/com/intellij/openapi/ui/LoadingDecorator.kt`：加载生命周期驱动动画，短任务可延迟显示，结束移除加载层。

反馈循环：新增只读 `examples/interaction_profile.rs`，直接调用选择提交使用的 `Repository::details`、`commit_changes::Snapshot::load`、`reset_operation::Prepared::prepare`，包含 Git 子进程时间，不包括 UI/GPU 绘制。不执行 reset、不修改被测仓库。Windows release 在用户确认的 `C:\code\spring-framework` 实测 5 次：详情 283–337 ms，本地刷新 494–595 ms，Reset 准备 1550–1753 ms；独立小 fixture 同样约 285–391 / 579–665 / 1609–1744 ms，排除仅大仓库规模导致。以每路径 100 ms 为探测预算运行返回 1；此预算仅定位可感知等待，不是跨设备 CI 门槛。

逐阶段探针确认 Windows `ProcessTree::spawn` 的系统线程 snapshot 约 8–9 ms，逐项枚举约 43–45 ms，每个 Git 命令额外付出约 52 ms。不是 Git 本身在计算这些时间。`src/process_tree.rs` 改为 PSS_CAPTURE_THREADS，仅采集本次仍暂停的 Git 进程线程，不抓取地址空间/句柄/context；仍保持 CREATE_SUSPENDED → AssignProcessToJobObject → ResumeThread，失败由 Drop 杀掉未启动的子进程。snapshot 和 walk marker 均 RAII 释放。官方 API 依据：[PssCaptureSnapshot](https://learn.microsoft.com/en-us/windows/win32/api/processsnapshot/nf-processsnapshot-psscapturesnapshot)、[PssFreeSnapshot](https://learn.microsoft.com/en-us/windows/win32/api/processsnapshot/nf-processsnapshot-pssfreesnapshot)（当前进程持有的 snapshot 用 GetCurrentProcess 释放）。Windows 8.1+ 可用，满足现有 Windows 10/11 应用范围。未使用未文档化 Nt API、未取消进程树保护。

单独此改动后的同机同仓库 5 次：spawn 准备 2–3 ms，详情 144–165 ms，刷新 292–311 ms，Reset 准备 804–850 ms。原始证据 `artifacts/interaction-perf-20261021/{baseline,stages,process-fix}.txt`。100 ms 探测预算仍未全部达到，不能宣称所有操作瞬时完成。

本地映射与保留差异：
- `src/detail_cache.rs` + `ui/log_selection.rs`：窗口级 LRU，32 条/估算 2 MiB 双上限；repository、OID、parents、shallow 状态均纳入身份；只缓存成功的详情，不缓存可变 refs/所属分支/写操作验证。命中同步恢复面板，未命中保留 generation 与 Cancel。`ui/repository.rs` 切换仓库或非分页历史刷新即清空，比上游更保守，避免浅克隆边界、replace/config 变化后沿用旧结果。低内存目标下不复制上游 10,000 条缓存。
- `ui.rs` 右下角为正在执行的操作/历史或详情加载增加 Spinner；`local_view.rs` 刷新按钮沿用既有 300 ms feedback 延迟显示 loading。没有伪造百分比、没有更改取消/Reset 不可中断语义。
- 尚有优化空间：选择提交会额外启动 Reword eligibility 和 containing-branches 查询；Reset 准备/复验存在重复 metadata 查询；历史刷新仍重新建图。此次未放松写前验证，也未用缓存替代仓库真实状态；没有宣称已完整复刻上游批量预取/增量 dirty scope。

继续测量发现 Log 刷新还有独立瓶颈：在进程启动修复后的 spring-framework 上，`operation_state + history::read(1000) + graph_print` 仍为 3953 ms。Git Trace2 证实该刷新调用了 37 次 `log`（1 次列表＋36 次 HEAD 拓扑分页）；显示窗口包含未合入 HEAD 的提交时，原 `contained_in_head` 必须扫完整个历史且每页重新 `--skip`。此时不只是进度提示缺失。

进一步实际读取同参考 commit 的 `ContainingBranchesGetter.kt`（分支 checksum 失效、限量 LIFO 后台任务与图查询）以及 `CurrentBranchConditionCache.kt`（单线程后台构建 permanent graph 的 membership predicate，返回旧 condition，变动时置 outdated）。本地没有全量 permanent graph，采用轻量差异实现：`git.rs::contained_in_head` 一次 `rev-list <固定 HEAD> --`；`head_membership.rs` 流式处理，只保留当前窗口匹配 OID，单行最长 66 字节，输出规模不随整仓历史进入内存；沿用进程超时/取消。没有为了性能取消未合入分支的正确性，没有使用日期截止或只扫前 1000 条的近似判断。查询依然作为快照的一部分完成，而不是上游的异步旧 condition，避免高亮短暂显示旧 HEAD。

最终验证记录：
- 一次全量 `cargo test --locked --no-default-features` 491 项通过；HEAD 流式改动后补跑 lib 215 项、repository 112 项通过。桌面 50 项通过，仍显式排除原有 `releasing_diff_clears_padding_from_retained_source` 已知失败。
- Windows 原生 process_tree 5 项（含 32 次启动后的 snapshot/job/process 句柄不累积、父进程先退出后的后代清理、不误杀无关进程）、operation_process 7 项、reset_operation 5 项通过；HEAD membership 原生真实 Git 合并/未合并/detached 测试通过。所有 Reset 写操作只在自动生成的临时 fixture。
- Windows release 构建通过。独立 profile 的原生 Plico 打开 spring-framework，检查选择提交、返回已查看提交及加载反馈；`artifacts/interaction-perf-20261021/final-details-busy.png` 可见右下角 Spinner 和文件面板 Reading Git data。截图只核对状态/结构，不作为 GPU 延迟的精确计时。既有 Rebased/IDE 进程未关闭，用户仓库未 reset/修改。刷新按钮动画、各种 DPI 与操作模式的完整原生矩阵仍待手测，不宣称像素级完成。
- 最终 Trace2 为 1 次 log + 1 次 rev-list；完整 1000 条刷新 1343 ms，初步对照 3953 ms。该轮宿主机普通 git --version 本身由约 43–47 ms 波动到 66–75 ms，因此不混用各轮极值宣传固定倍数。详情缓存 1000 次查询共 43 µs（纯查找，不是端到端 UI 时间）。原始阶段探针已移除，保留明确标识的只读 profiling example，不将硬件耗时断言放入 CI。
- 流式实现每次遍历一次 HEAD；对单分支超长历史仍有遍历成本，没有上游全量图常驻缓存。这是低内存权衡，后续应单独测超长线性历史，而非宣称所有规模都无延迟。

### 后续：Reset 单次校验内的重复查询

前一轮优化已提交并推送为 `842c526`。继续使用同一只读 profiling example，未修改用户的 spring-framework。重新读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/reset/GitResetAction.java`、`GitResetOperation.java` 全文：上游使用 repository 对象和已取得详情，确认后后台执行，保存文档并刷新 VFS/dirty scope。安装版仍为 1.1.20，不保证对应此快照。

本地发现 `reset_operation::Prepared::check` 已读取并比较 directory、HEAD、symbolic HEAD，紧接着 `Repository::operation_state` 又查询同一 directory 和 symbolic HEAD。现在提取 crate 内部 `git.rs::operation_state_at`，仅复用本次 check 刚查询的数据；每次 prepare 与 execute 仍各自重新查询身份。markers、unmerged index 和 pending recovery 仍现场读取；不缓存弹窗授权，不跳过 operation guard、目标合法性、HEAD/branch 复验、取消边界和执行后状态刷新。公开 `operation_state` 入口仍独立查询全部状态。保留本地比上游更显式的写前检查，不借提速变更模式或丢弃检查。

功能保障测试在改动前后均通过，新增覆盖弹窗打开后 linked worktree 出现 Merge、Cherry-pick、Rebase、pending recovery，以及没有 operation marker 但 index 出现 unmerged stages 的情况；断言 Hard reset 被拒绝，工作区内容、HEAD、冲突 index 保留。原模式、detached、HEAD/分支改变、锁、取消测试继续保留。

Windows release profiler 顺序执行 before/after，每种 5 次，并开启相同 Trace2；该轮未并行运行测试。Reset 准备 1043–1121 ms（中位 1069）→ 867–897 ms（中位 883），约减少 17%。整个 harness 的 Git 启动数 103→93：5 次准备各少一次 `rev-parse --absolute-git-dir` 和一次 `symbolic-ref`，其他命令数不变。普通 Git 启动约 67–74 ms，不能与先前不带 Trace2、不同宿主负载的极值混算。此处是后端准备时间，不含用户确认或实际 reset 写盘。证据 `artifacts/interaction-perf-20261021/reset-{before,after,query-counts}.txt`。

验证：Linux repository 112 项与 reset 8 项通过；Windows 原生 reset 7 项通过（另一个测试为 Unix smudge fixture），Windows release profiling 构建通过。没有 UI 布局/交互变更，本轮未新做窗口截图；未发布包。

进一步源码对照实际读取 `GitRewordAction.kt` 和 `rebase/log/GitCommitEditingActionBase.kt`：上游 update 使用 cachedMetadata、repository state、quick containing-branches 判定，未知信息在 actionPerformed 时严格复验。当前 `ui/reword.rs::Gate::new` 每次选择立即启动完整 `Prepared::prepare`，属于下一处候选；本轮不修改，尚未量化其与详情查询的竞争。后续需拆分 availability 与写前准备并保留受保护分支、HEAD/merge/rebase 禁用规则，不能只取消后台检查或复用过期写操作授权。

### 后续：历史编辑的保护分支查询

Reset 优化已推送为 `cde05ff`。本轮继续对照相同参考 commit，实际读取 `plugins/git4idea/backend/src/GitProtectedBranches.kt` 全文及 `rebase/log/GitCommitEditingActionBase.kt::checkCommitsCanBeEdited`：保护策略作用于真实 remote branch 的远端分支名，然后判断提交是否被该远端分支包含；不需要 Pull 的当前分支/upstream 目标选择。上游快路径使用已有 graph/repository 数据，执行时再严格检查。已安装 1.1.20 与源码快照仍分别记录。

新增只读 `examples/reword_profile.rs`，调用 Log 选中时当前真实使用的 `reword::Prepared::prepare`，每次以 eligible/protected 记录结果；其他错误直接失败。100 ms 探测预算在 spring-framework 上返回 1，5 次 869–944 ms。这是后台检查自身耗时，未证明它使详情加载串行等待，也不等同于端到端 UI 延迟。

先缩小查询而不改变 eligibility 生命周期：`squash_check::protected_containing` 原来获取完整 Pull inventory 后，再取全部本地/远端 containing branches；现在通过 `git.rs::remote_branches_containing` 使用 `git remote` 与受限 `for-each-ref --contains <OID> refs/remotes/`。每次仍现场读取，无缓存写操作授权；保留已配置远端筛选、既有 remote prefix 解析、跳过 symbolic refs、默认 main/master 与 explicit targets 顺序、取消/超时和 1 MiB ancestry 输出上限。HEAD 归属、merge/rebase、恢复记录、执行前及发布前复验仍由原调用链处理。Pull/Push inventory 与展示用 containing branches 未改。Reword 和 Squash 共用这项优化。

同时修复原字符串交集的身份歧义：本地 `refs/heads/team/nested/main` 即便包含目标，也不能替未包含目标的 `refs/remotes/team/nested/main` 证明已发布。新集成测试在原实现上失败，改动后通过；覆盖带 `/` 的配置远端、symbolic master 别名、真实保护分支命中，以及 prepare 后新增保护包含关系时 execute 仍拒绝且 HEAD/消息不变。既有 Squash 测试继续覆盖默认/自定义保护、未保护远端、保护祖先与 detached。

Windows release 同仓库、同 Trace2 设置顺序执行各 5 次，结果都为 protected：准备耗时中位数 1258→971 ms，约减少 23%；Git 命令总数 66→51，每次准备 13→10（额外 1 次是 harness 取 HEAD）。只减少此检查的查询，不声称整体点击延迟下降同等比例。证据 `artifacts/interaction-perf-20261021/reword-{before,after,query-counts}.txt`。Linux repository 112 项、reword 8 项通过，补充晚发布断言单独复跑通过。Windows release profiler 构建通过。本轮无 UI 布局变更；未新做窗口截图，未发布包。

保留的优化空间：选择时仍运行完整 Reword prepare，且与详情里的 containing-branches 查询独立；尚未拆分快速 availability 或引入 refs 代际共享缓存。本次通过缩小必要查询获得收益，不以省略检查、更改禁用条件或持久缓存可变 refs 换速度。

Windows 原生收尾：reword 7 项通过（另 1 项是 Unix hook fixture），Squash 相关 8 项通过；涵盖真实临时仓库的消息改写、作者/树/index/工作区保留、merge、rebase、取消、晚发布拦截和 Squash 过期确认/恢复边界。记录 `artifacts/interaction-perf-20261021/reword-native.txt`。未对用户仓库执行 Git 写操作。

## 已实现功能横向检查与 Commit Diff 对齐（2026-10-21）

完整覆盖表、证据等级、保留差异与优先级见 [REBASED-ALIGNMENT.md](REBASED-ALIGNMENT.md)。参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，现场安装 Rebased 1.1.20；本轮不声称二者是同一版本，也不声称所有功能像素／边界已穷举验收。

本轮实际读取 `SimpleLocalChangeListDiffViewer.kt::createTitles/getStatusTextMessage`、`LocalTrackerDiffUtil.kt::ExcludeAllCheckboxPanel/PartiallyExcludeSelectedLinesFromCommitAction/getStatusText`、`DocumentTracker.kt::RangeExclusionState.countAffectedVisibleChanges` 和 Islands gutter checkbox SVG。规则与完整路径见横向检查文档。本地 `editable_diff/partial.rs`、`partial_selection.rs`、`diff_tab.rs`：文件总勾选移入右侧版本标题，删除额外的 25px 工具行；普通块按一处、拆分块按增删行显示差异数，全选时不追加 included 数；总勾选、块／行勾选与提交树继续共享 scope，父视图订阅选择变化。中栏采用上游中性 14px SVG，保留版权；右键包含／排除根据选区实际改变状态决定是否可用，无变更选区两项禁用。提交内容合成计数不受显示计数影响。没有为显示计数在每次绘制重新扫描完整 Diff。

另实际读取 `ChangesBrowserFileNode.java`、`ChangesBrowserChangeNode.java::render`：文件名在前、父路径在后附加为同一行文本。本地 `local_view.rs` 改为单个带灰色后缀的 StyledText，修复窄侧栏父路径挤压短文件名；长路径仍截断。没有改变文件身份、勾选范围与排序。

验证：

- `cargo test --locked --lib partial_selection`：4 项通过，新增整块／拆分显示计数、普通文本不可操作、选中部分／整块及空侧菜单状态覆盖。
- `cargo test --locked --test partial_commit`：8 项通过，保留混合部分／整文件提交、index／工作区保留、换行符／filter、过期选择拒绝。
- 既有 50 项 desktop-tests 回归通过；仍明确排除此前已记录的 `releasing_diff_clears_padding_from_retained_source` 基线失败。新增 `title_checkbox_click_updates_shared_scope_and_toggles_back` 真实点击事件测试单独通过，验证标题点击更新共享 scope 且可再次取消。
- `cargo check --locked --bin plico`、Windows release 构建通过，`git diff --check` 通过。
- 自建 Windows Plico 使用独立 profile，只读打开 Rebased 当前的 `arrow-boundary-fixture`；本轮只切视图／提交 inclusion，不修改文件、不写 Git。`artifacts/alignment-20261021/rebased.png` 是用户现有 Rebased 原生参考；`commit-before.png`／`diff-before.png` 为改前；`final-commit.png` 核对短文件名，`final-diff.png` 核对标题／块控件与计数，`final-included.png` 核对标题全选联动及全选计数，`final-excluded.png` 核对从块控件取消，`final-menu.png` 核对未修改行的两项菜单禁用。截图窗口／侧栏宽度不同，用于结构和状态核对，不作为像素完全重合的证据。部分状态 SVG 与算法有测试；本轮原生 fixture 只有一个块，未把它宣称为多块三态视觉全覆盖。
- 原生验证曾因预览图缩放导致点击坐标偏差（原图 2182×1361），按原图尺寸修正后上述点击均正常；未为脚本问题修改产品事件机制。没有滚动自动化；没有内存／帧率性能结论。
- 已关闭本轮自建 Plico 进程；用户原有 Plico、Rebased 保留。未提交、未推送、未发布新包。

保留：Unified 逐块／逐行入口、排除区域淡化和拆色、菜单动态文案／快捷键、悬停临时 checkbox 未在本轮补齐。普通历史 Diff 不引入提交范围控件；版本标题仍使用 Working tree 文案。Rollback 布局、远端／Tag Smart Checkout、Stash 高级恢复分别记录，不能将这些功能差异归结成颜色／间距问题。

## Log 右侧展示区：分组与布局（2026-10-21）

参考源码仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`；宿主机 Rebased 1.1.20 与该快照不视为同一版本。本轮读取：

- `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/VcsLogChangesBrowser.kt`：继承 Show Diff，追加 Log changes toolbar，不重复创建 Group By；工具栏以 Log 工具栏作为高度参照。
- 同目录 `MainFrame.java`：文件区／详情采用上下拆分；Show Details 仅切换第二个组件，不改变提交选择；Diff preview 独立于详情可见性。
- `platform/vcs-log/impl/resources/intellij.platform.vcs.log.impl.xml`：PresentationSettings 顺序为 Group By、Layout；Layout 包含 Show Details、Show Diff Preview 和预览位置。
- `ShowDetailsAction.java`、`ShowDiffPreviewAction.java`、`ChangeDiffPreviewLocationActions.kt`：分别控制 SHOW_DETAILS、SHOW_DIFF_PREVIEW，位置选项只在预览开启时可见。
- `platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ui/DirectoryChangesGroupingPolicy.kt`：目录节点来自文件路径，分组不重新查询 Git。
- `ChangesBrowserBase.java`、`VcsActions.xml`、`RevertSelectedChangesAction.java`、`TabbedShowHistoryForRevisionAction.java`：弯箭头是反向应用所选历史变更；时钟使用所选文件的 revision（删除使用 before revision），不能替换成工作区 Rollback 或 HEAD 文件历史。

本地映射：`src/ui/files_options.rs` 管理眼睛菜单及选择保留；`src/ui/files.rs` 管理树／平铺、文件数根节点、详情显隐及空状态；`src/layout_preferences.rs` 持久化目录和详情选项。Ctrl+Alt+P 仅在 Files 焦点上下文切换；平铺显示父路径，切换不读 Git、不改变所选文件，恢复分组保留折叠状态。工具栏继续共用 LOG_TOOLBAR_HEIGHT。Diff 开关复用现有底部预览加载和设置持久化，不影响已打开的独立 Diff Tab；切换时退出本地变更预览来源，避免显示错误来源。读取提交详情期间不取消其加载任务。

保留差异：本轮对齐展示选项和文件／详情布局；未添加反向应用历史变更与指定版本文件历史两个入口，它们需要独立的历史目标／写操作流程，不能用现有近似操作代替。预览仍沿用应用现有共享的底部模式，不新增右侧预览位置；多父提交分组没有在本轮扩展。未宣称整个右侧面板已像素级对齐。

验证：配置默认值／独立值／异常输入测试、分组切换保持选中项与折叠状态的桌面测试通过；Windows release 编译通过。原生测试使用独立 profile，文件区菜单、详情隐藏、Diff 预览、平铺及重启持久化截图存放 `artifacts/log-files-20261021/`。未执行 Git 写操作；滚动不做自动化验收。

### 预览入口去重（2026-10-21）

用户确认以右侧眼睛菜单管理底部 Diff 预览。依据同一快照 `ShowDiffPreviewAction.java` 的 SHOW_DIFF_PREVIEW 展示属性入口，移除设置中的 Diff 页面及其底部／Tab 单选项，删除 SettingsDraft 中重复的预览状态和 Apply 写回路径。`set_bottom_preview` 移至 `ui/files_options.rs`；保留原 `diff-location` 持久化文件，因此已有偏好无需迁移。独立 Diff Tab、编辑器文件预览设置、Diff 自身显示选项均保留。该变更不新增设置，而是去除重复入口，也避免旧设置草稿覆盖眼睛菜单的即时选择。

去重验证：`cargo check --locked --bin plico`、Windows release 构建通过；Windows 独立测试进程截图 `artifacts/settings-preview-20261021/settings.png` 确认版本控制下仅保留 Git 设置，编辑器 Tab 等页面仍在。本轮仅删除重复入口，未增加镜像实现的测试。

## 底部 Diff 预览共用处理器与生命周期（2026-10-21）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，宿主机安装版 1.1.20 仍单独看待。本轮实读：

- `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/FrameDiffPreview.kt`：OnePixelSplitter，默认主区域比例 0.7；预览开启时创建 viewer，关闭时拆除并 dispose；不是永久保留一个隐藏 viewer。
- `MainFrame.java` 的 createViewer：底部 viewer 来自同一个 changes browser 的 `createChangeProcessor(false)`，工具栏高度引用 Log 工具栏。
- `VcsLogChangesBrowser.kt::createChangeProcessor`：editor 与底部选择不同 Diff place，但共享处理器／树 handler；底部监听可见性。
- `VcsLogChangeProcessor.kt`：从选中／所有 changes 构造 wrapper；预览换文件回选 changes tree；监听树模型变化。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/TreeHandlerDiffRequestProcessor.kt`：区分 selection/model/full 更新，默认 100ms 合并队列，可见时刷新、隐藏时清理；生命周期绑定 disposable。

发现的本地差异：普通单提交底部模式仍通过 `Review::load_file` 独立读 patch，用早期 DiffState／工具栏渲染；多选和 Commit 预览已走 DiffTab。旧路径还复用详情加载的取消令牌，文件选择可能影响详情任务。

本地调整：

- `ui/files.rs` 的 load_file 只更新选择；`ui/preview.rs::log_preview_target` 按仓库、提交、文件身份选择 Target/Chain，已打开的不可变链可复用。`ui.rs` 普通 Log、多选与 Commit/Stash 的底部预览统一使用 `DiffTab::preview`。单提交不再执行第二套 patch reader；移除旧 DiffState、监听器、源位置状态和旧面板工具栏。
- 隐藏底部／切独立编辑器时 deactivate 并释放预览，异步读取取消；显示时从当前树选择重建；选择同一目标不反复读 Git。自动激活不抢文件树／Log 焦点。F7 跨文件事件沿用 subscribe_diff_navigation，回选并展开对应树路径。
- `diff_tab.rs` 对 embedded toolbar 使用 LOG_TOOLBAR_HEIGHT、平面背景及底部分隔线；保留独立 Tab 的工具栏样式。底部去除 Compared with 总标题、额外圆角卡片及 Expand 模式，保留版本标题、只读标志、差异导航、跳转源文件、视图／显示设置。大文件显示、首处差异定位、中线拖动和差异高亮沿用同一个 viewer。
- 主预览拆分默认70/30，持久化拖动比例到 log-preview-ratio；上下区域直接相接，只有外部底角圆角。空／加载／错误状态不显示旧内容，错误保留刷新与复制详情。

保留差异：本地仍是单文件按需预览，不实现 upstream Combined Diff 多文件堆叠；暂保留底部位置，不增右侧位置。本地使用取消令牌＋generation 丢弃旧结果，未加入 upstream 的100ms合并延迟；细分更新队列并非完全复刻。工具栏非所有上游高级选项均已实现，未声称像素级全量对齐。

验证记录：`cargo check --locked --bin plico` 无警告；桌面回归52项通过，新增“文件选择不取消详情、相同路径不同 revision 不复用预览”测试单独通过。既有 `releasing_diff_clears_padding_from_retained_source` 基线失败项继续排除，未宣称全套测试通过。Windows release 构建及独立 profile 原生验证覆盖普通提交替换、根提交新增、预览显隐；截图位于 `artifacts/bottom-preview-20261021/`。原生显隐验证发现 retained resizable state 会带回单面板高度，已在显隐时重建拆分状态并保留记录的比例；复验确认。未进行滚动自动化排查。

补充原生拖拽验收：纵向分隔线拖动后保存比例 `0.588723`，`resized.png` 记录结果；此操作是面板调整，不是 Diff 滚动自动化。独立测试进程已关闭，用户的 Plico/Rebased 未关闭。

## 左侧工具栏拖动排序（2026-10-21）

参考快照仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，已安装 Rebased 为 1.1.20；不将源码快照等同于安装版本。本轮读取：

- `platform/platform-impl/src/com/intellij/openapi/wm/impl/SquareStripeButton.kt`：按钮标记为可拖动，左键执行，拖动期间抑制提示；`resetDrop` 清理鼠标状态。
- 同目录 `AbstractDroppableStripe.kt`：依据窗口 order 排序；拖动时计算插入位置并预留按钮大小的落点，绘制圆角反馈；`finishDrop` 才调用 `setSideToolAndAnchor`，`resetDrop` 清理临时布局。
- `platform/platform-impl/src/com/intellij/toolWindow/ToolWindowDragHelper.kt` 的 drag start/move/finish/cancel/stop：拖动有启动阈值；移动与松手分离，取消恢复按钮并移除目标反馈；工具栏外还支持浮窗（不在本轮范围）。
- `platform/platform-impl/src/com/intellij/openapi/wm/impl/DesktopLayout.kt` 的 `setAnchor` / `writeExternal`：以稳定窗口身份维护位置顺序，调整其他窗口 order，布局可持久化。

本地映射：`src/tool_window_order.rs` 封装稳定工具身份、归一化及排序，隐藏的 Comparison 继续占用保存序列的位置；`src/ui/tool_rail.rs` 管理 GPUI 拖动会话、图标预览、圆角占位、落点与取消；`src/ui.rs` 仅组合工具栏并捕获 Esc；`src/layout_preferences.rs` 有界读取 `tool-window-order`，只在有效且顺序变化的松手操作保存，失败保留原顺序并使用既有错误区域。取消/栏外松手不写配置，拖动不触发面板开关；正常点击保留原功能。拖动视图释放时清理共享临时状态，不保留 Review 强引用。

范围及保留差异：只在现有左栏内排序，未扩展左右停靠、上下工具组或浮窗；按当前 Plico 布局设置惯例保存为每用户顺序，不实现上游完整的项目布局配置系统。GPUI 原生启动阈值代替 Swing 的阈值；落点以单个按钮格计算；使用既有图标、选中底色及集中式边框色，未宣称像素完全一致。其他窗口的拖动物不接收。没有新增 Git 写操作。

功能阶段验收：

- 入口/布局：保持原左栏位置、32px 图标按钮和相邻面板关系；拖动时源按钮让出位置，目标处预留圆角格。
- 信息/操作：普通点击仍打开/收起 Project；拖动松手仅排序，Esc 和栏外松手取消。
- 状态/生命周期：有效放置保存；取消不变；重启读取归一化顺序；未知/重复设置条目自动补齐；隐藏 Comparison 的位置不会丢失。
- 验证：2 项排序模型测试通过，涵盖来回移动、隐藏项、未知/重复/缺失身份与身份唯一性；`cargo check --locked --bin plico` 和 Windows release 构建通过。Windows 独立配置原生验证 Project 从第二位移至第一位、拖动没有打开面板、Esc 取消、栏外取消及正常点击；截图位于 `artifacts/tool-rail-20261021/`。临时 Comparison 出现/隐藏由模型用例覆盖，未另做原生三图标场景；未进行滚动自动化或像素级验收。

最终 Windows 构建重启验证通过：保存的 Project → Commit 可见顺序恢复，证据 `artifacts/tool-rail-20261021/restarted.png`。本轮自建测试进程已关闭，保留用户程序；未提交或发包。

## Project 文件树：定位、全部折叠与复制路径（2026-10-21）

继续对照 `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 与该快照不保证一致。已读源码：

- `platform/platform-impl/resources/idea/PlatformActions.xml::ProjectViewToolbar`：标题工具栏放置 SelectInProjectView 与折叠动作；`platform/lang-impl/resources/intellij.platform.lang.impl.actions.xml` 为定位指定 `AllIcons.General.Locate`。
- `platform/lang-impl/src/com/intellij/ide/projectView/actions/SelectFileAction.kt` 与 `platform/projectView/shared/src/actions/SelectFileActionServiceImpl.kt`：Project 工具窗内的入口定位已打开文件，由编辑器上下文提供目标；`platform/lang-impl/src/com/intellij/ide/projectView/impl/SelectInProjectViewImpl.kt::selectInCurrentTarget` / `ProjectViewImpl.java::MyAutoScrollFromSourceHandler`：定位与打开编辑器分离，手动请求和自动跟随的焦点策略不同。本轮只实现手动定位，不启用自动跟随。
- `platform/lang-impl/src/com/intellij/ide/projectView/impl/AbstractProjectViewPane.java::createTreeExpander`、`platform/platform-api/src/com/intellij/ide/DefaultTreeExpander.kt`、同模块 `util/ui/tree/TreeUtil.java::collapseAll`：非 strict 折叠保留顶层根节点，选择归一化到可见祖先，并滚动到选择。
- `platform/lang-impl/src/com/intellij/ide/actions/CopyPathProvider.kt`：绝对路径、内容根相对路径和文件名分别由 provider 提供，没有可复制值时禁用。多模块/源代码根与语义名称复制依赖 IDE 模型，不在本轮实现范围。

本地位置：`src/project_view/navigation.rs` 封装定位/取消/全部折叠；`src/project_files.rs::Project::reveal_file` 复用分页读取，只读取祖先链，遵守原有目录页和总节点预算，保留链接/管理目录限制；目标超出第一页时自动读取后续页，受预算限制时明确报错，不能假装定位成功。UI 在后台结果成功且 generation 仍有效后合并祖先链，保留其他展开目录和较长的已有分页；手动选中其他节点、折叠或切换目标会使旧定位失效。定位不会发出 Open/预览事件。`src/ui/chrome.rs` 提供当前文件 Tab 或独立 Diff Tab 的路径，无编辑器目标时禁用定位按钮。历史删除文件不可恢复到工作区，只显示读取错误并保留原树。

`src/project_view.rs` 标题新增定位/折叠按钮；右键 Copy Path 改为绝对路径、项目相对路径、文件名子菜单，并继续捕获右键时的路径而非可变行号。相对路径以仓库根为基准，使用 `/` 便于引用；根节点没有非空相对路径时禁用该项。中英文文案在 `src/i18n.rs`。

保留差异：Plico 只建模一个仓库根，没有 IDE 模块/源码根/PSI；未加入自动定位、递归展开、删除/重命名/移动或全项目索引。现有刷新按钮保留；布局沿用 Project 标题工具栏，而非新建另一层工具栏。存量目录分页预算不变；本轮不声明全部像素对齐。

功能阶段 UI 验收与验证：

- 入口/布局：Windows 原生 Project 标题的定位/折叠按钮与已有刷新同排；无文件 Tab 时定位禁用，截图 `artifacts/project-tools-20261021/project.png`。右键复制子菜单的三项、层级及选中态见 `copy-menu.png`。
- 信息/操作：在独立 fixture 浏览并打开 `src/nested/example.rs`，全部折叠后根目录保持展开、选择回到 `src`，中央文件 Tab 保留（`collapsed.png`）；点击定位重新展开 src/nested 并选中文件（`located.png`）。绝对路径、`src/nested/example.rs` 相对路径和 `example.rs` 文件名的剪贴板内容逐项断言通过，见 `copy-results.json`。
- 状态/生命周期：桌面测试验证定位期间的折叠/主动选择会取消旧任务并使 generation 失效，后台结果不能抢回选择；复制菜单在目录刷新重排行后仍使用右键时路径。文件读取错误沿用现有错误区域，失败不清空原目录树。未新增自动跟随。
- 检查：`cargo test --locked --test project_files` 13 项通过（新增跨分页祖先链及取消/缺失/过滤/越界/链接测试）；`cargo test --locked --features desktop-tests --bin plico project_view::` 3 项通过；最终 Windows release 构建通过。最后将相对路径拼接改为按平台路径组件连接，避免破坏 Unix 文件名内合法的反斜杠，Windows 输出规则保持不变。
- 原生验证仅使用自建测试仓库和隔离配置；测试进程已关闭。中文文案已接入，中文截图、大目录 Windows/WSL 实测、完整像素对比仍未执行，不将它们计为本轮已验收。未提交、推送或发包。

## Project 自动定位打开文件（2026-10-21）

参考快照 `4aec35256d765c217aa0d662b82ac4e860c396fd`；宿主机 Rebased 1.1.20，版本并非同一承诺。
已读：
- `platform/platform-api/src/com/intellij/ui/AutoScrollFromSourceHandler.java`：selectionChanged 触发，100 ms 合并请求，隐藏/关闭自动跟随时不定位。
- `platform/lang-impl/src/com/intellij/ide/projectView/impl/ProjectViewImpl.java` 的 `myAutoscrollFromSource`、`MyAutoScrollFromSourceHandler`：树有焦点时不跟随，自动定位不请求焦点。
- 同目录 `ProjectViewState.kt`：默认关闭。
- `platform/projectView/shared/resources/intellij.platform.projectView.xml`：工具窗口次级 Behavior 菜单提供 AutoscrollFromSource；ActionsBundle 文案为 Always Select Opened File。

本地：`project_view/navigation.rs` 合并切换任务、复用受限异步目录定位并支持缓存命中；`ui/chrome.rs` 传递活动文件及面板可见性；`layout_preferences.rs` 持久化开关。手动选择/展开/折叠、切换目标、隐藏面板均取消过期任务；完成目录查询时再次检查焦点和可见性；不发送打开文件事件。
保留差异：Plico 单项目窗口将该唯一行为项放在 Project 标题栏 View Options 内，暂不增加只有一项的 Behavior 子菜单；偏好按用户保存，未复刻 IDE 多 Project pane/global 三层配置。只处理现有文件/Diff Tab 的文件路径，不引入 IDE PSI 元素导航。

验证：4 项 ProjectView desktop tests 通过（缓存定位不启动 IO、不抢焦点、手动选择取消、树聚焦抑制；以及原有定位/菜单回归）。Windows 原生独立 fixture 已核对默认未勾选菜单、开启设置、两个文件 Tab、全部折叠后切换 Tab 自动展开并选中目标；截图在本地 `artifacts/project-auto-follow-20261021/`。Windows release 构建通过。功能结构/交互已核对，精确像素及真实 WSL 跨机体验仍未验收。

## UI 横向检查：默认字体、控件和通用图标（2026-10-21）

参考源码 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版 Rebased 1.1.20 / IC-262.10968.SNAPSHOT。本轮以独立配置启动安装版，在自建 project-tools fixture 中只读查看；不是修改用户 IDE 设置。原生 JBR FontProbe 输出：Windows messageFont = Microsoft YaHei UI 12，JetBrains Mono 13 的 FontMetrics 高18。配置 other.xml 的 NotRoamableUiSettings 也记录12；编辑器配置没有字号覆盖。

已读取与提取规则：
- `platform/util/ui/src/com/intellij/ui/scale/JBUIScale.kt::computeSystemFontData`、`platform/platform-impl/src/com/intellij/ide/ui/laf/IdeaLaf.kt::initComponentDefaults`、`platform/util/ui/src/com/intellij/util/ui/JBFont.java`：Windows 消息字体是 UI 字族及字号来源，普通控件使用同一 Label 字体；不能只读字族然后固定13。
- `platform/editor-ui-api/src/com/intellij/openapi/editor/colors/FontPreferences.java`：JBR 默认 JetBrains Mono 13；默认 line spacing 1.2。`platform/platform-impl/src/com/intellij/openapi/editor/impl/view/EditorView.java::initMetricsIfNeeded` 用字体实际 metrics 和舍入策略计算行高，不能直接将13乘1.2当作行高。
- `platform/platform-resources/src/themes/islands/ManyIslandsDark.theme.json`：普通 Button 72×28、ComboBox 49×28，Tree/List 普通行高24、compact22；Tree 使用现代 Chevron；背景、边框、前景分层与现有共享色板基本一致。
- `platform/platform-impl/src/com/intellij/ide/ui/laf/darcula/ui/DarculaComboBoxUI.java::paintArrow`：New UI 使用 General.ChevronDown；`platform/icons/src/PlatformIconMappings.json` 映射 Cancel/通知关闭到现代 close，Actions.More 到 moreVertical。
- `platform/platform-impl/src/com/intellij/openapi/wm/impl/ToolWindowImpl.kt::GearActionGroup`：New UI 的工具窗口选项入口为 Actions.More，而非查看选项眼睛。
- `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/table/VcsLogGraphTable.java::getRowHeight`、`ui/render/GraphCommitCellRenderer.kt::calculateHeight`、`JBUI.java::VersionControl.Log`：Log 行高取字体内容高度（含垂直 padding）与主题最小行高较大值，不能套用 Tree 行高。

本地修正：
- `src/ui_font.rs` 从96-DPI Win32消息字体同时获取字族/字号，GPUI负责显示器缩放，避免二次DPI放大。API失败默认12；非Windows保留原来Inter13。已有 fonts.json 的显式设置不覆盖，恢复默认会使用新系统默认。
- `vendor/gpui-component/src/sizing.rs`、`menu/popup_menu.rs`、`tooltip.rs`：标准控件文字使用UI基础字号，移除默认0.875缩小。输入框/Select/Checkbox/Radio复用同一 sizing；显式样式仍优先。
- Reset、错误提示、提交面板、用户/路径筛选和文件历史普通正文取消固定13。ContainingBranches 使用当前UI字体绘制并测量，修复自定义字号后省略位置与文字宽度不一致。
- `src/assets.rs` 将框架内置关闭及四向Chevron路径映射至原版现代资源；Project 标题栏选项改为 moreVertical。资源来源保留在 assets/icons/SOURCE.txt。

检查范围与剩余差异：
| 区域 | 本轮结论 |
| --- | --- |
| 标题栏、项目头像、拉取/推送/分支 | 主图标已有上游资源；头像使用独立Mono字体，继续保留，不跟随正文任意缩放 |
| 默认UI字体、菜单、普通控件、筛选 | 上述系统默认与字号继承问题已修正；未统一修改所有明确为辅助信息的小字 |
| Project/Commit/Log右侧文件树 | 现有分别26/28/28行高，上游Tree普通24；选中背景、缩进/边距也有差异，尚未完成统一密度与焦点态还原 |
| Log/文件历史 | 上游按字体metrics计算行高，本地固定28；大字号下行高/graph命中同步调整仍待实现，不能只改CSS高度 |
| 文件编辑器/Diff | 默认字体13已对齐；本地字号×1.5行高与Java metrics×line-spacing不是同一机制；折行、gutter及光栅化仍有差异，本轮保持现有滚动/光标几何 |
| 设置与标准弹窗 | 通用正文及关闭/下拉图标已统一；自绘弹窗的精确间距、控件高度/宽度与全DPI对照仍待逐项验收 |
| Push/Pull/Reset/Stash/Changelist | 保持已有结构与操作逻辑，本轮不改Git操作；历史验收不能替代本轮所有弹窗同条件截图 |
| 菜单与快捷键 | 原版Chevron替换；菜单阴影、键盘焦点环、勾选标记和辅助快捷键小字仍有框架绘制差异 |

本轮是跨界面检查与明确问题修正，不宣称全功能像素级一致。原生截图/字体采样保存在本地 `artifacts/ui-audit-20261021/`；参考程序使用Safe Mode，因此其文件类型插件/Git入口状态不作为功能差异结论。字体与共同控件的视觉核对有效，Git Log、写操作弹窗同条件完整对照仍需后续验收。

验证结果：`cargo check --locked --bin plico`、Windows release 构建通过；55项桌面回归通过，明确排除原有 `releasing_diff_clears_padding_from_retained_source` 基线失败。原生Plico设置页确认未保存自定义字体时UI为Microsoft YaHei UI 12、编辑器JetBrains Mono 13，Project三点菜单、Log、设置打开/关闭截图已查看。验证exe与最终构建SHA256均为 `c6230d720cb5c51efa6050537b48b07ea1efcc8c181dbb4b36188106750bd3e4`。两款窗口尺寸不同，截图用于功能结构/字体检查而非逐像素差分。仅关闭本轮自有Plico与Rebased实例。

## 三处文件树密度与选中态（2026-10-21）

参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版Rebased 1.1.20。
已读取 `platform/platform-impl/src/com/intellij/ui/tree/ui/DefaultTreeUI.java` 的paint选择分支：现代单行选择左右留白12，Tree.Selection.arc取8（圆角半径4）；`platform/util/ui/src/com/intellij/ui/render/RenderingUtil.java` 按焦点选择背景；`ManyIslandsDark.theme.json` 使用活跃blue-50 #2A4371、非活跃gray-40 #33353B，Tree.rowHeight=24。
缩进依据：`Control.java` 默认ClassicPainter，`ClassicPainter.java` left=max(controlWidth/2,Tree.leftChildIndent)，逐层步长left+right。`darcula.theme.json` left=7/right=11，现代16px Chevron得到19；`UISettingsState.kt` compactTreeIndents默认false，registry中compact painter默认false。另读CompactPainter以确认其独立算法，本轮不新增紧凑模式。

本地在 `ui_style.rs` 共用Tree行高24、层级步长19、选择留白12与圆角背景。Project从26改24；Commit变更树及Log右侧文件树从28改24；统一图标间距4。Project与Commit选中文件从固定全宽蓝底改为有留白圆角、随焦点变色，复用现有Log右侧选择层。只改行容器布局，uniform_list的实际布局继续负责鼠标命中和滚动，不另写固定像素坐标。提交图/graph的28行高不在本轮改动范围。

保留差异：未引入所有IDE树的hover层/连续多选合并圆角、目录组选择模型和紧凑模式；本地有checkbox的Commit树用现有层级模型，未复刻Swing renderer所有像素偏移；大字号下的动态行高仍需后续统一处理。不是所有文件树状态的像素级完成声明。

验证：`cargo check --locked --bin plico`通过；桌面回归55项通过，排除已知基线失败`releasing_diff_clears_padding_from_retained_source`。最后仅将rem间距改为明确4px后再次通过Windows release构建。使用独立Windows仓库和配置验证Project展开/键盘打开文件、Commit选中文件与焦点移入提交信息框、Log右侧嵌套文件树；截图已查看，蓝色焦点态与灰色非焦点态切换正确，记录在本地`artifacts/tree-ui-20261021/`。本轮未自动化滚动，也未完成Rebased同尺寸全状态像素差分；大字号、多选连接圆角和紧凑模式仍待后续验收。仅关闭本轮自有测试实例。

### 文件树大字号行高（2026-10-21）

参考commit同上。新增读取 `platform/platform-impl/src/com/intellij/ide/ui/laf/LafManagerImpl.kt` 的 `patchHiDPI`/`patchRowHeight`：Tree行高由主题值通过用户缩放修正，并剔除上次缩放避免重复叠加；`platform/util/ui/src/com/intellij/ui/scale/JBUIScale.kt` 的 `getFontScale`、`scale`、`scaleFontSize` 区分字体/用户缩放与系统DPI，Windows基准为12。`DefaultTreeUI.NodeDimensions` 使用实际rowHeight计算行边界；`treeStructure/Tree.java` 字体变化会使尺寸失效。

本地 `ui_style::tree_row_height` 将默认24行高按UI字号/12增长（最小24），Project、Commit、Log文件树在每次render取当前值，交给uniform_list统一布局、命中、滚动范围；没有重复乘显示器DPI。编辑器字号不参与。保留差异：Plico尚无独立的全局UI缩放设置，本轮仅以UI字号驱动树行高，图标/缩进不随其整体放大；小于12的字号仍保留24行高，不复刻IDE全局密度切换。Log提交图的固定行高以及全局控件高度仍待独立处理，不宣称任意字号的全部UI完成对齐。

验证：cargo check、Windows release构建通过；55项桌面测试通过（仍排除既有 `releasing_diff_clears_padding_from_retained_source` 基线失败）。Windows隔离配置使用UI20/编辑器13，Project与Log右侧文件树行高随字号增长，展开后键盘打开example.rs成功，编辑器字号保持13；已查看原生截图 `artifacts/tree-ui-20261021/large-project.png`、`large-current.png`。一次截图采样全黑未计入验收，重新捕获有效截图后继续检查。未做全尺寸Rebased像素差分。
Commit大字号文件行也已截图核对（large-commit.png），无文字垂直裁切；同时发现窄提交栏的Amend旁计数横向空间不足，属于后续响应式控件布局问题，未纳入本次树行高修正。

### 提交栏窄宽度计数（2026-10-21）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`（安装版Rebased 1.1.20）：读取 `platform/vcs-impl/src/com/intellij/vcs/commit/NonModalCommitPanel.kt`，statusComponent左侧是工具栏，右侧留给计数；`CommitStatusPanel.kt::adjustLegendToFitPanel` 按当前字体测量完整legend与可用宽度，窗口尺寸变化时切换compact；`openapi/vcs/changes/ui/CommitLegendPanel.kt::CommitLegendComponent` 保留完整accessibleName。另读 `CommitActionsPanel.kt`，上游按钮区使用MigLayout与不换行工具栏，本地保留既有独立Commit/Commit and Push按钮及窄栏换行，暂不改变操作入口。

本地 `local_view/status.rs` 隔离计数绘制：以实际布局宽度、当前UI字体测量完整文本，不够时显示数字，右对齐；tooltip和aria label保留完整含义，不新增异步布局任务。Amend最多占行宽80%，留空间给状态；计数不再固定12号。保留差异：目前仍为选中文件总数（包含0），不是上游按新增/修改/删除分色的legend；不在本轮更改计数语义。极端窄栏仍可能裁切很长数字，整个提交栏的最小宽度和超大字号布局未宣称完全对齐。

验证：cargo check与Windows release构建通过。原生隔离仓库UI20下计数紧凑显示，勾选README后从0即时变为1；Amend文字在此宽度换行，底部按钮换行但均完整。UI12下完整显示“0 selected”。截图已查看并保存于`artifacts/tree-ui-20261021/status-*.png`。只验证显示与选择，不执行Commit/Push。此次低影响布局改动未重复跑整套Git测试；未完成全部字号/语言/极窄宽度验收。

### Log与文件历史统一行高（2026-10-21）
参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版1.1.20。读取`GraphCommitCellRenderer.kt::calculateRowContentHeight/preferredHeight`（字体或DPI变化重算，字体度量+垂直padding，与refs高度取max）、`JBUI.java::VersionControl.Log`（默认最小26、padding7）及`SimpleGraphCellPainter.kt`（paint与hit共用rowHeight和rowCenter）。
本地`ui_style::log_row_height`使用GPUI 1.5em行盒+7、最小26；Log/文件历史行与表头、graph端点/节点/箭头命中、键盘相邻提交弹窗锚点和刷新滚动恢复共用高度。`graph::arrow_at_height`保留原有节点优先/边顺序规则；默认28入口保留用于旧调用和原测试。保留差异：GPUI行盒不是Java FontMetrics；横向lane宽度、节点直径和线宽保持现有值，未整体按字号放大。字体变化时不保证保留当前行的精确像素锚点；异步刷新使用当时行高恢复。
验证：16项graph相关单测、56项桌面回归通过（排除既有diff padding基线失败）；新增26/37/55高度的箭头命中边界和37高度的部分可见行刷新恢复检查。cargo check及Windows release通过，原生UI20的3提交测试图线连接与节点居中截图已查看（`artifacts/tree-ui-20261021/log-adaptive.png`）。不自动化滚动；复杂图形/文件历史原生全状态尚待用户验收。

### Tab和设置导航大字号适配（2026-10-21）
参考同一commit。读取`IslandsTabPainter.kt::paintTab`：普通内容高度28、最少垂直留白8，紧凑24/4，值经过用户缩放；`SettingsTreeView.java::setUI`使用主题SettingsTree.rowHeight，结合此前已读LafManagerImpl的缩放逻辑。保留差异：本地通过GPUI行盒+padding撑高，不引入全局缩放/紧凑模式，不宣称Swing度量逐像素一致。
本地`ui_style::text_row_height/tab_height`共用字体计算，`islands_tab`与chrome内Log/编辑器/Commit/Stash的外层tab条同步增高；设置导航取消固定26，使用可容纳正文的行高。默认UI12仍保留28/36的tab高度和26导航高度。
验证：cargo check与最终Windows构建通过；原生UI20下Commit/Stash/Log Tab、设置导航文字及选中区域截图核对正常（`tabs-adaptive.png`、`settings-adaptive.png`），一次全黑截图已弃用并重新采集。只关闭独立测试实例。未重复全套Git测试；最高32字号和所有设置页仍待完整验收。

### 提交分类legend（2026-10-21）
参考同一commit，读取`ChangeInfoCalculator.kt`、`CommitLegendPanel.kt::CommitLegendComponent/LegendChunk`与`CommitStatusPanel.kt`：只统计included；NEW、MODIFICATION/MOVED、DELETED分别计数；新增和未跟踪同时存在时显示a+b，零项隐藏，顺序新增/修改/删除，紧凑符号+/*/-，完整可访问文本保留。
本地`commit_inclusion::Inclusion::summary`遍历已选身份，不读Git、不复制文件列表，before/after区分增删改，重命名归修改；部分选择身份仍计一个文件。`local_view/status.rs`分色绘制三段并提供中英文文字、完整tooltip/aria；按真实字体测量紧凑文本的最小宽度，空间不够时整段可换行，防止Amend吞掉计数。零项不绘制。保留差异：颜色沿用本地文件状态色；整数未做区域千分位格式；极窄窗口和很大计数仍受面板宽度限制。
验证：新增身份计数测试覆盖未选择、重复勾选、重命名、删除、未跟踪和清空；57项桌面回归通过（排除既有基线失败），compact规则测试覆盖+1+2/*3/-4与零项。cargo check/Windows release通过。原生独立仓库勾选三项tracked及一项unversioned后显示绿色+1+1、蓝色*1、灰色-1；未选择时为空。截图`legend-*.png`已查看。未执行真实提交或推送；原生中文/大计数仍待进一步验收。

### Push、Reset与编辑提交信息弹窗审查（2026-10-21）
参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；原生安装版Rebased 1.1.20 / IC-262.10968.SNAPSHOT。读取 `plugins/git4idea/backend/src/reset/GitNewResetDialog.kt`（目标摘要、radio组及comment说明）、`plugins/git4idea/backend/src/rebase/log/GitNewCommitMessageActionDialog.kt`（预填完整消息、聚焦编辑器、非空校验）、`plugins/git4idea/backend/src/push/GitPushOptionsPanel.java`（标签checkbox控制selector启用，FlowLayout同排）、`platform/dvcs-impl/src/com/intellij/dvcs/push/ui/VcsPushDialog.java`（默认Push split action、Cancel、Help及底部options布局）。
本地 `reset_dialog.rs` 说明文字从固定18px改为1.5em行盒，缩进随字体；`ui/remote.rs` Push底栏控件按字体撑高、selector宽11em、footer支持换行，540逻辑像素高度限制在窗口可用范围内；Help按钮也随控件高度增长。`push_dialog.rs`提交与文件行、头栏使用共同行高。未更改Push/Reset/reword的Git操作与校验流程。
原生隔离仓库对照已打开三种Rebased弹窗；Plico使用相同仓库、UI12与UI20，检查Push加载完成、目标与两条outgoing commit、Reset四种模式及说明、消息预填和初始焦点，取消后回到Log。未执行Push/Reset/重写。截图已查看，存于本地 `artifacts/tree-ui-20261021/` 的reference-*、plico-*、large-*。
保留差异：本地消息编辑框固定180逻辑像素、使用UI字体和圆角输入框，参考版采用commit editor并有历史入口；Reset仍缺Help且目标整段semibold；Push右侧toolbar项目组织与分隔线仍有差异。原生20号检查不是Rebased相同字号/相同窗口的逐像素对比，极窄窗口、32字号和多DPI尚未完整验收。Log空提交说明横向溢出另记后续，不混入本轮弹窗修改。
验证：cargo check和Windows release构建通过；本轮仅布局修改，沿用此前57项桌面回归结果（已知diff padding基线失败仍排除），未宣称重跑全部写操作测试。
补充原生验收：UI20且窗口缩小至1100×1000物理像素时，Push标签选项、帮助与确认/取消均完整可见（narrow-push.png）；帮助符号随控件高度修正后恢复显示。该尺寸仍为单行底栏，不作为极窄换行已验证的声明。

### Log相对日期单行显示补修（2026-10-21）
UI20原生检查发现“23 minutes ago”在145px日期列中换行，行盒居中后上下被截断。对照同commit的 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/table/VcsLogStringCellRenderer.kt`（按字体度量估计列宽）、`platform/platform-api/src/com/intellij/ui/ColoredTableCellRenderer.java`及`SimpleColoredComponent.java`绘制循环（共用baseline水平绘制fragment、按宽度裁剪）。本地`ui/log_history.rs`给日期单元格加入单行省略，与作者列一致，防止换行影响行内对齐；完整提交日期仍可从提交详情读取。保留固定145px列宽，不在本轮引入上游可调列宽与样本文字测量机制。
验证：最终Windows release构建通过，原生UI20截图log-date-single-line.png确认相对日期单行省略并与提交正文居中对齐；此纯布局补修未增加重复单测。

### 空提交说明限制在面板内（2026-10-21）
参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版Rebased 1.1.20。读取 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/VcsLogChangesBrowser.kt`、`VcsLogChangesTreeComponents.kt::updateStatusText` 和 `platform/platform-api/src/com/intellij/util/ui/StatusText.java::getTextComponentBound/paintComponentInBounds`：空状态属于changes viewer，随model状态更新，在owner/fragment绘制范围内居中显示。上游普通空提交changes状态会清空提示，不等同于本地长说明。
原生UI20、空提交、右侧窄面板截图稳定复现文字跨过左右边界。原因是`ui/files.rs`绝对定位居中层直接放置未限宽的文本；`ui/preview.rs`有同类排版。本地给文本独立的满宽、可收缩、居中换行容器，外层加padding及绘制裁剪，底部空Diff容器也限制最小宽高，避免内容撑大面板。保留本地对空提交/浅克隆/过滤无结果的解释文字，采用换行而不是上游短提示/空白，是明确的可读性差异。Git查询和选择生命周期不变。
此修正是纯布局，使用同一原生测试仓库截图回归，不添加只断言样式属性的单测。
验证：Windows release构建通过；原生UI20下普通窗口、底部预览开启和1100×1000物理像素窗口均确认提示在各自边界内换行，截图empty-fixed.png、empty-bottom.png、empty-narrow.png。右侧最窄约300物理像素时说明仍完整可见。此次未执行Git写操作，仅关闭自有测试实例。

### F7跨文件加载后的焦点交接（2026-10-21）
参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版Rebased 1.1.20 / IC-262.10968.SNAPSHOT。读取 `platform/diff-impl/src/com/intellij/diff/tools/util/CrossFilePrevNextDifferenceIterableSupport.kt`、`impl/DiffRequestProcessor.java::doApplyRequest/getPreferredFocusedComponent/isFocusedInWindow` 和 `util/DiffUtil.java::runPreservingFocus`（后两者同在`platform/diff-impl/src/com/intellij/diff/`）：替换request/viewer前记录Diff是否持有焦点，仅在此前持有焦点时交给新的首选组件；跨文件导航与文件内导航分开处理。
本地`src/diff_tab.rs`：加载时旧DiffState未挂载，但原先把焦点给了它；工作区EditableDiff创建后没有交接，F7离开有效事件路径。现在使用挂载在DiffTab根容器的稳定loading_focus承接异步加载，完成后仅当此容器仍持有焦点才交给真实编辑器。显式跨文件/文件选择在嵌入预览中也承接焦点，后台预览加载仍不主动抢焦点。保留差异：上游同步runPreservingFocus，本地异步Git加载通过持久焦点句柄衔接，不照搬Swing生命周期。
验证：新增回归先红后绿，覆盖工作区/历史、split/unified、独立Tab/嵌入预览、前后文件、加载时主动移开焦点。完整toolbar渲染触发上游GPUI重复hover的debug_assert，因此该测试在debug配置下显式ignore，使用`cargo test --locked --config 'profile.test.debug-assertions=false' --features desktop-tests --bin plico cross_file_navigation_retains_editor_focus_after_loading`运行通过（普通assert仍有效）。该配置跑全套时local-tree debug_bounds为空；默认debug配置重跑其余57项全通过，既有padding基线失败仍排除。未修改生产依赖来绕过断言。
Windows release构建通过；独立三文件、每文件两处变更的仓库，原生split工作区Diff从a到b，再不点击鼠标按F7定位b的第二处；Shift+F7返回a后继续定位a的第一处，左侧选择同步。截图`artifacts/f7-focus-20261021/focus-{cross,next,back,back-next}-fixed.png`已查看。嵌入模式与unified的焦点交接由自动回归覆盖，此轮未做完整原生矩阵。只关闭自有验证实例，未修改用户仓库。

### 最小可用阶段1：分支创建执行层（2026-10-21，进行中）
参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；安装版仍为Rebased 1.1.20 / IC-262.10968.SNAPSHOT。本轮读取`plugins/git4idea/backend/src/branch/GitNewBranchDialog.kt`（默认checkout=true，名称和目录冲突校验、创建/切换选项）、`GitCheckoutNewBranchOperation.java`（创建并切换、未合并阻挡、结果刷新）及`GitDeleteBranchOperation.java`前半段（普通删除失败后区分未合并及其他错误）。删除后续流程及上游测试尚需继续读取，不据此宣称删除已对齐。
本地新增`branch_create.rs`，Prepared封装显示起点与HEAD身份；执行前在仓库操作锁内复验名称、source、HEAD/OID及操作状态；checkout -b一次完成，避免先建后切失败留下分支；创建不切换不改变HEAD。所有命令结果后复查实际引用，不以取消推断完全未写入。当前明确差异：尚未做UI名称自动清洗、远端同名提醒、名称补全及smart checkout交互；不覆盖已有分支，不设置隐式tracking，UI尚未接入。阶段1尚未完成，不提交阶段完成记录。

阶段1续进：读取同一commit的`validators/GitBranchValidator.kt`、`validators/GitRefNameValidator.java`、`ui/branch/GitBranchActionsUtil.kt::createOrCheckoutNewBranch`、`actions/branch/GitCheckoutAsNewBranch.kt`、`GitDeleteBranchOperation.java`删除/恢复/未合并部分及`plugins/git4idea/tests/git4idea/branch/GitBranchWorkerTest.kt`创建、非HEAD起点、删除/恢复跟踪测试。上游HEAD名称校验不区分大小写；远端引用全名冲突也禁止；createOrCheckoutNewBranch显示覆盖已有分支选项，上游名称输入有清洗和补全。本地新增HEAD大小写/远端名碰撞校验；覆盖、清洗、补全仍待对齐，不能声明阶段1完成。
本地`ui/branch_management.rs`接入新建弹窗、默认切换、异步准备/取消及草稿保存，`repository_popup.rs`和`log_history.rs`加入从HEAD/引用/提交创建入口；提交前复验打开弹窗时显示的HEAD/目标。增加Ctrl+Alt+N（最新快捷键尚未包含在本次原生验证binary）。`branch_delete.rs`准备阶段检查worktree占用、HEAD/上游合并状态，确认未合并后才允许-D；复验tip、HEAD、上游base与branch config。恢复不切换、不覆盖现存名称，并恢复原分支配置；用最后一个点拆分config变量，避免把task.other配置当成task的配置。恢复入口当前放状态栏、只保存此窗口最近一次删除，与上游notification动作不同，后续统一恢复入口在阶段5。
测试：9项真实临时仓库测试通过，覆盖旧提交创建/不切换、草稿保留、覆盖失败不留下新ref、名称和目录/远端冲突、来源变化、取消、未合并确认、上游未合并但HEAD已合并、其他worktree占用及恢复跟踪配置。cargo check通过；中间版本Windows release通过。原生独立fixture `Plico-validation/branch-mvp-20261021/repo` 已验证创建native-task并切换、未合并分支确认列出提交、删除unmerged后Log刷新、状态栏恢复unmerged；Git命令确认ref恢复且a.txt草稿始终保留。截图在`artifacts/branch-mvp-20261021/`，branch-create-dialog含系统IME候选浮层不作为纯净视觉验收；branch-delete-confirm是布局稳定前截图，branch-deleted.png显示稳定弹窗（该次点击未命中按钮），branch-deleted-result/branch-restored为操作中截图，最终refs已另行核验。仅关闭自有Plico PID36992，保留用户Rebased PID27100。待继续原生同条件Rebased对照、UI细节/错误翻译、创建取消/不切换/Log起点等验收后才能阶段提交。

### 阶段 1：菜单禁用状态与恢复取消时序（2026-10-21）

继续读取同一参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`：
- `plugins/git4idea/backend/src/actions/ref/GitDeleteRefAction.kt`：当前 ref 禁用删除；本地映射 `src/ui/repository_popup.rs`，级联及右键菜单均禁用当前分支删除，写操作／保存文档期间禁用创建和删除。其他 worktree 占用继续由后端提交前复验。
- `plugins/git4idea/backend/src/ui/branch/GitBranchCheckoutOperation.kt`：已有同名分支先检测额外提交，checkout 与 create 分别处理 reset/rebase/use-local；不能将它简化为直接 `-B`。当前还未实现该交互，阶段 1 不据此宣告完成。
- `plugins/git4idea/backend/src/validators/GitRefNameValidator.java` 和 `branch/GitNewBranchDialog.kt`：输入时清理名称并调整 caret，提交时再清理末尾字符；当前输入清理仍待实现。

本地修复 `src/branch_delete.rs::restore` 的取消窗口：子进程退出（含取消）后先独立查询真实引用；若已恢复到预期 tip，完成分支配置恢复，避免留下缺失 upstream 的半恢复状态。与 Rebased 的恢复意图一致，具体取消生命周期适配本地异步进程执行器。

验证：`cargo test --locked --no-default-features --test branch_create` 10 项通过。新增真实临时仓库测试通过 `reference-transaction` 的 committed hook 在写入引用后触发取消，检查 tracking、tip 和当前 HEAD；该 hook 测试仅 Unix 执行。菜单改动尚未进行新一轮 Windows 原生验收，之前阶段截图不作为本次菜单状态的验收证据。

### 阶段 1：分支名称输入清理（2026-10-21）

重新读取 `GitRefNameValidator.java`，并确认同一参考 commit 的 `plugins/git4idea/backend/resources/intellij.vcs.git.backend.xml` 中 `git.branch.cleanup.symbol` 默认为 `-`。本地 `src/branch_create.rs::{clean_name_on_typing,clean_name}` 翻译 DROP／REPLACE／ENDPATTERNS 规则，保留 Unicode、输入时末尾目录分隔符，提交时移除末尾 `.lock`／`.`／`/`。后端仍执行 Git 名称校验，清理不能替代校验。

`src/ui/branch_management.rs` 订阅名称输入变化及失焦，按清理前光标前缀换算 UTF-8 caret，避免统一跳到末尾；IME marked range 存在时不改写预编辑。确认时将最终名称回填输入框再检查，以免用户看到的名称与实际提交名不同。本地暂采用上游默认清理符，不新增高级设置。

验证：名称规则单测通过（空格、控制字符、非法字符、目录、中文、末尾清理），`cargo check --locked` 通过。Windows 输入光标／IME 原生交互待验收；同名分支交互仍未完成，尚未提交阶段 1。

### 阶段 1：同名分支显式覆盖（2026-10-21）

再次读取同一版本 `GitBranchCheckoutOperation.kt::performCreate/performCheckout/create` 和 `GitNewBranchDialog.kt` 的 reset/conflict 条件。提取规则：覆盖默认关闭、存在本地同名分支时可选；额外提交以 `startPoint..existingBranch` 判断；覆盖当前分支使用 checkout -B，其他分支且不切换使用 branch -f。

本地映射：`branch_create::Prepared::read_options` 保存已有 tip、起点、HEAD，并最多读取 101 条额外提交供确认；`execute_confirmed` 必须有显式丢弃授权才能移除额外提交，写入前复验 tip 与 worktree 占用，普通 execute 不自动授予该权限。`ui/branch_management.rs` 增加默认关闭的覆盖复选框（名称更改清除勾选），确认列表最多展示 100 条并提示截断；取消不执行写入。

13 项真实 Git 分支集成测试通过，新增覆盖当前分支时脏文件保护、显式确认与过期确认拒绝、其他 worktree 占用拒绝。UI 编译通过（随后消除重复翻译 match 警告）；本轮 Windows 原生验收尚未执行。

尚存差异：Rebased 在 checkout 同名分支并存在额外提交时还提供 Checkout and Rebase／Checkout Local 分支，当前覆盖确认仅接入 Drop Local Commits 与 Cancel，不能宣称完整对齐；接下来继续复用既有 checkout 的相关路径。分支名称补全尚未接入。阶段 1 仍未提交。

### 阶段 1：已有分支选择与 rebase 状态（2026-10-21）

再次读取 `GitBranchCheckoutOperation.kt::performCheckout/performCreate`：勾选 checkout 且已有额外提交时提供 Rebase；另一按钮随 overwrite 选择切换为 Drop Local Commits／Checkout Local。创建但不 checkout 时保留已有分支可为无写入操作。

本地 `branch_create::Choice` 统一表示 Create、Discard、KeepLocal、Rebase；UI 按 prepared 条件构建选择按钮。复用已有 reference checkout 的 Git rebase 参数约定（`--no-autostash`）、操作锁、文档保存以及操作状态刷新；本地分支 tip 与起点仍于执行前复验。Report 新增真实 operation state，冲突或取消后的状态不因失败报告而丢失。

已实际读取 `GitRebaseProcess.java` 的 dirty-root 保存、冲突 resolver、SUSPENDED 与 Continue／Abort 通知代码。确认差异：上游自动保存脏工作区并在冲突解决后继续，本地目前拒绝脏工作区启动 rebase，冲突仅传入已有 operation_state；统一解决闭环是阶段 2 的既定内容，但这不等于上游完整 rebase 生命周期已经实现。阶段 1 的原生验收尚待完成。

验证：分支集成测试 15 项通过，新增 KeepLocal 保留 tip、Rebase 保留任务内容及基线、冲突产生 in-progress 状态且不误报成功；测试在临时仓库 abort 后检查原分支 tip。`cargo check --locked` 无警告通过。

### 阶段 1：最新 Windows 原生交互验证（2026-10-21）

上一轮 release Windows 交叉构建成功。仅启动本次自有 Plico PID 16588，使用 `Plico-validation/branch-mvp-20261021/repo`，结束后只关闭该进程。

截图 `artifacts/branch-mvp-20261021/branch-new-final.png`：Ctrl+Alt+N 打开窗口，输入框聚焦，checkout 默认勾选，overwrite 在无同名分支时禁用。`branch-clean-name.png`：实际键入空格被清理，IME 仍显示预编辑候选（该图不证明所有中文输入边界）。`branch-existing-input.png`、`branch-choices-final.png`：输入 unmerged 后出现额外提交列表及 Checkout and Rebase、Keep Local Branch、Cancel。Esc 取消后只读 Git 核对 HEAD 仍为 native-task、unmerged tip 仍为 b5d9d2ad…，a.txt 的 draft remains 保留，无新增分支。

验收发现选择弹窗按钮靠左，本轮调整为右对齐；该调整尚未重新构建截图。新增文件已 rustfmt 格式化。自动保存脏工作区与上游同条件截图仍待完成，不将本次部分原生验收视为整个阶段通过。

### 阶段 1：分支 rebase 的本地修改保存恢复（2026-10-21）

复读同一参考版本 `GitRebaseProcess.java::saveDirtyRootsInitially/notifyNotAllConflictsResolved`：启动前保存已跟踪修改，未解决冲突时保留保存内容并提供 Continue／Abort／View Stash。Rust 分支流程现在复用 `update_session::Session` 的仓库锁、不可中断保存、按 stash OID 检查的恢复。通过已有 `squash_saved` 工作区恢复记录衔接 `rebase_operation::execute` 的 Continue／Abort；为了与该恢复器的元数据核验一致，启动 rebase 显式使用 merge backend。

行为：保存后再次复验 HEAD／已有分支 tip／起点；已有恢复记录时不启动另一轮 rebase；完整成功或确认尚未开始时尝试恢复 index 与工作区；冲突／不确定状态保留 stash 和记录，并在错误详情标出 stash OID。未跟踪文件不纳入保存，沿用上游已跟踪修改保存范围。

验证：17 项真实临时 Git 仓库测试通过。新增暂存与未暂存两层修改分别恢复、未跟踪文件保留、成功后清理 stash／记录，以及冲突后经应用 Abort 恢复草稿和原分支 tip。`cargo check --locked` 通过。保存记录暂复用历史内部命名（gitui-squash-saved），避免改变现有记录格式；后续统一错误文案及命名仍需核对。最新保存逻辑尚未进行 Windows 原生验收。

### 阶段 1：保存取消、恢复冲突及 Windows 写操作验收（2026-10-21）

新增真实 Git 测试：reference-transaction hook 在 stash 已写入时触发取消，确认变基不启动、原 HEAD 和目标分支不动、草稿恢复且 stash／记录清理；另一测试让成功变基后的 stash 恢复产生冲突，确认不报成功且保留 stash 与记录。分支集成测试 19 项通过；核心 `cargo test --locked --no-default-features --lib` 222 项通过。

最新 Windows release 构建成功。自有 Plico PID 20248 在独立测试仓库中，通过 Ctrl+Alt+N、输入 unmerged、选择 Checkout and Rebase 完成带草稿操作。只读核验：HEAD 为 refs/heads/unmerged，a.txt 仍为 draft remains，stash list 为空、gitui-squash-saved 不存在。截图保存为 `branch-save-input.png`、`branch-save-choice.png`、`branch-save-result.png`；右对齐按钮已实际确认。仅关闭此次 PID。

进一步读取 `GitBranchWorker.java::checkoutNewBranchStartingFrom` 与 `GitImpl.java::checkout`：无 reset 时传 -b，有 reset 时传 -B。本地目前对没有额外提交的已存在同名分支默认保留并切换，而上游 TRY 会尝试 -b；该分支需继续核对并对齐。原生 Rebased 同条件对照仍未完成，阶段 1 不提交为已完成。

### 阶段 1：原生 Rebased 新建弹窗对照（2026-10-21）

用独立 REBASED_PROPERTIES 配置启动自有 Rebased 1.1.20 / IC-262.10968.SNAPSHOT（PID 7604），打开同一独立 fixture；未操作用户原有实例，未开启 Defender 排除项。Ctrl+Alt+N 截图 `artifacts/branch-mvp-20261021/rebased-new-branch.png` 确认：标题 Create New Branch 与应用图标；当前分支名预填并全选；Checkout 与 Overwrite 同行、左边缘跟输入框对齐。再次读取 `GitNewBranchDialog.kt` 对应 row(空标签) 与 selectAll 实现。本地已按该结构修正（540px 宽度预留中文及英文复选框），最新布局还需 Windows 重建核验。参考进程已关闭。

另对照 `GitBranchCheckoutOperation.kt`、`GitBranchWorker.java`、`GitImpl.java` 修正无额外提交且未勾选 overwrite 的默认动作：checkout 情况尝试普通 -b，遇同名保留原状态并报错，不暗中切到已有分支；不 checkout 时保留已有分支。新增测试后分支集成 20 项通过。桌面回归 57 项通过，1 项既有 debug-hover 断言测试忽略、1 项既有 padding 失败测试过滤，未将这两项计为通过。

### 阶段 1交付核对（2026-10-21）

最新 Windows release 原生截图 `branch-layout-final.png` 已确认：Plico 标题图标、当前分支预填全选、两个复选框同行且与输入左缘对齐、右侧 Create／Cancel。测试仅使用自有 PID 31008，现已关闭；Rebased 原始用户实例未关闭。

阶段要求对应证据：HEAD／历史提交／引用创建入口分别位于 Review快捷键和分支菜单、log_history、repository_popup；可选 checkout 及草稿保留通过分支集成与原生创建核验；删除的当前分支禁用、worktree占用复验、未合并显式确认、删除恢复通过临时 Git 测试与原生删除／恢复；取消、引用变化、保存和恢复冲突均有真实 Git 回归。20 项分支集成、222 项核心单元、57 项桌面以及27项相邻Git操作回归通过；既有桌面1项忽略与1项过滤如前文记录。阶段1功能交付完成，进入阶段2前单独提交。

保留差异与范围：单仓库流程不做 Rebased 多仓库选择；删除未合并分支采用显式确认后 -D，以满足本阶段未合并保护要求；删除恢复入口在状态栏且只保留本窗口最后一次删除，持久恢复属于阶段5；不包含整个 Rebased 的补全建议／高级设置系统。输入框焦点描边、对话框尺寸和标题栏间距仍有像素差异，不宣称像素级一致。统一冲突文件编辑和解决闭环按已授权阶段2推进，现有 rebase 状态与恢复记录为其保留完整真实状态。

### 阶段 2起点：冲突内容与两侧语义（2026-10-21）

阶段1已提交 `f77f7dc` 后开始阶段2。已读同一参考版本 `plugins/git4idea/backend/src/merge/GitMergeProvider.java` 前200行：detect依据仓库状态决定左右两侧反转，rebase/unstash的 yours/theirs 不能直接等同 index stage 2/3；loadRevisions 委托 GitMergeUtil；解决普通文件后 addFilesForce；列表通过 `ls-files --unmerged -t -z` 获取 index stage，二进制类型独立识别。后续必须继续读 MyMergeSession 的删除处理和 GitMergeUtil 的 acceptOneVersion/markConflictResolved，尚未读到的逻辑不算已确认。

本地现有 `operation_state::State` 只有冲突路径列表；`ui/history_operations.rs` 与 `ui/chrome.rs` 已有 rebase Continue/Abort，但缺少按冲突文件展示各 stage、编辑或外部工具及标记解决入口。下一步建立有明确 stage 身份的冲突快照，保证修改／删除、二进制和 rebase 左右语义正确。

### 阶段 2：冲突版本快照（2026-10-21）

已继续读取 `GitMergeProvider.java::MyMergeSession` 与 `GitMergeUtil.java::loadMergeData/acceptOneVersion/markConflictResolved/isReverseRoot`。规则：index stage1为base、2为ours、3为theirs；缺少一侧代表删除；DETECT 仅在 rebase 时反转，stash 需要调用者明确指定反转情境；接受现存版本先 checkout 对应一侧，再 add，接受删除则走 deletePaths。不能仅按文件是否存在判断冲突一侧。

新增 `src/conflicts.rs`：按 NUL 分隔解析原始路径与 mode/OID/stage，保留删除和 add/add 的 stage 缺失；rebase 展示侧反转；按不可变 blob OID 延迟读取内容；冲突操作前可重新核对选中文件的完整 index 身份和仓库 HEAD／操作类型。gitlink 不能作为普通文件 blob 读取。文件读写／外部工具和解决 UI 尚未接入。

验证：2 项解析单测通过（含路径制表符换行、截断、重复 stage、非法路径），真实临时仓库测试验证 merge/rebase 的 Yours/Theirs 内容及外部 git add 后旧快照拒绝。现有 raw Git 读取8MiB上限仍适用于单 blob 预览，后续接 UI 时须给明确大文件处理入口，不能静默截断。

### 阶段 2：单文件解决操作（2026-10-21，未接 UI）

继续读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitMergeUtil.java:414–470` 和 `plugins/git4idea/backend/src/util/GitFileUtils.kt::addPaths/deletePaths`：接受现存侧执行 checkout ours/theirs，随后 add -A -f；接受缺失侧执行 rm；手工解决通过 add 标记，删除文件也由 -A 登记。上游批量 add 使用 --ignore-errors；本地每次只处理一条精确路径，不使用批量部分成功选项。

本地 `src/conflicts.rs::resolve` 接入既有 operation_guard 与 operation_process；逐条保留命令结果，无论失败／取消均重新读取冲突状态，不把 checkout 成功当作解决成功。所有路径使用 --literal-pathspecs 和 --，拒绝 .git、父级链接、目录以及 gitlink 的普通文件解决；不递归删除。Gitlink 仍需外部处理。当前尚未补上游的版本相关 --sparse 参数，稀疏仓库支持需继续补齐。接受一侧必须由 UI 显式选择，标记解决前须先保存共享文档；本次没有自动选择或继续操作。

临时真实 Git 测试覆盖手工标记、二进制修改双方选择、修改／删除双方选择、含方括号文件名、无关文件暂存／未暂存内容保留、解决后旧快照拒绝；merge/rebase 侧语义测试保留。UI、原生截图、外部合并工具和统一 Continue/Abort 尚待实现，不计阶段2交付。

### 阶段 2：冲突列表 UI 接线（2026-10-21，待原生验收）

已读取同一参考 commit 的 `MultipleFileMergeDialog.kt`（列表、选择启用、acceptForResolution）和 `flow/OneShotMergeFlowDelegate.kt`：左侧文件表格，右侧纵向 Accept Yours／Accept Theirs／Merge，底部 Close；无选择时禁用，处理完成更新未解决列表。支持按目录分组；新版迭代解决另有独立 delegate，不将其与已安装版本混同。

本地新增 `src/ui/conflicts.rs`，标题栏在存在冲突时显示 Resolve Conflicts，异步读取最新快照，文件表格显示双方修改／删除；右侧接受双方版本、编辑文件、标记解决，底部关闭。写操作前保存共享文档，使用既有操作生命周期与文档锁，完成后刷新工作区与历史并重新读取冲突；成功条件包括命令与最终 index 状态。编辑复用已有文件 Tab，关闭列表不等于中止 Git 操作。增加中英文文案。

保留待办／差异：尚为单选平铺列表；内置三方编辑器未实现，当前 Edit File 是现有编辑页入口；二进制／大文件的外部工具入口、稀疏仓库、stash 显式反转语义及完整操作继续／中止仍需补齐。当前仅 Linux 桌面编译检查通过，正在构建 Windows 原生版本，未宣称 UI 验收或阶段完成。

阶段2本轮原生核验：Windows release 构建通过，独立进程 PID40176 打开自有 `Plico-validation/conflicts-mvp-20261021/repo`。`artifacts/conflicts-mvp-20261021/conflicts-list.png` 确认列表／双方状态／右侧按钮／底部 Close；点击 Accept Theirs 后 `conflicts-resolved.png` 显示无剩余冲突、按钮禁用，主窗口出现 Complete Merge。Git 只读复核 `ls-files -u` 为空，工作区与 index 的 review.txt 都为 task 内容，MERGE 尚待用户完成。Linux 读取该 Windows fixture 的 status 额外显示工作区执行位 100644→100755（内容 diff 为0/0），属于跨环境 mode 差异，未把它误报为冲突残留。只关闭自有 PID40176，保留用户应用。阶段2其他待办未变，不提交阶段完成记录。

### 阶段 2：已有 Cherry-pick／Revert 的继续与中止（2026-10-21）

已读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitAbortOperationAction.kt`、`CherryPickContinueCommand.kt`、`GitCherryPickContinueProcess.kt`、`EmptyCherryPickResolutionStrategy.kt`、`GitRevertProcess.kt`：中止需确认并调用对应 --abort；Cherry-pick --continue 区分冲突、覆盖本地修改、无操作、空提交与未知失败；空提交有 skip/create-empty 策略，不能仅当成功处理。Revert 原始流程以 AUTO_COMMIT 执行，发生冲突进入共用 apply-changes 流程。额外读取的 GitRevertResolvedAction 是“撤销冲突解决结果”，不是继续 Revert 的依据。

本地新增 `sequencer_operation.rs` 与 `ui/sequencer.rs`：限定已有且类型匹配的操作，未解决冲突拒绝 Continue，执行后无论失败／取消均读取实际状态；Continue 接入既有提交信息 editor helper，Abort 确认后执行，保存共享文档并统一刷新。标题栏在 CherryPick/Revert 状态显示对应 Continue/Abort，无冲突才可继续。本次没有新建 Cherry-pick/Revert 的入口（分别属于阶段5/3）。保留差异／待办：空提交仍显示失败并保留状态，尚需提供明确 skip/create-empty 选择；尚未做本轮 Windows 原生按钮与 editor bridge 验证。

已读 `GitRepositoryReader.kt::GitSequencerTodoReader`：多提交序列中手工 commit 会移除 *_HEAD，但 sequencer/todo 的 pick/revert 仍标识正在进行的操作。本地 operation_state_at 增加有界7字节读取，以完整命令前缀识别，未知序列仍保留 Sequencer，不擅自解释。临时真实 Git 回归覆盖 Cherry-pick/Revert 冲突继续和中止、未解决拒绝、错误操作类型拒绝，以及手工提交后继续剩余 Cherry-pick 队列。

### 阶段 2：空 Cherry-pick 显式策略（2026-10-21）

依据前述 `EmptyCherryPickResolutionStrategy.kt`，补充 Skip (--skip) 与 Keep (commit --allow-empty，优先 -F MERGE_MSG)；本地使用显式选择弹窗而非应用级默认策略，取消保留操作现场。`EmptyCommit` 记录 HEAD 与 CHERRY_PICK_HEAD，选择前复验暂存区无变化、无冲突及对象身份；只有 sequencer/todo 没有 CHERRY_PICK_HEAD 时不误判为空提交。Skip 另拒绝未暂存修改，避免丢失用户解决后继续编辑的内容。多提交序列保留空提交后若队列仍在，可通过 Continue 继续剩余项，不伪报整个序列完成。

新增真实 Git 测试验证两种策略、保持树内容不变、弹窗期间新增暂存内容时拒绝旧选择。sequencer_operation 集成共3项通过；Linux 桌面检查及 Windows 全 bins release 构建通过。独立 Windows PID32660、fixture `sequence-mvp-20261021/repo` 实测 Continue→空提交弹窗→Keep：`sequence-empty.png` 和 `sequence-completed.png` 留在 artifacts/conflicts-mvp-20261021；Git复核新提交186cd89、与父提交树无差异、工作区干净、CHERRY_PICK_HEAD消失。进程已关闭。此路径未触发提交信息编辑器，普通非空继续的 editor bridge 原生验证仍待补。

此前尝试复用上一轮跨环境 fixture 时 merge --abort 因工作区文件状态不一致拒绝，未强制清理；改用新的独立 fixture，并显式设置 core.filemode=false/core.autocrlf=false 消除测试环境混用。用户仓库未参与写测试。

### 阶段 2：外部合并工具数据生命周期（2026-10-21，UI 待接）

已读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/tools/external/ExternalMergeTool.kt` 和 `ExternalDiffToolUtil.java::executeMerge/tryExecuteMerge/waitMergeProcessWithModal/runWithTempMergeFiles/createMergeCommandLine`。规则：三个输入与结果文件分离；参数 %1=左／yours、%2=右／theirs、%3=base、%4=output；外部命令使用独立 argv；trustExitCode 分支按进程退出码，否则要求用户 Mark as Resolved；成功后 apply 输出，结束清理临时文件。

新增 `src/external_merge.rs`：独立随机临时目录、按不可变 stage 读取输入，结果以当前工作区内容初始化；无 shell 的 argv 占位符替换（不递归替换文件名中的 %）；沿用进程树取消；输出只有显式 apply 才以共享安全保存机制写回，apply 前重新核对冲突 index 和磁盘基线；独立的 MarkResolved 才暂存。临时目录生命周期由 Prepared 所有，失败可保留结果供 UI 决定。符号链接和 gitlink 不进入外部普通文件流程；普通二进制不解码，可作为字节数据处理。

验证：参数替换单测和真实临时 Git 外部进程测试通过，覆盖输出隔离、外部磁盘修改拒绝覆盖、apply不等于stage、解决后旧快照拒绝、临时目录清理。conflicts集成现3项通过。当前上限仍为工作区5MiB／blob8MiB，删除导致工作区文件不存在也尚未支持此入口；后续需补大文件路径／明确处理，不作为阶段完成。工具配置与UI确认尚未接入，未做此路径Windows原生验证。

### 阶段 2：外部工具配置与确认接线（2026-10-21）

补读 `ExternalDiffSettingsConfigurable.kt`、`ExternalToolsTablePanel.kt` 和 `ExternalToolsTreePanel.kt`：上游支持按类型选择多个工具，编辑实体包含 programPath／argumentPattern／mergeTrustExitCode，merge默认参数 `%1 %2 %3 %4`，附参数说明。本地 `ui/external_conflicts.rs` 提供冲突窗口的外部工具入口、Program与Arguments输入及说明，保存到应用配置而非仓库配置；本阶段单工具，每次启动前可修改。不实现多工具／类型映射／默认接管，且始终显式确认结果，这些差异不冒充完整设置还原。

参数先解析成argv再替换占位符，无shell变量或通配符展开，保留Windows路径反斜杠，未闭合引号拒绝；与上游通用ParametersListUtil相比不包含其全部转义语法。新增Windows路径／空格引号测试；2项参数单测和3项conflicts真实Git测试通过。外部执行占有应用操作生命周期与文档锁，执行前保存共享文档；进程结束弹出结果确认，Cancel不apply。确认采用后安全写回、标记解决、刷新工作区和Log。

外部工具原生验证：Windows release构建后独立PID41656，仍使用自有sequence fixture；测试工具是独立powershell脚本，仅把%2复制至%4。原生配置页／结果确认／已解决列表截图为 external-config.png、external-confirm.png、external-resolved.png。工具退出、尚未确认时，Git unmerged三stage与工作区冲突标记保持原样；确认MarkResolved后，unmerged为空，工作区与index均为task内容。随后原生Continue Cherry-pick生成20cd300，工作区干净、CHERRY_PICK_HEAD消失、Log刷新（nonempty-completed.png）。当前Git未请求编辑器，因此验证的是非空继续完整路径，不宣称已触发editor桥接弹窗。自有进程已关闭。Windows编译中的仅Unix需要mut的警告已通过cfg局部绑定消除，后续构建复核。

阶段2仍待：大文件外部处理、结果apply失败后的保留／重试、稀疏仓库、stash显示侧的上下文，及整体验收。未提交阶段完成。

### 阶段 2：外部合并失败结果保留（2026-10-21）

复读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `ExternalDiffToolUtil.java::handleMergeRequest/runWithTempMergeFiles`：上游仅在用户接受结果后 apply，finally 清理输出及输入。本地仍保持显式采用；另增加保守差异：工具失败／中断、写回或标记解决失败时保留独立临时目录并报告结果路径，防止磁盘基线复验失败后丢失用户已经完成的外部编辑。成功流程及采用前普通取消仍清理。保留文件需要用户复制／清理，尚未提供恢复结果管理页或一键重试，不宣称完整恢复交互完成。

本地位置：`external_merge.rs::Prepared::retain_output`、`ui/external_conflicts.rs`。完成判据额外检查新冲突快照不再包含目标文件，不能仅以 Git 退出码断言解决成功。真实 Git 集成验证：外部编辑期间工作文件变化导致拒绝写入，关闭 Prepared 后保留的结果内容仍存在，冲突 index 不变；常规成功生命周期仍删除临时目录。conflicts 三项集成通过，桌面编译通过；新增错误详情界面尚待 Windows 原生验收。阶段 2 仍未完成，不提交阶段完成记录。

### 阶段 2：外部合并大文件流式处理（2026-10-21）

复读同一参考 commit 的 `ExternalDiffToolUtil.java::createInputFiles/createOutputFile`、`LocalOutputFile`、`NonLocalOutputFile`、`DocumentOutputFile`：上游按文件／文档内容类型选择传输，文件输出可直接刷新或文件复制，不以编辑器文档大小作为统一门槛。本地保持独立输出与乐观内容复验，外部合并不再经过5MiB编辑器读取或8MiB Git文本输出限制。

实现：`operation_process::run_to_file` 将 cat-file blob 的二进制 stdout 直接导向临时文件，保留 stderr、进程树取消和退出检查；`external_merge::Prepared` 将工作区基线保存在磁盘；`file_save::save_from_file` 使用64KiB缓冲复制与内容比较，完整写好同目录临时文件后复验并使用既有平台替换方法，保留权限及Windows访问WSL的替换策略。基线比较为乐观检查，仍不是跨进程文件系统CAS；另一个进程可在最终比较与替换之间竞争，限制与原编辑器保存一致。

验证：新增9MiB+17字节二进制真实Git冲突测试，检查base／yours／theirs完整长度及尾字节、完整输出写回、取消不改原文件、apply不自动stage、没有残留保存临时文件。conflicts集成4项、既有file_save单测5项及桌面编译通过。此规模不是内存测量；传输缓冲有界不代表Git子进程内存已测量。Windows构建与原生验收仍待完成。

本轮后续验证：共享 `operation_process` 集成7项通过（stdin／双输出并行、取消、超时、早期拒绝与有界诊断）；Windows x64 release 构建通过，无编译警告。新增流式路径已被9MiB真实Git测试直接覆盖；该Windows二进制尚未进行本轮原生运行验收。

下一项源码依据已核对：`GitVersionSpecialty.kt::ADD_REJECTS_SPARSE_FILES_FOR_CONFLICTS` 从Git2.34.0起成立，`GitMergeUtil.java::markConflictResolved` 据此为add与rm添加--sparse。当前本地尚未实现该分支，须在阶段2交付前补齐及测试。

### 阶段 2：稀疏范围之外的冲突（2026-10-21）

按已读 `GitMergeUtil::markConflictResolved` 与 `GitVersionSpecialty::ADD_REJECTS_SPARSE_FILES_FOR_CONFLICTS`，`conflicts::resolve` 使用当前仓库实际Git执行环境查询版本；2.34及以上为add/rm追加--sparse，checkout保持上游--ours/--theirs参数。无法识别版本则在写入前报错，不猜测能力。版本边界单测包含Windows和Apple后缀。

新增真实Git sparse-checkout fixture：冲突文件原本不在检出范围，合并后对修改／删除冲突分别接受Yours、接受Theirs、手工MarkResolved，均验证最终index无冲突、文件内容／删除正确且范围内无关文件不变。conflicts集成5项及模块3项通过，桌面编译通过。原生验收待补。

进一步澄清stash侧语义：已实际读取`GitStashUtils.kt::UnstashConflictResolver.makeParams`，普通手工Apply/Pop只设置customizer，不setReverse；`GitConflictResolver.Params`默认reverse=false。`GitStashChangesSaver.java::UnstashConflictResolver.makeParamsOrUse`的自动保存恢复默认setReverse(true)。因此不能把所有Operation::None的冲突统一反转；后续需为自动恢复失败传递明确上下文，避免从仅有stash或恢复记录推断。此项仍待实现，不将阅读结论当作行为已完成。

阶段2原生补验：最新Windows release构建通过，自有Plico PID12468打开独立Windows Git fixture `Temp/plico-conflict-audit-ad44f257e83d43499e7598e59241c03c`。Abort Merge弹窗取消后只读核验三个unmerged stage仍存在；再次打开并确认Abort后，HEAD仍为80e2e703，review.txt恢复main内容，status为空，MERGE_HEAD已删除，顶部冲突／中止按钮消失，底部提示Merge aborted。截图`artifacts/conflicts-mvp-20261021/abort-confirm.png`、`abort-completed.png`；仅关闭自有PID，未操作用户Rebased或业务仓库。该验证覆盖Merge中止及取消确认，不等同于所有新功能原生验收。

### 阶段 2：自动恢复 Stash 的双方语义（2026-10-21）

依据上段已读的`GitStashChangesSaver::UnstashConflictResolver.makeParamsOrUse`与`GitStashUtils::UnstashConflictResolver`，本地自动保存恢复失败后写入工作树专属`plico-conflict-saved-changes`上下文，记录HEAD及原始unmerged index版本；普通手工Apply/Pop在无既存冲突时清除旧上下文，不反转双方。`Snapshot::read`仅在无Git操作标记且HEAD、剩余冲突的路径／mode／OID匹配记录时反转，接受前还复验反转状态；逐文件解决允许剩余集合缩小，最后一个解决后移除上下文。该上下文不等于原有stash恢复所有权记录，不自动删除保留的stash，也不把解决冲突冒充恢复流程全部完成。

接线位置：`update_session::Session::restore_saved`、`squash_saved::restore`、`stash_operation::execute`及`conflicts`。自动恢复前已有操作／冲突则拒绝再次pop；Git进程报告错误后仍读取实际冲突，避免只按退出结果猜测。上下文读取有界并拒绝链接／reparse；写入失败显式报错。与上游仅内存参数不同，该记录在重开窗口后仍有效；外部工具若完全重建相同HEAD及index冲突，无法仅靠内容身份区分新旧事件，此限制未声称解决跨进程事务问题。

测试：真实分支rebase后自动恢复两个文件产生冲突，“我的版本”均为保存草稿；解决首文件后重读仍反转，解决末文件后上下文清理，stash恢复记录仍保留。手工Apply测试确认旧上下文清除、Yours为当前工作区。conflicts集成6项、branch_create20项、reference_checkout11项、stash_drop3项及核心单测227项通过，桌面编译通过。本轮新增上下文逻辑尚待Windows构建及原生验收；阶段2仍未提交。

### 阶段 2：功能阶段交付核对（2026-10-21）

最新Windows release构建通过；独立Plico PID41792、自有Windows fixture `Temp/plico-conflict-audit-58a2c86efcf34f33bf427060bdb631fc`完成以下原生核验：

- 外部工具使用独立copy脚本产生task结果，等待确认期间将工作文件改为其他内容；确认后写回拒绝、三条unmerged stage不变，错误详情显示保留路径。只读检查结果文件仍为task，工作区仍为期间修改内容。截图`external-preserved-failure.png`。后台任务已在错误返回前保留文件，即使界面回调无法执行也不会清理失败结果。
- 从冲突列表Edit File进入已有编辑Tab，实际键入后自动保存；重新打开列表并Mark Resolved，index读取与编辑内容一致，unmerged为空，空态禁用操作按钮，顶部出现Complete Merge。截图`manual-resolved.png`。
- 关闭列表后点击Complete Merge，生成双父提交9dba922（b0b013e、4c9ca85），status为空，操作入口消失。截图`manual-merge-completed.png`。仅关闭自有PID41792，保留用户应用。

功能范围核对：冲突列表、接受两侧、手工编辑并标记、外部工具、继续／中止已具备；修改／删除、二进制、大文件、稀疏检出、rebase与自动恢复stash的侧语义、过期状态拒绝、空cherry-pick和多提交队列均有真实Git回归。核心227项、branch_create20项、reference_checkout11项、stash_drop3项、conflicts6项、sequencer_operation3项、operation_process7项通过；最新Windows原生构建包含全部实现。自动恢复stash两文件的侧语义由集成测试证明，未额外声称在Windows逐项点选所有边界。

UI阶段核对：左侧文件／双方状态表，右侧纵向动作，底部关闭；加载与操作共用底部状态和取消，空态禁用，失败详情可复制，编辑复用现有Tab与保存生命周期。明确保留的实现差异：列表单选平铺，没有目录分组／批量处理；内置三方合并编辑器未引入，使用现有编辑页或单个外部工具；外部工具始终确认而非信任退出码，失败结果以路径恢复而非独立恢复管理页；继续／中止沿用Plico已有顶部操作入口，不复制IDE通知系统。上述均为本阶段轻量冲突闭环的实现边界，不宣称完整复刻Rebased或像素级还原。gitlink冲突保持外部处理，普通二进制可接受一侧或外部工具，不能在文本编辑器编辑。新增中英文标签已接入集中资源。

阶段2功能交付完成，按用户要求提交后再开始阶段3的主动分支Merge与普通提交Revert；本阶段仅增加既有Cherry-pick/Revert的恢复入口，不冒充这两项的发起能力已完成。

### 阶段 3：分支 Merge 与普通 Revert 源码入口（2026-10-21）

阶段2已提交3821fcd后开始本阶段。已读同一参考commit的`branch/GitMergeOperation.java`前225行：保存文档后尝试merge，区分已有冲突、本地修改覆盖、未跟踪覆盖、合并冲突、已最新与未知失败；引用歧义时使用完整ref重试；冲突后进入共用resolver；成功可建议删除源分支。Smart Merge后续方法尚未读完，不能据此前半段实现完整保护流程。

已读`revert/GitRevertProcess.kt`全文件：继承GitApplyChangesProcess，按Log逆时间序处理，普通Revert自动提交、不沿用原提交作者元数据，空结果单独识别，冲突通过共用apply-changes流程。需继续读取GitApplyChangesProcess、GitImpl.revert与GitRevertTest；仅定位到测试路径不算验证。现有`merge_update.rs`服务Pull／Push更新且带fetch／tracking要求，不能直接当作任意分支合并入口。

### 阶段 3：普通 Revert 后端及 Log 入口（2026-10-21，进行中）

继续读取相同参考 commit 的 `revert/GitRevertAction.kt`、`revert/GitRevertProcess.kt`、`commands/GitImpl.java::revert`、`applyChanges/GitApplyChangesProcess.kt`执行与结果处理部分，以及`tests/git4idea/revert/GitRevertTest.kt`。上游普通Revert自动生成提交，根提交可用，包含merge commit时禁用；实际执行前重新获取详情；本地修改／暂存覆盖失败不自动丢弃，空结果单独提示，冲突进入公共resolver。多选按Log逆时间顺序执行，上游还支持保存修改后重试与部分成功通知，本轮未冒充已完成这些分支。

本地`revert_operation.rs`在执行前复验HEAD、分支、gitdir及操作状态，通过已有editor/auth桥执行`git revert <oid>`；不自动stage或stash。执行后即使失败／取消也读真实状态与HEAD，区分成功、空结果、冲突、取消、失败。`ui/sequencer.rs`复用现有文档保存、操作锁、编辑器请求与刷新生命周期；Log右键菜单新增Revert Commit，单选普通提交且仓库无进行中操作／冲突才启用。冲突打开公共解决窗口；空结果正常提示。中英文标签已接入。未将后端HEAD读取失败或意外未生成提交当作成功。

验证：真实临时Git仓库5项集成测试通过，覆盖普通／根提交、重复撤销空结果、无关未暂存修改保留、覆盖本地修改与暂存内容拒绝、过期快照、merge提交禁用、开始前取消不写入、冲突手工解决继续及中止。桌面编译检查通过。当前尚未完成Windows原生UI验收；单选只是本轮接线进度，多选／部分成功／保存后重试及主动分支Merge仍待本阶段后续实现，不提交阶段完成记录。

### 阶段 3：Revert 多提交执行与部分结果（2026-10-21，UI 队列待接）

复读同一参考 commit 的`GitRevertProcess.kt`和`GitApplyChangesProcess.kt::execute/executeForRepository/executeForCommitChunk/handleLocalChangesDetected/notifyResult`：按照Log逆时间序逐条执行；成功项和空结果分别记录；冲突／覆盖／失败停止；保存后重试过滤掉成功及空结果项；冲突解决后可继续。实际读取`VcsLogUtil.java`确认MAX_SELECTED_COMMITS=1000。

本地`revert_operation::Batch`先验证整组选中提交，禁止混入merge commit、无效对象或重复选择；取得同一仓库写锁后按原顺序执行，逐条复验预期HEAD／分支／gitdir。结果保留successful、skipped、remaining及最后命令状态，取消或验证失败也不丢失先前成功记录。remaining包括停止项及其后未处理项，排除已处理项。超过1000项明确拒绝而非截断。当前批处理尚未接入UI，菜单仍为单选；剩余队列的冲突恢复与保存后重试尚需实现，不能把数据结构当作完整恢复功能。

真实Git集成共9项通过，新增覆盖空结果后继续、成功后下一项冲突、无效后续对象在首次写入前拒绝、重复选择拒绝、执行前HEAD变化和1000项上限。当前新增部分为后端，未新增Windows原生验收记录；阶段3仍进行中。

### 阶段 3：多选 Revert UI 与冲突后队列继续（2026-10-21，原生验收进行中）

沿用已读GitApplyChangesProcess的逐提交策略。Log菜单按当前显示顺序收集选择并检查每条父数，保持1000项上限；后台通过Batch执行，显示成功／跳过／剩余数量。冲突后的Recovery保存精确gitdir、branch、HEAD、REVERT_HEAD及剩余项，继续时在同一写锁下复验，完成当前冲突提交后执行后续项。Abort成功及切换仓库清空窗口队列；重复恢复及外部Abort后旧恢复均拒绝。队列目前仅窗口内保留，重开窗口的剩余项恢复／覆盖本地修改后的保存重试尚未补齐，不视为阶段完成。

新增真实Git测试覆盖解决首个冲突后继续下一条、未解决拒绝、重复恢复拒绝、外部Abort后拒绝旧队列。Revert共11项通过，共用sequencer原有3项通过；桌面检查和Windows release构建通过。原生独立PID38584及临时fixture `plico-revert-audit-6df9ee7c6b044acab49742f83c4d5d86`发现多选菜单启用查询错误：snapshot.commits已被移动到Review.commits，导致Revert禁用。已改为读取实际列表数据，正在重新构建，尚未认定UI通过。一次截图全黑被采集校验拒绝，随后有效截图确认两条选择及禁用状态；不把全黑结果记为验收。仅关闭自有38584，用户进程未动。

阶段3本轮原生补验：修正菜单数据源后Windows release再次构建通过。独立PID42824、自有Windows fixture `Temp/plico-revert-audit-28ffd92ec7574116884b730c3e3945e5`，选中second与first两条，右键Revert启用；点击后依次生成f045199 Revert second与198412f Revert first，Git status为空。Log新增两条且保留原选择，右下角显示Operation completed—Reverted:2,Skipped:0,Remaining:0。截图`artifacts/revert-mvp-20261021/multi-menu.png`与`multi-completed.png`。仅关闭自有42824。当前验证多选成功链路，不等于冲突队列与editor桥接的原生验收；保存后重试、重开后队列恢复、主动分支Merge仍待完成。菜单多选仍显示单数Revert Commit，待对齐计数文案。

### 阶段 3：主动分支 Merge 后端（2026-10-21，UI 待接）

已完整读取参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的`branch/GitMergeOperation.java`，包括proposeSmartMergePerformAndNotify、doSmartMerge、doMerge、restoreLocalChanges与rollback：先保存文档，执行普通merge；本地修改覆盖提供Smart Merge确认，用GitPreservingProcess保存后再试；冲突进入公共resolver，未跟踪覆盖独立报错；已最新与普通成功区别通知；同名tag歧义重试完整ref；成功可提议删除源分支。另实际读取`tests/git4idea/branch/GitBranchWorkerTest.kt`的本地修改／空白与单行路径、同意／拒绝Smart Merge、简单合并、已最新、同名tag、已有冲突测试片段；多仓库回滚不作为本地单仓库事务实现依据。

新增`branch_merge.rs`：准备时记录完整本地／远端分支ref及OID、当前分支、HEAD、gitdir；执行前复验，并拒绝已有操作／冲突、同一分支或detached HEAD。直接使用完整ref，避免先尝试歧义短名；不fetch、不改变tracking。复用Session写锁及stash保存恢复，save_changes必须由后续UI显式确认；未跟踪文件不自动保存或删除。结果区分成功、已最新、本地覆盖、未跟踪覆盖、冲突、取消和失败；取消后读取真实状态，未照搬Pull专属reset--merge清理行为。Smart Merge无未完成操作时恢复草稿，失败／冲突保留saved_oid，不能宣称已自动恢复。

真实Git集成5项通过：完整ref与同名tag、已最新、本地编辑先拒绝且无stash副作用、明确Smart后合并并恢复不重叠修改、冲突保留可读取草稿及Abort、未跟踪覆盖保护、目标引用变化拒绝、远端分支引用创建双父merge且不依赖remote配置。当前仅后端；Smart保存记录跨继续／中止的自动恢复、UI入口／确认／删除建议及Windows原生验收仍待接入，阶段3未完成。

### 阶段 3：分支 Merge 菜单与 Smart Merge 确认（2026-10-21，原生待验收）

已读同一参考commit `ui/branch/GitBranchPopupActions.java::MergeAction`：标签表达所选分支合入当前分支，调用brancher.merge；本地非保护分支成功后PROPOSE删除，其他NOTHING。已读`branch/GitSmartOperationDialog.java`主体：覆盖本地修改后展示受影响文件、Smart按钮、Don't Merge取消按钮，取消默认焦点；Merge不提供Force按钮。上游中心可使用ChangesBrowserWithRollback或简单路径列表。

新增`ui/branch_merge.rs`，接入顶部仓库分支弹窗的子菜单／右键以及Log分支树右键，非当前本地／远端分支可用；detached、已有操作或冲突时禁用。执行前保存共享文档，后台复用editor请求桥、操作取消及完成刷新，冲突打开公共冲突列表。Smart Merge明确确认后使用首次准备的引用身份重新执行，弹窗期间分支变化由后端拒绝；取消无stash副作用。列表复用有界checkout_files解析，仅作显示，不作为命令参数；不完整时提示。取消默认焦点，Enter按焦点处理。

桌面编译检查通过；Windows构建进行中。当前差异／待办：未复用完整ChangesBrowserWithRollback；菜单暂为通用“Merge into Current”，未动态展示两侧分支；成功后的删除建议、Smart冲突后继续／中止自动恢复草稿，以及原生UI验收仍待完成。保存的stash OID在状态提示保留，但不能以提示替代恢复流程。未提交阶段完成记录。

本轮Windows x64 release构建已完成，无编译警告；尚未启动该二进制进行Merge UI原生验收。

### 阶段 3：Smart Merge 跨窗口草稿恢复（2026-10-21）

补读同一参考commit `util/GitPreservingProcess.kt::execute/load/configureSaver`及`stash/GitStashChangesSaver.java::load`：保存与恢复使用不可取消区段，autoLoadDecision控制暂缓恢复，load只执行一次；stash pop在启用staging时带--index，自动恢复冲突的左右语义反转。之前GitMergeOperation的resolver完成后restoreLocalChanges为恢复依据。

新增`merge_saved.rs`，在Smart Merge保存后、实际merge前记录工作树专属`plico-merge-saved`。记录包含原HEAD、目标提交、分支、stashOID，有界读取，拒绝链接／Windows reparse；不能覆盖旧记录。Merge Commit／Abort在消耗MERGE_HEAD前核验归属，结束后核验分支、实际HEAD或双父提交，再恢复且只pop仍在顶部的指定stash，保留原staged与unstaged。成功清理记录，恢复失败保留stash及记录并返回独立restore结果；UI不能仅凭合并命令成功显示全部完成，额外显示恢复错误并同步共享文档。

本地保留差异：记录落盘而非仅保存在上游操作对象中，重开窗口后仍可在应用内继续／中止并恢复；外部完成合并、恢复失败后的显式重试／放弃记录入口尚未补齐，当前保留数据不等同于完整恢复管理。跨进程外部写入仍只能做身份复验，不宣称文件系统事务。

真实Git分支Merge集成8项通过，覆盖中止后草稿恢复、重新构造Repository后完成冲突并恢复staged/unstaged、无关stash成为顶部时拒绝自动应用／删除、恢复自身冲突时保留草稿和记录。旧Abort测试从“stash保留”更新为“草稿已恢复且stash/记录清理”，与新增行为对应。共用conflicts6项及sequencer3项通过，桌面编译通过。本轮恢复逻辑Windows原生构建／验收待补，阶段3仍未完成。

### 阶段 3：保留草稿的显式恢复入口（2026-10-21，原生待验收）

依据已读`GitStashChangesSaver::showSavedChanges`及UnstashConflictResolver的Show Stash／Resolve动作，复用Plico既有Stash页，不另造stash清单。上游没有本地持久化记录清理动作；本地需明确解除应用记录，沿用既有Squash恢复确认结构，作为已记录的实现差异。

`merge_saved::Pending`捕获记录、gitdir、HEAD、branch；Retry与Clear均在写锁下复验，存在Git操作／冲突时拒绝。显式重试还要求原始／合并结果身份、指定stash仍在顶部，以及tracked工作区／index无修改，避免失败pop部分成功后再次套用；未跟踪覆盖仍由Git拒绝。支持外部完成／中止后重试；Clear只移除记录，保留工作区、index及stash，不把Clear称作恢复。

新增`ui/merge_recovery.rs`及标题栏条件入口，仓库刷新读取有界记录以便重开窗口后可见。窗口提供Show Stashes、条件启用Retry Restore、二次确认Clear；清理确认的Enter仅取消，显式按钮才清理。后台错误有详情，完成后共享文档与Log同步刷新。当前入口使用Plico已有顶部恢复动作位置，不复制IDE通知系统；此为结构差异。

分支Merge真实Git集成10项通过，新增外部Abort后重试、弹窗期间新修改导致拒绝、旧分支快照拒绝、进行中冲突拒绝清理，以及清理只移除记录而保留stash和本地内容。桌面编译通过；Windows构建进行中，原生恢复窗口尚待验收。阶段3仍未完成。

本轮包含显式恢复窗口的Windows x64 release构建已通过，无编译警告；尚未进行该窗口原生点选验收。

阶段3 Smart Merge Windows原生补验（2026-10-21）：已核验自有PID35740仍运行且路径为验证目录Plico-merge.exe，在独立fixture `Temp/plico-merge-audit-258e0b11fa144c6aaf49e86f36bc57a2` 从顶部task分支子菜单执行Merge。首次显示覆盖文件review.txt的Smart确认，取消默认焦点；点击Don't Merge后HEAD仍为8611971、草稿末行FIVE保留、stash列表为空。再次执行并点击Smart Merge，实际HEAD变为目标22cb487，内容同时保留目标首行ONE和草稿末行FIVE；stash为空、plico-merge-saved不存在。Log自动刷新HEAD/main位置，状态显示Branch merged。截图`artifacts/merge-mvp-20261021/smart-confirmation.png`与`smart-completed.png`。此验收覆盖取消和成功恢复链路，不代替恢复失败窗口与Revert冲突队列验收；阶段3仍在进行中。

### 阶段3：Revert冲突队列跨窗口恢复（2026-10-21）

本轮重新读取参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的`applyChanges/GitApplyChangesProcess.kt::executeForRepository/executeForCommitChunk/handleLocalChangesDetected`：逐条执行，冲突暂停；保存重试排除successful与alreadyPicked。上游队列存在process对象中，本地窗口可独立关闭，因此增加Git工作树目录内的有界持久化记录，作为明确实现差异。

新增`src/revert_operation/recovery_record.rs`：冲突返回前保存剩余OID顺序、当前HEAD及分支；读取拒绝非普通文件、链接／Windows reparse、超过72KB、超过1000项、重复或非法OID。继续前复验gitdir、HEAD、branch、REVERT_HEAD及磁盘队列归属；成功完成当前项后清理其记录，后续再次冲突保存新队列。共享sequencer Abort成功且实际操作结束后清理归属记录。`ui/sequencer.rs`重开窗口后的Continue在后台加载队列，不能默默只继续当前一项。

真实Git测试12项Revert与3项sequencer通过；重构原冲突继续测试为销毁结果、重新创建Repository并从磁盘读取队列，确认继续当前项后执行剩余项并删除记录。新增Abort清理及非法记录拒绝测试。桌面编译检查通过。Windows构建进行中，原生重开窗口验收待补。此记录不宣称进程崩溃事务安全：命令完成与后续队列转移仍有间隙；外部完成／中止产生的过期记录需要显式处理入口，保存本地修改后的Revert重试也仍待完成。

本轮包含Revert队列持久化的Windows x64 release构建通过（51.65秒，无警告）；原生重开与过期记录入口仍待后续验收／实现。

### 阶段3：外部结束Revert后的队列管理（2026-10-21）

再次读取同一参考commit的GitApplyChangesProcess::handleLocalChangesDetected及notifyResult，确认已完成项不重试。上游没有Plico持久化队列，因此显式清理为本地生命周期补充，不宣称复制上游对话框。结构沿用现有恢复动作：条件入口、待处理OID列表、说明、明确清理与Cancel；不自动推断外部操作后应该继续哪些提交。

`revert_operation::Pending`读取队列和当前HEAD/branch快照；只允许在无进行中Git操作／无冲突时清理，在写锁内复验快照与队列。清理不修改index、worktree或refs。新Revert批次发现旧队列会先拒绝，避免执行后才发现不能保存新队列。UI后台刷新检测记录，`ui/revert_recovery.rs`提供中英文入口及有界滚动列表，Enter不触发清理。

Revert真实Git13项及共用sequencer3项通过；新增外部Abort、进行中禁止清理、新批次拒绝覆盖、弹窗后切分支拒绝，以及同时保留staged/unstaged和HEAD的验证。桌面检查通过。Windows构建进行中，原生入口／取消／清理待验收。损坏记录仍报告错误，不自动删除；保存修改后重试及Merge成功删除建议仍待完成。

本轮Windows x64 release构建通过（50.60秒）。原生自有PID40296、fixture `plico-revert-audit-7bb728348e6440bb98dc57086b9bd170`：Log右键first执行Revert，真实冲突产生plico-revert-queue，含HEAD470b662、refs/heads/main、target3dca114。关闭自有窗口，在该fixture使用宿主机Git abort，再以自有PID33032重开：Saved Revert Queue入口仍在，弹窗显示原target。Cancel后记录仍存在；重开弹窗显式Clear后记录删除，HEAD仍470b662、status为空、review.txt仍为second换行。截图`artifacts/revert-mvp-20261021/queue-review.png`和`queue-cleared.png`。仅关闭自有测试进程，用户Rebased未动。此次验收为外部Abort后的跨窗口发现／取消／清理，不替代冲突继续编辑器桥接验收。

### 阶段3：Merge完成后的源分支删除建议（2026-10-21）

已重新读取参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的GitMergeOperation::notifySuccess与GitBranchPopupActions.MergeAction::deleteOnMerge：仅本地非保护分支PROPOSE，成功通知提供显式Delete动作，不自动删除。Plico当前保护策略与Push一致为main/master；尚无上游自定义正则／provider配置，这是既有差异。

`branch_merge::Prepared::prepare_deletion`点击时复验gitdir、合并目标分支身份、源tip和源提交被HEAD包含，再交给既有branch_delete准备／执行（worktree占用、upstream未合并检查、删除确认及恢复）。UI成功状态旁增加源分支名的Delete链接和Dismiss，替代上游通知动作；切仓／新Merge清空建议。取消、失败或仍保留Smart草稿时不显示。未直接自动删除，建议后源tip移动也不能删除新tip。修正Merge准备失败时残留“Merging…”状态。

真实Git Merge11项及桌面检查通过，新增尚未合入拒绝、成功生成删除准备、切换当前分支拒绝、默认保护分支禁止建议、源tip变化拒绝。原生按钮验收待补；冲突后独立Continue完成的删除建议尚未接入，不能据此认定阶段完成。

同轮完整读取GitApplyChangesLocalChangesDetectedNotification.kt：Show Files展示本地差异，有本地修改时提供保存重试；staging环境使用stash，其他遵循保存策略。结合GitApplyChangesProcess，重试过滤successful及alreadyPicked；Plico的Revert保存重试尚待接入。

包含删除建议的Windows x64 release构建通过（52.51秒，无警告）；该新按钮原生验收尚未进行。

### 阶段3：Revert保存重试后端（2026-10-21，UI待接）

完整读取参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的`cherrypick/GitLocalChangesConflictDetector.kt`：匹配would be overwritten by操作名，不限包含文件列表的merge头。重读GitApplyChangesProcess::execute，确认显式保存、逐项执行、失败保留saver、全部完成后load。流式事件增加ApplyChangesWouldBeOverwritten，Revert结合非零退出、无操作/冲突、HEAD未变以及非untracked事件分类LocalChanges，不能把未跟踪覆盖当成可stash重试。

BatchReport新增retry，包含未处理项并绑定本次已完成部分之后的HEAD，排除成功和跳过项。`revert_operation/save_retry.rs`在同一写锁下复验、显式Session.save、执行剩余批次，全部完成后restore_saved(--index)。新工作树专属plico-revert-saved记录保留stash/head/branch，8KB上限、独占创建、拒绝链接及reparse；保存失败若已产生stash也保留OID并停止。冲突／失败不自动pop，不覆盖上一份恢复记录。

Revert真实Git16项通过：新增部分成功后只重试剩余且过期请求拒绝；暂存和未暂存内容成功恢复、stash/记录清理；重试冲突保留stash且重建Repository可读归属记录。此前LocalChanges测试的Failed断言更新为具体状态，其文件保留断言不变。桌面检查通过，diff --check干净。当前仅后端：UI按钮、跨Continue/Abort自动恢复、恢复失败显式入口尚未接入，不能视作保存重试功能交付。Windows本轮后端构建待补。

补充未跟踪同路径覆盖测试：不提供保存重试、不生成stash、HEAD和未跟踪内容保持原样。Revert共17项通过，流式事件专项1项通过；Windows x64 release构建通过（53.30秒，无警告）。UI与跨操作恢复待接范围不变。

### 阶段3：Revert保存重试UI与继续／中止恢复（2026-10-21）

继续依据已读GitApplyChangesProcess、GitApplyChangesLocalChangesDetectedNotification、GitStashChangesSaver的显式保存／成功load／失败保留规则。Plico提供居中“本地修改阻止了撤销提交”确认，列出只包含剩余提交的OID，Save and Retry显式触发，Cancel/Enter不写入。它替代上游重要通知的保存动作，尚未复刻其Show Files入口；这是待补的结构差异。

`ui/sequencer.rs`新增SaveRevert，复用文档保存、editor桥、取消、后台结果与刷新。将原五元结果改成命名SequenceCompletion，保留retry与recovery分工；保存／恢复错误不吞掉，恢复后重新读取真实冲突状态。后端Recovery::resume在写锁内读取并核验保存记录与原历史，全部剩余提交完成后restore(--index)；公共Revert Abort验证当前队列身份后恢复到该停止点的草稿。新批次遇到保留的saved记录先拒绝，不能覆盖旧草稿。实际HEAD／分支／顶部stash复验失败保留记录，恢复冲突记录反向语义。

Revert18项和共用sequencer3项通过；新增重建Repository后Continue/Abort恢复staged+unstaged、并替换refs/stash模拟外部变化，确认完成Revert但报告恢复失败、旧草稿记录和新stash引用均保留。桌面检查通过，diff --check干净。原生保存确认及editor桥验收待补；保留草稿的显式恢复/清理入口尚待实现，阶段3未完成。

本轮Windows x64 release构建通过（52.12秒）。原生自有PID29664、fixture `plico-save-revert-audit-b9feaca896be4b53986046fa7658d00b`：draft.txt同时含staged draft与unstaged draft；Log右键second触发保存重试确认。取消后实际HEAD仍209785b，index与工作区两版本均保留，stash为空。再次确认Save and Retry后生成d72eac3 Revert second，review.txt恢复first，draft两版本分别恢复，stash及plico-revert-saved清理，Log自动更新并显示成功1/跳过0/剩余0。首次过早点击的采样仍停留确认弹窗，未误记为完成；确认就绪后点击及Git断言通过。截图`artifacts/revert-mvp-20261021/save-confirmation.png`、`save-completed.png`。仅关闭自有29664；本场景无提交编辑器弹窗，不据此宣称冲突Continue编辑器桥验收通过。


### 阶段3：保留的 Revert 草稿显式恢复（2026-10-21）

重新读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/stash/GitStashChangesSaver.java::load/showSavedChanges/UnstashConflictResolver`：恢复使用 stash pop，暂存模式携带 --index；保留内容通过 Stash 界面查看，恢复冲突进入 resolver。Plico 保留同一工作区与暂存区恢复语义；跨窗口持久化记录及显式清理属于本地生命周期补充，不宣称是上游原生弹窗。

`revert_operation/save_retry.rs::SavedPending` 捕获记录、HEAD 和分支；恢复前在操作锁内重新读取，要求原分支、原历史可达、保存的 stash 仍为顶部、无进行中操作／冲突、tracked 工作区和 index 干净。清理只删除应用记录，保留 stash 和全部文件；HEAD／分支改变后旧确认不能生效。`ui/revert_saved.rs` 提供查看 Stash、条件启用的恢复、二次确认清理及关闭；仓库刷新发现保留记录后显示标题栏入口。Enter 不执行清理。

验证：桌面 cargo check 通过；Revert 真实 Git 集成 19 项通过。新增测试覆盖冲突中禁止恢复／清理、外部 abort 后重建 Repository 恢复 staged/unstaged、弹窗后新增修改拒绝恢复、切分支／新提交拒绝旧请求、清理记录保持 stash/index/worktree/HEAD。Windows release 正在构建，原生入口、取消及恢复交互尚待验收；不作为阶段完成或提交依据。

本轮包含 Saved Revert Changes 入口的 Windows x64 release 构建通过（53.15 秒）；git diff --check 通过。原生界面验收仍待补，不宣称阶段3完成。


阶段3保留草稿入口原生验收（2026-10-21）：最新Windows release以自有PID11808运行，独立fixture `plico-save-revert-audit-d79071c25f3146bcb85cda0226435450`。本项通过真实Git stash及按当前格式创建恢复记录构造“重开后存在保留草稿”状态，不冒充完整Revert失败链路；此前真实Git集成覆盖生成链路。原生标题栏入口可打开居中窗口，显示main、stash OID、说明及四个操作。点击Retry Restore后，宿主工作区草稿为unstaged draft，index为staged draft，HEAD仍b6a573e；stash列表为空，plico-revert-saved消失。完成后标题栏恢复入口消失，右下角显示Saved changes restored，取消/进度控件退出。截图`artifacts/revert-mvp-20261021/saved-revert-dialog.png`及`saved-revert-complete.png`。本项覆盖显式重试成功，不扩张为清理二次确认及全部异常原生验收；后者仍由集成测试及后续验收覆盖。


### 阶段3：Revert 本地文件查看与重开后的冲突入口（2026-10-21）

重读参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitApplyChangesLocalChangesDetectedNotification.kt`、`GitRevertAction.kt` 及 `GitApplyChangesProcess.kt::executeForCommitChunk`：本地覆盖错误提供Show Files和Save and Retry；菜单按选择数量切换单复数；每次序列遇到冲突均进入resolver，不能因恢复入口不同而省略。

本地 `ui/sequencer.rs` 的重试确认新增Show Files，异步读取并去重tracked本地路径，在同一确认窗口展开有界文件列表，保留未处理提交队列和Save and Retry。没有自动stash或改变提交选择；取消后台读取不执行Git写入。与上游独立可查看差异的错误窗口有结构差异：本地展示只读路径列表，实际Diff继续由既有Commit面板提供。`ui/log_history.rs`多选改为Revert Commits并接入中文。重开后经Continue加载的Revert队列再次冲突，也会打开共用冲突窗口；此前仅新发起/内存Resume分支自动打开，已修正条件。

桌面检查在Show Files及菜单接线后通过；冲突入口条件调整后的检查及本轮原生验收待补。阶段3仍进行中。

本轮桌面检查及33项真实Git集成（Merge 11、Revert 19、sequencer 3）通过，git diff --check通过。已进一步复查GitMergeOperation::resolveConflicts/notifySuccess：冲突处理后成功仍有源分支删除建议。本地普通成功已有建议，但独立Continue路径尚未衔接，继续列为阶段3待完成项。


### 阶段3：冲突后完成合并的删除建议（2026-10-21）

复读相同参考commit的 `GitMergeOperation::resolveConflicts/notifySuccess`，上游解决冲突后仍按PROPOSE显示删除源分支动作。Plico `ui/branch_merge.rs` 保留本窗口主动Merge的Prepared身份；`ui/history_operations.rs::start_merge_operation` 在Continue前通过 `Prepared::matches_pending_merge` 核验gitdir、原HEAD/branch、源tip及MERGE_HEAD，完成后再次通过prepare_deletion检查包含关系与已有删除保护，然后显示已有显式删除建议。中止不产生建议；新合并／切仓清除旧上下文；结束后清理pending。建议仅为按钮，点击仍走删除确认和执行前复验。

本轮保留差异：建议身份存在窗口生命周期中，关闭窗口后仍能继续Git合并和恢复草稿，但不推测应删除哪个来源分支；用户仍可走常规分支删除入口。这不是恢复数据丢失，未引入额外持久化元数据。上游长生命周期process建议也不作为跨重启保证。

桌面检查通过，Merge集成12项通过。新增真实Git用例覆盖冲突开始/完成身份、冲突解决后prepare_deletion、Abort后禁止建议、源tip移动后拒绝旧身份。git diff --check通过；原生建议按钮及本轮Windows构建待补。


本轮Windows release构建通过（1分02秒），完整 `cargo test --locked --no-default-features` 通过。原生自有PID42520、独立fixture `plico-merge-audit-76dadf16176a4fedb2d0a04d4d469709`：从分支菜单主动Merge task，产生真实冲突并自动打开resolver；Accept Theirs后显示All conflicts are resolved和顶部Complete Merge；点击完成产生daeefcc，父提交153626e/456ee1f，工作区干净，MERGE_HEAD消失。Log增加合并行，底部显示Merge completed及Delete branch task。点击建议进入已有Delete Branch确认，Cancel保留task=456ee1f及HEAD=daeefcc。截图在artifacts/merge-mvp-20261021的merge-continue-resolve/ready/final及merge-delete-ready.png。未自动删除源分支，未操作用户仓库。此验收覆盖冲突后完成及建议/取消；Revert继续的编辑器桥和新Show Files原生交互仍待补。


### 阶段3：Revert跨窗口Continue及提交注释修正（2026-10-21）

原生fixture `plico-revert-audit-980bf31a29a34267b6392caea27cd23f`，PID41864主动Revert first(1c7eec6)产生冲突，队列绑定HEAD702e3f9。Accept Theirs后工作区/index为base，重开为自有PID19276，Continue Revert从磁盘恢复并生成e81e255；队列及REVERT_HEAD消失、工作区干净、Log+1。截图revert-continue-conflict/reopened及revert-editor-ready.png存于artifacts/revert-mvp-20261021。Git本次未调用编辑器，不将截图文件名理解为弹窗证据。

此原生验证发现提交说明泄漏 `# Conflicts:`。依据diagnosing-bugs流程新增真实Git回归 `continued_revert_does_not_commit_generated_conflict_comments`，修前0.23秒稳定失败。复读相同上游commit的GitImpl.revert及GitImplBase::loadFileAndShowInSimpleEditor：前者直接revert commit，后者过滤自身约定的注释行。Plico原始冲突生成未改commentChar，但sequencer Continue单独改为U+0001，使既有MERGE_MSG中的#不再被Git视作注释。仅移除Continue的配置覆盖后同一回归通过；不手工改写MERGE_MSG、不强制弹编辑器或改变原提交消息。进一步覆盖仓库自定义`;`注释字符，确认尊重仓库配置。源码位置src/sequencer_operation.rs，相关测试/桌面检查进行中，Windows修正后原生复验待补。


阶段3修正后原生复验（2026-10-21）：Windows release构建通过（54.13秒）；修正后的Revert 19项、sequencer 4项及桌面检查通过。独立fixture `plico-revert-audit-074b6fc9fb224fa9b787a9086be94fa6`、自有PID15112，预置真实Git冲突及当前格式队列后经原生Continue生成e8b617；提交说明只包含正常Revert文本，没有Conflicts注释，REVERT_HEAD和队列均清理。此预置用于检查Continue，不冒充由本次窗口发起的完整冲突链路。

随后给draft.txt分别写入staged draft和unstaged draft，原生Revert进入本地修改确认。Show Files展开Local Changes，只显示一次draft.txt，剩余目标e8b617及保存重试按钮保留。点击Cancel后通过真实Git和文件字节断言：HEAD仍e8b617，index为staged draft，工作区为unstaged draft，stash为空，plico-revert-saved/queue和REVERT_HEAD均不存在。截图`artifacts/revert-mvp-20261021/revert-current.png`、`revert-show-files.png`、`revert-files-cancel.png`。此项完成Show Files及取消验收；保留Merge草稿恢复窗口的原生验收仍待完成，阶段3尚未提交。


阶段3Merge保留草稿入口原生验收（2026-10-21）：自有PID35928、独立fixture `plico-save-merge-audit-235e0f38ad434ccbb218353415624876`。用真实Git创建stash并按已核对格式预置plico-merge-saved，覆盖恢复入口本身；自动保存记录的生成链路由前述Smart Merge原生与集成测试覆盖。标题栏Saved Merge Changes打开居中窗口，展示分支、stash及四个操作；Retry Restore后HEAD仍76381f6，index为staged draft，工作区为unstaged draft，stash为空且恢复记录清理。等待后台刷新后入口消失，状态显示Saved changes restored。截图`artifacts/merge-mvp-20261021/saved-merge-dialog.png`及`saved-merge-complete.png`。本轮重跑Merge 12、Revert 19、sequencer 4项，共35项全部通过。此项不扩大为清理二次确认所有分支的原生验收。


### 阶段3收尾：合并继续失败诊断与交付核对（2026-10-21）

复读Rebased同一commit `plugins/git4idea/backend/src/merge/GitMerger.java::mergeCommit`，其runCommand.throwOnError保留Git失败。Plico原Continue/Abort只取外层Err，内层非零命令失败丢失详情。提取原展示判断后真实Git回归稳定失败（0.10秒，fatal输出丢失为空）；`merge_operation::diagnostic`现在合并command、state、restore错误，复用rebase已有输出筛选、URL凭据脱敏和长度限制。UI成功判断补充input_error检查，恢复错误不再遮蔽命令错误。两个诊断回归通过，覆盖真实Git非零退出及输入/状态/恢复错误组合与脱敏；Merge此前12项、Revert19项、sequencer4项及桌面检查通过。最新Windows release构建54.05秒通过。

原生错误路径：独立fixture `plico-merge-audit-012e3319d19f44689332bab0371b41ac` 预置冲突后已暂存解决内容，用测试pre-commit hook拒绝提交；自有PID38644点击Complete Merge，弹窗准确展示fatal: Plico merge audit hook rejected，底部显示Merge operation failed。HEAD保持15443dc，MERGE_HEAD及已暂存resolved内容保留，可继续处理，不误报成功。截图artifacts/merge-mvp-20261021/merge-error-result.png。

阶段3功能核对：指定本地/远端分支合并、显式Smart保存重试、覆盖文件提示、冲突继续/中止、删除来源分支建议与复验、保留草稿恢复；Log普通/root单选及多选Revert、merge commit禁用、空项跳过、部分完成与后续队列、保存重试、跨窗口继续/中止及外部结束后的记录处理均已接线，并有前述真实Git测试/原生主路径证据。阶段3可按功能阶段提交。

保留差异：通知采用现有状态栏和恢复入口；Show Files在确认内列路径而非上游独立Diff窗口；保护分支沿用main/master默认策略；来源删除建议不跨窗口保存；应用Revert队列不是崩溃事务日志。精确像素、恢复记录清理二次确认的全部原生异常组合未作已验收声明；清理语义和过期拒绝已有真实Git测试。未强制弹出Git本来没有请求的提交编辑器。阶段4/5仍未实现，不将本次提交视为五阶段目标完成。


### 阶段4源码对照起点（2026-10-21，尚未实现）

阶段3已提交0a2a0e6后开始本阶段。继续参考4aec35256d765c217aa0d662b82ac4e860c396fd，已读取GitConfigureRemotesDialog.kt：名称/URL表格、origin优先、增删改及双击编辑、删除确认；编辑先rename再set-url，任何失败需刷新真实状态而非假设事务回滚；添加/编辑可选fetch。已读取GitDefineRemoteDialog.kt：名称格式/唯一性与非空URL验证、默认勾选Fetch、异步ls-remote和loading、关闭取消scope、访问失败保留错误并使OK可用以便重试；doOKAction每次仍执行checkUrlAccess，失败分支不调用super.doOKAction，不能据withOKEnabled推断可绕过访问检查保存（2026-10-23审计复读纠正）。

已读取GitUserNameNotDefinedDialog.kt：Name/Email两个输入、非空和@校验、全局配置复选框、明确设置后继续提交。其实际配置写入/提交前检查调用链尚待读取，不把界面声明当作已验证写入语义。GitCheckoutProvider.java已读clone任务段：后台进度、成功后checkout listener打开、失败通知、委托CloneableProjectsService；取消及残留目录具体处理仍待沿该委托读取，不能据此决定删除目录。

本地候选映射：欢迎页/项目入口新增Clone，仓库菜单新增Remotes管理，提交前身份引导复用现有commit流程；操作层复用Git子进程、askpass、取消和刷新机制。下一步先读完整clone生命周期、Git底层remote命令及identity检查/写入，再写后端真实Git测试。以上为已确认规则与后续读取边界，不是实现或原生验收完成记录。


### 阶段4远端与身份配置后端（2026-10-21，UI未接）

继续读取同一参考commit的GitImpl.addRemote/removeRemote/renameRemote/setRemoteUrl与GitRemoteTest（HTTP认证/错误密码/浅克隆用例，上游该测试标注缺服务器而禁用）。`remote_management.rs`按GitConfigureRemotesDialog顺序实现增删改和origin优先列表，保留fetch/push URL及远端配置快照；执行前在工作区操作锁内复验来源gitdir、名称唯一性及旧配置，外部修改后拒绝旧弹窗。URL采用独立参数，拒绝选项形态/控制字符；不输出凭据或给Remote实现Debug。访问检查复用askpass和可取消ls-remote；fetch仍由后续UI显式选项驱动。

真实Git表明多URL的set-url可能失败，沿上游顺序保留已完成rename及未变URL，不声称事务回滚。测试覆盖该部分成功、额外push URL保留、branch.remote随rename更新、同名冲突、跨仓/过期快照、非法参数、预先取消、无写入访问检查、含点名称隔离、HEAD/index/worktree不受配置操作影响。

完整读取GitCheckinHandlerFactory.kt的GitUserNameCheckinHandler检查/写入段及GitConfigUtil.setValue、GitUserRegistry.readCurrentUser。上游按显式范围顺序写user.name/user.email，失败不继续提交；表单要求非空及@。`identity_config.rs`提供独立读取、校验、Repository/Global显式范围、逐命令结果与最终有效配置；不自动提交。保留差异：Plico会对空email也提示配置，而上游Registry只有空name返回null；这是防止已有name但缺email时反复提交失败的引导。测试目前只写临时仓库local配置，不改真实global；Global隔离测试、提交UI恢复与原生验收尚待补。

CloneableProjectsService的runCloneTask/cancelClone/onCancel已读：取消交给进度指示器，状态区分SUCCESS/FAILURE/CANCEL，没有在该层递归删除目录。GitImpl.clone已读：progress、Windows longpaths、可选depth与recurse-submodules、使用默认Git执行环境。本轮尚未实现Clone，也未据此假定所有失败目录已由Git清理。


### 阶段4身份配置提交入口与原生验证（2026-10-21）

沿已读GitUserNameNotDefinedDialog/GitUserNameCheckinHandler实现`identity_dialog.rs`：姓名/邮箱行、默认不选的全局范围、设置并提交/取消、姓名初始焦点、异步保存与关闭取消。中文文案已接入。`ui/identity.rs`后台预检配置，经操作身份及仓库路径核验后继续原提交；接入LocalView的普通/Amend/提交并推送以及SourceChanges变更块提交。既有暂存恢复动作跳过身份门禁。没有配置成功时不进入commit/index写入。全局默认不选为明确的局部作用域策略；当前不持久化该复选框，也不猜测操作系统用户名。

Identity测试5项（其中一项为父测试启动的隔离子进程入口）、Remote6项通过，桌面检查通过；全局写入仅在子进程设置GIT_CONFIG_GLOBAL到测试临时文件，父进程和真实全局配置未改。Windows release构建44.52秒通过。

原生PID42336、fixture `plico-identity-audit-c90da013ce224615b07369347d9591a0`：本地姓名/邮箱为空，勾选draft.txt并输入消息后点击Commit，显示居中配置窗口。Cancel后HEAD49a3600、空身份、index staged draft和工作区unstaged draft均保持。随后填Native User/native@example.invalid，未选global。夹具最初追加了重复user键，Git拒绝普通config写入，窗口显示错误并保留输入、没有提交；只在测试夹具中整理为单个空值后原窗口重试，生成0164278，作者为输入身份，提交选中文件，status干净。消息在测试SendKeys时受中文IME影响成为Identitynativetest；不将该输入工具行为记为产品消息转换。截图保存在artifacts/identity-mvp-20261021。普通提交主路径及取消/配置失败重试已验收；Amend/部分提交/提交并推送的独立原生回归仍待补。远端UI与Clone尚未完成，阶段4不提交。

### MVP 第 4 阶段：Define Remote 连接检查接线（进行中）

参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，本轮实际重读
`plugins/git4idea/backend/src/remote/GitConfigureRemotesDialog.kt` 和
`GitDefineRemoteDialog.kt`。后者名称／URL 校验后异步 `lsRemote`，检查时禁用输入，
Fetch 默认勾选，URL 改动清除访问错误，dispose 取消检查；名称已有值时优先聚焦 URL。
注意 `withOKEnabled()` 不能单独解释为“再次确定就跳过连接检查”：当前读取的
`doOKAction` 每次仍会调用 `checkUrlAccess`，没有验证到无条件绕过检查的执行路径。

本地 `remote_management::validate_edit` 提供只读预检查；实际 execute 仍在操作锁内
重做同一检查，防止表单打开后其他工具修改配置。修复 check_access 只启动会话、
却没有设置 GIT_ASKPASS 的遗漏，显式传入并验证 helper 路径，与既有 Fetch 一致。
`tests/fetch_auth.rs` 增加本地 HTTP 认证成功／拒绝／关闭认证三条真实 Git 测试，
验证 refs 查询成功、取消传递、无配置／引用／工作区写入；三条均通过。

新增 `src/define_remote_dialog.rs` 为独立表单，包含默认 Fetch、异步校验、认证弹窗、
busy 状态、取消与输入保留。尚未接入远端管理列表／写操作回调，未做原生视觉验收，
因此不算第 4 阶段可见功能交付；编译时的未使用警告来自待接入表单，不予掩盖。
剩余为管理列表和操作生命周期接线、Clone、Windows 原生验收；本阶段尚不提交。

### MVP 第 4 阶段：远端管理列表接线（继续进行）

本轮进一步读取同版本 `GitConfigureRemotesDialog.kt::editRemote/doFetch/changeRemote/runInModalTask`：
无变化编辑不触发写入或 Fetch；Fetch 刷新后匹配远端名称和 URL；rename 成功但 set-url
失败时不能假装整体回滚。本地 `src/remotes_dialog.rs` 提供名称／URL 表格、新增／编辑／
删除、双击编辑和删除确认；URL 展示隐藏 HTTP 凭据和查询参数，编辑表单保留真实值。
`src/ui/remote_management.rs` 接入 Git 菜单和 ConfigureRemote 操作生命周期，所有结果
重新读取配置；有真实命令且成功、名称与 URL 仍匹配时，调用 `start_fetch_named`，只取
该远端。暂保留差异：单仓库表格、文本 Edit 按钮与加减字符图标，尚待 Windows 视觉核对。

远端集成测试 8 条通过；桌面编译无警告；Windows release 构建通过（44.40 秒，随后
增加了无变化编辑不 Fetch 和 URL 匹配检查，后续原生测试须重新构建包含这两项）。
列表／表单现在有实际入口，不再是上一记录中的未使用组件；原生交互仍待验收。
第 4 阶段尚缺 Clone，未提交；第 5 阶段未开始。

### MVP 第 4 阶段：Clone 后端与认证（进行中）

实际读取同参考版本 `GitCloneDialogComponent.kt`、`GitCloneUtils.kt`、
`GitCheckoutProvider.java::clone`、`GitImpl.java::clone`，以及平台
`CloneDvcsValidationUtils.java`、`CloneDvcsValidationUtilsTest.kt` 和
`CloneableProjectsService.kt::runCloneTask/cancelClone/removeCloneableProject`。
规则：目标目录可不存在或为空；创建父目录后克隆；显式进度；可选 depth；成功后才通知
打开项目；取消是独立结果，不等于成功，也不等于目录已清理。读取到的服务层没有额外
递归删除目录的流程。URL 接受 HTTP／SSH／本地路径，去掉粘贴的 `git clone ` 前缀。

本地新增 `src/clone_repository.rs`：参数直接传 Git，不经 shell；复用进程树取消与有界
进度尾部、AskPass；Windows 加 core.longpaths=true。父目录创建后再次校验目标为空。
支持 depth 和可选递归子模块；成功判定同时检查退出码、停止原因和输入错误。失败时
保留 Git 实际留下的内容，报告 destination_exists，应用不额外删除；UI 尚待接入。
保留差异：拒绝作为目标的符号链接；`--` 放在源地址前，同时阻止参数形式地址，理由
是避免用户粘贴文本成为 Git 选项；具体项目选择／打开策略复用现有窗口流程。

`tests/clone_repository.rs` 6 条测试通过：普通克隆及 tracking、空目录／非空保护、
提前取消不创建父目录、失败不删除同级数据、浅克隆 depth、URL 形式。HTTP 认证测试
复用本地服务和真正 AskPass helper，覆盖克隆成功／拒绝／关闭认证，不访问外部远端。
这些仅证明后端，不能作为 Clone 界面或第 4 阶段完整验收；仍需 UI 和 Windows 验证。

### MVP 第 4 阶段：Clone 界面接线与 Windows 路径问题

新增 `src/clone_dialog.rs`／`src/ui/clone.rs`，欢迎页和 Git 菜单均有 Clone 入口；表单
URL、目标目录和文件夹选择、浅克隆／depth、子模块选项；执行时禁用输入并显示有界进度。
关闭弹窗请求取消，操作所有权保留到进程真正结束；成功且窗口仍打开才调用现有
choose_project_window。失败保留输入和真实残留目录提示。继续参考同版本
DvcsCloneDialogComponent 的 URL／Directory 行和首个 URL 聚焦。保留待办：URL 推导
默认目标目录、记忆目录，尚未实现，不宣称完整视觉对齐。

Windows 构建通过（55.29 秒），原生欢迎页和 Clone 表单已截图。首次本地源克隆失败，
显示 fatal: Could not read from remote repository，目标没有创建。最小宿主 Git 对照
确认普通 C:\\ 路径 exit=0，相同路径加 Rust canonicalize 返回的 verbatim 前缀
\\\\?\\ 后 exit=128，stderr 为 hostname contains invalid characters。
本地 `clone_repository::git_local_path` 转回普通盘符／UNC 表达（保留 canonical 结果），
Windows core.longpaths=true 继续保留。正在重跑原始原生流程，不将编译通过当成修复验收。

修复后原生复验通过：自有进程 PID 27624，欢迎页填写同一个本地 source 和 destination，
点击 Clone 后成功打开 destination 项目，Log 显示 Clone fixture、hello.txt、main 和
origin/main。源与目标 HEAD 均为 c7f90f91406020ec49f48afde1700aa5d506626e；原始失败
同条件已不复现。截图 `artifacts/clone-mvp-20261021/dialog.png`、
`windows-path-failure-before.png`、`opened-after-clone.png`。复验前 6 条克隆集成测试
通过，新 Windows release 构建通过（55.29 秒）。只关闭本次启动的 Plico-clone.exe。
仍待：远端管理完整原生交互、Clone 取消／失败重试、默认路径与相关配置引导的阶段审计。

### MVP 第 4 阶段：远端管理 Windows 原生验证

自有 Plico-clone.exe PID 36644，使用前一轮 Clone 创建的 destination 临时仓库。
Git 菜单进入 Git Remotes 后显示 origin；新增 audit，URL 指向同一临时 source，
保留默认 Fetch，完成后真实 refs/remotes/audit/main 指向源 HEAD。
选择 audit 后点击 Edit，改名为 audit-renamed，完成后对应远端引用迁移。
点击删除先取消：Git remote 仍有 audit-renamed 和 origin；再次删除并确认：
Git remote 只剩 origin，旧 audit-renamed/main 引用不存在（show-ref exit=1）。
HEAD 始终为 c7f90f91406020ec49f48afde1700aa5d506626e，status --porcelain 始终为空。
新增／编辑执行时显示 Updating remote configuration，结束后列表重读，Log 引用同步。
截图位于 `artifacts/remotes-mvp-20261021/`。仅验证工具栏编辑入口，双击入口未原生验证。

原生核对发现远端 Name 复用了作者“姓名”翻译，Edit／Remove 缺中文；现已分别改为
Remote name（名称）、编辑、删除。截图来自改文案前构建，待最后构建复核文案。
继续读取 ClonePathProvider.java 和 FilePathDocumentChildPathHandle.kt：默认父目录用
上次克隆路径；根据 URL 最末段推导子目录，用户修改目录后停止自动覆盖。这部分仍待接入。
阶段尚未提交；下一步补 Clone 默认目录行为及剩余原生取消／重试，然后做阶段完整审计。

### MVP 第 4 阶段：Clone 默认目录行为

按前轮实际读取的 ClonePathProvider／FilePathDocumentChildPathHandle 接入
`src/clone_destination.rs` 和 Clone 表单 URL／目录事件。默认父目录取上次有效克隆请求
的父目录，无记录则用用户主目录（与 IDE 的 projects base dir 保留差异）；URL 的末段
去 `.git` 并解码后建议子目录，拒绝解码后包含路径分隔符／控制字符的名称。
一旦手动改目录或使用选择器，就停止后续 URL 自动覆盖；不扫描目录／仓库。
只保存目录，不保存可能包含凭据的 URL 历史；读取最多 32 KiB，拒绝相对路径和损坏数据。
配置写失败不阻断实际克隆。3 条模型测试通过，桌面编译通过；原生事件与记忆回读仍待核对。

### MVP 第 4 阶段：取消、重试、目录记忆与最终构建复核

继续沿用上述已读 Rebased 4aec352 的 Clone 生命周期与目录建议规则。
原生取消验证使用独立 loopback TCP fixture：Git 停止后表单恢复可编辑，保留输入，
明确提示残留目录，不自动删除。更改 URL 后手动目录保持不变；换到有效本地源和新目录
retry 后成功打开，源与目标 HEAD 均为 c7f90f91406020ec49f48afde1700aa5d506626e。
截图：artifacts/clone-mvp-20261021/clone-cancelled.png、
clone-manual-directory-preserved.png、clone-retry-opened.png。

最终 Windows 构建（43.00 秒）在自有 PID 25212 上复核：重新启动后读取上次父目录，
URL 推导 repo 子目录；执行中只有底部一个 Cancel，点击后显示已取消和真实残留目录，
恢复 Clone／Cancel 按钮及输入框。截图 clone-remembered-parent.png、
clone-single-cancel.png、clone-single-cancelled.png 位于同一目录。测试 app 和本地慢速
HTTP fixture 均已按进程路径关闭，未操作用户程序。

本地 src/ui/clone.rs 将失败后的目录存在性检查移到后台（优先使用后端 Report），
避免网络／WSL 文件系统元数据查询阻塞 UI。36 项定向测试通过：Clone 6、HTTP 认证14、
身份5、远端8、目录建议3；桌面 cargo check 通过，完整核心回归另行运行。
第 4 阶段仍未提交；提交前需完成完整回归结果及剩余阶段审计，不将像素级一致性视为已证明。

阶段提交验收：完整 `cargo test --locked --no-default-features` exit=0。
功能范围由 Clone 后端／表单／原生成功、取消、重试，远端 CRUD／自动 Fetch 原生流程，
身份本地／隔离全局配置测试及原生配置后提交覆盖。源码依据与保留差异见以上记录。
未原生逐项覆盖：远端双击编辑、最新远端中文标签、Amend／按块／Commit and Push 各自
触发身份弹窗的组合；后三者接入共享提交前置检查，不能把共享代码当作独立原生验收。
精确图标／间距仍保留差异，不宣称像素级还原。第 4 阶段功能交付完成，进入提交。

### 前四阶段 UI／i18n 收尾（用户收口范围，2026-10-21）

用户明确本次 goal 只完成前四阶段与 UI 对齐，第五阶段不启动。
参考仍为 Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd；安装版本为
Rebased 1.1.20 / IC-262.10968.SNAPSHOT，与源码快照不等同。本轮实际读取：
GitConfigureRemotesDialog.kt 的 createActions/createCenterPanel/updateTableWidth，
GitDefineRemoteDialog.kt 的 createCenterPanel/getPreferredFocusedComponent，
GitUserNameNotDefinedDialog.kt 的身份字段，DvcsCloneDialogComponent.kt 的表单和
isOkActionEnabled，GitCloneDialogComponent.kt 与 GitShallowCloneViewModel.kt。

提取规则与映射：远端管理 Add/Remove/Edit 顺序，单 OK，名称／URL 对齐的表格，
行数随远端数量增长并封顶；src/remotes_dialog.rs 使用真实上游 add/remove SVG、
已有 edit SVG，表格共享列宽及边框。src/clone_dialog.rs 使用内嵌目录浏览图标、
常驻但条件禁用的 depth 字段及 commits 单位；URL 为空禁用 Clone，输入事件即时刷新。
定义远端和身份表单标签与输入垂直居中。远端语境 Remote name 在中英文分别为
“名称”／“Name”，作者 Name 保持“姓名”／“Name”；回归测试防止再次混用。
补齐 Clone 校验错误、Delete、File 中文；Git 原始诊断及仓库数据保持原文。

保留差异：单仓库对话框固定宽度、超长 URL 截断；不复刻尚无功能的托管账户／插件入口，
不存储可能包含凭据的 URL 历史；保留已有真实子模块选项。Clone 使用应用内弹窗，
不宣称与 IDE 独立窗口像素一致。用户截图用于结构对照，本轮没有操作用户 Rebased 窗口。

最终验证：桌面 check 无警告；名称上下文翻译回归通过；Windows release 构建通过
（最后一轮 41.08 秒）。对四阶段修改文件进行静态 t("literal") 翻译键核对，无遗漏；
这不是动态 Git 诊断全量翻译的声明。最终 Windows 自有 PID 26524（en）、12440（zh-CN）
核对 Git Remotes 与 Clone，截图 artifacts/setup-ui-20261021/ui-final-{remotes,clone}-{en,zh-CN}.png。
确认名称／Name、工具栏顺序、列起点、OK 宽度、目录轮廓图标、禁用 depth 及空 URL
禁用 Clone，两种语言均无本次表单文字溢出。测试进程已关闭，配置仅修改独立测试 profile。
前四阶段提交 f77f7dc、3821fcd、0a2a0e6、6939478，完整核心回归已在第4阶段收尾通过；
本轮仅 UI／翻译变化。按用户最新收口要求，本次目标完成，不启动第五阶段。

## 外部删除文件后 Diff 无法关闭（2026-10-22）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，本轮实际读取
`platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/EditorWindow.kt`
的 `closeFile`，以及同目录 `FileDocumentManagerImpl.java` 的
`isDocumentUnsaved`、`doSaveDocument`、`doSaveDocumentInWriteAction`、`fileDeleted`。
安装版仍为 Rebased 1.1.20 / IC-262.10968.SNAPSHOT，与参考快照不同。
规则：关闭基于 editor composite 生命周期，不要求磁盘内容可读；文件删除解除文件／
文档绑定并通知监听者；无效文件不应成为持续待保存义务。读取失败与未保存编辑分开。

本地根因：`Editing::pending` 将任何 error 算作未保存，外部删除后的只读错误
因此进入强引用待保存集合；关闭 Tab 与仓库操作经过保存屏障后又被同一错误拦截。
`src/editing.rs` 改为只判断真实草稿、写入、重载和无效输入；
`src/ui/operation.rs` 只用仍有待保存内容的文档错误阻止后续操作。
保留差异：不照搬 IDE 对无效文件的丢弃策略；Plico 的未保存草稿仍保留，
外部删除后禁止自动重建文件，错误横幅和显式重载入口保留。

回归：关闭按钮／关闭菜单共用入口测试先失败再通过；外部删除 clean document
不再 pending、不启动写入且关闭后释放引用；未保存草稿、IME preedit、外部冲突
仍受保护；读取错误不阻断后续操作，真实保存失败释放锁但不执行后续操作。
桌面相关测试共 13 项通过（editor_tabs 3、editing 7、operation 3）。
Windows release 构建通过（42.58 秒）。整窗线程卡死尚未复现，不能将本次已确认的
保存屏障问题直接等同于用户补充的整窗卡死；原构建的错误页仍能打开菜单和切换日志。

Windows 原生复验：独立临时仓库、独立 profile；旧构建 PID 33896 出现删除后
读取错误且 X 关闭无效，但日志／菜单仍可响应；新构建 PID 41268 同一流程
（打开 untracked Diff → 外部删除 → 切日志 → 返回错误页 → 点击 X）可关闭，
进程 Responding=True，确认磁盘文件未重建。截图位于
`artifacts/deleted-diff-20261022/{before-error,before-responsive-menu,after-error,after-closed}.png`。
右键关闭的共同动作入口由回归测试覆盖，未单独做新构建右键菜单原生点击验收。
只关闭本次自有测试进程；没有修改用户仓库、关闭用户 Rebased 或操作发布流水线。

2026-10-22 架构整理沿用上节已读源码：文档保存判断收进 `editing::SaveBatch`，
调用方按 Saving／Ready／Blocked 处理，保留既有文档强引用、锁释放与草稿保护行为。
没有改变 Tab／Diff 的视觉、导航或磁盘操作规则。其余自有 Rust 改动为 rustfmt 排版，
vendor 上游代码不参与批量格式化。

## 主 Log 请求生命周期整理（2026-10-22）

实际读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的
`platform/vcs-log/impl/src/com/intellij/vcs/log/visible/VisiblePackRefresherImpl.java`：
构造函数以 SingleTaskController 集中请求和完成状态；过滤／索引完成可取消运行中任务；
MyTask.run 对取消不提交新 State；computeState 选择最新过滤请求并区分加载更多、
刷新和有效性变化。安装版仍为 Rebased 1.1.20 / IC-262.10968.SNAPSHOT，非同一快照。

提取的是请求控制与界面显示分离、取消不发布旧结果的规则；本地对应
`src/ui/log_query.rs` 和 repository/log_selection/log_branches 的调用入口。
保留 Plico 原有单个替换请求策略，不复制 IDE 的索引队列和永久图缓存。
History／Details 控制读取反馈，Navigation 保留旧比较并使用既有定位提示。
进入 Diff 的显式取消清除加载状态；无新增控件或图标，不改变布局。

验证：21 项相关桌面测试与 Windows release 构建通过；原生截图黑屏保护使交互
核对未完成，不能将测试通过等同于原生视觉验收。此轮未改布局或做滚动自动化。

2026-10-22 原生补测已通过：对真实 Git log 仅在独立测试进程中加 8 秒延迟，
在读取中切换 Diff 后返回日志可再次刷新，加载状态正常结束；同一新构建外部删除
文件后错误 Diff 可关闭且不重建文件。截图与时序见 ARCHITECTURE.md 的原生补测记录
及 `artifacts/log-query-acceptance-20261022/`。此前黑屏导致的相关交互待验收已补齐。

### 现有行为核对第一轮：内置文本三方合并（2026-10-22）

本轮范围：用户要求修正既有行为，而非继续扩展功能；内置三方合并是明确例外。先收敛冲突处理这一条完整路径，未将此轮记作全产品行为对齐完成。

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`；已安装 Rebased 1.1.20 / IC-262.10968.SNAPSHOT，与源码快照版本不同。实际读取：

- `platform/diff-impl/src/com/intellij/diff/merge/TextMergeViewer.kt`：左右原始内容只读，中间替换为输出；关闭有编辑结果须确认。
- 同目录 `MergeConflictModel.kt::rediff/setInitialOutputContent/replaceChange`、`MergeThreesideViewer.java::handleResolvedCase/handleCancelCase`：输出从共同祖先初始化；逐块选择／保留、应用非冲突更改；存在未解决块时应用须确认，取消不作为解决。
- `platform/diff-impl/src/com/intellij/diff/requests/TextMergeRequestImpl.java::applyResult`：取消恢复原始输出，接受侧／结果仅在明确确认后提交。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/merge/MultipleFileMergeDialog.kt`、`flow/OneShotMergeFlowDelegate.kt`：双击文件进入合并，Merge 为默认合并入口；取消返回文件列表，非取消结果才保存并标记解决。
- `plugins/git4idea/backend/src/merge/GitMergeProvider.java::MyMergeSession`：展示侧经反转语义映射后解析；Merged 最终走 markConflictResolved。已有 rebase／自动恢复 stash 反转实现保留，未改为固定 stage2=Yours。

本地实现：`text_merge.rs` 持有共同祖先差异块及结果范围，`ui/text_merge.rs` 持有一次合并草稿，`external_merge::Prepared` 复用隔离输入／输出和工作区字节基线；`ui/external_conflicts.rs::apply_merge_result` 共用写回、标记、失败保留和刷新流程。两侧只读，中间独立编辑；自动应用不覆盖已手工修改块，逐块动作进入编辑器撤销栈。全范围手工改写跨越多个块时禁用有歧义的块替换，仍可手工编辑并标记完成；未解决直接应用和放弃修改各有明确确认。写回期间禁止退出／修改合并结果，失败保留编辑窗口及恢复文件。取消完全不写工作区，原文件被外部修改或 index 身份变化均拒绝旧结果。

本轮已有行为修正：冲突列表双击进入合并，合并取消回到列表；失败不因关闭窗口丢掉结果。解决成功共用工作树、Log 和冲突列表刷新，未自动继续整个 merge/rebase。

明确差异：本地仅文本 UTF-8（包括 BOM），沿用既有 5 MiB 文档限额，过大／二进制明确报错，可继续接受整侧或用已有外部工具；符号链接及 gitlink 不进入文本合并。保留原工作区 EOL/BOM。没有复制 IDE 语言专用自动合并、迭代解决模型、多文件自动推进；三栏块高亮／工具栏选择代替中缝逐块箭头，尚无三栏连线及滚动映射；下一处定位同时移动三栏。相邻改动无法可靠分离时保守合并为冲突块，不猜测解决。布局及交互做结构核对，不宣称像素级还原。外部工具作为可选入口保留。

验证：新增纯模型测试涵盖双方重建、Unicode、插入／删除、add/add、无尾换行、手工编辑后范围变动、自动应用不覆盖手写；8种输入的三方组合全部验证选择每侧能还原该侧。真实 Git conflicts 集成7项通过，包括新增隔离取消、CRLF/BOM、外部修改拒绝、写回不等于 stage、stage 后旧快照拒绝。既有21项 UI 回归通过。本轮新增界面 Windows 原生验收记录待补。

原生验收补记：Windows 独立 fixture `Temp/plico-native-merge-8a50f740468f4a1a87272a77dc62f915`，自有 PID12440 验证基线结果、单块接受、放弃确认及返回冲突列表，文件未变。原生验收发现 F7 在工具栏操作后因焦点留在弹窗层未生效；修正为打开合并和逐块操作后聚焦结果编辑器，并在合并视图接收 F7／Shift+F7，避免调用主窗口 Diff 导航。自有 PID38872 复验“首块接受你的版本 → F7 → 第二块接受对方版本 → 应用”，工作区精确得到 `title\nyours-first\nseparator\ntheirs-second\nend\n`，Git `ls-files -u` 为空、`status --short` 为 `M  review.txt`；冲突列表为空态，顶部显示完成合并，不自动创建合并提交。截图位于 `artifacts/native-merge-20261022/`。取消截图来自修正漏译前的版本，后续版本已补全 Modified、Deleted、Merge、Discard 和导航标签的中英文映射。

继续补读同一 commit 的 `MergeModelBase.kt::appendChange`：按行追加而不是无换行的字符串粘接。本地接受双方对相同／单侧改动只保留一次，对双方无尾换行内容插入行分隔；新增回归通过。范围构造改为有序遍历编辑及预计算行偏移，避免每个冲突块重扫整份文件；改后全部6项模型测试通过，包括三方组合重建。完整核心测试607项通过，随后新增“接受双方”测试单独通过；21项现有UI回归通过；最终Windows release构建通过（56.89秒）。最后的内部范围构造优化由模型测试覆盖，未重复完整原生点击流程。滚动联动未实施／未自动验收。只关闭自有测试进程，用户Rebased／IDEA未动。


## 三方合并块操作：实现前实机核对（2026-10-22）

参考源码仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，宿主机安装版 Rebased 1.1.20 / IC-262.10968.SNAPSHOT，二者不是同一版本。使用独立配置 rebased-branch-profile，通过已读 `platform/diff-impl/src/com/intellij/diff/applications/MergeApplication.kt` 的 `merge yours theirs base result` 入口打开独立文件夹 reference-merge-fixture；未操作用户 PID 27100 的仓库。三个输入含两个分离的同位冲突。该轮验证的是共享文本三方合并 viewer，Git 冲突列表入口沿用上一轮源码核对，尚未作本轮 Rebased Git 入口截图验收。

截图在 `artifacts/merge-alignment-20261022/`：`reference-merge-initial.png`、`reference-merge-left.png`、`reference-merge-both.png`、`reference-merge-ignore-left.png`、`reference-merge-ignore-both.png`、`reference-merge-right-first.png`、`reference-merge-cancel.png`。实际观察：左／中／右三列分别只读 Yours、可编辑 Result、只读 Theirs；左右行号与每块的叉号／箭头靠近结果区。左边叉号在箭头左边，右边镜像；初始结果为 base。采纳左侧后只隐藏左侧操作，右侧箭头变为追加；再采纳右侧，结果按点击顺序包含 yours-first、theirs-first。第二块两侧分别忽略后保留 base 的 second，直到两侧都处理才减少未解决数。Apply 写出的完整结果已检查为 title/yours-first/theirs-first/separator/second/end。反向先采纳右侧亦只改变右侧状态；取消弹出丢弃结果确认，确认后独立输出恢复原内容。

已读同目录 `MergeConflictModel.kt::{replaceChange,ignoreChange}`、`TextMergeChange.kt`、`ThreesideMergeHighlighters.kt`：每块左右各有 resolved 状态；首个冲突采纳替换 base，后续采纳 append；忽略不改文本；普通非冲突或 Ctrl-click 直接解决整块；另一侧为空的采纳直接解决；单侧 resolved 后移除该侧按钮；图标对应 Diff.Arrow/ArrowRight、ArrowLeftDown/ArrowRightDown、Remove，Ctrl-click 提示由 renderer 提供。`MergeThreesideViewer.java` 保留取消确认与未解决 Apply 确认。此前整块 Accept Yours/Theirs/Both/Base 工具栏不能替代这些交互。

实施映射：`src/text_merge.rs` 改为独立两侧决策；`src/ui/text_merge/` 承载同帧几何块操作和连接区，主视图保留生命周期与导航；外部工具专用配置／执行／入口移除，隔离输出与快照复验迁入内置合并模块。保留 UTF-8／大小限制及符号链接、子模块的专门处理边界。实现后的 Windows 对照、失败／取消及未验证差异继续记录，不将截图读取等同于完成实现。


### 本地实现与验收

`src/text_merge.rs` 管理独立的左右处理状态、首个替换／随后按操作顺序追加、忽略不修改文本、Ctrl 整块处理，以及删除侧为空时的终结规则。`src/ui/text_merge.rs` 管理三个编辑器、导航和确认；`src/ui/text_merge/gutter.rs` 在编辑器本帧布局后绘制靠近结果区的行号、曲线连接和左右镜像按钮；处理后该侧按钮消失，另一侧切换追加图标。导航改用紧凑上下箭头；没有可自动处理项时禁用应用非冲突更改。同步滚动复用已有双侧 diff 的行范围映射与 peer offset（不递归发送滚动事件）；本轮不做滚动自动化，长文件滚动体感留作手测。图标进一步核对 `DiffUtil.java::{getArrowIcon,getArrowDownIcon}`，对应原生 SVG，来源记录在 assets/icons/SOURCE.txt。

外部工具入口、参数解析、设置读写、进程启动及确认界面已删除。`src/merge_result.rs` 仅保留隔离输入／结果、工作文件基线复验、编码与换行保存和失败结果保留；`src/ui/merge_result.rs` 承担写回后显式标记解决及刷新。历史遗留的用户配置文件不主动删除，也不再读取。符号链接、子模块、非文本及超限内容继续使用已有冲突列表的选边／编辑文件处理，不偷偷启动外部工具。

Windows 原生测试仓库 `C:\Users\echoyn\AppData\Local\Temp\plico-native-merge-ef5009881a6244758927d8224a43c0c5`，测试进程 PID12380／41008。首次原生检查发现绝对定位的绘制层缺少 inset，修正后复验。截图同目录 `aligned-merge-list.png`（没有外部工具入口）、`aligned-merge-initial.png`、`aligned-merge-left.png`、`aligned-merge-both.png`、`aligned-merge-ignore-left.png`、`aligned-merge-ignore-both.png`、`aligned-merge-cancel.png`、`aligned-merge-cancelled.png`、`aligned-merge-right-first.png`、`aligned-merge-reverse-both.png`、`aligned-merge-f7.png`、`aligned-merge-ready.png`、`aligned-merge-complete.png`。

实际点击通过：先左后右按顺序追加；两侧忽略第二块保留 base；取消二次确认后工作区仍为原始 Git 冲突标记内容；重新打开后先右后左，F7 保持在合并视图并选择第二块，再忽略两侧并应用。最终文件字节为 `title\ntheirs-first\nyours-first\nseparator\nsecond\nend\n`；异步写回／stage 完成后 `ls-files -u` 为空，`status --short` 为 `M  review.txt`，冲突列表显示全部解决，顶部出现完成合并入口，无自动合并提交。测试中的首次取消断言误将 Git 合并在一起的冲突标记预期成两个块；检查实际 Git 生成的单个标记块后修正验收预期，未为此修改产品逻辑。

验证：核心回归 608 项、现有 UI 回归 21 项通过；其中 8 项三方模型测试覆盖采纳顺序、忽略、Ctrl、修改／删除、手工编辑与无尾换行；7 项真实 Git 冲突测试覆盖隔离输出、陈旧文件／快照、BOM／CRLF 和工作区未受影响。格式检查与 Windows release 构建通过。Clippy 完成，仓库既有警告仍存在；新增模型的 collapsible_if 警告已修正。

保留差异／尚未验收：合并仍是应用内大弹窗而非独立系统窗口；本轮聚焦逐块采纳／忽略，不新增 Rebased 全套设置及底部整文件 Accept Left/Right 快捷入口（冲突列表已有选边）；已解决连接轮廓当时为实线（下文颜色与细线修正已替换）；块按钮尚无悬停说明；文本编辑撤销保留，但忽略状态尚未与编辑器 Undo 事务一起回放，不能宣称完整三方合并行为已逐项对齐。源码核对过的 Ctrl 操作通过模型测试，尚未作宿主机 Ctrl+点击截图；故本轮完成上述核心交互和外部工具移除，不宣称像素级完全一致。

### 三方合并类型颜色与已解决细线：实现前核对（2026-10-22）

同一参考 commit、安装版本。独立 reference-merge-colors fixture 包含左侧删除、左侧修改、右侧新增及双方冲突；在原生 Rebased 逐项采纳前三项。截图 reference-colors-initial/delete/modified/inserted.png 保存于 artifacts/merge-alignment-20261022。观察：新增绿色、删除灰色、修改蓝色、冲突棕色；未修改的源侧不绘制高亮。采纳后背景消失，同色点状上下边线贯穿文本、行号与连接曲线，空区间保留一条较粗的点线。

源码依据：platform/util/diff/src/com/intellij/diff/util/MergeRangeUtil.kt::getMergeType 分类并标记实际变化侧；platform/diff-impl/src/com/intellij/diff/tools/simple/DiffViewerHighlighters.kt 仅高亮变化的源侧及结果，已解决侧不再内联高亮；util/DiffDrawUtil.java::LineHighlighterBuilder/ PaintMode.RESOLVED 明确 NONE 背景、DOTTED 边线，空范围 doubleLine。准备映射到 text_merge::Change 的语义类型及 ui/text_merge/gutter.rs 的当前帧绘制；修正现有全部非冲突绿色、未变化侧着色和仅连接区实线轮廓的问题。保留本地整行高亮，词级强调仍未复制。


实现与原生复验：Change::kind/changed 按三方内容分类，已解决后保留原类型；编辑器不再为已解决／空范围／未修改源侧建立背景装饰。gutter 以当前帧范围画类型色连接、空区间细线与已解决点线，结果边线单独绘制，避免双方重复绘制。单侧已解决时仅该源侧和对应连接转为点线，中间继续高亮直至整个块完成。

Windows 独立真实 Git fixture `Temp/plico-native-merge-71c48b9afa774230b41956f65089090b`、自有 PID29788：依次采纳删除、修改、新增，再采纳冲突左侧。已观察各类型颜色、空侧细线、完成后无填充而保留贯穿点线、未变化侧无高亮、半解决冲突仍保留结果及另一侧背景。截图 `artifacts/merge-alignment-20261022/colors-initial/delete/modified/inserted/half-conflict.png`。未点击应用，测试草稿不写回工作区。源码与实机对应规则已修正；本地仍为整行填充，Rebased 的词级强调、线条抗锯齿及精确像素粗细未宣称完全一致。

9 项三方模型测试通过（新增分类覆盖单侧／双方新增删除、相同修改及删除-修改冲突，解决后类型不变）；Windows release 构建通过（53.81 秒），格式检查通过。原有本轮完整核心／写回回归结果见上文。

颜色及边线调整后，21 项桌面 UI 回归通过。


## 现有功能独立逻辑复核（2026-10-22，基线 eceaca9）

本轮为行为审计，不扩展功能、不修改产品代码。参考仍为 4aec35256d765c217aa0d662b82ac4e860c396fd；安装版 Rebased 1.1.20 / IC-262.10968.SNAPSHOT 不等同源码快照。重点读取本地三方合并、checkout、reset、rollback、stash、push 重试、提交 index 恢复、reword 检查及刷新入口。不是所有菜单／失败组合的全量原生验收。

确认问题，按修复优先级：

1. 三方合并手工编辑会重置两侧决策及 append 状态。src/text_merge.rs::edited 对触及块无条件 sides=false、appended=false、resolved=false；先接受左侧、手改为 left edited、再接受右侧，实际只剩 right。上游 MergeModelBase::processDocumentChange 保留／存储块状态，MergeConflictModel::replaceChange 根据 isOnesideAppliedConflict 追加，并非每次文本变更清空决策。本地临时模型用例已复现；上游半解决后手工编辑的完整键鼠序列仍须修复前单独复验（此次后续实机点击直接完成了整个冲突，不能用该截图证明半解决语义）。
2. Undo/Redo 只回放文本，缺少合并决策事务。上游 MergeModelBase.kt::executeMergeCommand/registerUndoRedo 与 TextMergeChange.kt::storeState/restoreState 保存双方 resolved、onesideAppliedConflict、范围；本地 ignore 不产生编辑器事务，编辑回放只经 edited 重算，重做不能恢复已采纳侧。模型用例确认文本 undo/redo 后 sides=false、appended=false。原生独立 PID21752 在 reference-merge-colors 中忽略删除块后计数 3 changes→2 changes，Ctrl+Z 回到 3，按钮及灰色背景恢复；截图 artifacts/behavior-audit-20261022/audit-reference-initial/ignore/undo.png。未写回 reference 输出。
3. 应用合并结果后暂存失败的重试不是幂等。ui/merge_result.rs 先 Prepared::apply 再 conflicts::resolve；apply 未更新 working-baseline。真实临时 Git 仓库用 .git/index.lock 阻止暂存，解除锁后再次 apply 返回 Conflict，尽管磁盘是自己刚保存的结果。保留输出避免数据丢失，但同一弹窗的再次应用无法完成。应区分尚未写回／已经写回待暂存，而非取消基线保护。上游 MultipleFileMergeDialog 的文档保存与 processedFiles/mergeSession 完成处理独立；此项首先是本地失败恢复缺陷，未声称已在原生 Rebased 人为锁 index 对照。
4. 远端分支／标签 checkout 绕过共用覆盖冲突恢复。src/ui/reference_checkout.rs::execute_reference_checkout 仅展示失败 stderr；本地分支／指定 revision 则通过 ui/checkout.rs 提供 Smart Checkout/Force。上游 GitCheckoutAction.kt 的非远端与 GitRemoteBranchesUtil 的远端操作最终经 GitCheckoutOperation::runCheckoutAndHandleCommonFailures，均检测本地修改并进入 smartCheckoutOrNotify。本项已确认源码调用分叉，尚未做本轮原生覆盖冲突截图，不作为 UI 修复已验收。

其余核对：GitRollbackEnvironment 的 unindex/delete/revert HEAD 顺序与本地 rollback_operation 基本一致；GitStashUtils::createUnstashHandler 的 apply/pop/--index 与 stash_operation 对应；GitPushOperation 的 10 次上限、force/非跟踪目标不自动更新，与 push_recovery 对应。GitCheckinEnvironment 的暂存区保存/恢复及 GitRewordAction 的 HEAD merge 例外在本地有对应路径。本轮未发现这些已读路径的新明确错误，但不代表全部边界已证明正确。

独立实现的保留差异不混入缺陷：单仓库；保护分支默认 main/master 而无自定义规则；内置文本合并 UTF-8/5MiB；refs 5 秒轮询及远端 300 秒检查（GitRepositoryUpdater 上游走 VFS 事件）；Push 冲突解决后不自动重试发送。Reset 的 Keep 覆盖失败缺少上游 SmartResetPolicy.PROPOSE 恢复流程，列为后续对照候选，尚无本轮实机复验。

证据：3 个临时诊断用例成功复现以上状态与失败路径，日志、可还原测试源保存在 artifacts/behavior-audit-20261022；这些断言描述现有错误行为，不充当修复回归。临时 tests/_behavior_audit.rs 已移除。未重新跑无改动的全仓测试，未修改用户仓库。

### 四项偏差修复：合并决策的实现前复验（2026-10-22）

同一参考 commit 4aec352、安装版 1.1.20。独立 reference-merge-fixture、自有 Rebased PID42040：先点击左侧首块采纳，再在结果末尾输入 123，再点击右侧追加。结果为 yours-first123\ntheirs-first，首块转为点线且冲突数从 2 降为 1。截图 artifacts/behavior-fixes-20261022/fix-ref-initial/left/edited/appended.png。结合上一轮 ignore→Ctrl+Z 的实机记录，确认手工编辑保留决策，撤销须同时恢复块范围、双方状态和追加标志。重新读取 MergeModelBase.kt::processDocumentChange/executeMergeCommand/registerUndoRedo、TextMergeChange.kt::storeState/restoreState。准备将轻量块状态接入现有编辑器事务，保留连续输入和 IME 的事务合并，不建立第二套全文撤销栈；映射 text_merge.rs、ui/text_merge.rs 和编辑器 UndoManager。

Checkout 前置实机：独立 fix-checkout-fixture / PID40992，在 main 上修改 review.txt，再从分支弹窗 Tags→review-tag→Checkout。原生展示 Git Checkout Problem、受影响文件、Force Checkout / Smart Checkout / Don't Checkout。截图 fix-ref-tags-expanded/tag-menu/checkout-problem.png。已读取 GitCheckoutOperation.java::runCheckoutAndHandleCommonFailures/smartCheckoutOrNotify/checkoutOrNotify：本地覆盖进入共用保留流程，force 仅显式触发；unmerged/untracked 有独立失败分类。准备把远端／标签的原始 Prepared 与 Choice 交给共同恢复入口，保留 HEAD/ref/upstream 复验和跟踪分支创建语义；不把远端当成本地短名，不把重放本地提交的 Rebase 当作强制 checkout。

写回／暂存前置实机：同一独立仓库创建真实 main/target 冲突，在原生冲突列表 Resolve Manually 中采纳 main 并忽略 target，创建测试 index.lock 后 Apply Changes。工作文件已保存为 main change，而 ls-files -u 仍有三阶段；关闭冲突列表触发暂存，原生显示 IDE error occurred，idea.log 明确 GitMergeProvider→markConflictResolved→git add 因 index.lock 失败。截图 fix-ref-real-merge-ready/real-resolved/apply-locked/stage-locked.png。随后只删除测试创建的锁。上游安装版使用迭代解决，保存与暂存的分离与 MultipleFileMergeDialog.kt::saveDocument/finishResolution/markFilesProcessed 相符，但其错误提示并非期望复制的行为。本地保留当前失败弹窗与草稿，仅修正 Prepared 已成功写回后的基线；用固定的已写入快照支持重试，不放宽外部编辑／index 身份检查。

### 四项修复的实现与回归

沿用上述源码与安装版本；本轮没有复制上游实现或资源。`src/text_merge.rs` 的手工编辑仅更新范围／手动编辑标记，不清除双方处理与追加决策。`ui/text_merge/history.rs` 把轻量块元数据接入 gpui-base 现有 UndoManager 事务；连续输入、IME、采纳、忽略及不改文本的选边均复用同一撤销栈，回放完文本后原子恢复范围与决策再发 Change。元数据不保存三份源文本或每步完整结果；保留编辑器原有 1000 事务上限。对应上游 MergeModelBase 的文档更新与 undo 注册、TextMergeChange 的状态保存／恢复；普通编辑器未安装回调时保持原路径。

`merge_result::Prepared` 在写回前固定一个结果快照，成功后立即把它作为本实例下一次写回的预期基线。暂存失败可以重试，也可以继续编辑结果再重试；外部文件改动、冲突 index 身份变化仍被拒绝，原始编码／换行基线保持不变。回归先证明旧实现重试返回 Conflict，再验证解除测试 index.lock 后成功、结果再次修改后成功、外部修改仍拒绝。失败草稿仍按现有方式保留，不复制安装版 Rebased 的 IDE 错误弹窗。

`reference_checkout::{attempt,recover}` 复用 checkout 的失败分类和 update_session 的保存／恢复。远端与标签覆盖本地修改时，`ui/reference_checkout.rs` 将原 Prepared/Choice 送入 `ui/checkout.rs` 共用恢复弹窗；Smart 保存并恢复修改，Force 只在显式选择后执行。完整 ref、HEAD、现有本地分支／upstream 仍复验，成功后再核验最终 HEAD 与 symbolic HEAD；远端保留跟踪分支创建语义，标签保持 detached HEAD。Rebase choice 不转换为强制 checkout，untracked/unmerged 仍沿用独立失败处理。恢复冲突保留 stash 并提示，不自动丢弃。13 项 reference_checkout 回归涵盖远端／标签 × Smart／Force、staged 保留、过期引用拒绝及恢复冲突留存。

Windows 独立合并仓库 `plico-native-merge-1deff6dbf1114c66a3c69aaac46b3af1`，自有 PID42164：采纳左侧→输入 123→追加右侧，结果为 yours-first123/theirs-first；Ctrl+Z/Y 同步恢复／重做文字、追加箭头、点线与剩余数；第二块忽略两侧后，撤销恢复右侧按钮与未解决数、重做再次解决。人工 index.lock 阻止暂存后，工作文件已保存、三阶段仍在；解除自有锁后再次应用成功，ls-files -u 为空，文件精确内容核验通过。截图 `artifacts/behavior-fixes-20261022/fix-plico-{merge-ready,edited,appended,undo,redo,ignore,ignore-undo,ignore-redo,stage-locked-ready,retry-complete}.png`。

Windows 独立 checkout 仓库 `plico-checkout-fix-ef56aea983be49509580ef2c77833383`，自有 PID9920，本地 bare remote：远端 origin/task 的检出因本地修改受阻，进入共用问题弹窗，Smart 后本地 task 跟踪 origin/task，目标修改与原本地修改同时保留，临时 stash 已清除。截图 `fix-plico-{remote-menu,remote-problem,remote-complete}.png`。无真实网络写入，未修改用户仓库。

自动验证：核心完整测试 613 通过；最终 checkout 定向复跑 13 通过；桌面 65 通过、1 失败、1 原有忽略，新增真实编辑器决策／输入／IME 撤销回归通过。失败为 `editable_diff::lifecycle_tests::releasing_diff_clears_padding_from_retained_source`，独立修改前 worktree eceaca9 定向复跑得到相同 actual=0/expected=2，确认为既有失败，本轮未修改／屏蔽该测试。日志 core-tests.log、desktop-tests.log、baseline-desktop.log、checkout-final.log。Windows release 构建通过；cargo fmt 检查及仓库规定的无默认特性 Clippy 通过（仍有既有警告）。额外尝试全目标桌面 -D warnings 因既有告警未通过，不宣称全仓 lint 零警告。没有滚动自动化或像素级完全还原声明。

同一 Windows 测试仓库切回 main 并准备新的本地覆盖后，Tags→review-tag→检出同样进入问题弹窗，显式 Force 后 HEAD 分离并等于 tag、工作文件精确等于目标内容。截图 fix-plico-tag-menu/tag-problem/tag-complete.png。远端 Smart 与标签 Force 已完成原生入口验收，其余组合由上述核心矩阵回归覆盖。测试结束仅停止自有 Plico PID42164/9920 与此前自有 Rebased PID42040/40992；用户 Rebased 保留。

## Log／Commit 路径压缩与平铺文本（2026-10-22）

实现前：源码 4aec35256d765c217aa0d662b82ac4e860c396fd，Windows Rebased 1.1.20（安装构建与源码非同版）。自有 PID10356、独立 compact-path-fixture，以截图同形的 main/test Java 路径和 resources XML 创建提交及本地修改。Log 选中提交后观察到 spring-core\src、main\java\org\springframework\core\env、test 下的 java\…\env 连续目录压成一行，遇到分叉或文件停止；目录模式不把文件并入路径。切换 Directory 后，Log 和 Commit 平铺行均是文件名后紧接灰色父路径，窄栏按整体文本右边界裁剪，不给父路径分配独立右对齐列。Commit Directory 使用相同单链压缩。截图 artifacts/compact-paths-20261022/compact-ref-grouped.png、compact-ref-commit-options2.png、compact-ref-local-grouped.png，选项入口其余 compact-ref-*.png。

已读 platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ui/TreeModelBuilder.java::build/collapseDirectories/collapseParentWithOnlyChild/appendCollapsedParent：单子节点非叶且双方为 FilePath 时压缩，保留末端节点及折叠父身份；DirectoryChangesGroupingPolicy.kt 构造辅助目录。ChangesBrowserFilePathNode.kt::render/getRelativeFilePath/addCollapsedParent 和 ChangesBrowserNode.java::appendParentPath 使用父节点相对路径；平铺先 append name 再追加灰色路径。ChangesBrowserChangeNode.java 采用相同文件名／父路径文本顺序。

本地拟映射 file_tree 的可选压缩投影、ui/files 的 Log 行、local_changes/local_view 的 Commit 行。真实路径与文件索引保留用于选择／勾选／导航，展示名独立；只压缩已加载变更清单，不扫描磁盘或改变项目浏览器。Log 平铺改为与 Commit 相同的单段带色文本，消除两列宽度竞争。现有根计数行与省略号裁剪方式保留，横向滚动未纳入本轮。

实现完成：`file_tree::compact_rows` 使用末端真实路径作为身份，单独保留相对展示标签；Log 与 Commit 接入，其他树保持原投影。Log 平铺使用单段 StyledText，父路径仅作为灰色后缀，不再竞争文件名宽度。Commit 原有平铺实现已经是同样规则，原生确认后保留。

验证：file_tree 5 项、local_changes 9 项及桌面 Log 分组选择回归 1 项通过；cargo check 与 Windows release 构建通过。测试覆盖分叉、有文件的中间目录、多根目录、原文件索引、折叠、目录勾选范围、reveal 展开和分组切换保留选择。Windows 自有 Plico PID43984 使用同一 compact-path-fixture，Log 目录压缩与平铺切换、Commit 平铺／目录切换及压缩目录折叠通过。截图 `artifacts/compact-paths-20261022/compact-plico-{start,flat,local,local-grouped,local-collapsed}.png`。平铺时 PropertySource.java、PropertySourceTests.java、log4j2-test.xml 后立即跟灰色路径，文件名不再因路径长度被提前挤缩；超过可用栏宽的文件名仍正常省略。

保留差异：现有 Log 根计数行、整体省略号、Git 路径的 `/` 分隔符不变；Rebased Windows 展示 `\` 且偏向直接裁剪／横向浏览。此次修正路径压缩和文本布局，不扩展横向滚动或修改排序；未宣称完全像素一致。没有修改用户仓库或自动化滚动。

## Commit 控件与 Clone 焦点边框（2026-10-22）

实现前实机：Rebased 1.1.20 自有 PID20712、compact-path-fixture，勾选 Changes 后目录和文件采用同一蓝底白勾复选框；未勾选为灰边灰底，目录带 folder 图标及灰色文件数，只有 Changes 标题加粗。打开 Clone Repository 切到 Repository URL，点击 URL 后只有一条连续蓝边。截图 controls-ref-{commit,checked,clone,clone-focus}.png；Plico PID520 同场景 controls-plico-clone-before.png 复现外圈顶部截断及双边。证据最终归档 artifacts/controls-ui-20261022。

源码仍为 4aec35256d765c217aa0d662b82ac4e860c396fd：ChangesTreeCellRenderer.kt 的 ThreeStateCheckBox 与 textRenderer 独立布局；ChangesBrowserFilePathNode.kt::render/setIcon 普通字重、目录图标和 appendCount；expUI/icons/dark/checkBox{,Selected}.svg 的可见框为 16×16、圆角 2.5、灰边 #5A5D63、蓝底 #3574F0、白色 2px 勾。DvcsCloneDialogComponent.kt 使用标准 URL／Directory 字段，DarculaTextBorderNew.kt 与 DarculaNewUIUtil.kt::paintComponentBorder 在普通／聚焦边框间择一绘制。

本地原因：Clone 同时启用 Input 自身 focus border 和 external_focus_ring，后者越过表单顶边被裁剪；关闭外圈，保留自身连续焦点边。Commit 使用组件默认 rem 大小与暗色 input 边框，且目录没有图标／计数、普遍加粗。拟采用应用层复选框皮肤（保留组件原有键盘、三态、禁用和事件语义），补齐目录展示；不改 Git 提交选择规则。

实现与验收：`ui_style::checkbox` 选择上游普通／选中／混合及禁用 SVG，裁掉资源外围透明留白但不改路径颜色，固定可见框 16px；vendor Checkbox 新增可选 indicator renderer，保留原控件事件与无障碍语义，未使用的调用点保持原样。Commit、Amend、Clone 接入该皮肤。local_view 补目录图标与灰色本地化文件数，仅分组标题加粗；Log／Commit 共用文件计数格式。Clone 三个输入框关闭 external_focus_ring，圆角改为 4px，保留 Input 自身 focus border，消除裁剪根源而不放宽弹窗裁剪。

Windows Plico 自有 PID42376，实际勾选 Changes 4 文件，再取消单个文件，父分组显示蓝底白横线、其余三文件仍为白勾且底部显示 3 项修改；目录图标及普通字重正确。截图 controls-plico-{commit,checked,mixed}.png。Clone 初始 URL 与切到浅克隆深度字段均为连续单层蓝边，URL 失焦恢复普通边，浅克隆点击启用深度输入，截图 controls-plico-clone-{after,depth}.png。没有执行 clone 或 commit。源码与截图证据保存在 artifacts/controls-ui-20261022。

验证：cargo check、Windows release、桌面 Log 分组选择回归通过，cargo fmt --package plico -- --check 和 git diff --check 通过。曾尝试直接运行依赖包 Checkbox 单测，但 Cargo 拒绝非 workspace 成员的 dev-dependencies，因此不计为测试通过；本次新增皮肤的鼠标勾选与三态切换由原生验证，未专门复验键盘及运行中禁用态。保留差异：Clone 是应用内弹窗、无账号提供商侧栏／URL 历史，焦点边框保留组件的 1px 而非 Rebased 更厚描边；此轮未声明整页像素级一致。

## HTTPS Token／凭据生命周期：实现前源码核对（2026-10-22）

仅分析，不修改功能。参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd；已安装 1.1.20 与参考源码不同版。本轮尚未原生复验认证弹窗、helper 切换和失败重试，不能算功能前置 UI 验收完成。

已读源码及规则：

- plugins/git4idea/backend/src/commands/GitHttpGuiAuthenticator.java：每次远端操作一个实例，AuthenticationGate 串行获取认证；askUsername 一次取用户名和密码，askPassword 复用，已知 URL 用户名走 known-login 路径。NONE 不提供凭据；SILENT 仅 PasswordSafe／silent providers；FULL 增加交互 provider 与通用弹窗。通用 provider 优先 PasswordSafe，再扩展，再弹窗。
- GitHttpLoginDialog.kt：用户名、密码、记住密码；已知用户名不可编辑；存在 helper 配置时显示切换选项，另有平台登录 provider。GitVcsApplicationSettings.java 的 USE_CREDENTIAL_HELPER 默认 false；GitHandlerAuthenticationManager.java 按该开关和 NONE 模式通过命令级 credential.helper= 覆盖，而非永远优先系统 helper。
- PasswordSafeProvider：普通凭据按 URL＋login 保存，协议统一成 http 作为键，记住用户名与记住密码分开；DialogProvider 在返回凭据前调用 savePassword，认证失败再移除对应键，不能误记成“只在 Git 命令成功后才保存”。GitHttpGuiAuthenticatorTest.kt 覆盖成功保存、错误清理、已存密码失效、不记住、取消传播、用户名/密码只弹一次；本轮读取，未运行上游测试。
- GitHandlerAuthenticationManager.java：只有 askpass 曾被请求才识别认证失败；匹配 authentication failed、403、特定 repository not found、permission denied 等输出，再决定 forget/save；取消不算认证失败。不是任意非零退出都删密码。GitImplBase.java 的 handlerConstructor 重试路径仅 FULL 且认证失败重建命令，最多两次认证尝试；helper 开关变化不消耗次数。不是所有 runCommand 重载都自动重试。
- plugins/git4idea/backend/src/remote/hosting/http/SilentHostedGitHttpAuthDataProviderBase.kt：匹配主机及未失败账号，项目默认账号优先，其次唯一候选；多个候选不随意选。已知 login 必须匹配。失败回调验证 provider 身份，只标记当前 provider 提供的账号。HostedGitAuthenticationFailureManager.kt 与 platform/collaboration-tools/src/com/intellij/collaboration/auth/AccountUrlAuthenticationFailuresHolder.kt 按账号＋URL 暂停复用，凭据流变动/作用域结束清除；不删除账号的全局 Token。SilentHostedGitHttpAuthDataProviderBaseTest.kt 读取主机、默认、唯一/多候选、无凭据、指定 login、失败排除和 provider 隔离断言，未运行。
- plugins/github/github-core/src/org/jetbrains/plugins/github/extensions/GHSilentHttpAuthDataProvider.kt、GHSelectAccountHttpAuthDataProvider.kt：已有 token 配合账号信息查询提供 Git AuthData；多账号选择可设项目默认。plugins/gitlab/gitlab-core/src/org/jetbrains/plugins/gitlab/git/http/GitLab{Silent,Http}AuthDataProvider.kt：静默获取/刷新凭据并查当前用户名；交互区分零/一/多个账号、重登录、OtherMethod/取消。这层会调用平台 API，并非纯 Git HTTPS 密码桥接。
- GitHostingUrlUtil.kt：matchHost 只比较忽略大小写的 host，未纳入 port；PasswordSafe unifyUrl 合并 HTTP/HTTPS。Plico 后续需明确保留协议/端口与仓库边界，不能盲目复制这两处身份归一化。

本地映射与修订方案：现有 auth_request/askpass/auth_dialog 已有操作级缓存、用户名与密码单次表单和取消传播，git::operation_command 目前保留系统 helper。待补的重点是独立的凭据来源策略、保存生命周期、错误分类、有限重试、后台静默模式及多账号绑定。优先完善通用 HTTPS＋安全存储／helper 兼容层，平台账号 API/OAuth 作为独立后续层；不是把 GCM 当作 Rebased 的默认实现，也不因“不做 PR”就照搬其 API 权限需求。凭据来源明确互斥/有序，失败只处理本次使用的凭据；普通网络错误、分支保护、非快进拒绝不触发清除。

实现前需原生复验并建立本地 HTTP 测试矩阵：无凭据、正确/错误/过期 Token、只读 Token 推送、已知用户名、同主机多账号、不同端口/协议/仓库、取消、helper 切换、后台请求不弹窗、重试上限、配置保存失败、Windows/WSL 执行环境差异。Push/Pull/Clone 的重试必须在各自副作用边界接入，不能对失败操作一概重放。

### 认证实现前原生复验

2026-10-22，安装版 Rebased 1.1.20，自有 PID41160、独立 compact-path-fixture 与本地 HTTP Basic bare remote（18763）。Fetch 首次弹出 Enter credentials、Username、Password、默认选中的 Remember、Use credentials helper 和 Log In/Cancel。错误测试密码提交后重新弹出，保留 fixture-user、密码清空；第二次正确输入成功。再次 Fetch 无弹窗完成。改为 URL 指定 other-fixture 后显示不可编辑用户名，焦点在密码，Esc 取消并结束操作。截图 artifacts/http-auth-20261022/auth-ref-{login,wrong-ready,retry,reused,known}.png；取消后的主窗另行记录。所有凭据为本地测试值，不使用用户账号。对照 GitHttpGuiAuthenticator、GitHttpLoginDialog 与 GitHandlerAuthenticationManager，确认表单与生命周期；尚未原生验证 helper 切换、系统存储拒绝及第三方平台账号选择。

拟实现：在现有 Requests/AskPass 上分离持久凭据存储和 HTTP 认证状态；Windows 使用 Credential Manager，非 Windows 暂为会话保存并明确文案。通用凭据按协议/主机/端口/路径/用户名隔离，命令设置 credential.useHttpPath=true，保留与上游合并协议的差异。默认应用凭据、显式切换系统 helper，FULL 有限重试，SILENT 禁止 UI。Fetch/ls-remote 可重试；Clone 仅目标仍为空时重试；Push 有任一引用成功不重放；Pull 不重放已进入合并/rebase 的操作。失败仅清除当前尝试使用且值仍一致的凭据。GitHub/GitLab Token 通过通用密码字段使用，不添加平台 API/OAuth/PR 权限。

### 通用 HTTP 认证实现与验收

参考版本仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`，原生参考为 Rebased 1.1.20 / IC-262.10968.SNAPSHOT。本轮翻译其通用认证状态流程，不引入 GitHub/GitLab 账号 API。新增 `http_auth.rs` 管理 FULL/SILENT/NONE、保存来源、失败清理、helper 切换及记住选项；`authenticated_command.rs` 统一网络命令与有限重试；`credential_store{,/windows}.rs` 封装 Windows Credential Manager。`auth_request.rs` 保留现有 AskPass 通道与操作级串行请求，`auth_dialog.rs` 负责表单状态，Git 设置由 `update_settings.rs` 接入。Token 不写入 Git URL、配置或普通文件；Windows 使用当前用户 generic credentials，前缀 `Plico:git-http:v1:`。非 Windows 当前仅会话保存，界面明确标注。

上游 `GitHttpGuiAuthenticator` 的保存先于返回、认证失败清理当前来源、记住用户名独立于密码、known-login 只读、一次表单回答 username/password 均保留。缓存键保留协议、端口、仓库路径及用户名；不沿用上游 HTTP/HTTPS 合并。错误只清除本次使用且值仍一致的条目，不删其他账号；普通网络失败、非快进、分支保护失败不当作密码失效。保存失败保留输入及等待中的请求，允许取消记住后继续；helper 偏好无法保存时，本次操作仍可使用助手。取消后过期表单不能写入凭据。

Fetch、Pull、Push、Clone、remote ls-remote 使用同一认证入口；后台分支检查走 SILENT，不弹窗。认证最多两次，切换 helper 不占次数。Clone 仅在目标仍为空时重试；Pull 要求 HEAD 不变且未进入合并/rebase/冲突；Push 复验原分支/OID，若 porcelain 已报告任何引用成功或输出截断，不自动重放。现有 SSH、编辑器、force lease 和 Git 操作恢复流程保持原路径。后台 app 凭据仍需 Git 调用 AskPass，不能以 credential.interactive=false 禁用桥接；静默由 Requests 保证，helper 静默模式另外禁止 GCM 交互。

追加前置核对：原生 Rebased 自有 PID41160、本地 localhost 测试 remote，选择 Use credentials helper 后表单字段禁用，Log In 后 Fetch 成功。Git 设置搜索 credential，观察到已勾选 Use credential helper（截图 auth-ref-settings-helper.png）；源码 `plugins/git4idea/backend/src/config/GitVcsPanel.kt::cdOverrideCredentialHelper` 读写应用级 `isUseCredentialHelper`。本地同样在 Git 设置提供应用级开关，允许助手返回错误凭据时切回手工 Token；设置保存失败显示错误，不假装已保存。

原生 Windows Plico 独立 fixture：错误密码后重新提示且清空密码；正确密码成功；重启进程后从系统凭据库复用且无弹窗；切换 localhost 与 127.0.0.1 后重新请求，验证目标隔离；选择 helper 后字段禁用并完成 Fetch；URL 指定 other-fixture 后用户名只读，Esc 取消并结束操作。补充修正 Fetch 的分支弹窗遮挡认证对话框、英文 Password or token 标签宽度。中英文表单均使用现有单层焦点边框和复选框皮肤。截图归档 `artifacts/http-auth-20261022`；所有 Token 为本地测试值，未使用真实平台账号。

范围与待验收：没有新增 OAuth、平台账号选择、Token 权限 API、PR 或评论；SSH agent／key 保持原行为。测试证明通用 Git HTTP 认证链路，不代表已验证 GitHub/GitLab 服务端 SSO、组织策略或真实过期 Token。Windows 原生认证已复验；Windows 打开 WSL 仓库的本轮认证组合尚未专项原生复验。UI 结构与关键行为已核对，不宣称像素完全一致；已安装 Rebased 与参考源码不同版。

收尾回归：628 项核心测试（包括 18 项真实 Git HTTP/AskPass 入口测试）和 2 项登录表单桌面测试通过，Windows release 构建通过。另补凭据库不可用时仍可本次切换 helper 的回归。原生 PID41768 英文 Password or token 已保持单行；Git 设置开关启用／应用、停用／应用均可执行，截图 auth-final-settings-{enabled,disabled}.png。完整桌面套件未作为本轮通过项；已有 editable_diff 生命周期测试失败不在本轮修改范围。

用户指出 Remember 与勾选框错位后，追加对照 `DarculaCheckBoxUI.java::updateTextPosition/createLayout`：单行文本使用 CENTER。本地自定义皮肤沿用默认 indicator 的 2px 顶部补偿，但该皮肤已裁去 SVG 透明留白，导致视觉中心偏低。`vendor/gpui-component/src/checkbox.rs` 对自定义 indicator 改为行内垂直居中并去掉固定补偿；默认 indicator 路径不变，勾选/禁用/键盘事件不变。

对齐修正后 Windows release 再次通过（1m50s）；自有 PID19232 英文与 PID43140 中文原生截图 `auth-final-alignment-{en,zh}.png` 验证勾选框与文字居中。核心 Clippy 完成，保留现有 warning；格式及 diff 检查通过。验收结束关闭本次自有测试进程，并仅删除本地测试 endpoint 的 Plico 凭据及本轮测试创建的偏好条目，不读取／删除用户其他凭据。

### OAuth 专项源码复核（2026-10-22）

用户要求继续参照 Rebased，参考仍为 4aec35256d765c217aa0d662b82ac4e860c396fd。此处是源码调查，尚未实现 OAuth，也未做本轮原生 OAuth 授权验收。

已实际读取：github/authentication/GHOAuthService.kt、GHOAuthCredentialsAcquier.kt；collaboration-tools/auth-base/.../OAuthServiceBase.kt；gitlab/authentication/GitLabOAuthService.kt、GitLabOAuthSettings.kt、GitLabCredentials.kt、accounts/GitLabAccountManager.kt、ui/GitLabOAuthLoginViewModel.kt；GitLabOAuthSettingsTest.kt（读取未运行）。

GitHub 并非此前建议的 Device Flow：GHOAuthService 打开 account.jetbrains.com/github/oauth/intellij/authorize，携带 code_challenge 和本机 callback_url；回调 code 后通过同一 JetBrains 服务的 access_token 端点＋code_verifier，从 X-OAuth-Token 头取得凭据。OAuthServiceBase 合并并发授权请求、结束后清空当前请求。公开客户端源码不能证明该中转后端的内部实现；Plico 不能假定自己可作为 JetBrains 产品依赖此服务。可翻译浏览器等待、回调和取消生命周期；授权端点及应用身份要使用 Plico 自有配置，若选直连 GitHub 则明确为传输层差异。

GitLab 可直接参考程度更高：标准 Authorization Code＋S256 PKCE，随机 state 关联 pendingLogins；取消／结束 finally 移除请求；缺 code、错误 state、服务端错误均有处理；令牌交换后重定向至不带授权 code 的结果页，设置 no-referrer。GitLab.com 内置上游 client ID，自建实例支持 server→client ID 配置。Plico 应使用自己的注册身份，保留配置自建实例入口。上游 scope 为 api read_user write_repository openid；本产品只做 Git 认证，应按用户名校验和 Git 操作需要缩小权限，不原样附带协作功能权限。

GitLabCredentials 明确区分 PAT 和 OAuth，后者保存 access/refresh token、client ID、createdAt/expiresIn；到期前 60 秒刷新。PersistentGitLabAccountManager 在 dataGuard 锁内读取、刷新并更新凭据，避免多个 Git 操作并发刷新竞争；取消与刷新失败独立传播。GitLabOAuthLoginViewModel 防重复登录、验证指定用户名和账号唯一性，允许转 OtherMethod。配置测试覆盖空值、无效 URI、重复服务器、空白与分隔符解析。

后续映射：独立 OAuth provider／账号凭据模型接入现有 Requests 来源策略；复用 Windows 安全存储而不把 refresh token 当普通 Git 密码；共用账号刷新锁；保留手动 Token/helper 回退。实现 UI 前按 AGENTS 原生复验。不能把“Rebased 有 OAuth”解释为只复制登录按钮即可完整交付，也无需重新自行设计其已验证的生命周期。

### OAuth 与托管仓库 Clone 实现前复验（2026-10-22）

原生 Rebased 1.1.20，自有 PID15388，独立 compact-path-fixture：Find Action→Clone Repository，GitHub 页显示浏览器登录/Use token，切换 Token 页显示 Server/Token/Generate/Log In/Back；GitLab 页显示 Server/Token，底部 Clone 在未选择仓库时禁用。截图 oauth-ref-{github,token,gitlab,browser}.png。安装版 GitLab 无 OAuth 入口，采用参考 commit 4aec352 的较新 GitLabCloneLoginComponentFactory/GitLabOAuthService 行为；尚未取得真实账号授权及登录后仓库列表原生截图，用户已明确未注册应用、先完成实现，真实平台授权验收留待注册后。

已读 GHCloneDialogExtensionComponentBase、GHCloneDialogRepositoryListLoaderImpl、CloneDialogLoginPanel、GHOAuthCredentialsUi，GitLabCloneLoginComponentFactory、GitLabCloneRepositoriesComponentFactory、GitLabCloneRepositoriesListViewModel。规则：账号独立加载/取消，查询保留选择，错误提供重试/重登录；搜索置顶、目录与浅克隆置底；登录/等待/取消与克隆进行中分离。GitHub 请求当前用户仓库并补组织仓库，GitLab 源码用 GraphQL 分页。Plico 拟分离 hosting 的账号/API/OAuth 与 Clone 页视图；保留 HTTPS、账号/目标绑定、安全存储、取消迟到结果保护；不加入 GitHub Enterprise，GitLab 支持自建地址。API 可采用 REST 分页，记录与上游查询差异。

### OAuth / Clone 实现与注册（2026-10-22）

实现位置：`src/hosting.rs` 管理账号、凭据轮换和仓库绑定，`hosting/oauth.rs` 负责直接平台 PKCE、state 校验、本机回调、取消/超时；`hosting/api.rs` 负责受限 REST 请求及分页；`hosting_panel.rs` 管理登录/Token/列表状态，`clone_dialog.rs` 保留 URL 和实际克隆流程。Git HTTP 通过 `http_auth` 复用账号，失败只排除本次操作中的账号，不删除共享 OAuth 凭据。Windows 使用 Credential Manager，Linux 当前仅会话内保存。

用户指出开发者配置混入登录页后，重新阅读 `CloneDialogLoginPanel::buttonPanel/setupNewUi` 和 `GHTokenCredentialsUi::centerPanel`：普通用户页仅有浏览器登录 / or / Use token；Token 右侧 Generate、下方权限说明、第二列下方并排 Log In/Back。移除 Clone 页 Client ID/Secret 表单和配置按钮；公共应用身份随客户端提供，GitHub 应用参数由构建环境 `PLICO_GITHUB_CLIENT_SECRET` 注入。自建 GitLab 无默认注册身份时可用 Token，不借用 JetBrains 应用。保留开发者 `oauth::save_config` 接口，不向普通 Clone 用户暴露。

通过 ego-browser 注册 Plico GitHub OAuth App 和 GitLab 公共客户端应用，完整记录按用户要求保存到 Windows 桌面 TXT，不进入仓库。GitHub 开启 access token 到期；GitLab Confidential=No，scope 为 read_api/read_user/write_repository。GitHub repo/read:user/read:org/workflow，支持同步工作流文件，不申请 gist。Windows 临时核心验收程序调用同一生产 OAuth、账号保存和 repository API：GitHub 成功，第一页 37 个仓库；GitLab 成功，第一页 1 个仓库。未对真实远端执行 Push 等写操作。发行流水线通过仓库 Actions secret 注入 GitHub 应用参数，用户 Token 不进入构建。

保留差异：GitHub 不依赖 JetBrains 中转服务，直接 code+PKCE；GitLab REST membership 分页代替 GraphQL；GitHub 当前加载 user/repos 全部 affiliation，尚未补组织公开但未参与的仓库。使用每页 100 项手动 Load more，不一次性抓取全量。回调页无外部资源并设置 no-store/no-referrer，但尚未实现上游跳转无 code URL 的结果页。GitHub Enterprise 不在范围内。真实账号登录已验证不等于所有自建证书、代理、组织 SSO 场景均验证；视觉验收另记，不宣称完全复刻。

修正后核心回归 634 项通过，6 项 hosting 专项覆盖 PKCE/state/拒绝授权/取消释放回调/Token 到期数据/跨主机仓库地址/HTTP 不跟随重定向，2 项认证表单桌面测试通过。Clippy 完成并保留既有 warning。Windows 原生 GitHub 登录页、Token 页已复验；另发现长列表挤占底部操作区，限定列表和宿主 flex 高度，补 Server/Generate/Back 中文及 Repository URL 的 VCS 图标。

最终原生 PID42872：`artifacts/oauth-clone-20261022/oauth-delivery-login.png` 已无 OAuth 配置按钮；`oauth-delivery-token.png` 为中文表单，Generate 右置、权限说明下置、登录/返回对齐；`oauth-delivery-list.png` 可见等待指示、取消及账号操作。根因不是列表自身高度，而是 Clone 内容区固定 520 DIP 在 150% 缩放的小窗口内超过弹窗可用高度；改为 viewport height - 150 DIP，上限 520 DIP。发行 Windows release、格式与 diff 检查通过。仍采用 Plico 内嵌弹窗，未宣称与 Rebased 独立原生窗口逐像素一致。

### 2026-10-23：GitHub 登录后 Clone 页面实际对照

- 参考源码：Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd`；原生参考版本 1.1.20 / IC-262.10968.SNAPSHOT。本次在原生 Rebased 完成浏览器 GitHub 授权，查看真实仓库列表及账户菜单。
- 已读：`GHCloneDialogExtensionComponentBase.kt`、`GHRepositoryListCellRenderer.kt`（github-core/ui/cloneDialog），`platform/collaboration-tools/src/com/intellij/collaboration/auth/ui/CompactAccountsPanelFactory.kt`。
- 提取规则：搜索框右侧分隔线及账户入口；菜单列出账号和登录方式；自己的仓库显示短名，其他所有者保留 fullName；仓库列表下目录与浅克隆选项常驻；浏览器登录等待替换登录按钮为状态及取消。
- 本地映射：`hosting_panel.rs` 收拢账户操作至搜索栏菜单、调整列表名称与登录状态；过滤保留仍可见的选择；切换账号清空旧列表，避免失败后把旧仓库绑定到新账号。`clone_dialog.rs` 观察托管面板状态，登录后的目录不依赖仓库选择。
- 保留差异：账户入口暂用首字母而非远端头像；多账户仍切换查看而非聚合；菜单保留刷新、移除账号；分页 Load more 保留以限制规模；Plico 使用应用内弹窗，保留克隆子模块选项。不能宣称像素级完全一致。
- 原生参考截图：`artifacts/oauth-clone-20261022/rebased-hosted-{login,wait,success,account}.png`。
- 验证：Windows release 构建通过；仓库名称规则测试 1 项通过；格式化及 diff whitespace 检查通过。原生 Plico 已查看真实 GitHub 仓库、点击选择确认 Clone 启用、打开账户菜单，截图 `plico-hosted-aligned-{list,account}.png`。未执行实际克隆；本轮未重复全量测试。

### 2026-10-23：Clone 账户头像与菜单第二轮

- 延续原生已授权 GitHub 的 `rebased-hosted-success.png` / `rebased-hosted-account.png` 真实截图；本轮重新打开参考程序确认登录页，测试 profile 重启后未保留账号，未将匿名截图当作已登录菜单验收。
- 同一参考 commit 追加读取 `GHAccountsDetailsProvider.kt`、`CachingGHUserAvatarLoader.kt`、`CompactAccountsPanelFactory.kt`、`VcsCloneDialogUiSpec.kt`。规则：头像独立懒加载、失败保留默认图标；工具栏 24 DIP，菜单 40 DIP；账号名与服务器分两行，服务器省略 https://；账号下只显示登录方式，不混入刷新及移除。
- 本地：`hosting/api.rs::avatar` 经用户 API 获取头像 URL，头像请求不携带 Token、不跟随重定向、10 秒超时、1 MiB 上限；`hosting_panel.rs` 独立任务及面板生命周期缓存，不阻塞仓库列表，过期账号响应不覆盖当前账号。菜单真实图片与两行身份信息；登录标题使用当前 UI 字号的比例，不改全局字号。
- 保留差异：多账号仍通过账户行切换仓库列表，非上游聚合；头像缓存仅限当前面板，无上游五分钟共享缓存和自动退避重试；无头像或加载失败回退图标。
- 原生验收中确认了英文正文与菜单文字的字体继承偏差；Clone 弹窗显式沿用 `Theme.font_family/font_size`，账户菜单自定义行也使用主题字体。头像在图片元素上设置圆角，避免仅给父容器圆角仍绘制为方形。
- 最终 Windows release 构建通过；6 项 hosting 核心测试及 1 项桌面仓库标签测试通过；diff whitespace 检查通过。原生真实 GitHub 头像与圆形裁切已确认，菜单截图 `artifacts/oauth-clone-20261022/avatar-final-menu.png`。主题字体已显式统一，但与 Rebased 的字体观感仍有差异，不能认定字体像素验收完成；本轮没有修改用户全局字体配置。

### 2026-10-23：GitLab Clone 独立对齐

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 gitlab-core 的 `GitLabCloneLoginComponentFactory`、`GitLabCloneRepositoriesComponentFactory`、`GitLabCloneListRenderer`、`GitLabCloneListItem`、`GitLabCloneLoginViewModel`；账户头像继续沿用 CompactAccountsPanelFactory 的 24/40 DIP 规则。

- 登录入口区分 gitlab.com 与自建服务器；服务器字段仅在 Token / 自建服务器表单出现，重新认证已有账号时锁定服务器。菜单在浏览器登录与 Token 之间包含自建服务器入口，并使用分隔线。
- 列表使用 `nameWithNamespace` 显示名称，保留群组和子群组；本地 REST 映射 `name_with_namespace`，克隆仍使用 `http_url_to_repo`，搜索兼容显示名称及 URL 路径。旧服务器响应缺字段时回退 `path_with_namespace`。
- 列表网络失败可重试，缺少/无效凭据进入原账号重新登录；Token 返回在有账户时返回列表，否则返回登录入口。默认头像采用上游中性头像，避免 GitLab 显示 GitHub 图标；头像 URL 支持服务器相对路径。
- 本地实现：`hosting_panel.rs`、`hosting/api.rs`、`assets.rs`、`i18n.rs`。新增命名空间显示与真实克隆 URL 独立的回归测试。
- 版本差异：安装版 Rebased 1.1.20 的 GitLab 页面为 Token 表单，源码已支持 OAuth 入口；OAuth 入口按源码实现。自建服务器 OAuth 需要该服务器注册的 Plico 应用配置，未配置时明确提示使用 Token；不借用 JetBrains 应用身份。仍保留 REST 分页、单账号切换，不宣称多账号聚合完全一致。
- 验证：7 项 hosting 核心测试和 1 项桌面标签规则测试通过，最终 Windows release 构建通过。原生参考 Token 页面已截图；Plico 真实 GitLab 账号仓库、图片头像、三项菜单、自建服务器表单、切换 Token 并返回列表均已实际操作，截图 `artifacts/oauth-clone-20261022/gl-*.png`。截图使用菜单文案完成后的构建；最终构建仅追加 Retry 中文翻译。未执行真实克隆，未重新验证 OAuth 授权与自建服务器授权。

### 2026-10-23：认证全链路审计（进行中）

参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`。追加读取 `GitHttpGuiAuthenticator`、`SilentHostedGitHttpAuthDataProviderBase`、`HostedGitAuthenticationFailureManager`、`AccountUrlAuthenticationFailuresHolder`、`GitLabHttpAuthDataProvider`、`GHSilentHttpAuthDataProvider`、`NativeSshGuiAuthenticator`、`NativeSshAuthService`、`SshPrompts`。

前置实机：Rebased 1.1.20，自有 PID17792；独立 `auth-audit-20261023/repo` 配置本地 SSH transport fixture（不连接服务器、不读取密钥），Fetch 后出现 SSH Key Passphrase 弹窗：密钥路径说明、掩码输入、Remember、OK/Cancel。截图 `artifacts/auth-audit-20261023/ref-ssh-passphrase.png`。HTTP 错误凭据/重试/取消规则延续本会话 `artifacts/http-auth-20261022` 的原生验证。

已确认缺口：SSH AskPass 命令桥未接入且现有 HTTP 表单不适用于 SSH；托管凭据拒绝仅操作内记忆，缺少上游跨操作 URL/账号失败隔离；移除绑定账号后旧绑定阻止唯一有效账号回退。拟补独立 SSH 提示分类与凭据生命周期、静默不弹窗、账号失败缓存随 token 版本失效。多账号交互选择、账号管理、自建 OAuth 应用配置及回调收尾继续审计。

账号设置前置复验：自有原生 Rebased PID17792，Find Action→GitHub Settings；空账号页为带 + / - / 默认账号工具栏的列表，+ 菜单提供 Browser / Token / Enterprise，底部显示 helper 已启用时托管账号不参与 Git 操作的提示。截图 `artifacts/auth-audit-20261023/ref-settings-{github,add}.png`。读取 `GithubSettingsConfigurable`、`GitLabSettingsConfigurable`、`AccountsPanelFactory`：增删与新凭据保存在草稿、Apply 才写入账号管理器，Cancel 丢弃；GitLab 新源码有自建 OAuth server=clientID 配置，安装版没有该入口。映射 `hosting_settings.rs` 与 `hosting_panel.rs` 的独立登录模式；不加载 Clone 仓库列表，登录凭据暂存在设置实体。Enterprise 与 PR 设置继续按产品范围省略；默认项目账号和交互式账号选择仍待实机多账号场景验收。

#### 审计实现与恢复原生验收

用户解锁后重试成功。原生 Rebased 自有 PID24656，独立 profile 加入两个不含凭据的 fixture 账号；本地 transport 仅调用 AskPass，不连接真实服务器。Fetch→Choose GitHub Account：首项默认选中、当前项目默认账号复选框、Log In/Cancel；选择缺少凭据的账号后显示 Browser/Token 登录入口。截图 `artifacts/auth-audit-20261023/ref-multi-{choose,login}.png`。追加读取同 commit：

- `plugins/github/github-core/src/org/jetbrains/plugins/github/extensions/GHSelectAccountHttpAuthDataProvider.kt`、`authentication/ui/GithubChooseAccountDialog.kt`；GitLab 对应 `plugins/gitlab/gitlab-core/src/org/jetbrains/plugins/gitlab/authentication/ui/GitLabChooseAccountDialog.kt`。
- `platform/collaboration-tools/src/com/intellij/collaboration/auth/ui/AccountsPanelFactory.kt`、`auth/AccountUrlAuthenticationFailuresHolder.kt`。
- `platform/external-process-auth-helper/src/externalProcessAuthHelper/NativeSshGuiAuthenticator.kt`；GitLab `authentication/GitLabOAuthService.kt`、`GitLabSettingsConfigurable.kt`。

落地规则与位置：

| 场景 | 本地实现及边界 |
| --- | --- |
| SSH 密钥口令、密码、PIN、交互式提示、主机确认 | `ssh_auth.rs`、`askpass.rs`、`authenticated_command.rs`、`auth_dialog.rs`。接入 SSH_ASKPASS；按提示类型隔离缓存，重复失败只清除本次使用的旧值，保留并发更新；主机确认 Yes/No，不保存为密码；静默模式不排队弹窗。原生 OpenSSH 继续管理 agent、密钥选择及 known_hosts。 |
| 托管账号选择和重登录 | `auth_request.rs`、`auth_dialog.rs`、`hosting_panel.rs`。多个候选交互选择，缺失/被拒凭据进入指定账号登录；登录结果核对账号身份，不能将另一账号 Token 保存到原账号。可回退手动/helper，取消释放等待。 |
| 项目默认账号与失效绑定 | `hosting.rs`，远端操作与 `branch_updates.rs` 传递项目上下文。项目默认隔离，保留 Clone 明确目标绑定优先；删除的绑定可回退唯一匹配账号。按精确 origin、GitLab 路径边界及已知用户名筛选，不跨服务器借用令牌。 |
| 跨操作失败处理 | `hosting/failures.rs`、`http_auth.rs`。按账号+URL+凭据指纹记录拒绝，最大256项；更换凭据恢复可用。只归因给最后实际使用的提供方，不清除其他账号/端点共享凭据。 |
| GitHub/GitLab 设置 | `hosting_settings.rs`、`ui/settings.rs`。添加、移除、重登录、当前项目默认；新增账号/凭据暂存，Apply才保存，Cancel丢弃。helper启用时提示托管账号不参与Git HTTP认证。 |
| 自建 GitLab OAuth 配置与回调 | `hosting/oauth.rs`。设置 `server URL = client ID`，检查HTTPS、重复地址及空值；回调跳转不带code的结果页、no-store/no-referrer、取消和超时释放监听。结果页只表示收到响应，授权交换结果由客户端显示。 |

Windows Plico 自有测试进程已实际验证：SSH口令/取消、主机确认拒绝；GitHub设置、移除草稿后Cancel保留原账号；GitLab设置与自建OAuth配置入口；本地模拟凭据拒绝→账号选择→指定账号重登录→其他凭据→取消，不影响真实账号Token。截图均在 `artifacts/auth-audit-20261023/`，`plico-final-*` 为新交互流程，`plico-account-*` 为设置取消。新图标来源/许可证见 assets/icons/SOURCE.txt 与 docs/THIRD-PARTY.txt。

保留差异及未覆盖项：安装版GitLab没有新源码OAuth设置，按源码实现；GitHub直接平台PKCE而非JetBrains中转，交互认证使用真实login而非上游x-oauth-basic替代用户名；GitLab只允许HTTPS，避免明文令牌传输。Plico为内嵌弹窗，账号设置尚无头像，列表行与弹窗尺寸尚非逐像素一致。GitHub Enterprise继续不做；Clone单账号切换、手动分页及未补组织公开仓库是既有列表差异。真实组织SSO、硬件PIN、自建GitLab授权、异常证书/代理本轮未实测；已有真实GitHub/GitLab授权记录不能代替这些覆盖。自定义SSH命令的后台远端检查仍保留原有保守禁用规则，未自行覆盖用户transport。

验证结果：全量核心测试646项通过；最后补充AskPass提示分类回归后核心库268项通过，认证表单桌面测试3项通过；最终Windows release（主程序与AskPass）构建通过，格式化及diff whitespace检查通过。Clippy完成，仍有既有及样式建议warning（未宣称零警告）。当前项目默认账号在独立repo中Apply后重启复验；只关闭本次自有测试程序，Rebased测试profile的虚构账号与helper设置已清理还原。未对真实远端执行写操作。

### 2026-10-23：OAuth 回调页与欢迎页按钮间距

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`。自有原生 Rebased 1.1.20 PID10544 通过 Close Project 返回欢迎页，确认 Open/Clone Repository 为有独立间隔的工具栏按钮；截图 `artifacts/oauth-page-20261023/ref-welcome-spacing.png`。读取 `platform/platform-impl/src/com/intellij/openapi/wm/impl/welcomeScreen/ProjectsTabFactory.kt::createActionsToolbar`：首组操作使用独立文本按钮。Plico `src/ui.rs` 对应横向容器补8 DIP间距，不改变按钮操作；Windows原生截图 `welcome.png` 已确认不再相贴。

回调页面读取 `plugins/gitlab/gitlab-core/resources/html/oauth-response.html` 和 `authentication/GitLabOAuthService.kt::sendHTMLResponse/buildResponsePage`：HTML模板、system-ui、viewport、结果文案、no-referrer；安装版GitLab为旧Token流程，无法实机复现这份新源码OAuth页，本项以源码和用户实际Plico回调截图为依据。按用户明确要求采用Plico居中卡片样式，而非复制JetBrains品牌页。

本地 `assets/html/oauth-result.html` 与 `src/hosting/oauth.rs::result_page`：max-width 520px、水平居中、移动端边距、浅/深色样式；文案通过 `i18n::t`，lang通过已解析的应用语言设置，不读取浏览器语言。`Content-Type` 改为text/html，保留UTF-8、no-store/no-referrer和有界监听；不将code/state/服务器返回内容插入HTML。页面继续表示“收到授权响应”，不能在令牌交换结束前宣称登录成功。

验证：ego-browser在Windows预览同一模板的中文、英文及360px深色布局，确认居中且无横向溢出（`browser-*.png`）；这是模板预览，没有重新触发真实平台授权。OAuth现有6项测试通过，增加结果页HTTP类型、模板占位符清空与UTF-8字节长度断言；应用语言仍复用既有显式中文/英文/跟随系统解析。Windows原生欢迎页间距验收通过。并非整页逐像素复刻；字体使用系统字体，不加载远程资源。
最终Windows release构建通过，格式与diff whitespace检查通过。截图与模板验收使用ego-browser skill；本轮未提交或重新发布preview。

## Markdown 预览前置核对（2026-10-23）

用户授权安装并重启 Rebased。安装官方 Marketplace Markdown 262.10968.63（插件7793，更新1171956）到 Rebased 用户插件目录，匹配 Rebased 1.1.20 / IC-262.10968.SNAPSHOT。安装前 `.md` 由 TextMate 打开，无预览；重启后独立目录 `GitReview-validation/markdown-20261023/README.md` 可正常分栏渲染。截图 `artifacts/markdown-20261023/reference-plugin.png`、`reference-source.png`、`reference-preview.png`。默认左右分栏；右上 Tab 工具栏按源码、源码与预览、预览排列；源码模式保留编辑器，预览使用比例字体、标题层级、表格边框和独立代码块。模式切换的 JCEF 首帧可能短暂为空。测试目录未修改用户仓库。

源码基线 `4aec35256d765c217aa0d662b82ac4e860c396fd`：已读 `plugins/markdown/core/src/org/intellij/plugins/markdown/ui/preview/MarkdownEditorWithPreview.java`（默认双栏、预览焦点、可见行同步）、`MarkdownPreviewFileEditor.kt`（文档更新、单任务合并、可见性与释放）、`MarkdownPreviewFileEditorProvider.kt`（预览提供者与文件类型检查）、`platform/platform-impl/src/com/intellij/openapi/fileEditor/TextEditorWithPreview.kt`（三模式、Tab actions、状态保留与布局记忆）。图标取自同 commit 的 `platform/icons/src/expui/general/{editorOnly,editorPreview,previewOnly}_dark.svg`（初查 general，实机比对后采用 Islands 对应 expui 资源）。

本地接入点为 `project_view::FileTab`、`ui/chrome.rs` 与独立 Markdown 预览模块。复用同一编辑文档，模式切换不重建编辑器；采用原生 TextView 而非 JCEF，保持低内存。初始范围为项目文件 Tab 的 Markdown 阅读／编辑预览；Git Diff 仍保留文本比较，不把渲染后的段落作为差异算法输入。原生渲染器与浏览器 HTML/CSS、脚本／扩展的差异需在交付记录中明确，不声称完整浏览器兼容。

### 实现与原生验收

`src/markdown_preview.rs` 维护原生 TextView 与编辑文档的订阅；`project_view::FileTab` 接入三模式、分栏和文档重载，`ui/chrome.rs` 将模式按钮放到 Tab 行右端。`layout_preferences.rs` 记住新文档默认模式，`i18n.rs` 提供中英文提示。模式切换不复制可编辑缓冲区；编辑和磁盘重载更新同一个预览，后台解析未追上当前文档时不使用旧源码坐标同步位置。非活动且无待保存内容的 Tab 释放预览。相对图片以文档所在目录解析；相对文档链接通过既有项目文件打开入口，仅在当前仓库内路由，不执行任意协议。

Windows 独立仓库实测通过：默认双栏、源码／预览切换、标题／表格／代码块、本地含空格图片路径、相对文档链接、输入法提交后预览实时更新。截图位于 `artifacts/markdown-20261023/`：`plico-split-final.png`、`plico-source.png`、`plico-preview.png`、`plico-link.png`、`plico-live.png`。输入法候选尚未提交时不更新预览，提交后更新；文档保存继续遵守既有自动保存机制。

保留差异：原生渲染不包含完整 HTML/CSS、Mermaid／数学扩展及 data URI 图片；页内锚点和跨文档锚点暂未实现。表格占满预览宽度，代码块有语言标题，任务列表显示只读复选框，与安装版插件的具体绘制不同。普通文本 Diff 不增加预览模式。Tab 释放后重新打开会重建预览，未保存独立预览滚动位置。源码到预览为可见范围跟随，不保证浏览器版像素级滚动对齐；按项目约定，滚动手感留给用户手测。以上差异不宣称已像素级还原。

验证结果：核心测试647项通过，项目视图桌面回归4项、Markdown测试3项通过；最终Windows release构建、格式检查、diff whitespace检查通过。最终构建原生截图 `plico-final.png`。本轮未提交、推送或发布安装包。

### 2026-10-23：Rollback 未选中 gitlink 阻断审计

实现前在已安装 Rebased 1.1.20 的独立 `GitReview-validation/markdown-20261023` 仓库新增嵌套 gitlink，产生 README 修改与 gitlink HEAD 变化。工具栏 Rollback 可正常打开包含两个变更的对话框，取消 gitlink 勾选、只保留 README 后执行，界面仅留下 gitlink 修改。截图 `artifacts/audit-20261023/reference-rollback-{selected,result}.png`。未修改用户仓库。

读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/RollbackChangesDialog.kt`：浏览器显示候选变更，但 doOKAction 只向 RollbackWorker 传 includedChanges。`plugins/git4idea/backend/src/rollback/GitRollbackEnvironment.java::rollbackChanges` 对传入集合分类 unindex/unversion/revert/delete。规则是未选中项不参与执行前的文件验证；候选列表不应因未选中特殊文件而无法打开。

本地 `rollback_operation::prepare` 对所有候选计算指纹，遇到目录/gitlink直接返回错误；真实 Git 回归已复现。修复应保留候选的独立准备错误、在确认后仅验证所选项，不能把不支持的项伪装为可执行，也不能取消已选文件的迟到修改检查。普通文件确认框保持现有结构，子模块本身的递归恢复仍不在本轮支持范围。

修复后 Windows 原生 Plico 在同一独立仓库复验：确认框正常列出 README 与 gitlink；取消 gitlink 勾选后回滚成功，列表只剩 gitlink。Windows Git status 和嵌套仓库 log 验证 gitlink 的 `nested change` HEAD 保留。截图 `artifacts/audit-20261023/plico-rollback-{selected,result}.png`。当前确认框仍为平面路径列表，不宣称与 Rebased 树形分组逐像素一致。8项Rollback真实Git回归、Windows release构建通过。

## 2026-10-23：提交状态查询与 stash 展示配置

实现前实机：Rebased 1.1.20，独立 `markdown-20261023` 仓库配置 `status.showStash=true`、保留一个 stash，并同时修改 README 与 gitlink。仅勾选 README，输入消息后点击 Commit，成功提示 `1 file committed`，Log 增加 `Auditstashconfiguration`，gitlink 仍未提交，stash list 仍含 audit stash。截图：`artifacts/audit-20261023/reference-commit-stash-{selected,result}.png`。本次未验证 Amend 原生界面。

源码依据：参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，`plugins/git4idea/backend/src/checkin/GitIndexInfoStagingAreaStateManager.kt::listStagedChanges` 显式设置 `--no-show-stash`、`--no-ahead-behind`，使暂存保存查询不受展示配置影响。Plico `commit_operation.rs` 缺少前者，真实 Git 回归已稳定报 `Unexpected commit status record`。计划在查询边界补齐这两个选项；保留严格解析器，不通过吞掉任意未知记录掩盖格式错误。`commit_index.rs` 更新输入契约。截图布局无新增差异；修复后 Plico 原生验收待执行。

Windows 构建与原生复测完成：Plico 在同一 fixture、同一配置下仅提交 README，状态显示“已提交”，Log 自动出现 `PlicoStashAudit`，未勾选 gitlink 留在变更列表。Windows Git diff-tree 确认新提交只有 README.md；refs/stash 执行前后均为 `da5fbc58efdc836030c46cc02fb67c979eb9d5e8`。截图 `artifacts/audit-20261023/plico-commit-stash-{selected,result}.png`。Amend 由真实 Git 回归覆盖，本轮未另做原生 Amend。没有新增界面，既有中英文与布局差异不在本修复中调整。

## 2026-10-23：手动 Stash 恢复冲突入口

实现前 Rebased 1.1.20 实机：独立 markdown-20261023 的 audit stash 与 HEAD 修改 README 同一位置。Stash 页点击 Apply 后自动打开居中 Conflicts 窗口，列出 README，左右 Accept、Resolve Manually 可用，Accept and Finish 初始禁用。截图 `artifacts/audit-20261023/stash-ref-{before,conflict}.png`。没有自动删除 stash 或替用户选择内容。

已读参考 4aec352 的 `plugins/git4idea/backend/src/stash/GitStashUtils.kt::unstash` 和 `stash/ui/GitStashActions.kt`：普通 Apply/Pop 不带 --index；index conflict 单独报错，内容 conflict 调用 conflictResolver.merge；无论成功与否刷新受影响文件与 index。Plico `ui/saved_changes.rs::restore_stash` 当前只记录状态与 refresh，缺少打开既有冲突窗口。拟在未取消且结果为 Conflicts 时调用共用 open_conflicts，保留 IndexConflict/本地修改/未跟踪文件阻断的独立提示，不改变 Git 操作或 stash 删除规则。

修复后 Windows 原生验收通过：独立 fixture 同一 audit stash 再次 Apply，自动打开共用“冲突”窗口并列出 README；宿主 Git 确认 README 为 UU、refs/stash 保持 da5fbc58efdc836030c46cc02fb67c979eb9d5e8，没有自动采纳或删除。截图 `artifacts/audit-20261023/stash-plico-fixed.png`；旧行为截图 `stash-plico-baseline.png`。Windows release 构建通过，Stash 九场景集成用例、3项删除保护测试及1项侧栏视图生命周期测试通过。后者只覆盖视图复用，不冒充自动弹窗回归；自动弹窗由本次原生验证覆盖。

保留差异：共用冲突列表当前右侧集中动作按钮，上游逐行 Accept 与底部 Resolve Manually；上游还显示 stash 专用描述，本地仍为通用标题。本次只修复缺少自动引导，没有宣称窗口像素或专用文案完全对齐。

## 2026-10-23：Log 审计补充（本轮无产品改动）

复核 4aec352 `VisiblePackRefresherImpl.java::computeState/getCommitCountUpdate`、`VcsLogFiltererImpl.kt::applyHashFilter`、`VcsLogGraphTable.java::updateDataPack` 与 `SelectionSnapshot.kt`。本地对应 `ui/log_query.rs`、`ui/repository.rs`、`ui/log_selection.rs`、`history.rs`、`ui/log_viewport.rs`。过滤替换取消旧请求、分页固定 roots、详情完成身份门槛均有代码与回归证据。完整范围、测试重合及未验证项见 AUDIT-2026-10-23.md。

保留差异重新明确：哈希消息扩展阶段未实现；上游刷新优先可见选中行作为滚动锚点，本地使用顶部可见行。后者待原生手测，本轮不自动化滚动、不声称完全一致。此次为源码与测试审计，没有新原生交互截图，不作为 UI 修改验收。

## 2026-10-23：隐藏 Markdown 预览生命周期

实现前 Rebased 1.1.20 + Markdown 262.10968.63 原生独立 fixture：点击 README Tab 显示预览，再切换 Log，README Tab 保留。截图 `artifacts/audit-20261023/markdown-ref-{active,hidden}.png`。截图只证明切换与内容状态，不据此推断内存释放。释放依据为参考 4aec352 `MarkdownPreviewFileEditor.kt::AttachPanelOnVisibilityChangeListener.componentHidden/detachHtmlPanel`：隐藏时 dispose panel，文档 listener 随 panel 释放，重新显示 attach 并渲染当前文档。

本地 FileTab::deactivate 目前将 markdown 与编辑文档放在同一保留条件下；Editing.edited 用于保存编辑器撤销历史，因此编辑过的隐藏 Tab 还持有预览。拟独立释放 markdown，而保留原 editor/document 与其状态；activate 的 attach_document 从同一文档重建预览。现有源码模式仍持有预览的情况另行记录，本项不声称覆盖所有可见性优化。

隐藏预览修复后原生验收：Plico独立README编辑 LifecycleAudit → Log → README，预览重建显示新文本；Ctrl+Z 能撤销切换前的输入，预览同步更新。截图 `artifacts/audit-20261023/markdown-plico-{edited,hidden,returned,undo}.png`。实体释放由新增weak回归证明，截图不作为内存释放证据。桌面74项及F7专用1项通过；Windows构建通过。保留编辑器/文档实体和撤销历史，不把视觉资源释放与用户状态丢弃绑定。

## 未提交的无冲突合并期间 Checkout（2026-10-23，实施前核对）

参考源码 4aec35256d765c217aa0d662b82ac4e860c396fd，安装版 Rebased 1.1.20。独立 Windows fixture `GitReview-validation/checkout-merge-audit-20261023`：main 基础提交，side 新增 merged.txt，topic 指向 main，main 执行 `merge --no-ff --no-commit side`，无未解决 index stage。原生顶部显示 Merging main，分支弹窗 Local/topic/Checkout 可用；点击直接成功，切换为 topic，无额外确认。宿主 Git 验证 MERGE_HEAD 消失，merged.txt 仍为 A、索引内容仍为 side content。截图 `artifacts/audit-20261023/checkout-merge-ref-{before,menu,result}.png`。打开自建 fixture 时仅信任该项目，取消 Defender 排除勾选；没有修改防护设置。

已读 `plugins/git4idea/backend/src/actions/ref/GitCheckoutAction.kt` 全文及 `branch/GitCheckoutOperation.java::runCheckoutAndHandleCommonFailures/smartCheckoutOrNotify/smartCheckout/checkoutOrNotify`。普通 local 与 tag 都调用 GitBrancher.checkout；未解决冲突由 Git 命令分类，不把仅有 MERGE_HEAD 等同于未解决冲突。远端另有本地分支创建/重放选择，此场景未做原生远端及 tag 点击，不扩大截图结论。

本地 `checkout::execute_local` 与原生行为一致；`reference_checkout::Prepared::validate` 却统一拒绝所有 in_progress，导致等价引用路径不一致。计划允许无未解决冲突的 Merge 状态执行普通引用 checkout，仍拒绝 Rebase choice、未解决 index 和其他进行中状态；不将本轮规则扩大到 rebase/cherry-pick/sequencer。普通引用、远端创建与 tag 的身份复验保留，执行后按 Git 结果与实际 HEAD/分支确认。产品实现与验收随后单独记录。

实施及验收：`reference_checkout::Prepared::validate` 接收 Choice，普通 checkout 允许 Merge 状态，但未解决 index、其他 operation 和 Rebase choice 的原检查保留。execute 与 recover 使用同一复验，不在 UI 复制判断。新增真实 Git 回归修复前因统一 in_progress 拒绝而失败；修复后15项引用 checkout 测试通过，覆盖 tag/远端无冲突 merge 的暂存保留及 unresolved merge/Rebase choice 仍拒绝且 HEAD/index/文件/标记不变。格式检查、Windows release 构建通过。

同一 Windows fixture 重新生成无冲突未提交 merge，最新 Plico 自有 PID12604 从分支弹窗 Tags/audit-tag/检出成功；顶部显示 audit-tag，Log 显示 HEAD (detached)，完成/中止合并入口消失。宿主 Git 确认 detached、MERGE_HEAD 不存在、merged.txt 仍 A 且 staged blob 内容保留。截图 `artifacts/audit-20261023/checkout-merge-plico-{before,menu,result}.png`。没有原生重做远端 case（由真实 Git 测试覆盖），没有更改其他操作中状态的策略或声明像素对齐；测试 Plico 已关闭，Rebased 保留。

## 2026-10-23：中止合并的结果反馈预检

在 Rebased 1.1.20（IC-262.10968.SNAPSHOT）独立 checkout-merge-audit-20261023 仓库执行无冲突的 merge --no-ff --no-commit side，点击顶部中止，再确认 Abort。截图 `artifacts/audit-20261023/merge-abort-ref-confirm.png` 显示居中确认框、Abort/Cancel；`merge-abort-ref-result.png` 显示顶部恢复 main、合并动作消失、通知明确为 “Merge abort succeeded”。Git status 只剩测试 IDE 的 .idea 未跟踪目录，merged.txt 暂存新增已撤回。保留用户 Rebased 进程。

读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/actions/GitAbortOperationAction.kt 全文：确认后后台运行 --abort，按命令结果通知 abort 成功或失败，刷新仓库和原暂存文件。此处未包含 Plico 持久保存记录恢复的同名流程，不能声称原生覆盖恢复失败。

本地 src/ui/history_operations.rs 对 restore_error 分支不区分 Commit/Abort，统一提示 Merge completed；与实际动作不符。拟保留 command/state/restore 三个独立结果，仅按 action 区分恢复失败时的提示，并补中文。恢复失败保留 stash/记录的执行器行为不改变。特殊失败场景由 tests/branch_merge.rs::unrelated_top_stash_is_not_restored_or_dropped 覆盖；Plico 原生特殊失败提示尚待验收，不能用参考普通 abort 截图替代。

Plico 后置验收：Windows release 构建通过；独立 fixture 注入合法持久恢复记录模拟重启状态，两份真实 stash 保留。原生 Abort 后准确显示中文中止/恢复待处理提示，错误详情是顶部 stash 已变化；Git 复验合并结束、保存内容和记录均未丢失。截图 `artifacts/audit-20261023/merge-abort-plico-{before,confirm,result}.png`。内部诊断仍英文，列为后续 i18n 差异；不将本次测试称为完整 Smart Merge UI 验收。

## 2026-10-23 OAuth 回调读取取消前置核对

安装版 Rebased 1.1.20 / IC-262.10968.SNAPSHOT，独立 checkout-merge-audit-20261023。Settings → Version Control → GitHub → + → Log In via GitHub，出现居中 Logging in… 动画和 Cancel；点击 Cancel 后返回空账号列表，无错误框、无新增账号。截图 artifacts/audit-20261023/oauth-cancel-ref-{entry,busy,result}.png。未完成浏览器平台授权，也未改设置；随后 Cancel 关闭设置。此观察证明等待授权可取消，不证明已安装版对慢 HTTP 请求的实现细节。

源码 4aec35256d765c217aa0d662b82ac4e860c396fd：已读 plugins/github/github-core/src/org/jetbrains/plugins/github/authentication/ui/GHOAuthCredentialsUi.kt，acquireToken 等待 future，协程取消时 completeExceptionally(ProcessCanceledException) 并重新抛出；已读同 authentication/GHOAuthCredentialsAcquier.kt，token交换独立。此前已读 GitLabOAuthService.kt，取消清理 pendingLogins。安装版 GitLab 为 Token-only，不能用本次 GitHub 实测声称安装版 GitLab OAuth 已验证。

本地 src/hosting/oauth.rs::authorize 读取头部期间缺少取消和总时限检查，慢小包探针已失败。计划保留本地轻量 listener，在每次读取前和处理响应前复验生命周期；严格限制头部缓冲区，保持 existing state/PKCE 验证与 token交换前取消检查。Windows Plico 修复后验收待做。

Windows OAuth 原生复测：本轮构建的 Plico-audit.exe PID12440，仅打开独立 checkout-merge-audit-20261023；Settings/GitLab/+ 浏览器登录，等待中点击内联取消后恢复登录按钮，无错误；端口34561可重新绑定。再次登录，Get-NetTCPConnection确认127.0.0.1:34561属于PID12440；点击整个登录页的取消后返回原账号列表，端口再次可绑定。未完成平台授权，未Apply设置，现有账号未修改；取消设置并正常关闭本轮测试程序，保留Rebased。截图 artifacts/audit-20261023/oauth-cancel-plico-{busy,result,retry,accounts}.png。

证据限制：Windows慢包探针连接后333ms即关闭，早于后续UI取消，故不能声称该平台复现了Linux持续读取取消场景；需进一步查明接受socket模式/读取错误，不能把窗口恢复当作慢包工作线程已被取消的证明。Linux正式测试仍确认原代码2.36秒失败、修复后小于1秒并释放端口。原生界面取消与重试已通过，Windows慢包边界保留待查。Plico登录表单内联等待/两个取消入口与参考GitHub居中小弹窗结构不同；安装版GitLab只有Token登录，该差异不能在本轮直接宣称像素对齐。

### Windows callback TCP 模式补充（修复前）

延续上述已完成的原生浏览器等待/取消操作及截图，本次定位实际 Windows 连接提前关闭原因。独立Rust程序 /tmp/plico-socket-audit 同时运行Windows/Linux：非阻塞listener.accept后，不显式改变stream模式，Windows首次read在2.9微秒返回WSAEWOULDBLOCK(10035)，Linux等待约150ms得到18字节；显式set_nonblocking(false)后Windows等待149.7ms读到同样18字节。500ms SO_RCVTIMEO不改变nonblocking标记。该探针没有浏览器凭据或平台访问。

重新读取4aec352 GitLabOAuthService.kt::onCallback，接收FullHttpRequest，不能把还没收到字节的连接等同完整HTTP请求。本地authorize与finish_browser均忽略此平台差异；准备通过显式配置已接受socket为有时限的阻塞模式，保持listener本身非阻塞、取消和总期限复验。原生外观无改动；延迟分段请求回归将直接执行Windows测试程序，不能以Linux通过代替。

上述TCP修复已实现于configure_callback_socket，两处调用共享平台模式配置。Windows正式分段请求回归先失败后通过，hosting 12项Windows/Linux均通过，详细日志与边界见AUDIT-2026-10-23.md的Windows OAuth分段请求误拒绝修复记录。GUI release重建及复测仍待做。

### TCP修复后的Windows原生慢包验收

重新构建release成功（/tmp/plico-audit-oauth-tcp-windows.log，49.19秒），新启动Plico-audit PID13116。GitLab浏览器等待期间发送GET /及每150ms一个字节的未结束请求；Get-NetTCPConnection确认Established和Listen同时属于13116，不再在333ms提前断连。点击内联取消后发送端退出（连接总存活38983ms，这是操作前等待时长，不是取消延迟），界面恢复登录按钮，独立TcpListener成功重新绑定34561。截图 artifacts/audit-20261023/oauth-tcp-plico-{busy,cancelled}.png。未完成平台授权、未Apply、未修改账号；关闭本次测试程序，保留Rebased。Windows正式单元测试已单独断言取消返回小于1秒，此原生操作不伪造亚秒精确测量。该结果取代前文“最新TCP修复原生复测待做”，不覆盖真实平台token交换验收。

## 2026-10-23 Squash worktree身份前置核对

安装Rebased1.1.20 / IC-262.10968.SNAPSHOT，独立Windows仓库squash-identity-audit-20261023，topic分支base/one/two三条线性提交。原生Log选择two和one，右键Squash Commits可用；居中Squashed Commit Message弹窗，预填two空行one，OK/Cancel在右下；确认后Log变为two/base，通知Commits Squashed并提供Undo。Windows Git确认HEAD从8dc2e30变为298b141，base277e638保留。截图artifacts/audit-20261023/squash-ref-{entry,confirm,complete}.png。仅信任此fixture，取消Defender排除项，不改系统安全设置。

源码4aec352：已完整读取plugins/git4idea/backend/src/rebase/log/squash/GitSquashOperation.kt及同rebase/log/GitCommitEditingOperation.kt。操作实例、editor handler、rebase spec和result均绑定构造时的GitRepository；原生操作确认仅在当前fixture执行。不能从截图推断任意跨worktree请求，核心身份语义来自源码。

本地squash_check::Prepared没有保存gitdir，独立linked worktree同detached HEAD探针已证明revalidate误接受。实现将加私有directory，prepare保存、revalidate先核验；沿用Reword/Reset现有editor_directory身份规则，保持HEAD/branch/plan/保护分支及恢复记录的再次检查。不移除UI迟到回调的repo.path保护，不引入新的操作类层次。Plico原生普通Squash及核心跨worktree拒绝修复后待验收。


### Squash 身份修复后的 Plico 原生验收

Windows Plico-audit PID39196，独立 squash-identity-plico-20261023，topic 原始 base/one/two。选择 two/one 后打开压缩提交消息，预填 two 空行 one；确认后 Log 更新为 two/base。Windows Git 断言提交数为2、HEAD 875982e 的 tree 与原始23c549c完全相同、消息为 two\n\none、index/worktree干净且没有rebase状态。点击状态栏“撤销压缩”后 Log 恢复三条，Git断言HEAD精确恢复23c549c且仍干净、没有rebase状态。截图 artifacts/audit-20261023/squash-plico-{entry,confirm,complete,undone}.png。已正常关闭本轮PID39196，保留Rebased。

该结果补齐普通Squash/Undo原生回归；跨worktree拒绝由正式同HEAD不同gitdir测试证明，不冒称从GUI触发该内部场景。入口、消息预填和执行结果与本轮参考一致；Plico成功反馈位于状态栏，Rebased为通知，且Plico保留结果选择，参考执行后清空选择，未宣称像素及选择行为完全对齐。


## 2026-10-23 Markdown 仅源码模式生命周期前置核对

Rebased 1.1.20 / IC-262.10968.SNAPSHOT + Markdown 262.10968.63，现有独立 markdown-20261023/README.md，在测试窗口35484打开。预览模式显示标题、表格和代码块；点击右上Editor only后显示同一文档源码，模式按钮保留且源码按钮选中。截图 artifacts/audit-20261023/markdown-source-ref-{preview,source}.png。未编辑文件、未操作其他用户窗口；本次没有自动滚动。截图证明布局与模式切换，不作为资源释放证据。

源码4aec352：platform/platform-impl/src/com/intellij/openapi/fileEditor/TextEditorWithPreview.kt::setLayout/adjustEditorsVisibility 将SHOW_EDITOR的preview.component设为不可见；plugins/markdown/core/src/org/intellij/plugins/markdown/ui/preview/MarkdownPreviewFileEditor.kt::AttachPanelOnVisibilityChangeListener.componentHidden 调用detachHtmlPanel，移除并dispose panel，再显示时attach。与隐藏整个文件Tab是同一面板可见性规则。

本地src/project_view.rs::attach_document不检查markdown_mode，始终创建MarkdownPreview；按钮回调只修改mode，Render的Source分支隐藏视图但保留实体，其InputEvent::Change订阅仍刷新解析。源码模式启动和从Split切Source两条路径都需覆盖。准备将预览创建/释放独立于编辑文档绑定，并让工具栏可用性按Markdown文件类型及文档可用状态判断；切回时从保留编辑器当前文本创建，保留Undo与分栏尺寸。尚未改此产品行为或运行此场景回归，不把之前隐藏整个Tab的测试当作已覆盖。


### 仅源码模式修复 Windows 原生验收

最新xwin release Plico-audit PID26180，独立markdown-20261023/README.md。项目树打开后双栏正常；切Editor only，源码占满且三个模式按钮保留。源码内输入SourceModeAudit，再切Preview only，渲染包含该新文本；切回Split后Ctrl+Z删除该输入，源码与预览同时更新，再次Undo并保存后文件字节与本轮操作前备份完全一致。Source模式关闭文件Tab再从项目树打开，仍为Source且模式工具栏可用。最终恢复本轮初始Split偏好，正常关闭仅本次Plico进程，保留Rebased。截图artifacts/audit-20261023/markdown-mode-plico-{split,source,preview,undo,reopen-source}.png。

该原生结果证明模式入口、最新文档重建和Undo跨模式保留；资源释放由弱引用/订阅回归证明，不以截图或一次内存读数推算节省量。没有新增滚动自动化，原生渲染器与JCEF既有表格/代码块等视觉差异继续保留。


## 2026-10-23 Define Remote 失败诊断前置核对

Rebased1.1.20 / IC-262.10968.SNAPSHOT，独立squash-identity-audit-20261023仓库，没有remote。Manage Remotes打开空表，+打开Define Remote，默认origin、URL初始焦点、Fetch remote勾选。输入不存在的本地路径C:/Plico-audit-missing-20261023.git并点击OK，URL边框转红、焦点回URL，上方错误浮层保留Git原始原因“does not appear to be a git repository / Could not read from remote repository / access rights ... exists”，表单及输入保留，OK仍可用。截图artifacts/audit-20261023/remote-error-ref-{entry,failure}.png。取消两个弹窗后Windows Git remote仍为空。未连接真实远端或修改凭据。搜索自动化第一次因激活主窗关闭临时搜索框误输入fixture README，已用Undo恢复，并逐字节确认与本轮前备份相同；不将该自动化失误记作产品问题。

参考4aec352已完整读取plugins/git4idea/backend/src/remote/GitDefineRemoteDialog.kt：checkUrlAccess失败使用result.errorOutputAsHtmlString构建ValidationInfo，doOKAction保留表单并聚焦URL，后续重试仍需成功。GitConfigureRemotesDialog.kt::runInModalTask也显示各命令errorOutputAsHtmlString并更新repository。此处确认诊断信息是实际可见行为，不仅日志内容。

本地src/define_remote_dialog.rs::check将Outcome映射为bool，Ok(false)只留下通用提示；失败Git stderr已在这一层丢失。src/ui/remote_management.rs同样仅把报告映射为success/error摘要。准备沿现有clone_repository的有界、脱敏诊断模式保留检查失败详情，继续使用既有error_ui详情容器，避免长Git输出撑开表单；与上游红框浮层的视觉形式差异须明确记录。不要把取消当作可继续保存，保留关闭、迟到结果、重新认证及配置复验门槛。本轮尚未修改产品实现或做Plico同场景新原生验证。


## 2026-10-23 Diff 工具栏跨文件导航前置验证

安装版 Rebased 1.1.20 / IC-262.10968.SNAPSHOT，独立 squash-identity-audit-20261023，在三个已跟踪文件追加测试行（无提交/远端操作）。Commit面板打开base.txt Diff，显示1/3 files；点击工具栏向下箭头一次显示“Press again to go to the next file”，再次点击切到one.txt与2/3 files，左侧选中项跟随。向上箭头第一次显示previous提示，第二次返回base.txt与1/3 files。截图 artifacts/audit-20261023/arrow-ref-{entry,next-hint,next-file,prev-hint,prev-file}.png。原生观察证实鼠标入口同样二次确认，不需改按F7；没有执行滚动自动化。

源码参考4aec35256d765c217aa0d662b82ac4e860c396fd：platform/diff-impl/src/com/intellij/diff/actions/impl/DiffDifferenceNavigationAction.kt::actionPerformed、DiffNextDifferenceAction.kt及impl/DiffRequestProcessor.java::DiffNavigator，文件内导航优先，否则prepare NEXT/PREV后同向第二次切文件；提示不限定按键。源码快照与安装版版本不同，本次行为一致。

本地实际鼠标事件测试连续两次点击仍停index0；首击到达navigate且无loading。src/diff_tab.rs根容器on_mouse_down清除确认，而按钮on_click重建，导致鼠标无法完成二次确认。准备仅在上下导航按钮消费鼠标按下事件，保留内容区点击/滚轮/其他按键清除提示；共用navigate和现有跨文件焦点/加载机制，不新增导航状态机。提示改为按钮与快捷键均适用的“再次操作”文案。5秒自动取消沿用用户既有要求，是保留差异。底部预览/Unified原生组合尚待验证。


### Diff 箭头跨文件修复及验证（2026-10-23）

根因确认：工具栏按钮的鼠标按下事件冒泡到DiffTab根容器，提前清除了navigate二次确认状态。仅在上下差异按钮消费Left mouse-down，内容区点击/滚轮/其他按键仍清除；实际点击回归由index0失败转为通过，未改变加载/导航状态机。提示改为中英通用“再次操作以转到下一个/上一个文件”，与参考Press again语义一致。

回归已扩展为独立/嵌入两种布局、前后两个方向，以及点击内容区取消确认。测试只隔离边界事件，不代替真实Git加载：跨文件后deactivate并清除缺失fixture异步错误再测反向。完整桌面75通过、2默认忽略；两个绘制工具栏测试均遇到已有GPUI duplicate-hover debug断言，独立使用debug-assertions=false运行整个editable_navigation_tests模块7项全部通过、无忽略（普通assert仍生效）。CI专用命令和贡献规范同步到该模块，不能仅报告默认绿色。日志/tmp/plico-arrow-{desktop,navigation-tests}.log；Windows xwin全部bin构建50.43秒通过，/tmp/plico-arrow-windows.log。

原生Plico-arrow PID38916使用同一独立fixture：base.txt1/3，第一次下箭头显示中文通用提示，第二次切到one.txt2/3，左侧文件选择跟随；连续两次上箭头返回base.txt1/3，未需点击编辑区恢复焦点。截图artifacts/audit-20261023/arrow-plico-{entry,hint,next,prev-hint,prev}.png。只关闭本次Plico-arrow，保留原Plico-audit18500和Rebased35484。测试追加内容以精确后缀校验后移除；未提交/修改Git索引或远端。

保留差异：提示在Plico底部而非参考编辑器气泡，保留既有5秒自动取消规则；本轮原生核验工作区Split独立Tab，底部/Unified事件由回归覆盖，未声称全套原生组合已验收。按钮仍在首尾保持既有可用外观（点击不越界）；未顺带调整禁用样式。下一项继续同服务器多仓库认证复用，整体审计未完成。


## 2026-10-23 HTTP 跨仓库认证范围前置核对

Rebased1.1.20 / IC-262.10968.SNAPSHOT，独立squash-identity-audit-20261023，受控HTTP Basic服务127.0.0.1:18769提供repo.git和other.git两个bare仓库。Manage Remotes添加origin，关闭Fetch，URL检查弹出Log In to 127.0.0.1:18769（标题不含仓库路径），选择Enter credentials，输入仅测试用凭据、Remember保持选中，成功。随后添加other指向另一bare仓库，URL检查成功且没有再次输入凭据；两个remote出现在列表，Git读取确认两个地址。截图artifacts/audit-20261023/auth-scope-ref-{login,first,second}.png。原生验证的是两个远端仓库；不同本地project由生产入口集成测试另覆盖，不混称已切换两个Rebased项目窗口。未使用真实账号，未Fetch/Push。

源码4aec352：plugins/git4idea/backend/src/commands/GitHandlerAuthenticationManager.java::prepare按应用偏好覆盖credential.helper，但没有强制credential.useHttpPath；GitHttpGuiAuthenticator.java::askUsername/askPassword/getRequiredUrl/PasswordSafeProvider依据Git AskPass提供的URL取得凭据，只有Git没传URL才用preset；unifyUrl只合并协议，不移除路径。本地src/authenticated_command.rs::run却无论helper与否都强制credential.useHttpPath=true，覆盖用户明确false，导致每仓库保存一次。真实Git跨两个本地project、两个远端路径的测试remembered_http_credentials_follow_git_path_scope_across_projects失败：第一个交互认证成功、第二个Silent失败。/tmp/plico-auth-path-red.log。

拟删除命令层强制路径配置，让Git根据默认/全局/仓库/URL级credential.useHttpPath生成认证范围，继续按收到的target+login保存；不自行丢弃路径或无条件按host兜底。显式true仍隔离路径；协议、端口和用户名继续隔离，不复制上游HTTP/HTTPS合并。旧版按仓库保存的条目不自动扩大授权范围；默认范围首次使用可能需要重新确认一次，之后共享。需验证helper分支、显式路径隔离、URL级配置及账号拒绝处理，Windows Plico同场景待验收。


HTTP范围方案补充（实现前）：真实AskPass断言已证实直接删除覆盖会把Pending.http_target从完整/repo.git变为host根路径（/tmp/plico-auth-target-red.log），破坏已有子路径账号匹配。改为仅应用AskPass模式保留完整路径请求；在通用凭据读写之前，用原始命令上下文（覆盖前的环境/全局参数/工作目录）查询Git config --type=bool --get-urlmatch credential.useHttpPath，再计算存储scope。缺省false按origin复用，显式true保留路径，查询失败保守保留路径。platform目标与一次操作配对仍完整；helper模式不覆盖useHttpPath。由Git解析URL特定配置，不自行重写优先级，不推断remote别名/多个push URL或submodule目标。按操作缓存有限scope查询，避免UI线程子进程；等待中的Pending应携带提前解析结果。此机制与参考的直接AskPass scope不同，理由是保留本地自托管子路径匹配和严格目标失败隔离；同服务器复用的用户行为仍遵循前述原生证据。

### 2026-10-23 HTTP 跨项目密码范围实现补充

沿用本节已核对的 4aec352 GitHttpGuiAuthenticator.java / GitHandlerAuthenticationManager.java 及 Rebased 1.1.20 原生双仓库验证。Plico 映射：http_credential_scope.rs 在 AskPass 后台线程解析原始 Git credential.useHttpPath，auth_request.rs 分离通用 credential_target 和完整 http_target；authenticated_command.rs 保留应用 AskPass 的完整目标但不再把该范围强加给 helper。必要差异：平台子路径账号仍需完整 URL，所以命令层应用模式保留 useHttpPath=true，通用存储另按用户原配置解析；没有放松平台 server 子路径匹配。Define Remote 由临时 Store 接入系统 Store，修复“记住”不能跨进程保存，与 PasswordSafe 持久化规则对齐。协议/端口隔离保持现有更严格边界，旧路径秘密不自动扩大范围。

Windows 同条件复验：首次记住后重启 Plico，换本地项目并换同 host 的 repo.git→other.git，认证检查直接成功，无再次输入。截图 audit-20261023/auth-scope-plico-{first,second}-persisted.png。参考是同项目双 remote，本地补充跨进程双项目；不混同验证范围。自动化覆盖显式 true 路径隔离及 helper 原配置，未把跨主机共享作为目标。

Define Remote 失败诊断 Plico 原生补验（2026-10-23）：自有 PID18500，独立 fixture，缺失本地仓库 → 保留表单和输入 → Details 显示 Git 原因 → 关闭仅顶层 → 编辑清除旧详情 → 重试仍检查失败。取消后 remote 列表为空。截图 artifacts/audit-20261023/remote-error-plico-{failure,details,edited,retry}.png。对应前述 4aec352 GitDefineRemoteDialog.kt 与 Rebased1.1.20 原生失败记录；Plico 使用详情弹窗的呈现差异保留。

2026-10-23 内部结果模块归并：复核4aec352 GitPushOperation.java::update 调用 GitUpdateProcess、execute 根据结果控制重试；GitUpdateProcess.java::updateImpl 仅在完成条件允许时恢复保存的更改。本地新增 update_report.rs，project_update::Report / push_flow::Update 重导出同一结果类型，统一完成/取消/保存状态解释；两入口的确认、执行和重试保持原实现。本轮为无行为变化的模块搬移，没有新界面，也不以此声称新增原生功能验收。

2026-10-23 共享凭据失败分类复核：4aec352 GitHandlerAuthenticationManager.java认证监听器区分认证失败输出与普通命令错误；GitHttpGuiAuthenticator.java::PasswordSafeProvider.onAuthFailure删除URL+login凭据。Plico保持同一403/指定400/not-found分类；新增本地真实Git的500保留、403清理、返回原项目再次Silent访问回归。此轮无行为/UI改动，未新增平台原生权限场景的验收声明。

### 2026-10-23 远端配置写入失败：原生前置完成

Rebased1.1.20 / IC-262.10968.SNAPSHOT，独立squash-identity-audit-20261023，无remote；Define Remote填origin和另一个本地fixture的.git地址，关闭Fetch。在该fixture独占创建config.lock（固定测试内容），点击OK：访问检查成功，添加配置失败，弹出Add Remote错误对话框，包含“could not lock config file .git/config: File exists”与“could not set remote.origin.url”，底层Git Remotes仍保留。截图artifacts/audit-20261023/remote-write-ref-error.png。自动化采用枚举实际模态HWND，并核对鼠标命中目标后点击，解决此前主窗焦点误判；未操作真实远端。

已读4aec352 GitConfigureRemotesDialog.kt::runInModalTask、changeRemote：执行后刷新repository，失败将Git命令错误输出合并进错误弹窗。本地ui/remote_management.rs丢失Report命令详情，Manager::complete刷新失败覆盖操作摘要。准备复用现有error_ui展示有界脱敏详情，保留底层管理表及最新配置读取；同时失败时保留操作摘要和刷新摘要。不改变确认、写入、取消、Fetch门槛；避免显示未脱敏URL。

远端写入失败本地映射已验收：Report::diagnostic保留失败命令的有界脱敏输出，ui/remote_management完成刷新后用error_ui弹出，Manager保留原操作与刷新失败摘要。Windows同条件config.lock原生复现通过，详情关闭后底层管理窗口保留；artifacts/audit-20261023/remote-write-plico-{error,manager}.png。Plico标题为Git远端且附复制按钮，参考标题为Add Remote，保持现有错误容器的差异，不宣称逐像素一致。


### 2026-10-23 Log 哈希搜索包含提交说明：实现前核验

参考4aec352 platform/util/resources/misc/registry.properties: vcs.log.filter.messages.by.hash=true。VcsLogFiltererImpl.kt::applyHashFilter 首阶段返回ID命中并允许继续加载，后续通过 toTextFilter/fromPatternsList(hashes,false)并入说明匹配，忽略其他筛选；按原图顺序显示并集，去重。Windows Rebased1.1.20，自有 squash-identity-audit-20261023 增加空提交 e004ca1（说明 audit references 277e638 in message），输入277e638后无需滚动即显示说明命中和base两个提交，two被过滤。截图 artifacts/audit-20261023/hash-ref-{log,result}.png。没有操作真实远端，工作区仅既有.idea未跟踪。

本地history::read_hashes/locate_filtered/adjacent仅匹配ID，导致已有哈希命中时漏掉引用它的提交。准备共享哈希结果集生成：在固定roots下读取ID+消息匹配并集，保留预算、取消、分页与原始父边；不加载全量消息文本。Plico没有上游内存索引首阶段，采用一次后台查询返回完整并集，保留结果发布时序差异，避免首阶段offset与并集分页混用。原生验收后再记录完成。


### 哈希说明匹配修复与验收

已读VcsLogFilters.kt::fromPatternsList、VcsLogTextFilters.kt::VcsLogMultiplePatternsTextFilter，确认多token是大小写不敏感的字面OR匹配。本地src/history.rs::hash_matches统一ID+说明结果集，read_hashes/locate_filtered/adjacent共用；有效十六进制token用ERE OR传给Git，只读取匹配ID/父边、不加载全量消息。复用现有固定roots、10万提交/50万边预算与取消检查。仍一次后台发布并集，不模拟上游内存索引快照的先ID后消息阶段；列表内命中文字的黄色强调尚未实现，不宣称视觉完全一致。

真实Git新增hash_filter_includes_message_references_in_pages_and_navigation先失败（只有base未返回reference）后通过，覆盖正文大写哈希、额外作者/路径过滤不阻断、固定roots后新增提交不泄漏、分页more与定位/投影相邻导航。定向hash2项、filter11项、navigation4项全部通过（有重叠，不能相加报告独立用例总数）。日志/tmp/plico-hash-{union-tests,filter-tests,navigation-tests}.log。Windows release全部bins构建50.09秒通过。

Windows Plico-hash-union PID33188在同一reference fixture输入277e638后显示reference和base两行，two消失，与Rebased相同；截图artifacts/audit-20261023/hash-plico-result.png。首次SendKeys被当前中文IME转换，改用定向WM_CHAR输入后核实文本正确；不把自动化输入错误记为查询缺陷。只关闭本次Plico，保留Rebased及已有Plico实例。此次未测大仓库新增消息扫描延迟，整体审计仍保留性能及超预算场景。


2026-10-23 过滤图读取优化：在前节4aec352 VcsLogFiltererImpl.kt固定图/哈希并集规则下，将Plico history::read_topology/read_matches的1000行重复git log --skip读取改为Repository::bounded_topology单次流式读取，topology.rs集中解析与10万节点/50万边限制，普通分页API不变。内部性能优化不改变UI规则；Rebased仍使用持久索引/permanentGraph，Plico仍按需重建。42,083提交本地测试副本，Windows生产核心查询23.6–23.8秒降至3.7–4.0秒（三次固定roots，含Git、不含UI/GPU，非冷缓存）；100001提交明确失败、不显示截断结果。675项核心回归及完整Windows bins通过；详见AUDIT-2026-10-23.md的测量条件、证据与尚未测量的内存限制。


2026-10-23 提交模型内部清理：复读4aec352 GitCheckinEnvironment.kt的部分index构造/排除已处理路径/未选index恢复，GitRepositoryCommitter.kt的Last Amend及作者参数。移除仅剩测试引用的commit_selection，两个真实Git回归迁到生产commit_changes::Snapshot→commit_inclusion::Inclusion；保留组范围、冲突、刷新身份、rename端点回归，复制场景用真实collector验证为A。partial Amend成功/失败新增真实Git测试证明原作者/父提交/未选staging/工作区保持；测试与局限详见AUDIT记录。无现用UI行为改变，未新增原生前置/截图声明。此前文档中旧模块路径是历史实现，不再是当前入口。


## 账号持久化失败边界复核（2026-10-23）

本轮实际重读参考 4aec35256d765c217aa0d662b82ac4e860c396fd：
`platform/collaboration-tools/src/com/intellij/collaboration/auth/AccountManagerBase.kt`
的 doUpdateAccount/doRemoveAccount/saveCredentialsSafe，以及同目录
`PasswordSafeCredentialsRepository.kt` 的 persistCredentials/retrieveCredentials。
参考在 dataGuard 内以 NonCancellable 更新账号；先保存或删除凭据再更新账号集合，
saveCredentialsSafe 捕获并记录持久化异常，仍继续集合和事件更新；PasswordSafe 在 IO dispatcher 存取。
这不是凭据与账号集合的跨存储事务保证，不能直接推导为失败不会残留凭据。

本地映射 src/hosting.rs、src/hosting_settings.rs、src/credential_store.rs：显式返回存储错误，
设置保留草稿供重试；保留差异是错误对用户可见而非只记日志。多条写入一致性仍待故障注入，
本轮未调整运行时策略。Windows 单键原生测试与 CI 门禁只证明测试键的保存、预检拒绝、
条件删除语义；未模拟系统后端不可用，也未操作真实账号。安装版仍为 Rebased1.1.20 /
IC-262.10968.SNAPSHOT，本项仅补内部测试，没有新增原生 UI 对齐结论。


## GitLab 账号移除生命周期：原生前置与源码（2026-10-23）

安装版 Rebased1.1.20 / IC-262.10968.SNAPSHOT，独立进程24776；config/system/log/plugins均位于自建account-lifecycle-reference-20261023目录，未复制真实账号或Token，保留原进程35484。按GitLabPersistentAccounts的序列化结构放入唯一fixture账号gitlab.invalid/lifecycle-fixture，无凭据。克隆页识别账号，显示Missing access token，头像菜单仅Log In with Token。设置GitLab选中账号→减号：No accounts added，Apply可用、显示Revert changes；Apply/OK后独立gitlab.xml账号集合为空。重新打开Clone/GitLab：No accounts与登录表单，无旧账号错误行。截图artifacts/audit-20261023/account-lifecycle-ref-{gitlab-selected,menu,selected,removed-draft,after}.png。仅验证缺失凭据账号正常移除；未人为破坏PasswordSafe、未触发真实过期Token刷新或OS删除失败。

参考源码4aec352：
- plugins/gitlab/gitlab-core/src/org/jetbrains/plugins/gitlab/authentication/accounts/GitLabAccountManager.kt：getAndRefreshCredentials在dataGuard内刷新并doUpdateAccount，与AccountManagerBase移除串行；不能概括为其HTTP请求也在锁外。
- 同目录GitLabPersistentAccounts.kt：账号集合更新发布accountsFlow。
- plugins/gitlab/gitlab-core/src/org/jetbrains/plugins/gitlab/ui/clone/model/GitLabCloneRepositoriesListViewModel.kt：accountsState.mapStatefulToStateful构造每账号任务。
- platform/collaboration-tools/src/com/intellij/collaboration/async/MappingScopedItemsContainer.kt::update：缺失账号对应scope.cancel；同模块test/.../CoroutineUtilTest.kt的mapStatefulToStateful cancels deleted scopes测试已读。
- plugins/gitlab/gitlab-core/src/org/jetbrains/plugins/gitlab/api/GitLabProjectConnectionManager.kt：集合不再包含currentAccount时closeConnection。

本地拟映射：src/hosting.rs保留网络期间不持ACCOUNTS锁的轻量策略，读取凭据和刷新回写时在该锁内同时复验账号索引与凭据版本；账号索引已移除时拒绝旧任务取得或更新孤立Token，不自动恢复账号。不能把这一防线宣称为所有页面都会实时取消或多记录存储事务。系统删除失败由独立内存故障注入覆盖，与本轮原生正常移除证据分别记录；Plico同条件UI验收仍需完成。

后续实现记录：上述本地映射已落地到credentials_from；原失败探针转为正式回归并由红转绿，增加残留Token不得发起刷新用例。核心683项、Windows账号19项及原生凭据1项通过。正常账号移除参考已原生验证，Plico页面验证与系统后端故障原生验证仍分别保留，不称全面对齐完成。


## 进程树清理验证补充（2026-10-23）

重新读取4aec352的platform/platform-util-io/src/com/intellij/execution/process/OSProcessHandler.java::doDestroyProcess/killProcessTree，递归关闭及失败回退规则见源码。Plico映射保持src/process_tree.rs的Job Object/process group所有权不变。仅强化tests/process_tree.rs失败路径自有测试进程清理；Windows原生test target5项通过，包括leader已退出、无关进程保留、管道关闭及32次启动句柄回落；Linux4项通过。此次无行为/UI修改，未另操作Rebased界面，不能据此声称所有Git命令取消时机与上游一致。


账号移除本地原生补验：当前Windows构建Plico34348，仅追加并操作.invalid服务器fixture账号。移除→取消后重新打开账号仍在；移除→Apply后账号与Windows唯一凭据键均不存在（CredReadW=1168），其他账号ID顺序保持基线。重开Clone不再提供fixture，保留原有用户账号，未强行制造No accounts空页。截图见artifacts/audit-20261023/account-lifecycle-plico-{draft,cancel-retained,applied,clone-after}.png。此结果补齐普通移除UI验收，不替代故障注入测试的删除失败/刷新竞争验证。头像、内联凭据状态和选择样式差异仍保留，无像素对齐完成声明。


## 二进制冲突手动处理：原生差异已确认（2026-10-23）

独立 binary-conflict-reference-20261023 / binary-conflict-plico-20261023 仓库均含 main/feature 两侧 NUL 字节冲突，以及 unrelated.txt 的不同暂存/未暂存内容。Rebased 1.1.20（IC-262.10968.SNAPSHOT）隔离进程41428的 Conflicts 窗口实际采用逐侧 Accept、Resolve Manually；后者打开三栏 Merge Revisions，分别为 Changes from main / Result / Changes from feature，未知类型显示 Cannot show diff for an unknown file type，底部提供 Accept Left、Accept Right、Cancel，没有手工编辑后完成按钮。截图 artifacts/audit-20261023/binary-ref-{conflicts,manual}.png。与旧源码的冲突列表布局有版本差异，不能声称列表已像素对齐。

再次完整读取参考4aec352的 platform/diff-impl/src/com/intellij/diff/merge/BinaryMergeTool.java：以 ThreesideBinaryDiffViewer 呈现三份内容，getResolveAction(RESOLVED) 返回 null，左右采纳/取消交给 MergeUtil。与本次手动处理原生结果一致。Plico当前原生进程30380点击合并后弹出 Unsupported merge text: Binary，截图 binary-plico-{conflicts,merge-error}.png。src/ui/conflicts.rs::merge 无类型分流，src/ui/text_merge.rs 将 Prepared::text_inputs 的错误作为打开失败；不是采纳核心失败。下一步应在准备结果到展示层的边界区分文本与二进制，沿用既有采纳/复验入口，不放宽文本校验，也不能用禁用合并按钮冒充对齐。该修复尚未实现。

现有核心定向 resolves_binary_and_modify_delete_without_touching_other_paths 已通过（/tmp/plico-binary-conflicts-core.log），覆盖二进制/修改删除、左右版本及无关路径保留。原生此次仅打开界面未执行采纳；分别复验两个fixture仍 UU binary.bin、MM unrelated.txt，文件字节与暂存字节均保持。未以核心测试宣称原生采纳已验证。本轮新增对照证据，无产品代码修改、提交或推送；整体审计继续。


### 二进制合并入口修复与原生验收

上轮已完成对应Rebased实机与源码前置。本轮 src/merge_result.rs::native_text_inputs 明确返回文本或二进制（Option），IO/超限/不支持编码仍作为错误，不通过匹配错误字符串分流；text_inputs 保留文本专用拒绝语义。src/ui/text_merge.rs 仅在文本内容上构建Merge模型，二进制交给新增 src/ui/binary_merge.rs 三栏占位及 Accept Left / Accept Right / Cancel。左右采纳复用 ui/conflicts.rs::resolve_conflict 与已有 with_saved_documents / Snapshot复验/写入/刷新，不复制Git操作。界面没有“手工编辑后完成”按钮，与BinaryMergeTool.getResolveAction(RESOLVED)==null一致。增加中英文文案。

回归 tests/conflicts.rs::native_merge_classifies_binary_versions_without_changing_files 覆盖工作副本、base、yours、theirs分别含NUL，准备与丢弃不写文件/索引；文本专用调用继续拒绝Binary。冲突集成9项通过，日志/tmp/plico-binary-tests.log；桌面cargo check通过/tmp/plico-binary-check.log；Windows release全部bins构建61秒通过/tmp/plico-binary-windows.log。原生Rebased点击Accept Right得到theirs二进制字节并暂存、未提交，unrelated.txt staged/unstaged都保留。Plico新构建PID38804同场景先取消（仍UU且工作字节保持），重新打开后采纳右侧（目标工作/索引均为theirs，unmerged为空，MERGE_HEAD仍存在），无关文件两份字节均保持。截图 artifacts/audit-20261023/binary-ref-accepted.png、binary-plico-fixed-{view,cancel,accepted}.png。原生只验证采纳右侧，左侧及反向rebase核心路径由既有测试覆盖，尚未做新原生矩阵。

保留差异与后续：Plico沿用应用内模态与“你的版本/合并结果/对方版本”标签，没有Rebased独立窗口、分支Show Details入口；采纳完成后沿用现有“所有冲突已解决”列表而非直接关闭。未知类型占位适用于已识别二进制，未实现图片等二进制专用查看器。不支持编码和超过FILE_LIMIT的输入继续报告错误，未通过放宽读取预算隐藏差异；此轮仅修复已复现的有界二进制打开失败，不宣称整个二进制处理能力已完整对齐。整体审计仍进行中，未提交推送。


## 编辑提交信息的引用发布失败边界（2026-10-23）

再次读取参考4aec352的 plugins/git4idea/backend/src/inMemory/rebase/log/reword/GitInMemoryRewordOperation.kt（requiresWorkingTreeUpdate=false）、同级 GitInMemoryCommitEditingOperation.kt::execute/updateRefToNewHead/copyNotesForRewrite/runPostRewriteHook、GitUtil.java::updateHeadReference 及 commands/GitImpl.java::updateReference。确认先生成对象、复验HEAD、发布引用，成功后才复制notes和调用hook；发布失败进入Incomplete。上游updateReference传入HEAD与新ID，本地src/reword.rs额外传旧OID并使用已确认分支ref/--no-deref作为发布边界；这种保守检查保留，不为了代码形式一致退回无旧值检查。

新增真实Git回归 tests/reword.rs::locked_reference_failure_preserves_state_and_allows_explicit_retry：普通分支与detached两种状态，编辑旧提交且有后继、notes、同时暂存/未暂存修改；准备后创建本测试独占的引用lock模拟另一写者，确认execute失败来自cannot lock ref，而不是提前选择拒绝。检查HEAD、所有refs（含notes）、HEAD reflog、原始index字节、工作文件都未改变，锁内容不被清理；只由测试自身释放锁，再以同一准备显式重试，消息修改/notes复制成功，index/worktree仍原样。允许失败阶段留下不可达Git对象，这是原有对象生成后发布模型，不伪称完全无磁盘写入。

Linux reword全11项通过（/tmp/plico-reword-publication-boundary.log）；交叉编译后的Windows测试exe实际执行10项通过（/tmp/plico-reword-windows-native.log），差额为#[cfg(unix)] post-rewrite hook用例。没有新增忽略项，没有产品/UI行为修改，因此本轮不以核心故障注入冒称Rebased或Plico原生错误弹窗验收。最新全量684结果来自前轮，本轮新增1项只做相关测试集验证。源码边界与测试结果未显示此处新缺陷，不为制造改动重写已正确流程。整体审计仍未完成，未提交推送。


## 外部结束合并未刷新操作状态：原生已复现

当前两独立binary-conflict-{reference,plico}-20261023 fixture在上轮采纳后均有MERGE_HEAD、无unmerged index。分别启动隔离Rebased34276和Plico34240：前者顶部Merging main，后者顶部完成合并/中止合并。Rebased信任提示先取消Defender排除且不勾选信任整个父目录，没有更改防护配置。外部对两个fixture各执行git merge --quit，程序断言HEAD、show-ref、ls-files --stage和两个文件字节均不变，只有合并元数据退出，/tmp/plico-operation-refresh-quit.json记录执行。等待超过5秒轮询周期并重新取图：Rebased分支标题恢复main（继续/中止图标在本次图中仍可见，不宣称所有控件都已消失）；Plico仍显示完成合并/中止合并。截图artifacts/audit-20261023/operation-refresh-{ref,plico}-{before,after}.png。最初并行截图互相遮挡，已串行重取参考after，最终记录使用重取版本。

源码实际读取4aec352 plugins/git4idea/backend/src/repo/GitRepositoryUpdater.kt全文，以及GitRepositoryFiles.java::isMergeFile/isRebaseFile：MERGE_HEAD和rebase目录变化会触发repository.update，独立于HEAD/branch变化。Plico src/worktree_events.rs::Pending::push仅将index/HEAD/packed-refs/refs归入index事件，忽略MERGE_HEAD/rebase等；ui/worktree_sync.rs仅刷新文档/文件树/本地变更，没有操作状态同步；ui/auto_refresh.rs只比较branch_updates::References，故refs完全相同的退出不会进入refresh_impl来刷新operation_state。手动Log刷新会重读operation_state并重建reword门禁，但自动路径缺失。

下一步需要明确区分操作状态失效与普通文档/索引变化，覆盖merge/rebase/cherry-pick/revert/sequencer及linked worktree/UNC fallback；复用已有操作状态解析和菜单门禁，避免每个文件事件都重读整个Log。本轮只复现并定位，产品修复尚未实现。测试fixtures保持merge已quit且暂存/未暂存内容原样；两个本轮进程保留用于后续同场景复测，用户原有进程未动。整体审计继续，未提交推送。


## 操作状态独立刷新：实现与原生验收（2026-10-23）

前节Rebased原生场景和4aec352 GitRepositoryUpdater.kt/GitRepositoryFiles.java提供前置依据。本地src/operation_state.rs新增Observation，以实际worktree Git目录的HEAD、index、merge/rebase/cherry-pick/revert/sequencer及嵌套元数据共11项metadata作为失效键；变化后调用既有operation_state解析，再复验键，失败或并发变化不缓存为正常状态。src/ui/auto_refresh.rs在既有轮询中独立比较操作状态；仅状态变化时更新顶部操作与src/ui/reword.rs的选择门禁，不重载Log。仍保留操作中、文档锁、查询中及取消结果的接纳保护。

保留机制差异：Rebased监听Git元数据事件，本地目前使用已有5秒轮询，尚未增加操作标记的即时事件通道；轮询包含一次Git目录查询和11项metadata读取，不能宣称零Git命令或已完成性能基准。linked worktree使用各自Git目录，UNC沿同一路径轮询，但尚无本轮UNC原生测量。该实现解决漏刷新，不宣称响应时延完全对齐。

Windows新构建PID31700，独立operation-refresh-fixed-20261023：no-ff/no-commit合并后显示完成合并/中止合并；外部merge --quit（已验证HEAD、show-ref、index条目保持）后，无手动刷新，两个操作按钮消失。前后选中topic同一提交、added.txt及右侧详情保持，截图artifacts/audit-20261023/operation-refresh-fixed-{before,after}.png。截图间隔不能当作精确刷新耗时；此场景选中topic不在main祖先链，不能据此声称编辑消息门禁已原生验收。

tests/conflicts.rs新增真实merge quit且refs不变、稳定重复观察和取消，以及linked worktree元数据隔离和嵌套head-name/sequencer todo变化测试；后者部分用合成标记，非完整原生rebase/cherry-pick流程。冲突11项通过（/tmp/plico-operation-observation-tests.log），桌面76通过/2默认忽略（/tmp/plico-operation-observation-desktop.log），Windows全部bins构建通过（/tmp/plico-operation-observation-windows.log）。导航专用7项沿用此前结果，不称本轮重跑。cargo fmt检查通过。整体审计继续，未提交推送。


## 历史编辑与外部合并交错：执行边界复核（2026-10-23）

重新读取4aec352 GitRewordAction.kt及GitCommitEditingActionBase.kt中update/checkCommitsEditingAvailability/getProhibitedStateMessage：合并中禁止改写，rebase中的HEAD有专门例外；包含分支信息尚未加载时可在执行入口进一步检查。本地ui/reword.rs的Gate负责选择可用性，Prepared::execute取得operation_guard后再次validate，读取真实operation_state，不依赖上一次菜单结果。上轮refresh_reword_gate修复只更新呈现缓存，不能代替此执行复验。

新增tests/reword.rs::prepared_message_rechecks_external_merge_without_head_change：准备HEAD消息编辑后，真实git merge --no-ff --no-commit topic，明确断言HEAD及show-ref未变；旧Prepared执行和新prepare都拒绝，错误为another Git operation；refs、reflog、原始index、MERGE_HEAD和新增工作文件保持。git merge --quit后新prepare恢复，旧Prepared显式重试成功，暂存index和文件仍保留。没有自动重试或修改产品行为。首次测试编译发现Completed无Debug，不为测试扩大产品derive，改用err().expect；最终reword整组12项通过、0失败/忽略，日志/tmp/plico-reword-operation-transition.log。git diff --check通过。

本轮是核心真实Git交错验证，不是原生菜单禁用/重新启用验收；该Windows UI组合仍待核对，不能以测试代替。前轮全量687项结果不包含本轮新增用例，未虚增全量计数。整体审计继续，未提交推送。


## 操作状态切换后的消息编辑菜单：Windows 原生验收

本轮重新确认固定构建Plico31700仍存活且窗口属于operation-refresh-fixed-20261023，选择main HEAD的base提交，右键菜单“编辑提交信息”正常可用。先尝试Shift+F10未打开菜单，随后改用向测试窗口定向发送右键事件；不将未出现菜单的截图作为验收。外部真实merge --no-ff --no-commit topic后，等待既有自动检测，重新打开同一提交菜单，消息编辑及撤销提交变灰，顶部显示完成合并/中止合并。外部merge --quit后无手动刷新，重新打开菜单恢复可用，顶部操作消失，选择仍为同一base。截图artifacts/audit-20261023/reword-state-plico-{normal,merging,restored}.png。证明重新打开菜单的状态，不证明已展开菜单会原位动态更新，也未点击执行改写。

fixture初始已有此前merge quit留下的A added.txt；第一次重做merge被Git正常拒绝，未作为产品错误。随后备份本测试文件和index，只移除该测试文件暂存及文件后重新merge生成；quit后复验status仍仅A added.txt，工作与暂存blob均与原备份一致，未触碰用户仓库。没有恢复旧index字节覆盖潜在外部写者，不宣称index元数据逐字节不变。

对应上游GitRewordAction/getProhibitedStateMessage源码规则见前节；本轮完成Plico侧组合，未重新执行Rebased同一菜单全序列，参考实机仅沿用此前操作状态刷新证据，不将其扩大为同条件菜单验收。无新增产品代码、提交或推送。下一步仍需补上Rebased菜单序列以及检查已展开菜单的失效处理。


## Rebased 消息编辑菜单状态及展开期间变化：原生对照

Rebased1.1.20独立进程34276、binary-conflict-reference-20261023，选中main HEAD yours。正常右键Edit Commit Message可用；本轮仅为隔离操作标记事件，使用exclusive create临时写入指向已有feature的MERGE_HEAD，没有重新运行merge，不能称为完整合并工作流。展开菜单保持不关闭：顶部随后变为Merging main，而原菜单Edit Commit Message仍呈可用外观。Escape关闭再右键，同项变灰。删除本轮创建且内容复验一致的标记，第一次迅速重开仍处于旧合并状态；后续重新打开时顶部main恢复、Edit Commit Message可用。截图artifacts/audit-20261023/reword-state-ref-{normal,open-menu,merging,restored}.png。未点击旧菜单项，不能据此外推上游执行安全性。

前后sha256验证原index、binary.bin、unrelated.txt字节不变，show-ref原样，测试标记已移除；/tmp/plico-ref-menu-invariants.json。保留用户及本轮参考进程，没有修改防护设置。本地src/ui/log_history.rs在context_menu构建时捕获can_reword结果，click回到ui/reword.rs再次检查；Prepared::execute仍读取真实Git状态，前轮真实merge交错测试已证明拒绝。此对照不支持将“展开菜单未动态变灰”单独认定为Plico相对Rebased缺陷，不为此重做菜单框架。两边重新打开时的正常/禁用/恢复行为一致；响应时延未精确量测，Plico5秒轮询差异仍保留。

本轮新增原生证据，无产品修改、提交推送。操作刷新与消息编辑这条链路的本次检查收束；完整审计继续转向其他未覆盖项，不将此场景等同全部历史菜单、rebase或冲突组合通过。


## 提交恢复的目录替换边界与职责复核

参考4aec352 GitIndexInfoStagingAreaStateManager.kt::prepareStagingArea/restore再次读取：排除项暂存mode/OID先保存、临时恢复HEAD，提交后恢复；restore异常上游仅warn。本地持久记录及显式恢复是已声明的额外保护，不伪称Rebased提供同一恢复UI。

检查commit_operation::execute_impl、commit_recovery::restore、commit_feedback::from_report、local_view::execute_submit：Git执行/恢复策略、纯错误呈现、界面草稿生命周期分开；restore使用新Cancel，提交取消不打断必要清理；恢复记录阻止新提交覆盖，显式恢复不会再次执行commit，失败不触发push。UI仅在完整成功时清草稿和partial选择，失败保留并触发Changed刷新。没有为了审计引入通用操作框架。提交成功但恢复失败后草稿保留的具体原生呈现仍需验证，不把源码推断当作已完成UI验收。

新增tests/commit_index.rs::explicit_recovery_does_not_replace_a_new_staged_ancestor：post-commit测试hook创建owned index.lock使未选新增nested/excluded恢复失败；删除本测试lock后，另一写入把nested暂存为文件。恢复必须报告overlaps，原始index字节、新父文件blob/工作内容、HEAD、恢复记录保持。与已有新增后代路径测试互补，避免恢复旧子项时悄悄替换新父项。commit_index全37项通过、0失败/忽略（/tmp/plico-recovery-ancestor.log）。本轮仅新增测试，无产品修改或原生新验收。未提交推送，整体审计继续。


## 提交成功但暂存恢复失败：Windows 原生验收

本轮使用当前Windows构建、自建commit-recovery-native-20261023及自有Plico40084。两个文件：chosen.txt工作修改，excluded.txt同时有saved staging与working version。只勾选chosen，提交消息recoveryauditcommit；测试post-commit hook仅创建内容fixture-owned-lock的index.lock，真实commit成功后阻止排除项恢复。Log出现新提交，chosen退出变更树；提交输入保留，提示“需要恢复提交前的暂存内容”，有详情/关闭提示及“恢复暂存内容”入口，普通提交按钮禁用。截图artifacts/audit-20261023/commit-recovery-{ready,failure}.png。未实际打开Details，本次不声称详情交互已验证。

只在复验lock内容后删除测试锁及测试hook，再点击恢复。界面显示“暂存内容已恢复”，恢复入口消失；Git断言HEAD与恢复前已成功commit一致、总提交数仍2、恢复记录删除、excluded索引回到saved staging，工作区仍working version，chosen工作内容保留。截图commit-recovery-restored.png。没有新增提交或自动push（fixture无remote，网络push失败路径不在本次范围）。原消息继续保留，与当前完整成功才清草稿策略一致；是否需要更清晰区分“commit已成功”属于呈现评估，不将此验收直接推导为应改草稿生命周期。

此次验证本地持久恢复功能，Rebased该saver仅warn且无同等持久恢复入口，源码差异见前节；没有修改产品行为，不声称新Rebased故障实机覆盖。测试键盘传入含空格消息但输入显示无空格，尚需区分SendKeys/参数传递与真实输入处理，本轮提交恢复结论不依赖消息空格；留作后续输入核验线索。整体审计继续，未提交推送。


## Log 空筛选恢复时的选择差异（2026-10-23）

实现前原生对照：已安装 Rebased 1.1.20 / IC-262.10968.SNAPSHOT，独立 binary-conflict-reference-20261023 仓库；Plico 为独立 commit-recovery-native-20261023 仓库。选中提交后输入 absent-audit-20261023 并回车，随后清空筛选并回车。两者空结果均清除详情；恢复列表后 Rebased 保持 No commits selected，Plico 自动选中第一条并展示详情。截图 artifacts/audit-20261023/log-filter-{ref,plico}-{empty,cleared}.png；不同 fixture 的提交内容不作视觉像素比较，仅验证选择生命周期。未执行滚动或 Git 写操作。

已读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/table/SelectionSnapshot.kt 全文及 VcsLogGraphTable.java::updateDataPack。更新前按 commit ID 快照，替换 model 后仅恢复仍可见的旧选择；无选择时不补第一行。源码快照与已安装版本不同，但该场景行为一致。本地 src/ui/repository.rs::refresh_impl 将非 preserve 查询的空选择补为第一行，需与首次加载区分；候选原因还包括空结果清理及迟到详情回写。新增桌面真实 Git 查询回归在 src/ui/log_query.rs，先验证选中→空筛选→清空筛选的生产入口。产品逻辑尚未修改；首次打开、分页与刷新保留选择另需覆盖，不能只据这一截图删除全部默认行为。


### Log 空筛选选择修复与回归

真实 Git + GPUI 生产 refresh 入口回归已先失败：clearing_an_empty_filter_does_not_select_an_unrelated_commit 在最后“clearing filters must preserve empty selection”断言失败，前面的空列表/空详情检查均通过；/tmp/plico-filter-selection-red.log。由此将错误定位到恢复列表时重新选择，而非旧详情未清理。单变量修正 src/ui/repository.rs：查询开始捕获 initial_history = snapshot.is_none()，仅首次历史加载且非 preserve 时允许默认选首行；其余更新沿用已有按 ID reconcile。去掉无必要的 previous 行号回退，不增加状态字段或额外缓存。

修复后同一测试通过，并覆盖首次打开仍默认选择、手动刷新保留可见选择。完整桌面77通过、0失败、2默认忽略，日志 /tmp/plico-filter-selection-{green,desktop}.log；默认忽略仍为已记录 GPUI 工具栏断言，未称本轮重跑导航专用配置。cargo fmt --package plico 已执行。保留差异：首次打开默认选首行沿用产品行为，本次没有参考空白初始启动；窗口分页锚点和空结果提示布局未改。修改后的 Windows 原生同场景复验尚待执行，当前不能宣称该 UI 修复已完成原生验收。未提交推送，整体功能审计继续。


### Log 空筛选修复：Windows 原生复验通过

Windows xwin release 全部 bins 构建完成（49.59秒，/tmp/plico-filter-windows.log）。自有 Plico PID26156 从 filter-selection-build-20261023 启动，打开独立 commit-recovery-native-20261023：初始选首行；搜索 absent-audit-20261023 后无列表/旧详情；清空后两条提交重新出现但均未选中，右侧显示“选择提交以查看更改”；主动点击首行后 chosen.txt 和该提交详情正常加载。截图 artifacts/audit-20261023/filter-selection-fixed-{entry,empty,cleared,reselect}.png 已逐张查看。与此前 Rebased 清空筛选保持无选择的观察一致。全程无 Git 写操作，无滚动自动化，其他用户进程未关闭。本轮不把进程内存采样作为性能基准；启动布局与旧截图侧栏可见性不同，不称像素一致。产品代码本轮未再改，未提交推送。

后续检查线索：refresh_impl 的 filtering 错误分支会 snapshot=None，但保留 log_selection；这与成功空结果的 reconcile 不同，需单独验证查询失败后恢复。已读取参考4aec352 VcsLogFilters.kt::fromPattern：开启正则且检测为正则时尝试 Java Pattern.compile，语法异常回退字面匹配；本地 LogFilter::args 直接将 regex=true 传 --extended-regexp。该源码差异尚未完成相同输入的 Rebased/Plico 原生对照，不能当成已验证缺陷或直接采用 Rust regex 替代（语法兼容性仍需考虑）。下一步用非法正则与合法字面符号核对，再决定是否修正，保留全部过滤组合范围。


## 主 Log / 文件历史完整性与自动加载评估（2026-10-23）

用户要求评估滚动自动加载、当前/总数量开销、右键文件历史的完整加载。本轮未改产品代码，未执行滚动自动化。

已读 Rebased 4aec352 对应源码：GraphTableModel.kt::getValueAt 在访问最后行且 canRequestMore 时 requestMore；VcsLogUtil.java::canRequestMore/requestToLoadMore 用 VisiblePack 的 MORE_REQUESTED 标记避免同一批重复请求。VcsLogRefresherImpl.kt::loadFullLog/readFullLogFromVcs 读取全部哈希，compactCommit 将ID和parents映射为int并保留timestamp，构造完整图；这不等于预加载所有文件内容或Diff。CommitCountStage.kt 的默认过滤阶段为5/100/2000/ALL，不能误称主Log始终仅加载5条。FileHistoryFilterer.kt 起始阶段30/ALL，索引可用时先走索引；否则复用相同path/hash/filters的RevisionCollectorTask，waitForRevisions(100) 返回阶段快照。RevisionCollectorTask.kt 同时支持快速首批与后台完整collectRevisions，首个完整任务结果替换快速结果；FileHistoryCollector 委托collectHistory，不设置本地1000条硬截止。本轮读到平台层，具体Git provider重命名实现尚未新增读取，不把它当作已核对。

本地映射：src/git.rs HISTORY_PAGE=1000/HISTORY_MAX=10000；src/ui/repository.rs::refresh_impl 逐次扩大查询limit，到上限换offset窗口并滚回窗口顶部；src/ui/log_history.rs 仅手动Load more/Older window。src/file_history.rs PAGE=100/MAX_REVISIONS=1000，load_root钳制limit并读取limit+1；src/history_view.rs 的more按钮在1000禁用，即即使history.more=true也无法继续浏览。后者是完整可访问性的实质缺口，不能只增加自动按钮触发掩盖。

只读计时：Linux /tmp/plico-perf-spring 与Windows本地NTFS bare副本 C:/Users/echoyn/AppData/Local/GitReview-validation/hash-profile-spring.git 均42083 commits。Windows Git实际执行3次，Python perf_counter含进程启动/WSL调用边界：rev-list --count --all 为351.2/354.5/339.7ms；带fixed-strings忽略大小写grep=fix计数442.4/396.1/386.9ms，4999条；log --follow --format=%H -- PropertySource.java 为716.3/714.8/720.5ms，33条。Linux对应总数557.8/263.4/257.3ms、消息计数303.9/309.6/309.5ms，文件历史1152.3/648.0/718.4ms。没有清系统缓存，不称冷启动；不包含GUI/图构建，不据此推算内存。文件历史命令是成本探针，并非本地合并/重命名算法的等价基准。原始数据 artifacts/audit-20261023/history-count-{linux,windows}.json。

建议顺序：先修文件历史硬截止，完整历史可达、首批快出、后台分批/可取消、内容Diff按需；主Log引入接近底部加载的独立触发门槛，固定roots并保留commit-ID选择及视口锚点，去重并拒绝旧结果。不能直接绑定refresh(true)，因其扩大重读、清详情、10000窗口切换会破坏连续体验。全量常驻提交字符串不是必需，可使用紧凑元数据/受限页缓存实现全部可达，具体方案仍需规模测量。当前数量读容器长度几乎无额外开销，但需明确“当前窗口”与“累计已加载”含义。精确总数后台计算，按roots+全部筛选语义缓存；变化后失效，取消/错误显示未知而非0。不能用简单--count --all替代筛选总数，尤其hash消息联合、路径历史简化、shallow与多ref去重；耗时路径可等待收集完成再给总数，避免重复遍历。还需原生对应入口验证和滚动手测；本记录是评估，不宣称已实现或验收。


### 非法正则原生差异已确认

本轮最初SendKeys输入受输入法影响成为中文成对括号，之前regex-*-invalid截图不能证明ASCII非法正则。改用剪贴板粘贴明确ASCII单字符[，保留并恢复原剪贴板，两个窗口均确认正则选项开启：Rebased34276返回No commits matching filters，不报仓库错误；Plico26156报“无法加载仓库数据”，清除了顶部分支展示。有效证据仅artifacts/audit-20261023/regex-{ref,plico}-ascii-active.png。一次前台验证失败拒绝发送按键，之后通过自有窗口命中校验恢复焦点；不把失败动作当有效验收。

源码依据仍为VcsLogFilters.kt::fromPattern在PatternSyntaxException回退字面字符串，本地LogFilter::args直接传Git ERE；已确认行为差异，尚未修复。需要按Git所用语法和上游规则核对有效正则、反斜线、作者组合等，避免用不兼容regex库使合法表达式静默退化。用户新提出的历史加载与计数评估已另行记录，未将这次正则验证冒称修复完成。


### 正则修复验收边界：非法语法与 Git 不支持必须分开

新增 tests/repository.rs::invalid_regex_falls_back_to_literal_across_history_queries，真实独立仓库含两条带[提交，调用snapshot_filtered_at首批、history::read_filtered分页及locate_filtered（后两项目前未到达），要求字面匹配/真实parents与隐藏提交定位仍正确。命令 cargo test --locked --no-default-features --test repository invalid_regex_falls_back_to_literal_across_history_queries 实际失败于首个生产查询，fatal Invalid regular expression，耗时0.19秒，/tmp/plico-invalid-regex-red.log。产品尚未修改，新增回归暂为失败状态，不能报告全量绿色；旧message_filter与filtered_navigation测试将[报错作为正确行为，修复时需同时更新。

重新读取4aec352 TextFilterModel.kt::createTextFilter、VcsLogFilters.kt::fromPattern 与plugins/git4idea/backend/src/log/GitLogProvider.kt文本/作者参数段（468起、573起）：只有Java语法检查失败才转换isRegex=false；有效Java表达式仍交给Git ERE。Windows Rebased34276实际输入ASCII (?=yours)，正则开启，出现Invalid preceding regular expression并提供Reset filters；截图artifacts/audit-20261023/regex-ref-lookahead-result.png。这与[的字面回退不同。新增git_unsupported_regex_is_not_silently_retried_as_literal保护该区别，不能一概吞掉Git错误。Git零结果语法探针log --all --no-walk --max-count=0证实[、(、尾反斜线以及(?=foo)都会返回128，因此单靠Git退出码无法区分上游两类行为。未引入Java运行时或新正则库，也未采用不兼容语法解析器草率修复；共享查询参数层的规范化方案仍待定，核心缺陷未修复。用户历史加载评估与完整性改进范围保持不变。

### Log 正则语法回退（2026-10-23）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，安装版 Rebased 1.1.20 / IC-262.10968.SNAPSHOT。
已读 `platform/vcs-log/impl/src/com/intellij/vcs/log/visible/filters/VcsLogFilters.kt::fromPattern`、
`ui/filter/TextFilterModel.kt::createTextFilter`、`util/VcsLogUtil.java::isRegexp`（后两路径相对于同一 vcs/log 目录），
以及 `plugins/git4idea/backend/src/log/GitLogProvider.kt` 的文本/作者参数构造。
规则：无元字符时用字面模式；Java Pattern 语法异常回退字面；有效表达式仍由 Git ERE 匹配，
作者与消息共享正则和大小写选项。不是所有 Git 正则错误都能重试为字面。

实现前 Windows 独立 fixture 的 ASCII `[` 在 Rebased 无匹配、Plico 仓库错误；`(?=yours)` 在 Rebased
仍产生 Git 错误。截图 `artifacts/audit-20261023/regex-{ref,plico}-ascii-active.png`、
`regex-ref-lookahead-result.png`。早期 SendKeys 生成中文全角括号的图片不作为证据。

本地 `src/log_pattern.rs` 使用已有依赖树中的 regex-syntax 0.8.11 AST 解析器，只识别 Java/Rust
共有的结构错误；`src/git.rs::LogFilter::arguments` 在同一入口决定 grep/author 模式，覆盖首批、分页和定位。
无额外 Git 探测进程、不执行 Rust 正则匹配、不捕获 Git 查询错误。解析有嵌套限制，16 KiB 以上直接交 Git。
依赖默认 Unicode 表关闭，未引入 Java 运行时、Oniguruma 或 java_regex。

保留差异：这不是完整 Java Pattern 校验器。Rust 不认识的 Java 语法、未知错误和资源限制均保留正则模式，
由 Git 返回结果/错误；某些复杂非法 Java 表达式仍可能报错而不会回退。例如 `(?=a)[`、非法转义 `\j`。
不把这些表达式误判为合法，也不声称与 Java 全语法等价。只回退已确定的共享结构错误，避免把有效 Java
表达式静默改成字面含义。独立 JVM 探针对测试里的 23 个有效/无效表达式逐项核对，结果在
`artifacts/audit-20261023/regex-java-validity.txt`；JVM 仅用于验证。

实现后：Windows 自有 Plico22660，独立 commit-recovery-native-20261023，`[` 无匹配且保留分支，
`(?=base)` 保留查询错误，清空恢复列表。截图 `artifacts/audit-20261023/regex-fixed-{literal,lookahead,cleared}.png`。
测试 `tests/repository.rs::invalid_regex_falls_back_to_literal_across_history_queries`（含分页/定位/作者/大小写/取消）、
`git_unsupported_regex_is_not_silently_retried_as_literal`、`src/log_pattern.rs` 兼容/资源边界均通过。
保留UI差异：不支持的表达式仍用仓库查询错误页面，未新增上游 Reset filters 呈现；查询错误恢复首行选择未在本次改动。

## 历史加载与数量状态栏（2026-10-23）

本轮用户确定主 Log 保留手动按钮，不绑定滚动；显示当前加载量及异步总数。安装版 Rebased
1.1.20 / IC-262.10968.SNAPSHOT，自有 PID34276、独立 binary-conflict-reference-20261023：
打开 unrelated.txt → Find Action / Show History，历史工具窗显示 HEAD 筛选、紧凑图标工具栏、
提交列表及右侧按选中项加载的预览；一条记录完成后没有加载按钮/常驻百分比。截图
`artifacts/history-loading-20261023/rebased-file-history.png`。实机测试中输入焦点曾迟到，
测试文件被输入 Show History；只发生在独立 fixture，未提交，不把输入失败动作作行为依据。
主 Log 沿用此前原生截图，无对应的“当前/总数”页脚；该页脚是用户指定的保留差异。
未自动滚动，大历史加载进行中/取消尚待后续原生验证，不宣称已测。

已读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`：
- `platform/vcs-log/impl/src/com/intellij/vcs/log/history/FileHistoryFilterer.kt`：30/ALL阶段；同路径/hash/filters复用任务，改变条件取消旧任务。
- `platform/vcs-log/impl/src/com/intellij/vcs/log/util/RevisionCollectorTask.kt`：快速首批、后台收集、100ms阶段快照、取消与错误传播；不设置1000条终点。
- `plugins/git4idea/backend/src/history/GitFileHistory.kt`：按路径遍历，新增边界逐父查rename再继续，按commit去重，流式consumer；不能以单条--follow替换本地已有语义。
- `platform/vcs-log/impl/src/com/intellij/vcs/log/data/VcsLogStatusBarProgress.kt`、`platform/vcs-impl/src/com/intellij/vcs/ui/ProgressStripe.java`：延迟显示、可取消、低干扰加载状态。

拟映射：Log计数独立于详情/导航任务，固定列表roots及完整筛选，成功首屏后启动；新条件取消旧计数，失败显示未知并允许重试。
文件历史解除1000条交互硬截止，保持rename/合并规则；后台分批扩展，保留当前阅读和已加载结果，关闭/切分支取消。
页脚使用现有JetBrains字体、细分隔线、弱化数量文字、小型spinner及链接按钮；不伪造精确百分比。
保留现有元数据、图和内容安全预算，预算错误明确呈现，不能把截断冒充加载完成。

### 实现与验收

本地映射：`src/git.rs::count_history`/`src/history.rs::count` 复用 LogFilter 参数及固定 roots；
`src/ui/log_count.rs` 独立任务、150ms 首屏让步、取消及代次检查、计数失败重试。
过滤结果按相同条件先取前 N 条，再追加；总数不是整个仓库总数。Hash ID/消息并集沿用已有
Rebased 优先级，不擅自改变组合筛选含义。分页复用计数，普通刷新重算（相同根也可能发生浅克隆补全）。
`src/ui/log_history.rs` 页脚始终显示当前数量/总数，超过内存窗口时另显示范围。
移除左下角重复“历史尚未完整加载”；浅克隆独立显示“总数仅包含已有历史”，错误提示保留。

`src/file_history.rs::Scope` 固定历史根，单次 Git 元数据响应分页，沿用逐父 rename/merge 规则；
`src/history_view.rs` 首批100、后台倍增、暂停/继续、取消过期任务，发布时保持当前选中及阅读位置，
失败保留已加载前缀。预览仍按选中项加载。与上游流式 collector 的差异：本地按倍增批次重算有界前缀，
没有引入常驻全仓库索引；保留16000条元数据、16MiB文本、100000条祖先拓扑等既有安全预算。
超预算是明确错误而非“全部加载”。选中行历史仍手动扩展，避免自动加载全部版本文本。

Windows 独立1505提交 fixture，自有 Plico41512验证100→1100/1505；最终构建33380验证100/1505、
过滤 revision 14 后111/111且加载按钮消失、重复左下提示消失。文件历史先100及预览，随后1505/1505，
选中 revision 1504 和预览不变。截图在 `artifacts/history-loading-20261023/`：
`history-plico-more.png`（早期构建尚有旧左下提示）、`history-final-count.png`、`history-filtered.png`、
`history-file-complete.png`（早期构建加载中）、`history-final-file-complete.png`（最终构建）。
原生暂停点击时任务已完成，因此该动作不算暂停验收；暂停/继续及旧任务取消由桌面回归覆盖。
未进行滚动自动化，阅读位置的滚动体验留手测。未宣称像素完全一致：Plico文件历史仍为独立Tab，
Rebased为底部工具窗；用户指定的数量页脚和手动主Log加载也是保留差异。

回归覆盖固定roots下新HEAD、1105条跨rename历史、过滤/作者/日期/路径/正则/哈希计数一致性、
首屏先于计数、详情不取消计数、切过滤拒绝旧总数。核心测试与桌面测试、F7专项通过；
Windows release交叉构建通过。Clippy完成但仓库既有警告仍在，未把它报告成零警告。

## 使用流程闭环增量（2026-10-23）

用户要求逐项实现并分别提交，范围保持六项：文件历史Get、主动Cherry-pick、Reflog找回提交、
创建/推送标签、初始化仓库、子模块打开及父仓库指针提交。各项分别进行源码/原生前置验证、
实现、核心/桌面与Windows验收和提交；未完成项不因其他项完成而缩减。本轮起点ba4afbc，工作区干净。

### 1. 文件历史 Get：实现前依据

源码4aec35256d765c217aa0d662b82ac4e860c396fd；原生Rebased1.1.20 / IC-262.10968.SNAPSHOT。
独立binary-conflict-reference-20261023、自有34276，在unrelated.txt文件历史单选base并右键：
Get位于Cherry-Pick分隔线下、Show File at Revision上方，没有额外版本选择弹窗。
本地文件有已暂存与未暂存变化，点击Get出现Get Revision警告：文件已在本地修改，
主按钮Overwrite Modified File，次按钮Cancel；覆盖后编辑器立即显示base，Git仍为MM，
没有替换原先已暂存内容或切换HEAD。截图`artifacts/workflow-closure-20261023/get-version-ref-{popup,confirm,done}.png`。
测试只覆盖自建fixture，原有用户Rebased进程未修改。clean、missing、rename、删除记录禁用尚待原生补测。

已读`platform/vcs-log/impl/src/com/intellij/vcs/log/ui/actions/history/GetVersionFromHistoryAction.java`：
文件非目录、非空revision才可用；使用历史revision内容，写到FILE_PATH的当前路径。
已读`platform/vcs-impl/src/com/intellij/openapi/vcs/history/actions/GetVersionAction.java`：
先确认修改、检查只读、后台取内容，写入或创建文件、重新加载编辑器；历史Get的provider不传oldFilePath，
不把普通文件Get实现成Git checkout更改index。上游LocalHistory记录是其平台能力，本地需明确与Git恢复区分。
已读`platform/vcs-api/src/com/intellij/openapi/vcs/ui/ReplaceFileConfirmationDialog.java`：
仅存在且非NOT_CHANGED的文件要求确认；不存在/干净文件不弹覆盖确认。
拟本地映射：独立Get核心预取不可变revision、目标磁盘基线和状态，执行前复验；复用安全文件保存，
历史菜单事件交由主窗口操作/文档协调，完成触发变更与打开文档刷新。正在实现，尚未提交。

### 1. 文件历史 Get：实现与验收（2026-10-24）

本地 `get_revision::Prepared` 读取历史路径的原始 blob，写回当前文件路径；复验 HEAD、
目标 index 条目及磁盘基线，保留暂存区。`file_save::create` 使用完整临时文件及不覆盖目标的
hard-link 发布，缺失父目录逐级验证；`Project::resolve_missing` 拒绝链接祖先。
`history_view::GetRevision` 经 FileTab 转发至 `ui/get_revision.rs`，复用文档保存屏障、
操作互斥与 worktree 同步。菜单仅单选非删除记录可用，选中行历史不提供文件 Get。
确认框包含覆盖／取消，已有本地修改需要确认，缺失文件直接恢复。
图标来自上游 `platform/icons/src/expui/general/download_dark.svg`，来源记录在 assets/icons/SOURCE.txt。

Windows 自有 Plico43424 验证修改文件的取消、覆盖及打开文档刷新；最终构建25716验证菜单图标、
打开编辑器后外部删除文件，再从历史 Get 重建，界面仍可操作，打开文档显示恢复内容。
通过原始字节和 SHA256 比较确认目标为 base\n，index、HEAD 引用及无关 binary.bin 不变。
截图 `artifacts/workflow-closure-20261023/get-plico-{menu,confirm,restored}.png` 与
`get-final-{menu,missing}.png`。原生 Rebased 的 clean/missing/rename/删除记录禁用仍未逐项补测，
对应规则来自已读源码；不将核心测试当作这些原生参考场景的验收。

核心六项回归覆盖二进制/CRLF、重命名当前路径、缺失文件与父目录、并发创建/修改、
HEAD/index 改变、取消、ignored 文件确认及链接祖先；桌面新增跨仓库拒绝与真实恢复事件回归通过。
核心全套、桌面79通过/2显式忽略（新增Get专项另1通过）、额外F7七项通过；
Windows release 构建、格式检查、git diff --check 通过。Clippy有既有警告，未声明零警告；CI未运行。

保留差异：历史仍是独立Tab；未提供JetBrains平台LocalHistory撤销服务。
当前恢复复用编辑器5MiB大小预算，超限明确失败；不宣称大文件恢复完成对齐。
新建文件需文件系统支持hard-link，失败不覆盖目标；失败时可能保留空父目录。
这些限制不更改Git index或refs；大文件流式恢复及非hard-link文件系统适配仍需后续完善。

### 2. 主动 Cherry-pick：实现前依据（2026-10-24，进行中）

参考源码仍为4aec352，原生1.1.20 / IC-262.10968.SNAPSHOT。独立
`cherry-pick-reference-20261024`仓库：main为base，feature包含pick one、pick two。
使用自有34276的新窗口28447474（未关闭其他窗口），取消Defender排除勾选，仅信任本测试项目。
Log右键pick one：Cherry-Pick在Copy Revision Number/Create Patch之后、Checkout之前，
带樱桃图标；点击直接开始，没有额外确认，底部显示Cherry-Picking及进度。
Git验证main新增126c494（pick one），one.txt出现，来源b6da95c保持，提交消息无来源后缀。
截图 `artifacts/workflow-closure-20261024/cherry-{menu,success,complete}.png`；
success为执行中快照，不把它当作已刷新完成。多选、冲突、本地阻挡、空提交原生验证仍待补充。

已读源码：`platform/dvcs-impl/src/com/intellij/dvcs/cherrypick/VcsCherryPickAction.java`
保存文档、阻止重复任务；`VcsCherryPickManager.java`反转Log选择顺序后后台执行。
`plugins/git4idea/backend/src/cherrypick/GitCherryPicker.java`委派process；
`CherryPickProcess.kt`自动提交、按registry选择整批Git sequencer或逐条执行、已发布提交按设置追加-x、
记录成功与已应用集合；`applyChanges/GitApplyChangesProcess.kt`区分冲突、untracked阻挡、
local修改阻挡（显式save/retry且排除已处理提交）、空提交和其他失败，最后刷新仓库。
`EmptyCherryPickResolutionStrategy.kt`支持Skip/Create Empty，创建前复验暂存区为空。
已读测试 `GitCherryPickTest.kt` 和 `GitCherryPickAutoCommitTest.kt`前部覆盖普通成功、
受保护发布提交后缀、脏工作区、untracked、冲突对话框与多提交；后部测试仍需继续阅读。
本地将复用已实现的sequencer_continue/abort/empty与三方合并，主动入口与批次执行尚未实现。

实现进度：`src/cherry_pick.rs`已加入不可变目标预检、反转Log顺序、HEAD/分支/仓库复验、
受保护remote refs来源后缀、取消与结果分类；`ui/sequencer.rs`统一启动与继续/中止的文档屏障、
editor桥接及结束刷新，Log菜单接入主动入口，冲突进入已有三方合并，空提交进入已有选择框。
补读上游AutoCommitTest全部余下用例，以及`commands/Git.java`默认cherryPick方法和
`GitProtectedBranches.kt`发布判定。上游registry默认git.cherry.pick.use.git.sequencer=false，
本地采用其已提供的true分支：由Git持久化剩余队列，避免重启丢失待应用提交；这是明确的默认差异。
目前四项核心集成回归通过，包括中途冲突继续处理后续提交与整批abort回到起点。
尚未完成：本地变更阻挡的save/retry界面、菜单图标与最终Windows验收；当前未提交此项。

补充原生阻挡与重试：同一fixture先仅stage base.txt，pick two失败通知只有Show Files；
再修改base.txt形成MM，重试通知出现Shelve Changes and Retry。点击后成功生成pick two，
本地base.txt恢复为未暂存修改（上游Shelf不保留原index分层）。截图cherry-local.png、
cherry-local2.png、cherry-retry-result.png。已读GitApplyChangesLocalChangesDetectedNotification.kt：
仅localChanges非空且retryAction存在才显示保存重试；staging模式选择Stash，否则使用保存策略。
本地计划沿用现有Stash策略并保留index，明确区别于本次原生Shelf表现。
按用户要求已关闭过期测试实例及额外窗口，仅保留Plico25716与Rebased34276的Cherry-pick窗口；
后续换构建先关闭旧Plico，不累积测试进程。


### 2. 主动 Cherry-pick：实现与验收（2026-10-24）

已完成 Log 单选/多选入口、上游樱桃图标、后台执行及进度、文档保存屏障、结果刷新，
复用冲突解决、Continue/Abort 和空提交 Skip/Keep。`src/cherry_pick.rs` 持有预检快照，
执行前复验仓库、HEAD、分支和操作状态；整批按 Log 选择逆序执行。
本地修改阻挡后显式提供查看文件、保存并重试和取消；保存用 update_session 统一操作锁，
成功恢复原 index/工作区分层，失败保留 Git stash 并打开 Stash 列表。
仅当原 HEAD 未改变且未留下操作队列时允许重试，避免重复应用已成功的前缀。
合并提交因尚无 mainline 选择 UI 禁用，核心同样拒绝，不静默跳过。

Windows Plico40584 在独立 cherry-pick-plico-20261024 仓库验证本地 MM 阻挡、保存重试、
成功产生新提交、Log 刷新；核对 one.txt 内容、base.txt 磁盘 unstaged 与 index staged 各自保留，
stash 列表为空。截图 cherry-plico-{menu,retry,done}.png。最终构建44800验证三按钮弹窗与
查看文件打开左侧本地更改列表，截图 cherry-final-dialog.png、cherry-show-files.png。
旧 Plico 已关闭，目前仅44800及Rebased34276各一个测试实例。

核心全套通过；Cherry-pick 七项集成回归覆盖顺序、冲突后继续/整批中止、过期HEAD、
暂存保护、已发布后缀、空提交、保存重试恢复分层、失败保留stash、untracked阻挡和取消。
桌面81通过/2显式忽略，额外F7七项通过；Windows release构建、格式检查通过。
Clippy执行完成，有既有警告；CI未运行。多选、冲突和空提交未完成逐项原生对照，
不以集成测试代替这些原生视觉验收。

保留差异：使用上游Git sequencer可选模式（原生默认关闭），使剩余队列由Git持久化；
保存策略采用Stash并保留index，而此次Rebased默认Shelf不保留分层。
阻挡采用现有模态弹窗而非通知，查看文件打开完整本地更改列表而非仅阻挡文件弹窗；
完成状态沿用通用操作完成文案。中断后保留stash并显式恢复，不在之后Continue时自动pop。
来源后缀沿用应用现有受保护main/master策略，尚无单独偏好设置。不宣称像素级还原完成。


### 3. Reflog 找回提交：参考边界（2026-10-24，进行中）

同一参考4aec352/原生1.1.20。全树文件名与git4idea、vcs-log源码检索未找到独立Reflog浏览器；
原生Actions搜索reflog（包含disabled actions）仅匹配Refresh，截图reflog-reference-search.png。
这不是证明任意插件都无此能力；只说明当前参考版本没有找到可直接翻译的入口。
本项Reflog列表应明确为Plico扩展，不伪称复刻上游；恢复写操作复用现有创建分支流程。

已读 GitCreateNewBranchFromCommitAction.kt：仅单选提交，从选中hash创建分支，
可由单一现存分支建议初始名；GitBranchActionsUtil.kt::createOrCheckoutNewBranch 打开
GitNewBranchDialog，允许选择checkout和显式覆盖，再交GitBranchCheckoutOperation。
已读GitNewBranchDialog.kt前170行，默认checkout=true/reset=false，名称输入清理并调整光标；
GitCreateBranchOperation.kt执行branchCreate后更新仓库并通知，失败保留错误。
原生Ctrl+Alt+N显示HEAD的新分支弹窗（预填main），不是选中提交专属入口，
截图reflog-reference-branch.png；后续需从Log右键验证选中hash的新分支及取消，不能混淆两者。
计划：有界异步读取Reflog，保留不可变OID供恢复，不能以易变HEAD@{n}作为延迟写入目标；
从条目打开既有分支创建对话框，执行时复验目标对象与仓库状态。当前尚未实现/提交此项。


实现进度：src/reflog.rs读取所有可用ref日志的有界前缀，保留同commit的不同事件，
以NUL分隔字段，明确指定Unix时间避免用户日期配置干扰。每批200、上限10000，
额外读取一条判断是否还有记录；加载更多重读前缀避免并发写入移动ordinal导致漏项。
恢复始终传固定完整OID到branch_create，不使用HEAD@{n}作为写入目标。
三项真实Git测试通过：reset后找回、读取后新增提交再恢复旧OID、保持HEAD/index/磁盘，
分页与重复事件、日期配置、取消、空仓库和非法上限。

src/ui/reflog.rs提供Git菜单入口、异步虚拟列表、刷新、加载更多、新建分支；
关闭取消读取，迟到结果检查token，分支动作复验当前仓库；复用既有分支对话框与写入检查。
src/ui/branch_management.rs允许来自Reflog且未在当前Log页中的完整OID，核心仍复验对象。
中英文文案已接入。桌面编译检查通过；完整桌面/核心回归与Windows构建正在执行，
原生Plico列表及恢复尚待验收，当前不提交、不声明完成。


### 3. Reflog 找回提交：本轮验收（2026-10-24）

Windows原生33344验证Git菜单、中文列表、刷新、选中与新建分支；独立fixture通过
commit-tree生成9460da1d，临时移动HEAD再移回以形成失去分支引用的提交，index/worktree不变。
列表选中旧OID后打开既有分支对话框，取消checkout勾选，创建recovered-reflog-audit；
Git验证新分支精确指向9460da1d，HEAD仍80ff0ec4，原始index SHA256与base.txt内容均保持。
Log自动刷新并显示恢复提交。再次打开列表按Escape关闭，界面回到Log。
截图artifacts/workflow-closure-20261024/reflog-plico-{menu,list,lost,branch,restored,closed}.png。

核心全套通过；桌面81通过/2显式忽略，额外F7七项通过；新增Reflog真实Git三项通过。
Windows release构建通过。Clippy执行完成，修正本模块constant chunk建议，其余既有警告保留；
该修改后重新执行Reflog三项通过。格式与diff检查通过；CI未运行。
保留边界：列表为Plico扩展，并无已找到的Rebased直接参考界面；只读取仍保留的reflog，
不扫描已被GC清理的对象，也不伪称能恢复已过期日志。最多10000条、8MiB执行器输出预算。
当前展示commit/ref/action/time，未提供独立过滤和差异预览，创建分支后可在Log审阅。
Rebased选中commit专属新建分支实机对照尚未补齐，既有HEAD新建分支已核对；
本轮恢复核心路径和Plico原生通过，不宣称像素级完全对齐。


### 4. 标签创建与推送：前置依据（2026-10-24，进行中）

参考4aec352、原生Rebased1.1.20。独立cherry-pick-reference-20261024，Log选中feature的
83d7d4a8，右键New Tag在New Branch之下；弹窗标题包含目标hash，只有名称输入、OK/Cancel，
空名时OK禁用。输入audit-v1确认，Git验证refs/tags/audit-v1精确指向83d7d4a8，HEAD未切换。
截图artifacts/workflow-closure-20261024/tag-reference-{menu,dialog,created}.png。

已读GitCreateTagAction.java：单提交动作，名称非空无空白；GitBranchWorker.createNewTag
调用GitImpl.createNewTag（git tag name reference，无-f/消息）后刷新tags，失败通知。
另读GitTagDialog.java主要构建、runAction/validateFields：通用Git Tag界面含root/current branch、
name/force/commit+validate/message；消息非空才-a -F，重名才启用force，空名禁用。
这两个入口不能混为同一个弹窗。本轮先实现Log同款轻量标签入口，再补通用标签和单标签推送。
GitPushTagAction.preparePushSpec使用createTag来源和同名完整refs/tags目标，false force；
GitPushTagsActionGroup按repo/remote生成动作，6项起折叠菜单。GitPushOptionsPanel保留
随分支推送All/Current Branch选项（本地已有），不能以此替代单标签推送。
尚未实机验证通用弹窗、重名与单标签推送；这些部分保持待验收。


Log轻量标签核心已实现于tag_create.rs：固定commit OID、名称check-ref-format、同名拒绝，
执行持有仓库操作锁并复验；不使用-f，Git拒绝其他写者在预检后创建的同名tag。
两项真实Git测试通过，覆盖旧commit目标、HEAD/index/工作区不变、重复与并发重名、非法名和取消。
src/ui/tag.rs接入单名称弹窗，Log New Branch之后New Tag，后台执行及刷新；关闭取消token，
关闭后的结果不关闭后续弹窗。中英文与operation Kind::Tag接入，桌面编译检查通过。
正在运行桌面回归及Windows构建，原生Plico尚未验收，不提交。

补读GitPushOperationSingleRepoTest.kt的test push tags/single tag/existing tag：单tag首次NEW_BRANCH，
再次UP_TO_DATE，远端不同同名tag为REJECTED_OTHER，不自动覆盖。GitPushOperation.doPush/createPushSpec
对非branch来源保持完整目标ref，未启用force。原生Log Tag audit-v1子菜单当前只显示Merge/Delete，
截图tag-reference-push-menu.png；与源码有差异，尚需检查顶部branch popup入口。
测试remote仅新建本地bare tag-remote-reference-20261024.git并配置tag-audit，无外部远端写入。

单标签推送原生入口已确认：顶部分支弹窗→Tags→audit-v1→Push to tag-audit；
不弹确认框，执行后bare remote只包含refs/tags/audit-v1=83d7d4a8，没有分支。
截图tag-reference-{branches,branches-expanded,branch-actions,pushed}.png（同上artifacts目录）。
此前Log标签菜单的观察不代表顶部入口缺失。再次核对GitPushTagsActionGroup.kt与
GitPushTagsAction.kt：每个remote一个动作，达到6个时折叠，后台任务允许取消。

本地tag_push.rs保存原始tag对象OID（附注标签不剥离），执行复验tag与remote配置，
固定OID推送到同名refs/tags；显式禁用follow-tags/mirror，不强推、不设置branch upstream。
固定OID是本地防并发移动的保留差异。复用push结果解析及既有认证重试规则。
两项真实Git测试覆盖附注对象、只推单tag、再次up-to-date、同名拒绝、准备后tag/remote变化与取消。
src/ui/tag.rs新增顶部tag子菜单按remote推送，6个起折叠菜单；remote列表随弹窗后台读取，
取消/仓库切换沿用弹窗generation，执行复用操作归属、认证弹窗与结束刷新。
桌面编译检查通过。Plico新增推送入口与通用Git Tag弹窗仍待原生验收/实现，当前不提交。

通用弹窗前置实测已完成：Actions搜索Tag选择带Git分组的New Tag，显示Git Root、
Current Branch、Tag Name、Force、Commit+Validate、Message，初始焦点名称，Create Tag禁用。
输入已有audit-v1启用Force、底部显示同名错误且Create Tag仍禁用；未选Force，未覆盖该标签。
改为audit-annotated，Commit填写HEAD~1、Message填写Annotation audit后创建，Git验证
对象类型tag、剥离目标126c4944fbcc02d420b0a5ebe5963cba89b2790a，消息完全一致。
截图tag-general-{results,dialog,duplicate,filled}.png。Validate点击后的详情和Force覆盖实机未验。
再次读取GitTagDialog.java的runAction/validateFields/fetchTags：空白消息走轻量标签，非空消息
使用UTF8临时文件-a -F，Force仅同名且勾选时-f；Commit trim后空则不传目标，使用HEAD。
切换名称至非同名会取消Force。当地实现应保持这些输入规则与控件组织。

单标签入口接线后，tag_create/tag_push共4项测试再次通过，Windows release构建通过；
通用弹窗实现和本轮Plico原生验收未完成，尚不提交。

通用标签已接入src/ui/tag_general.rs与Git主菜单：名称、强制、提交及验证、消息；
名称变为非重名时重置Force，关闭取消验证/执行，晚到结果不关闭其他弹窗。
tag_create::read_options支持默认HEAD、提交表达式、附注及显式覆盖，执行前复验目标和旧tag OID。
消息用-a -F -经UTF8 stdin传入，避免临时消息文件；这是与上游-F临时文件的等价传输差异。
覆盖沿用git tag -f，不声称能锁住其他Git进程在预检后的写入。
新增真实Git测试覆盖UTF8多段消息、空白消息轻量tag、覆盖、tag/HEAD过期、非法revision；
目前tag_create3项+tag_push2项通过，核心全套与桌面81通过/2显式忽略，Windows构建通过。

原生Plico26588（替换并关闭旧33344，当前仅一个Plico）验证Git→新建标签、中文字段、空名禁用、
填写plico-annotated/HEAD~1/Annotation audit后Create Tag；Git确认对象类型tag、剥离OID
3bbe6ac5046ca7de1cb24f4de0b1b498c9cc0e05及消息一致。截图tag-plico-{menu-open,general,filled,created}.png。
原生发现输入框外层focus ring与参考不同，已改external_focus_ring(false)，此细化尚待新构建复验。

补读GitReferenceValidator.java并实际点击Rebased Validate：成功后打开Paths affected in commit弹窗，
上方文件列表、下方提交消息、右下Close（tag-validate-details.png）；关闭后回到Tag对话框。
本地目前仅解析提交并显示OID，该展示仍需补齐，不以验证命令成功当作界面已对齐。
未完成本轮单标签Plico原生push、Log新tag原生、Force原生及Validate文件详情，当前不提交。

### 4. 标签创建与单标签推送：本轮交付（2026-10-24）

补读GitUtil.java::showSubmittedFiles、ChangeListViewerDialog.java与GitReferenceValidator.java。
验证后文件浏览在src/ui/tag_details.rs实现：目录压缩折叠、展开/折叠全部、文件颜色、提交消息、
Close，双击或Show Diff按需读取只读版本，复用现有Diff导航。tag_create::preview只读取元数据，
新增真实Git测试覆盖历史rename两侧路径、root commit、工作区文件已删除仍可读取历史与取消。
标签相关测试总计6项通过。父标签草稿关闭或编辑source时取消旧验证；启动写操作也取消验证。

Windows Plico26588验证顶部Tags→plico-annotated→推送到tag-audit，独立bare remote只有
refs/tags/plico-annotated=a5d3c33975f959bb491777a3a0406d9c090181d7，保留附注tag对象，无分支。
最终构建43968替换并关闭26588，确认单层输入焦点框、重名时Force可选且Create禁用；
Validate HEAD显示one.txt和pick one消息，Show Diff正确显示只读新增one，连续Escape返回原草稿。
Force勾选后Create可用，消息留空覆盖为轻量标签80ff0ec43676fd5202c21766a6feb37df9e0bd42。
Log选中base→新建标签→plico-log-tag→Enter，Git确认轻量tag指向3bbe6ac5046ca7de1cb24f4de0b1b498c9cc0e05。
截图tag-plico-{branches,tags,push-menu,pushed,duplicate,validated-ready,details-diff,force,log-menu,log-dialog,log-created}.png。
这些只写独立fixture与bare remote；当前只有Plico43968和Rebased34276各一个实例。

完整核心回归通过（新增preview后再跑6项定向测试）；最终桌面81通过/2显式忽略，F7七项通过；
Windows release与Clippy通过（既有警告保留，无新增tag警告）；CI未运行。
保留差异：单仓库Git Root显示只读路径；错误保留在当前弹窗而非上游通知；验证详情使用模态栈，
未复制上游禁用的rollback/history按钮、目录设置菜单或独立窗口偏好。展开折叠目前为文字按钮，
无额外全体根节点，布局结构已验收但不声明像素级还原。上游Force实机执行、6个remote折叠菜单、
网络认证失败与取消未逐一做新增原生截图；核心测试和既有认证机制覆盖不能代替这些视觉状态。
签名标签沿用用户Git配置，没有新增GPG身份配置界面。后续功能不以增加签名配置为前置条件。

### 5. 初始化到首次推送：初始化前置依据（2026-10-24，进行中）

Rebased 1.1.20，独立空目录 init-reference-20261024。Actions 搜索 Create Git Repository，
打开 Windows 单目录选择器，允许选择已有非空目录；选择新建空目录后后台创建 .git，
Windows Git status --porcelain=v2 --branch 确认 branch.oid (initial)、branch.head main。
再次选择同目录弹 Git Init 警告，说明已受 Git 管理并询问创建新 VCS root，Yes/No；
本轮选择 No 取消。截图 artifacts/workflow-closure-20261024/init-reference-{action,picker,current,existing}.png。
选择器第一次 Enter 导航到目录，第二次确认，不能将导航误判为初始化失败。

已完整阅读参考 4aec352 的 plugins/git4idea/backend/src/actions/GitInit.java，
以及 commands/GitImpl.java::init：单目录选择、已有 Git 管理时确认、后台 git init 无附加参数，
失败通知；成功刷新并增加当前项目 VCS mapping，默认项目不增加 mapping。
因此本地保留 Git 的 init.defaultBranch/template/object-format 默认，不硬编码 main，
不要求目录为空，不自动提交或添加文件。Plico 单仓库窗口没有多 root mapping，成功后
沿用现有打开仓库/选择窗口机制作为保留差异。初始化入口拟放欢迎页与 Git 菜单。
初次提交、配置远端与首次推送还需串联验收；当前未完成第 5 项，不提交。

初始化核心 src/init_repository.rs 与欢迎页/Git 菜单入口 src/ui/init.rs 已接入。
补读 GitUtil::isUnderGit/findGitRootFor：上游向父级查找 Git root。当地采用 .git 标记向上检查，
包括 worktree 的 gitfile；对损坏/悬空标记也要求确认，较上游更保守，避免静默覆盖。
执行前复查标记路径；它不是跨进程文件系统事务，不声称阻止复查后的外部替换。
未新增 init.defaultBranch 或模板覆盖，保留 git init 默认。取消不删除目录或部分 .git。

3 项真实 Git 测试通过：非空目录/默认分支不变/不暂存，已有与嵌套确认/准备后状态变化/取消，
以及初始化→产品 commit_operation 首次提交→remote_management 配置→push 首次推送与上游设置。
完整核心测试通过；桌面 81 通过/2 显式忽略，F7 7 项通过，Clippy 通过（既有警告），Windows 构建通过，CI 未运行。
Windows 关闭旧 Plico43968 后启动43712，仅一个Plico；Rebased34276保持一个。
原生 Plico 欢迎页创建 init-plico-20261024，已有README保持未跟踪，空Log显示0/0；
勾选README，Initial commit→提交并推送，首提交成功且Log自动刷新1/1。
截图 init-plico-{welcome,created,changes,commit-ready,first-push,remote}.png。
发现无remote时Push的Select remote进入空编辑器，没有Define Remote入口；这不是初始化失败，
但首次推送入口不闭环。下一步先实测并读取Rebased无remote的Push流程，再补齐入口后提交第5项。
Plico已有目录确认框、取消/失败原生状态尚未逐一验收。

第5项首次Push缺口前置：原生Rebased34276，移除仅用于tag验收的tag-audit remote，
Ctrl+Shift+K显示 main→Define remote 链接（空仓库第二root显示Empty repository并禁用），
点击打开Define Remote，Name默认origin，URL默认焦点，Fetch remote默认勾选；
填写独立bare init-reference-remote-20261024.git，确认后返回Push并刷新目标。
截图init-reference-{push-empty,define,target-ready}.png。
已读4aec352 GitPushTargetPanel.java构造、updateComponents、showDefineRemoteDialog、
addRemoteUnderModal、showRemoteSelector、getPopupItems及GitDefineRemoteDialog.kt完整文件。
无remote不展示branch编辑器，不伪报加载失败；已有remote菜单末尾同样包含Define remote。
添加执行后台git remote add，成功更新默认目标并触发提交重载。
源码PushTargetPanel未消费DefineRemoteDialog.shouldFetch，Plico会保留勾选框真实行为，
勾选时在后台fetch后重载目标与提交，复用既有认证，作为明确保留差异。

### 5. 初始化到首次推送：交付验收（2026-10-24）

初始化入口与核心沿用上述规则；首次Push无remote时不执行无效outgoing查询，
源分支后提供Define Remote链接，已存在remote的选择菜单也有分隔后的Define Remote。
src/ui/push_remote.rs复用DefineRemoteDialog与remote_management、认证fetch执行；
写入参加Review操作归属，关闭后的Push不再接纳迟到快照；新增配置后重新读取实际
Push snapshot并加载提交。fetch失败仍保留已成功添加的remote，不做隐式删除。
Define Remote两个输入框取消外层focus ring，避免双框。无目标时不显示误导的No outgoing commits。

原生Plico32076：无remote链接→Define Remote（origin默认、URL焦点、Fetch默认勾选），
填写独立bare init-plico-remote-20261024.git，确认后自动回到origin:main及Initial commit。
点击Push成功，bare show-ref只有refs/heads/main=f03e64ad014964d8cd2a653f934b92b6439dd28e；
本地status确认upstream origin/main、+0/-0且无工作区变更，Log自动刷新origin与main标签。
Git菜单Create Git Repository选择同目录显示已有Git确认，Escape取消，原提交与工作区保持。
最终构建2720替换并关闭32076，验证有remote菜单的Define Remote入口及单层URL焦点边框，
取消不新增remote。截图init-plico-{define-entry,define-form,target-ready,pushed,menu-open,existing-confirm,remote-menu,final-define}.png。

初始化到推送3项真实Git测试、完整核心回归通过；桌面82通过/2显式忽略，F7七项通过；
最终仅调整焦点框/空状态后定向Push测试通过，最终Windows构建通过。格式检查通过，
Clippy核心全部target通过（既有警告）；CI未运行。新增原生网络认证失败/初始化权限失败
未逐个截图，沿用既有错误/认证机制，不声称覆盖所有原生异常状态。
保留差异：单仓库窗口成功后走已有Open Project窗口选择而非Rebased增加多root mapping；
目录选择器按钮文案本地化但标题由系统提供；确认框复用应用模态而非独立系统警告，
未复制警告图标/精确间距。功能和结构验收完成，不宣称像素级还原。
本项完成，下一项子模块导航与父仓库gitlink提交。

### 6. 子模块：父仓库提交前置依据（2026-10-24，进行中）

Rebased 1.1.20，沿用独立 cherry-pick-reference-20261024，添加本地源仓库
submodule-source-20261024 为 modules/library；先提交 gitlink=f0f103d，再在子仓库
提交 ec8a8fd 并继续修改 module.txt 未提交。父仓库另有未勾选 base.txt 修改。
原生 Commit 显示 library 为目录图标的一条变更；取消 base.txt 勾选，仅提交 library，
消息 Advance library pointer。成功通知 1 file committed；Git 验证父提交0d1f312的
modules/library mode=160000、OID=ec8a8fd；子模块 module.txt 仍为未暂存修改。
截图 artifacts/workflow-closure-20261024/submodule-reference-{changes,diff,commit-ready,committed}.png。
勾选范围与提交消息同普通提交面板，不为子模块引入递归提交选项。

已读参考4aec352：GitCheckinEnvironment.kt::sortChangesByGitRoot使用变更路径的父目录
确定所属仓库；stageAndCommit通过暂存管理器保存未选择index，GitFileUtils.kt::stageForCommit
将选择路径addPathsForce，随后GitRepositoryCommitter.kt提交index。
GitCommitTest.kt的excluded submodule场景保留未选中的已暂存gitlink。
GitSubmoduleContentRevision.kt、GitSubmoduleDiffRequestProvider.kt将子模块作为父仓库文件，
内容为前后提交号；GitIndexUtil.java::loadSubmoduleHashAt验证ls-tree单条StagedSubrepo。
GitSubmodule.kt依赖已注册子仓库。当前安装版本动态添加子模块后打开diff报cannot read object，
见diff截图；尚未验证重新注册根目录后的表现，不把错误视为应复刻行为。

本地第一步映射src/commit_operation.rs：只对index里确认为160000且存在子仓库标记的
目录允许git add，普通目录继续拒绝，避免把选择扩展到整棵树；沿用未选index恢复与HEAD检查。
子模块导航入口、差异审阅及原生Plico验证尚待完成，本项不提交、不标完成。

父仓库提交核心已实现（未提交）：新增真实嵌套仓库回归，经产品execute入口提交子模块HEAD，
验证child的index、工作区以及parent未选中的暂存内容不变；另验证伪.git普通目录不递归暂存。
commit_index全39项通过（/tmp/plico-submodule-commit-tests.log），格式化完成。
尚未完成导航、gitlink差异审阅核对及Windows Plico验收，不作为第6项完整交付。

导航前置：原生Rebased File→Open选择modules/library，出现New Project窗口选择框，
This Window默认焦点，另有New Window/Cancel；确认后同窗口标题library，Log显示子仓库
Advance library和module base两条历史。截图submodule-reference-{file-menu,open-picker,open-confirm,opened}.png。
已读OpenFileAction.kt::actionPerformed/doOpenFile：目录走ProjectUtil.openExistingDir AS_PROJECT，
不是作为文本打开。Plico沿用现有choose_project_window和保存屏障；项目树目录菜单增加
Open Repository快捷入口（相对上游File/Open是明确的入口扩展），后台确认目录本身是仓库根，
普通子目录不能误开成父仓库。不开新递归扫描，不改变双击目录展开行为。

导航已接入（未提交）：Project树目录菜单末尾Open Repository，路径在后台经Project.resolve
拒绝链接/越界后检查.git及rev-parse根身份；普通子目录不能回落到父仓库。
ProjectView请求复用取消令牌/代次，Review仅接纳当前可见Project实例事件，之后复用
choose_project_window及当前窗口保存屏障。未改变双击展开与Copy Path菜单顺序。
core commit_index39项（含真实gitfile子仓库导航与普通目录拒绝）及Project菜单回归通过。
Windows构建启动于/tmp/plico-submodule-windows.log；尚未进行本版原生验收。

差异加载修正依据：本地DiffTab::activate后台把所有非删除working_tree_document送进
working_copy::read，子模块目录因NotFile失败；上游GitSubmoduleContentRevision仅提供提交号。
拟按index的160000身份识别本地gitlink，走已有只读Git --submodule=short patch展示，
保留Subproject commit和dirty提示（相对上游纯hash文本的明确差异），不创建可写文档。

本轮原生Plico4396：Project展开modules→library右键Open Repository→当前窗口Enter，
标题变为library，Log正确显示子仓库3/3提交，截图submodule-plico-{menu,open-confirm,opened}.png。
打开不修改父/子Git内容。此轮未点击新窗口，以免留下多余实例。
首次gitlink diff原生发现NotFile：分流曾误接到disk_refresh而非activate首次加载，已修正
首次加载入口并新增gitlink_initial_load_uses_readonly_diff实际异步加载回归（待运行结果）。
submodule-plico-diff.png是失败证据，不能作为最终通过截图。
核心全量、桌面82通过/2显式忽略、F7七项、Clippy通过（既有警告）；其后修正首次加载需再验收。
修正后的Windows构建/tmp/plico-submodule-windows-final.log已通过，尚未替换4396运行中的旧版。
独立fixture submodule-plico-20261024父HEAD=0d1f312，childHEAD=59cc241，child仍有未暂存修改；
父base.txt亦未暂存。本版原生父指针提交仍待验证，不提交第6项。

### 6. 子模块：交付验收（2026-10-24）

最终Windows Plico44236替换并正常关闭4396，父仓库双击library首次加载正确显示
HEAD ec8a8fd→工作区59cc241-dirty，两侧锁图标，不能编辑目录。仅勾选library，
Record child pointer from Plico→Commit，生成父提交4c66c578；ls-tree确认mode160000、
OID59cc241d51cab07bf7c8e085b40f06c95003fb2b。父base.txt仍未暂存；child module.txt仍
为child remains uncommitted after parent commit，status为未暂存，未被递归提交。
提交后diff自动刷新为相同OID与dirty标记；Commit消息清空并显示成功反馈。
截图submodule-plico-{diff-final,commit-ready,committed}.png为最终通过证据。

新增真实Git测试覆盖gitfile子仓库打开、普通目录拒绝、gitlink只读patch和dirty标记、
提交父指针保留child已暂存/未暂存及parent排除index；新增GPUI首次activate真实加载回归，
防止只在刷新路径分流。核心全量通过；最终桌面83通过/2显式忽略，F7专项8通过；
Windows最终release通过，fmt/diff检查通过，Clippy通过（既有警告，新增clone切片警告已修正）；
CI未运行。最后切片修正仅去除临时clone，不改变查询或界面。

保留差异/范围：Project目录菜单Open Repository是File/Open的快捷入口扩展；仍使用单仓库
窗口而非Rebased多root模型。gitlink保留Git Subproject commit及dirty文本，而非上游纯hash。
普通目录/未初始化submodule不能当父gitlink提交，初始化/递归更新子模块不在本项范围。
Commit列表图标沿用通用文件图标，尚未复刻Rebased目录图标；功能及结构通过，不宣称像素还原。
新窗口/关闭窗口保存提示沿用已有流程，本轮原生选择当前窗口，未逐一重测所有窗口策略。
本项实现完成；仍需全六项完成性核对，不能据此直接宣称整轮goal完成。

### 收尾：历史Get大文件恢复前置（2026-10-24）

Rebased1.1.20，独立library子仓库：restore-large.txt历史5402674为6,650,000字节，
HEAD ea08601为15字节。Project右键Show History→选Large historical file→Get，无修改确认
（目标原为clean），成功恢复6,650,000字节，SHA256=15af3261c8ec1050ccfe68d1489ad628859cb07f72868d7691c263572e13c292。
diff --cached为空。截图artifacts/workflow-closure-20261024/get-large-reference-{start,menu,history,get,restored}.png。
重新读4aec352 GetVersionAction.java::MyWriteVersionTask、writeCreation/writeModification，
provider.loadRevisionContent后写原始字节并reloadFiles，没有编辑器5MiB限制，也不依赖hard-link。
本地拟用已存在的operation_process::run_to_file、file_save::copy_stream/same_file_bytes/
save_from_file，Prepared持有磁盘快照而非完整Arc字节，保留HEAD/index/磁盘复验及取消。
Windows新建发布使用无覆盖MoveFileExW，避免对文件系统hard-link的要求。

流式实现已接入（尚未提交）：Prepared用随机私有临时目录持有baseline/revision文件，
后台Git stdout直接写磁盘；baseline流式复验，执行继续核对HEAD/index，复用save_from_file。
缺失目标用create_from_file写完整sibling再发布；Windows MoveFileExW不带REPLACE_EXISTING，
保留并发创建者，不再要求hard-link。非Windows暂沿用hard-link不覆盖发布。
7项Get真实Git测试通过，新增12MiB历史二进制+10MiB工作区baseline及缺失文件恢复，
验证index/HEAD不变，原有取消/并发修改/重命名/链接祖先回归保留。
Windows构建/tmp/plico-get-stream-windows.log正在运行，原生Plico大文件验收待完成。


### 历史 Get 流式恢复验收（2026-10-24）

Windows release 构建通过，实际运行文件与构建 SHA256 一致。Plico41664 在独立
get-large-plico-20261024 仓库，通过 History 旧提交右键 Get 恢复 6,650,000 字节文件；
再删除自有测试文件，取消先前的修改确认后重新发起 Get，缺失文件创建恢复成功。
两次核验 SHA256 均为15af3261c8ec1050ccfe68d1489ad628859cb07f72868d7691c263572e13c292，
HEAD 保持ea08601dea905d666576dadffb1c0b2b90cc897f，暂存区无差异。
截图 get-large-plico-{get,details,restored-final}.png 在上述 artifacts 目录。
原始工作区小文件实际16字节，前置记录的15字节为笔误。

核心全量测试通过，Get专项7项通过，桌面Get事件测试1项通过；Windows release、
Clippy（既有警告）、格式检查通过，CI未运行。恢复不再受编辑器5MiB限制；
预览仍保留限制。非Windows缺失文件发布仍依赖hard-link，Windows已使用无覆盖移动。
原生发现预览超限错误误用“无法加载文件历史”提示并影响总数显示，列表与Get可用；
此为后续独立修正项，不把截图中的错误状态算作完整UI通过。


### 文件历史预览失败隔离：实现前依据（2026-10-24）

原生同一6.65MB文件：Rebased1.1.20的历史列表仍显示两条，预览独立显示内容；
截图history-preview-reference.png（此前已通过旧版本Get实际交互）。Plico相同历史
预览触发5MiB限制，错误错误地出现在列表顶部，并使总数成为—。
实读4aec352的platform/vcs-log/impl/src/com/intellij/vcs/log/history/FileHistoryDiffProcessor.kt：
独立SingleFileDiffPreviewProcessor由选中change产生Diff请求；
platform/diff-impl/src/com/intellij/diff/impl/CacheDiffRequestProcessor.java::doLoadRequest
将producer异常转换为ErrorDiffRequest，不修改历史查询状态。
本地src/history_view.rs将preview_error与列表error分离，在预览区显示可重试错误，
切换选择只清除预览错误。保留本地5MiB预览限制，未声称与Rebased文件大小上限一致；
上游异常状态本轮未人为破坏Git对象触发，错误区域归属以已读源码为依据。
真实GPUI异步回归preview_failure_does_not_fail_history修复前失败，确认两条历史完整
但预览超限污染error；查询失败与迟到结果不是该最小复现的原因。


历史预览隔离验收：新增真实Git+GPUI异步回归，5MiB以上旧版本触发预览失败，
断言历史两条完整、history.error为空、preview_error保留明确原因。修复前失败、
修复后通过；既有暂停/继续历史加载测试亦通过。完整桌面84通过/2既有忽略，
F7专项8通过；Windows release及格式检查通过。Plico41952原生重新打开同一文件历史，
列表显示2/2，无顶部历史错误；底部显示本地化“无法加载差异。”及刷新/详情，
截图history-preview-fixed.png。保留预览上限与错误提示控件外观差异，不宣称像素还原。


## 本轮六项闭环交付核对（2026-10-24）

范围以本文件“使用流程闭环增量（2026-10-23）”的六项为准，未把整个PLAN的后续功能
纳入本轮，也未将本轮缩减为单独通过的路径。各功能实现提交均已存在，工作区干净后核对。

| 要求 | 实现提交与入口 | 已核对的证据 |
| --- | --- | --- |
| 文件历史Get | cbe1e0f；History右键→ui/get_revision；e76130d流式恢复 | tests/get_revision.rs的7项覆盖原始字节、重命名、缺失、并发、取消和大文件；原生覆盖/取消/删除后恢复；HEAD及index保持 |
| 主动Cherry-pick | 0c90b41；Log选中→sequencer协调 | tests/cherry_pick.rs七项覆盖顺序、冲突Continue/Abort、空提交及保存重试；cherry-final-dialog及done截图和原生MM保留记录 |
| Reflog找回 | 1639569；Git菜单→Reflog→新建分支 | tests/reflog.rs用reset丢失提交并移动ordinal后按固定OID恢复；原生recovered-reflog-audit与restored截图 |
| 创建/推送标签 | a536d8e；Log及Tags菜单 | tag_create/tag_push六项验证轻量/附注/Force/验证预览及单标签推送；原生bare仅收到指定tag，pushed及force截图 |
| 初始化到首次推送 | fc1f601；欢迎/Git菜单→初始化→Commit→Define Remote→Push | init_repository三项真实流程；再次读取原生bare的refs/heads/main=f03e64ad014964d8cd2a653f934b92b6439dd28e，pushed截图 |
| 子模块打开及父指针提交 | 597ff8a；Project目录Open Repository及Commit | commit_index真实gitfile子仓库导航、父指针与排除index保护；原生父HEAD的modules/library为160000/59cc241d51cab07bf7c8e085b40f06c95003fb2b，committed截图 |

核对时读取了测试实际断言及/tmp/plico-get-stream-core.log的对应成功结果，
未仅凭文件存在判定功能完成。最后UI修正6f6859e将预览错误与历史加载分离，
真实异步回归先失败后通过；最新全桌面84通过/2既有忽略、F7专项8通过，
Windows构建、原生2/2历史计数和独立预览错误、fmt/diff检查、Clippy通过（既有警告）。
六项均在各自章节记录了Rebased安装版本、参考commit、源码规则、原生操作与差异。

交付边界保持原记录：非像素级完全还原；Reflog是Plico扩展，最多10000条；
Cherry-pick合并提交不提供mainline选择；子模块初始化/递归更新不在本轮；单仓库窗口
不提供多root。历史Get已移除恢复大小上限，但预览仍有5MiB限制，非Windows新建发布
仍要求hard-link。JetBrains LocalHistory、Shelf和额外签名配置不是本轮新增能力。
原生异常状态未逐个截图的项目仍按各章标注，不把核心测试冒充视觉验收。
本轮要求的六项实现及独立提交已完成；CI未运行，尚未推送或发布preview。


## 协作工作流补全计划（2026-10-24）

本轮目标：按用户确认的工作流缺口逐项补全；起点fb7aa58（preview.11）。
每项按原生Rebased操作/截图→对应源码及测试→规则与本地映射→实现→回归和
Windows原生同条件验收→独立提交推进，不批量铺设未经参考的临时界面。
每项保留中英文、取消/迟到结果、文档保存屏障、工作区/index保护及刷新规则。
不自动推送或发布；功能验收和像素差异分别记录。

| 顺序 | 交付范围 | 必须验证的关键工作流 | 参考入口（未逐项深入前仅为线索） |
| --- | --- | --- | --- |
| 1 | Fork拉取/推送目标分离 | upstream/main拉取、origin/topic推送；pushRemote/pushDefault优先级、refspec、无跟踪/失效remote、手动目标；无意外写远端 | GitPushSupport、GitPushTarget、GitPushSpecParser及测试 |
| 2 | 当前分支Rebase到所选分支 | 本地/远端分支、detached禁用、dirty保存、冲突继续/中止、空提交及刷新 | GitRebaseBranchAction、GitBrancher、GitRebaser及测试 |
| 3 | 分支重命名 | 当前/其他本地分支、非法/重名、保留或取消upstream、工作树占用及刷新 | GitRenameBranchAction、GitNewBranchDialog、GitRenameBranchOperation |
| 4 | Worktree管理 | 列表、创建新/已有分支、打开、确认删除含本地修改的工作树、已占用分支、失败/取消和跨窗口刷新 | workingTrees actions/dialogs/services、GitWorktreesViewModel及测试 |
| 5 | 分支对比 | 任意两分支独有提交、整体文件差异、方向交换/空结果/按需Diff/引用刷新 | GitCompareBranchesHelper、CompareBranchesHelper、GitCompareBranchesUi |
| 6 | 子模块初始化/同步/更新 | clone未递归后补初始化；父指针变化后更新；嵌套/dirty子模块/URL同步/失败恢复 | GitSubmoduleUpdater、GitSubmoduleTest及对应UI入口 |
| 7 | 浅克隆获取完整历史 | 仅shallow仓库入口、确认remote、认证/取消/失败、完成后历史计数刷新 | GitUnshallowRepositoryAction、GitFetchSupport及测试 |
| 8 | 交互式历史整理 | 补齐此前表中提到的完整交互式Rebase：顺序调整、pick/reword/edit/squash/fixup/drop、资格限制、冲突恢复；具体控件按上游实机确认 | Git交互式Rebase action/dialog/editor/model及测试（待定位） |

第8项在前五个优先方向之后，不以已有Squash/Reword冒充完整交互式Rebase。
此前明确后排的PR/评论、Git Flow专用向导、LFS、Bisect、补丁导入导出不纳入本轮。
Rebased参考checkout为4aec35256d765c217aa0d662b82ac4e860c396fd，安装版1.1.20 /
IC-262.10968.SNAPSHOT。若其行为与Git配置或本轮用户目标不同，先记录差异依据，
不能把上游存在的限制直接当成我们的功能目标。

当前状态：第1项已提交6505a25；第2项已提交030edf1；第3项已提交e4046e0；第4项主流程已实现并通过验收（本次独立提交；来源搜索、键盘/右键、Windows权限重试等剩余对齐项见提交前记录）；第5项进行中（分支对比原生参考与有界查询）；第6—8项待实现。


### 1. Fork推送目标：原生前置结果

独立fork-reference-20261024/project，origin/upstream均为同目录本地bare，topic跟踪
upstream/main，branch.topic.pushRemote及remote.pushDefault均为origin，push.default=current。
Rebased1.1.20自有43328：Ctrl+Shift+K打开Push，实际默认topic→upstream:main，
左侧列出feature，右侧readme.txt；remote链接菜单有origin/upstream及Define remote。
截图artifacts/collaboration-workflows-20261024/fork-reference-{project,push,remote-menu}.png。
仅打开/查看，未执行Push。信任仅针对自有fixture，已取消默认Defender排除项。

实读参考4aec352的GitPushSupport.java::getDefaultTarget/getPushTargetIfExist与
GitPushTarget.java::getFromPushSpec/getRemoteToPush，当前上游也是先对应remote的push refspec，
再trackInfo，最后默认remote+同名branch；本地旧行为不是对该快照的翻译错误。
但用户要求Fork分离目标，不能因此保留忽略显式Git推送配置的限制。本地计划保留Push布局、
可编辑source→target、预览和确认流程，在目标选择层支持branch.pushRemote优先于
remote.pushDefault，同时保留pull_targets的upstream不变；未配置时沿用既有上游策略。
显式不存在的remote不得静默回落到upstream。选定remote的refspec继续优先；分离remote默认
使用当前分支名，并进一步对照push.default的适用模式与Git行为。尚未实现或宣称验收通过。


### 1. Fork推送目标：实现及原生正向验收（2026-10-24）

src/push_target.rs保持pull::Targets不变，单次有界Git config查询读取branch.*.pushRemote、
remote.pushDefault、对应remote.push及push.default，分支级优先，全局次之，最后既有默认。
选定remote的refspec仍优先；显式current使用本分支名，upstream仅同remote跟踪目标，
simple拒绝同remote异名跟踪；nothing/matching不擅自生成单分支默认目标，允许用户显式选定。
未配置push.default时保留既有Rebased默认规则；无效显式remote返回错误、不回退写upstream。
PushConfiguration来源与TrackingBranch分开，避免把推送目标当成拉取跟踪目标。

真实Git回归先红（实际upstream、预期origin）后绿，覆盖remote优先级、对应refspec、
配置末值、失效remote、取消、push.default及两个本地bare端到端推送：只新增origin/topic，
保留upstream/main、当前upstream以及index/工作区分别未暂存/已暂存内容。
19项既有及新增push筛选回归通过，新增端到端后3项fork_push通过；最新全核心重跑中。
Clippy通过（既有警告），Windows release通过，桌面push定向测试运行中；CI未运行。

原生Plico41732，独立fork-plico-20261024/project（复制前置fixture及两个bare隔离）：
点击顶部Push→显示topic→origin:topic、新分支、feature及readme.txt；点击推送成功，
Log自动出现origin/topic。实际bare核验origin/main与upstream/main仍为dea1da42，
仅origin/topic为61c9ab61；@{upstream}仍refs/remotes/upstream/main。
截图artifacts/collaboration-workflows-20261024/fork-plico-{start,push,done}.png。
保留上游Push布局，本项改变默认目标解析而非重做弹窗；不宣称像素级还原。
异常模式的原生提示与手动目标切换尚需补验，当前未提交，不将正向通过冒充全部完成。


补验记录：最新全核心通过（/tmp/plico-fork-core-final.log），原桌面push用例通过。
原生push.default=nothing显示选择远端、禁用推送，但错误地触发No push target加载错误；
截图fork-plico-no-default.png证实此问题。已将PushDialog首次加载及重载条件收紧为
实际target存在，增加已有remote但无default的GPUI回归（运行中）。该UI修正尚需新Windows
构建和原生复验。失效remote确实阻止打开Push/写操作，但共享error在当前主窗口截图中
不可见，需继续改善诊断呈现，不能把此状态算为完整UI验收通过。其余计划项目未开始。


Fork收尾复验：实读同commit的platform/dvcs-impl/src/com/intellij/dvcs/push/PushController.java，
loadCommits在target=null时stopLoading并返回，onTargetChanged清错后才load。
本地PushDialog相同地等待显式目标，不把无默认目标作为Git读取失败；空remote编辑按钮
明确显示“选择远端”。失效配置沿用本地错误详情弹窗自动呈现；这是Plico对Git显式配置
的扩展状态，上游此版本不解析pushRemote，不能声称上游有同一失效配置界面。

Windows37276已复验无默认目标不报错且推送禁用，点击选择远端→origin→输入topic→应用，
显示origin:topic及“没有待推送提交”，推送恢复可用（此前已推送，未再次执行）。
截图fork-plico-no-default-fixed、manual-menu、manual-applied-final；后者属于手动目标路径。
失效remote显示可复制/关闭的Git错误详情，明确给出missing，截图invalid-fixed。
底部瞬时错误被刷新替换的问题通过持久弹窗解决，不修改后台刷新行为。
桌面push_dialog两项通过，最终label变更Windows构建仍在收尾；核心全量已通过。


第1项最终交付：Windows最终构建通过，Plico45116原生复验选择远端按钮的中英文入口
（当前中文截图fork-plico-select-label.png），不再出现无文字按钮。全核心通过；
桌面PushDialog两项通过，Clippy通过（既有警告），fmt/diff通过，CI未运行。
本项功能及结构验收完成，不宣称像素完全一致；错误详情保留原始英文诊断，按钮/标题
沿用现有中英文。显式nothing/matching不自动选择目标，但用户仍可明确指定单分支推送；
未配置push.default保留上游旧策略，不能将此称为对无参数git push全部模式的完全模拟。
第2项开始前置：已创建rebase-branch-reference-20261024，topic/main各自一条提交，
local.txt保留未暂存修改，等待原生Rebased入口/执行验证；尚未修改第2项产品代码。


### 2. 独立分支Rebase：原生正向前置与源码（2026-10-24）

Rebased1.1.20自有44688，rebase-branch-reference-20261024：main=2d1e6fe、topic=a4cbf3f
分别从477c32b分叉，local.txt未暂存。顶部topic→main子菜单，已有Checkout and Rebase
onto topic与Rebase topic onto main两个相反方向动作；本项是后者，在Compare/Diff之后、
Merge之前。点击后没有额外正常确认，自动保存local.txt，执行后通知Rebased topic on main。
topic变成d4787c2、parent=main，Windows Git status仅local.txt及IDE创建的.idea未跟踪，
local.txt仍为unstaged work；未修改跟踪配置。截图rebase-reference-{log,branches,actions,
action-popup,success}.png。最初全窗口坐标点击未命中操作，最终使用实际子菜单窗口点击
后确认Git状态变化；只把最终操作作为成功证据。信任仅自有fixture，已取消Defender排除。

实读4aec352：GitRebaseBranchAction.kt仅非当前分支、所有repo附着分支才启用；
GitBrancherImpl.doRebase后台→GitBranchWorker.rebase→GitRebaseUtils.rebase→GitRebaseProcess。
GitRebaseParams.kt的普通构造仅指定upstream，不主动开启interactive/rebase-merges等选项。
GitRebaseProcess.saveDirtyRootsInitially先保存dirty roots；遗漏dirty只允许一次按需保存重试；
未跟踪覆盖报错；冲突进入resolver并保留进行中状态；成功加载保存修改，失败保留恢复入口。
GitSingleRepoRebaseTest.kt已读普通冲突/多次冲突、continue、skip、自动保存/漏检保存、
错误不恢复保存、重试成功恢复、abort恢复本地修改等断言。

本地映射拟新增branch_rebase核心与UI协调，复用update_session的保存/恢复及现有rebase
继续/中止协调；不能直接把本地分支伪装成pull::Target（目前Session只接受remote目标）。
执行前固定目标OID、当前branch/HEAD并复验，保留文档保存屏障、取消和迟到结果规则。
本次仅完成正常带dirty场景原生前置；冲突/中止、远端分支和detached禁用尚待原生补验，
产品代码未开始，不能据此宣称第2项已完成。


### 2. 独立Rebase冲突/中止前置补充及执行边界

同一自有Windows fixture：conflict-main=211d409、conflict-topic=babee4df，
从477c32b分别修改shared.txt，同时保留local.txt未暂存修改。
Rebased执行Rebase conflict-topic onto conflict-main后显示Conflicts，列出shared.txt，
支持Resolve Manually及两侧Accept。关闭冲突窗口不会中止操作；主窗口保留Rebasing状态，
通知提供Resolve/Continue/Abort及View Shelf，说明local changes已shelved。
Abort会再次确认；确认后回到conflict-topic的babee4df，Windows Git status恢复
local.txt修改，未跟踪.idea保留。截图位于artifacts/collaboration-workflows-20261024/
rebase-reference-{conflicts,suspended,abort-confirm,aborted}.png。
本次未验收冲突解决后Continue、远端目标或detached菜单，不能扩大实机结论。
源码补充GitAbortRebaseProcess：确认后执行abort，成功再加载保存修改。

本地第一步：update_session::Session::rebase_onto_commit接收经过调用方验证的固定OID，
拒绝detached；与Pull共用start_rebase的状态检查、editor、取消后abort及结果读取。
不Fetch、不变更upstream，不把本地目标伪装为pull::Target。原Pull入口保留remote存在检查。
后续仍需Prepared的HEAD/目标复验、持久恢复记录、菜单及Windows同条件验收；此处不是功能完成记录。

本次执行边界验证：真实临时仓库的独立Rebase定向测试通过，覆盖无remote、固定OID、分支身份、重复执行拒绝及detached拒绝。完整核心回归`cargo test --locked --no-default-features`通过（/tmp/plico-independent-rebase-core.log），fmt检查及git diff --check通过。尚未新增UI，未进行Plico原生功能验收；未提交。


### 2. 独立Rebase的保存归属与跨会话恢复

实读同一4aec352的GitRebaseProcess：先saveDirtyRootsInitially，SUCCESS或NOT_STARTED
才load；冲突暂停保留saver。GitAbortRebaseProcess在确认并成功abort后load。
本地branch_rebase::Prepared绑定worktree目录、当前分支、原HEAD及完整目标ref/OID，
执行持有现有操作锁，保存前后复验。自动保存tracked编辑，不带走未跟踪文件。
恢复存储从squash_saved更名为rebase_saved，统一普通Rebase/Squash/Checkout-and-Rebase；
保留gitui-squash-saved文件名与v1格式兼容已发布版本，避免破坏用户尚未完成的恢复。
for_rebase核对orig-head/onto/head-name，扩展到rebase-merge及rebase-apply；
若两套目录同时存在则不猜测归属。正常Rebase不创建Squash origin/Undo。
成功后恢复index与worktree；冲突保留stash及记录，现有Continue/Abort可跨会话读取。
发生状态不确定、保存记录持久化失败或目标并发变化时保留stash，报告恢复信息；
这是比上游内存saver更保守的归属保护，不能自动恢复到别的分支。

新增真实Git用例覆盖merge/apply两个后端各自的success、continue、abort：
执行后丢弃原Repository对象，重新建立对象继续/中止，确认原分支、HEAD/parent、
暂存与未暂存内容分离、untracked保留、stash和记录清理，并确认没有Squash Undo。
另测目标ref在准备后变动时，在保存前拒绝，不创建stash、不修改HEAD或工作区。
3项independent_rebase测试已通过。新UI与Windows完整验收仍待完成，不作整项交付声明。

共享恢复模块调整后，完整核心回归通过（/tmp/plico-branch-rebase-recovery-core.log），桌面入口cargo check --locked --features desktop-tests --bin plico通过（/tmp/plico-branch-rebase-desktop-check.log）。桌面编译不代替原生验收；本项仍待菜单、提示文案与完整Windows流程验证，暂未提交。


### 2. 菜单接入与Windows正向验收

新增ui/branch_rebase，顶部弹出菜单/右键与Log分支右键共用branch_rebase_item，
动态显示Rebase {current} onto {target}及中文“将…变基到…”，放在Merge之前。
当前分支/标签不提供可执行Rebase，detached、busy、未结束Git操作时禁用；
菜单是已验证的Rebased GitRebaseBranchAction对应方向，不是Checkout and Rebase。
文档保存屏障前固定UI显示的HEAD/target，后台Prepared再次核对；完成后工作区和Log刷新，
冲突进入现有Resolve窗口，保留Continue/Abort恢复链路。恢复确认标题改为通用Rebase。

补验Rebased detached HEAD：在自有fixture checkout --detach babee4df，顶部选择
conflict-main后，Rebase babee4df onto conflict-main明确置灰，Merge仍可见。
截图rebase-detached-action.png的子菜单右侧被主窗口截图边界裁切，但禁用项可辨认；
未声称此图完整覆盖子菜单。参考源码禁用规则仍为GitRebaseBranchAction.isOnBranch。

Windows构建/tmp/plico-branch-rebase-windows.log通过。独立rebase-branch-plico-20261024
从a4cbf3f建立native-rebase，保留local.txt=unstaged work，顶部main菜单点击
“将native-rebase变基到main”。无需额外确认，完成后提示“分支变基完成”，Log刷新。
Windows Git读取HEAD=8ce97a8、parent=2d1e6fe(main)，status仅M local.txt，
内容仍为unstaged work，stash list为空。截图rebase-plico-{initial,branches,action,result}.png。
原生自有Plico PID46596/窗口43912172，测试构建rebase-branch-build-20261024。
Rebased参考PID44688仍在detached fixture，另一旧参考PID43328尚未关闭。

桌面测试/tmp/plico-branch-rebase-desktop-tests.log：85通过、2既有忽略。
本次未修改Diff焦点/Tab代码；Windows编译和正向原生不替代剩余场景。
远端分支原生、Plico冲突继续/中止、取消/失败提示、英文菜单仍待验证；第2项未提交。


### 2. Windows冲突中止与继续验收

自有Plico46596，同一fixture切换conflict-topic=babee4df，保留local.txt未暂存修改。
顶部conflict-main→“将conflict-topic变基到conflict-main”，自动进入冲突窗口，
Windows Git status为UU shared.txt；顶部Resolve、禁用Continue、Abort可见。
关闭冲突窗口后点击Abort，出现仓库确认；确认后HEAD恢复babee4df，当前分支
conflict-topic，local.txt仍为unstaged work，stash为空，gitui-squash-saved已清理。
截图rebase-plico-{conflict-action,conflicts,abort-confirm,aborted}.png。

再次从同一菜单进入冲突，点击“接受你的版本”，冲突列表变成“所有冲突已解决”，
顶部Continue启用。关闭窗口→Continue→提交说明编辑器保留topic conflict→确定。
最终HEAD=2850819、parent=211d409(conflict-main)，当前分支conflict-topic；
shared.txt=topic conflict，local.txt仍为unstaged work，status仅M local.txt，stash与
保存记录清理完成。截图rebase-plico-{accept,continue-editor,continued}.png。
此项是真实Windows UI点击与Git结果核对，不是以核心测试代替原生流程。
原生进程重启恢复尚未本次手测；核心跨对象恢复的验证范围保持前述记录。


### 2. 远端目标原生对照

使用自有rebase-branch-remote-20261024.git bare作为两套fixture的origin，预先fetch，
两套仓库均从a4cbf3f新建remote-rebase。Rebased搜索origin/main后，其菜单分别提供
Rebase remote-rebase onto origin/main、Merge、Pull Using Rebase、Pull Using Merge；
验证独立Rebase不是Pull入口。点击独立Rebase后成功通知，HEAD=9d2f655，parent=2d1e6fe，
local.txt修改恢复，.idea未跟踪保持。Plico相同入口操作后HEAD=af55b9e、parent=2d1e6fe，
当前分支remote-rebase，status仅M local.txt，Log自动刷新并提示分支变基完成。
截图rebase-{reference,plico}-remote-{search,action,result}.png。
与参考相比Plico菜单尚未具备本轮其他计划中的Compare/Worktree等项，未用假入口占位；
本次新增Rebase在Merge前，操作方向与行为一致。成功通知仍使用现有状态栏而非浮动通知。
原生执行过程很快，本轮未截到进行中帧，不能把结束截图算作loading验收。
已关闭旧Fork自有Rebased43328，仅保留参考44688与Plico46596两个测试实例。
第2项仍待英文/失败取消反馈检查及提交前最终检查，未提交。


### 2. 失败恢复、英文与最终验证

Plico英文菜单实际显示Rebase rejected-rebase onto origin/main，布局无截断；
在自有fixture的pre-rebase hook输出fatal并exit 1，原生操作进入Rebase requires attention，
完整显示hook错误，底部保留Saved changes retained及stash短OID，不误报成功。
HEAD仍为a4cbf3f、branch仍为rejected-rebase；Stash页选中自动保存项后点击Pop，
提示Saved changes restored，local.txt恢复unstaged work，stash及恢复记录均清理。
测试hook已删除；没有修改真实仓库或远端。截图rebase-plico-{english-action,reject-action,
rejected,recovery,recovered}.png位于前述artifacts目录。恢复页曾错误沿用Squash提示，
本次将提示和读取失败文案统一为Rebase，中英文同时修正；recovery截图保留发现时状态。

真实Git新增hook拒绝及执行中取消用例，检查原HEAD/分支、保留stash和持久归属记录，
不会将取消当成功或删除待恢复修改；共4项independent_rebase定向测试通过。
最终完整核心回归通过（/tmp/plico-rebase-final-core.log）。英文验证后恢复应用中文设置。
原生取消、进程重启恢复和进行中动画未在本轮完整截图验收：取消及跨对象恢复由真实Git
回归覆盖，已有Continue/Abort机制由前述原生场景覆盖。功能验收与未完成的视觉验证分开记录。

最终桌面回归85通过、2既有忽略（/tmp/plico-rebase-desktop-final.log）；未改Tab/焦点，
本次未重跑debug-assertions=false的F7专用配置。Clippy通过但保留既有警告
（/tmp/plico-rebase-clippy-final.log）；fmt与diff检查通过。最终Windows release构建通过
（/tmp/plico-rebase-windows-final.log），新构建已启动并确认中文恢复，PID48388；旧自有
Plico46596/11816已关闭，仅保留一个Plico和一个Rebased。最终通用恢复文案编译与桌面
测试通过，其出现状态的更新截图尚未补拍，不宣称像素级验收全部完成。CI未运行。


### 3. 分支重命名前置：原生与源码

参考版本仍为4aec352，原生Rebased1.1.20/IC-262.10968.SNAPSHOT，自有44688。
在rebase-branch-reference-20261024当前remote-rebase菜单末项Rename…（F2），
弹窗标题包含旧名，单行Branch Name输入默认全选，底部Rename/Cancel；没有upstream
时没有复选框。输入已有main并Enter后红框且Rename禁用；改为renamed-reference成功，
顶部和Log分支标签刷新，local.txt dirty和.idea未跟踪保留。
用Windows Git给测试分支设置origin/main upstream后，首次菜单缓存未更新；执行原生
Reload All from Disk再打开，出现未勾选的Unset upstream branch。输入renamed-untracked，
Tab+空格勾选后Enter，实际分支重命名且for-each-ref确认upstream为空。
截图rename-reference-{start,menu,actions-popup,dialog,duplicate,result,tracking,
tracking-refreshed,unset-result}.png已保存到本节artifacts目录；tracking为刷新前状态。

实读plugins/git4idea/backend/src/actions/branch/GitRenameBranchAction.kt：仅本地，
按hasUpstream决定选项；branch/GitNewBranchDialog.kt：RENAME隐藏仓库选择，不显示
checkout/overwrite，默认unset=false，输入清理及重复/目录/远端名称冲突校验。
branch/GitRenameBranchOperation.java：branch -m成功后再可选unset-upstream，分别处理
两步错误，不把重命名成功而清理upstream失败当整体成功；更新repository并发rename事件。
commands/GitImpl.java确认-m、不强制覆盖；validators/GitBranchValidator.kt确认名称规则。
实读tests/git4idea/branch/GitBranchWorkerTest.kt的保留upstream、取消upstream、多仓库
失败回滚三例。Plico为单仓库窗口，不新增多仓库事务；使用既有操作锁、准备信息复验和
后台执行，分别报告rename与unset结果，保持工作区/index不动。工作树占用、非当前分支
和失败原生尚待补验；UI与本地实现尚未完成，不算第3项交付。

第3项执行层初版：新增branch_rename::Prepared/execute，固定目录、HEAD、symbolic HEAD、
源分支tip和有效分支配置，持锁后重新核对；名称检查拒绝覆盖、目录冲突和远端同名，
执行branch -m后可选unset-upstream。返回两步原始结果以及fresh token读取的实际ref状态，
取消不等同未发生写入。未引入checkout/stash，不触碰index/worktree。
真实临时仓库定向2项通过（/tmp/plico-rename-core.log）：当前/非当前分支重命名，
暂存和未暂存内容分别保留，upstream默认保留/显式取消、pushRemote/description继承，
非法/重复名称拒绝、弹窗期间配置变化拒绝。fmt及diff检查通过。
此处为执行层进展，菜单/弹窗、工作树与两步部分失败测试及Plico原生验收待续，尚未提交。


### 3. 菜单与弹窗初版、Windows正向验收

新增ui/branch_rename，顶部下拉/右键和Log分支右键共用入口，仅本地分支可见。
后台读取Prepared后显示旧名称且全选，输入复用clean_name_on_typing并避开IME预编辑；
有upstream才显示默认不选的取消跟踪，确认前后台校验名称，关闭取消校验且不接纳迟到结果。
执行使用独立RenameBranch生命周期，分别显示成功、取消、重命名后upstream失败；完成刷新Log。
中英文标签齐备。尚未补齐F2入口、rename事件对过滤/收藏/最近分支引用的迁移，不能算交付完成。

Rebased补验：创建自有rename-linked-reference-20261024，使用rename-occupied分支。
主仓库菜单提供Open Worktree且Rename可用；Rename弹窗提交rename-occupied-done，
Windows Git确认linked worktree的当前分支随之更新。参考截图rename-worktree-reference-*。
真实Git新增锁失败/执行前取消、linked worktree重命名且保留draft用例；共4项rename测试通过
（/tmp/plico-rename-core.log）。桌面测试85通过、2既有忽略（/tmp/plico-rename-desktop.log）；
Windows release构建通过（/tmp/plico-rename-windows.log），不是CI结果。

Plico自有41756/窗口1247570：顶部当前rejected-rebase→重命名，输入renamed-plico并Enter，
执行后当前分支和Log标签均更新，Windows Git status仍仅M local.txt。
截图rename-plico-{search,actions,dialog,result}.png已保存。结构对照：输入/按钮及可选
upstream组织一致；Plico使用现有应用内弹窗，标题字重、宽度、焦点描边尚有视觉差异。
Plico原生upstream选项、错误/取消、快捷键及过滤状态迁移仍待验收，暂不提交。
本轮关闭旧Plico48388，现仅保留Plico41756和Rebased44688；无额外测试窗口。

第3项状态衔接：实读同一4aec352的ui/branch/GitBranchManager.kt、平台
DvcsBranchManager.java与BranchStorage.kt，收藏按类型/仓库/名称存储；没有在这些实现或
全仓BranchRenameListener引用中读到收藏迁移消费者，所以保留既有按名收藏语义，不推断
为随OID迁移。最近分支继续使用既有reflog查询，不新增独立持久化历史。
Plico已将选中的精确branch过滤及selected_tree_ref随实际重命名结果更新，避免旧ref查询
失败；这是本地查询模型的显式保障，未宣称上游具有相同的过滤迁移实现。
新增分支树F2和BranchPopup专属F2命令，History的F2仍是编辑提交说明。

补充真实Git include配置用例：本地unset-upstream成功后，include文件中的upstream仍
有效，不能据退出码宣称完整成功。执行层新增有效upstream复查及部分成功诊断；5项rename
定向测试通过。桌面回归85通过、2既有忽略（/tmp/plico-rename-desktop-followup.log）。
最终Windows构建及原生快捷键/upstream验收继续进行，当前尚未提交。


### 3. 最终验证与交付

最终Windows构建通过（/tmp/plico-rename-windows-final.log）。自有Plico42392，
当前分支菜单搜索后F2打开带upstream选项的重命名弹窗；勾选取消跟踪后重命名为
renamed-no-upstream，Windows Git确认新名称和空upstream，工作区修改保留。
随后在Log分支树选择并过滤该分支，F2重命名为rename-filtered，顶部、分支树选中项、
精确分支过滤及Log标签全部更新，过滤结果保持2/2条，无旧ref错误。
截图artifacts/collaboration-workflows-20261024/rename-plico-{f2-upstream,unset-selected,
unset-result,filter-dialog,filter-result}.png。仅保留自有Plico42392和Rebased44688。

完整核心回归通过（/tmp/plico-rename-full-core.log），重命名定向5项通过；
桌面85通过、2既有忽略（/tmp/plico-rename-desktop-followup.log）。Clippy通过且保留
既有警告，fmt检查及diff检查通过；本项未修改Tab/编辑器生命周期，未重复F7专项配置。
CI未运行。原生英文、错误/取消弹窗及进行中帧未完整截图验证，不能据核心用例声称
这些视觉状态已验收；前述标题字重、弹窗宽度、焦点描边差异仍保留。
相对上游，本地对unset失败保守显示部分成功，并复查include配置中仍有效的跟踪，
不把rename成功误报为整项未发生，也不改写外部include配置。第3项功能交付完成。

### 4. Worktree前置：创建、列表与删除

参考commit仍为4aec35256d765c217aa0d662b82ac4e860c396fd，原生版本1.1.20 /
IC-262.10968.SNAPSHOT，自有44688。Find Action→Worktrees打开底部Git工具窗口的
Worktrees页；列表为名称、分支、路径，当前项加勾，左侧竖排新增/删除/刷新/清理。
新增弹窗依次From branch、New branch复选框及输入、Project name、Location和完整目标
路径提示，底部Create & Open Worktree/Cancel。选择conflict-topic后自动建议目录名称，
输入worktree-native后自动选中New branch且目录建议更新；确认后Git生成同OID的新分支
及工作树，再弹New Project选择This Window/New Window/Cancel。取消打开不删除已创建
工作树，原列表更新。Windows Git worktree list确认实际创建及原分支不变。

随后仅在本次自建worktree-native内写入own-dirty-probe.txt，选中该工作树点击删除，
原生只显示包含路径的Delete Worktree确认，不提示dirty；确认后目录及未跟踪探针被删除，
Windows Git列表只剩原来的两个工作树。测试未触碰用户仓库或文件。
截图artifacts/collaboration-workflows-20261024/worktree-reference-{search,list,create,
selected,new-branch,open-choice,created,remove-confirm}.png。

已实读workingTrees下ShowWorkingTreesAction、GitCreateWorkingTreeAction、
GitWorkingTreeDialog、GitCreateWorkingTreeService、GitWorkingTreesService、
GitWorktreeListParser、GitWorktreesViewModel、RemoveWorkingTreeAction及
GitImpl.createWorkingTree/deleteWorkingTree/listWorktrees；测试读取
RemoveWorkingTreeActionTest与GitWorkingTreeFromRemoteBranchesTest。
规则：已有本地分支直接checkout；远端创建对应本地跟踪分支；tag是detached；新分支用-b
而非-B。已有占用分支由Git拒绝后额外询问，不默认force。main/current不允许从列表删除；
上游删除确认后实际使用--force，Windows权限失败还有递归删除重试。

本地实现计划：执行层独立worktrees模块，列表按需后台查询，NUL分隔保留特殊路径；
创建前复验源ref/OID和目标路径，串行化同仓库操作；取消后刷新实际工作树列表，不假定
没有写入。删除遵从本轮明确的dirty保护目标，拒绝当前/main/locked及含修改或未跟踪文件
的工作树，调用不带force的Git remove；不照搬递归删除兜底。该差异是明确保留的保护，
不是宣称上游会阻止dirty删除。已有分支占用、打开已有树、原生失败/取消状态仍待续验。

用户后续明确要求按Rebased逻辑并允许改计划：第4项删除规则已改为确认后使用
worktree remove --force，main/current禁止，locked由Git单次force拒绝，不能自动升级
为双force。撤销上一段额外dirty禁止规则；测试改为确认删除目标dirty/untracked，
同时验证原工作树编辑不受影响。Windows权限失败重试仍需独立验证及打开窗口处理，
不得未经二次确认递归删除。核心实现开始，UI尚未接入，不算整项交付。

第4项执行层初版：新增worktrees模块，NUL列表保留换行/空格路径，常数次Git查询识别
当前工作树（不对每一项启动Git）；Create准备固定来源OID/目录，执行持同仓库锁复验，
已有本地分支保持分支身份、远端默认创建跟踪分支、标签detached。操作结束含取消均用
新token重新读列表。删除遵从用户更新后的规则：非main/current在确认后单次--force，
不额外阻挡dirty；locked仍由Git拒绝。尚未接入UI，不向用户暴露未经确认的删除入口。
真实Git 4项通过（/tmp/plico-worktrees-core.log），覆盖原工作区dirty保留、删除目标dirty
和untracked、当前/main拒绝、locked失败、源ref变化/取消不执行、路径空格/换行、远端
跟踪和tag detached。仍待UI、跨窗口处理、完整核心及Windows原生本地功能验收。

补验原生已占用分支：创建弹窗选择rename-occupied-done时橙色提示“Selected branch
is checked out, so a new branch is required”，Create & Open禁用，截图
worktree-reference-occupied.png。此处安装版与4aec352源码不同：源码已允许提交后由Git
拒绝并询问force，安装版直接要求新分支。本地初版按安装版阻止重复checkout，提供从
该分支创建新分支的路径；后续UI应保留打开已占用工作树入口，不把源码新行为误写为
已在安装版验证。窗口打开与权限失败二次确认仍待验证，不能宣称完整对齐。

执行层完整核心回归通过（/tmp/plico-worktrees-full-core.log）。当前只有核心模块及测试，
尚无Plico界面改动或原生验收，不提交为完整功能。分支重命名已独立提交e4046e0，未推送。

第4项原生补验打开/关闭删除：在Worktrees列表双击rename-linked-reference-20261024，
出现This Window/New Window/Cancel。选New Window后信任仅本次自建fixture，取消勾选
Defender排除（未更改系统设置），打开工作树窗口44762922。回到主仓库选中该树删除，
第一次Delete Worktree确认后出现第二次“currently opened… close it first?”，按钮
Close and Delete/Do Not Delete，截图worktree-reference-close-delete.png。确认后关闭
本次自有工作树窗口并删除对应目录。对应源码GitWorkingTreesService.doDeleteWorkingTree/
shouldStopDeletion/closeProject，与实测顺序一致。本地应按窗口状态复验、保存文档后关闭，
再执行已确认删除；不得在工作树窗口未关闭时直接删除。原生取消删除和权限失败重试
仍未截全。本轮仍只使用自有fixture及Rebased44688，未关闭用户窗口。

第4项布局返工（2026-10-24）：用户指出创建弹窗和底部工作树面板未对齐，本轮优先纠正
布局后再继续功能收尾。重新在 Rebased 1.1.20 原生点击左栏底部 Git 图标，确认同一按钮
切换底部工具窗口显示/隐藏；面板在左侧工具条右侧，横跨 Project 和编辑区，工具条贯穿
上下两区。截图 worktree-reference-{layout,hidden,dialog-layout}.png。新建弹窗实际宽约
500 逻辑像素，来源下拉框、新分支输入、项目名、位置共用标签列；分支区和目录区之间
有较大留白，下方路径提示从输入列起排。之前按整个窗口追加面板和按钮自收缩布局不符。

再次读取参考 4aec352 的 GitWorkingTreeDialog（LABEL_ALIGNED、Align.FILL）、
GitWorktreesTabPanel（纵向操作条）、GitWorkingTreeRowComponent。注意此版本源码已
改为两行工作树单元格/左侧 500px 列表，安装版仍为名称、分支、路径的单行显示；本轮
视觉采用已实测安装版，明确不把两者说成同一布局。Plico 将 Git 入口固定到左栏底部，
面板改用左条右侧的纵向可调整分隔容器，隐藏保留列表状态，关闭标签释放列表；表头
补足横向宽度、标签框及隐藏按钮，单行去掉名称/分支固定列宽。创建框收紧宽度、对齐
标签列，完整下拉框显示箭头。原生重新验收待本轮构建后记录。

此前初版原生验证：创建 worktree-plico 并在新窗口打开，删除先确认再 Close and Delete，
关闭对应窗口后移除工作树（包括测试新建的 own-dirty-probe.txt）；最终列表只剩原树，
截图 worktree-plico-removed.png。修复 Add 回调重借 Browser 导致的崩溃，回归测试先红后绿；
86 桌面测试通过、2 个导航测试按既有配置另行运行，共 8 个导航通过。初版构建通过，
这些结果不替代本轮布局变更的测试及原生验收。

本轮布局实现后第一次 Windows 原生检查：Git 底部入口与面板横向范围已正确，工作树
名称/分支/路径相邻显示；创建框已形成完整下拉框与统一输入列，并且位置输入保留真实
绝对路径，不再错误填入仅供展示的 `~` 路径。截图 worktree-plico-layout-{start,panel}.png、
worktree-plico-dialog-layout.png。继续按用户反馈调整纵向操作栏：36px 容器中居中放置
24px 按钮，左右各留 6px；标题 Git 与 Worktrees 标签间距改为 16px。删除图标采用参考
expui/general/delete_dark.svg（保留 Apache-2.0 来源），补齐关闭标签图标资源映射；创建
框使用项目已有 JetBrains checkbox 展示，输入框底色/边框及上下留白再次校准。
读取 GitWorkingTreesContentProvider/ShowWorkingTreesAction 确认 Worktrees 是 VCS
工具窗口可关闭内容页；隐藏与销毁区分。保留差异：当前没有 Console 页，也没有伪造
无行为的 Console/Prune/Help 控件；这部分不宣称已完全像素对齐，属于第4项后续收尾。

最终本轮验证：Windows release 重建通过（/tmp/plico-worktree-layout-final-windows.log），
原生截图 worktree-plico-layout-final.png、worktree-plico-dialog-final.png 显示按钮居中留白、
16px 标题间距、关闭标签图标及复选框/输入列对齐；点击 Git 可收起面板，见
worktree-plico-layout-hidden.png。截图均复制至 artifacts/collaboration-workflows-20261024。
完整桌面86通过/2按既有配置单独运行，导航8通过；最后纯展示微调后 worktrees 回归1通过。
格式和 diff 空白检查通过。调整高度的鼠标拖动、英文弹窗及 Prune/Console 页本轮未验收，
不宣称整体 Worktrees 功能或像素对齐全部完成。保留一个本轮 Plico 和一个 Rebased 窗口。

第4项 Prune 前置（2026-10-24）：原生 Rebased 自有参考仓库创建 detached
worktree-prune-reference-20261024，随后仅删除该测试目录，刷新后显示 Prunable；扫帚
启用，确认框列出该目录名称，Prune Worktrees/Cancel。确认后 Windows Git 列表只剩主树。
截图 worktree-reference-prunable.png、worktree-reference-prune-confirm.png。
实读 4aec352 PruneWorkingTreesAction、GitWorkingTreesService.pruneWorkingTrees、
GitImpl.pruneWorktrees：以存在 isPrunable 为启用条件、列名称确认、后台普通 worktree
prune（不添加 expire now）、失败通知、成功刷新。OpenWorkingTreeAction 禁用 current
和 prunable。实现映射 worktrees::prune、ui/worktrees 工具条与确认入口；确认后复验已确认
的失效路径集合，变化则要求重新确认，避免无关新增记录混入旧确认。

Prune 实现与验收：worktrees::prune 持操作锁复验确认路径集合，调用普通 Git prune，
返回实际刷新列表；UI 仅存在 prunable 且非 busy/loading 时启用，关闭确认不执行。
5项真实 Git worktrees 测试通过，新增覆盖执行前取消、空/陈旧确认、目录恢复、锁定
记录保留、其他工作树未提交修改保留；桌面 worktrees 回归1项通过，Windows构建通过。
原生 Plico 在独立测试仓库删除自建 worktree-prune-plico-20261024 目录后显示可清理，
点击确认并 Enter 后记录消失，Windows Git worktree list 确认只剩原树；截图
worktree-plico-{prunable,prune-confirm,pruned}.png。首次图标误取旧 GC 垃圾桶；随后读取
backend.xml 确认 action 使用 AllIcons.Actions.ClearCash，替换为 expui/actions/
clearCash_dark.svg 扫帚资源（保留来源许可证），该资源替换原生复验待下轮。
第4项仍有创建表单英文/失败/取消、已打开窗口复用、Windows删除失败恢复等收尾，未提交。

工作树窗口复用前置：参考创建 worktree-open-reference-20261024，第一次双击提示 New
Project，选择 New Window；信任对话框取消 Defender 排除后仅信任本次测试目录。
窗口3606084打开后回主窗口再次双击该工作树，没有再次出现窗口选择弹窗；Win32读取
前台 HWND=3606084，枚举仍只有两个项目窗口。截图 worktree-reference-open-choice-2.png、
worktree-reference-reused.png。源码4aec352 GitWorkingTreesService.openWorkingTreeProject/
openProjectInNewWindow→ProjectUtil.doOpenOrImportAsync先findAndFocusExistingProjectForPath，
随后才询问新窗口。目录不存在时通知并重载工作树。Plico映射：工作树打开专用入口先
复用同路径窗口；新目录检查放后台，仓库切换/后续请求取消前一任务，检查后再次复用，
否则走已有选择窗口流程。不把兄弟工作树合并为同一仓库窗口。

工作树窗口复用实现/验收：ui/worktrees::open_worktree_project 先查询同应用窗口，再后台
验证目录、检查仓库身份和 busy 状态，回到 UI 后再次查询窗口，避免异步检查期间重复
打开。Review 持有打开任务，切仓库及新请求释放前一任务；列表 current/prunable 禁止打开。
新增双窗口桌面回归，区分同一路径和兄弟工作树，确认不生成选择弹窗/额外窗口。
完整桌面87通过/2既有导航配置忽略；导航专用配置8通过；Windows release通过。
原生 Plico 独立 worktree-open-plico-20261024：首次选择新窗口2426952；返回主窗口双击
同一行，Win32前台确认为2426952，枚举仍仅两个项目窗口，无重复弹窗。截图
worktree-plico-open-choice-2.png、worktree-plico-reused.png；本次额外测试窗口已关闭，
参考额外窗口3606084也已关闭。扫帚资源修正亦随此构建原生显示确认。
打开后目录被删除/恢复的竞态、Windows删除失败恢复及剩余英文/创建表单边界仍待验证。

Windows删除失败原生补验（2026-10-24）：在自有 worktree-open-reference-20261024
使用 .NET 文件句柄拒绝删除共享，Rebased确认删除后 Git 已移除 worktree 记录但目录
残留，通知 Worktree deletion failed。再用全新的 worktree-permission-reference-20261024
仅对测试目录/探针文件临时拒绝删除权限，结果相同；Console复制最后错误行确认两例
均为 failed to delete '…': Invalid argument（不是permission denied）。本次锁持有进程
已退出，临时ACL恢复确认RESTORED；未更改用户仓库或其他文件权限。

源码4aec352 GitWorkingTreesService.doDeleteWorkingTree只在Windows且stderr含
permission denied时显示Worktree Deletion Failed/Try Again/Cancel，用户再次确认后
EelFileUtils.deleteRecursively；其他错误直接通知，不自动递归删目录。本机两种真实
失败均不满足该分支，因此不能声称已实测Try Again，也不把Invalid argument纳入重试。
Plico同条件 worktree-open-plico-20261024 测试返回完整原始错误、列表反映实际只剩主树，
没有误报成功、没有残留运行中操作；截图worktree-plico-delete-failed.png。参考截图
worktree-reference-{locked-delete,delete-error-console,permission-delete}.png。
此场景已对齐。permission denied专用递归重试仍缺实机前置，不凭推测实现；不把它当作
本机文件占用问题继续重复验证。后续独立实现需真实触发该错误与确认界面。

另按同服务notifyWorkingTreeDeletedSuccess补齐成功删除后的最近项目清理，放在操作
后台任务中，失败/取消/部分删除不移除最近项目记录；UI接纳返回列表。此小改动待当前
桌面回归，原生成功删除后的最近项目列表检查仍待收尾。
删除后最近项目清理改动的desktop worktrees回归2项通过；fmt/diff检查通过。
三处本轮明确自建且Git元数据已移除的测试残留目录已清理，文件句柄持有任务均已终止。

第4项创建来源补齐（2026-10-24）：再次读取参考4aec352的
GitWorkingTreeDialog.kt::computeRefsWithWorkingTrees/RefWithTreeCellRenderer：
本地分支、远端分支、标签分别排序并依次展示，引用身份不能仅用短名称。
安装版Rebased原生创建框已验证选择worktree-reference-tag后自动建议项目名，
未勾选New branch亦可创建；截图worktree-reference-tag-{options,selected}.png。
本地src/ui/worktrees/form.rs此前只读取Snapshot.branches，现合并Snapshot.tags，
保留完整refs身份并按本地/远端/标签排序，排除后端不接受的远端HEAD入口。
核心5项真实Git测试通过（含remote追踪与tag游离身份），桌面工作树2项通过；
格式及diff检查通过。此结果不替代Plico新标签入口的Windows原生验收，后者待重建。
来源菜单尚未复刻参考的图标、占用工作树后缀及搜索，此差异仍保留待处理。

创建来源菜单展示补齐：读取同参考GitBranchesTreeIconProvider.kt及
PlatformIconMappings.json/AllIcons.java，确认BranchNode在现代UI映射为
expui/general/vcs.svg；标签使用DvcsImplIcons.BranchLabel，当前分支使用
CurrentBranchLabel。Plico复用既有相应暗色图标，在来源菜单展示图标及灰色
占用工作树名称，不再只有无类型区分的短名称。Browser传递Tree快照，保留
原来的占用校验；不在弹窗渲染阶段查询文件系统。桌面工作树2项测试通过。
收藏分支标记与可搜索来源菜单仍未对齐；此处不宣称完整像素级还原。

Windows来源菜单原生验收：新构建PID33660，独立rebase-branch-plico-20261024仓库，
选择worktree-source-tag，未勾选新分支并执行创建。宿主Git确认新工作树HEAD为
标签目标a4cbf3f1183dea1dafb5fdb2ff942f6c840b92aa且detached；主仓库仍为
rename-filtered。打开窗口选择页取消后保留已创建工作树；验收完成使用普通
worktree remove清理此干净测试树。截图worktree-plico-source-{menu,tag-selected,
tag-open-choice}.png。菜单图标/灰色占用提示已显示；图标实际视觉偏小、来源按钮
选择后还未显示图标，继续保留为视觉差异。原生进程仅保留一个Plico与一个Rebased。

工作树图标与选择状态收尾：来源菜单原图标受PopupMenu::render_icon的xsmall
覆盖，现仅在该菜单自绘16px图标；来源按钮也显示相同图标，避免修改通用菜单
引起全局尺寸变化。再次读取4aec352 GitWorktreesTabPanel.kt::applyEntries/
selectDefaultEntry：保留原路径，路径消失时优先当前工作树，否则第一项。
本地Browser::reload按此恢复选中项，不再将刷新后的选择清空。桌面2项通过，
Windows重建及视觉复测进行中。

Windows PID48480原生复测：16px菜单图标及默认当前工作树选中已生效，
见worktree-plico-icons-menu.png。来源按钮图标已出现，但发现通用Button在
有图标/文本/caret时justify_between把文字挤到中间；已将图标与文字组合成
一个左侧内容组，caret仍在右侧，保留可访问标签。该最后修正等待下一轮
Windows重建复测，不将worktree-plico-icons-selected.png的中间态记为对齐完成。

2026-10-25创建来源按钮原生收尾：重建后PID40432验证标签图标与名称靠左相邻，
caret保持右侧；截图worktree-plico-source-aligned.png。取消弹窗无Git写操作。
另补Report::diagnostic，保留取消/超时/非零退出/输入失败原因与Git输出、刷新错误，
避免无stdout/stderr时产生空白详情。tests/worktrees.rs新增真实git diff --quiet
非零且无输出案例，并核对取消/超时及刷新失败组合，6项工作树测试通过。
此诊断修正晚于本次Windows构建，尚待最终构建；不声称已做原生超时故障注入。

### 第4项提交前验收（2026-10-25）

Windows自有PID40432通过设置切换English，创建框标签、复选框、路径说明与按钮
没有截断/重叠；关闭后拖动Git工具窗口上边界，面板高度随拖动变化，Log/详情同步
缩放而左侧入口保持底部。截图worktree-plico-english-form.png、worktree-plico-resized.png。
验收结束通过设置恢复中文。此为调整分隔线验收，没有执行内容滚动自动化。

完整核心测试通过，桌面87通过/2按既有配置忽略，另行导航8通过；Clippy通过但
保留既有警告，格式与diff空白检查通过。日志/tmp/plico-worktrees-final-{core,
desktop,navigation,clippy,format}.log；最终Windows构建结果待结束补录。

交付范围为列表、创建本地/远端/标签或新分支、复用/新开窗口、关闭已打开窗口后
确认删除（允许丢弃目标树本地修改）、清理失效记录、异步刷新和失败详情。
保留差异明确记录：当前单仓库工具窗口无Console页；来源菜单无收藏/搜索，主仓库
根目录作为关联工作树默认项目名前缀尚未复制；列表未支持参考的右键和键盘入口；
Windows仅permission denied时的额外文件系统重试分支尚未实现，本机占用测试
返回Invalid argument，两者均保留错误而不误报成功。上述不作为已对齐项，后续
对齐继续保留，不将此提交描述为完整复制Rebased或完全像素一致。

最终Windows release构建通过（1分24秒，/tmp/plico-worktrees-final-windows.log），包含诊断修正。完整核心743项通过；本次提交是工作树管理主流程交付，前述剩余对齐项仍保留，完整目标未完成。

### 5. 分支对比：原生与查询层（2026-10-25）

安装版Rebased 1.1.20 / IC-262.10968.SNAPSHOT，自有仓库
rebase-branch-reference-20261024，当前renamed-untracked，分支菜单选择
conflict-topic → Compare with renamed-untracked，打开独立编辑区Compare页。
上下两份Log分别显示topic独有1条、current独有2条，每份有自己的过滤栏、
方向说明、提交区及右侧文件/详情区。分支菜单另外提供Show Diff with Working Tree，
不能把该入口等同于纯提交树比较。只读查看，没有执行checkout或改写用户仓库。
截图compare-reference-{branches,action-popup,dual-log}.png。

实读4aec352：GitCompareBranchesUi.kt的create/createTopUi/createBottomUi、
MyFilterUi及asReversed；GitBranchesUIHandler.kt比较入口默认另一端为当前分支，
无共同分支时HEAD；GitCompareBranchesFilesManager.kt按range/root复用编辑页；
GitCompareBranchesHelper.java/CompareBranchesHelper.java确认日志语义A..B。
GitBranchWorker.java::loadTotalDiff明确工作区比较是另一条路径。
规则：上下分区独立range和过滤状态，清除用户过滤仍保留range/root，反向查询
交换两端，不将独有提交补丁累加成整体Diff。原生与读取版本差异继续保留。

本地新增src/branch_compare.rs，Pair固定两个完整OID；两方向分页读取DateOrder日志，
count独立查询，refresh重新resolve；changes复用revision_changes比较两棵树，
与现有CompareWithLocal视图区分。复用git.rs::parse_history，未放宽现有history根
校验，也未把任意用户输入直接作为Git选项。tests/branch_compare.rs真实Git验证
3对1分叉、分页/末页、交换、固定版本与引用移动后刷新、相同版本空结果、
整体两文件差异、工作区修改保持不变、失效引用/选项输入拒绝及取消，测试通过。
新增代码尚未接UI；过滤、两区graph/详情、tab复用生命周期及原生Plico验收待实现。

第5项双日志组件实现中（2026-10-25）：新增src/ui/branch_compare.rs，View持有
上下两个独立Pane，复用graph_cell和Graph布局、Git提交元数据与log_commit_changes
的merge-parent规则、Diff Target/Chain；不借用主Review的选中项或请求状态。
每个Pane独立异步读取首批/总数/按需详情，取消与generation拒绝迟到详情。
两区分别可调日志/文件区宽度，上下分隔可调；双击文件产生OpenDiff事件。
列表每个保留窗口不超过HISTORY_MAX，继续向后时替换旧窗口，可返回前一窗口，
没有通过删除后续历史入口来限制内存。数据层补shallow_boundary标记，避免把浅
边界伪装成真实root比较空树。

cargo check通过（因尚未接入Review入口有5个dead_code警告，未屏蔽），分支对比
真实Git测试复跑通过，格式/diff检查通过。此组件尚不可从用户界面打开，不能据此
宣称功能可用；下一步接Tab排序/关闭/切换仓库的生命周期、分支菜单、详情Diff事件，
以及过滤、方向交换/刷新与整体差异入口。尚未进行Plico原生验收或提交。

第5项编辑页生命周期接线（2026-10-25）：再次读取同一参考版本
GitCompareBranchesFilesManager.kt，落实按范围/仓库复用页与关闭时移除身份。
Review 新增独立分支对比页集合，接入统一 editor_tabs 排序、激活、标题、关闭按钮、
右键批量关闭及 CloseFile 快捷动作；关闭活动页回到相邻编辑页，切仓释放集合及
Pane 持有的取消令牌。Log 选中态和本地底部预览不再把对比页误认为主 Log。
open_branch_comparison 以引用对复用页，双击文件接现有 DiffTab 文档和导航机制。
当前尚未接分支菜单的异步引用解析入口，原生界面验收待完整入口后进行；本条
只记录生命周期接线，不代表分支对比功能已交付。过滤和刷新等范围仍按上述计划保留。
定向桌面回归 branch_comparison_reuses_tabs_and_closes_to_neighbor 通过：相同范围
复用、反向范围独立页、关闭回退相邻页、最后一页清理；cargo fmt 及 diff 检查通过。
本轮未运行完整桌面/导航回归，也未完成 Windows 原生验收，不提交本功能。

第5项比较入口接线（2026-10-25）：再次实读同一版本
GitBranchesUIHandler.kt::compareWithCurrent/compare，当前分支作为第二端，detached
使用 HEAD。沿用已保存 compare-reference-action-popup.png 的入口，加入顶部本地/
远端分支子菜单、右键菜单和 Log 分支树右键；当前分支自身不提供有效比较动作。
文案“与 {current} 比较”按界面语言显示。引用解析在后台执行，Request 同时拥有
任务与取消令牌，替换请求/切仓/销毁时取消；结果接纳检查该次令牌，错误走现有
操作失败展示。已有相同引用对直接激活，避免重复解析打开多个页。
cargo check 无警告通过，Tab 定向回归复跑通过；原生构建和界面验收继续进行。
Windows 原生初步验收：release 构建完成，新自用 Plico29260 替换并正常关闭40432。
独立 rebase-branch-plico-20261024 从顶部 conflict-topic 子菜单选择“与 rename-filtered
比较”打开编辑页；两区分别2/1条，宿主 Git rev-list 两方向 --count 同为2/1。
选中上区 topic conflict 后右侧读取 shared.txt，双击打开211d4091→28508190的
只读Diff（main conflict→topic conflict），原比较页保留。截图
compare-plico-{start,menu,action,dual-log,details,diff}.png 已复制 artifacts。
实机发现选中行宽度未铺满、空状态文案漏翻译，已补 w_full 和复用已翻译提示；
这两处最后改动待下次原生构建复验。尚未对齐过滤工具栏、日期/引用列、详情排版、
空结果、刷新/交换及整体树差异入口，不以本次入口贯通替代第5项完整交付。

第5项普通过滤查询层（2026-10-25）：再次读取4aec352的
GitCompareBranchesUi.kt::MyFilterUi/setFilters/setCustomEmptyText，以及
plugins/git4idea/backend/src/log/GitLogProvider.kt::getCommitsMatchingFilter/
getGitLogParameters。规则：普通text/user/date/path与独有提交range相交；清除
用户过滤不删除range；path使用full-history/simplify-merges；count与page必须
同条件。本地Pair新增page_filtered/count_filtered，复用LogFilter参数生成及
路径校验，显式literal-pathspecs；Some(empty authors)返回空集。路径匹配后
按固定OID恢复真实提交元数据，不能让简化父链变成Diff基线。仍保留有界分页。

实读platform/vcs-log/impl/src/com/intellij/vcs/log/visible/VcsLogFiltererImpl.kt
filter/applyHashFilter发现：命中哈希时优先返回，甚至绕过range及其他filters；
未命中才回普通文本路径。与“所有查询必须交集”直觉不同。此时仅实现普通
详情过滤查询；hash优先级属于后续query controller，不能把page_filtered的grep
包装为已完成Text or hash。该特例尚未在原生Rebased实测，UI搜索入口暂未接入。
普通过滤层沿用前置双Log实机结构记录，本轮没有新增可见控件。

真实Git三项通过（/tmp/plico-compare-filter-test.log）：两方向固定引用与分页；
文本大小写/正则、作者、日期、路径交集、清空恢复range、非法路径、空作者；
路径过滤跳过中间提交后仍保留真实父OID；含方括号的路径按字面匹配，merge
父提交恢复完整两端，page/count一致。cargo fmt与diff检查通过。新增过滤层
尚未接UI，过滤图投影、hash实机验证及原生过滤交互仍待后续，未提交。

第5项搜索前置补验（2026-10-25）：Rebased1.1.20自有44688仍在同一参考仓库
Compare: conflict-topic and renamed-untracked；上区原有topic conflict一条，下区
feature/main update两条。上区粘贴公共base哈希477c32b并Enter，显示范围外base，
下区不变；清空并Enter恢复上区独有一条，下区仍两条。截图
compare-reference-{filter-start,hash-pasted,search-cleared}.png。最初SendKeys受IME
影响产生错误文本，故改用剪贴板粘贴；不以错误输入截图作语义证据。
再次读取同一4aec352 VcsLogFiltererImpl.applyHashFilter：哈希ID优先及后续消息引用
合并；复用本地主Log已实现的ID/消息匹配规则。Query固定仓库refs与比较两端OID，
准备哈希匹配一次并保留有界ID结果，分页仅加载需要的元数据，count复用同一集合；
无命中退回原range普通查询。沿用现有完整图预算，超过预算报错；大规模哈希扫描
优化未完成，不宣称无界低成本查询。

本地新增Query与history::ordered_hash_matches；真实Git四项通过，新增验证公共
祖先哈希绕过range/空作者/日期/路径、另一分支消息引用合并、忽略大小写、分页、
引用移动后固定结果、清空恢复range、无命中及取消。UI两Pane分别拥有InputState、
LogFilter、Arc<Query>，Enter/模式按钮/清除操作取消旧请求并重置本侧页与详情，
page/count从同一Query派生；未复用主Log选中或筛选状态。cargo check通过。
作者/日期/路径控件仍未接入，搜索和清除控件像素位置尚需精调；原生搜索验收进行中。
本轮Windows原生搜索验收：自有Plico6892替换并正常关闭29260。搜索框输入公共
base哈希477c32b后上区显示base且计数1/1，下区feature及1/1不变；点击清除按钮
恢复上区两条独有提交及2/2，下区不变。初始空状态现为中文，行宽已铺满。
截图compare-plico-{search-start,hash-result,search-cleared}.png。Windows release
构建与Tab定向回归通过；未完成User/Date/Paths控件和所有空结果/刷新/图投影等
验收，不提交或声明第5项完成。仅保留此自用Plico和参考Rebased实例。

第5项User/Date/Paths控件（2026-10-25）：实现前在原生Rebased44688同一Compare页
依次打开上区User、Date、Paths，保存compare-reference-{user-menu,date-menu,
paths-menu}.png。User为Select/me，Date为Select/Last24hours/Last7days，Paths为
Select/Select in Tree；这些是每份Log自己的控件，不属于比较页全局筛选。
再次实读4aec352的DateFilterPopupComponent.java::createActionGroup/SelectAction，
UserFilterPopupComponent.java::createActionGroup/createSpeedSearchActionGroup及
StructureFilterPopupComponent.java::createActionGroup。日期按本地日历减1/7天，
自定义无上下界不覆盖既有过滤；User有身份选择/me/最近项，Paths支持文本和树。

本地Pane各自持有UserMenu与PathsMenu实体，复用既有身份解析、路径选择、弹窗、
取消和错误规则；日期复用date_filter::presets/Bounds及DateDialog。过滤应用从
组件取条件，清除时同时清除组件状态而非只改LogFilter；旧请求令牌及详情失效。
Date菜单与参考相同三个入口，激活日期后提供独立清除。未加入比较范围的Branch
过滤入口。cargo check通过；本轮新控件尚未Windows原生验收，窄窗溢出处理待补。
桌面定向回归pane_filters_cancel_old_work_without_touching_the_other_side通过：
日期/输入正确进入Query条件，修改与清除均取消旧令牌，清除保留范围，另一侧
状态及令牌不受影响。测试首次编译因GPUI测试宏通配导入递归失败，按已有测试
显式导入core::prelude::v1::test修正，未增加递归上限。格式/diff检查通过。
本轮控件原生验收及完整桌面回归仍待继续，功能未提交。

第5项窄面板前置（2026-10-25）：原生Rebased44688拖动上区列表/详情分隔线，
上区日志可用宽度减小后User/Date/Paths进入右箭头菜单，搜索保留；下区宽度与
工具栏保持不变。仅拖分隔线，没有自动化内容滚动。截图compare-reference-narrow.png。
重读4aec352 platform/editor-ui-api/src/com/intellij/openapi/actionSystem/
toolbarLayout/AutoLayoutStrategy.kt::doCalculateBounds：预留溢出按钮位置，
放不下的后续控件隐藏到溢出入口。本地Pane新增自身工具栏分配宽度观察，复用
主Log的650px阈值与PopupMenu子弹层命中规则；窄时收起三个过滤控件，仍访问同一
UserMenu/PathsMenu实体及DateDialog。保留差异：阈值适配，不是完整Swing逐项
宽度策略；宽/窄原生操作验收继续进行。

第5项窄面板原生验收：Plico11452在上区Paths选择feature.txt得到0/0，下区仍1/1；
缩窄上区后打开溢出菜单→路径→选择，改为shared.txt并Ctrl+Enter，上区显示两条
及2/2，下区保持1/1。子弹层未因父菜单外部点击判定误关。截图
compare-plico-{full-filters,path-applied,narrow,overflow,overflow-path,overflow-applied}.png。

第5项过滤空结果前置：原生Rebased44688同一比较页上区输入不存在的主题
plico-no-such-subject-20261025，显示居中的“No commits matching filters”和
“Reset filters”链接，下区不变。截图compare-reference-empty-filter.png。
实读4aec352的GitCompareBranchesUi.MyFilterUi.setCustomEmptyText及
VcsLogFilterUiEx.java::setCustomEmptyText：附加条件空结果用通用文案与重置入口，
无附加条件才用范围为空的专有文案。本轮先补已实测的过滤空结果，重置复用Pane
已有clear_filters，仅清除本侧条件并保留比较范围；原生复验待构建。

过滤空结果实现后原生验收：Windows release通过，关闭旧自用Plico11452后启动
45580/2099310（仍仅一个自用实例）。同一比较页上区输入上述不存在主题，显示
中文空结果和重置链接；点击链接后输入清空，上区恢复两条及2/2，下区保持feature
及1/1。截图compare-plico-empty-{filter,reset}.png。本轮未测试下区选中提交保持；
过滤重置隔离已有桌面状态测试，不把截图扩大为未操作过的选择保留验收。
保留视觉差异：当前通用link样式为下划线/浅色，参考为蓝色；无附加条件的空范围、
详情面板结构、图投影、引用刷新和整体文件差异仍需补齐，不宣称第5项完成。
完整核心747通过（/tmp/plico-compare-full-core.log），桌面89通过/2按既有配置忽略
（/tmp/plico-compare-full-desktop.log），另跑导航8通过（/tmp/plico-compare-navigation.log）。
最后ghost改link为纯展示调整，Windows重建与原生点击通过；格式检查通过。

第5项引用刷新前置：Rebased44688上区保留topic文本筛选，对自有reference仓库
conflict-topic用commit-tree创建同树子提交并update-ref（babee4d→2d4139d），
未切换分支、未改变工作区/index。原生比较页自动出现topic comparison refresh probe，
输入仍topic，下区仍原有两条。截图compare-reference-ref-refresh.png。
实读4aec352 GitCompareBranchesUi.kt::MyLogUiFactory（创建独立VisiblePackRefresher）
与VisiblePackRefresherImpl.java::setDataPack/setValid/refresh：数据包变化重算可见范围，
保留过滤状态；不存在打开页时永久固定范围OID的规则。
本地View拥有引用解析任务与取消令牌，仓库既有refresh_impl通知比较页；重新解析
原始引用名后更新两侧固定查询，保留各自已应用条件。手动刷新强制重查，引用未变
的自动通知不重载普通查询；哈希查询还需刷新其他引用根。关闭/切库取消View请求。

引用刷新原生复验：关闭旧自用45580，Windows新构建26776/49484496打开比较页，
上区应用topic筛选显示1/1。自有Plico仓库conflict-topic通过同树commit-tree/
update-ref由2850819变为32e2693；未手动刷新，上区自动出现probe并变为2/2，
输入仍topic，下区feature与1/1不变。截图compare-plico-ref-{before,after}.png。
未修改fixture工作区/index。仅保留此自用Plico及参考Rebased44688。
定向桌面2项通过（/tmp/plico-compare-refresh-desktop.log）：真实临时仓库引用推进，
已应用条件/未提交输入草稿隔离、连续刷新旧令牌取消、引用删除后失效结果清理及
引用恢复后重载；Windows release通过（/tmp/plico-compare-refresh-windows.log）。
本轮未重跑完整桌面套件；刷新选择/视口保持仍需对照并补齐，不能视作完整刷新验收。
继续源码阅读GitCompareWithBranchAction.kt发现其入口先saveAllDocuments，当前本地
比较入口尚未接保存屏障，列为第5项待补；GitCompareWithBranchPopup.kt为独立选取
比较分支的树（排除当前本地分支、优先选择、搜索），后续入口对齐需先原生操作。

比较刷新选择前置：Rebased44688选中上区topic conflict（babee4d），为自有
conflict-topic再追加同树提交9496ed0。自动刷新后新提交在顶部，babee4d仍选中，
右侧shared.txt及topic conflict详情保留。截图compare-reference-selection-refresh.png。
重读4aec352 SelectionSnapshot.kt全文与VcsLogGraphTable.java::updateDataPack：
发布新模型前捕获选择/视口，按提交身份恢复仍可见的选择；顶部随新增提交保持顶部。
本地复用log_viewport::restore，在新页发布前保留旧行和详情，引用刷新替换同一
有界窗口，已选OID仍在时保留，否则取消详情请求并清空。过滤替换继续清空选择。

刷新选择实现后原生验收：Windows新构建37240/5768742，旧自用26776正常关闭。
上区选中topic conflict，独立fixture同树追加a562637（前32e2693），自动刷新后
新增提交在顶部，topic conflict仍选中，右侧shared.txt与详情保持，下区未变。
截图compare-plico-selection-{before,after}.png。刷新期间保留旧列表，发布时按OID
恢复选择；同一有界窗口保留已加载规模，不恢复到默认首批。复用主Log视口机制，
偏离参考之处是非顶部锚定首个可见提交而非优先可见选择；内容滚动不做自动化验证。
真实Git桌面回归增加选中→新增提交保持、引用重写导致选择消失清理详情，再覆盖
引用删除/恢复。完整桌面90通过/2既有导航配置忽略（/tmp/plico-compare-selection-all-desktop.log）；
本轮未再运行导航专用配置。Windows构建及格式检查通过。第5项仍未提交，整体差异、
入口保存屏障、任意两分支选择、图投影及剩余UI仍待补齐。

第5项整体差异入口前置（2026-10-25）：原生Rebased44688打开conflict-topic分支
子菜单，Compare with current与Show Diff with Working Tree为并列独立入口。点击
后不是比较Tab内的Files子页，而是左侧Changes工具窗口，标题Changes Between
conflict-topic and Current Working Tree；包含feature.txt、local.txt、main.txt、
shared.txt四项。local.txt是当前独立fixture未提交修改，证明其比较的是工作区。
顶部说明当前工作树分支及目标分支，Swap branches链接改变方向，新增/删除颜色
随方向改变，工作树标签与文件树保留；双击Diff及Get from branch尚未本轮实测。
截图compare-reference-{branch-options,submenu,working-tree-result,working-tree-swap}.png。

实读4aec352 GitBranchesUIHandler.kt::compare、GitShowDiffWithBranchPanel.kt全文、
GitBranchWorker.java::loadTotalDiff、GitChangeUtils.java::getDiffWithWorkingDir/
getDiffOutput、CompareBranchesDiffPanel.java::swapSides/refreshView/updateLabelText/
OnSwapSidesTreeStateStrategy与GitLocalCommitCompareInfo.java::copyChangesFromBranch。
现代分支比较使用双Log编辑Tab；工作区差异加载前saveAllDocuments，后台读取，
关闭disposable后不接收结果。默认工作区在左、目标分支在右（diff -R），-M检测
重命名，未跟踪文件不在该Git diff范围；交换是在已加载Changes端点上反转且保留
文件选择，并持久化方向。Get from branch单独确认/保存/写操作，不是Swap的副作用。
旧CompareBranchesDialog.java含Log/Files标签但现代GitBranchesUIHandler不调用它，
不能拿旧Dialog给当前比较页增加无实机依据的布局。

计划第5项范围不缩减：任意两个分支的纯树差异继续保留，需明确独立入口与标签；
另对齐已证实的分支对工作区Changes面板，不能用纯树差异替代工作区比较，也不能
把未提交修改放入声称两个分支版本的结果。现有CompareView仅接受两端完整OID，
combined_changes::Revision只读Git blob；不能用空字符串/HEAD占位代表本地文件。
下一步在既有Diff目标中显式建模工作区端点并复用Changes面板/保存屏障，先覆盖
增删改名、交换方向、已暂存与未暂存、未跟踪排除、取消/错误，再接原生界面。
本轮新增为已操作的入口/方向证据与实现依据，未声称整体文件差异已实现。

第5项工作区比较数据层：新增working_comparison.rs，Endpoint显式区分Revision
（固定commit/path）与WorkingTree（path），缺失侧使用None。load使用完整OID的
name-status/-z/-M，不写index，不包括未跟踪文件；默认将端点排列为工作区→分支。
Change.reversed只交换端点；按需read读取固定Git blob或复用Project.read_bytes的
有界磁盘读取/取消/读取中变化检查，不缓存本地内容，也不以HEAD冒充本地端点。
复用git::parse_names，未另建Git输出解析器。核心真实仓库测试验证暂存后继续编辑
读取磁盘最终内容、增删改名、方向交换、index字节与HEAD不变、未跟踪排除、文件
后续编辑实时读取、磁盘删除报错、二进制文本预览拒绝、非法OID及取消。
branch_compare集成5项通过（/tmp/plico-working-comparison-core.log）。
尚未连接DiffTarget/Chain与界面，不宣称工作区比较可见功能已交付。当前文本内容
读取不支持二进制、gitlink或符号链接；遇到unmerged状态会返回错误，这些边界仍需
参考现有Diff支持与Rebased进一步完成，不能把数据层临时限制当最终功能范围。

工作区Diff接入前置：Rebased44688在已交换方向的Changes面板双击local.txt，打开
Changes: local.txt编辑Tab；左侧固定9496ed0e内容keep，右侧Current version内容
unstaged work，文件导航2/4且左侧保持local.txt选中。截图
compare-reference-working-tree-diff.png。实读4aec352 ChangeDiffRequestProducer.java
的CurrentContentRevision身份比较及createContent/getRevisionTitle/MASTER_SIDE片段，
ChangeDiffRequestChain.java的ListSelection/getRequests/createGoToChangeAction：
每项包含明确两端来源，导航在同一变化列表，工作区侧并不必然位于右边。

本地DiffTarget.Identity和ChainData新增独立working_comparison端点元数据，避免
伪造Commit/HEAD；read_contents统一分派原有combined和新工作区端点，content_paths
提供各侧真实路径，DiffTab复用from_sources及标题映射。源文件跳转按本地端点路径
而非交换后的右端路径，避免重命名反向比较跳到已不存在的旧名。Chain导航保留
固定端点方向；反向目标身份不同，不能复用同方向游标。暂不经HEAD编辑加载路径。
核心回归增加导航step/fork、反向身份拒配、重命名source_path以及按需内容读取；
旧read_combined调用和对应测试统一迁移到read_contents。左侧入口/编辑能力与
Windows实际Plico界面仍待接入验收，不把只读数据连通宣称为整体功能完成。

Diff端点接入回归结果：完整核心748通过（/tmp/plico-working-target-all-core.log），
桌面90通过/2按既有配置忽略（/tmp/plico-working-target-all-desktop.log），导航专用
配置8通过（/tmp/plico-working-target-navigation.log）。cargo check及格式检查通过。
本轮没有启动额外Plico或重建Windows；当前原生Plico37240仍是上一轮选择保持构建，
新工作区端点尚未有用户可操作入口，Windows功能验收不能沿用旧构建截图。

第5项工作区比较入口接入（2026-10-25）：沿用上述Rebased原生入口、Changes面板及
工作区Diff三组截图；重读4aec352 GitShowDiffWithBranchPanel.kt的saveAllDocuments、
后台读取与disposed检查，以及actions/branch/GitCompareWithBranchAction.kt的保存顺序。
CompareView新增明确的工作区来源，刷新重新解析目标分支OID，复用working_comparison
与Chain；两端提交比较仍保留。提取Review.show_comparison共用现有Changes预览、
选择同步、跨文件导航和关闭生命周期。顶部分支菜单与Log分支菜单增加独立“与工作树
比较差异”入口，并提供中英文标题。读取前等待现有文档保存屏障；刷新按钮和错误
重试均发事件交由Review保存后读取，旧面板被替换则不再响应迟到的保存回调。
双Log比较也补上待保存文档屏障。引用解析提取为branch_compare::resolve_reference，
仍只接纳HEAD/完整本地或远端分支引用，不引入任意revision表达式。
新增桌面真实仓库回归覆盖工作区差异列表、导航目标路径、刷新保留选择、连续刷新取消、
分支删除后的错误/清理与重建恢复。完整桌面91通过/2按既有配置忽略。
方向交换说明/持久化、工作区侧编辑、特殊文件类型及剩余树控件仍待补齐；此次接入
不代表第5项完成。原生新构建与导航专项结果另行记录。

本轮入口原生验收：Windows release构建通过（/tmp/plico-working-panel-windows.log），
只正常关闭旧自有Plico37240并替换构建；新44748/1247494，参考Rebased44688保留。
顶部分支conflict-topic子菜单点击“与工作树比较差异”，左侧Changes显示feature.txt、
local.txt、shared.txt，Windows Git同一fixture diff名称列表也为这三项（参考fixture
当前分支不同，有额外main.txt，不能直接把两者文件数量当布局差异）。双击local.txt，
左侧工作区unstaged work，右侧a562637c keep，导航2/3。点下一文件变3/3 shared.txt，
复用同一预览Tab且左侧选择同步。截图working-panel-{menu,result,diff,next}.png已查看并
复制到artifacts/collaboration-workflows-20261024。当前确认为只读预览；工作区标题仍有锁、
Tab前缀仍“仓库差异”，需随编辑能力和展示细化继续对齐，未宣称完整像素还原。
导航专项8通过（/tmp/plico-working-panel-navigation.log），核心分支比较5通过
（/tmp/plico-working-panel-core.log），格式及diff空白检查通过。

第5项方向交换（2026-10-25）：前置沿用compare-reference-working-tree-{result,swap,diff}
原生截图与操作记录。重读4aec352 CompareBranchesDiffPanel.java::swapSides/refreshView/
updateLabelText/OnSwapSidesTreeStateStrategy：读取后仅交换Revision端点，不重新查询；
交换后的变更用于恢复树选择，说明文字随方向改变，读取时禁用交换。GitVcsSettings.java
（plugins/git4idea/shared）使用项目WORKSPACE_FILE保存该偏好。
本地Chain.reversed_working_comparison保持固定端点及文件游标；CompareView交换后
重建树并发Reloaded替换既有Diff预览，展开选中文件的新路径祖先；刷新重新查询后
应用保存的方向。新增方向说明/交换入口及中英文文案，保存偏好复用带项目身份校验
的本地布尔设置，未写入版本库或Git配置。选中重命名项时按原变更索引交换，不靠
交换前后可能不同的展示路径猜测身份。核心补充双次交换身份/游标测试，桌面补充
选中项交换、重复交换恢复及刷新后的方向保持；91桌面测试通过、2按专用配置验证。
说明与链接排版仍有差异，原生新构建核对与剩余项另记。

方向交换本轮验收：核心完整748通过（/tmp/plico-compare-swap-all-core.log），桌面91
通过/2按既有配置另跑，导航专项8通过；Windows release重建通过。正常关闭旧自有
44748后，新Plico21760/29627122在同一fixture双击local.txt并交换方向：同一Tab保持
2/3与左侧local.txt选择，左侧a562637c keep、右侧工作区unstaged work；刷新后方向与
选择不变。截图working-swap-{initial,result,refresh}.png已查看并复制到上述artifacts。
说明文字位于文件树上方；交换链接目前单独居中一行，与参考行内链接及分支加粗有差异，
文件颜色、工作区编辑、锁图标与Tab前缀仍待下一步处理。仅保留自有Plico21760和
Rebased44688；本轮未自动化滚动，未新增或修改fixture提交。格式及diff空白检查通过。

第5项比较说明和文件颜色细化（2026-10-25）：重新查看此前原生working-tree-swap截图，
实读4aec352 ChangesBrowserChangeNode.java::render（shared目录）按change.getFileStatus
着色、CompareBranchesDiffPanel.updateLabelText使用加粗分支与行内链接、DvcsBranchUtil
.swapRevisions单侧缺失时重建状态。CompareView复用ui_style::change_color；交换后新增/
删除颜色随端点改变。说明使用StyledText+InteractiveText，分支加粗、行内蓝色交换链接
随文字自然换行；加载/错误时不注册可点击范围。只扫描原始翻译模板做替换，避免真实
分支名中的花括号被误作为另一个占位符。旧的整行居中按钮已移除。
cargo check、Windows release构建通过。只关闭自有Plico21760，替换后22000/1706850
打开同一比较；项目已保存方向在重启后恢复，feature.txt绿色、修改文件蓝色。点击
行内交换链接后方向文字改变，feature.txt灰色，选中项保持。原生截图working-inline-
{initial,swapped}.png已查看并复制。说明/颜色这次未重跑不相关核心测试，未宣称完整
工具栏/文件树已全部还原；此前根节点/计数/分组/工具栏等差异仍在。

工作区编辑前置实机验证：Rebased44688/29167666，仍为独立reference fixture，先在
右侧Current version输入EDIT_PROBE、截图compare-reference-edit-right.png，再撤销；
交换方向后在左侧Current version输入LEFT_PROBE，截图compare-reference-edit-left.png，
固定9496ed0e侧keep未改且锁图标跟随提交侧，中间采纳箭头反向。撤销并保存后磁盘
local.txt读取为精确b'unstaged work\\n'，测试内容已恢复。start/right/left截图均保存。
本地EditableDiff.new/recompare、working_copy.read_base审查确认现有编辑路径将工作
文档绑定modified侧、基线默认HEAD/index；不能直接启用working_tree_document绕过限制。
下一步需将编辑端点、固定基线、显示方向及采纳方向一起接入；本轮未宣称编辑已实现。

第5项编辑端点读取（2026-10-25）：沿用上一轮已操作的Rebased左/右Current version
编辑与撤销截图。继续实读4aec352 ChangeDiffRequestProducer.java::createContent/
createRequest：CurrentContentRevision经VirtualFile创建共享内容，固定版本走字节内容，
MASTER_SIDE选择工作区实际所在侧；SimpleDiffViewer.isEditable/ApplySelectedChangesActionBase
按目标侧判断编辑资格，替换操作只写目标编辑器Document。
本地working_comparison新增WorkingSide与EditableContents，包含真实工作文件路径、
显示侧与既有working_copy::Contents；read_editable要求恰好一个工作区端点，固定
revision/path读取基线，Project读取磁盘并保留Document原始字节，不从HEAD/index
推断比较基线。不允许将缺失的本地端点自动创建为可写文件；双工作区/无工作区端点
拒绝。只提取共用端点文本读取，没有改变原有只读Diff入口或提前开启编辑。
新增真实仓库测试：目标提交固定后HEAD前移并重命名，index与工作区内容各不同；
两个方向都读取旧提交base和新路径磁盘内容；BOM/CRLF保留，未编辑begin_save无操作，
编辑后Save.expected仍是磁盘原字节，Save.bytes保留编码/换行。验证仅本地新增、删除
缺失、双本地端点、越界路径、二进制、磁盘删除及取消，并确认读取不写HEAD/index/
工作文件。分支比较集成6通过（/tmp/plico-comparison-editable-core.log）。
本轮是编辑数据准备，尚未连接EditableDiff左右侧渲染、统一模式、采纳/保存、文件事件
及标题锁图标，不能视为工作区编辑功能交付。Windows现有22000仍是上一轮UI构建。

第5项左右工作区编辑继续（2026-10-25）：EditableDiff 将显示侧与工作文档身份分开，
双栏左侧工作区也绑定共享 Document，固定侧保持只读，采纳箭头方向随工作区改变。
实际工作路径取 source_path，避免交换重命名方向后将固定版本旧路径当作保存路径。
新增左侧编辑/采纳/撤销/旧结果拒绝/布局恢复测试；上一轮桌面92通过、导航专项8通过，
见 /tmp/plico-comparison-oriented-all-desktop.log 和 oriented-navigation.log。
目前加载入口仍未启用该能力，避免统一模式和文件事件尚未衔接时暴露半成品。

统一模式前置：自有 Rebased44688/29167666（1.1.20，参考源码仍4aec352）比较相同
local.txt，工作区在左侧。切统一模式，灰色旧行 unstaged work 可追加 UNIFIED_PROBE，
绿色固定9496ed0e行 keep 不变；撤销后，在绿色行粘贴 FIXED_PROBE 被拒绝，内容未变。
磁盘仍为 b'unstaged work\n'。截图 compare-reference-unified-left{,-edit,-readonly}.png
保存于 artifacts/collaboration-workflows-20261024。布局仍旧行在前、新行在后，
不能为了编辑绑定而倒置统一展示。未测试该参考模式下所有输入法组合状态。
实读 platform/diff-impl/src/com/intellij/diff/tools/fragmented/UnifiedDiffViewer.java：
构造器根据唯一可编辑侧设置 myMasterSide；installEditorListeners 将 Undo 关联 master
文档；应用结果时按 masterSide 选择另一侧 change range 建立 guarded block，并保护
fake newline。对应本地 UnifiedEditProjection 增加显式工作侧，保留旧→新显示顺序，
固定片段与用于换行的虚拟字节不得映射到工作文件。UnifiedEditing 调用层及最终加载
接入仍待完成，不能视为工作区比较编辑功能交付。
统一映射本轮验证：unified_edit_projection 8项全部通过，覆盖左右显示方向、固定片段
输入拒绝、中文/emoji字节映射、空文件、末尾无换行、编辑后只读区域保持和源位置往返；
日志 /tmp/plico-left-projection-tests.log。cargo check --locked 通过，cargo fmt --package
plico 与 git diff --check 通过。本轮仅修改映射基础设施，尚未构建新的 Windows Plico，
原生 Plico 编辑验收待调用层接入后进行；不把参考程序截图计作本地功能验收。

统一编辑器接入（2026-10-25）：继续依据上节 Rebased 原生左右工作区与统一模式截图、
UnifiedDiffViewer 的 masterSide/guarded block/共享Undo规则。UnifiedEditing 构造器显式
接收工作侧，刷新校验对应源文本，按显示侧设置高亮，但将编辑和Undo转到工作文档；
从固定行跳源码时返回固定侧，从工作行返回实际工作侧。EditableDiff 创建统一视图、
清除工作区布局间隙、F7定位与延后定位改为选择工作区实际侧，显示仍旧→新。
新增左侧输入、固定行输入拒绝、共享Undo、两侧源位置及外部文本变更后拒绝旧映射
用例；原双栏左工作区用例增加切换统一模式的展示内容核对。初次测试编译缺少测试
模块 DiffSide 导入，已补齐后重跑。桌面93通过、2按专用配置另跑，导航专项8通过，
日志 /tmp/plico-unified-side-{desktop,navigation}.log。git diff --check通过。
加载入口仍保持原有限制：需一并接通固定比较基线刷新、真实磁盘路径、标题锁图标
和部分提交资格后才开启工作区比较编辑。尚未新增Windows Plico构建或原生验收，
不宣称第5项已交付；现有只读比较入口行为不变。

第5项工作区加载接通（2026-10-25）：沿用已验证的 Rebased 左/右 Current version
及统一模式原生证据，ChangeDiffRequestProducer 固定revision内容/共享本地Document、
SimpleDiffViewer 目标侧编辑资格和 UnifiedDiffViewer masterSide 规则。
Change::working_path/read_baseline 统一验证恰好一个工作端点；working_copy::read 对
显式比较读取固定基线与真实磁盘文档，read_base 即使磁盘文件删除也只读固定提交。
Target::working_tree_document 暴露实际工作路径，Gitlink检测也改用实际路径，避免
交换重命名方向后误查旧路径。DiffTab 初次加载按工作侧排列源文档；只读显式比较的
文件事件走端点读取，事件匹配取实际工作路径，标题锁图标按editable_side显示。
分块提交作用域仅用于原本HEAD/工作树比较，不让任意分支基线进入提交选择模型。
真实仓库验证两个方向经working_copy入口读取，HEAD/index/disk三个版本不同仍得到
选定提交，删除磁盘文件后read_base成功而完整可写文档读取失败。核心分支比较6通过。
新增DiffTab真实异步loader测试验证两个方向均生成可编辑Contents且源顺序正确；首次
测试误将Chain工厂元组当Chain使用，修正后桌面94通过、2按专用配置另跑。
本轮日志 /tmp/plico-working-loader-{core,desktop}.log；原生验收另记。
本轮导航专项9通过（包含新增loader用例）；Windows release构建成功。
正常关闭自有22000后启动新48844/4522358，保留Rebased44688。原生同一fixture，
local.txt左侧工作区输入LEFT_SAVE_PROBE并Ctrl+S，磁盘精确读取为
b'unstaged work LEFT_SAVE_PROBE\n'；撤销保存后交换方向，锁图标到左边、采纳箭头向右，
右侧输入RIGHT_SAVE_PROBE保存亦只写同一文件。再次撤销保存、交换回左工作区，统一
模式第一行输入UNIFIED_SAVE_PROBE保存，磁盘仅工作行更新，不含固定keep行；撤销保存后
第二行keep粘贴FIXED_PROBE被拒绝，最终磁盘精确恢复b'unstaged work\n'。
截图working-edit-{loader,left-start,left-saved,swapped,unified-left,unified-saved,
unified-readonly}.png保存于既有artifacts目录。未新增测试实例堆积或自动化滚动。
发现剩余展示差异：统一模式仍将替换块两行都画蓝色且只有统一行号，Rebased是旧行灰/
新行绿与两侧行号；当前Tab前缀“仓库差异”也待改为Changes语义。上述差异尚未修复，
本轮仅证明工作文档与固定版本隔离、编辑保存及左右方向，不宣称像素级完成。

统一Diff着色修正（2026-10-25）：前置原生对照沿用上一轮已查看的
compare-reference-unified-left-readonly.png 与 working-edit-unified-readonly.png：参考旧行灰/
新行绿，本地误将替换块两行都画蓝色。实读4aec352 UnifiedDiffChangeUi.doInstallHighlighters
委托DiffDrawUtil.createUnifiedChunkHighlighters；后者无论是否同时包含插入/删除，都按
DELETED/INSERTED给对应行及inner fragment着色。双栏替换规则不能直接用于统一编辑器。
本地DiffFile保留双栏editor_change_highlights接口，新增unified_editor_highlights，
共用范围和词级inner计算、仅区分颜色语义；UnifiedEditing改用统一规则。定向测试
验证替换块两侧的行底色/inner颜色及双栏仍为蓝色，9项projection测试全部通过。
源码路径platform/diff-impl/src/com/intellij/diff/{tools/fragmented/UnifiedDiffChangeUi.java,
util/DiffDrawUtil.java}。双侧行号仍待实现：已读UnifiedDiffViewer安装两套converter，
UnifiedFragmentBuilder按旧→新顺序构建两侧映射，LineNumberConvertor.Corrector区分
同步/异步编辑并立即修正受影响区间；不能只保存重新比较时的静态行号。
着色原生验收：Windows构建通过，正常关闭自有48844后新41028/15863152打开同一fixture
local.txt；统一模式第一行unstaged work灰色、第二行keep绿色，固定提交锁图标保持。
截图working-unified-colors.png已查看并保存。旧/新颜色分类已修正，精确色深仍需连同
inner高亮选项核对，不宣称全部像素一致。双侧行号、统一模式采纳图标仍未接通。
另读取LineNumberConvertorCorrectorTest.kt的相同行区间增/减行、等长替换、多次修改和
修改区间内部再次编辑用例，后续行号实现需覆盖这些即时修正行为。

统一Diff双侧行号（2026-10-25）：沿用前置已查看的Rebased统一模式截图与编辑/撤销
场景，重读UnifiedDiffViewer.MyOnesideDocumentListener：以编辑首行至末行+1为区间，
工作侧同步修正，固定侧区间失去精确映射，之后重新比较。LineNumberConvertorCorrectorTest
覆盖多次与区间内部修改。UnifiedEditProjection现在记录紧凑双侧行号（每行8字节），
apply_edit同步替换受影响区间并移动后续工作侧行号，固定侧后续行号不变；渲染只查询
可见行，不每帧扫描整篇源文件。UnifiedEditing关闭投影自身顺序行号，使用两列源码
行号；遵循line_numbers显示偏好。10项projection测试通过，新增左右工作侧连续插行、
等长替换、固定映射失效/后续保持测试。桌面94通过、导航专项9通过。
补充Rebased原生插行：44688/29167666在工作行尾Enter并输入LINE_PROBE，左行号1/2、
固定keep仍右1、末尾共同空行左3右2。截图reference-unified-insert-line.png含输入法候选
窗口，因此不作为精确颜色样本；撤销并保存后磁盘精确恢复b'unstaged work\n'。
首次原生32236/3607394发现行号空白（working-unified-lines-before-order-fix.png），原因
为gutter在editor前prepaint，读取到尚未建立的source_geometry。改为DOM/绘制顺序先
editor后gutter，并使用flex_row_reverse保持gutter视觉上仍在左侧，与现有双栏右侧
外置gutter使用编辑器本帧几何的原则一致。编译通过；修正后原生验收另记。
绘制顺序修正后的Windows构建与原生30424/21892266验收通过，正常替换自有32236。
初始两行分别显示左1/右1，末尾空行2/2；工作行末粘贴换行+LINE_PROBE后左1/2、
固定keep右1、末尾3/2，与参考插行结果一致。截图working-unified-lines-{fixed,insert}.png
已查看并保存；撤销保存后磁盘精确恢复b'unstaged work\n'。未测试自动滚动。
最终桌面94通过、导航专项9通过（/tmp/plico-unified-lines-final-{desktop,navigation}.log）。
尚余统一模式采纳操作栏、gutter背景贯通和精确间距等差异；本轮只验收行号映射和
同帧布局，分支比较整个第5项仍在进行中。

统一模式采纳入口前置（2026-10-25）：自有Rebased44688/29167666，工作区在左的
local.txt，点击差异块首行勾号，工作文档变为固定版本keep，显示Contents are identical；
撤销保存后交换左右，首行图标变为回退箭头，点击并保存磁盘为b'keep\n'，再次撤销
保存精确恢复b'unstaged work\n'。截图reference-unified-accept-{left,right-start}.png。
实读4aec352 UnifiedDiffChangeUi.createOperation/getApplyIcon/createAcceptOperation：
图标位于block首行；源侧LEFT使用Diff.Revert，源侧RIGHT使用Actions.Checked；过期
比较或目标不可写时不提供操作，执行为目标文档WriteCommand并立即请求rediff。
本地UnifiedEditing缓存每个变更块在统一投影的起始行；EditableDiff增加独立24px操作
列，复用已有checked/rollback资源和revert_change的generation/当前文本/写锁保护。
只在比较与工作文本一致时建立命中区；点击后焦点回到可见统一编辑器，Undo仍转到
共享工作文档。行号隐藏偏好不隐藏采纳操作。原生本地验收与测试结果另记。

### Worktrees 操作栏与标题间距修正（2026-10-25）

本轮先操作自有 Rebased 1.1.20 / IC-262.10968.SNAPSHOT（44688），Find Action →
Worktrees，截图 `artifacts/collaboration-workflows-20261024/worktree-spacing-reference.png`。
操作栏在底部 Git 面板内部独立竖排，图标不贴容器边缘；Git 标题与内容页签分区。
本轮读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的
`plugins/git4idea/backend/src/workingTrees/ui/GitWorktreesTabPanel.kt`：工具栏为独立
VERTICAL action toolbar，置于 worktreeListPanel 左侧；读取
`platform/platform-impl/src/com/intellij/openapi/wm/impl/content/TabContentLayout.java`
的 getMinimumWidth，标题按首选宽度及 Insets 计入最小布局，不被当作可压缩列表内容。
源码快照与安装版本不同，未将截图测量值声称为上游固定常量。

Plico 修改前截图 `worktree-spacing-before.png`，同目录。`src/ui/worktrees.rs`：
操作列宽度 36 → 44 逻辑像素，增加左右各 4px 内边距；Git 与 Worktrees 间距
16 → 24px，标题和标签禁止 flex 收缩。保留已有按钮尺寸、顺序和点击行为。
这是本轮按用户反馈的间距细化；未添加参考中独立 Console 页签及帮助按钮，
不宣称整个工具窗口已逐像素一致。Windows 构建与修改后截图待下述验收。

本轮验收：Windows release 构建通过（53.68s，`/tmp/plico-worktree-spacing-build.log`），
fmt 检查及 diff whitespace 检查通过。仅正常关闭自有 Plico 30424，再启动 43608，
点击左下 Git 打开 Worktrees；修改后截图 `worktree-spacing-after.png`，缩窄窗口截图
`worktree-spacing-narrow.png`（均在上述 artifacts 目录），已查看确认操作列留白、
标题与页签间距正常，缩窄后未压缩重叠。纯布局修改未新增镜像测试，未运行 CI。

统一模式采纳与撤销验收（2026-10-25）：补上测试专用 debug_selector 后，鼠标点击
回归暴露真实问题：过期映射期间将整个投影编辑器设为 readonly，阻断立即 Undo。
再次读取同参考 UnifiedDiffViewer.java 的 ORIGINAL_DOCUMENT Undo 关联与
isEditable/getDocument：撤销属于 master document；本地保留写锁 readonly，
过期映射由 UnifiedEditing 的 edit_guard（有效标记、源文本一致性、可编辑区间及写锁）
拒绝输入，而不阻断共享文档撤销。新增测试实际鼠标采纳、固定侧不变、可见编辑器
焦点、过期输入拒绝及直接 Ctrl+Z；不是通过等待 rediff 掩盖此时序问题。

完整桌面 95 通过、2 既有忽略（/tmp/plico-unified-accept-full.log），导航专项 9 通过
（/tmp/plico-unified-accept-navigation.log）。Windows release 构建通过，替换并关闭
自有 43608 后启动 32552 / HWND4131146。独立 fixture 的 local.txt，工作区在左点击
勾号并 Ctrl+S，磁盘精确为 b'keep\n'；不另点编辑器，Ctrl+Z/Ctrl+S 精确恢复
b'unstaged work\n'。交换方向后显示 rollback 图标，重复采纳、保存、撤销恢复成功。
截图 artifacts/collaboration-workflows-20261024/unified-accept-{left-result,right-start}.png
已查看。仅保留一个自有 Plico 与一个 Rebased。CI 未运行；行号/操作列背景连续高亮、
悬停提示及 Contents are identical 横幅尚有视觉差异，分支比较整体仍未完成或提交。

相同内容提示前置与实现（2026-10-25）：再次操作自有 Rebased44688 的 unified local.txt，
工作侧在右，点击 rollback；截图 `reference-unified-identical.png` 已查看，信息条位于
工具栏与版本标题之间，蓝色信息背景，右侧 Hide。之后 Ctrl+Z/Ctrl+S 恢复 fixture。
同参考 commit 实读 UnifiedDiffViewer.java::apply、TextDiffViewerUtil.java::
createEqualContentsNotification/areEqualCharsets/areEqualLineSeparators、
platform/diff-impl/src/com/intellij/diff/tools/util/DiffNotifications.kt。
上游在完整比较相同时按 charset/BOM/换行符生成不同消息；隐藏仅隐藏当前 notification。

本地 DiffTab 将可编辑 Diff 的通知提升到版本标题之上，提供 Hide；新比较结果恢复提示，
切文件重置隐藏状态。EditableDiff::equality_notice 要求缓存结果与当前工作文本一致，
过期结果立即不显示；区分文本相同与仅换行符不同，统一/双栏共享此位置。
当前 Contents 的基线读取已剥离 BOM，不能可靠判断编码标记差异，因此临时使用明确的
“Text contents are identical / 文本内容相同”，不冒充字节一致；完整编码属性提示仍待补齐。
信息图标和只读历史 Diff 的同类通知也尚未纳入本次，保持差异记录。

本轮验收：完整桌面96通过、2既有忽略，导航专项9通过；补充基线一致性保护后
定向 equality_notice 回归通过。Windows最终release构建通过（日志
/tmp/plico-equality-windows-final.log）。自有27340/4131682：采纳全部差异后提示条
位于工具栏与版本标题之间，点击右侧收起后内容上移，撤销恢复差异后无旧提示；
截图 plico-unified-identical{,-hidden,-undo}.png 已查看，保存于上述 artifacts 目录。
Ctrl+Z/Ctrl+S 后校验 fixture 精确恢复 b'unstaged work\n'。格式/空白检查通过，
未运行CI；前述编码属性、图标、历史只读通知差异仍待处理，未宣称本项整体完成。

合并冲突中的工作树比较（2026-10-25）：新增独立仓库
compare-conflict-reference-20261025，main/topic 同行冲突，另一个文件有普通工作区修改。
自有 Rebased1.1.20 25596；仅信任这个测试目录，取消 Defender 排除选项。
分支 topic → Show Diff with Working Tree，在合并进行中仍可打开，列表显示两个蓝色修改
文件；双击 conflict.txt 是工作区冲突标记文本与 topic 的普通双栏比较，不自动替换为
三方解决器，也不要求先解决冲突。截图 compare-conflict-reference-{list,diff}.png
已查看并保存于 artifacts/collaboration-workflows-20261024。
实读4aec352 GitChangeUtils.java::getDiffWithWorkingDir/getDiffWithWorkingTree/parseChanges：
U 记录具有前后内容；不过本次真实 Git diff <OID> 输出两个 M，不能把 load 中的 U 拒绝
分支直接声称为已复现的全表失败。原先“整个列表遇到冲突一定失败”的推测被本轮证据否定。

新增 tests/branch_compare.rs::working_comparison_during_merge_preserves_conflict_stages_and_other_files：
真实合并产生冲突，双向比较读取固定 topic 内容与完整冲突标记，普通修改仍可读；检查
全部 index stages、HEAD、MERGE_HEAD 和冲突文件字节未变化。7项 branch_compare 集成测试
通过（/tmp/plico-working-conflict-tests.log）。本轮不添加无证据的全仓冲突门禁、不更改
查询语义。U 分支的极端输入尚未原生复现，保留待核对而不冒称已修复。临时参考窗口已请求
正常关闭，原有参考窗口保留。本轮只新增测试和记录，无产品二进制变化。
退出补记：原生 Confirm Exit 点击 Exit（未勾选不再询问），等待自有25596退出；
不终止原来的44688参考窗口。

分支比较过滤图的数据层准备（2026-10-25）：本轮重新读取4aec352的
platform/vcs-log/graph/src/com/intellij/vcs/log/graph/impl/facade/FilteredController.kt
及 collapsing/DottedFilterEdgesGenerator.kt，规则为完整永久拓扑上的可见集合，
跨隐藏行以 dotted edge 连接，分页箭头不能由缺失父节点臆造；GitCompareBranchesHelper
明确两侧使用相反的 first..second 范围。

src/branch_compare.rs 新增 Query::graph_page，复用现有 filtered_graph::window，
以固定 included OID 的完整祖先拓扑为图，以 excluded..included 加详情过滤选可见 ID；
hash 命中继续使用 prepare 固定的 roots/IDs 并覆盖范围。原先尝试只取范围拓扑会被
真实Git测试报“parent history is missing”，现已修正为完整祖先输入。投影后按原始ID
读取 Commit 元数据，Commit.parents 不被虚线连接改写。分页先投影再切片，保留同一
lane/segment；限制仍为已有100,000节点/500,000边与geometry预算，不静默截断。
Repository 提取私有 bounded_topology_revisions，新增完整OID验证的范围入口；
路径查询使用 literal-pathspecs，避免方括号等文件名被当成通配符。

新增真实Git用例覆盖可见→隐藏→可见的虚线、原父提交、分页geometry一致、范围相反的
空结果、取消；扩展路径过滤的merge双亲和hash覆盖用例。8项分支比较集成通过。
本轮仅准备查询/投影数据层，尚未替换原生 BranchPane 的布局调用；原生分支比较过滤
截图对齐和UI接入仍是下一步，不能据此声称界面已修复。全核心回归正在运行，另记结果。

本轮全核心回归完成：751项通过（/tmp/plico-branch-projection-core.log），命令正常退出0；格式与diff空白检查通过。UI未接入，未新增Windows二进制或宣称原生验收完成。


分支比较过滤图 UI 接入与原生验收（2026-10-25）：
`Query::needs_graph_projection` 只在详情过滤或 hash 定位时启用完整拓扑投影；无过滤
仍使用既有范围分页。`src/ui/branch_compare.rs::Pane::load_more` 接收投影行并按
替换/追加窗口更新 graph，避免再次对过滤后的 Commit 数组直接布局。原始 parents
仍供 Diff 使用。刷新测试增加可见—隐藏—可见链，验证刷新保留过滤且 graph 含虚线。

原生参考沿用 Rebased 1.1.20（源码规则见上一节）。两份独立测试仓库均通过
commit-tree 创建 graph-projection-test 三提交链，未切换 HEAD 或修改工作区/index。
从分支菜单 Compare with current 打开比较，输入 projection-visible 并确认：上方
从三条变为两条，以虚线跨越隐藏提交；下方无独有提交。参考截图
`artifacts/collaboration-workflows-20261024/projection-reference-{compare,filtered}.png`。
正常关闭自有 Plico 27340，替换本轮 Windows release 后启动 21924，按同样步骤操作，
截图 `projection-plico-{compare,filtered}.png`（同目录）。已查看确认两条结果间虚线、
计数 1–2 / 2、另一侧过滤独立；未进行滚动自动化。

Windows 构建通过（/tmp/plico-projection-windows.log）；聚焦 UI 两项通过，全部桌面
回归 96 项通过、2 项忽略（/tmp/plico-projection-desktop.log）。本轮验证的是过滤图
接入，不代表整个比较界面完成：引用选择、文件区工具栏、空态布局和日期/引用展示等
仍需逐项对齐。只保留自有 Plico 21924 和原参考 Rebased 44688，没有新增常驻实例。


分支比较空态补齐（2026-10-25）：先在自有 Rebased 44688 的 graph-projection-test
比较页输入 no-such-projection-commit。上方显示 No commits matching filters 和 Reset
filters，下方未过滤的空范围显示 graph-projection-test contains all commits from
renamed-untracked；见 artifacts/collaboration-workflows-20261024/compare-reference-empty.png。
对应 4aec352 的 plugins/git4idea/backend/src/branch/GitCompareBranchesUi.kt：
MyFilterUi.setCustomEmptyText 仅当存在 range/root 以外过滤时委托通用空态，否则使用
range.start contains all commits from range.end。shared/resources/messages/GitBundle.properties
确认该文案。旧 CompareBranchesLogPanel 属于另一实现，不作为当前原生布局依据。

本地 src/ui/branch_compare.rs 将加载结束且无错误的空列表统一交给空态布局：有额外
过滤时保留重置入口，无过滤时按当前比较方向显示包含关系。src/i18n.rs 增加对应中文。
提示文本置于有左右内边距的全宽文本容器中；不把请求错误或加载中误报为已包含。
本轮只改已有空态展示，无新 Git 查询。Windows 修改后验收待补。

原生验收补记：Windows release 构建通过（/tmp/plico-compare-empty-build.log，66秒）。
正常关闭自有21924并启动新构建39748，同一测试分支完成空结果和点击重置验证。
截图 compare-plico-empty.png 同时显示上方过滤空态/重置与下方正确的包含方向；
compare-plico-reset.png 确认点击后恢复三条提交，另一侧仍保留包含提示。均位于上述
artifacts目录并已查看。本轮布局/文案改动未新增镜像测试，未重复全套回归；fmt和
空白检查通过。英文代码文案已核对资源，英文原生截图及极窄宽度长名称仍待后续验收。


分支比较提交元数据（2026-10-25）：原生 Rebased 44688 在同一测试比较页重置过滤，
选择首条 projection-visible last，右侧上方为空文件区，下方显示加粗等宽正文、短hash、
作者邮件及提交日期；截图 artifacts/collaboration-workflows-20261024/compare-reference-details.png。
读取4aec352的 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/details/commit/CommitDetailsPanel.kt：
messagePanel 与 hashAndAuthorPanel 分离，先正文再元数据，侧边距14；refs及包含分支另列。
本地 src/ui/branch_compare.rs 复用 plico::commit_presentation，后台详情请求一次生成
双语正文/元数据；不在 render 中重新解析，不增加 Git 子进程。选择变化和刷新清空
presentation，沿用详情generation拒绝迟到结果。正文使用已有等宽 TextView、元数据
采用界面字体，两者支持选择复制；语言切换无需重读Git。refs/包含分支、可拖动的详情
高度和文件区工具栏尚未接入，不宣称全部右侧布局已一致。修改后原生验收待补。

验收补记：两项分支比较 UI 回归通过（/tmp/plico-compare-metadata-tests.log），其中
刷新测试验证 presentation 含选中提交的正文/短hash，提交离开结果集时同步清空。
测试曾发现 presentation 未写回，已修复并重跑通过。Windows release 构建通过
（/tmp/plico-compare-metadata-build.log），fmt/空白检查通过。正常关闭自有39748，
启动32176并打开同一比较、选择首条提交，截图 compare-plico-details.png（同目录）
确认加粗等宽正文、短hash、作者邮件和本地日期显示。邮件链接颜色/下划线、详情可拖动
高度及未选择时的占位区仍有视觉差异，保留待对齐；本轮没有覆盖文件工具栏交互。


分支比较文件选择与键盘打开（2026-10-25）：先在 Rebased 44688 切回 conflict-topic
比较页、选择 topic conflict 后单击 shared.txt：文件行蓝色选中且不打开；按 Enter 后
打开 Repository Diff: shared.txt，内容 base → topic conflict。截图
artifacts/collaboration-workflows-20261024/compare-reference-file-{selected,enter}.png。
读取4aec352的 VcsLogChangesBrowser.kt 和 ChangesBrowserBase.java：ChangesBrowserTreeList
把双击/Enter 都交给 onDoubleClick，canShowDiff 检查当前选择，再使用选中的 producer
建立 Diff chain；ShowDiff action 复制通用Diff快捷键。本地沿用已有 compare_view 的
Enter/Ctrl+D 约定，在 branch_compare.rs 独立 BranchCompareFiles 焦点上下文中绑定。
单击保存 selected_file 并取得焦点，双击/快捷键打开同一索引；切换提交或清空详情时
同步清空文件选择，防止旧索引打开新提交中的其他文件。选中行增加圆角高亮。
本轮未实现树形分组、多选、上下键移动、工具栏及预览复用；这些仍列入本阶段后续。

验收：Windows release 构建通过（54.81秒，/tmp/plico-compare-files-build.log），两项
branch_compare UI回归通过（/tmp/plico-compare-files-tests.log），格式/空白检查通过。
正常关闭自有32176并启动12020；在 conflict-topic 比较页选择 topic conflict，单击
shared.txt 保留比较页且文件行选中，按Enter打开该文件Diff。截图
compare-plico-file-{selected,enter}.png（同目录）均已查看。两fixture的该提交父链
此前分别经历不同rebase，因此参考before为base、本地before为main conflict；本轮
验证选择/焦点/打开，不把不同测试历史的内容差异误判为本轮Diff行为偏差。
Ctrl+D和双击走同一已有handler，但本轮原生只验收Enter；多文件键盘导航仍待补。


分支比较文件树与选项入口（2026-10-25）：原生Rebased 44688在比较页打开眼睛菜单，
确认Group By/Directory、Layout/Show Details/Show Diff Preview层级及选中态；截图
artifacts/collaboration-workflows-20261024/compare-reference-grouping.png。读取4aec352的
VcsLogChangesBrowser.kt：使用统一ChangesBrowser及其toolbar；读取
platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ui/DirectoryChangesGroupingPolicy.kt：
按父路径缓存目录节点并保留真实change节点。路径压缩沿用此前已对照的本地file_tree。

本地新模块src/ui/branch_compare/files.rs负责显示模型和工具栏：compact_rows保留原
chain索引，目录含文件数，根节点可收起；平铺按文件名排序，目录路径紧跟名称以灰色
显示。切换分组不改变selected_file或重新查询Git；点击目录清空文件选择，避免Enter
打开已隐藏的文件。两pane独立group_files/show_details。工具栏有真实ShowDiff及眼睛
菜单的Directory/Show Details，尚未接入预览开关、全部展开/折叠、历史/回滚、快捷键
Ctrl+Alt+P、持久化及多选，不宣称工具栏完整。当前列表仍逐行构造，虚拟化待补。

新增GPUI测试覆盖多目录紧凑折叠、折叠后隐藏子项、平铺排序保持chain索引与选择、
清空详情同步清空rows。3项branch_compare测试通过（/tmp/plico-compare-tree-tests.log）；
Windows构建通过（/tmp/plico-compare-tree-build.log），仅正常关闭自有12020并启动46060。
原生截图compare-plico-{grouped,group-menu,flat}.png确认根文件数、菜单层级/勾选、点击
Directory切到平铺；均在同一artifacts目录。多层目录在本轮由测试覆盖，尚未原生截图。
ShowDetails隐藏动作及切换后键盘焦点恢复待下一轮验证。未增加常驻实例或提交。


分支比较文件树键盘及可见行构造（2026-10-25）：先在Rebased44688的比较文件区
选中shared.txt，Up选中根目录、Left收起；Right展开后Down重新选择文件。截图
artifacts/collaboration-workflows-20261024/compare-reference-tree-{left,right}.png。
读取4aec352的ChangesTree.java与treeStructure/Tree.java，ChangesTree继承平台Tree，
安装SmartExpander及分组支持；本轮导航规则以原生观测为准，不把搜索结果当具体
键位实现证据。Enter打开仍沿用上轮已读ChangesBrowserTreeList的处理器。
本地files.rs用文件chain索引/目录路径作为选择身份，上下选可见行、左收起或选父目录、
右展开或选首子行。branch_compare.rs使用uniform_list，只构造当前可见行；保留
完整轻量行模型以支持目录操作，不新增Git查询。切换提交清空选择及滚动位置。
测试扩展覆盖深目录文件回父节点、左收起/右展开、上下移动后Diff索引正确；3项通过
（/tmp/plico-compare-tree-keys-tests.log）。本轮不声称滚动性能已测量，原生滚动留给用户。

本轮原生验收补记：Windows release 构建通过（53.72s，
/tmp/plico-compare-tree-keys-build.log）。自有 Plico 46084 在已选 topic conflict
提交的文件区，Up/Left 选中并收起根节点，Right/Down 展开并选中 shared.txt，
Enter 打开对应提交的只读 Diff；确认两侧 revision 为 211d4091 / 28508190，
与所选提交父版本及提交版本一致。截图 compare-plico-tree-{left,right,enter}.png
已保存至 artifacts/collaboration-workflows-20261024/，与上文原生 Rebased
方向键行为一致。此处只验证键盘导航及打开 Diff，不代表多选、预览开关或完整
工具栏已完成；多层目录的父子选择仍由上述测试覆盖，未做原生滚动自动化。

分支比较内嵌预览：实现前原生与源码核对（2026-10-25）

在自有 Rebased 1.1.20（44688）比较 conflict-topic / renamed-untracked，选中
上半区 topic conflict 的 shared.txt。眼睛菜单中的 Show Diff Preview 起初未勾选；
键盘选择该项开启后，预览出现在上半区完整列表/详情区域下方，占该半区的宽度，
下半区不受影响。菜单切换后保留打开及勾选状态。截图
artifacts/collaboration-workflows-20261024/compare-reference-preview-{before,menu,on}.png。
底部独立 History 面板是之前打开的文件历史，不能误认为本次新建的分支比较预览。

源码版本仍为 4aec35256d765c217aa0d662b82ac4e860c396fd：
- plugins/git4idea/backend/src/branch/GitCompareBranchesUi.kt：两半区分别创建
  VcsLogUiImpl，使用 GitCompareBranchesTopLogProperties / BottomLogProperties。
- 同目录 GitCompareBranchesLogProperties.kt：各自 SHOW_DIFF_PREVIEW 默认 false，
  项目工作区持久化，属性修改触发通知；不能借用主 Log 的同一个布尔值。
- platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/MainFrame.java：
  FrameDiffPreview 包裹完整 myChangesBrowserSplitter，处理器来自当前 ChangesBrowser，
  不是仅在右侧文件列表内部加一个 Diff，也不是全窗口共享一个底部预览。
- 同目录 FrameDiffPreview.kt：按需创建 viewer；关闭时移除 secondComponent、dispose
  viewer 并置空；方向由 DIFF_PREVIEW_VERTICAL_SPLIT 控制，默认分割比例 0.7。

本地接入位置确认：src/ui/branch_compare.rs 的 Pane 拥有独立 chain / selected_file，
应各自拥有预览实体与分割状态；src/ui.rs 的 local_preview / bottom_preview 是主界面
共享状态，不适合直接复用其所有权。可以复用 DiffTab::preview 的查看器实现，但需要
为每半区接入选择同步、导航回写、切换提交时清空旧请求和关闭时释放。当前仅完成
上述前置验收及设计核对，尚未添加菜单开关或宣称预览功能完成；关闭状态与持久化
恢复的原生验证仍待后续补充。

分支比较内嵌预览接入（2026-10-25）：在上述实机和源码前置核对后，
新增 src/ui/branch_compare/preview.rs，Pane 分别保存 show_preview、DiffTab 实体及
ResizableState；眼睛菜单增加真实 Show Diff Preview 开关。按当前 chain / 文件选择
惰性创建只读 DiffTab::preview，不同目标释放旧查看器，相同目标复用；关闭时
显式 deactivate 并移除实体。FileChanged 回写所属 Pane 的 chain 与 selected_file，
不改变另一个比较区。当前没有文件选择时以链首文件为预览起点。

GPUI 既有刷新测试改为真实文件提交，增加预览目标 revision、重复同步复用实体、
关闭释放/重新开启新建实体及第二 Pane 不受影响的断言。最初空提交 fixture 导致
“应存在预览”断言失败，改为真实文件提交后 3 项通过；不是绕过空提交行为。
Windows release 构建通过（51.28s，/tmp/plico-compare-preview-build.log），正常关闭
自有46084后启动38044。原生截图 compare-plico-preview-{menu,on}.png 验证菜单入口、
上半区底部真实 Diff 及下半区无变化。截图暴露初始等分占比压缩列表，随后按上游
默认0.7调整为70/30；该比例修正尚待下一次Windows构建截图确认。

仍未完成：开关/分割比例的项目持久化、预览方向切换、目录选择限定子集、预览内
完整文件导航工具栏与源码跳转、切换隐藏比较Tab的主动资源释放。当前只完成基础
嵌入及开关生命周期，不宣称完整预览对齐；现存原生截图是比例修正前的状态。

分支比较预览开关持久化（2026-10-25）：原生Rebased44688关闭 conflict-topic
比较页后，从分支菜单 Compare with renamed-untracked 重新打开，上半区自动恢复预览，
下半区保持关闭，未选提交时显示 Nothing to show。此前另一个已打开的
 graph-projection-test 比较页也同步显示上半区预览。截图
artifacts/collaboration-workflows-20261024/compare-reference-reopened-preview.png。
重新阅读4aec352的 plugins/git4idea/backend/src/branch/GitCompareBranchesLogProperties.kt：
项目级 Top/Bottom 服务分别持久化 SHOW_DIFF_PREVIEW，set 发属性通知。规则不是
按分支对存储，也不是与主Log共用开关；本次本地实现前已完成以上实机步骤。

本地 layout_preferences.rs 新增 load/save_branch_preview，复用有长度限制、项目
身份复核的布尔设置格式；按仓库身份及上下区生成不同文件，不写入仓库。Pane创建
时恢复开关，菜单先保存成功再改变状态；失败沿用详情错误展示。PreviewChanged
通过View转发到Review，同窗口其他比较页的同一区同步更新，关闭同步释放已有预览。
保留差异：尚未跨两个原生窗口广播；分割比例持久化及方向切换仍待实现。

新增隔离测试在临时目录验证上下区、项目路径隔离、身份不匹配不读/不覆盖，1项
通过（/tmp/plico-preview-settings-tests.log）；3项分支比较GPUI测试通过
（/tmp/plico-preview-settings-desktop.log）。当前未完成Plico关闭重开原生验收，
也未宣称跨窗口同步已完成；上轮70/30比例的截图复验随本次Windows构建继续进行。

预览设置及比例原生复验（2026-10-25）：构建 /tmp/plico-preview-settings-build.log
通过（1m08s），正常关闭自有38044后更新验证副本，启动33988（HWND6359906）。
从 conflict-topic 分支菜单进入比较，选择 topic conflict 后开启上半区预览。
compare-plico-preview-proportion.png 确认70/30默认分配，文件树shared.txt仍可见，
预览revision为211d4091/28508190；下半区无预览。关闭此比较Tab，再从相同入口
重新打开，compare-plico-preview-reopened.png 确认上半区恢复开关且下半区保持关闭。
随后关闭上半区预览，compare-plico-preview-disabled.png 确认列表恢复占满上半区。
以上截图均保存并查看于artifacts/collaboration-workflows-20261024/。

当前与参考保留的可见差异：无选择时Plico使用靠左的“选择文件以查看差异”，
Rebased显示居中的Nothing to show且保留禁用的预览工具栏；本次不把恢复开关验证
等同于空态/完整工具栏对齐。应用重启恢复、跨窗口同步、分割比例持久化仍未完成
原生验收。没有关闭用户进程或新增额外常驻测试实例；当前自有Plico33988与
Rebased44688供后续使用。

预览数据失效与隐藏生命周期核对（2026-10-25）：原生Rebased44688选中
 topic conflict，切到Log再返回比较Tab，提交选择、文件及预览仍保留；截图
artifacts/collaboration-workflows-20261024/compare-reference-return-selection.png。
继续追踪4aec352源码，不能把FrameDiffPreview的dispose推断为隐藏即dispose：
platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/VcsLogChangeProcessor.kt 的
VcsLogTreeChangeProcessorTracker监听模型变化；其基类
platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/TreeHandlerDiffRequestProcessor.kt
明确updateWhileShown：hideNotify取消待更新队列，showNotify执行FULL刷新；隐藏状态
收到更新时clearPreview（clear/cleanBlocks），并非每次隐藏均销毁容器。故本轮没有
加入无条件“切Tab销毁查看器”，避免未保存阅读位置的行为退化；后续隐藏资源优化
应遵循失效时清理及显示时恢复，而不能冒称上游总是隐藏即释放。

本地修复src/ui/branch_compare.rs：clear_details和select新提交时立即清空preview，
不用等Pane再次render才能丢弃过期数据，隐藏Pane失效也适用；DiffTab::Drop取消
请求token，实体丢弃释放其任务和查看器。保留show_preview用户选项。测试扩展
真实文件提交fixture，在不经过render的情况下断言clear_details立即移除preview且
开关保持开启，然后重新选择提交继续原有刷新场景。3项GPUI测试通过
（/tmp/plico-preview-invalidation-tests.log），格式化完成；此次失效改动尚未更新
原生验证二进制。无滚动自动化，未声称阅读位置恢复或内存收益已测量。

分支比较文件树全部展开/折叠（2026-10-25）：先在Rebased44688的
conflict-topic比较上半区点击右侧Collapse All，根节点保留、shared.txt隐藏，预览
仍存在；再Expand All恢复文件。截图compare-reference-{collapse-all,expand-all}.png
在artifacts/collaboration-workflows-20261024/。源码4aec352：
platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/ChangesBrowserBase.java
使用TreeActionsToolbarPanel包装；同目录TreeActionsToolbarPanel.java把ExpandAll、
CollapseAll放在BorderLayout.EAST。shared/.../ChangesTree.java的MyTreeExpander
在没有分组且模型扁平时隐藏两个action，expandAll使用TreeUtil.expandAll。

本地src/ui/branch_compare/files.rs在工具栏右侧增加两按钮，按目录模式显示，
无内容时禁用。折叠从完整chain生成目录集合，避免当前已隐藏子节点遗漏；展开清空
集合，重建轻量可见行，不重新查询Git。折叠时原选择转根目录（原生多层选中文件
折叠后的具体选择仍待验收），平铺模式直接忽略操作。扩展现有GPUI测试覆盖已隐藏
深层目录、所有子目录折叠、恢复3个文件和原chain索引、平铺不受影响。此阶段
尚未进行Plico原生按钮截图验收，不宣称工具栏已完整对齐。

补充验收：原生Rebased先选择shared.txt再Collapse All，选中态转到根目录，
预览保留，截图compare-reference-collapse-selection.png；该单层场景与本地处理一致。
3项GPUI测试通过（/tmp/plico-compare-expand-tests.log）；Windows release构建通过
（/tmp/plico-compare-expand-build.log）。下一步用该构建完成Plico原生工具栏验收。

展开折叠及预览失效原生验收（2026-10-25）：正常关闭自有33988，替换最新
构建后启动41012（HWND141952880）。比较conflict-topic，选中topic conflict中的
shared.txt，Collapse All后根目录选中、文件隐藏；Expand All恢复文件，根目录选择
保留。与上述Rebased单层场景一致，截图compare-plico-{expand-before,collapse-all,
expand-all}.png。开启预览后改选空提交topic selection refresh probe（a562637c），
文件树清空、旧shared.txt Diff消失，详情切到新提交；再选topic conflict，恢复
211d4091/28508190的正确Diff。截图compare-plico-preview-{new-selection,
selection-return}.png。均保存并查看于artifacts/collaboration-workflows-20261024/。

此轮验证真实最终状态，不将截图声称为逐帧无闪烁证明；立即清理不依赖render由
上一轮GPUI断言覆盖。多层目录主要由测试覆盖，未进行滚动自动化。仍有空态
工具栏、目录子集预览、完整文件导航、分割设置等差异，本项验收不代表阶段5完成。

分支比较预览文件导航与调试断言修复（2026-10-25）：继续以上原生
compare-reference-expand-all.png / compare-reference-return-selection.png 中的预览
工具栏结构；本轮重读参考4aec352的
platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/VcsLogChangeProcessor.kt，
VcsLogDiffPreviewHandler提供selected/all迭代以及selectChange回写树选择。
本地DiffTab增加可独立启用的文件导航工具栏，branch_compare/preview.rs显式启用，
已有主窗口底部预览默认行为不变。FileChanged回写chain和选择，展开目标所有祖先，
重建可见行并定位选择。真实两文件提交fixture通过move_file发出事件，断言目标、
树索引、折叠根节点展开和另一半预览隔离。目录选择子集、多选语义仍待后续对齐，
本次没有将整个chain导航声称为完整复刻。

默认调试测试实际渲染预览时触发duplicate-hover；回溯定位到diff_settings.rs的
ModeSelector：ButtonCustomVariant已有hover颜色，按钮又重复调用InteractiveElement
的hover。移除冗余调用，保留已有variant颜色与交互，不更改按钮视觉设计。
3项branch_compare测试通过；此前两项默认忽略的导航测试显式--ignored运行也通过，
故移除过时ignore并更新贡献规范，不再关闭断言绕过此问题。
日志/tmp/plico-compare-preview-navigation-tests.log和
/tmp/plico-navigation-debug-ignored.log。本轮新增导航控件的Windows原生多文件操作
及截图仍待验收，不以GPUI测试代替。未提交、未运行CI。

预览导航Windows原生验收（2026-10-25）：Windows release构建通过44.99秒，
/tmp/plico-preview-navigation-build.log。仅关闭自有41012，更新二进制后启动42676
（HWND3475704）；Rebased仍使用44688。两个独立测试仓库通过临时index和commit-tree
创建preview-navigation-fixture，父提交均为各自HEAD；增加preview-nav/alpha.txt与
preview-nav/nested/beta.txt，不改当前HEAD、工作区或真实index。
参考提交bb34f6d0，本地提交4b3f139c。Rebased比较该分支与当前分支，选择唯一提交，
预览1/2显示nested/beta.txt，点击右箭头后2/2显示alpha.txt且树选中alpha.txt。
Plico相同入口先显示alpha.txt；折叠全部目录后点击预览右箭头，2/2显示nested/beta.txt，
祖先全部展开、beta.txt选中。截图preview-nav-reference.png、
preview-nav-reference-next.png、preview-nav-plico-first.png、preview-nav-plico-next.png
保存并查看于artifacts/collaboration-workflows-20261024/。

本轮证明按钮/计数/目标内容/树选择同步及折叠后展开可用，同时发现尚未对齐的导航
顺序：Rebased按照树顺序（目录优先）枚举，Plico仍沿Git路径顺序。后续需按
VcsLogDiffPreviewHandler.iterateAllChanges / VcsTreeModelData树顺序生成预览chain，
分组开关也应影响顺序；不能将本次成功切换声称为完整导航语义对齐。
另外根目录合并、空态工具栏、目录子集/多选仍有已记录差异。本轮未自动化滚动。
完整默认桌面测试99通过0忽略（/tmp/plico-preview-navigation-desktop.log），格式检查
通过；此处没有用原生截图代替全部行为覆盖。未提交或运行CI。

预览导航树顺序修正（2026-10-25）：以前述两文件原生截图为实现前依据，补读
4aec352的platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ui/
VcsTreeModelData.java：AllUnderData遍历myNode.traverse，iterateNodes筛选meaningful
节点；不是可见行数组，所以折叠不应删去导航文件。与VcsLogDiffPreviewHandler的
iterateAllChanges对应。本地files.rs生成完整compact_rows中的文件顺序，平铺使用
现有文件名/路径排序；保留原chain索引作为树选择身份。

Chain::reordered校验完整无重复排列，同时重排普通/combined/working-comparison
端点并映射当前游标。只在创建预览或分组改变时生成独立导航chain，不在每次render
复制整个列表。preview.rs首次无选择时采用树中的第一文件，已有预览则保留目标；
切换分组时通过DiffTab::set_navigation_chain仅替换导航数据，保留实体/查看器及阅读
位置，失效旧文件选择弹层。不开启预览时不计算排序。FileChanged仍映射回原chain。

测试增加目录顺序、折叠不改导航、平铺排序、非法重复/越界/缺项排列拒绝、保留
combined文件端点、首次预览nested/second.txt、分组切换后实体/目标不变且导航游标
更新。分组切换的原生观察及本次排序修正后的Windows截图尚待验收；单独Diff Tab
的导航顺序、目录子集、多选尚未在本轮处理。此改动不代表阶段5完成。
本轮默认桌面回归99通过、0失败、0忽略（/tmp/plico-preview-order-desktop.log）；
cargo fmt完成，git diff --check通过。未提交、未运行CI，本次原生构建仍待更新。

预览排序原生验收补充（2026-10-25）：Rebased44688在两文件fixture中保持alpha.txt
已选（目录模式2/2），打开眼睛菜单关闭Directory后，树变为alpha/beta，目标alpha
不变、预览计数更新为1/2。截图preview-order-ref-{menu,flat}.png。说明分组切换应
保留目标，同时更新完整导航顺序，而不是沿旧chain计数。

Windows release构建通过1m07s（/tmp/plico-preview-order-build.log）。正常关闭自有
42676，更新二进制并启动12016（HWND61084716）。新建相同比较并选择唯一提交，
目录模式首次为nested/beta.txt（1/2）；点右箭头后为alpha.txt（2/2）。关闭目录分组，
alpha.txt仍选中且预览保持first file，计数变为1/2；再次点右箭头为beta.txt（2/2），
树选择同步。截图preview-order-plico-{grouped,menu,flat,flat-next}.png与参考截图均
保存并实际查看于artifacts/collaboration-workflows-20261024/。这证明已验证两文件
场景的初始顺序、下一文件、分组变更保留目标/更新顺序；实体保留由前轮GPUI测试
覆盖，截图不代表逐帧无重载或长文档阅读位置测试。原有目录子集、多选、独立Tab
链顺序等差异继续保留在阶段5待办；未提交或运行CI。

目录选择预览规则核对（2026-10-25，实现前）：Rebased44688两文件fixture先预览
alpha.txt，再切回目录分组并选择nested（折叠状态），预览切到nested/beta.txt，计数
仍1/2，右箭头可用。截图preview-folder-ref-selected.png已保存并查看于
artifacts/collaboration-workflows-20261024/。因此“目录选择一律缩成目录内chain”
不是准确规则。参考4aec352的platform/vcs-impl/src/com/intellij/openapi/vcs/changes/
ChangeViewDiffRequestProcessor.java，refresh从selectedChanges中保留当前项，若不存在
取第一项；空选择默认all。getSelectionStrategy在selectedChanges.size>1时限定范围，
否则遍历all并更新树选择。多个子文件的目录范围尚需进一步实机验证。
本轮先修复单目录选择目标：只在目录包含当前文件时保留它，否则切到目录内按树
顺序的第一文件，目录折叠不影响这个选择。多文件目录导航范围仍明确待实现。
本地preview.rs已按所选目录过滤保留目标/寻找第一个后代；测试使用真实两文件提交，
先切到review.txt，再选折叠的nested，断言切到nested/second.txt且完整导航仍2文件；
重复同步复用实体，选择父根目录继续保留该文件。3项定向GPUI测试通过
（/tmp/plico-folder-preview-tests.log），格式化及diff whitespace检查通过。此修复
尚未更新Windows原生二进制，验收待补。

额外Rebased原生观察：选中包含两文件的preview-nav父目录，再按右箭头，从beta
切到alpha，树中仍保持父目录选中，而非改选alpha。截图
preview-folder-ref-parent-next.png同目录。这补充确认了源码ChangesNavigatable的
多项选择updateSelection=false规则；下一步目录多文件范围需同时保留树选择，
不能只缩小chain却在FileChanged中清掉目录选择。当前该差异仍未修复。

多文件目录导航范围（2026-10-25）：实现前在原生Rebased44688扩展独立fixture为
alpha.txt、nested/beta.txt、nested/gamma.txt，参考提交ac3e65ad，本地fixture9772398e。
仅通过临时index/commit-tree更新测试分支，不改HEAD/工作区/实际index。选nested后
点下一文件，预览gamma.txt、计数2/2、下一文件禁用，nested目录仍选中；截图
preview-scope-reference-next.png（同artifacts目录）实际查看。点击文件计数后的
截图未取得弹层，因此不声称已验收弹窗内容。
源码4aec352的ChangeViewDiffRequestProcessor.java：getSelectionStrategy多项时使用
selectedChanges并updateSelection=false；getGroupOrMultiSelectionListSelection在多项
选择时同样返回selectedChanges（由registry控制新路径，实机计数已观察为2）。

本地Chain::subset为有序不重复子集保留各文件revision endpoints与当前游标，
reordered继续要求完整排列。preview.rs在所选目录含多文件时构造子集chain，单文件
目录仍全链；preview_folder参与导航数据失效判断，即使目标未变，变更目录选择
也必须更新范围。目录内FileChanged保留目录选择和折叠状态；单文件导航继续同步
选中文件。退出目录选择恢复完整顺序，复用当前查看器。不支持任意Ctrl/Shift多选
仍是独立待办。本轮新增GPUI场景只验证状态/事件/导航范围，使用不存在的fixture
路径，不冒称测试了Git内容读取；真实读取由此前真实fixture和后续原生验收覆盖。
本轮4项分支比较定向测试通过（/tmp/plico-preview-scope-tests.log），完整桌面100通过、
0忽略（/tmp/plico-preview-scope-desktop.log）。fmt检查和diff whitespace检查通过。
首次新增测试缺少项目既有gpui_kit别名/内建test宏导入导致宏递归，已按现有测试模块
方式修正；没有提高递归限制或忽略测试。新增状态测试验证范围2项、末尾不越界、
导航后仍选中折叠目录、切回文件选择恢复3项全链及当前viewer复用。Windows原生
本地二进制仍为上一轮版本，目录选择目标与本轮范围改动须一起更新后验收。
未提交、未运行CI；完整计划保持进行中。

目录范围及目标选择原生验收（2026-10-25）：Windows release构建通过1m07s
（/tmp/plico-preview-scope-build.log）。正常关闭自有12016，更新构建后启动47860，
HWND5113914。比较三文件fixture9772398e，选中nested并折叠，预览beta.txt，1/2；
点击右箭头到gamma.txt，2/2、下一文件灰色，目录仍选中且保持折叠。选择外部
alpha.txt后全列表恢复3/3；重新选择nested后切到beta.txt、1/2，不再保留目录外
alpha.txt。截图preview-scope-plico-{folder,next,outside,return}.png在
artifacts/collaboration-workflows-20261024/，均实际查看，与参考多文件目录规则一致。
本轮覆盖上一轮目标修复与本轮范围修复的真实文件读取/计数/选择/按钮状态；边界
越界拒绝同时由GPUI测试覆盖。不声称单文件夹全部边界或任意多选已完成。

准备独立Tab后续对照时，对Rebased选中nested按Enter，未产生新Diff Tab，仍是
目录选中、预览gamma.txt（preview-scope-reference-enter.png）；不能把该操作声称
为已验证“目录Enter打开Tab”。下一步需针对文件Enter/双击及Ctrl+D分别核对入口。
当前Plico独立Tab仍使用基础chain顺序，是已知待修正项。未提交、未运行CI。

分支比较独立 Diff Tab 导航同步（2026-10-25）：实现前操作原生 Rebased
1.1.20 / IC-262.10968.SNAPSHOT（44688），从三文件 fixture 双击 alpha.txt，
独立 Tab 显示 3/3；上一文件切换 gamma.txt 后返回比较页，文件树选中 gamma，
底部预览同步 2/3。截图 compare-tab-reference{,-return}.png 保存在
artifacts/collaboration-workflows-20261024/。参考源码仍为 4aec352：
platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/VcsLogChangesBrowser.kt
的 createChangeProcessor / selectChange，以及
platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/TreeHandlerDiffRequestProcessor.kt
的 collectDiffProducers / selectChange；树顺序决定 producer 顺序，导航选择回写来源树。

本地 src/ui/branch_compare.rs 的 open_file 使用完整文件树导航顺序重排 chain；
创建独立 Tab 时将 FileChanged 连接到来源比较 View 的 Pane，而非主 Log 的导航选择。
src/ui/branch_compare/preview.rs 共用 track_diff_navigation / select_diff_target，
以完整端点身份匹配当前 chain，防止旧提交事件修改新提交选择；展开目标祖先目录，
定位选中行，由现有预览同步机制更新内容。订阅归属 Pane 生命周期。
测试通过真实 OpenDiff / move_file 事件核对顺序、选中、展开及过期提交保护。

验证：定向 4 项及完整 desktop-tests 100 项通过，无忽略；fmt 与 diff whitespace
检查通过，Windows release 构建通过。正常关闭自有 47860，更新验证副本并启动
47864（HWND6425056），原生重复 alpha 3/3 → 上一文件 → 返回比较页，确认 gamma
选中、nested 展开、预览内容 third file 与 2/3 同步。
截图 compare-tab-plico-{open,return}.png 同目录，均已查看。
CI 未运行；任意多选、来源树反向更新已打开 Tab 及更广泛生命周期仍属后续核对范围，
不将本项验收等同于分支比较阶段全部完成。

分支比较独立 Tab 反向同步（2026-10-25）：先原生操作 Rebased44688，在比较页从
选中的gamma改选beta，切回已打开的Repository Diff后显示beta及1/3，未新开Tab。
compare-reverse-reference-{before,after}.png已保存查看。源码4aec352的
TreeHandlerDiffRequestProcessor.kt / TreeHandlerChangesTreeTracker.track监听selection及
model变更，refreshPreview区分来源模型更新；VcsLogChangesBrowser.kt:createChangeProcessor
以!isInEditor传updateWhileShown，因此独立Tab持续跟踪树状态。安装版与源码快照区别同上。

本地branch_compare.rs创建Tab时，仅连接完整端点匹配的来源Pane；弱引用Tab，关闭后
事件不再更新已移除的Tab。观察来源状态，以active_diff_index决定是否加载可见Diff。
preview.rs抽取navigation_selection，嵌入预览与独立Tab共用树排序、保留文件、目录范围
规则；diff_tab.rs:sync_source_selection相同文件仅更新导航chain，不重建viewer，目标改变
取消旧读取并替换，隐藏Tab延迟读取，来源暂为空则清空旧内容显示已有中英文选择提示。
没有引入上游100ms debounce：本地通知合并由GPUI处理，隐藏Tab不执行内容查询。
任意多选、重复打开Tab策略、关闭来源比较页后的Tab生命周期仍需后续核对。

测试：补充选择文件反向更新与多文件目录缩小chain断言；完整desktop-tests100通过，
无忽略，日志/tmp/plico-reverse-desktop.log；fmt检查及git diff --check通过，Windows
release构建通过（/tmp/plico-reverse-build.log）。正常关闭自有47864后更新验证副本，
启动38264（HWND2557476）。原生从alpha打开Tab后返回比较页选beta，再回原Tab：
标题beta、1/3、second file均同步；再选nested目录，原Tab保持beta，导航变成1/2。
截图compare-reverse-plico-{after,folder}.png已查看，均在
artifacts/collaboration-workflows-20261024/。仅保留这个自有Plico与原Rebased44688。
CI未运行；这不是分支比较全阶段完成声明。

分支比较独立Tab复用与来源关闭（2026-10-25）：实现前原生Rebased44688从beta预览
返回比较页双击alpha，复用原Repository Diff Tab，显示alpha/3 of 3；关闭该比较页后，
关联Repository Diff也消失，其他比较页和Changes Tab保留。截图
compare-reuse-reference.png、compare-close-reference.png已查看并保存到
artifacts/collaboration-workflows-20261024/。
读取参考4aec352的platform/diff-impl/src/com/intellij/openapi/vcs/changes/EditorTabDiffPreview.kt：
每个preview对象持有唯一previewFile，openPreview通过showDiffFile打开该文件；
TabPreviewDiffVirtualFile将viewer.disposable注册到preview，preview销毁后虚拟文件失效。
GitCompareBranchesUi.kt分别创建top/bottom Log UI，因此预览按来源面板归属。

本地Pane增加editor_preview弱引用，OpenDiff按端点匹配来源Pane，并查找仍存在于
Review.tabs的预览，存在则更新目标和激活；关闭后重新创建。View仅提供关联预览ID，
editor_tabs.rs关闭比较页时通过原有close_diff_tab_from_header清理关联Tab及任务。
测试independent_preview_is_reused_reopened_and_closed_with_source驱动实际OpenDiff事件，
验证同一entity复用、关闭重开得到新entity、关闭来源清空关联Tab。
完整desktop-tests101通过，无忽略（/tmp/plico-reuse-desktop.log）。原生修改后验收见下文。

本项原生验收：Windows release构建通过（/tmp/plico-reuse-build.log），fmt与diff检查
通过。正常关闭自有38264后更新验证副本，启动46760（HWND88609406）。比较页先双击
beta，再返回双击alpha：只有一个Repository Diff，内容alpha/3 of 3；关闭Diff后再次
双击alpha可重开；从Diff页点击来源比较页的关闭按钮，关联Diff同时关闭并回到Log。
截图compare-{reuse,reopen,close}-plico.png均已查看并保存到上述artifacts目录。
CI未运行；其他比较页面板隔离和未完成的多选等仍按阶段计划继续，不宣称全阶段完成。

分支比较多提交选择第一步（2026-10-25）：修改前原生Rebased44688在conflict-topic比较页
用Shift+Down选中前两条空提交：两行同时高亮，文件区空，底部No files/Nothing to show。
compare-multi-reference.png已查看保存。读取参考4aec352的
VcsLogAsyncChangesTreeModel.kt:createChangesState（多个details走collectChanges）及
platform/vcs-log/impl/src/com/intellij/vcs/log/util/VcsLogUtil.java:collectChanges：
按显示顺序反转已选details，再用CommittedChangesTreeBrowser.zipChanges汇总，不能以
最老和最新之间的完整revision range替代非连续选择。

本地branch_compare.rs复用log_selection::Selection处理Ctrl切换与Shift范围选择；
多提交文件查询复用log_changes::load，单提交保留现有查询；过滤刷新按OID reconcile，
选择集变化取消旧详情请求后重新汇总。已有真实仓库GPUI测试补上过滤后两条非连续
提交选择、合并前后blob内容检查、Ctrl取消后回到原单选的断言。
本轮暂保留第一条所选提交详情展示，多提交详情卡片及键盘扩选尚未完成；不将此项
记为多提交交互全部交付。当前已通过定向5项，原生修改后验证如下补充。

本轮验收：完整desktop-tests101通过，无忽略，fmt与diff检查通过；Windows release
构建通过（/tmp/plico-multi-build.log）。正常关闭自有46760，更新副本后启动46564
（HWND37556800）。原生比较conflict-topic与rename-filtered，Shift点击第二行，两条
空提交同时高亮且文件区为空；Ctrl点击第三条topic conflict，保留前两条选择并增加
shared.txt汇总差异，底部显示main conflict→topic conflict。compare-multi-plico.png与
compare-multi-plico-files.png已查看保存。Plico测试分支基线与Rebased fixture不同，
未将其差异内容声称为同一提交的像素对照；鼠标多选行为已验证。CI未运行。

分支比较提交列表键盘选择（2026-10-25）：修改前原生Rebased44688在上一轮两行选择上
Shift+Up收缩回第一行，截图compare-keyboard-reference.png已查看保存。
参考4aec352的VcsLogGraphTable.java构造函数安装ScrollingUtil.installActions(this,false)，
platform/platform-api/src/com/intellij/ui/ScrollingUtil.java:_move使用lead index计算下一行，
非循环模式越过首尾直接返回，并确保目标可见；表格范围选择使用标准selection model。

本地branch_compare.rs给提交列表独立commits_focus及BranchCompareCommits上下文，
绑定Up/Down、Shift+Up/Down，复用log_selection::Selection.step，边界不循环，按Nearest
定位目标；鼠标选择先将焦点交给列表。上下文仅包围列表，不包围搜索工具栏和文件树。
真实仓库GPUI测试使用simulate_keystrokes验证Shift扩选、反向收缩、末行不循环、普通
上移变为单选及另一比较面板不受影响，定向5项通过。原生验收见后续补充。

原生补验发现新增焦点div没有撑满内部uniform_list，提交行不可见；已将内部列表改为
size_full，并补上鼠标行点击对commits_focus的焦点交接。修正后完整desktop-tests101
再次通过，fmt/diff检查及Windows release构建通过（/tmp/plico-keys-{desktop,build}.log）。
正常关闭自有45656，更新副本后启动46484（HWND4589082）。原生点击首行后连续两次
Shift+Down选中前三行、Shift+Up收缩到前两行、普通Down恢复第三行单选，右侧文件及
底部Diff随选择同步；截图compare-keyboard-plico-{extend,shrink,single}.png已查看并保存。
旧的不可见列表截图不作为通过证据。CI未运行；多提交详情卡片仍未完成。

分支比较多提交详情卡片（2026-10-25）：先在原生Rebased44688选择两条提交，截图
compare-cards-reference.png已查看。尝试调整详情分隔条未改变布局；截图只证明当前
多选场景与首卡显示，未将隐藏的第二卡声称为直接可见证据。源码4aec352：
CommitDetailsListPanel.kt:rebuildPanel按输入顺序创建卡片并插入OnePixelDivider，统一
垂直滚动且不水平滚动；VcsLogCommitSelectionListenerForDetails.kt:onDetailsLoaded
取前MAX_COMMITS_TO_LOAD=50并显示overflow提示；CommitDetailsPanel.kt定义SIDE_BORDER=14。

本地branch_compare.rs使用Vec<Presentation>替代单条details/presentation，按实际选择
顺序后台生成最多50张卡片，总表示字节仍受OUTPUT_LIMIT限制；合并文件范围不受50条
详情上限影响。复用commit_presentation的双语metadata及已有Showing the first 50 commits
文案，卡片之间1px分隔并设置flex_shrink_0，避免内容互相压缩。真实仓库GPUI测试验证
过滤后选择topic two与topic one时两张卡片顺序、计数、取消选择后回到单条与清空。
当前仍使用原有max_h(130)滚动区域，尚未对齐可拖动详情分隔条；尚缺refs/包含分支与
请求总数的overflow文案细化，不宣称整个详情区像素一致。

多提交卡片原生补验（2026-10-25）：重新确认自有 Plico 45368 / HWND10421340 与
Rebased 44688 仍存活。截图 compare-cards-plico.png 验证两行选择及首张详情卡；
普通 Down 后截图 compare-cards-plico-single.png 验证恢复第三行单选，详情标题、
提交哈希、文件树及底部 shared.txt Diff 同步切换，没有保留前一次首卡内容。
截图已实际查看并存入 artifacts/collaboration-workflows-20261024/。
第二张卡片位于滚动区域下方，本轮未自动滚动，不将截图视为第二卡视觉通过证据；
两卡数据顺序由真实仓库 GPUI 测试覆盖。既有 /tmp/plico-cards-desktop.log 确认
101 项通过、0 ignored，/tmp/plico-cards-build.log 确认 Windows release 构建成功。
此次只补原生验收与记录，没有重跑未变更代码的测试，也没有运行 CI。
下一项仍为详情区可拖动分隔条，固定高度限制尚未解决，阶段 5 保持进行中。

分支比较详情分隔条前置验证（2026-10-25）：原生 Rebased 44688 同一两提交选择，
在物理位置 (1700,311) 将详情分隔条上拖 110px，文件区变小、详情区变大，底部
Diff 区边界保持不变；compare-split-reference-{before,after}.png 已查看并保存。
参考 4aec352 的 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/MainFrame.java
构造与 showDetails：详情使用独立垂直 OnePixelSplitter，默认文件区比例 0.7；
开关通过移除/恢复第二个组件控制，未选择提交时仍保留详情占位。Plico 将在 Pane
增加独立 ResizableState，替换 max_h(130)，使用同样 70/30 初始比例及空态。
会话内保留拖动位置；跨重启比例持久化仍为待补差异。

详情分隔条实现与验收：src/ui/branch_compare.rs 新增 details_split，各比较半区分别
持有状态，文件区与详情区使用 v_resizable，初始 70/30；未选中时显示居中的双语
Commit details。5 项分支比较测试通过（/tmp/plico-details-split-tests.log），Windows
release 构建通过（53.02s，/tmp/plico-details-split-build.log），fmt/diff 检查通过。
正常关闭自有45368并替换验证副本，新38392/HWND41159758：详情边界从物理y386
上移110px，底部Diff及另一半面板边界不变。随后首行Shift+Down选择两条提交，
扩大后的详情区可同时看到两张卡片及中间细线，切换选择未重置分区大小。
compare-split-plico-{before,after,multi}.png 已实际查看并保存。未运行CI，未测试滚动。
跨重启比例持久化、refs及包含分支展示仍待补齐，不宣称阶段5完成。

详情比例持久化前置（2026-10-25）：原生 Rebased44688 关闭 conflict-topic 比较 Tab，
通过分支菜单 Compare 重新打开；上下半区都使用最后拖动后的比例，未回到默认值。
compare-ratio-reference-reopen.png 已查看并保存。读取 4aec352 的 MainFrame.java、
platform/platform-api/src/com/intellij/ui/OnePixelSplitter.java 和 JBSplitter.java：
所有 MainFrame 共享 vcs.log.details.splitter.proportion，addNotify 时读取全局
PropertiesComponent，setProportion 时保存；不是按分支或比较方向保存。
本地复用 layout_preferences::{load_files_fraction,save_files_fraction}，与普通 Log
共用配置。每个 Pane 的已打开布局仍独立，新建 Pane 读取最近保存值；拖动结束写入。

比例持久化本地验收：定向5项通过，Windows release通过（55.20s）；日志为
/tmp/plico-ratio-{tests,build}.log。正常关闭自有38392，更新副本启动14492/5637842。
新比较页读取已有Log比例（0.791077，受面板最小尺寸约束）；上拖分隔条110px后
配置文件变为0.383886。关闭比较Tab并重新打开，上下两半均读取新比例，首半分隔条
恢复至拖动后的物理y276附近，第二半同比例变化。compare-ratio-plico-{load,reopen}.png
已查看并保存。验证了磁盘写入与新实体加载；本轮未再次重启整个进程验收，也未运行CI。
保留现有Log的0.1..0.9范围和组件最小尺寸保护；未做跨窗口实时联动。

分支比较详情 refs/包含分支前置（2026-10-25）：Rebased44688 选择 conflict-topic 顶端
提交，实际显示绿色标签 conflict-topic，下一行 In 1 branch: conflict-topic。
compare-refs-reference.png 已查看。读取4aec352的 VcsLogCommitSelectionListenerForDetails.kt：
先展示最多50卡和排序后的refs，再异步requestContainingBranches；选择变化取消请求。
ContainingBranchesGetter.kt使用最多2000项缓存、顺序LIFO队列10项，refs校验和变化清缓存；
不完整graph结果不缓存。Plico复用普通Log的commit_refs与ContainingBranches/Batch组件，
单个顺序后台worker逐卡更新，弱实体隔离过期结果，取消选择释放worker。保留差异：
目前直接查询Git，不增加上游graph缓存；仍受50卡及总输出字节限制。

本地接入进度：Pane 使用 DetailCard 将 presentation、refs、ContainingBranches 实体绑定，
元数据返回后启动 Batch，clear_details/reload_selection 释放旧 Batch 与卡片；不把包含
分支查询串入文件/元数据首屏任务。单选和多选复用同一UI及异步查询组件。
定向5项通过，真实仓库测试增加 topic 标签、Batch存在及清空后释放断言；Windows release
构建通过（56.21s）。日志 /tmp/plico-refs-{tests,build}.log。修改后仅新增测试断言并格式化，
生产代码未改变。本轮Plico新二进制尚未启动，原生展示验收待完成。
发现待处理边界：View::refresh 在比较两端OID不变时跳过reload，且保留相同选择时不会
刷新卡片refs/包含分支。必须在引用变化时使此附属信息失效，不能依赖用户重新选择。
该边界与原生验收未完成，不将 refs/包含分支对齐计为完成。

引用刷新前置（2026-10-25）：在独立Rebased测试仓库新增 details-ref-probe 指向
conflict-topic，不改比较两端或选择；原生界面自动出现第二个标签，包含分支由1变2。
compare-refs-reference-refresh.png已查看。依据前文ContainingBranchesGetter refs校验和
失效及监听回调规则，本地将增加独立附属信息刷新请求，不重建文件链或Diff。
另发现无路径过滤的比较page只parse_history，未填refs；单选详情需读取当前引用快照，
不能直接使用裸列表行的空refs。采用现有snapshot_commits，最多50张卡片。

引用刷新修复与原生验收：Pane::refresh_card_refs 使用独立可取消Request查询当前最多50卡
的引用快照，再替换包含分支任务；不改变文件链/选择/Diff实体。比较端点未变化或列表
刷新保留选择时均调用此路径。清空与重新选择取消该Request；单选首次详情也读取refs。
真实仓库GPUI测试新增/删除details-ref-probe，验证标签更新、选择保持、文件链保留及旧
包含分支实体释放。定向5项和完整desktop101项通过，Windows release构建54.81s通过；
日志/tmp/plico-refs-refresh-{tests,desktop,build}.log，diff检查通过，CI未运行。
正常关闭自有14492，新36892/52892368原生比较页选择首提交，显示conflict-topic标签及
“位于1个分支”；外部在独立仓库新建details-ref-probe后，无需重新选择，自动显示双标签
及“位于2个分支”，窄列出现“显示全部”。compare-refs-plico-{before,after}.png已查看。
本轮也证实新进程读回上一轮保存的详情比例。两测试仓库的details-ref-probe已清理。
仍保留直接Git查询而非全图缓存的差异；每次仓库刷新会重查当前卡片，后续可按refs指纹
减少重复查询。阶段5其余范围未完成，不作为整体完成声明。

### 阶段5下一步：任意两分支的入口与选择规则（2026-10-25）

原生前置：Rebased1.1.20/IC-262.10968.SNAPSHOT，自有44688；Log→展开Branches，
点击conflict-main后Ctrl点击conflict-topic，右键保留两项选择。菜单实际为Compare
Branches、Show Files Diff、禁用Update Selected、Delete Branches。按Down/Enter执行
第一项，打开Compare: conflict-main and conflict-topic：上半main独有1条、下半topic独有3条。
当前检出分支仍renamed-untracked，证明两端都可以不是当前分支。截图compare-pair-reference-
{log,tree,menu,open}.png已逐张查看并存于artifacts/collaboration-workflows-20261024。
首次用主窗口鼠标helper点击弹出菜单被HWND归属检查拒绝，未绕过检查；随后用同PID校验的
键盘操作成功。此轮只实测Compare Branches；Show Files Diff结果及HEAD组合尚待原生验证。

实读源码4aec352：
- plugins/git4idea/backend/src/ui/branch/dashboard/BranchesDashboardActions.kt：
  BranchesPairActionBase.getBranchPair允许恰好两条branch，或一条branch+HEAD（HEAD必须
  能解析为当前分支）；同名或无共同仓库禁用。ShowArbitraryBranchesDiffAction调用compareAny，
  ShowArbitraryBranchesFileDiffAction调用showDiff，两类入口不能合并为同一种比较。
- plugins/git4idea/tests/git4idea/branch/GitBranchesTreeActionsForSelectionTest.kt：
  current local+remote显示两比较动作，3条分支不显示比较；混合分组等非法选择无动作。
- plugins/git4idea/backend/src/actions/branch/GitCompareWithBranchAction.kt：单分支菜单
  是Compare with current，禁用当前分支，不能拿它代替任意两分支入口。
- plugins/git4idea/backend/src/branch/GitCompareBranchesUi.kt：固定range，双向独有提交，
  比较页隐藏branch filter，不在结果页添加临时端点选择框。
- GitCompareBranchesFilesManager.kt：按range/root身份复用已有比较Tab。

本地缺口已确认：ui/log_branches.rs只保存selected_tree_ref，Ctrl选择未实现；
ui/branch_compare.rs::prepare_branch_comparison固定第二端为当前分支。下一实现按上游
分支树多选→两项右键入口→复用Pair/View及净文件比较管线推进，不新造双输入弹窗。
需要同时处理右键保留选择、普通点击清除、多选超过2隐藏比较、刷新删除引用时协调选择、
键盘Ctrl/Shift与已有导航不冲突；保存屏障、取消迟到请求及Tab复用沿用现有路径。
此记录细化阶段5实现顺序，不删减任意两分支/净文件差异要求。产品代码本轮未修改。

任意两分支入口实现第一步（2026-10-25）：ui.rs增加branch_selection，复用身份选择模型；
ui/log_branches.rs支持Ctrl点击切换、Shift点击范围、Shift上下键范围，普通选择收拢；
右键已选中保留集合，未选中改为单选。重建可见行时reconcile，仓库切换清空。
恰好两条本地/远端分支提供Compare Branches；混合标签/分组或超过2条不显示该操作。
HEAD+branch解析规则已写入，但HEAD行本身的右键入口尚未接入，待补原生场景。
ui/branch_compare.rs抽出prepare_branch_pair，使双端点复用保存屏障、取消任务和Tab复用，
既有Compare with current仍调用同一方法。i18n新增“比较分支”。
新增GPUI选择回归涵盖Ctrl、Shift、超过两项、标签无效、删除引用后协调；完整桌面102项
通过（/tmp/plico-pair-desktop.log）。首次测试宏导入错误已修复，未扩大recursion_limit。
产品原生交互尚未验收；Show Files Diff入口未接入，F2/Enter等多选动作约束仍需核对。
本轮未提交，阶段5继续进行。Windows构建日志/tmp/plico-pair-build.log。

### 两分支净文件差异入口（2026-10-25）

原生 Rebased 1.1.20（44688），Log 分支树保留 conflict-main/topic 多选，右键第二项
Show Files Diff，替换左侧 Changes 面板，标题 Changes Between conflict-main and
conflict-topic，只显示 shared.txt；未包含工作树 local.txt。截图 pair-ref-files-open.png。
Plico 多选 Compare Branches 已打开指定两端（非当前 rename-filtered），截图
pair-plico-open.png；均存于 artifacts/collaboration-workflows-20261024。
实读 4aec352 的 BranchesDashboardActions.kt::ShowArbitraryBranchesFileDiffAction、
GitBrancherImpl.java::showDiff、GitChangeUtils.java::getDiff 及
platform/vcs-impl/src/com/intellij/vcs/CompareWithLocalDialog.java::showChanges/showAsTab：
两端 revision 的净差异，启用 rename detection，LocalContent.NONE，不保存工作文档；
后台加载并复用 Changes 工具窗口。端点解析为不可变 revision，文件内容延后加载。
本地拟复用 CompareView 和 show_comparison，增加分支来源以保留完整标题，刷新时解析
两端 OID；不混入当前工作树，也不复用双向独有提交列表代替文件比较。

实现与验收：CompareView 新增 Branches 来源，后台解析两端 OID，再复用 revision_changes
和只读 Chain::combined；标题保留分支名。Log 两分支菜单新增 Show Files Diff（中英文本）。
不经过保存屏障，符合 LocalContent.NONE；刷新与释放沿用取消令牌和 generation。
新增真实临时 Git 仓库 GPUI 测试：脏 b.txt 不进入结果，topic 移到 main 后结果为空，
删除 topic 显示错误且无陈旧链，恢复引用后再次加载成功。完整桌面 103 项通过，fmt 与
diff whitespace 检查通过，Windows release 构建 1m09s，日志 /tmp/plico-pair-files-{tests,build}.log。
正常关闭自有旧实例 39484，启动 32336；Ctrl 多选 conflict-main/topic → 第二菜单项，
仅展示 shared.txt，双击打开只读 main conflict → topic conflict，两端 OID正确。
截图 pair-plico-files-{menu,open,diff}.png 已逐张查看；参考 pair-ref-files-diff.png亦已查看。
尚有差异：现有 Changes 工具栏为刷新+文字“显示差异”，缺参考的图标/显示选项/展开收起；
文件根分组和计数未还原，不能宣称工具窗口像素级一致。原生两边 Diff 模式偏好不同，
本轮仅验收端点、内容、只读及入口，不将该截图作为统一/分栏像素对比。
下一步继续 HEAD+branch 入口及多选快捷键约束，再补 Changes 工具栏/文件树展示。
阶段5未结束，阶段6子模块、7浅克隆补全、8交互式rebase继续保留；未提交。

### HEAD 多选比较（2026-10-25）
原生 Rebased 1.1.20/44688，HEAD+conflict-topic 多选，在 HEAD 行右键出现两项比较；
HEAD+当前 renamed-untracked 多选则两项均灰显。截图 pair-head-reference.png、
pair-head-same-reference.png。实读 4aec352 BranchesDashboardActions.kt::BranchesPairActionBase：
HEAD组合返回 selectedBranch to currentBranch；同名 pair 禁用，不能隐藏；detached HEAD
无法取得 currentBranch 时不提供操作。本地 log_branches.rs 修正端点顺序、保留同名
pair供禁用显示；HEAD 行增加保留多选的右键入口，与叶子行共用菜单项构造。

验证：GPUI选择测试增加 HEAD比较顺序、HEAD+当前分支保留相同端点（菜单禁用）、
detached HEAD 拒绝比较；完整桌面103项通过，fmt及diff whitespace通过，Windows
release构建成功，日志 /tmp/plico-head-pair-{tests,build}.log。首次str.as_str误用
导致编译失败，已改直接引用比较，未引入unstable feature。
原生 Plico 旧32336正常退出，25288启动新构建，HEAD+topic右键显示两项且保留多选，
打开比较页顺序为conflict-topic/rename-filtered；HEAD+rename-filtered两项灰显。
对应pair-head-plico.png、pair-head-plico-open.png、pair-head-same-plico.png已查看，
与参考截图均保存artifacts/collaboration-workflows-20261024。菜单图标尚有差异。
附加参考验证：HEAD+当前分支多选后Esc关闭菜单、F2没有打开重命名，截图
pair-head-f2-reference.png。实读BranchesDashboardTreeController.kt::snapshotSelectionActionsKeys
仅selectedNodes.singleOrNull时提供单分支action数据，GitRenameBranchAction.kt
继承GitSingleBranchAction。本地F2仍取legacy lead，下一步补单选约束；Enter与AltF1
尚需各自验证，不能一概禁止。阶段5仍在推进，Changes工具栏和后续阶段未删减。

### 分支多选快捷键第一步（2026-10-25）
依据上节原生F2验证及4aec352 BranchesDashboardTreeController::snapshotSelectionActionsKeys、
GitRenameBranchAction/GitSingleBranchAction，tree_key在多行选择时不再取lead执行F2重命名。
103桌面测试通过，Windows构建54.18s，日志/tmp/plico-branch-f2-{tests,build}.log；
仅正常关闭自有25288，启动46232。原生Ctrl选择conflict-main/topic后F2无弹窗，
再普通点击topic，F2正常打开重命名框，最后Esc取消，未写Git。截图
branch-f2-plico-{multi,single}.png已查看；第一次单选截图尚在加载，等待后重新截图
确认弹窗，未将加载状态当作失败。
继续实测Rebased44688：HEAD+renamed-untracked多选按Enter，Log标题和结果过滤为
HEAD,renamed-untracked，选择仍保留，截图branch-enter-ref-{before,after}.png。
实读BranchesDashboardActions::UpdateBranchFilterInLogAction、
BranchesDashboardTreeController::updateLogBranchFilter、BranchesTreeSelection::selectedBranchFilters：
收集所有Branch与HEAD，忽略tag/group，传全部分支过滤，不取lead。导航action使用
logNavigatableNodeDescriptor的firstNotNullOfOrNull，亦不同于最后点击项。
本地目前Review.branch/Repository.snapshot_filtered/history::load仍单Option<String>入口；
下一步必须贯穿分页、计数、graph roots与过滤显示实现多分支并集，不能编码为逗号字符串
或仅对已加载列表过滤。Enter/AltF1这两项保持待修复；Changes面板细节及后续阶段仍在计划内。

### 多分支过滤查询层（2026-10-25）
依据上一轮原生Enter多选过滤截图，继续实读参考4aec352
plugins/git4idea/backend/src/log/GitLogProvider.kt::getBranchLikeFilterParameters：
遍历本地/远端分支及HEAD，将匹配的所有名字作为日志起点；无分支/修订/range过滤
才走LOG_ALL。采用分支可达提交并集，不是把多个名字拼成一个revision或做交集。
本地git.rs::snapshot_branches_filtered_at与history.rs::{read_branches,read_branches_filtered}
新增集合接口，旧单分支入口委托新接口，保持现有调用行为。引用元数据仍只读一次，
从同一快照解析全部选中分支，OID去重后作为graph/history/count共同roots；
后续分页fixed_tips优先，刷新才重新解析引用；不为每个分支追加Git查询。
保留本地差异：选中引用消失继续报错，用户刷新/重选，不静默扩大成全仓库历史；
hash查找覆盖其他过滤的已有优先级不变。
新增tests/repository.rs::multiple_branch_history_uses_one_union_for_pages_filters_and_counts，
真实临时仓库覆盖分叉并集、共同祖先去重、排除第三分支、两页拼接、文本/路径与count
一致、分支移动后的fixed与fresh区分、HEAD重复去重、选项形引用拒绝。该测试通过，
日志/tmp/plico-multi-branch-core-tests.log。当前仅查询层接入，Review.branch仍单值；
下一步替换界面分支过滤状态，并同步branch菜单、清除、导航、重命名与仓库切换。
不得宣称多选Enter已完成；阶段5及阶段6–8目标保持完整。
查询层回归：repository history筛选14项、filter筛选13项均通过（两组有重叠，
不累计声称27项独立测试）；fmt与diff whitespace通过。日志分别为
/tmp/plico-multi-branch-history-regression.log、/tmp/plico-multi-branch-filter-regression.log。
本轮未修改界面，尚未构建此查询层变动的Windows二进制，待界面接入后共同原生验收。

### 多分支过滤界面接入（2026-10-25）
沿用已验证4aec352 BranchesTreeSelection/BranchesDashboardTreeController/GitLogProvider规则，
Review.branch替换为branch_filter集合；Enter收集全部已选Branch/HEAD（忽略tag/group）
并保留分支树选择。单分支菜单仍将集合替换为一项，All/清除将集合清空。
repository.rs的普通/过滤history请求接入集合API；count继续使用返回的固定tips。
导航新查询解析完整集合，清除过滤的导航与仓库切换清空集合；branch_rename更新所有
命中的过滤引用，Log高亮和清除按钮以集合非空判断；菜单显示各分支短名，tooltip完整引用。
补GPUI测试覆盖应用多个分支忽略tag、保留选择、清除和替换单项。完整桌面103项通过，
补断言后定向selection测试亦通过；fmt、diff whitespace通过，Windows release成功。
日志/tmp/plico-branch-filter-ui-{tests,build}.log、/tmp/plico-branch-filter-selection-tests.log。
原生Rebased44688再次实测conflict-main/topic Enter，保留多选、显示5条，截图
branches-filter-reference.png；Plico正常关闭46232，启动16132复现相同操作，
保留多选、显示5/5，点过滤右侧X后恢复15/15，截图branches-filter-plico{,-clear}.png
已查看并存于artifacts/collaboration-workflows-20261024。首次捕获处于后台加载中，
后续截图确认完成，未将旧列表当作结果。
保留视觉差异：Rebased在Log Tab标题显示过滤分支，Plico目前在分支过滤控件显示；
大量分支的标签截断、Tab标题联动仍待细化。AltF1选择优先级、空选择处理以及
Changes工具栏/根分组等继续待对齐；重命名与导航的多分支组合尚未原生写操作验收。
阶段5仍未结束，后续阶段6–8保留，未提交。

Changes 工具窗口分组前置（2026-10-25）：原生 Rebased 1.1.20 /
IC-262.10968.SNAPSHOT，44688，conflict-main/topic 固定比较。已操作眼睛菜单、
Directory 关闭/开启及 Collapse All；截图 changes-{options,flat,collapse}-reference.png
保存在 artifacts/collaboration-workflows-20261024 并已查看。菜单只有 Group By /
Directory，平铺时无根组、文件名后跟灰色目录；分组时根组显示文件数，折叠保留根组。
源码 4aec352 的 platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/
ChangesBrowserBase.java:createToolbarActions/createLastToolbarActions 依次添加 ShowDiff、
分隔符、分组选项；SimpleChangesBrowser.java:buildTreeModel 按 grouping 构建；
shared/src/com/intellij/openapi/vcs/changes/ui/ChangesTree.java:expandAll 委托树展开。
platform/vcs-impl/src/com/intellij/vcs/CompareWithLocalDialog.java:MyRefreshAction.update
仅 LocalContent 非 NONE 显示刷新。计划在 src/compare_view.rs 补分组、根组、右侧
展开收起按钮及图标入口，固定版本比较不显示刷新。实现后原生验收另记。

Changes 工具窗口本轮实现与验收：src/compare_view.rs 增加分组切换、根组文件数、
展开/收起全部及平铺文件名后的目录；选中文件按原 change index 保留，Diff 定位会
展开根及祖先目录。Show Diff 改为图标，固定比较隐藏刷新，工作区比较保留刷新。
新增已有真实仓库测试中的分组切换/折叠/恢复定位断言；compare_view::tests 两项
通过（/tmp/plico-changes-tree-tests.log），Windows release 构建通过
（/tmp/plico-changes-tree-build.log）。正常关闭自有16132后更新测试副本，
新14536/15533636；独立fixture Ctrl选 conflict-main/topic→显示文件差异，
实际点击折叠及眼睛菜单 Directory；已查看 changes-{grouped,collapse,options,flat}-plico.png，
保存同 artifacts 目录。平铺不显示根组、展开收起禁用、文件名后跟灰色仓库名，
与本轮参考行为一致。未运行CI。保留差异/下轮项：目录图标当前被Icon遮罩填满，
应沿用文件树img渲染；菜单尚未显示Ctrl+Alt+P，键绑定已接入但原生尚未验；
目录键盘选择仍未接入；嵌套目录压缩及分组持久化尚待继续对照。本轮不宣称整体完成。

Changes 图标/快捷键修正依据：沿用上轮已验的 changes-options-reference.png 与
changes-collapse-reference.png；Rebased4aec352 ChangesBrowserBase.java 在 init 中
对 toolbar group 调用 DiffUtil.recursiveRegisterShortcutSet。目录项绑定实际
ToggleDirectory action，并向 PopupMenu 提供文件区 focus context 解析快捷键，
不硬编码提示字符串；目录 SVG 使用 img 保留原有轮廓/填充。继续原生核对键盘：
44688同一fixture，选根组 Right展开、Down文件、Left回父目录，截图
changes-parent-reference.png；目录键盘实现仍待读取树 action 源码后推进。
后续导航源码已读：platform/platform-impl/src/com/intellij/ui/tree/ui/TreeAction.java
selectChild：折叠目录只展开，已展开或叶子转下一可见行；selectParent：展开项先折叠，
否则选父路径；select按可见row设选择并scrollToVisible。ChangesBrowserBase.java
canShowDiff/showStandaloneDiff 经 VcsTreeModelData.getListSelectionOrAll 取目录后代；
目录选择不能沿用之前文件的index作为Diff目标。后代列表具体语义下一轮继续读取。

图标/快捷键本轮结果：Windows原生确认轮廓文件夹和Ctrl+Alt+P菜单提示，
changes-polish-plico.png（42876）及 changes-shortcut-plico.png（最终49104/8456440）。
快捷键处理范围提升到CompareView根，覆盖工具栏子焦点；定向测试实际派发两次
ctrl-alt-p验证group_files关闭/开启，2项通过（/tmp/plico-changes-key-tests.log）。
完整桌面103项、发布配置导航9项通过（/tmp/plico-changes-focus-tests.log、
/tmp/plico-changes-navigation-tests.log），Windows构建通过，未运行CI。
实机快捷键验收仍未通过：SendKeys及keybd_event按Ctrl/Alt/P均未切换；
同样按键在Rebased44688也未切换（changes-shortcut-reference.png），故尚不能
将失败归因于Plico焦点，可能是环境/按键注入；先保留实际菜单切换可用结果，
不声称原生快捷键验收成功。没有修改系统快捷键、IME或用户设置。
VcsTreeModelData.java（shared路径）SelectedData遍历目录后代去重；
getListSelectionOrAll对多条选择保留范围，单条/无选择扩展全部并选当前索引。
这是下一轮目录键盘选择和Diff链范围的依据，不直接沿用旧文件索引。

Changes目录导航前置（2026-10-25）：44688原生同一conflict-main/topic比较，
根目录Enter折叠而非打开Diff；点击ShowDiff打开shared.txt，截图
changes-directory-diff-reference.png。上轮Right/Down/Left回父目录截图继续有效。
本轮重新读取4aec352 TreeAction.java:selectChild及VcsTreeModelData.java:
getListSelectionOrAll；新增读取shared/.../ChangesTreeHandlers.kt:MyEnterListener，
单选非叶节点不调用Diff handler。计划CompareView独立保留目录身份，按可见行
处理方向键；Enter目录折叠/展开，ShowDiff按目录后代生成范围且不受折叠影响。

Changes目录导航实现验收：CompareView增加独立selected_folder、按可见行的
上下/左右导航、Enter目录展开折叠、三角展开命中区、列表选择定位。ShowDiff
按未折叠完整树顺序构造Chain子集：目录多后代限制范围，单后代保持全部导航范围。
刷新时复验目录存在，避免旧目录选中状态误用文件index。两项真实仓库测试覆盖
根目录导航/Enter及Diff首文件和链长度，完整桌面103项通过；发布配置导航9项
结果见/tmp/plico-changes-directory-navigation-tests.log。Windows release构建通过。
正常关闭自有49104，更新后20284/4196442；原生在conflict-main/topic比较页
选根目录后Left/Right/Down/Left、Enter折叠、ShowDiff打开shared.txt均已截图查看，
changes-directory-{parent,enter,diff}-plico.png保存同artifacts目录。
仍待验收/完善：嵌套多文件目录范围原生验收、折叠全部后的选择归并、目录选择切换
平铺时的选择迁移、紧凑路径与分组持久化。未进行滚动自动化，未运行CI。

Changes嵌套目录前置（2026-10-25）：Rebased44688独立fixture，搜索preview-navigation-fixture，
与HEAD(renamed-untracked)多选ShowFilesDiff；根直接显示preview-nav，子目录nested含
beta/gamma，旁边alpha。选nested后ShowDiff打开beta且导航1/2。Directory关闭后
beta/gamma两项同时保持选中，并非随便选第一文件；截图changes-nested-{reference,
scope-reference,flat-reference}.png（首项实际changes-nested-reference.png）。
已读4aec352 shared/.../DirectoryChangesGroupingPolicy.kt:getParentNodeRecursive创建
完整父目录辅助节点，TreeModelBuilder.java:collapseDirectories/collapseParentWithOnlyChild
合并只有一个非叶目录子项的路径。本地计划CompareView改用compact_rows，若共同目录
涵盖全部文件，不再额外插入空路径根；有根级文件时保留仓库根。目录分组切换的多选
迁移需要独立选中文件集合模型，当前不得宣称保留目录的多文件选择已完成。

Changes嵌套目录原生验收（2026-10-25）：确认104项桌面测试全部通过
（/tmp/plico-changes-nested-full-tests.log）。复用自有Plico46268/50926048，清除分支搜索，
多选preview-navigation-fixture与HEAD并ShowFilesDiff。changes-nested-plico.png确认
preview-nav直接作为根且显示3个文件，nested显示2个文件，没有额外空根。
点击nested折叠箭头再点击ShowDiff，changes-nested-scope-plico.png确认beta.txt内容
及1/2文件导航，折叠不丢失目录范围。两张截图已查看并存入
artifacts/collaboration-workflows-20261024，与上述Rebased三文件场景对照。
当前仍待处理Directory切换后保留多文件选中集合；另观察到分支搜索时Plico隐藏HEAD，
参考同查询保留HEAD，列为后续行为差异。本轮仅复用现有一个Plico和一个Rebased。

Changes分组切换选择迁移（2026-10-25）：沿用上述原生nested目录关闭Directory后
beta/gamma共同选中的截图证据。进一步实读4aec352的ChangesTree.java：
onGroupingChanged调用rebuildTree(KEEP_SELECTED_OBJECTS)，KeepSelectedObjectsStrategy
保存selected(tree).userObjects，恢复时setSelectedChanges匹配新树节点对象。
本地CompareView以selected_files路径集合保留目录后代，切换平铺不缩成一个文件；
恢复目录时展开所选文件祖先，Diff多选限制到集合内，单文件仍沿用完整chain规则。
刷新按路径保留仍存在的选择，选另一个文件/目录或外部Diff导航时清除旧集合，
加载错误清空集合。保留差异：此轮不提供任意Ctrl/Shift多选，仍列为后续行为对齐。
嵌套目录回归补充两次分组切换、两文件范围和祖先展开断言，专项3项通过。
Windows原生多行高亮及切换后Diff范围待新构建验收，不宣称本轮视觉已验收。

分组选择迁移原生验收（2026-10-25）：104项桌面测试通过
（/tmp/plico-selection-group-full.log），Windows release构建通过
（/tmp/plico-selection-group-build.log）。正常关闭自有46268后更新验证副本，
新Plico42564/22547824复用同一三文件fixture。选中折叠的nested，点击眼睛菜单Directory：
平铺beta/gamma两行共同高亮；ShowDiff打开beta，导航1/2，排除alpha。
再次点击Directory恢复树形，nested自动展开，beta/gamma继续高亮，Diff范围仍1/2。
截图changes-selection-{flat,diff,tree}-plico.png均已实际查看并存入
artifacts/collaboration-workflows-20261024。鼠标菜单路径验收通过；本轮SendKeys菜单
Down/Enter未触发切换，改用实际鼠标命中，未将该注入结果当成原生快捷键验收。
仍只有一个自有Plico和原Rebased44688，未提交、推送或运行CI。

分支搜索保留HEAD（2026-10-25）：复查原生Rebased1.1.20截图
changes-nested-search-reference.png，preview-navigation-fixture查询下HEAD仍在顶端。
该截图已复制并查看于artifacts/collaboration-workflows-20261024。
实读4aec352的plugins/git4idea/backend/src/ui/branch/dashboard/BranchesTree.kt：
FilteringBranchesTreeBase.getText仅为Ref/Repository/RemoteGroup/Group提供匹配文本，
其它节点返回null（注释明确始终匹配）；BranchesTreeModel.kt中Head是独立descriptor，
不是Ref。BranchesDashboardTreeComponent.kt将同一过滤树接入Log分支搜索。
本地src/ui/log_branches.rs::head_in_tree去掉名称查询约束，仍要求head_id存在；
视图和键盘/多选行索引共用该判断。新GPUI测试覆盖无匹配搜索下HEAD行、detached
保持可导航、unborn不出现HEAD。分支选择专项2项通过；Windows新构建原生验收待做。

HEAD搜索原生验收（2026-10-25）：105项桌面测试通过，Windows release构建通过
（54.79s，/tmp/plico-search-head-{full,build}.log）。正常关闭自有42564更新构建，
新48976/4458586搜索preview-navigation-fixture后HEAD仍在顶端；选择匹配分支并
Ctrl选HEAD，右键ShowFilesDiff打开preview-navigation-fixture与rename-filtered比较，
正确显示alpha/beta/gamma三文件，搜索条件与双方选择保持。截图head-search-plico.png
及head-search-comparison-plico.png已查看并保存artifacts/collaboration-workflows-20261024。
此验证覆盖搜索到两分支比较的实际入口；没有改动用户仓库或新增常驻测试实例。
第5项仍有多选/空选择和其它已列边界待对齐，第6—8项保持原计划，不以此次小项验收
宣称整个协作工作流目标完成。

分支空选择前置与修复（2026-10-25）：原生Rebased44688切Log，搜索仍为
preview-navigation-fixture，先单击该分支再Ctrl单击取消，按Enter后Log仍为
conflict-main/topic过滤且5条结果；branch-empty-reference.png已查看并保存artifacts。
实读4aec352 BranchesDashboardActions.kt::UpdateBranchFilterInLogAction.update要求
selectedBranchFilters非空；NavigateLogToSelectedBranchAction读取实际selection的
logNavigatableNodeDescriptor，无对象则不执行。
本地select_branch_row取消最后一项后仍保存旧selected_tree_ref，Enter又通过fallback
恢复它，后续Ctrl选择也可能重新加入旧行。本轮将legacy当前项约束为实际集合中一项：
被点行仍在集合则保留，否则取剩余项，空集清空。已有过滤不因空选择改变。
补测Ctrl取消最后一项、Enter保持既有filter、再次Ctrl选择不复活旧分支。
保留差异：多选Alt+F1首项规则尚未本轮调整；Windows本地空选择原生验收待新构建。

分支空选择Windows验收（2026-10-25）：release构建及fmt检查通过，正常关闭自有48976，
更新验证副本启动33120/52302304。展开分支树，单击preview-navigation-fixture再Ctrl
单击取消，按Enter后没有选中分支，Log保持未过滤15/15；随后Ctrl单击conflict-topic
只选中该分支，旧preview分支没有恢复。截图branch-empty-{plico,next-plico}.png
已查看并保存artifacts/collaboration-workflows-20261024。审阅tree_key确认F2/Alt+F1
在selected_tree_ref为空时无操作，右键先context选中命中行再构造菜单。
专项回归2项通过（/tmp/plico-empty-selection-tests.log），构建记录
/tmp/plico-empty-selection-build.log；没有扩大为所有键盘路径已原生验收的结论。

多选分支导航顺序（2026-10-25）：Rebased44688先选择preview-navigation-fixture，
再Ctrl选HEAD。Alt+F1本机实际打开通用Select In菜单（branch-navigate-reference.png），
因此不能把它当已验证的Log导航。关闭菜单后点击分支工具栏Locate，出现
“Commit or reference 'preview-navigation-fixture' doesn't match the filters”提示，
证明目标是先选分支，不是最后点击或视觉首行HEAD；branch-navigate-action-reference.png
已查看并保存artifacts。实读4aec352 BranchesTreeSelection.kt：selectedNodes来自
selectionPaths，logNavigatableNodeDescriptor取firstNotNullOfOrNull，跳过非ref节点；
BranchesDashboardTreeController.navigateLogToRef委托navigateTo(selection,true)。
backend.xml为该action声明use-shortcut-of=SelectIn，与本机实际快捷键分派差异单独保留。
本地log_selection::Selection补充加入顺序，ordered仍维持已有视觉排序契约，新增
first_matching供导航使用；删除/刷新/重选同步清理顺序。log_branches的Alt+F1选首个
可导航实际节点，不再选最后点击项。快捷键保留现有产品入口，不新增通用Select In。
新增两种HEAD/分支选择顺序测试，以及顺序在右键、取消、重新加入、刷新时的核心测试。
Windows本地导航验收待构建；未将源码或专项测试替代原生验收。

导航顺序共享模型验证补充（2026-10-25）：完整核心285项和桌面105项通过
（/tmp/plico-selection-order-{core-full,full}.log）。检查发现顺序集合若在每次remove
时retain，会让Ctrl+Shift批量取消产生重复扫描；改为每个select手势结束后统一retain，
保持集合删除O(1)且整次顺序清理O(n)。扩展既有批量取消测试，要求first_matching同样
为空；5项选择核心测试通过（/tmp/plico-selection-order-prune.log）。原生构建更新为
本次最终代码后再验收，不使用修改前的成功构建冒充当前产物。

导航顺序原生验收（2026-10-25）：最终Windows构建通过
（/tmp/plico-selection-order-final-build.log），关闭自有33120后更新并启动43076/45812814。
先选preview-navigation-fixture再Ctrl选HEAD，Alt+F1选中Preview navigation three files提交；
反过来先HEAD再Ctrl选preview，Alt+F1选中当前rename-filtered的feature提交，右侧详情
显示HEAD。双方分支多选保留。截图branch-navigate-{plico,head-first-plico}.png已查看并
存入artifacts/collaboration-workflows-20261024。本机Rebased快捷键分派差异沿用上段记录；
此次对齐的是已通过其Locate动作确认的首选路径规则。格式与diff检查通过，未提交。

### 第5项交付范围收口检查（2026-10-25）

按4473节原范围检查，不把逐次UI细化记录替代功能交付证据：
- 任意两分支独有提交：Pair双向范围、分页/过滤集成测试，以及此前两分支原生菜单与
  双日志截图已覆盖；HEAD/搜索/空选择/首个选择导航补充见最近各段。
- 整体文件差异：Pair.changes比较固定两棵树；新增真实Git测试覆盖远端跟踪分支与
  本地分支、重命名、二进制、反向新增/删除，不把逐提交补丁累加视为净差异。
- 方向交换/空结果/引用刷新：comparison_is_symmetric_paged_and_pinned_until_refresh
  及桌面reference_refresh_keeps_applied_filters_and_recovers_deleted_reference已覆盖，
  原生反向/空结果/刷新记录见前文；不把测试覆盖扩成全部生命周期已验收。
- 只读保护：新增remote_comparison_reads_renames_and_binary_without_touching_dirty_state
  在暂存与未暂存内容不同且存在未跟踪文件时，逐字节核对index、工作文件及未跟踪文件，
  并核对show-ref和symbolic HEAD。重命名预览只读提交base内容，binary明确返回不支持
  文本预览，反向比较端点互换。9项branch_compare集成测试通过
  （/tmp/plico-stage5-readonly.log）；未修改生产行为，无新增UI需要截图。

本轮再次读取4aec352 plugins/git4idea/backend/src/ui/branch/GitCompareBranchesHelper.java
（formatLogCommand的双点范围）、platform/dvcs-impl/src/com/intellij/dvcs/ui/CompareBranchesHelper.java，
沿用前文已实读的loadTotalDiff/固定范围规则。没有将这些入口误称为binary渲染实现来源。
尚不能提交为整个第5项完成：独立比较列表任意Ctrl/Shift文件多选、折叠全部的选择归并、
以及此前记录的保存/删除生命周期和按需Diff类型边界仍需逐项确认；精确颜色/标题等视觉
差异独立记录。第6—8项保留原计划，不移除、不宣称已完成。

Changes文件行多选前置与实现（2026-10-25）：原生Rebased44688平铺三文件，单击alpha
再Ctrl选gamma后ShowDiff为alpha、1/2；重新选alpha再Shift选gamma后为alpha、1/3。
changes-{multi,range}-reference.png已查看并保存artifacts/collaboration-workflows-20261024。
实读4aec352 ChangesTree.java::setSelectionMode以及
platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ChangeViewDiffRequestProcessor.java
的getGroupOrMultiSelectionListSelection/getSelectionStrategy：显式2+选择限制导航集合，
单项回到全体/同组范围，空选择依processor策略处理。
本地CompareView::select_file接入文件行鼠标Ctrl/平台键切换、Shift范围，按Path保留
anchor与集合，范围按当前显示行顺序，不受文件原始Git顺序影响。分组切换沿用集合迁移；
刷新删除anchor时清理；取消最后文件后ShowDiff不再回退到不相关首文件。
扩展GPUI回归覆盖不连续两文件、连续三文件及空选择；专项3项通过。
本轮是文件行多选第一步，目录行Ctrl/Shift、键盘Shift扩选和多选预览同步仍待补齐，
Windows本地多选原生验收待构建，不宣称整个多选已完成。

比较列表预览范围同步修正（2026-10-25）：复核此前自有 Rebased44688 的原生操作：
平铺列表选 alpha 后 Shift+Down，alpha/beta 高亮，已打开的 alpha Diff 保持内容并更新
为 1/2 files。截图 changes-keyboard-range-reference.png 已查看并归档同一 artifacts 目录。
参考仍为 4aec352，重新实读 ChangeViewDiffRequestProcessor.java 的
getGroupOrMultiSelectionListSelection/getSelectionStrategy：显式多选限制导航集合。
CompareView::selection_changed 与 refresh 改为使用 diff_selection 的实际选择范围，
不再发送完整 chain；editor_tabs 复用 DiffTab::sync_source_selection，同一目标仅更新
导航链，不重载编辑器。新增 unchanged_preview_updates_navigation_without_resetting_view
回归核对导航范围、generation 与阅读位置保留。CompareView 三项专项通过；新回归结果
另记。本地 Windows 多选及预览同步原生验收仍待完成，不作为整项交付完成声明。
新回归通过（/tmp/plico-preview-scope-regression.log）：测试使用真实比较入口的 combined
revision chain。初版测试误用普通未暂存 chain，该类型不支持 fork_for，已修正测试场景，
未为使测试通过而扩大普通工作区 chain 的契约。原生验收与目录/键盘多选仍继续跟进。

键盘范围选择源码核对（2026-10-25）：实读同一 4aec352 的
platform/platform-impl/src/com/intellij/ui/tree/ui/TreeAction.java：lineUp/lineDown 从 lead
可见行移动，ExtendSelection 不循环；select 使用 anchor 的可见行设 selectionInterval，
恢复 anchor，更新 lead；失效 anchor 回到单选。目录也是可见行，不能直接跳过目录或
把每次 Shift 步进当作单选再扩展。本地当前文件集合不足以表达多个目录的行选择与 lead，
后续需要独立行选择状态，再投影到 Diff 文件集合；没有用仅支持平铺的快捷键冒充完成。

文件多选与同目标预览原生验收（2026-10-25）：Windows release 构建通过
（/tmp/plico-preview-scope-windows-build.log）。正常关闭自有43076，替换测试目录产物，
启动22324/1443514；HEAD 与 preview-navigation-fixture 比较，平铺后打开 alpha Diff，
Ctrl选 gamma：alpha/gamma 高亮，仍显示 alpha 内容，计数即时为1/2。单选 alpha 后
Shift选 gamma：三文件高亮，仍显示 alpha，计数1/3。截图 scope-{multi,range}-plico.png
已查看并归档 artifacts/collaboration-workflows-20261024，与此前 Rebased 对应文件多选
截图一致。此项仅确认鼠标文件多选和预览范围同步；键盘/目录混选未完成，未宣称整个
第5项交付完成。保留一个自有 Plico 和一个 Rebased，没有新增多余实例，未提交。

目录混选与范围收缩前置（2026-10-25）：自有 Rebased44688，打开分组后选 nested，
Ctrl点 alpha：仅 nested/alpha 行高亮，Diff beta 为1/3；选 nested，再 Shift+Down、
Shift+Up：回到仅 nested 行高亮，beta 为1/2。截图 directory-mixed-reference.png 和
 directory-range-shrink-reference.png 已查看并归档。首次菜单切换时一次点击被窗口守卫
拒绝，未将该点击视为成功；菜单关闭后重新执行上述操作取得有效截图。
实读4aec352 platform/platform-impl/src/com/intellij/ui/tree/ui/TreeAction.java（上节），
以及 platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ui/VcsTreeModelData.java：
ExactlySelectedData取selectionPaths，SelectedData遍历所选节点的子树并unique。因此
行高亮、lead/anchor与Diff文件投影必须分离。
新增 tree_selection.rs 类型化 File/Directory 身份，保留原生Path编码，支持目录与文件
混选、可见行范围、反向收缩、边界不循环、失效anchor退回目标及刷新清理。此为比较列表
状态迁移的准备代码，尚未接入CompareView，不能将核心测试等同于已交付键盘操作；
Ctrl+Shift组合与折叠选择归并仍需进一步核对。既有UI行为本轮未改变。
选择核心专项3项通过（/tmp/plico-tree-selection-core.log），格式及diff检查通过。下一步
将CompareView迁移到该行选择模型，移除旧的仅文件anchor/目录单选状态，再做原生验收。

CompareView 行选择模型接入（2026-10-25）：沿用上一节已完成的 Rebased 原生目录混选、
Shift收缩截图及 TreeAction/VcsTreeModelData 源码依据。移除 selected、selected_folder、
selected_files、file_anchor 四份状态，统一为 tree_selection::Selection；高亮读取精确节点，
Diff按includes_file投影后去重，目录参与Ctrl和Shift范围选择。绑定Shift-Up/Down，按可见
行更新lead、保留anchor；分组切换将选择映射到文件并展开已选文件祖先；刷新按完整模型
保留有效节点，失效节点清理，不受当前折叠可见性影响。方向交换同样使用选择投影。
扩展既有GPUI回归覆盖目录+子文件范围、反向收缩及目录+外部文件的Diff集合。专项3项
通过（/tmp/plico-tree-selection-migration.log）。原先分组恢复后断言仍折叠的旧测试与此前
分组迁移原生规则不符，更新为已选文件可见；没有删除刷新/方向交换回归。
此次接入尚待 Windows 原生键盘/目录混选验收；Ctrl+Shift细节、折叠全部的可见选择归并、
空选择预览及Diff反向同步是否保留多选范围仍列为未完成项。
接入后完整桌面测试106项通过（/tmp/plico-tree-selection-desktop.log），diff whitespace检查
通过；没有将测试结果替代本轮待执行的Windows原生界面验收，未提交。

空选择及当前Diff保留规则修正（2026-10-25）：原生Rebased44688取消最后一个nested目录
选择，仍显示beta，范围由1/2恢复1/3；选gamma再Ctrl选beta，仍显示gamma。截图
comparison-{empty,retain}-reference.png已查看归档。实读同4aec352
ChangeViewDiffRequestProcessor.java::refresh：空选择在showAllChangesForEmptySelection=true
时取全量；当前change仍在范围内则保留，否则取首项。此前把空选择理解为需要清空预览
不符合此独立比较窗口，现按实际行为修正。
CompareView新增preview_selection(current)，与显式ShowDiff分开：已有预览空选择恢复全量、
保留仍有效当前文件；显式空选择ShowDiff保持不打开无关文件。SelectionChanged改成通知，
editor_tabs从对应view和当前tab解析选择后复用sync_source_selection。回归覆盖空选择、
当前文件仍在多选内、当前文件被排除三种情况。
参考截图gamma+beta显示1/2，表明多选枚举顺序还有与本地视觉顺序不同的细节；本轮只
修正当前目标与集合范围，选择顺序继续列为待对齐，未宣称导航顺序已经完全一致。
目录模型Windows构建已通过（/tmp/plico-directory-selection-build.log），但该构建发生在
本轮空选择修正之前，未用作当前代码原生验收证据；自有Plico仍为22324，未额外启动实例。
本轮最终比较列表专项3项通过（/tmp/plico-preview-retain-tests-final.log），格式与diff检查
通过；原生最终验收继续待办，当前修正未提交。

多选枚举顺序与Diff反向同步（2026-10-25）：Rebased44688沿用先gamma再Ctrl beta，点击
Diff文件下一项箭头，显示beta、2/2，左侧beta/gamma两行仍高亮。截图
comparison-multi-next-reference.png已查看归档，与前一张gamma、1/2组成连续证据。
再次实读4aec352 VcsTreeModelData.ExactlySelectedData按getSelectionPaths顺序遍历，
SelectedData遍历每个节点子树后unique；ChangeViewDiffRequestProcessor.ChangesNavigatable
在多选时myUpdateSelection=false，select只更新当前change，不改树选择。
本地tree_selection保留选择加入顺序（取消/重新加入/刷新去除同步维护），CompareView
按节点顺序展开文件，目录内部沿树顺序，统一去重；follow_target供Diff导航使用，目标
仍在含多个文件的选择范围时保留树选择。显式select_target仍用于指定定位，避免混淆。
扩展比较测试核对gamma在链首、beta在第二项以及导航后多选保留，专项3项通过
（/tmp/plico-selection-order-compare-final.log）。首次将保留规则加到显式select_target导致
既有定位回归失败，已拆分调用语义后重跑通过；未删除该回归。
多选与键盘最终原生验收（2026-10-25）：Windows release构建通过
（/tmp/plico-directory-order-final-build.log），正常关闭自有22324后更新产物，启动6092 /
7145750。先gamma、Ctrl beta：显示gamma 1/2；点击下一文件：beta 2/2，两文件选择保留。
选nested再Shift+Down：nested/beta行高亮，范围1/2；Shift+Up仅nested高亮，范围仍1/2；
Ctrl取消nested：无选择，保留beta，范围1/3。截图comparison-order-plico、
comparison-multi-next-plico、comparison-directory-{extend,shrink}-plico、comparison-empty-plico
均已查看并归档上述artifacts目录，与对应Rebased状态对齐。4项tree_selection核心测试
通过（/tmp/plico-tree-order-core.log）。保留一个自有Plico和一个Rebased，未提交。
折叠全部选择归并、Ctrl+Shift组合、刷新预览当前文件保留仍需继续核对，不以本轮验收
代替第5项整体完成或后续第6—8项。

折叠全部选择归并（2026-10-25）：自有Rebased44688，beta/gamma多选且当前beta 2/2，
点击Collapse All后仅preview-nav目录可见并高亮，beta保持预览、范围1/3。截图
comparison-collapse-reference.png已查看归档。实读4aec352 DefaultTreeExpander.kt：
collapseAll(tree,true,1)，随后居中显示选择；TreeUtil.java::collapseAll保存lead路径，
折叠后internalSelect(normalize(leadSelectionPath,minCount,keepSelectionLevel))，不是保留
不可见子节点多选。本地CompareView::set_expanded(false)在重建行后将lead归并到最深
可见祖先目录，单选该行并通知预览范围更新；展开全部不恢复已经归并的旧子文件选择。
扩展GPUI回归验证归并目录、Diff范围扩大、当前目标保留，旧测试中折叠后仍保持b.txt
单选的断言按上游规则改为目录投影；方向交换测试显式重新定位b.txt以保持独立覆盖。
此轮Windows本地原生验收待构建，未宣称折叠UI已验收。
折叠归并专项3项通过（/tmp/plico-collapse-selection-tests.log），格式及diff检查通过；
未提交。后续仍保留Ctrl+Shift、刷新预览与第5项其他生命周期，以及第6—8项原范围。
折叠全部Windows原生验收（2026-10-25）：最终构建通过
（/tmp/plico-collapse-final-build.log），正常关闭自有6092，更新并启动37220/2492518。
HEAD/preview-navigation-fixture文件比较，beta/gamma多选且当前beta；点击Collapse All后
只剩preview-nav目录并高亮，beta仍显示，文件计数1/3。comparison-collapse-plico.png
已查看归档，与comparison-collapse-reference.png相符。只保留一个自有Plico和Rebased。
顺带复核editor_tabs的Reloaded监听，目前直接replace_target(event.0)，使用刷新产生的
首项；与同4aec352 ChangeViewDiffRequestProcessor.refresh保留当前change规则仍有差异。
刷新时固定revision身份变化与同路径保留如何匹配尚需原生验证，未把只读源码结论当已
完成实现，也未在本轮修改该路径。第5项保持进行中，后续6—8项范围不变，未提交。

刷新预览按前后路径保留（2026-10-25）：Rebased44688先gamma再Ctrl beta，切换目录分组
重建模型后gamma继续显示（计数随重建顺序变为2/2）。截图
comparison-model-refresh-reference.png已归档；此次原生验证是模型重建，未执行远端引用
变化，不能声称跨提交内容已在原生验证。实读4aec352 TreeHandlerDiffRequestProcessor.kt
的ON_MODEL_CHANGE→refresh(true)，ChangeViewDiffRequestProcessor.refresh/ChangeWrapper.equals，
ChangeListChange.HASHING_STRATEGY和vcs-api-core Change.java::equals：普通Change按before/
after文件路径（含不存在端点）判等，不按revision号，重命名两端路径均参与。
本地CompareView::reloaded_selection重新匹配同仓库的before/after路径，使用新chain的
target，再按当前选择范围解析；editor_tabs的Reloaded改为从当前tab请求恢复位置，仍
replace_target加载新正文，不将模型刷新错误地当作仅更新导航范围。无匹配回退当前
选择首项，空模型/错误保持失效提示。方向交换复用同一事件，但前后路径不匹配时正常
回退，不保留旧端点。新增回归覆盖revision改变而路径相同、当前文件排除、模型清空。
最终专项3项通过（/tmp/plico-reload-selection-tests-final.log），格式及diff检查通过。
本地Windows刷新验收待最终构建；当前自有Plico仍37220/2492518，未替换修改前产物，
未提交，第5项及后续6—8项保持原计划范围。

刷新验收入口与回归（2026-10-25）：完整桌面106项通过
（/tmp/plico-reload-desktop-full.log），分支比较集成9项通过
（/tmp/plico-reload-branch-integration.log），Windows构建通过
（/tmp/plico-reload-final-build.log）。产物尚未替换自有37220，不能视为原生刷新验收。
检查发现Source::Branches独立文件面板不展示刷新，且repository::refresh_impl未触发它。
随后实读4aec352 GitShowDiffWithBranchPanel.kt：init启动一次loadDiffInBackground，
showDiff只应用结果；CompareWithLocalDialog.MyRefreshAction只在localContent != NONE时
可见。因此独立固定版本文件面板无刷新入口与此参考一致，不能直接把添加刷新按钮
当作上游已有行为；第5项的引用刷新交付在双分支日志比较中已有refresh入口与测试。
工作树比较的Refresh事件仍是本次Reloaded修正的原生验收入口，后续通过它验收当前文件
保留；若扩展独立分支文件面板为可刷新，需明确记录为本地产品差异，不混同源码还原。
本轮未新增不具参考依据的按钮，未提交；第5项及6—8项原目标继续保留。

目录分组与现有Diff导航同步（2026-10-25）：复用此前本场景的Rebased实机记录，
重新查看comparison-model-refresh-reference.png：先gamma再Ctrl beta，分组重建后
仍为gamma，导航2/2。重新读取参考4aec352的TreeHandlerDiffRequestProcessor.kt
中TREE_MODEL_PROPERTY→ON_MODEL_CHANGE，确认模型重建会同步预览。
发现本地CompareView::toggle_directory重建选择顺序后只notify视图，遗漏
SelectionChanged；现在通过已有preview_selection/sync_source_selection同步导航链，
不重新加载内容或重置编辑器阅读位置。回归补充当前gamma保留且导航索引变1、范围2。
compare_view专项3项通过（/tmp/plico-group-preview-tests.log），fmt与diff检查通过，
Windows release通过（/tmp/plico-group-preview-build.log）。正常关闭自有37220，
更新验证副本后启动44076/62522844；按同样顺序选择gamma、Ctrl beta、Show Diff、
关闭目录分组，截图comparison-group-sync-plico.png已查看并归档，gamma正文保持，
导航2/2、两文件仍选中，与参考一致。只保留此自有Plico和Rebased44688。
此验收证明分组重建同步，不替代工作树比较刷新正文的待验收场景；第5项继续，
第6—8项范围不变，未提交或推送。

工作树比较刷新原生验收（2026-10-25）：在自有Plico44076中，preview-navigation-fixture
右键→与工作树比较差异；选local.txt，Ctrl选beta.txt，打开Diff。备份自有fixture的
local.txt字节后在磁盘追加Native refresh retention check，点击比较面板Refresh。
截图working-refresh-retain-plico.png已查看归档：仍为local.txt，正文显示新增行，
local.txt与beta.txt仍选中，导航1/2。验证后原样恢复fixture字节；未修改项目仓库。
此结果补齐前述Reloaded工作树正文刷新验收；跨revision路径匹配由专项测试覆盖，
不把本场景声称为远端引用刷新实测。仍只保留自有44076和参考44688。

### 第6项子模块：源码预读（2026-10-25，尚未开始实现）

实读4aec352 plugins/git4idea/backend/src/update/GitSubmoduleUpdater.kt：
从父仓库执行submodule update --recursive -- <相对子模块路径>，isSaveNeeded=true，
不使用普通分支的isUpdateNeeded逻辑；命令失败返回ERROR，取消返回CANCEL。
实读GitSubmoduleTest.kt前180行：detached子模块通过submodule update跟随父指针，
但在分支上的子模块按普通仓库更新，不应全部强制detached；包含本地变更可见性测试。
实读GitComplexSubmoduleTest.kt前155行：嵌套fixture用update --init --recursive初始化，
以及依赖顺序测试。测试fixture使用init不证明生产UI存在独立初始化入口；所读Updater
也没有sync URL，所以当前不能把“初始化/同步/更新”三个按钮宣称为直接复刻上游。
本地目前只有clone recurse_submodules选项，尚无独立子模块管理模块。下一步需完成
原生Rebased子模块项目操作及生产入口核对，按计划保留补初始化、URL同步、嵌套和
本地修改保护范围；若上游无入口，记录产品扩展与理由，而非删除既定交付。

### 第6项子模块：原生更新、部分失败与重试（2026-10-25）

自有fixture submodule-reference-80s4zk56/project，父/子均本地bare远端，使用Windows Git
创建。父本地main仍为add child，远端为advance child；modules/child初始detached在
8d4dc065，父新指针cf261c3f。fixture生成脚本/tmp/plico-submodule-fixture.py，路径清单
/tmp/plico-submodule-fixture.json。自有Rebased14476/1640714，1.1.20；信任仅本fixture，
取消默认Defender排除选项。未改用户系统安全配置。

原生操作：Find Action搜索submodule，仅看到递归clone设置结果（截图
submodule-search-reference.png；输入法候选遮挡部分文字，不能据此断言所有入口不存在）。
Ctrl+T出现Update Project，Merge/Rebase单选、Don't show again、OK/Cancel，截图
submodule-update-reference.png。确认Merge后父仓库成功推进，子模块因file协议限制
失败；Project Partially Updated通知截图submodule-updated-reference.png。只读取本fixture
日志，确认依次merge origin/main及submodule update --recursive -- modules/child，
后者ERROR；git submodule status为+8d4dc065，故不能把通知当全部成功。
仅为此独立fixture子模块设置本地protocol.file.allow=always，点击顶栏Update重试并确认。
最终子模块cf261c3f/child two且保持detached，父记录匹配；截图
submodule-retry-final-reference.png。所有截图已查看并归档同artifacts目录。

再次实读4aec352 GitUpdateProcess.java的collectDetachedSubmodules、updateImpl和
 defineUpdaters：父更新和子更新组合结果，子模块即使父已最新仍可更新；嵌套detached
父已进入递归更新时跳过单独子更新，isSaveNeeded纳入GitPreservingProcess；失败不盲目
恢复本地修改。生产入口实机已确认是Update Project，独立init/sync入口仍未确认。
本地映射要求：不要只在父Pull有新提交时才更新子模块；提供部分成功/失败信息及重试，
绝不强制覆盖子模块修改。尚未执行dirty、未初始化及URL变更实机场景，尚未实现第6项。
本次新增参考实例验收后关闭，保留原44688和Plico44076；第5项未提交，6—8项目标不变。

第6项只读清单基础（2026-10-25）：沿用上一节原生项目中父/子仓库识别及detached
显示的前置记录；新增实读4aec352 repo/GitModulesFileReader.kt、GitSubmodule.kt与
GitSubmoduleInfo.kt。上游从.gitmodules取得同时具有path/url的声明，再通过仓库管理器
识别直接子模块，缺失字段不成为可操作对象。
本地src/submodules.rs用git config --no-includes --null --list解析Git配置，按声明路径
查询ls-files --stage -z，保留gitlink提交或冲突状态，不启动子仓库扫描、不加载正文。
不把NotTracked当作未初始化；初始化状态需要后续探测实际子仓库。声明中的URL可能
含凭据，结构不实现Debug，不能直接显示或记录URL。对重复路径、越界相对路径、
非普通.gitmodules文件明确报错；这是本地写操作前清单约束，上游reader较宽松，
保留差异以避免后续操作目标不明确。配置include不展开，与只读本文件的上游一致。
本阶段尚无UI和写操作，未声称子模块功能交付。

真实Git集成tests/submodules.rs四项通过（/tmp/plico-submodules-inventory-tests.log）：
声明与gitlink映射、缺失字段/重复和越界路径、带制表符/换行路径、unmerged阶段；
并逐字节确认index不变、HEAD不变、不创建子目录。子目录入口用例首次暴露cwd导致
NotTracked误报，已改为在解析出的工作树根执行查询，回归通过。格式和diff检查通过。
待实现：初始化状态/dirty探测、递归操作、认证取消/本地变更保护、UI及原生验收。
此前第5项的保存/删除生命周期边界仍保留待收口，不以第6项基础代码冒充已完成第5项。

### 第5项功能阶段收尾（2026-10-25）

补齐剩余工作树编辑生命周期的Windows原生验收：自有Plico44076/62522844，独立
rebase-branch-plico-20261024。查看local.txt比较时从磁盘删除该文件，编辑器保留
已读取内容并展示磁盘不可用提示；点击Tab关闭按钮正常返回Log，没有卡死或自动
重建文件。截图comparison-deleted-plico.png与comparison-deleted-closed-plico.png。
恢复原始字节后重新打开比较，在工作侧追加Native comparison save check并Ctrl+S，
磁盘确认保存，index SHA256保持不变；截图comparison-saved-plico.png。已查看并
归档上述截图，最后恢复fixture原始内容。没有改变该场景的产品行为，沿用既有
共享文档保存及读取错误处理；不将此正常保存验证扩成所有并发写入场景。
固定baseline/双方向/BOM与CRLF/删除后拒绝重建/取消由branch_compare集成覆盖；
共享文档保存屏障、外部变更及Tab/F7生命周期由完整核心和桌面回归覆盖。

本轮验证：cargo test --locked --no-default-features完整成功，日志
/tmp/plico-stage5-core-final.log；完整桌面106项与发布配置导航10项成功，日志
/tmp/plico-stage5-{desktop,navigation}-final.log。Clippy no-default-features all-targets
成功，仍有已有警告（/tmp/plico-stage5-clippy-final.log），不宣称零警告；格式与
whitespace检查通过。Windows构建沿用最近group-preview成功产物，随后未改第5项
产品代码，原生验收使用同一产物。未运行CI或发布。

按4473节交付范围，第5项功能阶段可独立提交：双向独有提交、有界加载及筛选、
整体固定版本文件差异、方向交换与空结果、引用刷新、按需Diff；各项证据见前述
收口检查及后续修正。保留此前记录的精确视觉差异；独立固定分支文件面板无刷新按钮
与参考一致，引用刷新通过双分支日志比较入口完成。大规模内存基准未在本轮重测。
第6项仅清单基础，代码不纳入第5项提交；第7、8项未开始，总目标仍未完成。


### 第6项基础模块独立提交（2026-10-27）

将项目成员及声明发现拆为可独立编译的基础提交：`submodules` 只读取声明、gitlink
与真实子仓库状态；`project_repositories` 管理已注册根、声明关系及持久映射，
不因gitlink删除丢失恢复入口，不将普通目录冒充父仓库。操作上下文使用已验证根
按最长路径前缀解析，不在界面线程读取磁盘。映射存储按项目隔离、有界读取、锁内
合并及原子替换，取消或损坏记录不能静默覆盖原文件。

参考仍为4aec352的GitModulesFileReader/GitSubmodule、VcsRepositoryManager、
ProjectLevelVcsManagerImpl及DvcsUtil；安装版1.1.20。此前原生“项目映射与子模块
声明分离”“映射持久化”记录的双根冲突与删除声明后重开是对应界面证据，截图
`artifacts/collaboration-workflows-20261027/plico-mappings-{before,reopened}-ready.png`。
独立提交仅交付领域基础及其真实Git回归；聚合Log、整组Update、操作路由及第7/8项
继续保留在工作区，不将基础提交等同于第6项全部完成。完整原生记录随后续接线提交。

独立暂存快照验证：完整核心775项通过；两项Windows测试数据修正后，Linux定向15项、
Windows宿主真实Git13项均通过（Unix专属路径测试不在Windows编译）。初次Windows
失败分别是fixture目录末尾空格被Git忽略、错误路径的verbatim前缀导致字符串比较
失败；修正为合法长路径并明确断言参数总长仍超过32767，以及按既有平台路径身份
验证错误归属，没有放宽产品结果断言。日志 `/tmp/plico-inventory-stage-core.log`、
`/tmp/plico-inventory-stage-final-tests.log`、`/tmp/plico-inventory-stage-windows-final-tests.log`。
格式和暂存差异检查通过；CI尚未运行，本提交无新增可见UI。


### 第6项事件与操作状态基础独立提交（2026-10-27）

在cce90ff项目成员基础上，独立提交 `worktree_events` 的根限定通知、外置gitdir和
shared common-dir路由，区分成员、refs/HEAD、index及工作区失效。ProjectEvents复用
未变化根的监听，移除根时释放监听，安装失败/网络路径保留轮询；通知不携带历史副本，
也不触发写操作。`project_operations::Observer` 按真实Git状态观察每个根，保留不可读
错误，取消不发布半份项目快照。`operation_queue` 整组原子占有多个common-dir，
等待时不保留部分锁；后续整组Update将使用此接口。

源码依据为此前已实读4aec352的 `plugins/git4idea/backend/src/repo/`
GitRepositoryManager.java、GitRepositoryChangeListener.java、GitRepositoryUpdater.kt，
以及GitOperationActionBase的操作资格规则；安装版1.1.20。沿用本轮多根原生依据，
本次只拆已有基础实现，不新增界面入口。DiffTab的两个旧index判断同时切到
baseline_changed，以保留HEAD/ref更新时的比较基线刷新；其余多仓库接线留待后续提交。
验证使用导出的暂存快照，避免工作区未提交代码掩盖依赖，结果待补。

独立快照验证完成：Linux完整核心786项、完整桌面106项、发布配置Diff导航10项通过；
日志 `/tmp/plico-events-stage-{core,desktop,navigation}.log`。Windows宿主运行真实Git
与原生文件监听测试：worktree_events 5项、project_operations 2项全部通过，日志
`/tmp/plico-events-stage-windows-tests.log`，包含分离gitdir、工作树私有index与共享refs、
移除根释放监听、外部冲突解决/中止及取消/不可读状态。没有用交叉编译代替宿主运行。
暂未被该基础提交消费的Guard::owns保留在后续工作区，不提前引入未使用API。
格式及暂存差异检查通过；CI未运行。本次没有启动桌面实例，也没有新增可见UI。


### 第6项整组 Update 核心独立提交（2026-10-27）

本次从累计实现中拆出可独立构建的 Update/恢复核心，不包括聚合 Log、GPUI
多仓库接线、Unshallow 或交互式 Rebase 计划界面。参考 commit
4aec35256d765c217aa0d662b82ac4e860c396fd；安装版仍为 Rebased 1.1.20 /
IC-262.10968.SNAPSHOT。

已读源码：plugins/git4idea/backend/src/update/GitUpdateProcess.java、
GitUpdateResult.java、GitSubmoduleUpdater.kt、GitMergeUpdater.java、
GitRebaseUpdater.java，以及 rebase/GitRebaser.java、GitPreservingProcess。
词级自动解决沿用 platform/util/diff/src/com/intellij/diff/comparison/MergeResolveUtil.kt
及对应上游测试；只采用 simple resolver，不采用 greedy 删除区间合并。

规则与本地映射：project_update/{roots,group,preservation}.rs 区分普通跟踪分支、
游离子模块和不可更新根，先 Fetch/复验再整组保存。submodule_update.rs 递归更新
已初始化子模块，不添加 --init；附着分支按自身 upstream 更新。关闭当前根冲突
交互返回 Incomplete 并继续其他根；只有整组成功才恢复修改。conflicts::Interaction
及 channel 保持 worker 对写锁的所有权；取消和迟到回复不能绕过索引/仓库身份验证。
merge_result/text_merge 复用已验证的保存、暂存及自动解决规则。editor::open_in_roots
只接受本次捕获目录下允许的编辑文件，不将父 .git 前缀当作所有子模块的授权。
stash::project 按项目映射重新发现保存结果，支持关闭窗口后的恢复发现。

前置及原生证据沿用本轮已完成的同场景验证：20261026 的
rebased-update-{conflict-a,parent-conflict,incomplete}.png；20261027 的
plico-update-ui-final-state.json、plico-update-close-state.json、
rebased-merge-state.json、plico-merge-state.json、plico-independent-detached-state.json
和各自同名前缀截图（均位于 artifacts/collaboration-workflows-日期）。
本轮重新读取 Merge/普通多根磁盘结果，确认双亲提交、无未合并索引、保留草稿及
跳过游离主根的结果。以上完整应用的 UI 验证不等同于本次核心提交新增了入口；
现有单仓库入口保持兼容，多仓库 UI 接线随后独立提交。

保留差异：本地使用 Git Stash 保存，参考测试实例使用 Shelf；暂不提供多仓库同步
控制设置，采用已实测的非同步规则；普通根可跳过并给出原因。父/子处理次序及
冲突列表精确视觉差异延续原记录，未宣称第6项或像素级还原全部完成。

独立提交快照验证（不依赖后续未提交模块）：核心839项通过，桌面106项、发布配置
导航10项通过；格式检查与Clippy完成，后者保留既有警告，没有为本次新增全局allow。
最终测试夹具修正后，子模块/项目更新52项再次通过。日志为
/tmp/plico-update-stage-{core,desktop,navigation,clippy,submodules-final,format}.log。

Windows真实Git验证覆盖conflicts14项、editor16项、project_operations4项、
submodules50项。首轮editor有1项因原生askpass路径未配置失败，配置后定向通过；
submodules首轮41通过、9失败，其中7项是Windows路径前缀/分隔符的测试断言，
2项是并行真实Git流程超过夹具15秒等待。已用既有仓库identity比较实际/预期根，
保持顺序、具体根、Stash内容、索引及工作区保护断言；诊断仍检查对应保存根的实际
展示路径。Windows夹具等待60秒、整段死锁看门狗120秒，Unix保持15/30秒，产品
取消/超时不变。修正后9项定向原生复验全部通过，无忽略用例。
子仓库编辑测试支持与既有测试一致的GITUI_TEST_EDITOR_EXE/GITUI_TEST_ASKPASS_EXE，
避免交叉编译机绝对路径混入宿主运行。原生证据日志：
/tmp/plico-update-stage-windows-{tests,final-tests,retests}.log。
本节“通过”是初次通过项与修正后定向复验的合并覆盖，并非声称首轮全部绿色。
没有新开Plico/Rebased界面窗口；原生测试程序均已结束。UI接线继续留待后续提交。


### 第6项多仓库 Log 数据层独立提交（2026-10-27）

从已完成分项原生验证的累计实现中拆出数据层，保留单仓库界面兼容；此提交没有
提前接入多仓库菜单或Roots控件。参考commit为4aec35256d765c217aa0d662b82ac4e860c396fd，
安装版Rebased 1.1.20 / IC-262.10968.SNAPSHOT。

本轮重新实读 platform/vcs-log/api/src/com/intellij/vcs/log/CommitId.java 和
platform/vcs-log/impl/src/com/intellij/vcs/log/data/VcsLogMultiRepoJoiner.java；
范围规则沿用已读 VcsLogFiltererImpl.kt::filter、VcsLogUtil.java::getAllVisibleRoots。
CommitId以根与hash共同确定身份，不能跨根去重；每次只在各根下一条提交之间按
committer时间选择，保留单根Git顺序，不能全局sort破坏拓扑。本地相同时间按输入
根顺序稳定排序；上游HashMap迭代没有固定平局顺序，这是保留的确定性差异。

映射：git.rs提供committer_timestamp、根内引用/跟踪元数据；project_log.rs统一
查询scope、错误、有限前缀合并、分页cursor与异步计数入口。每根固定tips与过滤条件，
只按实际输出条数推进该根offset；失败不发布半份下一页，部分根失败不把subtotal
当作完整总数。分页不保留之前提交正文/几何，图前沿保留根身份。history/filtered_graph
保留用于画图的投影parents，真实Git parents不改写。project_log/navigation.rs使用同一
scope恢复位置；selection.rs按根加载提交/文件，diff_target::Chain让同名文件跨根导航
仍指向正确仓库。log_selection复用同一选择语义并保留原单仓库API。

原生前置/对照证据沿用 artifacts/collaboration-workflows-20261027 中
rebased-independent-log-default.png、rebased-log-filter-third-final.png，以及
plico-log-default-final.png、plico-log-filter-third-final.png、
plico-log-child-details-ready.png、plico-log-cleared-final.png；完整界面验收记录见
上述默认聚合首屏章节。本提交只包含数据层和现有界面结构字段兼容，根颜色条、
菜单/过滤控件接线与精确视觉差异仍留后续提交，不以数据测试宣称界面全部完成。

独立索引快照验证：853项核心、106项桌面和10项发布配置Diff导航通过；
Windows原生新增6项真实Git场景全部通过（同hash/同路径跨根、过滤图真parents、
部分根失败与总数、固定tips与过滤的导航、跨根分页不重复/不漏项）。日志：
/tmp/plico-log-stage-{core,desktop,navigation,windows-tests}.log。
格式检查通过；Clippy完成，仍有包括navigation::Location大小建议在内的警告，
未通过全局allow或关闭检查隐藏，日志/tmp/plico-log-stage-clippy.log。
验证使用精确暂存内容导出的独立目录，未依赖未提交UI/Unshallow/交互式Rebase。
本次未启动原生界面窗口；Windows集成测试已结束，原生UI对照沿用上述已记录场景。
