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
