# Windows 轻量 Git 审阅器开发计划

日期：2026-10-15。状态：实施与 Windows 验证进行中。

目标是实现一个 Windows 优先、低内存、以审阅为主并提供基础 Git 操作的工具。以 IntelliJ 的 Git Log 和 Diff Viewer 的布局与操作逻辑为参照，借鉴 Rebased 的独立 Git 客户端定位，暂定使用 Rust 和 GPUI 独立实现。

核心使用场景：浏览其他开发者的提交和分支；审阅 AI 已提交或尚未提交的代码；识别当前仓库已有的冲突，理解分叉与合并关系。低内存是产品验收条件，不能靠删掉必要的审阅体验来达成。

Rebased 基于 IntelliJ 平台。本项目复现所需的界面与交互，不以裁剪 IntelliJ 平台为实现路线，也不预先假定 Rust/GPUI 一定能达到内存目标。

当前已具备 Rust/GPUI 实现、Git 读取层、graph 引擎、Windows 构建与验证脚本。以下范围和指标仍是完整验收依据；尚未勾选的项不代表全部未实现，而是其完整验收证据仍不足。实测观察与当前限制见 README.md。

## Plico 独立仓库（2026-10-18）

本目录 `/home/echoyn/code/plico` 从原工作树复制，独立初始化Git；原 `/home/echoyn/code/gitui` 保留供当前会话及历史产物查询。Cargo包／库／主程序统一为plico，脚本和Windows资源链接入口同步；内部helper及GitReview配置路径保留兼容。历史构建证据仍指旧命名构建，不能当作改名后Windows程序已验收。旧缓存、IDE个人配置、artifacts、dist、Git历史未纳入本仓库。

## 第一版封版决定（2026-10-18，用户最新安排）

当前版本按 **0.1.0 首版预览** 封版：以日常Git审阅、文件浏览／历史／Diff和既有基础Git操作、基础编辑／自动保存为交付范围。目前没有已确认的主流程不可用、数据丢失或错误结果硬阻断。停止自动追加视觉细化和边缘验证，后续仅处理实际复现的硬阻断；旧章节未勾选和阶段性“尚未实现”不自动转成首版门槛。

保留差异：单栏编辑Diff仍用投影行号，增删背景尚非整行；像素级细节、原生IME更多组合、多光标、完整干净系统及新增编辑后的全面内存压力验证留待后续。不将未验收内容标为通过，也不宣称正式稳定版或全部长期计划完成。整行底色调查本轮仅阅读源码，尚未修改实现，随封版停止推进。

交付构建：`B71CF4E6EF2A63F89BC59EC070DD7C3DA7A95D13D63D2DDDEBB5AC224C431176`，已有Windows保存冲突阻止退出、草稿保留及重试保存成功退出证据。仅继续打包及包完整性核对，不继续功能／UI重写。用户后续反馈再开启下一轮。

## 当前状态与剩余主线（2026-10-17）

本节是当前进度入口；产品范围仍以“侧栏、提交与历史整理增量”、F1–F4及完整验收标准为准，不替代或缩减它们。后文按日期追加的“下一步／尚未实现”仅描述当时状态，需结合较新的同功能记录阅读。下表“已实现”不等同整项验收通过；不按首页空复选框重新实施已有功能。

当前可手测包：[Plico 预览包](dist/Plico-windows-x64-preview-20261018-loading-feedback.zip)。主程序SHA256为 `6425b9af5efd9f6ce0c01fab38644e8b16f83f90d389410cd326cc32192b7e99`；包含原生对照后的项目菜单、分支状态图标与Commit入口，以及图标统一、筛选清除、Checkout/Tab菜单、详情滚动条、最近项目、只读代码高亮，以及文件/选区历史、文件历史graph、独立分支条件与多选版本比较、源码跳转。Windows原生集中交互、解压和哈希检查见文末。初始提交为8083893，验证工具提交aadca33；产品功能增量仍未额外提交；最近完整核心回归397项通过（含文件历史专项，另有桌面专项）；本轮Windows剪贴板所有权及短暂占用专项共2项通过；历史内存证据仍绑定各自构建，不冒充本包完整性能验收。分支改动审阅与合并冲突预检仍暂缓。

| 主线 | 已实现／已有证据 | 明确剩余工作 |
| --- | --- | --- |
| 左侧工具窗口与Project | Rebased 20px工具窗口图标、统一彩色文件类型图标、只读文件语法高亮、Tab右键关闭；Commit/Project/Stash独占切换；只读全文Tab、可选预览/固定、混合Tab关闭与活动选择恢复；目录256项分页及16384节点预算；刷新及失败重试保留展开/路径选择，后续主动选择优先，删除回退可见祖先；4202文件Windows分页、折叠及三轮内存验证 | 切仓/模态/慢读取预览竞态、多光标/缩放后的阅读状态、默认名单之外的IDE项目排除策略差异、视口恢复；首次加载中刷新/慢读时折叠/切仓与预算耗尽原生专项；目录响应时延 |
| 文件／选区历史 | 文件历史graph、独立Branch筛选、Ctrl/Shift多选比较、双父三栏合并结果预览、源码跳转；选区历史与Changes only；历史OID选择恢复、键盘关闭／焦点；完整提交元数据、分支／Tag及包含分支详情、按项目记忆显示开关；rename及选择专项 | 三栏词级分词精确差异／连接曲线、外部状态徽章／其余显示选项等保留差异；复杂合并与大规模选区的完整验收；精确视觉延后 |
| 欢迎页与最近项目 | 成功打开仓库后持久保存最多20项，按路径去重/最近优先；欢迎页名称与路径列表、打开/移除；中英文原生验证 | 已支持当前／新窗口选择；无搜索/分组/克隆；不是完整IDE项目管理器 |
| Commit／Amend | 文件勾选、父目录三态、消息草稿、Amend及Commit and Push；index保留与持久恢复API/UI已有实现；英文hook失败详情／草稿重试和消息Amend、中英文错误摘要原生验证 | 部分变更选择与上游的保留差异、hooks/签名/外部变更组合及中英文窗口验收继续核对；不将文件级提交描述为任意hunk编辑 |
| Log多选与Squash | 聚合变更及详情，最近/中间/不连续Squash，冲突Continue/Abort，重启恢复，新提交定位和Undo | 过滤/跨历史窗口后的选择与定位、入口过期、复杂拓扑及恢复交互矩阵收尾 |
| 外部恢复记录 | Stash查看/Apply/Pop；“已在外部处理”显式确认；过期记录、冲突、worktree隔离、超限等真实Git测试 | 非普通/超限记录仍需外部检查；Windows reparse专项、英文/键盘与检查-删除之间外部并发限制尚未全面验收 |
| Compare Versions与导航 | 两提交直接树比较，独立Changes侧栏、首文件、复用Diff预览、双向选择及Esc返回；父/子提交导航 | 复杂目录、空结果/连续刷新、非活动预览取消、关闭焦点、过滤及跨页导航专项；不等同merge-base分支审阅 |
| Fetch／Pull／Push／Checkout | 基础执行、认证/编辑器桥接、选项、结果分类、更新/重试、继续/中止与状态刷新已有代码和多批真实Git证据 | 按F2–F4逐场景收尾认证/取消/外部变更与linked-worktree组合；不能仅凭本地remote成功宣称完整网络验收 |
| 错误与中英文 | 系统语言默认、持久化、设置弹窗；Diff短摘要/重载/复制详情；全局错误移至右下；仓库所有权失败为Log空态+刷新/详情，不重复撑动上下区域 | 读取面板已共享短说明／重试／详情；写操作、认证与恢复各场景的错误呈现继续核对；完整英文/键盘/动态状态覆盖；通知浮层及历史尚未实现，状态栏为记录在案的差异 |
| Rebased UI/UX | 工具栏、分支菜单、设置/操作弹窗、Diff布局与图标已有多轮源码对照和原生修正 | 按功能统一焦点、禁用、关闭、返回；同尺寸/DPI截图验收字体/间距/圆角/色彩；未完成逐像素还原 |
| 性能与交付 | 当前605构建Windows：空欢迎页53.20MiB；实载10000提交／32轨道／975000字节Diff 120.44–128.01MiB，Job峰值184.84MiB；100文件199次查看／20刷新／10切仓229次全文核对，最终92.09–93.13MiB，Job峰值168.81MiB；GPU另记，预览包哈希／解压已验 | 真正冷缓存启动／数据就绪与加载反馈延迟、数百轨道其他拓扑与长会话、更多写操作峰值和干净Windows环境；本轮有界连续审阅已通过，单场景合格不代替整个发布门槛 |

### 接续执行顺序（按用户2026-10-17最新决定）

现有功能已形成初始开发版本，集中核心回归347项通过、Windows预览包可解压启动。用户要求不要无限细化，并明确暂不做分支改动审阅与合并冲突预检；该决定覆盖历史记录中的“下一主线”安排。

1. **初始版本提交**：记录当前可用范围与验证结果，纳入源码、测试、构建配置、依赖锁和必要许可；构建缓存、便携包及临时测试产物不纳入源码提交。版本为0.1.0开发预览，不宣称所有发布验收完成。
2. **现有功能和明显UI/UX问题**：围绕用户实际使用反馈与常用流程集中修正；数据安全、错误结果和主流程阻断优先。不再逐个新增Project预览/焦点/目录边缘组合专项。
3. **集中视觉与发布验收**：按需批量处理Rebased界面结构、中英文和错误反馈；性能验收补实际缺口，不重复已通过且未受改动影响的基线。完整发布验收仍与初始版本提交区分。

暂缓：merge-base分支改动审阅、显式合并冲突预检。两者尚未实现，本次仅阅读候选源码，未接入功能；没有用户新的安排，不继续推进。

延后池：罕见竞态组合、完整IDE项目排除模型、多光标/缩放阅读状态、通知历史及逐控件像素差异。保留已有差异与验收记录，不将延后描述为已完成。New Branch/New Tag仍为候选；Cherry-Pick/Revert/Reset、通用编辑器等不自动增加。

## 当前优先事项：基础编辑与自动保存（2026-10-18）

用户要求停止无限细化，围绕实际使用目标判断阻断项，并增加与 Rebased 类似的基础编辑／自动保存。此安排覆盖前文 Project 固定只读和通用编辑器不自动增加的旧范围；工作区源码 Tab 和 Commit Diff 中实际对应工作区文件的一侧可编辑；历史版本、暂存区、Stash 和推送预览保持只读。不增加补全、诊断、索引、格式化、插件或 IDE 项目模型。

本轮交付边界：输入／剪切粘贴／撤销重做，保留语法高亮；空闲与应用失焦自动保存、Ctrl+S；关闭／切仓／退出及相关 Git 操作前处理未保存内容；外部修改冲突不静默覆盖；保存失败保留草稿；保留 UTF-8 BOM／换行风格。干净非活动正文继续释放，未保存内容不得沿用只读 Tab 的直接释放路径。二进制、非支持编码及超预算文件维持只读提示。

完成条件是这条编辑→保存→查看差异→提交链路，以及外部修改、保存失败和关闭保稿的集中 Windows 验证。已有未复现的边缘组合和逐像素差异不追加为此功能前置门槛。当前没有据既有记录确认新的日常审阅主流程阻断项；完整发布验收不足仍如实保留。

进度：新增 `src/file_document.rs` 文档状态层，保存采用单文档串行快照，成功后推进磁盘基线，IO 期间的新输入继续为未保存；失败保留草稿和原基线，拒绝异文档确认；保留 BOM／换行，无编辑时不重写混合换行。6项状态测试通过。**尚未接入文件写入、外部冲突检测、自动保存触发和 UI，现有程序仍只读；不能将状态测试称为保存功能完成。** 下一步直接实现文件保存与 Tab 生命周期，再集中原生验收，不新增视觉支线。后续进度见文末。

## 功能优先增量（2026-10-16）

本节记录基础功能执行顺序；后续UI/UX及侧栏增量的执行顺序覆盖本节。基础范围覆盖旧记录中的“只读”“写操作不包含”及先像素验收再推进功能的约束。历史记录保留为当时事实，不作为新的范围限制。顺序：设置与语言 → Git 操作公共执行能力 → Fetch → Pull → Push → 指定 commit checkout → 功能回归及内存验收 → 继续像素级细化。通知浮层等纯视觉工作暂缓；以下清单按完整验收勾选，部分实现及验证进展见文末，未勾选不表示完全没有实现。

功能阶段同步对照 Rebased 的 UI 结构：新增 Pull／Push、冲突处理、设置等界面先核对上游入口、布局、信息层级、控件组织和交互顺序，再实施。像素阶段只延后精确间距、颜色、圆角等细化；结构差异须记录原因和后续方案，避免临时界面导致整体重写。

### UI/UX 优先级调整（2026-10-17）

按用户最新要求：先收尾当前 Rebase 继续／中止任务，随后优先审查并修正现有 UI/UX、弹窗风格和顶部分支入口，再继续其余功能验收。此顺序覆盖上文将全部界面细化排在功能与内存验收之后的安排；功能行为与低内存要求仍保留。结构、操作层级及交互差异不能归入最后的像素微调。

新基准已原样归档到 [截图索引](artifacts/rebased-user-reference/README.md#2026-10-17弹窗与顶部分支入口)：Update Project 弹窗及顶部分支菜单。安装版与源码快照是否一致尚未确认；本次仅归档截图与安排任务，未新增源码行为结论或完成视觉验收。

- [ ] 当前任务收尾：Rebase 继续／中止按上游行为实现，覆盖冲突、取消及保留修改，并完成 Windows 原生验证。
- [ ] 建立现有界面差异清单：逐项比对入口、布局、信息层级、弹窗位置与宽度、按钮分组、主次动作、焦点、关闭／取消和错误反馈；记录本地组件、上游 commit／路径、修正方案及验收截图。
- [ ] 优先统一对话框：检查现有过宽／整行按钮、内容与底部操作区、模态背景和尺寸策略；分别读取 Update Project、显式 Pull、Push 及风险警告的对应实现，保留各自默认按钮和决策语义，不能套用同一业务弹窗。
- [ ] 对齐顶部分支入口：参照锚定位置、搜索、动作分组、Recent／Local／Remote 树与收藏／子菜单组织；与 Log Branch 过滤器区分。先确认源码行为和现有功能映射，不添加无行为的占位控件。截图出现的完整 Commit／New Branch／Reset 不自动扩大当前范围。
- [ ] 将共用样式和交互整理为一致组件，保留 Git 执行与 UI 的边界，避免逐个弹窗重复实现；按需创建、关闭释放，不以增加常驻面板换取视觉相似。
- [ ] Windows 同条件验证代表性弹窗和菜单：中英文、鼠标／键盘、默认／禁用／错误状态；对照原生截图记录已修正及保留差异。精确像素细化可继续分阶段，不宣称已完成像素级还原。

### 侧栏、提交与历史整理增量（2026-10-17）

用户明确扩大产品范围：本地变更和已保存的更改迁移至左侧工具窗口；增加只读文件浏览；本地变更按 Rebased 的 Commit 工具窗口提交并支持 Amend；Log 支持多选 Squash，并评估少量简单右键动作。本节覆盖旧的“不包含 commit/stage”约束，截图中的其他动作仍须逐项确定范围。既有F1–F4、低内存和源码对照要求继续有效。

执行顺序：收尾当前Push内存记录 → 左侧工具窗口壳与只读Project文件树 → Commit／Amend与Commit and Push → Log多选及比较／导航 → Squash → 其余右键动作评估与完整回归。当前后续内存测量并入相关功能验收，不以无限补测延后本节主线。先对齐布局、交互与状态，再细化像素；同一版中英文同步。

- [ ] **左侧工具窗口**：竖向图标栏提供Commit（原本地变更）、Project、已保存的更改入口；原本地变更／Stashes不再占据中央与Log并列的主Tab。保留其原有真实功能与状态，区分工具窗口展开与中央审阅Tab；按用户最新决定（2026-10-17）每个左侧工具窗口独占侧栏，一次仅显示Commit、Project、已保存的更改之一；点击图标切换，再点当前图标收起。保留侧栏宽度调整及各工具窗口的选择/草稿，取消上下分区与其比例要求。Log内Branches仍属于Log自己的分支浏览区域，不能与Project混为一个树。切换窗口保留选中项、草稿及中央Diff，重型内容按需持有。
- [ ] **只读Project浏览**：按目录展开工作区文件，包含可见未跟踪文件；忽略项／隐藏项显示策略先对照Project View与Git仓库上下文，不能将整个目录一次性无界扫描。单击／双击／Enter的预览与固定Tab遵循上游编辑器规则，显示完整只读文件内容、路径、行号及可复制文本；无编辑、保存、删除、重命名动作。区分文件查看与Diff身份；覆盖中文路径、外部修改／删除、空文件、二进制、超大文件、符号链接／junction及循环、子模块；分页／取消／读取上限和切仓释放明确。
- [ ] **Commit工具窗口**：按截图组织勾选的变更树、Unversioned Files、Amend last commit、提交消息、Commit及Commit and Push。选择查看Diff与勾选纳入提交是独立状态；默认勾选与父目录三态、暂存／未暂存同文件、文件和部分变更选择的语义按源码确认，不能直接git add -A。未选修改及既有index必须保留；执行前固定仓库/HEAD/选择版本并复验。复用公共串行执行、认证／编辑器与真实状态刷新，不引入另一套Git写入器。
- [ ] **Commit／Amend行为**：普通提交、首次提交、改最后一次提交（包括仅消息修改）和Commit and Push均有完整路径。Amend加载原消息及退出时草稿处理按上游核对；作者／提交者、hooks、签名配置、消息清理和错误文案保持宿主Git语义。Commit and Push只有提交成功后进入已有Push流程，推送失败不能误报提交失败或自动重复提交。覆盖未解决冲突、进行中操作、空消息、空选择、hook失败／修改文件、取消、并发外部变更与保存失败；不静默跳过hooks/签名，不提供自动force push。
- [ ] **Log多选**：Ctrl切换、Shift范围、键盘范围扩展和右键保留已选集合按上游核对，选择身份采用完整OID而非行下标；支持筛选／刷新／翻页后的明确选择规则。右侧变更与详情在多选下按Rebased组织，不能只保留最后一行却让Squash处理另一集合。未加载元数据按需读取并有取消／上限。
- [ ] **Squash**：至少两个提交，先复验当前HEAD路径、提交拓扑、protected branch及操作状态；消息编辑后后台执行，完成定位重写结果。覆盖最近连续、中间连续和不连续选择，保留未选提交的变更与相对顺序；根／merge／不在HEAD／detached／浅历史／已发布提交的可用性、提示和恢复路径逐项依据源码及测试，不能用“只压缩最近N条”冒充完整功能。复用Rebase编辑器及继续／中止能力，区分取消与回滚；记录重写前后OID、分支及必要恢复信息，按上游验证Undo可用条件。低内存实现方式可不同，但必须说明与上游in-memory及Git fallback的差异。
- [ ] **其他Log右键动作**：优先补齐Copy Revision Number（包括多选格式）、Compare Versions、Go to Parent／Child Commit；沿用已有checkout。New Branch／New Tag列为下一批候选，源码、生命周期与测试成本确认后实施；Cherry-Pick、Revert、Drop、Reset、Push up to Here不因截图出现就加入本轮。保留菜单分组与禁用原因，不显示没有真实处理逻辑的占位项。
- [ ] **验收与交付**：每功能记录上游规则→本地代码→真实Git测试映射。临时仓库覆盖部分暂存、未选文件、未跟踪/删除/rename、hook失败、Amend消息与父链、Squash三类选择及冲突/取消/外部HEAD变化；核对实际tree/index/worktree/refs和恢复后数据。Windows原生验证独占侧栏切换/收起恢复、只读Tab、勾选与草稿、提交／推送结果、多选右键、Squash消息与禁用状态；将目录大规模、非活动Tab与新写操作纳入Job峰值与释放检查。全部未实现项保持未勾选。

依据：[两张用户原图](artifacts/rebased-user-reference/README.md#2026-10-17侧栏提交和-log-多选菜单)已保存；源码已确认部分规则与待深入入口见[对应源码记录](docs/REBASED-REFERENCE.md#侧栏提交与-squash2026-10-17)。截图不能证明Squash禁用的原因，也不能据此把任意多选都视为可执行。

### 功能阶段 UI 验收

每个新增可见功能在实现记录中逐项说明以下核对结果；源码与已保存截图作为依据，需要确认实际行为时再操作 Windows 上的 Rebased。

- 入口与布局：工具栏、菜单、弹窗或 Tab 的位置和所属区域对齐参考；保留左右面板关系、共享工具栏高度及内容起始线。现有截图中单搜索框、Log／文件树对齐和独立 Diff Tab 是结构基准。
- 信息与操作：字段分组、主次按钮、默认选择、确认／取消、键盘焦点及错误／冲突后的下一步按上游核对。Pull／Push 的选项与分支流程仍以 F3 的源码约束为准。
- 状态与生命周期：加载、禁用、成功、失败和取消反馈有明确位置；关闭弹窗、切换 Tab 或仓库后的状态保留和释放随功能实现，避免日后更换布局时重写操作逻辑。
- 实现边界：Git 执行与结果状态独立于界面布局；复用已有主题、图标、工具栏和 Tab 组件。颜色、间距、圆角及字号集中调整，避免逐页面硬编码形成不同设计。
- 延后范围：功能阶段就确定面板／弹窗／Tab 的承载形式、操作入口和主要尺寸比例，按 Rebased 组织组件；不能把这些结构选择作为“像素细化”推迟。确需不同承载形式时，在实现前记录理由、受影响组件及迁移方案；后续视觉阶段应以主题参数、资源和局部样式调整为主，避免重写页面或 Git 操作流程。
- 验收记录：Windows 原生截图确认大体结构和操作路径；列出尚存的结构差异、原因及修正安排。精确像素差异单独留待视觉阶段，不将结构待办混入纯视觉待办。

### F1：设置入口与中英文

- [ ] 顶部 Settings 改成现有 JetBrains 齿轮图标，提供本地化 tooltip 和可访问名称，沿用设置页面与键盘可达性。
- [x] 增加语言选项：跟随系统（默认）、中文、English；持久化用户选择。默认读取 Windows 用户界面语言，中文语言环境选择中文，其他语言回退英文；读取失败回退英文。先提供简体中文资源，繁体中文系统如何映射明确记录。
- [ ] 建立集中消息资源，覆盖欢迎页、工具栏、菜单、设置、提示、空态、错误和新增 Git 操作；翻译不修改提交正文、作者、分支名、路径、哈希和原始 Git 输出。错误摘要本地化，详细输出保持可排查。
- [ ] 切换后更新已打开界面且保留仓库、筛选、Tab 与 Diff 偏好；重启保持选择，跟随系统模式在启动时重新解析。测试显式覆盖、系统中文／英文／未知、缺失消息回退、格式化参数，以及中文字体回退；Windows 原生验证两种语言和设置持久化。

### F2：Git 操作公共执行能力

- [ ] 基于现有读取层增加明确的写操作生命周期，按仓库／worktree 关系串行协调修改动作；后台执行，有进度、取消和结果反馈，防止重复点击。固定目标仓库与引用，切仓后结果不能污染新仓库界面。
- [ ] 宿主 Git、credential helper、SSH 和认证交互按上游方式对照适配，不能把需要认证的命令无限挂起；输出有界，凭据信息不写入日志。读取仍禁用隐式拉取，显式 Fetch／Pull 的网络能力独立处理。
- [ ] 成功、失败或取消后重新读取真实仓库状态，包括 HEAD、refs、index、冲突、工作区；取消不等于回滚，部分已完成的写入不能被描述为未发生。更新当前视图，保留有意义的选择，避免过期历史／Diff 假装仍对应新 HEAD。
- [x] 更新 Read-only 文案、README 和便携包说明，准确表达“浏览为主＋基础操作”；此为当时基础操作说明验收；新增Commit／Amend范围由上方2026-10-17增量覆盖，交付时继续同步说明。

### F3：Fetch／Pull／Push／指定 commit checkout

**源码约束：Pull／Push 必须按 Rebased 实现和测试翻译，不能自行设计流程，不能只凭通用 Git 经验拼命令。** 实现前读取对应 action、操作类、选项、结果解析、冲突／失败处理及测试；记录每个上游规则到本地代码／用例的映射。为 GPUI、单仓和内存限制作出的必要适配须明确记录；不能把缺少上游处理分支称为完成。

以下为待阅读入口，已在参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 确认路径存在，**尚未阅读并确认它们的行为**：

| 功能 | Rebased 源码入口（plugins/git4idea/ 下） | 测试入口 |
| --- | --- | --- |
| Fetch | backend/src/actions/GitFetch.java、backend/src/fetch/GitFetchSupportImpl.kt、GitFetchSpec.kt、GitFetchResult.kt | tests/git4idea/fetch/GitFetchTest.kt |
| Pull | backend/src/actions/GitPull.java、backend/src/pull/GitPullDialog.kt、GitPullOption.kt、backend/src/config/GitPullSettings.java；继续沿调用追踪 update 实现 | tests/git4idea/update/GitSingleRepoUpdateTest.kt、GitUpdateMethodTest.kt、GitUpdateFixtures.kt |
| Push | backend/src/push/GitPushOperation.java、GitPushParams.java、GitPushNativeResultParser.java；继续追踪 Push 界面与目标选择 | tests/git4idea/push/GitPushOperationSingleRepoTest.kt、GitPushOperationFixtures.kt、GitPushNativeResultParserTest.kt、GitPushTargetTest.kt |
| Checkout commit | backend/src/actions/ref/GitCheckoutAction.kt、backend/src/actions/branch/GitCheckoutFromInputAction.kt、backend/src/branch/GitCheckoutOperation.java | tests/git4idea/remoteApi/GitCheckoutAndUpdateTest.kt；继续查找操作测试及工作区保护用例 |

- [ ] Fetch：远端选择、实际获取、结果与失败反馈以及 refs／graph 刷新按上游实现；普通刷新保持不触发 Fetch。
- [ ] Pull：远端／分支／upstream 选择、merge／rebase 等可用选项、默认策略、脏工作区处理和冲突反馈均由上游证据决定。不能擅自缩成仅 fast-forward，也不能未经明确动作自动丢弃本地修改。冲突后显示真实仓库状态；必要的继续／中止或引导路径按上游核对。
- [ ] Push：显示本地源与远端目标、待推送内容、upstream 状态；成功、已最新、非快进、服务端拒绝、认证失败等结果按上游处理。不得静默 force push 或擅自自动 Pull；是否提供相关动作及具体交互以 Rebased 逻辑和当前基础功能范围为据。
- [ ] Checkout commit：从 Log 提交操作或输入 commit ID 进入，解析并固定完整提交身份，明确 detached HEAD 结果；处理无效／歧义 ID、未提交修改、未跟踪文件覆盖、冲突／进行中的 Git 操作和 worktree 情形。保留用户数据，上游智能 checkout 等流程需逐项核对，不以强制 checkout 代替。

### F4：功能验收与覆盖

- [ ] 为每项建立上游测试场景 → 本地测试映射；直接复用代码／资源时保留许可证和来源。上游测试只作为依据，本地仍须运行真实 Git 集成测试。
- [ ] 使用临时工作仓库＋本地 bare remote＋第二个 clone 构造：无远端、多远端、无 upstream、远端前进、仅本地前进、分叉、已最新、空仓库、detached HEAD、脏工作区／index、未跟踪覆盖、merge／rebase 冲突、推送拒绝（含 hook）、错误对象、操作取消与重复触发。对不适用的场景按操作说明，不为了凑覆盖机械组合。
- [ ] 断言实际 HEAD／refs／远端 refs、工作区与 index 内容、upstream 配置及冲突状态；失败时确认用户数据保留。认证与网络失败另外覆盖，不能把本地 remote 成功当成认证验收。
- [ ] Windows 原生端到端验证中英文操作入口、进度、结果、错误和刷新后的视图；写操作使用专用 fixture，不修改用户工作仓库或真实远端。
- [ ] 网络／写操作进程纳入峰值和取消后释放检查，维持有界输出、缓存与低内存目标。功能与视觉验收分开记录；完成以上功能后回到逐像素对齐清单。

## 产品边界

### 第一版包含

| 能力 | 第一版行为 |
| --- | --- |
| 仓库 | 一次打开一个本地普通仓库或 linked worktree |
| 历史范围 | 本地分支、远程跟踪分支、指向提交的标签及当前 HEAD 可达的历史 |
| Graph | 拓扑顺序、分叉、合并、多父提交、跨行连线、加载边界 |
| 提交列表 | 单行标题、作者、时间、短 ID、HEAD 和 refs 标签 |
| 浏览 | 鼠标选择、上下键选择、滚动、复制完整提交 ID |
| 详情 | 完整提交说明、父提交、提交时间及作者信息 |
| Diff | 默认独立 Tab 内双栏对比；设置可切换底部预览；同步滚动、行号、增删配色、差异块跳转、文本选择复制 |
| 本地变更与提交 | 左侧Commit窗口浏览及勾选变更，支持Commit、Amend与Commit and Push；Diff保持只读 |
| 文件浏览 | 左侧Project树和只读文件Tab，按需展开／读取 |
| 历史整理 | Log多选及Squash，按上游限制复验并保留恢复路径 |
| 分支浏览 | 左侧本地／远程跟踪分支树，选择分支筛选其可达历史，支持返回全部历史；指定 commit checkout 见功能优先增量 |
| 冲突状态 | 从 index 识别当前未合并路径并显著标记；不把分叉或 merge 节点当成冲突 |
| 刷新 | 手动刷新，尽量保留选中提交及视口位置 |
| 过滤与查找 | Log 工具栏提供文本／哈希、Branch、User、Date、Paths；区分仓库历史过滤与已加载历史定位，支持组合与清除 |
| 已包含历史 | 以当前 HEAD 可达性标记已包含提交，使用 Islands 深蓝行背景，选中态优先 |
| 设置 | 齿轮图标入口；语言为跟随系统（默认）／中文／English；保留 Diff 打开方式及其他现有偏好，持久化在用户应用数据目录 |
| 基础 Git 操作 | 用户显式触发 fetch、pull、push、指定 commit checkout；按 Rebased 源码和测试翻译行为 |
| 历史扩展 | 分批加载，明确标识仍有历史或已到结尾 |
| 异常 | Git 不存在、非仓库、空仓库、对象缺失、输出超限等可见反馈 |

分支树用于浏览，标签可先仅在提交行显示。Merge commit 必须正确显示，默认相对第一父提交展示差异，并明确标注比较基准；不提供独立 merge 操作或完整冲突解决编辑器；Pull 所需的合并流程、状态展示及后续操作按 F3 实现。远程跟踪分支仅代表本地已获取的数据，刷新不会 fetch，也不宣称已同步服务器。

“当前已有冲突”与“将来合并是否冲突”是两种能力。首版提供前者；graph 只能表达历史关系，不能证明合并安全。分支合并预检列入首版后的优先增量，结果必须区分无文本合并冲突、发现冲突和无法判断，不能承诺没有逻辑冲突。

### 第一版不包含

- 独立reset、通用交互式rebase／merge、cherry-pick等尚未纳入的高级写操作；Commit／Amend、Squash及Pull所需流程按新增范围和Rebased实现。
- 自建登录／凭据存储和远程平台服务集成；基础网络操作复用宿主 Git 的凭据／SSH 能力。
- 通用文件编辑、撤销、删除／重命名及完整冲突解决编辑器；提交所需的选择与index管理由Commit增量覆盖。
- 图片差异及完整三方冲突查看器。现有文本语法高亮、行/词/字符强调选项已纳入实现，按对应Diff记录验收，不再列为首版排除项。
- 通用历史折叠；过滤引起的隐藏提交连线处理纳入本轮过滤实现。
- 自动监听、多仓库标签页、插件系统、自动更新。
- 所有 IntelliJ 功能与 graph 算法的完全复刻；核心布局、信息密度和浏览操作应尽量还原。

### 默认工程约束

Windows 优先；必须在实际 Windows 环境验证 GPUI 显示、字体、滚动和内存。当前开发路径为 Linux，不能用 Linux 构建或截图替代 Windows 验收。使用系统 Git，通过后台子进程读取数据。固定经过验证的 Rust 工具链及 GPUI 依赖，提交 Cargo.lock。其他桌面平台放在 MVP 之后验证。

浏览不会主动写仓库；只有用户明确触发基础 Git 操作才执行相应写入，不在项目目录生成应用配置。应用配置进入用户应用数据目录。部分克隆可能因读取缺失对象触发 Git 的网络行为，读取命令应禁用隐式拉取；缺失时给出说明。

## 用户流程与界面

```text
Log 展开分支面板时（默认收起为左侧 Branches 竖条）：
┌──────────────────────────────────────────────────────────────────┐
│ 仓库                 Log  |  Diff: 文件名 ×              设置    │
├────────────┬────────────────────────────────┬────────────────────┤
│ 分支搜索   │ 文本/哈希 Branch User Date Paths│ 文件操作工具栏     │
├────────────┼────────────────────────────────┼────────────────────┤
│ 分支树     │ Graph / 提交列表               │ 变更文件树         │
│            │ HEAD 已包含历史：深蓝底色      ├────────────────────┤
│            │                                │ 提交详情           │
└────────────┴────────────────────────────────┴────────────────────┘
Diff Tab：占用主内容区，显示比较版本、双栏 Diff 和差异导航。
设置为底部预览时：恢复 Log 下方的 Diff 分栏布局。
```

Log 采用 IntelliJ 左侧分支、中间 graph 与提交表、右侧文件与详情结构。过滤栏与文件工具栏共用高度，提交首行与文件树首行共用垂直起点。默认在独立 Diff Tab 审阅文件，保留 Log 状态；底部预览作为设置选项。单击、双击、Enter、Ctrl+D、Esc 的精确触发与焦点规则须先通过 Rebased 源码和原生操作核对，再按下述阶段实现。上下键选择，F7 / Shift+F7 跳转差异，Ctrl+F5 刷新。只呈现已实现的操作，明确区分浏览与会改变仓库／远端的动作。

默认以 Rebased／现代 JetBrains Islands 的深色界面为基准，各功能尽可能像素级还原，具体按下述逐功能验收要求执行。可拖动分隔条。Graph 与提交表共用固定行高和垂直滚动；过宽时水平滚动，不压缩到无法辨认。Diff 两侧按差异块对齐、同步垂直滚动，支持长行横向滚动及跨行复制。M0 固定参考截图，后续在相同窗口尺寸和 DPI 下比较布局与交互。

### 逐功能视觉与交互验收

当前按用户 2026-10-16 指示，先推进功能，像素级细化暂缓；以下视觉验收标准保留到随后阶段，不阻塞当前功能实现。此要求适用于全部已纳入范围的功能以及后续 R1～R3。低内存和还原度同时验收，通过按需加载、虚拟化、有界缓存及及时释放实现内存目标。

- 实现前确认对应 Rebased 源码规则和原生界面，记录参考 commit、已安装版本、主题、窗口尺寸、Windows DPI／缩放和字体配置。对照截图使用相同仓库数据、选择及展开状态；不同参考版本的差异单独说明。
- 逐项核对布局坐标与对齐、工具栏和行高、间距与内边距、Tab 和弹窗尺寸、边框与圆角、背景与状态颜色；核对字体家族、字号、字重、文字基线，以及图标来源、形状、尺寸和位置。测量时区分逻辑像素与物理像素。
- 同时还原操作逻辑：点击、双击、快捷键、焦点转移、打开／关闭及返回状态；验证实际涉及的悬停、选中、聚焦、禁用、展开、空结果、加载与错误状态。过滤栏、文件树、Diff Tab 和设置页面均按此要求验收。
- 实现后用 Windows 原生构建保存对应截图，按同尺寸并排或叠图检查，记录参考图、实现图、差异位置及修正结果；字体渲染或 GPUI 限制等暂不能消除的差异，写明原因、影响和后续处理，不默认为已经还原。滚动交互仍由用户手测，不增加滚动自动化排查。
- 每项功能分别记录功能验证与视觉验证结果。仅完成编译、查询或点击流程，不足以勾选视觉验收；未完成的样式或交互继续保留为待办，现有预览实现也须逐项补齐。

本地变更复用文件列表和 diff，明确显示比较对象：未暂存为 index → 工作区，已暂存为 HEAD → index，未跟踪为新文件预览。未跟踪文件按需读取且不跟随指向仓库外部的符号链接；冲突路径单独标记，不能伪装为普通两栏修改。无 HEAD 的仓库仍能查看本地变更。

打开仓库先加载首批历史；选择提交才读详情和文件列表，选择文件才读取 diff。快速切换时取消过期任务，旧响应不得覆盖新选择。手动刷新同时更新本地变更；读取期间外部修改导致不一致时提示重新加载。

## Rebased 交互与布局修订（用户反馈，2026-10-15）

本阶段原定优先于零散视觉调整；当前先执行上方“功能优先增量”，之后继续本阶段未完成项。这里保留默认交互及此前路径过滤的修订范围。以下均待实现，已有底部预览不代表完成 Tab 体验。源码入口与已确认规则见 [docs/REBASED-REFERENCE.md](docs/REBASED-REFERENCE.md)。

### R1：过滤工具栏与文件区对齐

单入口进展：已移除重复的已加载历史搜索框与 Filter／Find 按钮，模式开关嵌入唯一消息输入框；Windows 原生验证 Enter 查询，证据见 `artifacts/single-search`。仍须完成源码要求的哈希优先查询及回退、Text or hash 文案与蓝色聚焦边框，不能据此勾选完整过滤验收。

后续进展：已接入哈希优先匹配、无匹配消息回退和 Text or hash 文案，真实 Git Linux／Windows 用例通过，原生跨分支短哈希查询截图见 `artifacts/unified-query`。Rebased 可选的消息哈希扩展阶段、图规模限制、蓝色焦点边框及整体视觉仍未验收，详见源码对照记录。

分支菜单进展：已替换为 Local／远端级联结构，Windows 原生验证 Local → main 选择并同步左树和历史，见 `artifacts/branch-cascade`。Select…、HEAD、收藏、speed search、菜单字体及细节仍待完成，R1 不勾选。

HEAD 后续进展：已加入独立 HEAD 浏览入口与固定根查询，Linux／Windows 真实 Git 测试覆盖 detached、未诞生 HEAD 和翻页期间新增提交；原生截图见 `artifacts/head-filter`。R2 的已包含历史背景尚未实现。

- [ ] 原生操作 Rebased，记录相同 DPI／窗口尺寸下过滤栏、文件工具栏、列表首行的坐标和高度；记录各过滤器实际行为。
- [ ] 将 Log 过滤栏与右侧文件工具栏纳入同一高度约束，明确边框、内边距和首行基线；窗口变窄时采用溢出菜单等方式维持对齐。
- [ ] 提供文本／哈希、Branch、User、Date、Paths 过滤。Branch 与左侧树共用状态；定义字段内多选和跨字段组合规则、日期时区、文本匹配与哈希定位的区别，逐项与源码核对。
- [ ] 查询在后台完成，过滤变更取消旧任务并重置分页；过滤整个历史时不能仅筛选当前已加载窗口。已加载查找若保留，需明确标示范围。
- [ ] 过滤后保留真实祖先关系：对隐藏提交形成的连接使用明确的折叠边语义，不能将结果列表相邻行伪装成直接父子；路径过滤的 rename／merge 语义需与源码核对。
- [ ] 验收：同尺寸原生截图中左右工具栏下沿、首行顶部一致；组合、清除、无结果、分页、快速改变过滤、窄窗口可用，查询及图关系有真实 Git 样例验证。

### R2：当前 HEAD 已包含历史高亮

初步进展：已按当前 HEAD 全父路径可达性应用 #1D2336 背景，选中态优先；Linux／Windows 合并与分叉测试通过，原生截图见 `artifacts/head-highlight`，便携包已更新。当前使用有界分页查询，未命中候选可能遍历完整 HEAD 历史；大仓库延迟、内存、浅仓库和并发刷新仍待验收，R2 不整体勾选。

- [ ] 固定当前快照 HEAD，按可达性计算已包含提交；覆盖合并的所有父链。与鼠标选中提交、浏览分支过滤和 refs 图标分别建模。
- [ ] 使用 Islands 的 currentBranchBackground 深蓝色，选中行优先；核对源码中当前分支／HEAD 单分支过滤时的高亮抑制，以及 detached HEAD 行为，再记录本项目对“当前 HEAD 已包含”的取舍。
- [ ] 只给已加载行保留必要标记；避免逐行启动 Git、全历史无界 HashSet 或每帧重新遍历。分页与刷新必须对应同一个 HEAD 快照，异步结果不能串仓。
- [ ] 验收：分叉未合并提交不高亮，已合并侧枝高亮；当前 HEAD 自身高亮；切换浏览分支不改变仓库 HEAD 语义；刷新 HEAD、浅仓库和 detached 状态有明确表现。

### R3：独立 Diff Tab 与设置页

- [ ] 实际操作并源码核对 Rebased 的单击／双击／Enter／Ctrl+D、预览 Tab 复用、关闭和 Esc 返回规则；截图确认 Tab 外观和主内容布局。
- [ ] 增加 Log 与 Diff 的 Tab 状态，默认 Diff 在新 Tab 主内容区打开；保持提交、文件、过滤器、分隔条和 Log 视口。底部区域默认不占位，不以现有 expanded 布尔值替代 Tab 生命周期。
- [ ] 以仓库、比较双方 revision／本地 area、文件路径定义 Diff 身份；Log 后台选择变化不能悄悄改写已固定审阅对象。预览 Tab 是否复用、如何固定及关闭行为按源码与实测确定。
- [ ] 增加设置页，提供“Diff 打开方式：独立 Tab（默认）／底部预览”，在用户应用数据目录持久化，旧配置缺少该字段时采用新默认，异常配置安全回退。
- [ ] 两种布局共享 Diff 数据读取、导航和只读约束；切换设置保留当前审阅目标。只让活动视图持有重型 Diff 状态，非活动 Tab 保存轻量描述并按需重载；关闭时取消对应任务并释放资源。
- [ ] 验收：默认新安装打开 Tab，设置后重启恢复底部预览；Tab 返回／关闭恢复 Log 选择和视口；切仓、刷新本地文件、请求未完成时关闭或切换不串内容；普通／merge／rename／本地修改均可用。滚动本身由用户手测。

R3 基础进展：独立 Diff Target／Identity 已接入当前读取流程；Linux／Windows 真实 Git 测试确认捕获版本不随 HEAD 前进改变。尚未实现独立 Tab UI、标题与 Log 选择分离、viewer 释放及设置，不勾选 Tab 验收。

R3 接线进展：独立 Tab 实体、非活动 viewer 释放、打开／关闭及底部模式设置已写入代码；Windows 编译通过，原生只确认默认 Log 全高，完整交互仍待验证。按用户补充，Tab 须显示完整文件，已接入完整 Git 上下文并关闭默认折叠；300 行真实 Git 回归通过。纯重命名／仅权限变化全文、Tab 原生流程和设置重启仍待完成。

R3 原生进展：Enter 打开全文 Tab、返回 Log 后改选提交、旧 Tab 内容保持、关闭恢复 Log，以及底部设置重启恢复均已原生验证，见 `artifacts/diff-tabs`。完整生命周期压力与视觉还原仍未通过；纯重命名全文、版本标题及文件切换继续待办。

执行顺序：完成源码与原生行为基准 → R1 共享布局及真实过滤 → R2 高亮 → R3 Tab／设置及生命周期 → 同一 Windows 构建截图复核、内存复测、更新便携包。每个阶段记录实际完成项；新增控件不以占位实现验收。内存验证同时覆盖默认 Tab 和底部模式，保持既有预算及 Git 子进程口径。

## 开发里程碑

按一名熟悉 Rust 的开发者全职投入作粗估。新增双栏 diff 和本地变更后，原 5～9 周估算不再作为承诺；M0 验证组件复用、Windows 内存和交互后重新估算。先交付可用的纵向流程，再完善边界。

| 阶段 | 工作量估算 | 可交付结果 | 退出条件 |
| --- | --- | --- | --- |
| M0 技术验证 | 3～5 天 | Windows 上运行的审阅原型 | 真实 graph、单文件双栏 diff、本地变更入口可用，记录内存基线 |
| M1 数据与布局 | 5～8 天 | Git 读取层、可测试 graph 引擎 | 图不变量及边界测试通过，分页与一次加载结果一致 |
| M2 浏览体验 | 待 M0 重估 | 分支树、主界面、详情、导航、手动刷新 | 无错位，选中稳定，旧异步响应不会串到新仓库 |
| M3 审阅闭环 | 待 M0 重估 | 双栏 diff、本地变更及冲突标记 | 根提交、merge、重命名、二进制等行为明确 |
| M4 稳定化交付 | 4～8 天 | 可日常使用的 Windows 构建 | 验收矩阵通过，性能有记录，可从干净环境启动 |

### M0 技术验证

- [x] 创建最小 Rust 工程，锁定能在 Windows 构建的 GPUI 版本。
- [ ] 验证中文、英文、DPI 缩放、窗口调整和鼠标滚轮。
- [x] 用系统 Git 读取一个真实仓库的首批提交和父子关系。
- [x] 实现最小固定行高 graph，包含分叉和 merge。
- [ ] 点击提交后可查看单文件双栏 diff，验证对齐、同步滚动和文本复制的实现可行性。
- [ ] 读取本地变更列表，选择一个未暂存文件查看 diff。
- [ ] 记录 Windows 空窗口、首批历史、双栏 diff 的内存，确认框架基础开销是否满足预算。
- [ ] 追加第二批历史，验证线条与已有轨道连续。

决策点：若 GPUI 的 Windows 支持、文本交互或基础内存不满足要求，暂停扩展功能，评估其他原生 UI 方案后再定技术栈。若复杂历史布局正确但不够美观，继续 M1；美观不作为技术验证的阻断项。

### M1 数据与布局

- [x] 实现仓库识别、refs 与 HEAD 读取、固定历史快照。
- [ ] 实现提交批量读取、加载更多、后台任务取消与错误分类。
- [x] graph 布局成为纯 Rust 模块，与 GPUI 分离。
- [ ] 支持普通 merge、多父 merge、多个根和浅克隆边界。
- [x] 建立人工 DAG 用例和一次加载／分批加载一致性测试。
- [ ] 对 graph 宽度、轨道数量与线段数量记录诊断数据。

### M2 浏览体验

- [ ] 实现可见行绘制、refs 标签、提交详情及键盘选择。
- [x] 实现分支树及分支可达历史筛选；切换范围生成新快照，不在旧图中简单删行。
- [ ] 实现提交 ID 复制、已加载历史查找与定位。
- [ ] 实现加载中、空、失败、超限、已到结尾等状态。
- [ ] 刷新时使用新快照，保留仍存在的选中提交和视口锚点。
- [ ] 完成仓库切换及快速点击的异步竞态测试。

### M3 审阅闭环

- [x] 实现稳定的比较基准：普通提交对父提交，merge 对第一父提交，根提交对空树。
- [x] 从结构化 Git 输出读取变更文件，不从 patch 标题猜测路径。
- [ ] 实现双栏 hunk 对齐、行号、增删配色、同步滚动、差异跳转和选择复制。
- [ ] 支持已暂存、未暂存和未跟踪文件，以及 unborn HEAD。
- [x] 显示当前未合并路径；验证外部 Git 操作后刷新能正确更新状态。
- [ ] 支持新增、删除、修改、重命名、权限变化的展示。
- [ ] 为二进制、子模块、空 patch 和大文件提供明确状态。
- [ ] 对 patch 内容和解析结果设置有界缓存。

### M4 稳定化交付

- [ ] 完成下列正确性与性能验收。
- [ ] 验证应用关闭后无遗留 Git 子进程。
- [ ] 验证普通仓库、linked worktree、中文路径及空仓库。
- [ ] 用真实多分支仓库检查交叉线、长标题和大量 refs。
- [x] 提供构建方法、系统 Git 前置要求、限制和操作说明。
- [x] 保留一个可复现的性能基线及测试仓库生成方法。

## 验收标准

### 正确性

| 场景 | 通过标准 |
| --- | --- |
| 线性历史 | 节点顺序与关系正确，无多余分叉 |
| 分叉及合并 | 每条父边能追踪到正确提交，没有伪造连接 |
| 三个或更多父提交 | 全部父边可见，详情列出全部父提交 |
| 跨视口长边 | 两端不在视口时，中间线段仍连续 |
| 分批加载 | 与相同输入一次布局的结果一致，旧行不发生重排 |
| 刷新 | 选中对象按 ID 保留，允许新快照重新分配轨道颜色 |
| 浅克隆 | 截断父边明确标识，不误报为真正根提交 |
| 快速选择 | 最终内容始终对应当前选中的仓库、提交和文件 |
| Diff | 与相同参数的 Git 输出一致，旧新行号递增规则正确 |
| 特殊路径 | 空格、中文、括号及 Git pathspec 特殊字符不会选错文件 |
| 超限与错误 | 有说明及可执行的下一步，不显示伪完整结果 |

### 性能目标

先在 M0 记录测试机 CPU、内存、显卡、磁盘、Windows 和 Git 版本，再用同一环境比较。冷缓存和热缓存分别记录。

- 普通本地仓库约 10,000 个提交、同时活跃轨道不超过 20 条时，首批 1,000 条从打开到可交互目标为 2 秒内。
- 在已加载 10,000 条历史上连续滚动，目标为接近 60 FPS；以帧时间 p95 不超过 20 ms 作为初始观察指标。
- 点击选择后 100 ms 内显示选择和加载反馈；patch 生成耗时单独记录，不阻塞 UI。
- 内存作为发布门槛：初始目标为空窗口 ≤ 80 MiB；加载 10,000 条历史并显示一个 ≤ 1 MiB 的文本 diff，稳定态 ≤ 150 MiB，峰值 ≤ 250 MiB。均为待验证预算，M0 给出是否可行的结论，不能静默上调。
- Windows 主指标为应用及其 Git 子进程合计的 Private Bytes；同时报告 Working Set、独立／共享 GPU 内存和 Git 子进程峰值，不能只报告 Rust 堆大小。
- 固定仓库连续切换 100 个文件、刷新 20 次、打开关闭仓库 10 次，记录稳定态与峰值；回到初始状态后不得出现持续线性增长。
- 超过目标规模或数百活跃轨道时，允许变慢，但不能阻塞窗口、无限增长缓存或悄悄丢失边。

内存实现约束：列表与 diff 仅排版可见区域；完整提交说明和文件内容按需读取；初始 diff 缓存上限 32 MiB，历史窗口上限 10,000 行，可替换窗口继续浏览更早历史；历史窗口外只保留有界的分页／轨道边界信息。单文件文本读取上限暂定 5 MiB，超限显示原因，不静默截断。限制同时运行的 Git 任务和输出缓冲，过期任务终止并回收子进程；空闲时不持续重绘、不后台建立全仓库索引。上述子预算不代替总进程内存验收。

这些数字是初始预算。若未达标，先分别测 Git 遍历、布局、文本排版和绘制，依据瓶颈决定优化，不提前引入数据库或复杂索引。

## 主要风险与应对

| 风险 | 应对 |
| --- | --- |
| GPUI 依赖变化 | 固定版本，基础 UI 验证后再扩展，不持续追随主分支 |
| graph 算法边界错误 | 纯函数设计、人工 DAG、分页一致性与随机 DAG 不变量测试 |
| 复杂历史过宽 | MVP 允许水平滚动，暂不追求全局最少交叉或最窄布局 |
| 文本选择工作量超预期 | M0 检查可复用组件；M3 单独验收跨行复制，不把颜色文本当完成 |
| 后台请求串线 | repository epoch、snapshot ID、selection generation 三层校验 |
| 全仓库性能不稳定 | 首批限制、用户触发加载、有界队列及取消，记录后台遍历成本 |
| 功能不断扩大 | 写操作限于用户已授权的基础操作、Commit/Amend与Squash；不因参考截图出现而扩大到通用编辑器、语义索引或远程服务集成 |

## 后续演进顺序

首版稳定后，优先增加“选定分支相对基线的改动”和显式触发的合并冲突预检，再考虑自动刷新及其他审阅增强；已有字符强调和语法高亮继续按当前范围验收。分支改动比较必须区分 merge-base → 分支与两个 tip 直接比较，界面标明基准。

合并预检不能直接对源仓库执行写入对象的操作；评估 Git merge-tree 时需使用隔离的临时对象目录，确保源工作区、index、refs 和对象库不变。需验证 Git 版本、合并驱动行为、缺失对象和临时资源清理；这些条件未满足前不显示“可安全合并”。

后续仍需收敛浏览只读边界、授权写操作的恢复、异步竞态和性能验收缺口；具体接续顺序见文首当前状态；M0～M4 的未勾选项仍按原要求逐项验证，不能将便携预览包等同于验收完成。滚动交互按用户最新指示由用户手测，不继续花时间做滚动自动化。

## 当前证据核对（2026-10-15）

本节区分实现存在和完整验收通过，不缩减上文范围。代码、测试与 artifacts 是依据，历史 README 中的观察不自动视为最新版本全部通过。

| 要求 | 当前证据 | 结论／仍需补齐 |
| --- | --- | --- |
| 仓库、refs、HEAD、固定快照 | `tests/repository.rs` 的 refs 与 branch_snapshot_paging_merge_and_worktree；真实 Git 与 Windows 原生运行已通过 | 本项可勾选；普通仓库和 worktree 已有数据层证据 |
| 批量历史、扩展和边界连线 | history/graph 模块；定日期交错分支三页布局一致性；`artifacts/history-roundtrip` | 已有分页与往返证据；不等于所有复杂历史的 UI 验收 |
| 本地变更分类 | `artifacts/local-comparisons` 的 staged/unstaged/untracked；`artifacts/untracked-added` 的 unborn 新文件 | 流程已有证据；未跟踪路径边界仍有缺口 |
| 未跟踪文件不越出仓库 | git.rs 现逐段检查 symlink/reparse point；Linux 符号链接与 Windows junction 替换父目录测试通过 | 已覆盖列表读取后、预览前替换；检查与打开之间并非原子操作，该并发边界仍不能标为完全通过 |
| 图、refs、详情、复制、查找 | 当前 ui.rs；`islands-details3`、`islands-tabs4`、`history-roundtrip`；compact refs 和文件图标截图 | 多项独立证据存在；最新版本键盘焦点组合、全部 DPI 等仍未统一验收 |
| 合并、多父、浅边界 | graph 测试、32 父 merge fixture、shallow-boundary 截图及真实 Git 测试 | 核心实现和部分原生证据存在；不将全部父边/详情的可见性笼统勾选 |
| Diff、重命名、权限、特殊格式 | Git 集成测试、submodule-preview、diff-width、diff-version-titles | mode-only 的 staged/commit 数据层、解析和 Windows UI 已通过；其他特殊格式完整覆盖仍不足，同步滚动由用户手测 |
| 过期响应不能覆盖新选择 | UI 的 generation 校验及 Cancel；repo reads 测试仅覆盖提前取消 | active-job 已覆盖读取中切仓终止旧进程；仍需快速选择与迟到结果不覆盖新内容的回归 |
| 关闭后无 Git 子进程 | Review::drop 取消 token；read 中 kill/wait | active-job 原生慢进程替身回归已覆盖读取中关闭及切仓后关闭；真实 Git 各阶段仍需补测 |
| 刷新保留选中与视口 | refresh-anchor 截图、history roundtrip、代码按 ID 恢复 | 部分通过；外部 refs 改动及复杂图锚点仍需组合验证 |
| 内存稳定态 | current-memory：10,000 提交 + 975,000 字节 Diff，20 次 Private Bytes 均 83.62 MiB；GPU 单独记录 | 该稳定态样本通过 150 MiB 预算，不能外推瞬时峰值／长期无增长 |
| 峰值、子进程峰值、冷热启动、反馈延迟 | 新增 Job Object 从挂起启动前纳入应用及子进程，线性 10k + 大 Diff 进程组峰值 118.35 MiB；startup 仍固定等待截图 | 本场景峰值已有内核统计证据；Git 独立峰值、冷热启动、响应延迟及压力组合仍待验证，整体发布门槛未通过 |
| 空闲不持续重绘 | current-memory 约 20 秒 CPU 0.516 秒 | 不能由此确认绘制次数；仍需直接观测 |
| 100 文件／20 刷新／10 次切仓 | combined-job-memory 为当前构建同进程组合，Job 峰值 128.81 MiB；10 次重开采样 82.27–84.48 MiB | 本轮加载 1,000 条以匹配重开状态；回初态比首次高 11.79 MiB，长期增长结论仍未证明 |
| Islands 视觉还原 | 原生 Rebased/IDEA 参考、源码规则、最新 file-icons 截图 | 有持续改进但仍未像素级对齐，不宣称完成 |
| Windows 交付 | dist 便携包、package-windows.py、portable-package 原生解压启动证据 | 已交付可手测预览；不是干净 Windows 环境和正式发布验收 |

优先顺序：修补未跟踪路径检查 → 在运行任务的取消／切换／关闭回归 → 可解释的性能测量与峰值采样 → 对照参考收敛剩余核心布局。滚动不占用自动化排查时间；未确认项保留待手测状态。

## 参考资料

- [Rebased](https://github.com/detachhead/rebased)：独立 Git 客户端定位及 IntelliJ 平台背景。
- [IntelliJ Log tab](https://www.jetbrains.com/help/idea/log-tab.html)：分支、提交、文件和详情布局。
- [IntelliJ Diff Viewer](https://www.jetbrains.com/help/idea/differences-viewer.html)：差异浏览交互参考。
- [Git status](https://git-scm.com/docs/git-status)：工作区、index 与未合并状态。
- [Git merge-tree](https://git-scm.com/docs/git-merge-tree)：未来合并预检的能力及副作用边界。

### 快速文件切换回归（2026-10-15）

Windows release 原生窗口使用独立仓库，在 13,000 行大文件与两行标记文件间以 15 / 50 / 100 ms 间隔分别发送 12 对点击。三组结束后及额外等待 5 秒后的截图均显示 `z-final.txt` 选中、相同 Diff 路径及 `FINAL_SELECTION_SENTINEL` 正文；先行截图确认大文件也能够显示。脚本为 `scripts/windows-rapid-files.ps1`，证据位于 `artifacts/rapid-file-selection/`。

这是实际输入序列的最终内容一致性回归，不保证每次中间请求都执行，也未强制构造旧请求晚于新请求完成的时序，不能替代完整异步竞态证明。未执行滚动测试。

### 当前分支图标（2026-10-15）

分支树叶节点改用 JetBrains 标签形图标；由 snapshot.current_branch 的完整本地 ref 判断当前分支，采用 Rebased `platform/dvcs-impl/shared/resources/icons/new/currentBranchLabel_dark.svg` 的 #F2C55C，复用已有相同形状资源。参考版本 4aec35256d765c217aa0d662b82ac4e860c396fd。Windows release 构建与启动通过；`artifacts/branch-current-icon` 确认 feature/ai-review 为金色，点击 main 后 main 蓝色选中而当前分支仍金色，远端同名 main 不误标。已更新便携预览包；尚不等于整体视觉验收。

R1 进展（2026-10-15）：修复文件工具栏 30px 与 Log 36px 导致的首行错位，共用高度且禁止压缩；Windows release 截图验证默认尺寸对齐，见 `artifacts/toolbar-alignment`。过滤器、窄窗和其他 DPI 尚未完成，R1 整体保持未勾选。

R1 进展（文本查询数据层）：已按 GitLogProvider / TextFilterModel 规则实现仓库历史消息过滤与过滤后分页，真实 Git Linux／Windows 测试通过。尚未接入过滤 UI，保留真实 parent IDs，需先解决隐藏祖先连接；不将该步骤视为完整文本／哈希过滤或 R1 完成。

R1 进展（过滤图算法）：已翻译 Rebased DottedFilterEdgesGenerator 的完整 DAG 双向遍历，区分直接父边和隐藏路径虚线；Linux／Windows 各 4 项测试通过。代码在 `src/filtered_graph.rs`。分页装配、边界箭头、虚线绘制与过滤 UI 尚未接入；临时完整图 100,000 节点上限不能当作最终历史范围，后续仍需有界装配方案。

R1 进展（虚线布局／绘制接线）：投影边类型进入 lane frontier 与 Segment，新增半行虚线绘制路径；普通历史仍走实线。Linux 测试验证跨行／恢复和合流类型，渲染代码通过 Windows 编译。尚未完成查询到投影图的装配和原生虚线截图，不能据此勾选过滤交互验收。

R1 进展（过滤结果装配）：已连接完整快照 ID、匹配集合、投影边和有界行布局，分页重建只保留当前窗口几何；真实 Git Linux／Windows 用例确认过滤图不改变 Diff 父基准。完整输入分页不等于大仓库输入流式支持；后续仍需边界箭头、取消及过滤 UI。

R1 进展（轻量图输入）：新增仅含 ID／父关系的固定 roots 分页读取，并让过滤图装配直接接受轻量节点，避免为隐藏提交持有作者／标题。Linux／Windows 合并历史真实 Git 用例通过。每页有界不等于完整投影内存已验收；大图总上限、CPU 取消和 UI 接入仍需完成。

R1 进展（可手测文本过滤）：Filter messages 输入＋Filter 按钮已接入历史查询与折叠图。Windows 截图确认过滤、虚线、Diff 真实父基准、清除和空结果；Linux 33 项核心测试通过，预览包已更新。键盘首次 Enter 的自动输入表现仍需核对；User／Date／Paths、统一文本哈希入口、图 CPU 取消及大仓库限制仍未完成，R1 保持进行中。

R1 进展（过滤模式）：`.*` 正则和 `Cc` 大小写开关已接入 UI，点击立即重新查询；Windows 原生验证普通／大小写／正则／错误正则及关闭正则恢复，截图位于 `artifacts/filter-options`。预览包更新。控件内嵌样式、模式持久化、统一哈希和其余过滤器仍待实施。

R3 验证补充（2026-10-15）：Linux 核心 18 单元＋20 集成测试通过；Windows 宿主机 Git 的 300 行完整上下文用例通过，确认中间一行改动时仍包含首尾。活动 Tab 刷新路由通过 Windows release 编译。便携预览包已更新并校验 ZIP CRC 与程序 SHA-256（471e86a682b2eb01c8d7fa8c6c1d6002413d9261d265050b4bb7444cedbcfc0a）。纯重命名／仅权限变化的全文、Tab 视觉和生命周期压力测试仍待补齐。

## R3 无文本变更片段的全文（2026-10-15）

重新读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `ChangeDiffRequestProducer.java::createContent`，确认读取 revision 全文不依赖变更片段。`git.rs::patch_context` 对 100% 相似重命名及仅普通文件权限变化的无 hunk 输出，使用 `cat-file blob` 读取提交版本或 index，补为两侧未修改上下文；保留 metadata，处理 UTF-8 路径、缺少末尾换行、二进制拒绝、取消和输出限额。底部紧凑模式不变。

Linux 18 单元＋21 集成测试通过；Windows 两项真实 Git 测试通过，覆盖 staged／committed chmod、中文重命名、缺少末尾换行和提交后工作区改动不泄漏。Windows release 原生截图 `artifacts/diff-tabs/rename-full-file.png` 确认 before.txt → after.txt 两侧显示全部两行，增删数 0，正确保留无末尾换行提示。仅关闭本轮测试窗口。完整视图的 @@ 标题和多余文件头、版本标题、Tab／工具栏像素差异仍待处理；此项不代表 R3 整体完成。

## R3 版本标题及单侧布局（2026-10-15）

已读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 ChangeDiffRequestProducer.java（前后 revision 独立标题）、TwosideTextDiffViewer.java::createTitles、OnesideTextDiffViewer.java::createTitle 和 OnesideDiffViewer.java::canShowRequest：空侧采用单侧 viewer，标题选择有内容的一侧。

`src/diff_tab.rs` 为左右栏提供独立版本／路径标题，merge 标注第一父提交，本地区分 HEAD／Index／Working tree；新增／删除使用单栏模式。关闭重复文件头，以 LineInfo 分隔策略隐藏完整上下文中零缺失行的 @@ 标题；单独保留 old/new mode 信息。上下差异按钮复用已归档 JetBrains SVG。

Windows release 编译通过，原生 `artifacts/diff-tabs/titles-split.png` 确认纯重命名双栏标题／正文对齐，`titles-added.png` 确认根提交新增文件单栏。未单独验证删除和仅权限变化的新布局。与用户 diff-tab.png 相比，单栏仍有 + 标记，字体／行号留白、绿底色、右侧变化条、锁图标、Tab 外形和工具栏文件导航待还原；当前截图不同数据及窗口条件，不能作为像素级验收通过。未重复滚动测试，已关闭本轮专用窗口。

## R3 文件列表导航（2026-10-16）

已读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/impl/CacheDiffRequestChainProcessor.java`：当前 index 选择 request provider，前后导航受列表边界约束，切换更新 request；另有 GoToChangePopupBuilder 文件选择入口。

`diff_target::Chain` 保存一份 commit／repository 和 Arc 文件元数据列表；导航生成当前 Target，不加载其他文件内容。当前按选中文件 area 分组，避免混淆 staged／unstaged 版本；独立打开的多个 Tab 尚各自复制文件列表，跨 Tab 共享与总体规模仍需优化。`DiffTab::move_file` 取消旧请求、释放 viewer 后按需加载新文件，Review 观察 Tab 更新标题。工具栏接入左右 JetBrains SVG、文件序号和边界禁用状态，资源许可来源已记录。

Linux／Windows 真实 Git diff_chain_keeps_revision_order_and_stops_at_boundaries 通过：固定提交、HEAD 后续前进不影响目标、顺序／边界及返回原目标。Windows release 原生 `artifacts/diff-tabs/chain-{first,second,previous}.png` 确认 a→b→a、序号及 Tab 标题更新；中间回 Log 选择 Base 再回 Tab，仍浏览原 b63e0066。仅关闭专用测试窗口。

仍缺少点击序号打开文件选择弹窗、导航快捷键、跨文件差异跳转及精确尺寸；当前蓝色序号为静态状态文字。多 Tab 大文件列表内存与快速切换竞态未验收，不据此勾选整体 R3。用户截图和当前样例数据／窗口条件不同，尚未达到像素级验收。

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

## R3 文件选择交互／图标（2026-10-16）

再次读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GoToChangePopupUtil.kt::onDoubleClick／GenericChangesBrowserNode::render：双击确认文件，文件状态颜色、文件类型图标及目录计数。`diff_picker.rs` 改为单击选中、双击或 Enter 确认；独立 chevron 点击展开收起目录，复用 ui.rs 的文件类型资源与 Islands 状态色，文件序号改为 link variant。

为避免改变其他 GPUI 列表，vendor/gpui-component ListDelegate 新增默认 true 的 confirm_on_single_click，仅 FilePicker 返回 false；List 点击先同步 selection，再按策略确认，键盘确认不变。Windows release 编译通过。原生 `artifacts/diff-tabs/picker-style-{opened,single,enter,collapsed,double}.png` 验证单击留在原 b.txt Tab，Enter 打开 c.txt；目录收起／展开；双击重新打开 b.txt。仅关闭自己的测试窗口。

待完成：link variant 当前主题仍渲染白色及悬停下划线，需对齐 Rebased 蓝色；收起包含选中项的目录后应明确回选目录并校准键盘状态；搜索后首选项、左右键、分组工具栏、弹窗尺寸、颜色／字体和同条件原生参照仍待验收。本轮未重新做大规模内存或滚动测试。

## R3 文件选择器选中状态（2026-10-16）

重新核对参考 GoToChangePopupUtil.kt 的单选／初始文件索引／节点确认路径。本地修正 row index 在目录折叠后错位的问题：点击 chevron 重建后按目录路径定位并选中目录；搜索完成在 GPUI List 默认重置之后选中第一个匹配文件，空查询优先当前文件，异步回调检查 query 防止旧查询覆盖。搜索首选文件为本地交互决策，尚未等价验证 JetBrains speed search 的完整算法。文件序号子元素显式使用蓝色。

Windows release 编译通过，原生 `artifacts/diff-tabs/picker-selection-{collapsed,enter-folder,search-file,enter-file}.png` 确认收起后目录选中、Enter 只展开；搜索 c.txt 选中文件、Enter 打开 3/3 正确内容。蓝色序号显示正确。首次在树获得焦点后直接输入字符没有进入搜索，随后显式聚焦输入框验证通过；树 speed search 和左右键仍待实现。未测试空查询恢复全部组合，也未做大规模搜索压力，整体 UI／R3 保持进行中。测试窗口已关闭。

## R1 Date 查询与预设（2026-10-16）

已读参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 DateFilterPopupComponent.java（Calendar 当前日期减 1／7 日、Select…、选中范围文案）、VcsLogDateFilterImpl.java，以及 GitLogProvider.kt 的 DATE_FILTER 参数生成：after／before 交给 Git。将原 TextFilter 命名扩展为 `LogFilter`，增加固定 Unix 秒上下界，metadata 和 topology_filtered_page 共用参数；文本与日期组合，已有哈希匹配优先逻辑保留。UI Date 菜单接 Last 24 hours／Last 7 days 和清除，切仓清除日期，分页保留已捕获边界。

Linux 20 单元＋23 集成测试通过；Windows 新增 date_filter_uses_commit_time_combines_text_and_keeps_paging 通过，覆盖作者时间相同但提交时间不同、上下界、文本组合、分页、哈希优先及 HEAD 不变。Windows release 原生 `artifacts/date-filter` 验证最近一天 1 条、一周 2 条、清除恢复 3 条；工具栏和文件首行仍对齐。仅关闭本轮测试窗口。

差异／待办：当前预设在点击时减固定 86400 秒，而参考为打开菜单时 Calendar 日运算，夏令时地区可能不同；Select… 自定义日期 UI 尚未接入（后端 before 已支持），暂使用 All dates 清除；选中日期仅 tooltip／selected 态，不是参考 Since／Until 文案；User／Paths 未完成。日期过滤沿用完整临时图投影上限和性能限制，不能称为任意大仓库验收通过。字体／菜单几何仍需同条件对比。

### R1 Date 日历计算与条件文案（2026-10-16）

参照 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 DateFilterPopupComponent.java，预设改为菜单打开时捕获本地日期，减一天再减六天；工具栏显示 Since 日期。实现为 src/date_filter.rs 与 ui.rs。Linux 45 项测试、Windows 两项日历测试及 release 编译通过；纽约春秋日历日分别跨 23／25 小时。原生 artifacts/date-calendar 验证选一天显示 Since／1 条结果，清除恢复 Date／3 条，工具栏仍对齐；测试窗口已关闭。

待办仍包括 Select… 自定义范围、地区日期格式、DST 重复／缺失时刻的 Java Calendar 消歧（当前报错保留旧条件）、User／Paths。未据此完成 R1 或像素级验收。预览包已更新，CRC 和 release 一致；exe SHA-256 为 25bc10672fa1669c2ad98fc55ba35911c21584458d29e12ef0b2d98bf919838f。


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


### R3 全文件 Windows 复核（2026-10-16）

重新读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/actions/diff/ChangeDiffRequestProducer.java::createContent`：前后 revision 分别通过完整 bytes／content 构造 DiffContent，当前文件通过 VirtualFile 构造。全文是内容模型，差异区域用于标记；不能以有限 patch 上下文代替全文。

本地路径确认：`DiffTab::activate` → `Target::read_full` → `Repository::full_patch`，使用完整 unified context，viewer 的 `with_context_lines(None)` 不再折叠已有上下文。本轮未修改运行时代码。新增真实 Git 测试 `full_diff_local_versions_keep_index_and_worktree_separate`：300 行文件分别在第 150 行暂存、第 250 行工作区修改，确认 HEAD→Index 与 Index→Working tree 全文及差异相互隔离。Linux／Windows 三项 full_diff 测试均通过，包含提交全文和纯重命名／无末尾换行。

Windows 原生使用独立两提交、300 行样例（仅第 150 行变化），Ctrl+D 打开独立 Tab。`artifacts/full-file-native/tab-ready.png` 确认两侧从第 1 行开始显示连续未修改内容；未做滚动自动化，末尾内容由数据层测试验证，不称为原生全范围视觉验收。首次截图全黑，重新捕获后正常；黑图不作为通过证据。测试 exe SHA256 为 `074be56562a8ed70be5980603e6e25cd6613fcfa72310cfb6af95ace54938a61`，已逐字节确认与当前预览 ZIP 内 exe 相同。截图、启动信息和验证范围已归档。仅关闭本轮 PID 33196，未操作用户 Rebased。

保留差异：全文 patch 仍受 8 MiB 输出限额，超限报错而非静默截断；语法高亮、Islands Tab 外形／工具栏／行号留白仍未完成像素级还原。该记录只补足全文行为证据，不代表 R3 整体完成。


### R1 User 作者补全接入（2026-10-16）

重新读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `ValuesCompletionProvider.java`／`TextCompletionProviderBase.java`：固定候选集合、排序、PlainPrefixMatcher、分隔符隐藏候选。沿用此前已核对的光标前缀／UTF-16 匹配规则。

`src/user_menu.rs` 实际接入候选列表和输入监听：每次打开 draft 创建一次作者展示集合；正文／光标快照未变时跳过扫描；候选用索引保存、uniform_list 只渲染可见行（最多八行高），无全局缓存。按光标前的当前 pipe／newline 分段补全，借助输入组件的 selection + replace_text_in_range 保留正常编辑路径。支持 Enter、Tab 接受，方向键选择，Ctrl+Space 显式列出；Ctrl+Enter 应用 draft；Esc 首先隐藏候选、第二次取消草稿。关闭或切仓释放输入 observer 与候选集合。64 KiB 以上不运行补全；catalog 原有边界不变。

Windows release 原生 PID 23136 验证 `Bob Smith|Ali` → Enter → `Bob Smith|Alice Chen`，Ctrl+Enter 后显示 Bob 与 Alice 的三条提交。重新编辑成 Ali，第一次 Esc 保留草稿并关闭候选，第二次取消且保持原过滤。截图 `artifacts/user-completion/{candidates,accepted,applied,escape-lookup,escape-draft}.png`。Linux user_filter 四项测试通过，Windows 编译通过；本轮没有单独验证 Tab、方向键多候选、Ctrl+Space、撤销和光标位于词中时的原生行为，不能列为已验收。

视觉仍未通过：当前列表附于 draft 提示栏下方，尚未实现跟随光标的独立 completion lookup 浮层及相应阴影、尺寸、匹配强调。本轮读取原生 Rebased User 菜单；随后前台保护拒绝继续点击，因此未取得其候选浮层的新截图，没有绕过保护或更改用户过滤。需继续原生参考对照。源码与已安装 Rebased 版本不保证一致。仅关闭自己的测试 PID，用户 Rebased 保留。预览 ZIP 更新并通过 CRC／exe 字节校验，exe SHA256：`714e3215db0586ea7fa9d59675640ad7df0fca225b4d4daff9f4633156b28613`。


### R1 作者补全浮层定位（2026-10-16）

读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/lang-impl/src/com/intellij/codeInsight/lookup/impl/LookupUi.java::calculatePosition`：按 lookupStart 的实际文本坐标定位，在该行下面显示，扣除候选文本缩进；屏幕下方不足时可改到上方。不是附在 draft 底部的普通布局列表。

`src/user_menu.rs` 改为独立 deferred + anchored 浮层，用输入组件 `range_to_bounds` 获取 UTF-8 前缀起点的实际字形几何；优先级 101 位于父 popup 的 100 之上，保留虚拟列表，加入独立边框、圆角及阴影，不增高 draft。Windows 发现伸出父 popup 的候选被外部点击逻辑误关闭，已记录实际浮层 bounds 并在候选打开时由本组件处理外部点击，避免把候选点击当成取消。关闭 editor 时释放 bounds 状态。

Windows release 原生 PID 9528 验证：两行 `Bob Smith`／`i` 显示 Alice Chen 和 Bob Smith；浮层可延伸出父弹窗；点击延伸区域的 Bob Smith 后第二行正确补全、第一行及草稿保留；重新触发候选后点击真正外部区域取消草稿。证据 `artifacts/user-lookup/{multiline,mouse-accepted,outside-dismissed}.png`。较早 PID 32452 暴露的误关闭截图不作为通过证据；该进程及本轮自建测试进程均关闭。未做滚动自动化。

尚未像素级验收：宿主 Rebased 截图读取成功，但点击时前台检查失败，因此未新取得其 completion 浮层参考；未绕过点击保护。当前固定 200 logical px 宽度、边缘 snap 而非源码的上下翻转策略、匹配文本强调、上一帧 range geometry 与快速换行定位仍需完善／验证。预览 ZIP 更新并校验，exe SHA256 `e018177aec496f4bbb5f0d3d24eae68fee6cf511a7f8df03a3a2613e0fc18f4f`。


### R1 补全浮层边缘布局（2026-10-16）

继续读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `LookupUi.java::calculatePosition`：下方不足且上方更多时翻到输入行上方；裁切可用高度时不得覆盖当前输入行。新增 `src/lookup_layout.rs` 将逻辑像素布局独立为纯函数，`UserMenu::body` 传入当前 window viewport 和实际前缀字形边界，按结果限制列表高度、横向坐标及宽度。候选文字缩进修正为 border 1 + padding 4 + row padding 8 = 13 logical px，使候选文本与前缀起点一致。

Linux／Windows 两项几何测试通过，覆盖下方默认、向上翻转、两边不足、横向越界、零尺寸窗口。Windows release 编译通过，原生 PID 32588 的 `artifacts/user-lookup-layout/aligned.png` 确认普通窗口词首对齐；没有以该截图宣称边缘翻转原生验收通过（目前窗口最小高度 480，顶部过滤栏样例未触发翻转）。仅关闭自己的测试进程。

保留差异：Rebased 使用屏幕允许区域并在同一 lookup 生命周期保留 above 状态；本地 GPUI 浮层限于自身窗口且每次布局重新选择方向。固定宽度、匹配强调和上一帧文本几何仍需后续完善。读取宿主前台归属为 Paseo，未向其发送参考窗口点击。预览包通过 CRC 与 exe 字节校验并更新，SHA256 `0fcdef8e925c52f5d10d827ec0e13732c7c37964d684f3331ee1585750b47f32`。


### R1 Paths 查询与真实父关系（2026-10-16）

读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/log/GitLogProvider.kt` STRUCTURE_FILTER 分支：非空 paths 追加 `--full-history --simplify-merges`，路径置于 `--` 之后；不添加 `--follow`。读取 `VcsLogStructureFilterImpl.java`：多路径是并集，文件／目录祖先匹配检查 before 和 after 路径。读取 `StructureFilterPopupComponent.java::EditPathsAction`：手动路径编辑按换行分隔，空确认清除；树选择使用 VcsStructureChooser。这些作为后续 UI 依据，不表示 UI 已完成。

真实 Git 小样例确认 `--simplify-merges` 会把 `%P` 改写为路径相关祖先，不能直接用来构造 Diff Target。`LogFilter` 新增 literal repository-relative paths，`git.rs` 在两类查询中追加结构过滤标志及路径参数；直接 `snapshot_filtered_at` 通过不带过滤的指定 ID 查询恢复 metadata／真实父关系。`history::read_filtered` 对 paths 使用完整拓扑、结构查询的命中 ID 集合，再沿完整图投影／分页并按指定 ID 加载 metadata；避免简化历史排序与完整图排序不一致，同时保留已有折叠边和第一父 Diff 语义。`topology_filtered_page` 的结构过滤父 ID 只用于命中查询，已明确注释不能作为 Diff／原图父关系。

边界：路径用 / 分隔，相对仓库根；拒绝绝对路径、父目录跳转、空片段、反斜线、冒号及 NUL；最多 128 路径／8192 UTF-16 单位。`.` 可选择仓库范围。未新增全局缓存，沿用图 100,000 顶点／500,000 边、Git 输出／超时／取消限制。结构查询自身仍可能在输出第一页前遍历较多历史，与 Rebased flags 一致；大仓库延迟及参数总长（含其他过滤器／roots）尚需进一步评估。

新增三项真实 Git 回归：方括号文件名不被当通配符、目录与多路径并集、重命名两侧及不跟随旧名称、与文本条件组合、分页、空结果、哈希优先、原始父提交和 merge 第一父 Diff、非法／过量路径。Linux 34 单元＋30 集成测试通过；Windows 原生 Git 全部 30 集成测试通过；Windows desktop cargo xwin check 通过。

当前只完成查询层，UI 仍构造空 paths，未增加无行为的 Paths 按钮。下一步接入源码对应的编辑／选择菜单及持久化状态，随后 Windows 截图验收；本轮未更新预览 ZIP，不宣称 Paths 已可供用户操作。


### R1 Paths 手动编辑 UI（2026-10-16）

重新读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 StructureFilterPopupComponent.java::createActionGroup／EditPathsAction／path2Text／text2Path 及 VcsLogBundle 文案：Select… 打开 newline-only 多行编辑，Select in Tree… 为另一入口，路径可相对共同根目录或为绝对路径；空确认清除。标题为首路径及额外数量，Recent 保存组。

新增 `src/paths_menu.rs`，Date 右侧真实 Paths 控件，Select… 多行草稿、Ctrl+Enter 确认、Esc／外部取消、最近十组（仅会话内）、方向键和 Enter 菜单选择；Review 将 paths 与文本／作者／日期合并，切仓重置。`src/path_filter.rs` 将 Windows 反斜线、仓库内绝对路径、末尾目录斜线转换为查询层路径，newline-only 分隔保留 | 文件名，去重，空值清除；边界失败保留旧过滤并显示草稿错误。parser 一项测试 Linux／Windows 通过，Windows release 编译通过。

Windows 独立五提交仓库原生 PID 3604：src/ → 三条，../outside 确认失败保留原三条；取消错误草稿后与 Review 文本组合 → 一条，清除文本和路径 → 五条。证据 `artifacts/paths-filter-ui/{applied,invalid,combined-applied,cleared}.png`。首次 SendKeys 的 Review+Enter 只完成输入，追加 Enter 后才应用，combined.png 不是组合查询通过证据。未修改用户仓库，专用测试进程已关闭。

差异与剩余工作：Select in Tree… 尚未实现，因此没有无行为按钮；Recent 未持久化，目录末尾斜线展示、长路径省略、自然排序、弹窗调整尺寸及路径补全未还原；当前校验拒绝包含 .. 的路径（包括最终仍在仓库内的路径），绝对路径 Windows 大小写比较仅 ASCII。尚未获得同状态原生 Rebased Paths 参考截图，不宣称像素级验收通过。预览 ZIP 已更新，通过 CRC 与 exe 字节校验，exe SHA256 `fa83a73d2ebb6328fa91c20ab02d3acf7dbdf187cc66be70a383918247e5de93`。


### R1 Paths 树形选择器（2026-10-16）

对照参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/filter/VcsStructureChooser.java` 和 `platform/vcs-impl/src/com/intellij/util/treeWithCheckedNodes/SelectionManager.java`：树根显示仓库、目录优先、不显示未跟踪文件、保留隐藏文件、Space 切换选择、最多 100 项；父路径覆盖子路径，包含已选子项的父节点显示勾选且复选框禁用，空选择禁用 OK。目录过滤与文件过滤有别，正常目录可枚举；链接目录不展开是本地边界。

新增 `path_tree.rs`、`path_chooser.rs`，`paths_menu.rs` 接入 Select in Tree… 对话框。目录按需读取，Git ls-files NUL 输出流式处理，只保留直接文件名；单目录最多 4096 项／512 KiB 路径文本、扫描最多 8192 项，总展开缓存最多 16384 项。折叠释放子树缓存，关闭取消读取并释放选择器。选择父目录替换子项，初次加载期间禁止覆盖恢复状态。修复 DialogClose API、元素状态类型、动态 OK 刷新以及 open_dialog 接管焦点的问题；底部按钮宽度调整为 80 logical px。

Linux 两项状态／流式索引单元测试和一项真实 Git 目录测试通过；Windows 原生 Git 目录测试通过，release 构建通过。Windows PID 11532 验证鼠标选择 src 后 OK 启用并筛出 3/5 条提交（applied.png 是加载中，applied-ready.png 才是结果）；最终 PID 5848 验证打开即方向键／Space 选择 src、Cancel 保留未过滤 5 条。截图及启动信息存于 `artifacts/paths-tree` 和 `artifacts/paths-tree-final`。这两个本轮进程均已关闭，未操作用户 Rebased，没有滚动自动化。

剩余差异：尚未取得同状态原生 Rebased 树选择参考截图，不能宣称像素级验收；多行选择、speed search、对话框尺寸持久化及左右键完整树导航尚待补齐；目录大小写排序使用 Rust Unicode 转小写，不等价于 Java 的所有字符比较。当前只保留已存在的初始路径，但不自动展开到初选叶子。目录 I/O 在阻塞系统调用内尚不能立即取消。保留内存限额和链接目录限制并明确报错。

预览 ZIP CRC 与内含 exe 字节校验通过，exe 02fdb86bf6ceb31467ffbc3c09fd560edbd2bdbd13fe930fcefc0a3770ba8f7a256 `02fdb86bf6ceb31467ffbc3c09fd560edbd2bdbd13fe930fcefc0a3770ba8f7a`。


### 单搜索默认布局与 Branches 竖条（2026-10-16）

用户截图 `artifacts/rebased-user-reference/single-search.png` 的分支面板是收起状态。读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 中 `plugins/git4idea/backend/src/ui/branch/dashboard/BranchesDashboardTreeComponent.kt` 的 installSearchField／SearchLook.Inline，以及 `BranchesInGitLogUiFactoryProvider.kt` 的 treePanel、ExpandStripeButton、ExpandablePanelController、SHOW_GIT_BRANCHES_LOG_PROPERTY 接线。源码确实在展开树中提供分支搜索；收起时隐藏树及搜索，左侧保留竖向控制条。读取 ExpandStripeButton.kt 和 ExpandablePanelController.kt 全文：图标在上、文字逆时针旋转，点击切换实际面板可见性。

`src/ui.rs` 新增 branches_visible 与 branches_rail，默认按用户截图收起（参考源码主 Log 默认展开，本地显式采用用户目标布局）；点击箭头或竖向文字展开，左箭头收起。隐藏分支面板时不渲染其输入框及列表；Log 只显示一个 Text or hash 框，分支搜索内容仍保留在轻量状态。收起／展开／Local Changes 使用各自 ResizableState，避免按 panel index 误用尺寸，切换不改变 Git 过滤条件。当前显隐状态只在会话内保留，跨启动持久化仍待做。

GPUI 普通文本元素不提供旋转，竖向 Branches 使用随附 Inter Regular 的 SVG 轮廓，源脚本 `scripts/generate-branches-stripe.py`、资源 `assets/ui/branches-stripe.svg`，保留字体 SIL OFL 许可证；运行时无 fontTools 依赖。其字号目前固定 12 logical px，不会跟随未来自定义字体／语言，需要届时重新生成或补旋转文本组件。并非截图位图。

Windows release 构建通过；PID 10808 原生截图 `artifacts/branches-stripe/{window,expanded,collapsed-again,local,returned-log}.png` 确认默认单框、竖条文字展开、箭头收起以及 Local Changes 返回 Log。文件面板垂直边界往返保持，五条提交不变。已关闭本轮进程，未触碰用户 Rebased。此为结构和往返行为验证：参考图和本地窗口 DPI／仓库不同，尚非同条件像素验收；竖条 hover／图标、主 Tab、文件工具栏仍有差异。未做滚动自动化，也未以启动样例内存宣称大仓库内存验收。

预览 ZIP CRC 及内含 exe 字节校验通过；exe SHA256 `0ec8d3aaa68b0bb21a1580dfa9c395bd22f3d90a79fd21b4be5d6bec7c02a602`。


### Islands 主 Tab 外观（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/platform-impl/src/com/intellij/openapi/application/impl/islands/IslandsTabPainter.kt` paintTab／paintUnderline／颜色分支，以及 `platform/platform-resources/src/themes/islands/ManyIslandsDark.theme.json` EditorTabs 和颜色定义：普通模式 28 logical px 内容高度、圆角填充描边、无下划线；活动选中背景 blue-40 #233558、边框 blue-60 #2E4D89，非活动选中为 gray-30／gray-50。用户 `single-search.png` 和 `diff-tab.png` 作为状态／图标参考。

`src/ui.rs::islands_tab` 共用 28 px 圆角 Tab、蓝色选中框、未选中 hover；git_toolbar 去掉额外 Git 文本标题及下划线，Log 添加 vcs 图标，Diff 添加 expui/vcs/diff_dark.svg，关闭使用 expui/general/closeSmall.svg。新增图标直接取自上述 Rebased commit，保留 Apache 版权和 SOURCE 记录。Diff 整个 Tab 可点击（原先仅文字），关闭按钮 stop_propagation 避免关闭后误激活；Settings 打开时不把后台 Log／Local 标为选中。

Windows release 构建通过；原生 PID 34644 用五提交 fixture 验证 Ctrl+D 打开完整 rename Diff、点击 Log 返回且 Diff Tab 保留、点击 Diff 图标区域重新进入、点击关闭移除 Tab 并返回原 Log 选择。截图 `artifacts/islands-tabs/{window,diff-ready,back-log,reactivated,closed}.png`。进程已关闭，未操作用户 Rebased，没有滚动测试；这次只改 UI 样式和点击范围，未重复运行数据层测试。

剩余：尚未区分窗口失焦时的 inactive 选中颜色；参考主题 hover 的透明叠色与自定义 Tab 色未完整翻译，圆角尺寸需同 DPI 核对；Tab 溢出列表、拖动／固定预览、Local Changes 入口在整体工具窗口中的位置、Tab 与内容岛之间留白仍待还原。当前同级 Local Changes 是已有只读入口，尚非 Rebased 侧边 Commit 工具窗口布局。参考截图与本地仓库、DPI 不同，不能称为像素级验收通过。源码参考主题版本也不等于用户安装版全部细节。

预览包 ZIP CRC／exe 字节验证通过，exe SHA256 `d82829b8a8a00e6bca9ed65c3a07846828e0115fdca2973747e64d36c2a2723a`。


### 文件工具栏与展开／收起（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/VcsLogChangesBrowser.kt` 的工具栏封装和高度参照；读取 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/AsyncChangesBrowserBase.kt`（继承 ChangesBrowserBase）、ChangesBrowserBase.java 的 createToolbarActions（Show Diff）和水平工具栏 TreeActionsToolbarPanel 包装；完整读取同目录 TreeActionsToolbarPanel.java，右侧 EAST 排列 Expand All、Collapse All。读取 vcs-log 的 intellij.platform.vcs.log.impl.xml 工具栏组引用，其他动作仍需分别实现，不加占位按钮。

`src/ui.rs::files_toolbar` 替代 Changed Files 标题，左侧 Show Diff、右侧 Expand All／Collapse All；继续使用与 Log 相同的 LOG_TOOLBAR_HEIGHT=36，按钮 28 px，图标为参考 expui/general 的原始 SVG，来源及 Apache 许可证保留。Show Diff 复用 open_diff_tab；无选择禁用，无树目录／分组禁用展开收起。`set_files_expanded` 操作现有目录及 area 状态，按已加载 Change 路径构造祖先集合，不访问文件系统或额外调用 Git。展开清空折叠集合，收起记录全部目录与本地分组；保留已有 selected_file，展开后恢复选择。

Linux 现有 file_tree 三项测试通过（折叠后文件身份／计数、area 隔离、刷新清理旧目录状态），Windows release 编译通过。原生 PID 15068 使用五提交 fixture：collapsed.png 确认 src 收起，expanded.png 确认恢复原文件选择；点击工具栏 Diff 后 diff-ready.png 确认完整 rename 文件打开。window.png 确认左过滤栏和右工具栏下沿／首行对齐；empty-ready.png 记录干净工作区空列表。禁用条件已接入组件，但未额外点击空列表禁用按钮，图标灰度区别尚不明显，不把空截图视为完整禁用交互验收。截图位于 `artifacts/files-toolbar`，本轮进程已关闭，未操作用户 Rebased，未做滚动自动化。

保留差异：收起仍保留审阅文件，尚未翻译 Swing 折叠时把树选中移到父节点的完整语义；目录本身的多文件 Diff 也尚未支持。Show History、展示选项／分组菜单等只读动作仍待加入；写操作不属于范围。按钮 hover／disabled、目录根节点／计数字样及整体像素对照仍待完善。当前只验收三项按钮及布局结构，不代表文件工具栏全部还原。

预览 ZIP CRC 与 exe 字节验证通过，SHA256 `b34d664a4fdc8945074eaaafd91e5c114df4d1a02dfb513800d04aeef65e124c`。


### 当前独立全文 Tab 的内存复测（2026-10-16）

测量构建 SHA256 `b34d664a4fdc8945074eaaafd91e5c114df4d1a02dfb513800d04aeef65e124c`，与 release、Windows 测试 exe 及上一轮预览包一致。环境：Ryzen 7 5800H、63.9 GiB RAM、Windows 11 10.0.26300、RTX 3060 Laptop 驱动 31.0.15.4630、Git 2.54.0.windows.1。窗口沿用 150% DPI。未控制操作系统文件缓存，不是冷启动或首次可交互耗时测试。

修正 `scripts/windows-job-memory.ps1`：旧 LargeDiff 仅单击文件，在默认独立 Tab 模式下不能证明加载 Diff；现先选文件，再定向点击 Show Diff，保存 history／selected／opened／window 截图，并保存 20 次稳定态采样、GPU 计数器和 exe SHA。新增 DiffFileY 参数支持 tip 只有一个文件的双栏样例。首次采用 Ctrl+D 因前台激活失败而安全中止，不能算有效测量；改成向测试 HWND 发送点击消息后两次测量成功。未绕过全局键盘前台检查。

两个场景从挂起创建就纳入 Windows Job Object，应用与继承的 Git 子进程峰值由内核统计；每次 totalProcesses=61、结束采样 activeProcesses=1。稳定态采用 windows-memory.ps1 的 20 次采样，每次间隔至少 500 ms（另加 CIM 查询耗时），期间直接 Git 子进程采样均为 0。Job 峰值是进程组同时占用峰值，不能以 peakAnyProcessBytes 冒充 Git 独立峰值；GPU 单独列出不直接相加以免重复计数。

| 场景 | 应用 Private Bytes 稳定样本 MiB | Working Set MiB | 应用＋Git Job 峰值 MiB | GPU Dedicated / Shared MiB |
| --- | --- | --- | --- | --- |
| 10,000 已加载历史＋975,000 字节新增文件（单侧全文 Tab） | 88.83–90.19 | 85.13–86.47 | 119.60 | 55.42 / 3.45 |
| 10,000 已加载历史＋975,000 字节文件中间仅一行修改（双栏全文 Tab） | 102.11–103.82 | 98.07–99.64 | 120.26 | 55.44 / 5.23 |

第一场景复用 GitReview-benchmark（10,000 提交），第二场景在新专用目录 `full-context-memory-fixture` 中 git clone --no-hardlinks 原基准，将 review.rs 的 `bounded cache review text for entry 06500` 改为同长度 `updated cache review text for entry 06500` 并提交，实际 10,001 提交、显示上限 10,000。numstat 确认只增删一行，文件两版均 975,000 字节。未更改原基准或用户仓库。

证据归档 `artifacts/current-tab-job-memory-2`、`artifacts/full-context-job-memory`。history.png 已人工确认底部 10000 commits loaded；window.png 确认 review.rs 独立 Tab、单侧／双侧全文从第一行显示。双栏样例 fixture.json 保存 tip／规模；不做滚动自动化，截图不宣称末尾视觉覆盖。测试 PID 32256、5532 均已退出并单独复核不存在；用户 Rebased／其他窗口未关闭，也不计入该 Job 的内存。

结论：当前构建这两个场景满足稳定态 ≤150 MiB、峰值 ≤250 MiB 的预算，不需要为通过它们上调预算或削减全文显示。没有修改应用实现，没有重复打包。剩余发布门槛：最新构建空窗口 ≤80 MiB、Git 单独瞬时峰值、复杂分叉／路径过滤、底部模式、多 Tab／100 文件／20 刷新／10 次切仓长期增长，以及冷热启动延迟仍未由本次证据证明；早期结果不自动继承为最新构建整体通过。

### 全文 Diff Tab 重复打开／关闭（2026-10-16）

复核 `artifacts/full-context-tab-cycles` 已完成的 20 轮同进程测量，构建 SHA256 `b34d664a4fdc8945074eaaafd91e5c114df4d1a02dfb513800d04aeef65e124c`。沿用上述双栏全文 fixture（加载 10,000 条历史，两版各 975,000 字节，仅中间一行修改）。`scripts/windows-job-memory.ps1` 新增 `-TabCycles 20`，配合 `-LargeDiff -DiffFileY 193`；每轮打开、关闭后分别截图、等待一秒、采样，并保存 tab-cycles.json。固定坐标适用于当前 150% DPI 基准布局。

20 轮记录均确认每次打开新增 3 个 Git 进程，打开后 activeProcesses=1；这验证重新读取确实发生，不单凭点击判定加载成功。人工检查的 baseline、首轮打开、末轮打开／关闭截图显示全文 Tab 和恢复 Log；其余截图保留但不宣称逐帧人工验收。未自动化滚动。

- 首次关闭基线 Private Bytes：89.46 MiB。
- 20 轮打开后：102.43–104.18 MiB；关闭后：89.78–91.94 MiB。
- 后 10 轮关闭后：90.50–90.84 MiB；最后一次 90.72 MiB，比初始关闭基线高约 1.26 MiB。
- 最后关闭后另外 20 次稳定采样：90.69–90.72 MiB。这些样本属于 Log 状态，不能标成活动 Diff 内存。
- 内核 Job Object 应用＋Git 同时占用峰值：131.71 MiB；总计 121 个进程。GPU Dedicated 55.41 MiB、Shared 5.96 MiB，单列不相加。

结果支持本次 20 轮单文件全文 Tab 没有持续线性增长，且满足当前规模预算；不声称精确回到初始字节数或证明长期无泄漏。生命周期代码已检查：DiffTab::deactivate 取消读取、释放 viewer／picker，关闭 Tab 移除实体；GPUI observer 使用弱引用且实体释放时清理。测试进程已退出，复核时原 PID 34480 已被 PowerShell 重用，因此改按 Win32_Process 的 gitui-diff-tabs.exe 身份确认无残留，未终止任何其他进程，见 process-exit.json。

本轮仅补齐测量记录，没有改应用或重新打包。多文件／多 Tab、20 次刷新、10 次切仓、复杂过滤及冷／热启动等剩余门槛继续保留。

### Islands 语法配色与语言支持（2026-10-16）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`：读取 `platform/diff-impl/src/com/intellij/diff/util/DiffEditorHighlighterUpdater.java`、`DiffUtil.java::initEditorHighlighter/createEditorHighlighter`，确认按文件类型／语言为完整 document 设置 highlighter；读取 `platform/platform-resources/src/themes/islands/IslandSchemeDark.xml` 的 DEFAULT_*、TEXT、行号颜色。采用 Islands Dark 本身的颜色值，而非旧 Darcula 印象配色。

`assets/themes/islands-syntax.json` 将这些前景色映射到现有 Tree-sitter capture；`src/ui.rs` 设置 highlight_theme，保留原有 diff 增删背景状态色。主题资源来源及 Apache-2.0 声明在 SOURCE.txt，打包脚本增加 themes 的来源文件。关键字 #cf8e6d、字符串 #6aab73、数字 #2aacb8、注释 #7a7e85、函数 #56a8f5。共享 capture 无法完全区分调用／声明、静态／实例字段，未模拟语义索引；文档注释斜体、注入语言、行号实际组件用色和其他 Islands Diff 装饰仍需继续对照。

首次仅接入主题后 Windows 构建及启动正常，但原生 Rust 全文截图仍无高亮。继续检查确认 gpui-kit 默认 feature 不包含 tree-sitter，实际使用 wasm_stub 无语法实现；因此 Cargo.toml 显式启用 tree-sitter-languages。此项使以前的无语法解析内存测试不能直接作为当前实现证据。全语言依赖中 SQL grammar 要求 cc ~1.2.1，与原锁定 1.8.0 冲突；cargo update -p cc --precise 1.2.67 后依赖解析成功，未修改 vendor grammar。该次依赖调整后的原生验证、内存复测与打包结果见下文。

最终 Windows release 编译通过。`artifacts/islands-syntax-memory/window.png` 已人工确认双栏全文的 Rust 关键字橙色、常量紫色、字符串绿色，history.png 确认 10000 commits loaded。`artifacts/islands-syntax-rust` 保存第一次未启用解析器的失败视觉证据，不能作为最终验收。测试未做滚动。其他语言已编译接入，但尚无逐语言原生高亮验收；主题映射不代表语义颜色像素级完成。

同一 975,000 字节双栏全文 fixture 的 Job 测量：20 次稳定采样 Private Bytes 112.60–114.69 MiB，Working Set 114.50–116.41 MiB，直接 Git 子进程稳定采样为 0；应用＋Git 内核同时峰值 151.84 MiB，GPU Dedicated 55.44 MiB／Shared 5.23 MiB。相较无解析器的同场景，内存确实上升，但仍低于稳定 150／峰值 250 MiB 预算；只证明此场景，不覆盖高亮后的重复 Tab 生命周期、空窗口或多语言压力。按需构建所用语言的 highlighter，但内置语法资源进入程序；仍需关注首次解析耗时、任务取消及复杂文件峰值。测试 PID 33216、12364 已主动关闭，Job 测试 PID 32520 结束后按可执行文件名复核无残留；未关闭其他程序。

预览包已更新，ZIP CRC、内含 exe 与 release 字节一致、主题来源随包检查通过。SHA256 `4936c8399115fb4b8109ccb4011468de61884be30ffd5adcafc00011800c608a`。本轮没有重复运行无关 Git 核心测试；验证范围为 Windows 编译、原生 Rust Diff、内存采样与打包。

### 启用语法解析后的 20 轮 Tab 生命周期（2026-10-16）

当前构建 `4936c8399115fb4b8109ccb4011468de61884be30ffd5adcafc00011800c608a` 与宿主机测试 exe 哈希一致。运行 windows-job-memory.ps1，参数 `-LargeDiff -DiffFileY 193 -TabCycles 20`；沿用 full-context-memory-fixture，加载 10,000 条历史，双侧各 975,000 字节 Rust 全文。证据在 `artifacts/islands-syntax-cycles`，包括全部开关截图、原始采样、汇总与退出复核。

20 轮每次打开均产生 3 个新 Git 进程，打开后 activeProcesses=1。人工检查第 3、20 轮打开截图均有双栏全文及语法配色，第 20 轮关闭截图恢复 Log、保留原选择及 10000 commits loaded；其余截图存档但不宣称逐帧人工检查。未做滚动自动化。

首次关闭基线 Private Bytes 88.98 MiB；20 轮打开后 106.23–110.66 MiB，关闭后 89.92–90.65 MiB；后 10 轮关闭后 90.14–90.65 MiB，最终 90.33 MiB，较初始基线高约 1.35 MiB。最后关闭后的另 20 次稳定采样为 90.30–90.33 MiB。应用＋Git 内核同时峰值 149.84 MiB，总计 121 个进程；GPU Dedicated 55.41 MiB、Shared 5.77 MiB 单列，不加到 Private Bytes。当前场景符合预算，未观察到连续线性增长；不同测试进程的稳定态受分配器和采样时点影响，不据此声称比上一轮静态测量更省内存。

检查 DiffTab::deactivate、DiffState::ensure_presentation：非活动 viewer 释放，语法树／查询属于后台准备过程，高亮结果属于 viewer；世代检查阻止旧任务回填。但正在执行的同步 Tree-sitter 解析未接入取消标记，本次每轮有等待，不能证明解析中快速切换的峰值受控。此项连同多语言／多文件／多 Tab 压力继续待验收，不能将本次结果扩大为长期无泄漏证明。

测试 Job 正常结束；按 Win32_Process 可执行文件名确认无测试程序残留。未改应用、未重新打包，保持当前预览 SHA。本轮同时清理上一条记录中已过时的“最终验证尚待完成”文案。

### Diff 双栏／单栏选择器（2026-10-16）

对照参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/impl/ui/DiffToolChooser.kt`：分段图标按钮、tooltip 使用工具名、当前工具为选中项。读取 SimpleDiffTool.java 和 UnifiedDiffTool.java：两者对只有一侧内容的请求均优先 SimpleOnesideDiffViewer，不强行显示空栏。参考视觉为用户 diff-tab.png 右侧两个按钮。新增原始 expui/diff/{sideBySide,unified}_dark.svg，来源与 Apache 声明已保留。

本地 `src/diff_tab.rs` 工具栏右侧新增 ButtonGroup，preferred_mode 属于 Tab，在刷新、文件导航及离开后重新激活时保留；调用 DiffState::set_mode 复用已有全文和语法结果，不触发 Git 读取或重解析。该组件重建行映射并保留附近源行／选区，水平滚动归零；本轮不以源码推断代替滚动验收。新增／删除文件仍用已有单侧模式渲染。

Windows release 编译通过。原生 PID 26712 在双侧各 975,000 字节 Rust fixture 上验证双栏→单栏、返回 Log→激活同一 Tab 仍为单栏、再切回双栏，高亮及全文仍在。截图 `artifacts/diff-mode-selector`。本轮未原生验证新增／删除文件的两个按钮、刷新／文件切换后的保留，未进行滚动测试；这些行为仅实现／代码检查，不能称为全部原生验收。测试进程已按 PID 和程序名检查后关闭；未关闭用户窗口。

保留差异：当前偏好仅在单个 Tab 内保存，跨 Tab 共享及重启持久化未实现；分段按钮选中边框太弱、高度及设置按钮间距尚未像素级还原；Unified 版本标题布局仍沿用两侧标题。后续应继续对照参考选择偏好的持久化范围及标题实现。没有扩大本轮内存采样为性能验收结论。

预览 ZIP 更新并通过 CRC／exe 字节一致检查，SHA256 `a3d9bc871d463b23d876bfa609cdd9f5557fc589ec9ad609ef48cd5ef3f3e145`。

### Diff 分段按钮边框与高度（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/platform-impl/src/com/intellij/ui/dsl/builder/components/SegmentedButtonComponent.kt`：选中项另画边框，preferred height 为内容高度加间距与 Button.minimumSize.height−2 中较大者。读取 `ManyIslandsDark.theme.json`：Button.minimumSize 高 28，SegmentedButton.selectedStartBorderColor／selectedEndBorderColor 指向 control-border-raised（gray-70 #5F6269），普通 control-border 为 gray-50 #40434A，selectedButtonColor 为 gray-30 #26282C。

本地 `src/diff_tab.rs` 两个模式按钮显式 26 逻辑像素高、完整 1 px 边框，根据 preferred_mode 选择对应边框色，背景 #26282C。修正先前组件默认选中样式几乎不可辨的问题；现仍使用 ButtonGroup，选中内侧角不是 Swing 独立绘制的完整圆角，焦点填色／hover 也尚未逐状态还原。

Windows release 构建通过，原生 PID 24760 的 split.png／unified.png 已人工确认左／右选中边框变亮、内容切换正确、语法高亮保留。截图在 `artifacts/diff-mode-border`，测试程序已检查程序名后关闭，未操作其他窗口。未做滚动或重复内存压力测试，当前改动只涉及控件样式。

预览包更新并通过 ZIP CRC 与 exe 字节一致检查，SHA256 `6048e3586c91ad46f0401fee3c54b4d34757714234e5f27b384eb42908bb7d86`。跨 Tab／重启偏好、Unified 标题及其余 Diff 细节仍待继续完成。

### 单栏标题修正与场景偏好保存（2026-10-16）

用户指出单栏仍在半屏右侧显示 hash／文件名。对照 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 UnifiedDiffViewer.java::createTitles 和 DiffUtil.java::createStackedTitleComponents：前后两个非空标题采用 VerticalLayout.FILL，上下排列；空侧被排除。本地 `src/diff_tab.rs` 根据模式将标题改为纵向满宽／横向均分。普通修改在单栏下仍有两个版本标题，但均位于左侧，不再保留双栏的右侧位置；新增／删除文件仍只有有内容的一侧。锁图标、标题字号、间距尚未完整像素验收。

同时完成此前被用户反馈打断的模式偏好实现。读取 DiffSettingsHolder.kt：PlaceSettings.DIFF_TOOLS_ORDER 按 place 持久化，VCS_LOG_VIEW 默认 Simple→Unified，CHANGES_VIEW 默认 Unified；DiffRequestProcessor.java::getToolOrderFromSettings/updateToolOrderSettings 读取／保存顺序。本地 layout_preferences.rs 用 diff-view-log 和 diff-view-changes 两个小文件存用户偏好，Log 默认双栏、Changes 默认单栏，缺失／损坏／超长内容回退对应默认。Tab 激活或文件导航时读取本场景偏好；点击保存失败显示独立错误，不隐藏当前 Diff。当前只映射已有两种文本工具，不实现多工具排序。

Linux `cargo test --no-default-features diff_preference_tests` 通过，覆盖不同场景默认、无文件、非法／超长内容及 split／unified 值。Windows release 编译通过。原生 PID 31760 确认单栏标题上下靠左；退出后 PID 33824 新建窗口打开同一文件，自动恢复单栏及新标题布局。截图 `artifacts/unified-titles` 和 `artifacts/unified-titles-restart`，均已查看最终单栏状态。两个测试窗口已按 PID／程序名确认后关闭；测试前 diff-view-log 不存在，结束后仅删除本次写入且内容仍为 unified 的测试偏好，恢复原状态。未操作其他用户窗口，未做滚动测试。

本轮重启验证覆盖 Log 偏好；Changes 场景隔离只有测试／代码证据，跨 Tab 原生互相更新、保存失败提示和新增／删除标题仍待原生补验。没有宣称完整 Diff 视觉或性能验收。预览 ZIP 更新且 CRC／exe 字节一致验证通过，SHA256 `286a70d6fd8a1d256df7016d45f82687b61fa4673fd06b6e202a4b0c54ac6856`。

### 单侧标题及场景隔离原生补验（2026-10-16）

使用当前预览构建 `286a70d6fd8a1d256df7016d45f82687b61fa4673fd06b6e202a4b0c54ac6856`，新建独立 diff-title-variants-fixture：base 有 a_deleted.rs／keep.rs，下一提交删除 a_deleted.rs、新增 z_added.rs；工作区只修改 keep.rs。fixture 脚本及截图保存在 `artifacts/diff-title-variants`。所有仓库变更仅用于该新测试目录。

原生 PID 25344：删除文件单栏只显示 base revision＋a_deleted.rs，新增文件通过 Next file 切入，单栏与双栏工具选择下均只显示新 revision＋z_added.rs，无空侧标题或旧文件名；新增、删除代码均完整显示一行并有高亮。已人工查看 deleted-unified、added-unified、added-split 截图。Local Changes 首次打开 keep.rs 为单栏，Index／Working tree 标题上下靠左；改为双栏后，再将历史 Tab 改为单栏并返回本地 Tab，仍为双栏（local-isolated.png）。核对磁盘 diff-view-log=unified、diff-view-changes=split，确认场景偏好隔离和非活动 Tab 重新激活取值。

本轮没有新增运行时代码或重新打包。仅关闭本轮程序；两项偏好在测试前均不存在，结束后校验仍为本轮写入值再删除，恢复原设置。未操作用户窗口、未自动滚动。该小 fixture 验证功能／标题，不作为内存压力或全部语法颜色一致性的证据；同场景两个不同 Tab 共享、快速切换及保存失败提示仍需补验。

### Rust 数字及转义捕获修正（2026-10-16）

小样例发现数字 1／2／7 被渲染为常量紫色。源码核对 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 plugins/textmate/lib/bundles/rust/syntaxes/rust.tmLanguage.json：数字 constant.numeric.*，布尔 constant.language.bool.rust，转义 constant.character.escape.rust；TextMateDefaultColorsProvider.java 将 numeric 映射 NUMBER，escape 映射 VALID_STRING_ESCAPE，其他 constant 回退 CONSTANT。IslandSchemeDark.xml 中 NUMBER=#2aacb8、VALID_STRING_ESCAPE=#cf8e6d。布尔紫色符合这条参考路径，不能仅凭其他 IDE 印象改成关键字色；这不代表已安装版本的所有 Rust 插件语义方案。

vendor/gpui-component 的 Rust highlights.scm 原先 integer／float／boolean 全用 constant.builtin，escape 使用没有主题槽位的 escape。现将整数／浮点 capture 改为 number、转义改为 string.escape，布尔保持常量。改动及来源记入 vendor/gpui-component/PATCH.md，没有修改主题常量色或其他语言规则。

Windows release 编译通过；独立 syntax-token-fixture 原生 PID 4744、tokens.png 人工确认整数 7／浮点 1.25 青色、布尔 true 紫色、字符串正文绿色且反斜杠转义橙色，常量名称保持紫色。四行全部可见，证据与 fixture 创建脚本在 artifacts/syntax-tokens。只关闭本轮测试进程，未改用户仓库／偏好，未自动滚动。这是具体 Rust token 映射验证，其他语言、泛型／语义类型及嵌套注入仍未完整验收。

预览包更新并通过 ZIP CRC／exe 字节一致校验，SHA256 `a774ea84579182c7febc40e27edd23ba5a8d8763138ed615b6122095bb5571a4`。


### 去掉 Diff 行前加减号（2026-10-16）

用户指出 Rebased 默认以颜色区分增删，没有每行 +／−。对照参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/tools/fragmented/UnifiedDiffChangeUi.java` 与 `platform/diff-impl/src/com/intellij/diff/util/DiffDrawUtil.java::createUnifiedChunkHighlighters`：增删区间和行内差异使用高亮，gutter 操作与可编辑状态有关，不是逐行文本加减前缀。用户提供的 diff-tab.png 亦无此前缀。

本地组件默认 DiffChangeIndicator::Signs；在 src/diff_tab.rs 独立 Tab 与 src/ui.rs 底部预览显式设置 None，保留行号、背景和行内高亮，不改变 Git 数据或源代码中的真实运算符。组件 None 仍保留少量代码前间距；完整 gutter 尺寸尚未像素级验收。

Windows release 编译通过，原生测试 PID 33716 的删除／新增文件均确认无加减号且红／绿背景与语法高亮保留，截图 artifacts/diff-no-signs/{deleted,added}.png。底部预览本轮为配置与编译验证，没有另行原生截图；未做滚动或内存重测。首次截图路径误用发行版名导致保存失败，改用实际 legion-wsl 路径后成功，复用同一测试进程。仅关闭本轮测试进程，不操作用户窗口及偏好。

预览 ZIP 已更新，CRC 与内含 exe 字节一致检查通过，SHA256 `0273dde9f52afb5e9a3c4441a71db3ba0925636106ef2ede7dcd51960dc9ab17`。上一轮正在调查的锁图标及路径标题细节尚未实施，本轮不混入该改动。


### Diff 只读标题与重复路径（2026-10-16）

对照 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 DiffTitleWithDetailsCustomizers.kt：before 路径始终显示，after 仅在 before 不存在或路径不同时显示；FilePathDiffTitleCustomizer.kt：版本与路径分开，间隔 8，路径使用 UIUtil.getContextHelpForeground。JBUI.java 将该颜色映射 Label.infoForeground，ManyIslandsDark.theme.json 的通用 infoForeground→text-secondary→gray-80=#73767C。DiffUtil.java::createTitle 为只读侧添加 AllIcons.Ide.Readonly；现代映射使用 expui/general/locked_dark.svg。

src/diff_tab.rs 已使用独立 revision／path 元素、8 逻辑像素间隔和对应灰色路径；同路径不在 after 重复，新增／重命名保留 after 路径。两侧均只读，标题显示锁图标，资源原样引入并在 assets/icons/SOURCE.txt 保留来源。单栏继续纵排，双栏继续并排，新增／删除排除空侧。未改变 Diff 内容、读取方式或缓存生命周期。

Windows release 构建通过。原生 diff-title-variants-fixture 的 Local Changes keep.rs 验证 Index＋路径／Working tree 两行靠左，锁可见、路径仅一次，增删行没有加减前缀。初次截图 artifacts/diff-title-details/unified.png；最终路径颜色修正后再次启动并查看 artifacts/diff-title-details-final/unified.png。测试 PID 34276、30460 均按身份检查后关闭；未修改用户偏好，未自动滚动。新增／重命名标题此次未追加原生验收。路径目前尾部省略，尚未实现参考保留文件名的中间路径缩略、复制及完整路径 tooltip；字体／标题行高也未宣称像素级完成。

最终预览 ZIP CRC 与 exe 一致校验通过，SHA256 `ce232015898fb997505bd51efdc009c2b6b415d0734f89710a287d4084ed8f5a`。本轮为标题呈现改动，无新增内存验收结论。


### Diff 长路径中间缩略（2026-10-16）

读取同一参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 FilePathDiffTitleCustomizer.kt、platform/util/ui/src/com/intellij/util/ui/{FilePathSplittingPolicy,SplitBySeparatorPolicy}.java：按分隔符从尾部文件名、头部路径交替保留组件，中间使用 ...；用字体实际宽度选择可容纳结果，Diff 覆盖规则始终保留文件名，完整路径用 tooltip 显示。

新增 src/diff_path_title.rs，通过 GPUI canvas 测量和绘制标题路径，候选以完整路径组件为边界，不切割 UTF-8 字节；宽度足够显示完整路径，否则逐候选选择。源码已注明参考及 Apache-2.0 版权。src/diff_tab.rs 接入并用 repository＋相对路径提供完整路径提示。未增加后台进程或保存文件内容，标题候选只在当前渲染中存在。不同于 Swing 的逐字符长度循环，本地只测量不同组件候选；完整路径可容纳时直接返回。只处理 Windows 目标中的两种目录分隔符；复制／选择标题文本仍未实现。

Windows release 构建通过。新 title-path-fixture 的两提交修改同一深层目录文件；原生 PID 31912 的 artifacts/diff-long-title/split.png 确认 application/features/reviewing/components/.../comparison/rendering/titles/important_file_name.rs，文件名完整，右侧不重复路径。fixture 脚本已存档。Tooltip 已配置但悬停截图遇用户 Rebased 在前台遮挡（hover-obscured.png），不能作为本程序 tooltip 的验证，不继续干扰用户前台。中文路径、窄窗以及 Unified 模式缩略未在本轮原生补验。仅关闭本轮测试程序，未改用户偏好，未做滚动自动化或内存验收。

预览 ZIP 已更新并通过 CRC／exe 字节一致检查，SHA256 `21aa3b8cebef5f812b818da46b93a106b01957359b94d2d7337903736eff2cbb`。构建后仅新增源码版权注释，不改变运行时代码。


### 长路径窄窗与 Tooltip 原生验收（2026-10-16）

上一轮属于进展：新增路径缩略且有原生截图，但 Tooltip 仍未验证。本轮基于同一 title-path-fixture，将自有测试窗口调整为 1100×800 物理像素（Windows 150% DPI），以定向窗口消息悬停，不向其他窗口发送按键或点击。测试辅助脚本保存在 artifacts/diff-title-narrow/capture-title-narrow.ps1。

PID 30948 的 split.png 确认 application/...titles/important_file_name.rs 保留完整文件名，unified.png 确认扩大可用宽度后保留更多目录、两个版本上下靠左。同目录 tooltip.png 实际暴露问题：完整路径提示单行超出右边界。根据 FilePathDiffTitleCustomizer.kt 的 isAllowAutoWrapping=true，修正 src/diff_path_title.rs，使用 Tooltip::element 和正常换行，宽度限制为视口减 48 与 600 逻辑像素中的较小值。这是 GPUI 的边界适配，未宣称复刻 Swing Tooltip 的精确宽度。

重新构建后 PID 34812 的 artifacts/diff-title-tooltip-wrap/tooltip.png 已人工确认完整路径分两行显示，文件名在第二行末尾完整出现，未越过窗口右边界。构建通过，路径缩略与 Tooltip 现在均有具体原生证据。中文／超长单个文件名、文本选择复制和所有 DPI 仍未覆盖。此前临时创建的 Log unified 偏好已校验内容后删除，恢复测试前缺失状态；两个自有测试进程均检查身份后关闭，未关闭用户 Rebased，未做滚动测试。

补充 docs/THIRD-PARTY.txt 并让 package-windows.py 将路径算法改编声明随包分发，Apache 许可证沿用包内 icons/LICENSE.txt。预览 ZIP CRC、exe 字节一致以及声明／许可证包含检查通过。SHA256 `4379780844649915f2908b5ba8e88a374b978adb96187393be19387b631d5d5d`。没有重复运行无关 Git 测试或扩大本轮为内存验收。

### Diff 后台语法解析取消（2026-10-16）

本轮从低内存诉求检查后台生命周期，发现原有 generation 检查仅阻止过期结果回填，Task drop 不能打断已进入同步 Tree-sitter 的解析。读取 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 platform/diff-impl/src/com/intellij/diff/util/DiffTaskQueue.java：execute 前 abort，abort 取消 ProgressIndicator；tools/util/base/DiffViewerBase.java::dispose 调用 abortRediff。提取规则是释放／替换 Diff 时通知后台停止，不只是忽略结果。

vendor/gpui-component 的 DiffState 增加每轮 Arc<AtomicBool>：reset_presentation 取消旧轮并新建标记，Drop 取消当前轮。FilePresentation 将标记传到高亮器，文件／两侧／hunk／capture 映射／行内比较的行对边界检查取消；取消返回 None，不发布部分结果。capture_names_cancellable 使用 Tree-sitter ParseOptions progress callback，fresh parse 前 reset，取消后清理树及 resumable parser 状态。正常输入保留之前的无注入 Diff 高亮语义。WASM／无解析器 stub 同步调整签名，修改记录在 vendor/gpui-component/PATCH.md。

Windows 组件测试六个不同用例通过：预先取消不创建高亮器、取消后解析器复用与 fresh 结果一致，以及原有语言检测／每 hunk 语法／行长限制／行内比较回归；两个 filtered run 的重叠用例不重复计数。证据 artifacts/diff-parse-cancellation/tests.txt。首次直接测试本地依赖被 Cargo 拒绝；临时加入 workspace 后暴露发布包遗漏测试资源，随后使用 /tmp/gitui-gpui-kit 的精确包源 commit 67265a36e260ffe03c5aaebdcd8d654dbf892eab 提供真实资源链接，成功构建并在 Windows 运行。临时 workspace 与链接均移除，正常应用构建恢复并裁去测试专用 lock 依赖。未改写或伪造上游测试资源。

Windows release 构建通过。原生 full-context-memory-fixture，10,000 条历史，双侧各 975,000 字节 Rust 全文；人工查看 history.png 与 opened.png，历史和两侧语法高亮正常。artifacts/diff-cancel-memory 的 20 次采样 Private Bytes 109.03–111.18 MiB、Working Set 112.94–114.89 MiB，采样中的 Git 子进程内存为 0；应用＋Git Job 内核同时峰值 148.54 MiB。GPU Dedicated 55.39 MiB／Shared 4.89 MiB 另列。符合该场景稳定 150／峰值 250 MiB 预算，不据跨进程差值宣称确定降低多少内存。Job 测试 PID 10004 已结束，Win32_Process 按可执行名复核无测试窗口残留，未关闭用户程序。

限制：这些测试证明取消入口与正常呈现，但尚未确定性覆盖解析途中取消、所有快速切换时序或退出延迟；Query 编译、一次 query cursor step、单行 bounded inline diff 不抢占。仍需对解析中取消和反复切换峰值补验，不把当前测量扩大为无泄漏／所有取消路径完成。预览 ZIP 已更新并通过 CRC／exe 一致检查，SHA256 `b1cedd4f84af1a8062dbfb854386d630caf394207ad2fad88ccd80eb0ce3151b`。


### 解析途中取消及释放通知的确定性测试（2026-10-16）

上一轮实现并验证了预先取消，但未证明解析途中退出。本轮沿用已读 Rebased DiffTaskQueue／DiffViewerBase 的取消生命周期规则，为 capture_names_cancellable 抽出私有泛型 progress observer：正常调用传空闭包，测试在真实 Tree-sitter 第三次 ParseState 回调中设置相同 AtomicBool。测试使用 50,000 行 Rust 输入，断言确实到达三次回调、返回零 capture、tree=None、保留 text 长度为零；接着解析含中文的另一份文件，与全新高亮器的 capture 完全比较，排除取消后 resumable 状态串文档。

另加 gpui::test 验证 DiffState reset_presentation 取消旧 token 而新 token 未取消，Entity 释放并执行清理后新 token 也已取消。Windows 原生组件测试过滤 cancelled，共四项通过；原始 stdout／stderr 和测试 exe SHA 已存 artifacts/diff-parse-cancellation/inflight-tests.txt。单次调试构建记录第三次回调退出约 238.7 µs，这是特定测试的观察值，不是跨线程取消延迟保证或性能预算。测试构建使用精确 upstream 67265a36e260ffe03c5aaebdcd8d654dbf892eab 资源；临时 workspace、Cargo.lock 与资源链接通过 finally 恢复，已核对无残留。

Windows release 编译通过，运行时取消逻辑未增加新的功能分支，仅将回调提取以确定性测试；不重复前轮 UI 截图或内存采样。原有一万提交／大文件测量对应上一构建，不能冒充本构建新测量。Query 编译／单次 cursor step、实际快速点击触发的跨线程调度与多任务峰值仍待验证。

预览 ZIP 已更新，CRC 与 exe 一致校验通过，宿主机测试副本同步更新，SHA256 `12d982923c18b0659495259f05a42bfc058195634b2d8c0fbef15c9db3d52157`。本轮未启动额外图形窗口或操作用户 Rebased。


### 原生快速切换 Tab 的 Job 峰值（2026-10-16）

当前发布构建与宿主机副本 SHA 均为 `12d982923c18b0659495259f05a42bfc058195634b2d8c0fbef15c9db3d52157`。本轮未改运行时代码；扩展 scripts/windows-job-memory.ps1 的 RapidTabCycles 参数，在既有 suspended launch→Job attach 的采集方式下，定向 PostMessage 点击 Diff／Log Tab。参数与 CombinedStress／普通 TabCycles 互斥，坐标边界检查，Job finally 关闭仅本测试进程组。点击不使用全局按键，不自动滚动。

full-context-memory-fixture，10,000 条历史，双侧各 975,000 字节 Rust 全文；150% DPI 默认窗口。15／50／150 ms 请求间隔各 30 轮（总共 90 轮），实际发送阶段耗时分别 1832／3768／9375 ms，不能把请求 sleep 参数当精确事件间隔。三组分别产生 88／90／90 个新 Git 进程，组后 activeProcesses=1。证明产生了实际读取，但不能据此断言每条消息均进入语法解析；前轮确定性 progress callback 测试另外证明解析内部取消。

各组回到 Log 后单点 Private Bytes 为 99.03／99.24／101.44 MiB；再次打开 Diff 并等待后的单点为 115.50／115.46／117.65 MiB。全程应用＋Git 内核同时峰值 178.75 MiB（单进程峰值另记，不能相加），低于 250 MiB 预算。最后关闭 Diff 后 20 次稳定采样 99.20–99.32 MiB，Working Set 101.51–101.58 MiB，采样 Git 子进程内存为 0；GPU Dedicated 55.41 MiB／Shared 5.86 MiB 单列。本轮稳定采样是关闭后的 Log，不是打开状态；后三组差异可能受分配器／缓存及解析实际开始阶段影响，不宣称无泄漏或内存完全回到进程初始值。

证据 artifacts/diff-rapid-tabs-memory：原始 rapid-tabs.json／steady.json／memory.json 与截图。人工查看 rapid-15-diff、rapid-150-diff，双栏 review.rs 与高亮正确；rapid-final-closed 确认 Diff Tab 已关闭、Log 保留选择且仍有 10000 commits loaded。其余截图存档，不扩大为全部逐帧验收。PID 25060 已由 Job 流程结束，按 Win32_Process 可执行名复核无测试程序残留；未关闭用户 Rebased／IDEA。

当前场景补足快速切换压力证据；不同语言、多文件／多 Tab、query 编译阶段取消、所有 DPI 和长期运行仍未覆盖。没有重打包，保持当前预览构建。

### Diff 设置齿轮与显示选项（2026-10-16）

读取 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 platform/diff-impl/src/com/intellij/diff/actions/impl/SetEditorSettingsActionGroup.kt：GearPlain 设置入口、Appearance 中行号及 Soft-Wrap toggle；TextDiffSettingsHolder.kt 的 PlaceSettings：默认 SHOW_LINE_NUMBERS=true、USE_SOFT_WRAPS=false、HIGHLIGHT_POLICY=BY_WORD，按 place 持久化。tools/util/text/TextDiffProviderBase.java 将 Highlighting Differences 分组加入菜单，HighlightPolicy.java 区分 Lines／Words／Characters 等；DiffBundle.properties 和 ActionsBundle.properties 核对显示文案。PlatformIconMappings.json 将 GearPlain 映射到 expui/general/settings.svg，新增原始 settings_dark.svg 并记录来源。

本地 src/diff_tab.rs 模式选择器右侧增加齿轮菜单，支持 Lines／Words／Characters、Show Line Numbers 和 Soft-Wrap，使用真实 DiffState::set_inline_unit 与 Diff 渲染参数。Line 模式只去掉行内强调，仍保留增删背景和语法颜色；Words／Characters 对应现有单词／字素比较，底层算法与 JetBrains InnerFragmentsPolicy 并非逐字符结果保证。改高亮粒度会取消并重建 presentation，不重新读取 Git；仅切行号／换行不会改变 inline unit。保存失败保留当前设置并显示错误。

layout_preferences.rs 增加小配置 diff-editor-log／diff-editor-changes，每次读取最多 129 字节，超过 128 字节或读取失败回退默认，各非法字段保留对应默认。激活 Tab／切文件时重读本场景设置。Linux appearance_tests 通过，覆盖 12 种组合往返、缺失、非法与超长内容。Windows release 构建通过。

原生 diff-settings-fixture 含一行长注释及 1234→1254：PID 30196 的 menu.png 确认齿轮与菜单；wrapped.png 确认长行按各自栏宽换行、字符高亮只强调 3／5；lines.png 确认行号隐藏、取消局部高亮但保留整行背景。重启 PID 24616 的 artifacts/diff-settings-restart/restored.png 恢复无行号、自动换行和 Lines。所有证据与 fixture 脚本在 artifacts/diff-settings-menu。测试前两项新配置均不存在，结束后校验本轮 Log 配置内容再删除；仅关闭两个自有进程，保留用户窗口，未测试滚动。

保留差异与后续：底部预览尚未共用这组设置；跨场景隔离、同场景多 Tab、保存失败和大文件自动换行峰值尚未原生覆盖。菜单暂不包含尚未实现的 whitespace／indent guides／忽略空白／Split changes／None 等选项，也未宣称完整菜单像素级验收。**新原生样例暴露已有未换行双栏布局问题：长行使整体内容宽度扩展，右侧代码被推到屏幕外，而两个标题仍均分；需修复每栏视口约束，不能用自动换行默认开启来掩盖。** 本轮默认仍按参考保持 Soft-Wrap=false。

预览 ZIP 更新并通过 CRC／exe 一致校验，SHA256 `0427dd70830ebd400bb9ad0d74ebe6671907a7f5a3b92fcc7ba295db4ca80511`。本轮未重复无关内存测试，前轮数字不能算新增 wrap 场景验收。

### 双栏长行布局：原生失败回归与原因定位（2026-10-16）

新增 scripts/windows-check-split-columns.ps1，针对 diff-settings-fixture 两行样例（长的不变注释＋短的修改行），在已打开 Diff 的专用 gitui-diff-tabs 进程上截图并检查同一行左半区红色／右半区绿色背景及右半区红色泄漏。不是通用主题或任意布局验收器；调用前须确认 fixture、Split、默认深色、Soft-Wrap=false。不发送滚动输入。

当前预览 exe 在 Windows PID 14832 上实际打开 Diff 后，两次执行均退出 1：2182×1361、row=236、leftRed=944、rightGreen=0、rightRed=982、passed=false。截图 artifacts/diff-split-width-repro/{red,red-repeat}.png。失败准确反映左栏横跨中线、右栏不可见；已有首帧后的重绘仍复现。

参考源码 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，本轮读取 platform/diff-impl/src/com/intellij/diff/tools/util/side/TwosideContentPanel.java、tools/util/DiffSplitter.java，以及 tools/util/SyncScrollSupport.java 的 syncHorizontalScroll／doScrollHorizontally：左右各为独立内容面板和编辑器，通过 splitter 分配视口，横向同步传递的是编辑器可视区域 x 偏移，不是把两侧拼成一个宽画布。

本地 vendor/gpui-component/src/diff/mod.rs::render_diff 将 content_width=max(最长行测量宽度, viewport.width)，再令 column_width=content_width/2，并把虚拟列表设为 content_width。render_cell 使用该宽度；外层 diff-horizontal 又整体滚动列表。这直接解释当前失败。下一步须分离栏视口宽度和代码内容宽度，保留长行横向访问；同步检查 selection.rs::SelectionLayer 的单个 horizontal offset、文本 run bounds 和每栏裁剪，不能仅改 w() 或开启默认换行。尚未修改运行时代码、未修复此问题；本轮不更新包、不重做内存验收。

### 双栏长行布局修复与选择回归（2026-10-16）

沿用上一节已读取的 Rebased commit／三个类的规则。vendor/gpui-component/src/diff/mod.rs 将双栏可视宽度改为 viewport.width/2，长行内容宽度单独保留，每栏独立裁剪；新的 horizontal.rs 实现共享横向偏移，在 prepaint 时移动内容，保持虚拟列表缓存行也能读取当前偏移。两栏各有横向滚动条，纵向仍使用同一个虚拟列表，不复制文件或 presentation。全为单侧文件时按单栏布局处理（也覆盖 retained mode=Split 的底部预览）；混合文件列表的单侧行仍传递横向偏移。默认 Soft-Wrap=false 不变。

selection.rs 保存完整文本 run bounds 用于源位置映射和宽度测量，另保存与实际 content mask 相交的 visible_bounds 用于选择起点的栏位判定及命中区域。否则左侧长文本虽不再画入右栏，其完整几何仍可能抢走右栏选中。新增 GPUI 实际渲染测试 split_long_lines_keep_separate_viewports_and_selection_sides：800×400 窗口、左右长行，断言文本实际溢出但可见范围各在正确半区，再用鼠标按下／移动／抬起拖选右侧单词，selected_text 必须为 right。不是只检查设置值。

Windows release 编译通过；当前构建的 50 项 Diff 组件测试全部通过，日志 artifacts/diff-split-width-final/component-tests.txt。测试构建临时 workspace／源码链接和 Cargo.lock 均已恢复。原生 PID 28908 在 artifacts/diff-split-width-fixed 中确认不换行双栏、Unified 和 Soft-Wrap；最终构建 PID 31316 再跑原来的 windows-check-split-columns.ps1，退出 0：2182×1361、row=236、leftRed=943、rightGreen=944、rightRed=0。截图 artifacts/diff-split-width-final/green.png。两个自有窗口已按身份校验后关闭；本轮临时产生的 diff-view-log／diff-editor-log 在核对内容后删除，恢复测试前不存在的状态，不更改其他偏好。

未做滚动自动化或新内存验收。横向手感仍由用户手测；当前同步使用同一最大内容宽度，尚未实现 JetBrains 每侧按自身内容宽度夹取偏移、可拖动中间分隔条／连接差异块等行为，不宣称完整双编辑器像素级还原。只读完整文件、默认不换行、增删色块和无额外加减前缀保留。

预览 ZIP 更新并通过 CRC、包内 exe／release／Windows 宿主测试 exe 字节一致校验；SHA256 `238e3e8ba7bb33c9ecf535ffa53681573000c699bf36d7d458bffe4100182150`。

### 双栏修复后当前构建内存复核（2026-10-16）

测量构建 SHA256 `238e3e8ba7bb33c9ecf535ffa53681573000c699bf36d7d458bffe4100182150`，与当前 release、Windows 宿主测试 exe 和便携包内容一致。本轮未修改应用运行时代码，不重复打包。沿用现有 Windows 150% DPI 基准环境；未控制文件系统缓存，不作为冷启动计时或性能提升百分比证据。

full-context-memory-fixture 使用宿主 Git 重新核实：仓库 10,001 个提交，应用加载上限 10,000；HEAD 与 HEAD~1 的 review.rs 均为 975,000 字节。实际查看 artifacts/diff-split-width-memory/history.png，底部显示 10000 commits loaded；window.png 确认独立 Tab 双栏全文、语法高亮及默认不换行。脚本为 windows-job-memory.ps1 -LargeDiff -DiffFileY 193 -HistoryLimit 10000。

| 场景 | 稳定 Private Bytes（20 次采样） | 稳定 Working Set | 应用＋子进程 Job 峰值 | GPU Dedicated／Shared（另列） |
| --- | --- | --- | --- | --- |
| 10,000 条历史＋双侧 975,000 字节 Rust Diff | 105.88–108.37 MiB | 103.35–105.84 MiB | 144.30 MiB | 55.44／5.23 MiB |
| 未打开仓库的欢迎窗口 | 53.16–53.17 MiB | 55.09–55.10 MiB | 55.68 MiB | 17.37／1.22 MiB |

两个场景的稳定采样均未发现直接 Git 子进程；Job 从挂起启动时即包含继承子进程，Diff 场景全程 61 个进程、最后 active=1，欢迎场景全程只有应用一个进程。Job 峰值为内核同时峰值，不是把各进程各自峰值相加；没有另测 Git 单独的瞬时峰值。GPU 单列、不与 Private Bytes 简单相加。采样原始记录见 artifacts/diff-split-width-memory/{memory,steady}.json 和 artifacts/split-build-welcome-memory-final/{memory,steady}.json。

为复用相同峰值口径，windows-job-memory.ps1 新增显式 -Welcome，禁止同时指定仓库或仓库交互模式，不传仓库路径。首次空窗口测量触发旧脚本“必须至少启动两个进程”的断言（artifacts/split-build-welcome-memory，不作为完整通过证据）；改为仓库要求捕获子进程、欢迎窗口要求全程恰好一个进程后重跑通过。原生 window.png 确认是未打开仓库的欢迎界面；report.historyLimit 在此场景只是未使用的启动参数，不代表加载了历史。失败和成功路径均经 finally 释放各自 Job；末尾按测试进程名查询无残留，未关闭用户程序。

结论：当前构建这两个明确场景满足空窗口 80 MiB、完整 Diff 稳定 150 MiB／峰值 250 MiB 预算，无须缩减全文展示或提高预算。本轮不证明复杂图、多语言、超长单行、自动换行、大量 Tab、持续刷新／切仓或长期无泄漏；这些仍按完整计划验收。未进行滚动自动化。

### Log 提交详情的字体与信息顺序（2026-10-16）

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/details/commit/CommitDetailsPanel.kt（messagePanel 在前、metadataPanel 在后；SIDE_BORDER=14，消息上下 14／10，元数据上下各 10）、ui/frame/CommitPresentationUtil.java（短 hash＋作者邮箱＋on date at time、不同提交者及时间补充、连续空格和 tab 转换）、platform/vcs-impl/src/com/intellij/openapi/vcs/ui/FontUtil.java（提交消息为 editor font、元数据为 label font），以及 VcsLogBundle.properties 文案。ManyIslandsDark.theme.json 的 text-link=blue-100=#71A1FE。与用户 single-search.png 的消息／元数据顺序和等宽标题相符。

本地 src/ui.rs 去掉额外的 Commit 标题栏，将提交消息和元数据分开渲染；消息采用 JetBrains Mono 13 px，元数据 Inter 13 px，左右内边距 14 px、消息／元数据间距 20 px。src/commit_presentation.rs 从已有 Details／Commit 生成两段内容，不新增 Git 查询。时间由 RFC3339 转系统本地时间，显示 YYYY/MM/DD at HH:MM；比较 author／committer 时间时按真实瞬间判断，避免仅因时区表示不同重复显示。不同提交者仍可见，邮箱链接按引用主题使用蓝色。

完整提交哈希复制保留 Ctrl+Shift+C，并在详情区增加右键 Copy Commit Hash；右键菜单 Parents 列出全部已知父提交完整哈希，点击可复制，根提交／浅边界有对应说明。父提交信息从常驻正文移至菜单，是为保留原有只读信息所做的本地安排，不宣称此菜单复制了 Rebased 的菜单结构。浅边界说明也保留在元数据正文。

Linux 两项针对性测试通过：消息 HTML 转义、空行／连续空格、相同时刻不同 offset 不重复提交时间、不同提交者和非法日期回退、邮箱属性及 URI 编码。Windows release 构建通过。专用 commit-details-fixture 含相同作者／提交者的根提交，以及带空行、四空格缩进、&／<code> 文本和不同提交者的第二条提交。初次原生发现 HTML 的 br 未换行，已改为独立 p 段落加空行占位；旧 artifacts/commit-details-layout/window.png 为失败中间证据。最终 artifacts/commit-details-layout-final/{window,root,parents-menu}.png 已查看，确认正文行分隔、等宽字体、蓝色邮箱、日期格式、不同提交者、切换提交后元数据更新及父提交完整哈希菜单。未执行邮件链接、未改动系统剪贴板，复制回调已接线但此次未做原生剪贴板端到端断言。

两次自有 PID 33464／24896 均检查身份后关闭，未改动用户仓库或偏好，未自动滚动。保留差异：尚无 containing branches／Show all 区、提交说明中的 issue／hash 导航，提交者补充文字尚未灰化；身份比较沿用完整字符串而非 JetBrains isSamePerson，日期格式尚不跟随 OS 语言区域。正文分段后的超长消息复制／多父 merge 菜单及多 DPI 仍待验证。本轮不宣称像素级完成或继承上一构建内存验收。

便携预览已更新并校验 CRC 与包内／release／宿主 exe 一致，SHA256 `4eed94ee053d51c2af0db0a87cdaa01a48f126a96fa56a228daec197f306f8ba`。

### 提交直接引用与叠放图标（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/details/commit/ReferencesPanel.java`、同目录 CommitDetailsPanel.kt，以及 `ui/render/LabelIcon.java`、TagPainter.kt；registry.properties 的 branches/tags 上限均为 100。右侧 refs 是直接指向提交的引用，按 branch/tag 分面板、按类型分组，组首显示图标，多引用同色双图标，其余名称用逗号分隔、流式换行；这与 containing branches 独立。New UI 图标从后向前绘制，先填背景再描边，之间插入背景轮廓留隙，只有最前面的图标有圆点。

`src/ui.rs::reference_icon` 翻译上述曲线与 5.12 px 偏移，复用于 Log 与右侧详情；背景随当前行选中／HEAD 可达／普通状态变化。详情复用 Commit.refs，无新增 Git 请求；每面板最多 100 个引用、超出显示数量，名称可选择复制、HTML 转义。引用名称仍沿用快照顺序，未完成 JetBrains natural comparator 的严格对齐。绘制来源已加入 docs/THIRD-PARTY.txt。

Windows release 编译及原生验证通过。`artifacts/commit-reference-labels/window.png` 显示专用 fixture 的 HEAD、三个本地分支、一个远程分支和两个 tag；unselected.png 确认切到无直接引用的根提交后详情清除，首行图标在深蓝背景正常遮挡。最终收紧 branch/tag 面板间距后截图在 `artifacts/commit-reference-labels-final/window.png`。当前字号固定 13、图标固定 16，与 Rebased 按 FontMetrics 高度计算仍有差异；未做不同 DPI、100 个以上引用、超长单个引用或逐像素验收。捕获了用户 Rebased 1168 当前窗口 rebased.png；定向消息点击未改变 Swing 的选择，rebased-selected.png 不能作为直接引用原生交互成功证据。本轮直接引用分组语义由源码确认，叠放外观参照用户截图及实际窗口；未修改用户仓库。自有窗口 24448、26408 关闭，Rebased 保留。没有自动滚动测试或新的总体内存结论。

便携包已更新，release／包内／宿主 exe SHA256 `36faa84422503b3fe88d6999fd7d06a9d3ed2ea01d4b4324757fa69a2cbb345f`。

同时保留先前 containing branches 数据层：`src/git.rs::containing_branches` 使用一次可取消、输出限额 1 MiB 的 `git branch --all --contains <oid> --format=...` 查询，不依赖已加载历史，不含 tag 或 remote HEAD 符号别名。测试覆盖历史窗口之外的祖先、merge、detached HEAD、Unicode 分支、不可达提交、非法 ID 和取消；Linux 通过，Windows 原生通过（之后新增的窗口长度断言只在 Linux 重跑）。此查询尚未接入右侧 In N branches／Show all，不将直接引用展示等同于该功能完成；排序目前 UTF-16 而非完整 natural comparator。

### 包含当前提交的分支列表接入（2026-10-16）

继续对照 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/details/commit/CommitDetailsPanel.kt::ContainingBranchesPanel` 和 `ui/frame/CommitPresentationUtil.java::getBranchesLinkText`。加载、空结果、In N branches、Show all／Hide 分离；收起按当前可用宽度先预留省略号与链接，展开逐行展示，切换提交重置展开。已原生操作用户 Rebased 1168，`artifacts/containing-branches-ui/reference-expanded.png` 确认 Hide 与逐行结构，reference-collapsed.png 为收起，reference-direct-refs.png 进一步确认直接引用区 HEAD／本地分支／tag 与 containing branches 分开。最初 reference.png 点击坐标来自缩放后的图片，未改变选择；后续按 PNG 原始 2100×1500 坐标操作成功。没有修改用户仓库或关闭 Rebased。

本地新增 `src/containing_branches.rs`，`src/ui.rs` 每个有效提交选择创建一个实体，独立 Cancel／Task；查询结果仅写回自身弱引用。刷新／切仓／转 Local 会释放旧实体并取消子进程，文件加载任务不会取消该查询；仅保留当前选择一份结果，无跨提交缓存。错误与真实空列表区分。收起按名称逐个测量，够宽就停止；只在最后一个名称上二分 UTF-8 边界缩短，避免每帧拼接整份列表。展开使用可选择的逐行文字，收起移除展开视图。字体 Inter 13、行高 17；链接支持点击，并接 Tab focus／Enter／Space（原生键盘验证因前台保护拒绝，尚未证实）。

Linux containing_branches 真实 Git 集成测试通过，Windows release 编译与原生行为通过。`artifacts/containing-branches-ui` 中 loaded／expanded／root-expanded 验证显示与展开；在专用 fixture 增加 ancestor-only 分支后 refreshed 显示祖先由 5 变为 6。最终构建证据 `artifacts/containing-branches-ui-final/expanded.png` 显示祖先全部 6 个名称，tip.png 显示新提交仍为 5，切换后重置收起且不混入 ancestor-only。首次 window.png 只是历史查询尚未结束的空列表，不作为完成证据。自有 PID 25844、25588 均关闭，未做滚动自动化。

剩余差异和验证边界：引用排序仍非完整 JetBrains NaturalComparator；错误文案属于本地反馈；极窄宽度可能截断头部、Unicode grapheme 边界与 Java char 遍历不同；大批分支展开、拖动分栏、在途取消竞态压力和切仓端到端仍待验证。参考截图还确认 tag 是灰色，而当前图标仍为旧蓝色，需核对主题后更正。不同窗口／fixture 证明行为与结构，不作为同条件逐像素验收。没有重新测量正式大仓库内存预算，启动采样不可替代验收。

预览包更新并校验 CRC 与包内／release／宿主 exe 一致，SHA256 `8189b4a22f72ff2bdc98d84c45cd2db594b3b4312dfb1bc8668d42b1a8bbf2ba`。

### 引用主题、自然排序和详情留白（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 ManyIslandsDark.theme.json 的 VersionControl.GitLog：head #F5D273、local #5FAD65、remote #B589EC、tag/other #868A91；`src/ui.rs::ref_color` 已按原值更正，替换先前近似色和错误的蓝色 tag。原生参考为上一节 reference-direct-refs.png，灰色 tag 与源码一致。重新读取 CommitDetailsPanel.kt 的 metadata 上下各 10、refs 组末底部 10；详情改为分块 margin，保留 message→metadata 20，metadata→refs 和 refs→containing 各 10，去掉统一 gap20 产生的多余空隙。

读取 `platform/util/base/src/com/intellij/openapi/util/text/NaturalComparator.java` 全文及 Strings.java::isDecimalDigit/compare；GitRefManager.kt::GitRefComparator 使用 GitReference 的自然比较。新增 `src/natural_order.rs`，数字按有效位长度和值比较，再比前导零／空格，忽略大小写后相等使用大小写敏感 tie-break，保留空格与标点的传递性规则；不采用文件名特有的连字符规则。ASCII 路径直接比较字节，无临时分配，非 ASCII 使用 UTF-16 单元与不展开的大小写映射。Rust/JDK Unicode 数据及 simple-uppercase 特例仍可能不同，未宣称完整 Java Unicode 等价。来源许可证加入 THIRD-PARTY.txt。

`ref_labels::compact_title`、`git.rs` 快照 refs 和 containing_branches 共用自然比较；快照 refs 与简短标签共用 rank，当前分支／main／origin/main 的优先级不被名称排序覆盖，右侧按类型分组后保留组内顺序。Windows fixture 的 feature/2、feature/10、v2、v10 用于实际检查。

Linux 43 库测试＋32 Git 集成测试通过。新测试覆盖数字位数、前导零、大小写次序、极长数字不解析整数、Unicode UTF-16 顺序及样本集合上的反对称／传递性；已有引用集成测试更新旧的纯名称排序预期，并加入数字分支及 containing 查询顺序检查。首次全量失败是旧排序预期与新优先级冲突，已按已核对的规则更新；最终全量通过。Windows 两项原生集成测试通过，日志 artifacts/reference-style-order/windows-tests.txt。Windows release 和原生 window.png 确认灰色 tag、正确颜色、main 优先、2 在 10 前，以及紧凑的 refs/containing 间距。PNG 已查看；不同 fixture／窗口尺寸不是同条件像素验收，无新内存预算结论或滚动自动化。自有 PID 30904 关闭，用户 Rebased 保留。

预览包更新并校验 CRC、包内／release／宿主 exe 一致，SHA256 `de205d43e336bb1a454ba18a1ce0197e22241972e0d315975f325f105e9c4e81`。

### 引用图标绘制内存回归：隔离、修复与复测（2026-10-16）

本轮使用 diagnosing-bugs 流程，先补充可失败的 Windows 原生内存回归入口。`scripts/windows-job-memory.ps1` 新增 SelectionCycles（在前两条提交间按 15／50／150 ms 各往返指定次数）、MemorySamples 和 AssertBudget。每组等 Job Object 内子进程全部退出，记录查询进程数量／存活数量／Private Bytes／Job 峰值，最终回到第一条提交并通过真实文件工具栏打开完整 Diff。AssertBudget 对 Welcome 检查稳定 80 MiB，对 LargeDiff 检查稳定 150 MiB，Job 峰值 250 MiB；失败也写出报告并由 finally 清理本次 Job。没有自动滚动。计数和最终截图不能证明每一个中间请求都已完成解析。

失败构建 `de205d43e336bb1a454ba18a1ce0197e22241972e0d315975f325f105e9c4e81`：实际仓库 full-context-memory-fixture 共 10,001 提交，UI 加载 10,000，HEAD／parent 的 review.rs 各 975,000 字节。60 轮往返切换后完整 Diff 的 20 个稳定样本 153.58–157.52 MiB，Job 峰值 220.31 MiB，GPU dedicated/shared 99.17／7.21 MiB（artifacts/reference-selection-memory）。删除切换步骤、只打开相同完整 Diff，6 次样本均 151.06 MiB、峰值 222.06，`-AssertBudget` 明确以退出码 1 失败（reference-memory-baseline-check），故不是依赖连续切换才出现的问题。命令为 Windows PowerShell 执行 windows-job-memory.ps1，参数 `-HistoryLimit 10000 -LargeDiff -DiffFileY 193 -MemorySamples 6 -AssertBudget`，Executable 为宿主验证 exe，Repository 为上述 fixture，OutputDirectory 使用新的 artifacts 路径。

按单变量实验验证：暂时只把 reference_icon 绘制换成同尺寸空元素，保持查询、文本、文件和 Diff 全部不变；同一最小复现降至 110.08 MiB，Job 峰值 153.34，GPU 55.44／4.57（reference-memory-no-icon-probe）。这将回归定位到引用图标的 PathBuilder/paint_path 绘制路径，而不是 containing 分支查询；没有进一步追踪驱动内具体资源分配，不能断言某个 atlas 尺寸或驱动泄漏。无图标诊断版本没有发布，临时替换已恢复。

最终修复沿用参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 TagPainter.kt／LabelIcon.java 几何、5.12 px 偏移、前后顺序、背景遮挡及前景圆点。新增三个可缓存 SVG mask（assets/icons/reference-{mask,outline,front}.svg），`src/assets.rs` 注册，`src/ui.rs::reference_icon` 分层着色而不再逐帧提交这些路径；保留所有引用、颜色和功能。源声明与许可证更新至 assets/icons/SOURCE.txt、docs/THIRD-PARTY.txt。没有按行生成独立 SVG 数据、没有扩大历史／Diff 缓存。

修复构建 `c35e40ddbb62451e07b74ea50b5bef9dd7696b93b671a62ed0e90a4c423c886e` 在原始未缩减流程（每个延迟 20 轮，共 60 轮／120 次选择，最后完整 Diff，20 次稳定采样）通过 AssertBudget：110.92–113.22 MiB，Working Set 107.54–109.78 MiB，稳定 Git 子进程 0；Job 峰值 204.48 MiB，累计 889 个进程、结束 active=1；GPU dedicated 55.41／shared 6.04 MiB，独立报告不直接相加。证据 artifacts/reference-selection-memory-svg，已查看 10,000 行 Log、最终正确提交与完整双栏 Diff 截图；每批选择后的应用 Private Bytes 约 94.49／95.74／94.79 MiB，没有该批次间持续增长的证据。真实运行的机器／150% DPI 沿用前述 Windows 11／5800H／RTX 3060 环境；这不是冷缓存时间测试或长期无泄漏证明，也未单独捕获每个 Git 短命进程的峰值。

`artifacts/reference-svg-visual/window.png` 已查看，专用多引用 fixture 中灰色 tag、彩色分支、叠放遮挡和右侧分组保持，数字自然排序未回退。图标光栅化由 SVG 缓存承担，尚无不同 DPI 的逐像素差分验收。原生所有测试窗口均由测试 Job 或检查 PID／名称关闭，用户 Rebased 保留。正式回归命令和预算断言留在脚本中；没有新增无法覆盖原生渲染分配的模拟单元测试。当前便携包仅包含保留完整图标的修复构建。

同一修复构建 Welcome 20 次稳定样本 53.25–53.27 MiB，Working Set 55.11–55.12 MiB，Job 峰值 55.60 MiB，GPU dedicated/shared [(17.37, 1.22)] MiB；仅 1 个进程、无 Git，通过 80 MiB 稳定预算。证据 artifacts/reference-svg-welcome-memory。报告 historyLimit=10000 是参数默认值，Welcome 没有加载历史。便携包 CRC 与包内／release／宿主 exe 字节一致已验证。


### 底部预览与独立 Diff Tab 共享显示设置（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/diff-impl/src/com/intellij/diff/actions/impl/SetEditorSettingsActionGroup.kt` 和 `platform/diff-impl/src/com/intellij/diff/tools/util/base/TextDiffSettingsHolder.kt`：齿轮菜单提供真实编辑器显示选项，设置按使用场景保存。本地新增 src/diff_settings.rs，供 src/diff_tab.rs 与 src/ui.rs 共用 Lines／Words／Characters、Show Line Numbers、Soft-Wrap 菜单和强调粒度转换；底部预览接入真实渲染属性，沿用 diff-editor-log／diff-editor-changes 两份有界设置文件。保存失败单独提示，不覆盖 Git 内容错误或丢弃当前 Diff。

Windows release 编译通过，Linux appearance 持久化／非法内容回退测试通过。原生独立测试 profile 未修改用户偏好：artifacts/bottom-diff-settings/tab-shared.png 确认底部设置传至独立 Tab，returned-lines.png 确认 Tab 改回 Lines 后返回底部生效。初次 local-menu.png 暴露无本地修改时菜单继承了 Log 状态；将偏好读取提前至 load_file 的空文件返回之前后重建。最终 artifacts/bottom-diff-settings-final/local-menu.png 确认 Local 默认 Words／显示行号／不换行，log-menu.png 确认切回 Log 保持 Lines／隐藏行号／换行，并跨重启保存。两张最终截图已查看。自有 PID 2112／32100 均按名称校验后关闭，用户 Rebased 保留；未进行滚动自动化。

保留差异：底部仍为双栏片段预览及原有标题布局，本轮不包含底部单双栏切换或完整布局还原。JetBrains CHANGES_VIEW 默认不显示行号，本地既有 Changes 默认仍显示；当前 Log／Changes 场景映射不宣称覆盖所有 DiffPlaces。未重新验证大文件自动换行内存或写入失败原生路径，无新总体内存结论。源码及先前设置菜单参考继续适用，本轮未新增 Rebased 设置菜单同条件截图，未达到逐像素验收。

便携包 CRC 及包内／release／宿主 exe 字节一致校验通过，SHA256 `772396fa1ea1a9face094cc6b7d5233155a6380e0e968c34a0c50de020c8dff8`。


### Islands 表面层级、分支树入口与底部 viewer（2026-10-16）

用户指出标题／按钮区颜色、All branches 常驻行和圆角不统一。重新读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 ManyIslandsDark.theme.json：MainToolbar／外壳采用 main-window-bg=#26282C，editor／tool-window 背景 #191A1C。TitleBar 原有组件默认渐变已由 src/ui.rs 显式底色覆盖，包括 Windows 窗口按钮所在区域；Tab 栏与内容共享左右 8 px 外边距，删除二者间的外壳色间隙。Log／Diff／Settings 外部统一保留圆角，内部相邻分栏保持直边。原生发现仅 rounded＋overflow_hidden 不能阻止不透明子内容覆盖底角，新增四个本地几何 SVG corner mask（assets/ui/corner-*.svg，src/assets.rs 注册），按面板数复用，不逐行生成。10 px 外角为当前视觉近似；读取过 XNextRoundedBorder.kt，但它的 35 px 直径及 12 px insets 属于另一组件，未直接套用或宣称当前外角是该源码精确尺寸。

BranchesTreeModel.kt::NodeDescriptorsModel.buildTreeNodes 将 HEAD 放在 Local／Remote／Tags 等组之前，无 All branches 节点；BranchesDashboardTreeComponent.kt 将搜索放在树上方。本地去掉常驻 All branches 按钮，增加 HEAD (Current Branch) 行，仍通过既有 select_branch 查询；无 HEAD 时隐藏。清除分支过滤保留在 Branch 下拉菜单。HEAD 跟随搜索显隐，搜索命中 HEAD 时不再额外出现 No matching branches。用户 Rebased 1168 的真实截图 artifacts/islands-surfaces/reference-branches.png 已查看，证实该结构。尚未补齐分支树 Tags、收藏、空组隐藏和全部竖向动作栏，不宣称完整树一致。

DiffHeaderToolbarPanel.kt::DiffToolbarIslandPanelUI 的背景 Editor.SearchField.background=#212326，边框 #26282C、半径 6、左右 insets 6／顶部 2、内部高 40；独立 Tab 与底部工具栏均采用这些值。参考原生 reference-diff.png 及用户 diff-tab.png。底部新增共享单双栏选择器（src/diff_settings.rs），沿用 Log／Changes 持久化偏好；Unified 标题纵向排列，新增／删除只显示有内容一侧和对应标题，工具偏好不因单侧文件改变。沿用 DiffToolChooser.kt／SimpleDiffTool.java／UnifiedDiffViewer.java 的规则。底部仍读取片段且保留旧标题文案、文件 header，与独立全文 Tab 尚有差异。

Windows release 编译通过，Linux layout_preferences 三项测试通过。原生隔离 profile：artifacts/islands-surfaces-release/head.png 与 cleared.png 验证 HEAD 选择／Branch 菜单清除；added.png 验证 Split 偏好下新增文件仍单侧展示；tab-added.png 验证独立 Tab 的新工具栏和单侧内容。先前 artifacts/islands-surfaces-final/unified.png 验证底部 Unified、纵向标题；tab.png 验证偏好传到独立 Tab，settings.png 验证设置页圆角。这些截图已查看，未做滚动自动化。中间原生窗口 34668／31320／9020 已按名称检查后关闭，未改用户偏好或仓库。新构建未重新跑总体内存预算，启动样本不能代替内存验收；仍欠同尺寸多 DPI 像素差分、主工具栏项目色渐变及左右完整工具窗口按钮区。

### Diff 分段按钮实际边框问题（2026-10-16）

用户怀疑截图缩放导致单双栏按钮异常。进一步读取 SegmentedButtonComponent.kt::paint、SegmentedButtonToolbar.kt::SegmentedButtonBorder／SegmentedButtonLook，以及 Islands 的 SegmentedButton 配色。实际规则是整组外框 #40434A，选中半边独立完整圆角框 #5F6269，非聚焦选中填充 #26282C；未选中背景透出工具栏 #212326。现有 GPUI ButtonGroup 不另画组框，而会移除相邻子按钮内侧圆角／部分边；因此初步“双重组边框”猜测不成立。旧实现两半均填 #26282C，且中间直缝，确有源码与原生外观差异，不能归因于低分辨率截图。

src/diff_settings.rs 改为共享外框＋独立圆角按钮，保留 Button 自身键盘激活和 toggled 可访问状态；selected 半边保留完整轮廓，未选中透明。该改动同时应用底部和独立 Tab。尚未逐项对齐 JetBrains 的组焦点蓝框及左右箭头组内选择，不能宣称焦点态完整还原。

最终原始像素裁剪：artifacts/segmented-viewer-release/reference-control.png 为实际 Rebased，split-control.png／unified-control.png 为本地两个选择状态，均不缩放，已查看。另发现 Button.small 会重新缩放传入 icon，改为 child Icon 固定 16 px；每半边宽 49、高 28 按当前相同 DPI 原生控件尺寸近似校准。原生切换双栏／单栏截图验证真实布局随按钮变化。选中半边独立圆角轮廓、未选中透明背景、图标尺寸已校正；图标抗锯齿和全部 hover／focus 状态不作为逐像素完成证据。Windows 最终 release 编译通过。用户 Rebased 已返回原先 Log／分支树收起状态；本轮测试窗口均按 PID 与名称检查关闭，偏好仍隔离于 bottom-settings-profile。

最终便携 ZIP CRC 与包内／release／宿主 exe 一致，SHA256 `c284526c0e784646852355e90aa3a8c5f036e88a3b2cfdf85602f41555e41d89`。此前生成的中间 surfaces ZIP 已删除，不替代最终包。


### Islands／分段按钮修正后的当前构建内存验收（2026-10-16）

上一阶段已有运行时代码及原生交互进展，本轮补齐这些视觉改动后的内存证据，未修改应用代码。测量 SHA256 `c284526c0e784646852355e90aa3a8c5f036e88a3b2cfdf85602f41555e41d89`；重新验证便携 ZIP CRC、包内／release／宿主验证 exe 字节一致。使用 Windows 150% DPI 既有环境，通过子进程 LOCALAPPDATA 指向独立 islands-memory-profile，未修改用户偏好。仓库 full-context-memory-fixture 重新以 Git 核实共 10,001 提交，HEAD／parent 的 review.rs 均为 975,000 字节。

可复现入口 artifacts/islands-memory-launch/run.ps1 调用 scripts/windows-job-memory.ps1，参数 `-HistoryLimit 10000 -LargeDiff -DiffFileY 193 -SelectionCycles 20 -AssertBudget`；按 15／50／150 ms 各往返 20 轮，共 60 轮、120 次提交选择，最后打开首行完整双栏 Diff。查看 islands-selection-memory/history.png、window.png 和 selection-final.png，确认 10,000 条已加载历史、当前正确提交、完整 Rust 内容与新版工具栏。各批启动 Git 查询 219／243／360 次，等 Job 内子进程全部退出；每批结束 Private Bytes 约 91.84／93.54／93.08 MiB，不能据此证明长期无泄漏或每个中间请求完成解析。

| 场景 | 稳定 Private Bytes（20 样本） | 稳定 Working Set | 应用＋子进程 Job 同时峰值 | GPU Dedicated／Shared（另列） |
| --- | --- | --- | --- | --- |
| 10,000 历史＋连续选择后完整双栏 Diff | 112.51 MiB | 111.11 MiB | 202.42 MiB | 55.43／6.10 MiB |
| 未打开仓库的欢迎窗口 | 53.16–53.17 MiB | 55.06–55.07 MiB | 55.36 MiB | 17.37／1.22 MiB |

两项均以退出码 0 通过 AssertBudget（完整 Diff 稳定 150 MiB、欢迎稳定 80 MiB、Job 峰值 250 MiB）。Job 从挂起启动时包含继承子进程；Diff 场景累计 889 进程、稳定期 active=1，欢迎全程 1 进程。稳定采样直接子进程 Private 为 0，未另测每个短命 Git 的单独峰值；GPU 不直接加到 Private Bytes。原始结果在 artifacts/islands-selection-memory 与 islands-welcome-memory 的 memory.json／steady.json，欢迎截图已查看，报告 historyLimit=10000 在 Welcome 无效。最后验证自有 PID 33324／33336 均消失，用户 Rebased 1168 仍在。

结论限于这两个明确场景：当前圆角和分段按钮修正未重现此前引用图标绘制的内存超标，不需要缩减 UI 或扩大预算。未做冷启动计时、自动滚动、超长单行换行、多仓切换、不同 DPI 或长期压力验收；完整计划仍未完成。现有预览包即本次测量构建，无需重新打包。


### 分支树排序、空组与 Tag 导航前置数据（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 plugins/git4idea/backend/src/ui/branch/dashboard/BranchTreeNodeComparator.kt、BranchesTreeModel.kt::NodeDescriptorsModel.buildTreeNodes/groupByPrefix：当前引用权重 0，收藏 1、含收藏目录 2、普通目录 3、其余 4；同权重使用自然名称排序，仅添加非空顶层组。本地 src/branches.rs 对当前引用／目录／普通引用排序，复用已有 natural_order，删除无内容 Local／Remote 等空组；收藏尚未建模，未伪造收藏顺序。src/ui.rs 传入当前分支完整名，避免普通 main 永久冒充当前分支。原生 artifacts/branch-tree-order/expanded.png 已查看：main 最前、feature 目录在 ancestor-only 前、feature/2 在 feature/10 前。

进一步读取 BranchesDashboardTreeController.kt、BranchesTreeSelection.kt，发现 selectedBranchFilters 只纳入 Branch／HEAD，不纳入 Tag，Tag 在 logNavigatableNodeDescriptor 中走引用导航；因此撤回本轮中间“Tag 点击等同于分支过滤”的接线和 snapshot 接受 Tag 过滤参数的扩展。当前 UI 继续只显示分支，尚未完成 Tags 分组入口。该决定遵循源码行为，后续应实现真正引用导航（含已加载历史之外、过滤遮挡的目标），不可仅以筛选该 Tag 祖先来宣称还原。

保留前置数据：git.rs::Snapshot.tags 独立保存完整 refname＋已解析 commit ID，直接复用现有 for-each-ref 及嵌套 tag 解引用逻辑，无新增 Git 调用，非 commit 的 tag 不作为可导航历史目标。branches::filtered_ref_rows 已可构造 Tags 分组及目录，但 UI 暂传空 tags，等待导航接入；现有右侧直接 tag 引用展示不受影响。新增真实 Git 测试：仅加载一条较新历史时仍保留旧提交的 lightweight／annotated／nested tags，排除 blob tag，确保 tags 不混入 branches。Linux 与 Windows 原生均通过。新增树测试覆盖数字排序、当前分支优先、局部折叠与同名 tag 目录互不干扰、搜索自动展开和空输入不产生空组。

Linux 全量 44 库测试＋33 仓库集成测试通过；撤回 Tag 过滤扩展后重跑受影响的 tag_tree_preserves 测试通过，Windows 相同最终用例通过。Windows release 编译通过，专用 profile 未修改用户偏好，自有 PID 28860 已按名称检查关闭，用户 Rebased 未操作。没有滚动自动化或本轮内存预算复测。原生参考沿用已查看的 artifacts/islands-surfaces/reference-branches.png；收藏／Tags 导航、全部树图标和快捷键仍未齐全，不宣称像素级完成。

预览 ZIP CRC 与包内／release／宿主 exe 一致，SHA256 `bb5c10eae4cf9534f15dca9374a9e5d9d55629fbb4ef249e191165599c4b6ea2`。


### 引用导航：固定快照中的流式定位与窗口重建（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 BranchesInGitLogUiFactoryProvider.kt::navigateTo、platform/vcs-log/impl/src/com/intellij/vcs/log/impl/VcsLogNavigationUtil.kt::jumpToRefOrHash/jumpToCommit，以及 ui/VcsLogUiImpl.java::handleCommitNotFound。引用先解析为 commitHash，再在当前可见历史定位；“不存在”和“不匹配当前过滤”分开。过滤遮挡时提供清除过滤后查看或新 Log Tab 的显式操作，不默默重置过滤。单击树导航／过滤在 Rebased 为两个可配置动作，两者默认 false；本地既有单击分支过滤仍是待统一的交互差异，不能把它当作上游默认。

本地新增 src/history_position.rs 固定 8 KiB chunk＋64 字节记录缓冲，扫描 Git date-order 的完整 OID 序列，仅保存目标位置与计数。git.rs::commit_position 要求目标及 roots 为完整 OID，固定 roots，不使用可移动 refname；空 roots 返回未找到，非法参数／Git 失败／取消单独返回错误。复用 read_stream 的子进程取消、30 秒超时、stdout/stderr reader join 和回收；不把所有历史输出留在内存。找到目标后仍排空并验证后续流，以保留正常 Git 退出状态，当前 CPU 工作量仍为 O(可达历史)，不宣称命中后立即停止。

history.rs::locate 根据位置选择有界页，调用现有 read 重建此前 graph frontier，返回 offset／selected／Window，核实该页目标 ID；不会改写真实 parents。它明确只处理无过滤历史，UI 尚未接入，调用方必须显式处理当前过滤；Tags 面板入口仍未完成。没有新增无界历史索引或跨目标缓存。

Linux 两个流式测试通过：跨 chunk、210,000 条前置记录（超过旧 8 MiB 文本输出预算）、SHA-256 OID、空／未命中，以及命中后的错误记录、非法字符、超长和截断。真实 Git 集成 commit_navigation_keeps_pinned_history_order_and_graph_window 通过：分叉＋merge 的每个目标与完整 snapshot 顺序、真实 parents 和分页 graph 几何逐项相等；固定 roots 后 live branch 前移仍不串历史，新增提交明确不在旧 roots 内，非法 ID 和预先取消明确失败。首次测试编译失败因 Row 无 PartialEq，已改为逐字段比较节点／宽度／颜色／所有边几何，无生产结构改动。Windows 原生相同集成测试通过（artifacts/commit-navigation/tests.ps1）；未新测在途取消时点或超大真实仓库定位耗时。

本轮没有界面变化，不重做 GUI 截图或发布数据层未接入的预览。便携包仍为上一阶段 `bb5c10eae4cf9534f15dca9374a9e5d9d55629fbb4ef249e191165599c4b6ea2`；完整导航 UI、过滤遮挡提示、切仓／取消竞态及实际 Rebased 操作对比留待接入验收。

### Diff 分段控件的组焦点与方向键（2026-10-16）

继续读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/platform-impl/src/com/intellij/ui/dsl/builder/components/SegmentedButtonComponent.kt::init/moveSelection、SegmentedButtonToolbar.kt::getSegmentedButtonBorderPaint/SegmentedButtonLook，以及 platform/util/ui/src/com/intellij/util/ui/JBUI.java::Button.focusBorderColor/Focus.focusColor 和 ManyIslandsDark.theme.json。整组一个焦点，左右方向键在边界 clamp，不循环；外框使用焦点色，选中半边 focusedBorderColor 实际解析为 control-bg（#191A1C），不是蓝色；选中背景 #2E4D89。鼠标 hover 参考使用 #FFFFFF17。

src/diff_settings.rs 的共享 mode_selector 改为 RenderOnce 控件，用窗口 keyed FocusHandle 管理组焦点，子按钮不再各占一个 Tab 停点，左右键改变 viewer 偏好并调用原有保存路径；边界重复按键不重复保存。带修饰键的方向键不截获。选中态、普通边框和图标仍保留；两种 Diff 入口共用该实现，无文件缓存或 Git 查询变化。增加 hover 样式覆盖，但最终截图的未选中半边在鼠标停留时仍呈偏蓝，可能仍有组件状态样式叠加，悬停色尚不能宣称验收完成。焦点环厚度仍为本地 1 逻辑像素近似，未完成与 Darcula outline 几何的逐像素比对。

Windows release 构建通过。artifacts/segmented-keyboard 中第一轮通过前台 SendKeys 验证右键变单栏、左键恢复双栏、Tab 跳到齿轮；后续原生参考操作因前台保护拒绝，未发送鼠标／键盘到用户 Rebased，故没有新的参考焦点截图。第二轮 artifacts/segmented-keyboard-final 也被前台保护中止，不能作为最终键盘证据。最终 artifacts/segmented-keyboard-verified/check.ps1 改为仅向验证进程 HWND 投递按键，确认单栏、双栏、左右边界和 Tab 离开组；最终 unified、left-boundary、tab-next 原图已查看。最终测试进程按名称检查后关闭，配置使用隔离 segmented-keyboard-profile，未测试滚动或重做内存压力。本轮仅验证独立 Tab，底部复用接线但未补原生键盘覆盖。

最终预览 ZIP CRC、包内 exe、release exe 和宿主 exe 一致，SHA256 418ddeed454eaeda62b0214d27dcb92fc5e37c69d1493fcb86612d3ba2306477。中间 ZIP 已删除；新包包含前一阶段未接 UI 的导航数据层，但不表示 Tags 导航功能已完成。当前完整计划仍未完成。

### Diff 悬停偏蓝：组件状态覆盖修正（2026-10-16）

沿用参考 4aec35256d765c217aa0d662b82ac4e860c396fd，本轮重读 SegmentedButtonToolbar.kt::SegmentedButtonLook：POPPED 使用 ActionButton.hoverBackground，即 Islands #FFFFFF17；PUSHED 使用按组焦点决定的 selectedButtonColor。定位本地 vendor/gpui-component/src/button/button.rs::render/hovered：Ghost 在 render 中安装自己的 hover，取 accent 的 0.5 透明度，晚于调用者的 .hover；因此上轮仅添加 hover refinement 不能覆盖未选中按钮的蓝色。

src/diff_settings.rs 改用 ButtonCustomVariant，明确 foreground、hover 和 active 状态色，保留已有选中样式、组焦点、方向键、尺寸和图标。不修改全局 Ghost 样式，避免影响其他按钮；无新增后台任务、Git 调用或缓存。选中按钮的 hover refinement 保留，因为 Button 自身对 selected 状态不安装 hover。

Windows release 构建通过。专用 PID 27348、隔离 segmented-keyboard-profile，artifacts/segmented-hover/check.ps1 仅投递目标 HWND，验证右键单栏、右边界保持、左键双栏、左边界保持、Tab 到设置按钮；unified 与 tab-next 原图已查看。pixels.ps1 对相同 2182×1361 原图坐标 (1965,135) 取样，上一版 #273857，本版 #35373A，符合 #FFFFFF17 叠加 #212326 的灰色（见 pixels.json）。这证明此前偏蓝不是截图缩放问题，当前未选中悬停已修正。未补鼠标持续按下、多 DPI、底部预览原生键盘或 Rebased 新焦点态同条件对照，不扩大为全部状态像素验收。未操作 Rebased，本轮自有窗口已关闭；无自动滚动或总体内存复测。

预览 ZIP CRC、包内／release／宿主 exe 一致，SHA256 35bf44202e08eab8c7728129f21c7ec274511b51dd5e1fa42ebbd78320721130，已替换 dist/GitReview-windows-x64-preview.zip。整体计划仍在进行，下一步继续引用导航及未完成的 Islands 布局差异。

### 引用导航：保留过滤语义与目标分类（2026-10-16）

重读参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/VcsLogUiImpl.java::handleCommitNotFound：目标存在但不匹配过滤时，显式提供 clearFilters 后跳转／新 Log Tab，不能默默清除过滤。沿用已确认的 hash 优先、路径按原始图显示规则。

src/history.rs 新增 locate_filtered：固定完整 OID roots，复用普通过滤／哈希过滤的拓扑、匹配集合与投影实现，按原始图中的可见顺序计算目标页，只为该页加载提交正文元数据。FilteredLocation 明确区分 Found（含窗口、offset、selected）、Hidden（在 roots 内但不匹配过滤）、OutsideRoots；错误和取消仍走 Result。命中的 hash 优先于路径／作者条件，无 hash 命中则回退正常消息查询。没有把路径简化后的父提交当作 Diff 父提交，也不重复按页查询整个过滤图来寻找目标。read_filtered/read_hashes 共享提取出的 read_topology/read_matches/hash_matches，避免导航另写过滤规则。

内存边界仍沿用现有临时 100,000 提交／500,000 边过滤图限制，超出返回错误，不保留部分结果冒充完整历史；该限制不是最终大仓库验收结论。本次未新增持久索引或跨导航缓存，定位仍需构建有界完整拓扑。未接入 Tags UI，也未更改任何过滤控件状态。

Linux 全量 46 库测试＋35 仓库集成测试通过。filtered_navigation_preserves_display_order_filters_and_real_parents 用真实 Git 对比 read_filtered 的可见顺序、分页 more、图几何及真实 parents；覆盖文本、正则、路径、空作者集合、日期、hash 覆盖非法路径、未命中 hash 回退、隐藏／范围外分类、固定 roots 后分支前移、非法 OID 与取消。补充现有 path_filter_merge 用例逐个定位 merge 及两侧提交，确保保留两个真实父提交；Linux 补充用例通过。Windows 原生相同两个测试通过（14.89s／5.64s，artifacts/filtered-navigation/tests.ps1）。耗时是测试整组多次定位，不是单次导航性能测量。

本轮没有 UI 改动，未进行 GUI 截图／滚动或新便携包发布；当前预览仍为 35bf44202e08eab8c7728129f21c7ec274511b51dd5e1fa42ebbd78320721130。下一步将此结果接入树引用导航及显式清除过滤提示，并处理异步取消、切仓和过期结果，不能将本次数据层验证说成 Tags 功能已完成。

### Tags 分组与显式引用导航接入（2026-10-16）

进一步读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 plugins/git4idea/backend/src/ui/branch/dashboard/BranchesDashboardActions.kt::NavigateLogToSelectedBranchAction、BranchesDashboardTreeComponent.kt 的工具栏动作注册、backend/resources/intellij.vcs.git.backend.xml 和 shared/resources/messages/GitBundle.properties。确认双击／Enter 绑定的是 Change.Branch.Filter，显式导航另有 Navigate.Log.To.Selected.Branch 动作，复用 SelectIn 快捷键；动作原文是 Navigate Log to Selected Branch Head。不能把 Tag 双击直接改成导航并声称是上游默认。本地先提供同名右键菜单动作，Tag 单击只选择；已有分支单击过滤尚未统一到上游默认，工具栏导航按钮／SelectIn 快捷键亦待补齐。

src/ui.rs::rebuild_branches 启用 Snapshot.tags，树搜索提示改为 Branches and Tags，复用既有层级、折叠和自然排序。navigate_tree_ref 固定引用当时的 commit ID，已加载目标直接选中，否则后台调用 locate／locate_filtered。范围外目标再检查全仓 roots，区分当前分支／快照遮挡和不存在。导航不会把 Tag 当作分支祖先过滤。隐藏目标保留原历史、筛选和详情，通过明确 Clear filters and show 动作重查；成功后才清空 Branch／Text／Date／User／Paths 状态并替换历史，失败不清空原筛选。UserMenu/PathsMenu 新增不发刷新事件的 clear_filter，保留最近选择。导航成功从 Local Changes 切回 Log，避免使用本地 Diff 语义。

使用 Review 现有 token／generation／task 生命周期：开始导航取消旧查询但不清空当前比较；刷新、选择提交／文件、切仓、进入 Diff 等 reset_job 路径取消导航并清掉提示，过期 generation 不回填。当前历史刷新期间拒绝新导航，未假定旧 roots 可用。尚未做确定性的在途切仓／快速多目标竞态测试，不将代码检查当成竞态验收。加载和隐藏提示目前是 Log 上方横条，属于暂存布局差异；与 Rebased 通知气泡、新 Log Tab 备选动作及完整上下文菜单尚未逐像素还原。

Windows release 构建通过。专用四提交／四 Tag fixture、初始 limit=2：artifacts/tag-navigation-final/older-page.png 确认 v1 从首窗跳到 offset=2，选中 review 1 并更新真实文件／详情；local.png 与 local-to-log.png 确认从 Local Changes 通过菜单返回 Log 的已加载目标路径。前一构建 artifacts/tag-navigation/hidden.png 确认 review 4 文本过滤和选中项保留；cleared.png 确认用户点击清除后恢复完整历史、清空文本并选中 review 1。初次 older-tag.png 只拍到 Locating，最终 older-page 才是跨页完成证据。未做滚动自动化。自有 PID 33828／33504 已按名称检查关闭，profile 隔离，用户 Rebased 未操作。尚未原生验证 User／Paths／Date 的组合清除、分支范围外提示、多 DPI、通知外观及导航内存峰值；本轮未重复上一阶段已通过的定位核心测试。

最终预览 ZIP CRC、包内／release／宿主 exe 一致，SHA256 2bff5f1ea75f13ca01e50b7294d3d72c40b0ffbee524a58b419ec2d81d03ae6d，已替换 dist/GitReview-windows-x64-preview.zip。功能已有原生端到端进展，仍不标记完整导航／UI 还原验收完成。

### 分支树默认选择、过滤与键盘操作（2026-10-16）

重读参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 backend/resources/intellij.vcs.git.backend.xml、BranchesTreeSelection.kt，以及 platform/platform-resources/src/keymaps/$default.xml 的 SelectIn：双击／Enter 是 Update Branch Filter in Log，Tag 不属于 selectedBranchFilters；显式导航使用 Alt+F1。保留前文确认的“单击 FILTER／NAVIGATE 两项默认 false”。本地不再以单击分支立即过滤冒充默认行为。

src/ui.rs 的 HEAD／分支／Tag 行单击仅更新 selected_tree_ref 和树焦点，不调用历史查询；双击／Enter 对 HEAD／真实分支调用现有过滤，Tag 不触发过滤。树右键增加源码原文 Update Branch Filter in Log，仅分支显示。新增单一 tree_focus、保留虚拟列表 tree_scroll，上下／Home／End 选择可见节点，左右折叠展开、进入子节点／返回父节点；搜索期间不改变保存的折叠状态。Alt+F1 仅在树本身聚焦时导航所选 HEAD／分支／Tag，搜索输入拥有焦点时不拦截其按键。分组选中也有高亮，不再用 branch filter 冒充树当前选中。键盘选择不新增 Git 进程或提交缓存。

Windows release 构建通过。artifacts/tree-interactions 的专用四提交 fixture 增加 old→v1：single-select.png 确认选中 old 但历史仍 review 4／3，branch-enter.png 确认 Enter 后只有 review 1；tag-enter 保留 old 过滤，collapsed／expanded 确认左／右操作，已查看 single-select、branch-enter、collapsed 原图。navigate-key.ps1 通过前台检查后发送 Alt+F1，alt-f1.png 确认 v4 被 old 分支过滤遮挡，历史保留且出现明确清除操作；这同时补到分支范围外目标提示的原生证据。double-click.ps1 向目标 HWND 发送双击序列，double-main.png 确认 main 过滤恢复四条提交。验证 PID 22956 已按名称关闭，隔离 profile，不操作用户 Rebased，无滚动自动化／总体内存复测。

限制：仍未实现上游可配置的单击过滤／导航开关、多选、完整导航工具栏和所有树快捷键；文件夹整行单击折叠仍是简化交互；选中态圆角／失焦配色和通知横条尚待统一，未取得本轮 Rebased 同条件交互截图，不宣称完整像素验收。Home／End 与搜索焦点隔离为代码接线，未单独原生验证；在途切仓／快速导航竞态仍待验证。未增加镜像实现的测试，原生交互验证本轮变化。

最终预览 ZIP CRC、包内／release／宿主 exe 一致，SHA256 96a02b01e1996baea994a628ce9ebef33226df3a999f209d189cee4df7ab79f4，已替换 dist/GitReview-windows-x64-preview.zip。整体目标继续进行。

### 列表选中态焦点配色与树行圆角（2026-10-16）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd，读取 ManyIslandsDark.theme.json 的通配 selectionBackground／selectionInactiveBackground：分别 selection-bg-active→blue-50 #2A4371、selection-bg-inactive→gray-40 #33353B；检查父主题 expUI_dark.theme.json 未发现 Log 特定覆盖。VcsLogGraphTable.java 的 SELECTION_BACKGROUND／SELECTION_BACKGROUND_INACTIVE 取 List 对应色。DefaultTreeUI.java 根据 RenderingUtil.isFocused 绘制选中态，新 UI 的选中底色使用 HORIZONTAL_SELECTION_OFFSET=12 和 JBUI.CurrentTheme.Tree.ARC；JBUI.java 默认 Tree.Selection.arc=8（圆角直径）。源码中的左边距还会与 renderer/control 偏移取较小值，不能将固定 12 宣称覆盖全部缩进状态。

src/ui.rs 新增 row_selection，依据各自 tree/history/files FocusHandle 和窗口活动状态选择颜色，三个列表不再永久同时使用蓝色。on_focus／on_blur／window activation 通知使留存虚拟行重绘；Log 内引用图标的 SVG 遮罩同步使用实际行底色。树与文件叶子选中背景使用独立绝对定位底层，左右 12、半径 4，文字和图标布局不挪动，不改变列表行高。仅在选中行增加轻量 div，未新增 PathBuilder 图标或提交缓存。Log 行仍用原有全宽背景；其圆角／边距待继续对照，未随意套用 Tree 几何。

Windows release 构建通过，artifacts/selection-focus 的 tree-focused、files-focused、input-focused 原图已查看，树／文件选中行圆角和左右留白生效；历史引用遮罩无异色块。pixels.ps1 对 tree/history/files/input 四种焦点状态，各检查三个列表的原始 PNG 固定内点，12 项断言通过：聚焦 #2A4371，其余 #33353B，输入框聚焦时三项均灰。结果 pixels.json。测试 PID 21080 按名称检查后关闭，使用隔离 profile，未操作用户 Rebased，未执行滚动自动化。

保留差异：窗口整体失活／恢复、深层目录左边距、文件组／文件夹选中、多 DPI、Log 行圆角和通知气泡仍未完整验收；没有本轮 Rebased 同条件新截图，参考原生仍沿用已保存的 Islands 树／用户文件列表截图，不宣称逐像素完成。未重做总体内存压力，不能将启动采样当成预算验收。

最终预览 ZIP CRC、包内／release／宿主 exe 一致，SHA256 706e0e7fe38f2e731d9b3b9e8e4cb719e0745cef13f0e4f46ff6838c1eabd151，已替换 dist/GitReview-windows-x64-preview.zip。完整计划继续进行。

### 引用导航与焦点配色后的内存复测（2026-10-16）

当前 release、宿主 diff-tabs/gitui-diff-tabs.exe、便携 ZIP 内 exe 字节一致，ZIP CRC 通过，SHA256 706e0e7fe38f2e731d9b3b9e8e4cb719e0745cef13f0e4f46ff6838c1eabd151。本轮只验证，不修改程序或重新打包。前一轮对按钮旧证据的复核没有新增实现；本轮补齐近期 Tags／导航／列表焦点更新后尚缺的当前构建内存证据。

通过 artifacts/focus-memory-launch/run.ps1 调用 windows-job-memory.ps1：-HistoryLimit 10000 -LargeDiff -DiffFileY 193 -SelectionCycles 20 -AssertBudget，使用独立 focus-memory-profile。full-context-memory-fixture 实际 10,001 提交，HEAD 与 parent 的 review.rs 均 975,000 字节；history.png 确认 UI 加载 10,000，selection-final.png 确认最终打开完整文件双栏 Diff。15／50／150 ms 各 20 次往返，共 60 次往返／120 次选择；各组新增进程 230／243／360，组间均只剩应用进程。截图不证明每次短间隔请求都完成解析；没有自动滚动。

artifacts/focus-selection-memory：20 个稳定样本 Private Bytes 114.39–116.98 MiB，Working Set 116.98–119.48 MiB，采样时直接子进程 Private Bytes 为 0；从挂起启动绑定 Windows Job 记录的整组峰值 212.59 MiB，累计 900 个进程、结束统计时活跃 1。GPU dedicated 55.41 MiB、shared 4.29 MiB，单独报告，不叠加为 CPU 内存。稳定最大值低于 150 MiB，含 Git 子进程峰值低于 250 MiB，断言通过且执行退出码 0。

artifacts/focus-welcome-memory：无仓库欢迎页 20 个样本 Private Bytes 均 53.11 MiB，Working Set 55.03–55.04 MiB，Job 峰值 55.32 MiB，累计仅 1 个进程；GPU dedicated 17.37 MiB、shared 1.22 MiB。欢迎页 80 MiB／整组 250 MiB 预算断言通过，截图已查看。check-cleanup.ps1 确认自有 PID 24760／10692 均已退出，用户 Rebased PID 1168 保留。

相较旧 c284526c 构建的稳定 112.51 MiB／Job 峰值 202.42 MiB，本次有小幅上升，但未进行交替重复的受控 A/B 测量，不能归因于某个 UI 改动，也不称为完全无回退。结论限于当前两个场景满足预算；多仓切换、导航在途取消、多文件 Tab、长期运行及不同 DPI 仍未验收。后续继续 Log 行圆角、通知外观等尚未完成的 Rebased 对齐工作。

### Log 单行选中圆角与边距（2026-10-16）

读取 Rebased 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/vcs-log/impl/src/com/intellij/vcs/log/ui/table/VcsLogNewUiCellWrapper.kt：单仓无 root column 的选中背景左右 INSETS=4；SINGLE 四角使用 Popup.Selection.ARC，连续多选分别 TOP／MIDDLE／BOTTOM。background 取未选中样式，selectionColor 单独叠加；因此边缘应保留 current-branch 背景。JBUI.java 中 Popup.Selection.arc 默认 8（直径）；ManyIslandsDark 未发现此键覆盖。重新查看用户原生截图 artifacts/rebased-user-reference/single-search.png，所选末行可见圆角和双侧留白。

src/ui.rs::history 为每行增加 relative，仅选中行增加绝对定位的背景 div：左右 4、上下 0、圆角半径 4；删除全宽选中 bg，保留原来的当前历史底色。正文、graph、引用图标位置与行高不变，图标遮罩仍使用实际选中颜色。未新增 PathBuilder、Git 调用或缓存；未把 Tree 的 12px 边距误用到 Log。

Windows release 构建通过。artifacts/log-selection-shape 使用隔离 log-selection-profile 和两条可见历史：focused.png 已查看，圆角及侧边留白可见；inactive.png 和 second-row.png 记录移焦及切换行。pixels.ps1 三种状态的中部颜色断言通过，pixels.json 原图采样显示 x=66/70 与 1673/1678 保留 #1D2336，x=71..1672 为聚焦 #2A4371／失焦 #33353B，对应 150% DPI 下左右 6 个物理像素留白。原生操作仅投递自有 PID 32448，验证后按名称检查关闭，启动 session 正常退出；未操作用户 Rebased、未自动滚动。

限制：本地仍是单选；多选连续行圆角、hover、多个仓库 root column 及其他 DPI 不在本次验收范围。参考图和当前 fixture／窗口尺寸不同，只证明本轮单行几何与状态规则，不能宣称同条件完整像素匹配。此次不重复内存压力，上一条内存验收仅属于 706e0e7f 构建，不能直接归给新 exe。导航通知外观和其他既存 UI 差异仍待处理。

预览 ZIP CRC 通过，包内／release／宿主 exe 字节一致，SHA256 76072a2ac6af9db4e82a4ed261ae8a08b0f90a08930efa17c7f97b130ffe5473；已更新 dist/GitReview-windows-x64-preview.zip。

### F1 首批语言设置与文案接线（2026-10-16）

读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 LanguageAndRegionConfigurable.kt::LanguageAndRegionUi.createContent、LocalizationStateServiceImpl.kt：语言选择持久化、发布更新事件；上游通常提示重启且通过语言插件提供资源。本地为内置中英文资源、即时重绘，默认跟随系统为用户要求，明确不是照搬插件／重启机制。

src/i18n.rs 提供集中静态文案、System／Chinese／English 偏好解析、英文回退与进程内语言状态；src/layout_preferences.rs 在用户配置目录新增独立 language 文件，读取限制 32 字节，写入失败不切换当前语言。启动时读取偏好；src/ui.rs 设置页加入三种选项并调用 refresh_windows，即时切换不刷新仓库。顶部改用已有 settings_dark.svg 齿轮与本地化 tooltip。首批接入欢迎页、设置页、主工具栏、若干历史操作、Diff 设置和查看器 tooltip；提交、作者、引用、路径及原始 Git 输出不翻译。

Windows 通过 GetUserDefaultUILanguage 读取当前用户 UI 语言，不使用键盘语言或区域格式。依据 Microsoft Learn：https://learn.microsoft.com/windows/win32/api/winnls/nf-winnls-getuserdefaultuilanguage 。中文主语言 ID（包含简体和繁体地区）目前均映射内置简体中文；其他／未知 ID 回退英文。非 Windows 开发环境按 LC_ALL／LC_MESSAGES／LANG 的首个非空值解析。没有新运行时资源包、翻译网络请求或仓库数据缓存。

Linux 两项 i18n 测试通过：中文 LANGID 地区变体、英文／未知、显式选择覆盖系统、偏好往返及无翻译条目原文回退。Windows release 构建通过；artifacts/language-settings 默认使用新隔离 profile，原生 window/settings 截图显示系统中文与正常汉字；english.png 确认即时英文，文件值断言 en。重启到 artifacts/language-restart/window.png 已查看，确认仍为英文；随后显式中文和跟随系统操作分别断言 zh-CN/system，截图 chinese/system 已保存，language.json 记录真实宿主 UI LANGID。自有 PID 12404／27692 均按进程名检查关闭，两个启动 session 正常退出，用户 Rebased 未操作。

F1 尚未全部完成：筛选菜单、保留型输入 placeholder、动态计数／提交详情／通知及错误文案仍待迁移；缺少格式参数、偏好损坏／写入失败和已打开 Diff 多实体重绘的完整覆盖。设置在欢迎页的入口、齿轮可访问名称及键盘路径仍需补验。当前不宣称完整中文支持，不勾选 F1 整组；本轮无总体内存复测。后续先补语言覆盖，再按 F2–F4 推进基础操作，像素细化继续暂缓。

预览 ZIP CRC 通过，包内／release／宿主 exe 一致，SHA256 2a62e310b73d9723f26bb73123d1c0c79341c5d52f911288cba94b4ffdb37720，已更新 dist/GitReview-windows-x64-preview.zip。

### F1 筛选菜单与保留输入状态的语言更新（2026-10-16）

继续参考 4aec35256d765c217aa0d662b82ac4e860c396fd，核对 UserFilterPopupComponent.java、StructureFilterPopupComponent.java、DateFilterPopupComponent.java 的 VcsLogBundle 消息调用，沿用已确认的筛选流程，仅迁移显示文案；日期 since/until 是消息而非查询数据。src/i18n.rs 补充 Branch／User／Date／Paths 默认标题、预设、Select、Recent、me、编辑提示和对话框文案，ui/user_menu/paths_menu/date_dialog 接入。日期 caption 翻译提示词但保留日期与时间戳。分支名／路径／作者值仍直接显示原文。

src/ui.rs::set_language 接收 Window，通过 InputState.set_placeholder 原地更新 text/hash 与 branches/tags 两个已创建输入控件，不替换实体或调用 set_value；由此保持当前文本、筛选和输入状态。启动创建输入框也使用同一消息表。树竖条的英文 SVG、树分组、动态计数／详情／错误、日历组件内部文案等尚未全部迁移，不能将本轮称为中文覆盖完成。

Windows release 构建通过；Linux date_filter 两项日历／DST 单元测试和真实 Git 日期组合／分页集成测试通过。artifacts/language-filters 原生中文菜单截图已保存；date.png、paths.png 已查看，日期预设与路径选择文案正常。展开树后输入 review 4 并应用，切换 English 后 english-filtered.png 已查看：仍保留查询文本、唯一 review 4 结果和选中提交，Branches and Tags placeholder 已从中文更新为英文，User／Paths 的保留实体也显示英文。未改变仓库与筛选查询语义；多条件状态、所有弹窗内容及全部键盘路径仍待覆盖。使用隔离 language-filters-profile，自有 PID 8824 按名称检查关闭，启动 session 正常结束；未操作用户 Rebased、未做滚动或总体内存压力。

预览 ZIP CRC、包内／release／宿主 exe 一致，SHA256 9231f4e902643ed4592fcb2ed741283bee7d969ae1a00f7854047768b129b3f8，已更新 dist/GitReview-windows-x64-preview.zip。下一步继续补齐 F1 动态文案、欢迎页入口及错误，然后进入 F2–F4。

### F1 欢迎页设置与动态文案（2026-10-16）

参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 CustomizeTabFactory.kt，确认欢迎页复用 LanguageAndRegionUi 并能进入设置；本地复用同一 settings_panel，不要求先打开仓库。入口为右上齿轮，欢迎页／仓库页均补 accessibility_label，欢迎页返回动作关闭设置。当前欢迎页布局为轻量实现，未复刻整个 JetBrains Customize 页面，像素细化按用户指示暂缓。

src/i18n.rs 新增带类型参数的 Message，覆盖加载提交数与 offset、Diff 文件位置、所属分支数量、仓库 Diff 标题；避免对含仓库字符串的成品文本进行字符串替换。ui.rs、diff_tab.rs、containing_branches.rs 接入，同时翻译加载更多／历史末尾、分支查询加载／空结果、显示全部／收起、Diff 搜索与导航。ContainingBranches 为译文实际测量链接预留宽度，不再固定测量英文 Show all。树仅依据固定分组 key 翻译 Local／Remote／Tags，用户创建的同名引用／目录保持原文。

Linux 三项 i18n 测试通过，新增动态值用例验证带中文、花括号、&／尖括号的文件名逐字保留、英文分支单复数和零条／分页偏移。Windows release 构建通过。artifacts/language-welcome 的 settings/english 原图已查看：未打开仓库时从齿轮进入设置、切换 English、关闭后返回英文欢迎页，配置文件 en 断言通过。artifacts/language-details/window.png 确认中文提交计数和所属分支，diff.png 确认“仓库差异：review.txt”和“1/1 个文件”，文件内容／哈希未变；tree.png 留存分组状态。两组隔离 profile，自有 PID 22968／34120 按名称检查关闭，两个启动 session 均结束；未操作用户 Rebased。

仍未完成全部 F1：错误摘要、导航提示、提交身份／日期连接词、本地变更区域名称、部分空态、分支竖条 SVG 和日历组件文案仍有英文；访问性名称为代码接入，未独立用屏幕阅读器验收；欢迎页 Esc 与打开多个 Diff 后语言切换仍待测试。本轮没有内存压力复测，不以启动样本推断预算完成。保留这些缺口后继续功能增量，不回到像素打磨。

预览 ZIP CRC、包内／release／宿主 exe 一致，SHA256 fe902879ad3c7b1dec5628a048874ffd1cb09ce8a5290408b03345d0d24d3adf，已更新 dist/GitReview-windows-x64-preview.zip。

### F2／F3 Fetch 源码规则与请求模型（2026-10-16）

开始基础 Git 功能开发，F1 剩余翻译继续保留，不以文案收尾阻塞 Git 操作。读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的以下实现，不能再将这些路径只视为未读线索：

- backend/src/actions/GitFetch.java：顶部 Fetch 调用 fetchAllRemotes；无远端或 fetch 正在运行时禁用，后台可取消，结束展示结果。它不是 fetchDefaultRemote。
- backend/src/fetch/GitFetchSupportImpl.kt：default remote 为唯一远端，否则当前分支跟踪远端，否则 origin；多远端且无上述匹配不猜测。fetchAllRemotes 枚举每个远端；按 repository/remote 排队，收集每个远端成功／失败及 pruned refs，成功后 repository.update 和 afterSuccessfulFetch。进度读取 stderr；并发受 registry／SSH／credential store 限制。
- backend/src/fetch/GitRemoteOperationQueueImpl.kt：同 repositoryRoot/remote 串行、等待时检查取消、finally 释放 busy 标记。后续本地实现需要进一步协调 linked worktree 共用 git-dir 的写操作；不直接把此 key 当作全部写操作互斥方案。
- backend/src/fetch/GitFetchSpec.kt、shared/src/git4idea/fetch/GitFetchTagsMode.kt：默认禁用子模块递归；tag 模式对应无额外参数／--prune-tags／--tags／--no-tags，可选 refspec 与 --unshallow。
- backend/src/commands/GitImpl.java::fetch：配置认证 gate/mode/URLs，现代 Git 增加 --progress、--prune；旧版本能力检查是上游适配的一部分。
- tests/git4idea/fetch/GitFetchTest.kt：default／specific／all 三种获取验证远端跟踪 ref，并检查成功通知。本地测试借鉴 specific/all 行为，但没有把 UI 通知当成已覆盖。

src/fetch.rs 新增 Spec／Tags、现代 Git 参数构造及独立 default_remote 选择函数；保持顶部 all-remotes 与默认远端流程分开。参数通过 OsString 列表输出，选项在 -- 前，remote/refspec 为位置参数，拒绝空／NUL／换行输入，默认不主动打开 --tags／--prune-tags／--unshallow。src/git.rs::fetch_specs 只读枚举已配置 remote name，最多 4096；不读取 URL／凭据，不启动网络，不接入现有只读执行器来执行 Fetch。此层是后续网络执行和 UI 的前置模型，尚没有 Fetch 按钮或用户可用的 Fetch 操作。

Linux 两项 fetch 单元测试通过：默认远端优先级／无候选、参数值不能变成开关。真实 Git 集成 rebased_fetch_specs_fetch_all_prune_and_preserve_worktree 通过；临时两个普通仓库作为本地 transport endpoint，验证无远端、all 获取两个远端、specific 只前进 second/main、prune 仅删除 second 的过期引用、--no-tags 不获取 v2、HEAD/index/已暂存与未暂存/未跟踪内容保留，以及枚举前取消。测试未使用用户仓库或外部网络；本轮不声称覆盖 bare remote／认证／网络故障。Windows 原生相同集成测试通过（3.25s），脚本 artifacts/fetch-spec/tests.ps1。

下一步 F2：实现独立有界网络／写操作执行生命周期、认证、进程树取消和操作互斥，再接 Fetch 全部／指定远端、进度、逐远端结果及刷新。现有 Repository::read_stream 使用 30s 读取超时、null stdin、只终止直接子进程，不能未经适配承载 SSH／credential helper 网络操作。还需测试失败远端与部分成功、prune-tags/all-tags、浅克隆、submodule、取消中途和 refs 刷新；Pull／Push 尚未实现，仍须逐项读取上游操作与测试。无本轮 GUI 修改／发布包，当前便携包保持 fe902879ad3c7b1dec5628a048874ffd1cb09ce8a5290408b03345d0d24d3adf。

## F2 进展：操作进程树的所有权与清理（2026-10-16）

对照 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/platform-util-io/src/com/intellij/execution/process/OSProcessHandler.java`：启用递归销毁时同步结束进程树，失败时回退直接销毁。新增 `src/process_tree.rs` 的 `ProcessTree`，供后续独立 Git 操作执行器使用；它尚未接入现有读取或 Fetch UI，也不代表已完整移植 Git 的取消策略。

平台适配：Windows 使用无名、不可继承的专属 Job，设置 kill-on-close；进程挂起创建，加入 Job 后恢复初始线程，避免助手在分组前运行。显式终止和 Drop 均清理所属进程，回收直接子进程。Unix 使用独立进程组。重复终止幂等；创建／加入／恢复失败走资源释放。此处是强制清理底层能力，上游软取消、认证交互和操作状态刷新仍须在执行器层核对；取消不能被描述成回滚。

`tests/process_tree.rs` 使用自身测试程序构造父／子进程和继承管道，无网络、无真实仓库写入。Linux 4 项测试通过；Windows xwin 编译后，在宿主原生运行 4 项通过（0.15s），脚本与结果位于 `artifacts/process-tree/tests.ps1`、`windows-result.txt`。覆盖主动终止与重复终止、父进程已退出后清理存活子进程、创建不存在程序失败；两种清理路径都验证无关进程仍存活，随后仅清理自建测试进程。管道 EOF 验证在 5 秒内完成，无需等待助手的 20 秒自然退出。

限制：尚未验证真实 Git／SSH／credential helper、执行器并发／认证、取消后仓库刷新和网络操作内存；Unix 主动脱离进程组的程序不在本测试保证内；Windows 挂起创建到加入 Job 之间，宿主异常崩溃的极短窗口未用原子 Job 创建属性消除。未修改 GUI、未重新发布便携包、未操作用户 Rebased。下一步接入有界输出及取消生命周期，再完成 Fetch 交互；Pull／Push 仍严格先读取上游逻辑和测试。

## F2 进展：有界操作输出、进度与取消执行器（2026-10-16）

再次对照 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/fetch/GitFetchSupportImpl.kt`：`doFetch` 将 stderr 行用于进度，调用 Git.fetch，成功后更新 repository。新增 `src/operation_process.rs`，把上一节进程树能力接到阻塞后台执行 API；命令、认证、业务结果解析及仓库刷新仍由上层负责，不自行定义 Pull／Push 行为。

本地内存适配：stdout、stderr 各保留最后 64 KiB 原始字节并标记 truncated；进度是可读取的有界 stderr 快照，不使用无界事件队列，不记录命令或输出。两个流并行排空，主进程退出／取消／等待错误时先释放操作进程树，再等待读线程，避免助手持有管道阻塞完成。正常退出码（含失败）、取消、调用方指定超时分别返回；网络操作不继承读取层 30 秒超时。调用前已取消不会启动命令。

Linux 和 Windows 宿主各 4 项测试通过。`tests/operation_process.rs` 让 fixture 向两个流分别写入超过 512 KiB，断言末尾及失败退出码 7 保留、长度不超过 64 KiB、截断标记和进度一致；另验证观察到进度后取消、100ms 显式超时、启动前取消。Windows 脚本与结果位于 `artifacts/operation-process/`，原生运行 0.29s，无网络或真实仓库操作。上轮父／子管道清理测试仍为底层证据，本轮未扩大为真实 Git helper 验收。

尚未接入 Fetch UI、认证、仓库互斥与操作后刷新；stdin 当前为空，不能作为交互认证实现。尾部输出只供诊断／进度，未来 Push 等结构化结果必须流式解析完整结果，不能将截断尾部当作完整结果；显示前还需凭据信息脱敏。读管道异常的即时取消反馈、真实网络失败和长时间认证等待仍待补充。无 GUI 或便携包更新，无新的总体内存结论。下一步继续完成独立 Git 命令构造、认证及操作协调，并接入 Fetch；本节不表示 F2/F3 完成。

## F2/F3 进展：实际 Fetch 接入操作执行器（2026-10-16）

本轮读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitRemoteOperationQueueImpl.kt`（仓库 root＋remote 键、50ms 可取消等待、finally 释放）、`commands/GitImpl.java::fetch`（远端、认证 gate/mode、progress/prune）和 `commands/GitHandlerAuthenticationManager.java::prepare/prepareHttpAuth`（HTTP AskPass、认证回调、credential helper 配置条件）。已确认上游有应用自有认证服务，保留系统助手不能等同完整认证还原。

新增 `Repository::fetch`：读取并 canonicalize `--git-common-dir`，取得操作所有权后复查远端仍配置，使用 Spec 参数和独立 operation_process 执行，返回退出状态／取消原因／有界输出。`src/operation_queue.rs` 采用可取消等待和 RAII 释放；本地适配为按 common Git 目录串行化，以覆盖共享 refs/object store 的 linked worktree，而非上游每 remote 并行。作用域仅当前应用进程，不代替 Git 自身锁，也不能阻止其他应用修改仓库。

提取 operation_command 与读取 command：共享路径／Git 环境隔离；读取保留 GIT_NO_LAZY_FETCH=1 和 GIT_OPTIONAL_LOCKS=0，显式操作不继承这些限制。系统已配置 credential helper 保留，终端输入禁用；应用 HTTP/SSH AskPass、认证排队／取消和错误脱敏仍未接入，所以没有开放 UI 入口。操作返回后刷新真实状态仍由后续上层负责。

原 Fetch 集成用例已从测试直接运行 git fetch 改为调用 Repository::fetch。真实临时仓库验证两个远端获取、指定远端更新、prune、默认 tags／no-tags，以及 HEAD、index tree、已暂存／未暂存／未跟踪内容保留；新增未配置远端拒绝、预取消、无效本地传输返回失败后可继续成功 Fetch（证明所有权释放）。Linux no-default-features 全部通过（52 单元、36 仓库集成、4 operation_process、4 process_tree）；Windows 宿主定向运行真实 Fetch 用例，脚本与结果位于 `artifacts/fetch-executor/`。仍为本地普通仓库传输，非认证／真实网络验收。

限制：队列单元覆盖取消、不同仓库可取得所有权、释放后再次取得；尚未用确定性并发 fixture 验证 linked worktree 等待、跨进程大小写别名及窗口关闭取消。未完成多远端结果聚合、认证、仓库刷新和 UI；没有新便携包或内存验收。后续必须完成这些功能，不能将后端测试通过视为用户可用 Fetch；Pull／Push 仍待按源码逐项移植。

## F3 进展：Fetch 多远端结果聚合（2026-10-16）

读取 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitFetchSupportImpl.kt::fetch/mergeRepoResults/RepoResult/FetchResultImpl` 和 `GitFetchResult.kt`：结果按远端保存，整体成功要求全部成功，成功远端独立进入后续更新处理。`src/fetch.rs::execute` 按此实现逐远端结果，单次失败不跳过后续目标；本地串行执行沿用 common-dir 协调的低内存适配。取消保留既有结果，未开始目标显式 NotStarted，不宣称回滚。

新增 RemoteResult／BatchResult，区分成功、失败、取消和未开始。诊断只保留失败项尾部，每项最多 8 KiB、整批最多 64 KiB，并记录截断；成功项丢弃诊断输出。结果仍是原始字节，显示前必须脱敏；此实现尚不提供上游 prune 引用清单、认证消息或通知布局，不将诊断尾部用于完整结构化解析。

`tests/repository.rs::fetch_batch_retains_partial_success_and_continues_after_failure` 在临时真实仓库构造“失败→成功→失败”，验证全部结果、整体失败、中间远端引用实际获取及诊断有界；源仓库再次前进后预取消批次，验证全部目标未开始、consumer 引用不变。Linux 定向通过；Windows 宿主定向通过，证据 `artifacts/fetch-batch/tests.ps1`／`windows-result.txt`。没有网络认证、UI、在途批次取消或总体内存验收；取消已完成部分的确定性用例仍待补。下一步仍为认证服务及操作 UI／刷新接线，Pull／Push 尚未实现。

## F2 进展：认证请求与操作取消的桥接（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitAskPassApp.java`、`GitHttpAuthenticator.java`、`GitHttpAuthService.kt`、`GitHttpGuiAuthenticator.java`（askUsername/askPassword/acquireData/provider 及保存／遗忘路径）、`GitHandlerAuthenticationManager.java::prepareNativeSshAuth` 和 `tests/git4idea/commands/GitHttpGuiAuthenticatorTest.kt`。上游 HTTP helper 分别请求用户名／密码，按操作 handler 路由；GUI authenticator 复用一次获取的凭据，取消后密码请求不再弹窗。测试覆盖正确才保存、错误不保存／遗忘旧密码、不记住密码、取消传播及只显示一次对话框。SSH 上游另有服务并设置 SSH_ASKPASS_REQUIRE=force 与 DISPLAY fallback；SSH 提示解析类尚未阅读，不据此声称已实现 SSH 认证。

新增 `src/auth_request.rs`，提供操作内 helper worker→UI 的有界请求通道。一个会话同时只有一个待答请求，其余请求可取消地等待 gate；UI 消费 Pending 后提交答案，丢弃 Pending 视为取消，并传播至整个操作 Cancel。晚到回复不能重新启用取消的操作。提示／回复上限 64 KiB，回复拒绝 NUL／CR／LF，类型不实现 Debug，不写日志、不持久化凭据。此层仅负责请求所有权和取消，尚未实现跨进程 IPC、HTTP URL 提取及用户名／密码单弹窗复用，也未接认证 UI 或实际 Git 命令。

Linux 4 项单元测试通过；Windows 原生相同 4 项通过（0.06s），证据 `artifacts/auth-request/`。覆盖不同操作回复隔离、关闭提示时取消当前和等待请求、不再次弹出、取消后迟到回复拒绝及非法协议行输入。对应上游本轮仅实现取消传播基础；上游凭据成功保存／失败遗忘／单弹窗复用测试仍是待办，不以当前通道测试替代认证验收。下一步接实际 AskPass helper IPC 与 HTTP 会话，再连接 UI；无 GUI 发布包或内存复测。

## F2 进展：原生 AskPass helper 与本机通信（2026-10-16）

继续以已读 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitAskPassApp.java`／`GitHttpAuthService.kt` 为依据：独立程序接收 Git 提示，通过操作 handler 调用 UI，答案写 stdout；取消无答案，通信错误非零退出。新增不依赖 desktop feature 的 `gitui-askpass` binary（`src/bin/askpass.rs`）与 `src/askpass.rs`。本地适配采用每操作独立 IPv4 loopback listener 和 OS 随机 256-bit capability，路由信息仅通过子进程环境传递，不使用凭据临时文件；这不是复制 IntelliJ REST 服务本身。

协议限制提示／回复各 64 KiB，长度前缀，校验会话令牌后才产生 UI 请求；握手有 3 秒期限，读轮询可取消，写有超时，单连接顺序处理避免无界线程增长。Server Drop 停止服务并 join，等待中的请求解除；helper 仅输出答案或固定诊断，不打印令牌／提示／原始错误。HTTP username 分类按上游大小写无关前缀，其余按密码；SSH mode 只预留显式分发，尚未接实际 SSH_ASKPASS 配置或解析。依赖增加已缓存的 getrandom 0.3.4。

Linux 与 Windows 宿主各 4 项集成测试通过。覆盖真实独立 helper 的中文回复、关闭提示取消、错误会话拒绝、Server Drop 释放等待进程；真实 `git credential fill` 调用 helper 分别获取虚构用户名／密码并检查 Git 返回字段。此命令关闭 credential helper，不调用 approve、不联网、不保存凭据。Windows 脚本和结果见 `artifacts/askpass/`（0.18s），使用宿主 Git。没有真实远端凭据参与测试。

仍待：HTTP 单弹窗凭据复用／URL 与身份显示、SSH host-key/passphrase/密码交互、凭据存储策略、真实 Git fetch 认证失败／取消及界面接线。当前 Fetch 未配置本 helper；便携包脚本尚未包含新 exe，发布前必须补齐并验证路径含空格的 Git／SSH 调用。协议尚未覆盖恶意连接持续占用、部分消息／超长消息的原生测试；不将本轮 IPC 测试视为完整认证或安全审计。无 GUI 发布或总体内存复测。

## F2/F3 进展：Fetch HTTP 认证调用接线（2026-10-16）

沿用已读 Rebased GitImpl.fetch→GitHandlerAuthenticationManager→GitHttpAuthService/GitAskPassApp 调用路径，增加 `Repository::fetch_authenticated`，在取得 common-dir 操作所有权、重新确认远端后创建操作专属 AskPass Server，配置 GIT_ASKPASS 和会话环境，再运行 Fetch；结束后关闭并 join 服务。helper 必须为存在的绝对文件路径。原 fetch API 保持原行为；多远端 execute 尚未改为应用认证版本，UI 尚未接线。系统 credential helper 配置仍保留，HTTP session 复用和保存策略仍须实现。

新增 `tests/fetch_auth.rs`：临时本机 HTTP Basic 401 服务＋真实 git fetch＋真实 helper，所有凭据为固定 fixture，服务仅记录是否出现 Authorization（不保存／打印内容），不连接外网。拒绝场景验证用户名和密码请求各一次、确实发送认证请求、最终失败而非取消、没有 refs 写入；取消场景在用户名提示关闭，验证 token 取消、未发送授权请求、不再请求密码、返回 Cancelled。helper 位于含空格目录且文件名也有空格，验证其真实启动路径。

同时修正 operation_process 的取消竞态：AskPass 关闭提示后，Git 可能先于轮询检查退出；收尾再次检查 token，保留取消语义。Linux 两项 HTTP Fetch 测试＋四项执行器回归通过；Windows 原生相同测试通过（HTTP 1.09s，执行器 0.21s），记录 `artifacts/fetch-auth/{tests.ps1,windows-result.txt,process-result.txt}`。没有使用用户真实仓库／远端或凭据。

待完成：成功认证后完整 Fetch、HTTP 单弹窗复用与凭据处理、SSH 认证及 host-key 确认、UI 提示／进度／刷新、多远端认证会话和错误脱敏。这轮只证明真实 Fetch 的拒绝与取消链路，不宣称用户可用认证或全部网络行为完成；发布包未更新，helper 尚待打包，无总体内存复测。

## F2/F3 验证：HTTP 认证成功后完整获取提交（2026-10-16）

扩展 `tests/fetch_auth.rs` 的本机 HTTP fixture：仅在精确匹配虚构 Basic 凭据后提供临时源仓库的 dumb HTTP 文件，否则返回 401。源仓库包含真实提交，执行 update-server-info；测试运行现有 Repository::fetch_authenticated，通过独立 AskPass helper 回答用户名／密码，取回完整 Git 对象与 remote refs。

成功用例断言退出成功且未取消、认证请求确实发生、origin/main 与源 HEAD 完全一致、git show 得到预期 blob 内容；消费者工作区没有出现远端文件，HEAD 仍为 unborn。拒绝和取消用例继续通过并断言无 refs。HTTP fixture 仅服务其临时 Git 目录，拒绝路径中的父目录／反斜杠／冒号／百分号，不打印请求或凭据。

Linux 三项 HTTP Fetch 集成测试通过（0.26s）；Windows 原生同组三项执行结果见 `artifacts/fetch-auth-success/tests.ps1`、`windows-result.txt`。继续验证含空格 helper 文件／目录。证据限于本机 Basic＋dumb HTTP：没有验证 HTTPS 证书、代理、托管平台 token/OAuth、smart HTTP、SSH 或真实网络；没有使用用户凭据或远端。本轮不修改产品 UI 或认证策略，不更新发布包。下一步仍为 HTTP 单弹窗凭据复用与 UI 接线，F2/F3 未完成。

## F2 进展：HTTP 单次登录的凭据复用（2026-10-16）

再次核对 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitHttpGuiAuthenticator.askUsername/askPassword/splitToUsernameAndUrl`：用户名请求获取整组凭据，随后密码请求复用既有数据；对应上游 `GitHttpGuiAuthenticatorTest.test single dialog shown`。新增 `Pending::answer_credentials`，UI 将来可一次提交用户名／密码，Requests 对后续密码请求直接返回本次密码，避免第二个弹窗。

本地差异：复用前用 url 2.5.8 解析 Git 提示中的目标，去除 userinfo 后要求完整目标与编码后的用户名匹配；协议、主机、路径或用户名不同必须重新请求，不跨目标复用。无法解析的提示不允许保存成可复用登录。重复用户名请求先清掉旧凭据，关闭提示清除缓存并取消；AskPass Server 结束时清掉缓存，即使 UI 仍保留 Requests 句柄也不持有上次密码。没有实现 PasswordSafe／记住密码，当前仅操作内内存缓存；释放 String 不代表经过内存清零审计。

Linux 6 项 auth_request 单元、4 项 helper 集成、3 项 HTTP Fetch 集成通过。成功 Fetch 用例改为在用户名请求一次提交两字段，断言只有一个应用请求，Git 密码请求由缓存回答，远端 refs／对象仍成功取回；拒绝／取消回归保留。目标变化和显式清理后重新请求由单元测试覆盖。Windows 宿主相同三组测试见 `artifacts/http-login-session/`，使用实际 Git 与原生 helper。

仍未实现可见登录弹窗、已知用户名的密码表单、目标地址的本地化／脱敏显示、持久凭据成功保存与失败遗忘、SSH、多远端认证整合和发布包。此处只实现单弹窗所需的后端能力，不称 UI 已完成。新增 URL 依赖后尚未测 helper 或总体内存／体积，后续打包时一起验收。

## F2/F3 进展：Fetch 界面与中英文 HTTP 登录（2026-10-16）

读取 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/commands/GitHttpLoginDialog.kt`：用户名／密码表单、密码遮罩、非空校验、用户名为空时聚焦用户名、Login／Cancel、可选 credential helper 入口与记住密码。新增 `src/auth_dialog.rs` 的 GPUI 表单：一次提交两字段、密码 input content type＋遮罩、空字段校验、取消／关闭传播、首字段聚焦、中英文文案；标题去掉 URL userinfo、query、fragment，避免直接显示原始认证提示。已知用户名路径目前只显示密码框；尚未还原用户名只读显示、remember password、provider/helper 选择及完整键盘体验，不提供无行为选项。

Review 新增独立 Fetch token/task，与浏览读取任务分离。顶部 Fetch 获取全部配置远端，运行中禁重复点击／切仓入口，提供取消；应用销毁取消任务。每远端调用 execute_authenticated，后台请求通过 50ms UI 轮询打开登录表单，结果更新为完成／取消／失败远端。结束后强制刷新 Log/本地状态及作者目录，绕过普通刷新在 Diff Tab 或加载期间的早退；取消不当作回滚。认证弹窗结束仅关闭仍活动的本轮表单，避免关闭用户随后打开的其他弹窗。部分失败仍保留成功远端结果，但 UI 目前只展示失败远端名称，详细错误与逐远端进度仍待完善。

Windows 原生 artifacts/fetch-ui：专用 HTTP 401 服务、独立 profile／仓库；login 原图确认英文表单，empty 校验，masked-direct 确认密码遮罩，failure 确认一次提交后失败且没有第二个密码弹窗。最初 SendKeys 被宿主中文 IME 合并输入，不作为键盘 Tab 通过证据；改用目标 HWND 的 WM_CHAR，masked.png 黑图由 masked-direct 替代。随后补密码内容类型及强制刷新，以最终构建再次启动。artifacts/fetch-ui-final：中文 login、cancelled、refreshed 原图确认表单、取消反馈及本地远端 Fetch 后的新提交／origin/main／review.txt。assertions.json 比较源 HEAD 与远端 ref，并断言未检出工作区文件；success.png 拍于异步刷新完成前，refreshed.png 才是 Log 更新证据。

Linux no-default-features 全部通过：58 单元、37 repository、4 askpass、3 fetch_auth、4 operation_process、4 process_tree。Windows release 构建通过，原生 fixture 使用宿主 Git；本轮只停止自有 GUI 31612／7988 与记录 PID 的专用 HTTP 服务，两次启动 session 正常退出，未操作用户 Rebased。未自动滚动、未将启动内存采样称为性能验收。

更新 README、Windows 使用说明与界面“只读”文案，表达浏览只读＋显式 Fetch，Pull／Push／checkout 未开放。打包脚本增加认证 helper 及 manifest hash。已发布本地 `dist/GitReview-windows-x64-preview.zip`（保留版本名 fetch-preview.zip），ZIP CRC 通过，包内／release／已原生验证的宿主文件逐字节及 SHA256 一致：主程序 `dab22307c9c329f0849ff799f14a3132fa9dda0e25ff0aecb2bd1148a0e9462e`，helper `d0dc70e4d588adb8c46b03d218f67f53c770f7288b8130daa0162a7516c1c081`。helper 149,504 bytes；没有新总体内存结论。

限制与下一步：SSH 应用认证未接，当前仍依赖宿主已有配置；HTTP 可见成功登录后 Fetch 的完整端到端（已有后端 HTTP 成功＋原生 HTTP 失败／取消＋原生本地 Fetch 成功）、多远端 UI、在途关闭／切仓竞态、详细错误脱敏、进度、无远端按钮状态、凭据保存策略、可访问性与键盘仍待补。当前状态文字在语言切换后不会重新格式化，后续改 typed message。本轮是可手测 Fetch 增量，不是 F2/F3 全面完成；继续源码级移植 Pull／Push／checkout，像素细化仍暂缓。

## F3 进展：Pull 源码分流与选项模型（2026-10-16）

完整读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `actions/GitPull.java`、`pull/GitPullOption.kt`、`config/GitPullSettings.java`；读取 `GitPullDialog.kt` 的目标默认／校验／选项启用和 `GitMergeAction.java` 的 perform/handleResult。关键规则：显式 REBASE 走 GitUpdateExecutionProcess.launchUpdate(REBASE)，不是直接执行 pull --rebase；未选 REBASE 走 GitMergeAction 的命令／冲突与本地覆盖检测。初始选项为空，普通命令为 pull --no-stat + 已选选项 + -v --progress + remote + remote-operation branch，不强行设置 ff-only 或 no-rebase，保留 Git 配置作用。pull.rebase=interactive 还有专门 editor 接线，不能忽略。

新增 `src/pull.rs` 翻译六个选项及兼容性、初始空选项、commitAfterMerge=不含 NO_COMMIT，以及两种 Dispatch。RebaseUpdate 仅描述目标，明确要求后续更新流水线；不提供执行入口。普通命令加入 `--` 隔离参数值，是本地命令构造适配，执行前仍须验证配置远端和真实远端分支。NoVerify 与 Rebase 可共存，但上游 rebase 分流未把这些原始选项传进更新流水线，本地按同一分流保留，不能自行补命令。Linux 3 项测试通过：36 个选项配对矩阵、默认命令／rebase 路由／commitAfterMerge、参数边界。未执行 Pull 或修改用户仓库。

目标选择已读规则：远端优先当前跟踪远端，否则 GitUtil.getDefaultOrFirstRemote（该函数尚待沿读）；分支保留有效旧选择，否则当前跟踪分支的 remote-operation name，否则当前同名分支，再否则空，最终校验存在。不用 Fetch 默认远端函数替代 Pull 默认选择。

读取 `GitUpdateMethodTest.kt` 全部用例：无配置 MERGE，branch.<name>.rebase 优先 pull.rebase，true/interactive/preserve 对应 REBASE，分支 false 覆盖全局 true。这是 updater 配置解析规则，不能与 PullDialog 初始空选项混为一谈。读取 `GitSingleRepoUpdateTest.kt` 前四个 rebase stash 用例：有本地提交＋脏工作区需保存／恢复；无本地提交可 fast-forward 优化，不无条件 stash；干净工作区不 stash；AD（已暂存新增后工作区删除）需特殊覆盖。其余 update 测试目前只检索名称，不算完整阅读。

下一步继续读 GitUpdateExecutionProcess/GitUpdateProcess/GitUpdater 与 merge 检测器及对应测试，建立 dirty/index/untracked/冲突后续处理，再接实际 Pull。当前无 Pull UI／执行、无新发布包，最新可用包仍为 dab22307… 的 Fetch 增量；F3 未完成。

## F3 进展：更新策略配置解析（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `update/GitUpdater.java::resolveUpdateMethod/isRebaseValue` 与 `config/GitConfigUtil.java::getBooleanValue`，新增 `src/update_method.rs` 逐项翻译：branch.<当前分支>.rebase 为 true/yes/on/1/interactive/preserve 时 Rebase，false/no/off/0/空串时 Merge；未知分支值回退 pull.rebase，后者未识别则 Merge。大小写不敏感；不擅自补 Git 接受但此上游函数不识别的 merges 值。此函数解析 updater 默认策略，不替代 PullDialog 选项和 Git pull 原生配置行为。

新增 Repository::default_update_method，读取真实当前分支及两个配置；配置读取异常按上游回退，取消显式返回 Cancelled。Linux 两项配置表单元测试通过，覆盖上一节 GitUpdateMethodTest 用例与布尔别名／未知值；真实仓库用例验证 main、带斜杠分支和 detached HEAD 配置优先级、取消。Windows 宿主真实仓库用例通过，记录 `artifacts/update-method/`。未启动 Pull 或改动用户仓库，发布包保持上轮 Fetch 版。

另完整读取 GitRebaseUpdater.java，并读取 GitUpdateProcess.tryFastForwardMergeForRebaseUpdaters 和调用上下文：有本地 changes 时先实际尝试 merge --ff-only，成功则移出后续更新流程，失败交回常规保存／更新流程；不是仅凭有无本地提交决定是否跳过保存。isSaveNeeded 还检查 index（读取异常时保守要求保存），覆盖 AD 场景。真正 rebase 前检查是否重写已发布提交并询问，GitRebaser 返回 CANCEL 时调用 abort；这些执行／用户交互尚未移植，不能用简单 --autostash 或一条 pull --rebase 代替。

下一步继续读取保存恢复及 GitRebaser 冲突／取消状态机与测试，再实现实际操作。更新器没有执行入口，Pull／Push／checkout 仍未开放；本轮无 GUI／打包改动或新内存验收。

## F3 进展：读取合并／变基与冲突状态（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `repo/GitRepositoryReader.kt::readRepositoryState/findRebaseBranch` 及 `GitRepositoryReaderNewTest` 中“current branch is known during rebase”“rebase with conflicts while being on detached HEAD”两个完整用例。上游先检查 MERGE_HEAD，再检查 rebase-apply/rebase-merge；变基时从 head-name 找回原分支，detached HEAD 场景不伪造分支。

新增 `src/operation_state.rs` 和 `Repository::operation_state`：使用 Git 返回的绝对 per-worktree gitdir 读取操作标记；合并、两种变基、cherry-pick、revert、sequencer 分别表示，detached 单独记录。读取有界 head-name 获取原变基分支；用 ls-files --unmerged -z 获取独立的未解决路径，按路径去重，保留空格等原始路径。上游 sequencer todo 进一步分类尚未翻译，暂保留 Sequencer 标记；detached 与操作并列是本地表示适配，不在 detached 状态下隐藏其他操作标记。权限／读取错误不能当作无操作，取消传播。

真实 Git 集成在临时 linked worktree 制造 merge 冲突，验证操作类型及单一路径；解决并暂存后 conflicts 为空但仍 Merge，abort 后清除。再制造 rebase --merge 冲突，验证 RebaseMerge、detached=true、原分支 feature、冲突路径，并确认主 worktree 始终不被误标；abort 后恢复，预取消返回错误。Linux 通过（0.32s）；Windows 宿主同用例通过，证据 `artifacts/operation-state/`。实际写操作均为测试 fixture，产品读取 API 不执行合并／变基／abort。

继续读取 `GitRebaser.java` 的 rebase、abort、continue 和 handleRebaseContinueFailure：冲突进入 resolver；no-change 场景根据 index／工作区状态决定 continue、stage tracked 或 skip，不能仅依据错误文本自动跳过；ProcessCanceledException 返回 CANCEL。相关执行流程仍未移植，本轮只提供后续处理所需的真实状态。尚未验证 rebase-apply、detached 起始变基、sequencer 分类和状态读取中外部仓库变动；这不是原子快照。Pull 执行／UI、用户继续／中止路径仍待实现，无发布包更新。

## F3 进展：Pull 目标读取与默认选择（2026-10-16）

再次确认用户约束：Pull／Push 按 Rebased 源码及对应测试翻译；尚未核实的执行分支保持待办，不自行替换成通用 Git 流程。AGENTS.md 的“Git 写操作源码依据”和本计划 F3 已将此列为持续要求。

参考版本仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`。沿已读 `GitPullDialog.kt`，补读 `GitUtil.java::getDefaultRemote/getDefaultOrFirstRemote/parseRemoteBranch`：默认远端为跟踪远端、origin、首个配置远端；分支先保留所选远端上的有效旧值，再取跟踪分支名，再取存在的当前同名分支，否则为空。切换远端后跟踪分支名可能无效，应保留草稿交由存在性校验，不静默换目标。远端引用按斜杠从前到后匹配首个配置远端前缀，不取最长前缀。

新增 `pull::Targets` 和 `Repository::pull_targets`，读取配置远端、非符号远端引用、真实当前分支与存在的 upstream。排除 origin/HEAD 和无配置远端的孤立引用；后者为配置远端选择器的本地适配。首个远端使用 git remote 返回顺序，尚未证明与上游集合所有情况下排序相同。读取保持本地只读，取消传播；不增加 Pull 执行入口。

真实 Git 集成用例覆盖无远端、origin 优先、跟踪远端优先、有效旧选择、切换后无效草稿、当前同名分支、detached、符号引用排除、重叠远端名及预取消。现代 Git 拒绝通过 remote add 创建重叠远端名，fixture 改用显式配置复现已有配置场景。Linux 用例通过；Windows 原生结果记录于 `artifacts/pull-targets/`。本轮仅完成目标模型，Pull 执行／UI 和 Push 仍待实现，不更新发布包或宣称功能已完成。

## F2/F3 进展：过程输出事件与保存恢复源码核对（2026-10-16）

参考仍为 `4aec35256d765c217aa0d662b82ac4e860c396fd`。完整读取 `stash/GitStashChangesSaver.java`、`stash/GitStashUtils.kt`，以及 `commands/GitImpl.java::stashSave`：保存使用 stash push -m，不默认包含未跟踪文件；记录 stash@{0} 身份。恢复使用 pop，是否加 --index 取决于 staging area 可用性；index 冲突优先单独报错，内容冲突交 resolver，本地／未跟踪覆盖分别处理。恢复失败不能视为已经恢复；本地 staging 设置与上游映射、恢复交互和执行尚未实现。

读取 `GitSimpleEventDetector.java`、`GitLocalChangesWouldBeOverwrittenDetector.java`、`GitUntrackedFilesOverwrittenByOperationDetector.java`。新增 `src/operation_events.rs` 翻译其中六类简单事件的大小写无关短语判断，保留来源和许可证。operation_process 在 stdout/stderr 排空过程中检测，最终合并事件位；不依赖末尾 64 KiB 日志，额外状态有界，不保存完整行。CR/LF 清除匹配后缀，跨 pipe read 的短语仍可识别，EOF 不要求换行。事件只是证据，UnstashConflict 的广义 conflict 短语只应由 unstash 调用者解释，不能据此直接判定 Pull 结果或自动恢复；最终仍需结合命令退出码、取消与真实冲突状态。本地／未跟踪覆盖的文件列表解析尚未移植。

测试：Linux no-default-features 全套通过（64 单元、40 repository 及现有辅助集成）；随后新增真实仓库用例也通过。子进程 fixture 在两路各输出 512 KiB 后，末尾日志已无初始错误，但合并／index 冲突事件仍保留，取消与超时回归通过。真实 Git 用例制造 merge 冲突、abort、已最新和 stash pop 冲突，验证退出状态、index 未合并路径、stash ref 仍在且保存内容完整。Windows 原生执行器四项及真实仓库用例记录于 `artifacts/operation-events/`；脚本固定本轮构建的测试 exe，避免旧测试产物混用。

已读上游 `GitSimpleEventDetectorTest.kt` 实际测试的是 cherry-pick/revert 的 LocalChangesConflictDetector，不能当作本轮六类事件的上游覆盖；`GitStashTest.kt` 只验证 stash 列表元数据，也不是恢复冲突测试。当前真实冲突／输出截断用例为本地补充验证；后续继续沿 update 测试核对保存恢复及失败状态机。本轮不新增产品写操作入口、不更新发布包，Pull／Push 仍未完成。

## F3 进展：更新会话内的保存／恢复（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的完整 `util/GitPreservingProcess.kt`、`stash/GitChangesSaver.java`、`update/GitUpdateResult.java` 及 `GitUpdateProcess.java` 更新循环／autoLoadDecision。注意旧方法注释称错误后恢复，但实际 predicate 要求非 incomplete、结果非 null 且 isSuccess；因此 CANCEL／ERROR／NOT_READY 不自动恢复，INCOMPLETE 虽在 isSuccess 中也被单独排除。以执行代码为准。保存与恢复在 non-cancellable section，save 失败不运行更新；load 由原子标志保证只尝试一次，包括冲突／失败时。

新增 `src/update_session.rs`：Session 持有 common Git dir 操作队列所有权，保存成功／是否生成 stash／是否尝试恢复分开记录；显式 save 使用 stash push -m，开始前响应取消，开始后使用独立 token 完成不可取消区。成功、冲突已解决、无需更新允许 finish 恢复，其他结果保持 stash；pop 可按调用者 staging policy 加 --index。恢复只尝试一次，Drop 不执行命令。保存前后读取 refs/stash 身份区分干净树和实际保存，也在命令失败后尝试读取可恢复 stash；这替代上游英文 no-local-changes 输出匹配。读取失败显式报错，不能继续当作保存成功。

本地适配：自动 pop 前要求 stash 顶部仍等于本会话记录的对象，否则停止并保留修改，避免其他程序新增 stash 后弹出错误项。这不是跨进程原子锁，检查和 pop 之间仍可能受外部修改影响。会话期间不能调用会再次获取相同队列锁的 Repository::fetch；后续更新执行应在会话内部接非重入命令，Fetch 在进入保存流程前完成。当前只具备保存／恢复后端，没有接 Pull 命令、冲突 resolver、显示保存内容或持久恢复会话。程序退出仍保留真实 Git stash，但没有应用恢复入口。

真实 Git 集成覆盖七种更新结果的恢复条件、工作区和 index 不同内容用 --index 恢复、未跟踪文件不被保存、恢复仅一次、内容冲突后原 stash 完整保留、外部新增 stash 时停止、干净树无 stash、预取消及 index.lock 导致保存失败。Linux no-default-features 全套通过，日志 `artifacts/update-session/linux-result.txt`；Windows 原生三项集成记录于同目录 tests.ps1／windows-result.txt。现有 Fetch 共用锁提取后回归通过。上游 GitSingleRepoUpdateTest 的 AD 场景仍待接 updater 后验证；当前没有替代 stash/shelve 设置、快进优化、已发布提交确认或变基取消/abort 状态机。Pull／Push 产品入口和新发布包仍待后续完成。

## F3 进展：Rebase 更新前的快进优化（2026-10-16）

再次核对参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitRebaseUpdater.java::fastForwardMerge/isSaveNeeded/doUpdate`、`GitUpdateProcess.java::tryFastForwardMergeForRebaseUpdaters`，读取 `GitImpl.java::merge` 的实际参数构造。上游仅对有 local changes 的 RebaseUpdater 先运行 merge --ff-only；成功移出后续保存／更新集合，失败交常规路径。markStart 失败不执行，markEnd 失败只提示收集信息失败，不能重复执行已完成的 merge。这不是把整个 Pull 限制为 ff-only。

`update_session::Session::try_fast_forward` 在现有 common-dir 锁内执行快进，必须在保存之前；重新确认 attached branch、配置远端目标存在、无进行中的操作或未解决 index 冲突。记录执行前 HEAD 和执行后 HEAD，后者用独立读取 token，即使取消也尝试观察实际结果；收尾读取错误单独保留，不覆盖原命令退出状态。目标使用完整 refs/remotes 名与参数分隔符，避免与同名本地分支歧义，是本地参数适配。调用方仍须按上游先判断有无本地 changes，并把 cancelled 与普通失败区分；当前方法不自动选择常规 rebase，更不绕过已发布提交确认。

真实 Git 用例覆盖有暂存＋未暂存差异的无关本地文件随远端快进保留、分叉无法快进、目标覆盖本地文件无法快进。逐项断言 HEAD、index blob、工作区内容及 stash 列表；两个失败分支再使用同一会话保存成功，证明可进入后续保存流程。Linux 通过（0.70s）；Windows 原生证据 `artifacts/update-fast-forward/`。没有连接真实远端，只用临时仓库和本地 remote ref。

仍待：updater 对 changes/index（含 AD）与需要更新判断的完整编排、普通 Pull 命令路径、已发布提交确认、rebase 执行及冲突/取消后的继续或 abort、恢复结果 UI。当前快进是更新后端的一环，不开放独立合并入口，不更新可用包或宣称 Pull 完成。

## F3 进展：变基前已发布提交检测（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitRebaseProcess.java::isRebasingPublishedCommit/askIfShouldRebasePublishedCommit`、`GitRebaseUtils.java::getCommitsRangeToRebase`、完整 `GitProtectedBranches.kt` 和 `config/GitSharedSettings.java`。候选范围为 base..rebasingBranch；“已发布”实际要求候选提交被受保护远端分支包含，不是任意远端引用。默认保护 master/main，使用不含远端前缀的分支名；自定义规则是完整匹配 Java 正则，另可合并 provider 规则。查询异常时上游保守要求确认；确认框默认选择取消。未找到此判断对应的直接上游测试，不能将整个 RebaseTest 文件视为已覆盖证据。

新增 `src/published_commits.rs`：默认保护名称判断、Repository::published_rebase_target 接收已选保护分支集合并返回命中的目标。要求 attached HEAD 和真实配置远端引用存在，查询保留读取层禁止隐式 fetch 的环境；错误／超时／取消为 Err，不返回“无已发布提交”。后续 UI 必须区分错误和确实无需确认，取消不能再弹确认继续执行。保护规则设置和 Java 正则兼容尚未实现，API 接收最终分支集合，没有冒充支持自定义规则。

低内存适配：对 HEAD 与每个受保护分支运行 merge-base --all。两者共同可达提交等于所有最佳共同祖先的祖先集合；若任一共同祖先不被更新目标包含，则存在 base..HEAD 中的已发布提交。用有界 merge-base 输出及 rev-list --max-count=1 判断，避免上游先收集完整候选列表再逐提交查询；输出截断视为未知。无共同祖先的 exit 1 正常返回无命中，其他失败不吞掉。HEAD／目标分别解析对象后查询，外部 refs 在解析期间变化仍不是原子快照，最终执行前还需复核。

Linux 两项真实 Git 用例通过（0.57s）：普通 topic 不因发布就命中、显式保护 topic 后命中、master 包含本地提交、更新 base 已包含候选时无命中、无共同祖先、缺失目标、预取消；交叉合并历史断言存在两个 merge base 并验证两者均参与判断。Windows 原生结果见 `artifacts/published-commits/`。本轮没有执行 rebase 或连接真实网络；确认 UI、设置持久化与 Pull 编排仍待完成，无发布包更新。

## F3 进展：会话内变基启动、继续与中止（2026-10-16）

沿参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 再读 `GitRebaser.java` 的 start/continue/abort、handleRebaseFailure、handleRebaseContinueFailure，以及 `GitRebaseUtils.createRebaseEditor`、`GitImpl.REBASE_CONFIG_PARAMS`、`GitUtil.COMMENT_CHAR` 和完整 `GitRebaseProblemDetector.java`。实际 commentChar 是 U+0001，不是 #。普通非交互变基仍接应用 editor；continue 取消返回失败，不自动 abort；初始 updater 取消才调用 abort。no-change 分支还根据 index/工作区决定 continue、stage tracked 后 continue 或 skip，不能仅看错误文本跳过。

`update_session::Session` 增加低层 rebase/continue_rebase/abort_rebase，沿用会话锁。启动前复查 attached branch、目标存在、无进行中操作/未解决冲突，保存失败不得启动；命令使用上游 commentChar 和明确的应用 editor 参数。测试使用 fixture-only true editor，产品不默认跳过编辑器。启动取消后读取真实状态，仍处于变基时以新 token 执行 abort，分别保留原结果、abort 结果和收尾状态；继续取消保留当前变基。中止只允许由该会话启动过的变基，不自动恢复 stash。当前会话标记不是跨进程所有权证明；外部程序同时改写操作状态的竞争仍待加强。

真实仓库用例走完整保存→提交重放→恢复，断言新父提交、工作区恢复和 stash 清除；冲突路径断言 index 冲突、未解决前拒绝继续、用户在 fixture 中解决并暂存后继续成功，随后恢复；另一路 abort 回原 HEAD 并保留原 stash。post-rewrite hook 暂停 fixture，取消真实 Git 进程树，验证自动 abort 后原 HEAD 和 stash 内容。Linux no-default-features 全套通过（当时 48 repository），随后新增取消用例单独通过（0.33s）。Windows 原生筛选 rebase 相关集成，证据 `artifacts/update-rebase/`，没有改用户仓库或真实远端。

限制：这些是后端命令步骤，不是完整 Pull。公开前还需应用编辑器、发布提交确认、上下文/引用复核、普通 Pull 路径、no-change 恢复策略、覆盖文件诊断、UI 冲突/继续/中止及统一结果刷新；没有把命令成功等同于 UI 完成。没有新增自动 stage 或 skip；无新发布包和内存验收。

## F3 进展：普通 Pull 命令及 HTTP 认证（2026-10-16）

再读参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `GitPull.java::perform/performRebase/shouldSetupRebaseEditor/getHandlerProvider` 和 `GitMergeAction.java::perform/handleResult`。未显式选择 REBASE 走普通 pull，保留 Git 配置；pull.rebase 恰为 interactive 时接 rebase editor 和 commentChar。冲突解决后是否 commit 取决于 commitAfterMerge；no-commit 成功可以保留 MERGE_HEAD，不能只按退出码把仓库标记为无操作。

`pull::execute_command` 增加普通 Pull 后端：使用既有 dispatch 选项，拒绝把显式 REBASE 误走此路径；持有 common-dir 操作锁，复查配置目标及进行中状态，配置应用提供的 commit editor；interactive 缺 sequence editor 时在网络/写入前返回错误。可接已有 AskPass Server/Requests，helper 验证和操作内认证路由沿 Fetch 实现。命令结束后即使取消或执行器报错，也用独立 token 读取真实操作状态，命令结果和状态读取错误分开保留。无自动 stash、强制 ff-only 或失败后自动回滚；产品 UI 未接。

Linux 真实 Git 集成九种场景通过：快进、分叉合并、no-ff+no-commit、合并冲突、未跟踪覆盖、ff-only 拒绝分叉、Git 配置驱动 rebase、squash、interactive 缺编辑器。逐项断言 HEAD/父提交/index/冲突与工作区；合并失败时仍断言远端引用已获取到新值。Windows 同一用例通过（23.70s），见 `artifacts/pull-command/windows-result.txt`。

扩展本机 Basic HTTP fixture，共六项 Fetch/Pull 认证测试：Pull 成功取得新提交并更新工作区，拒绝/关闭认证提示取消时 HEAD 与本地 remote ref 保持原值。用户名请求一次提交整组虚构凭据后复用密码；成功只出现一个应用请求。Linux 六项通过（0.38s），Windows 原生证据 auth-tests.ps1/auth-result.txt。未使用用户真实远端或凭据，没有将本机 HTTP 覆盖视为 HTTPS/SSH/托管平台完整认证。

尚待：应用 editor UI 和 sequence 编辑器、普通 Pull 冲突 resolver/commit-after-merge、覆盖文件列表诊断、中英文 Pull 对话框和后台生命周期、配置驱动 rebase 的后续处理、SSH、完整原生 UI 验证。CommandSpec 中测试 editor=true 仅 fixture 使用，产品必须提供真实交互；没有发布 Pull 功能或更新便携包。显式 Rebase 更新路径仍按独立流程编排。

## F2/F3 进展：Git 编辑器 helper 与有界文档（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的完整 `rt/.../GitRebaseEditorApp.java`、`rebase/GitSimpleEditorHandler.java` 和 `GitRebaseEditorService.kt`。上游 helper 将文件路径与 cwd 发送给操作 handler，由应用读取/写入，返回退出码；取消编辑返回错误码，不能等同于取消整个操作 token。交互式编辑器的结构化 todo UI 仍须另行对照，当前仅实现其文件交接。

新增不依赖 desktop 的 `gitui-editor` binary、`src/editor.rs`；复用现有本机 capability 通信，增加独立 CommitEditor/SequenceEditor 请求类型。helper canonicalize 路径，发送路径而不是文件内容；应用 Document 负责文本，保存后回复 0，关闭/拒绝回复 1。此处文档 Drop 主动回 1，因此不触发认证 Pending Drop 的整操作取消。helper 参数有误/通信失败只输出固定诊断。Git shell editor 命令对含空格、引号的绝对 helper 路径转义，Windows 路径转换为正斜杠。

本地适配：Document 仅接受操作所对应 gitdir 内的提交说明文件或 rebase-merge/git-rebase-todo；UTF-8 文本上限 1 MiB，超限/非 UTF-8 报错不截断。保存前比较原内容，外部更改时拒绝覆盖；待答请求记录 server stop，已结束会话拒绝迟到保存。写入不是事务性跨进程锁，也未保证应用崩溃中的原子替换；编码扩展、更多上游允许路径、外部文件替换竞争仍待评估，不宣称通用编辑器或完整安全审计。

Linux no-default-features 全套通过，日志 `artifacts/git-editor/linux-result.txt`。首次全套中一个测试副本 spawn 出现 Text file busy；相同代码复跑通过，随后最终全套通过，保留为测试环境瞬态观察而非已定位根因。四项 helper 集成覆盖真实 Git commit 编辑中文多行、关闭编辑器不产生新提交且不取消 token、实际 interactive rebase todo 读写、拒绝 gitdir 外文件、文件已改时不覆盖、server 结束后迟到保存拒绝。路径含中文和空格；Windows 原生证据 `artifacts/git-editor/tests.ps1`/`auth-result.txt`。

尚无 GPUI 编辑弹窗、typed 请求分发到对应 UI、Pull/update 操作级 editor server 接线、结构化 todo 编辑、字号/快捷键等功能。既有 Fetch 不设置 editor helper，因此不会产生这些新请求。新增 helper 尚未加入便携包；须在功能接线与原生 UI 验证后一起打包，不以当前命令行 helper 测试称编辑器功能已交付。

## F3 进展：GPUI 提交说明编辑弹窗（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的完整 `rebase/GitUnstructuredEditor.java`、`GitImplBase.loadFileAndShowInSimpleEditor/ignoreComments/showUnstructuredEditorAndWait`。上游使用 CommitMessage 编辑控件、初始 caret 0、确定/取消；加载时过滤 U+0001 开头注释行并 trimLeading，取消不写回。UTF-8 是 root 为 null 时的来源策略；其他 commit encoding 仍需适配。

新增 `src/editor_dialog.rs`，用 TextareaState 提供多行草稿、初始聚焦、中英文标题/按钮/错误、确认保存、关闭/取消丢弃 Document（helper 返回 1）、保存失败不关闭并保留草稿。Ctrl+Enter 接线到确认，普通 Enter 由多行控件处理。原始文件仍在 Document 中用于外部修改检查，显示文本过滤不影响取消时原文件。sequence 暂仅提供原文编辑能力，不能视为 JetBrains 结构化交互式变基 UI 已还原。

新增 `examples/editor_dialog.rs` 为不打包的原生验证 harness，复用同一弹窗代码和相关主应用主题/字体。Windows artifacts/editor-ui：中文窗口定向输入/回车换行/点击确定后，save-result.json 验证退出码 0、输入已写回、注释过滤；英文窗口在外部修改文件后点确定仍保持待答并显示错误，再取消，cancel-result.json 验证退出码 1 且外部内容不变。仅关闭自有 preview 进程 31528、32780、29956，未操作用户 Rebased。

验证边界：首轮 harness 没有设置暗色主题，已修正；后续截图被仍打开的用户 Rebased 窗口部分遮挡，只能作为局部观察，不能宣称完整视觉验收。Windows 拒绝全局键盘前台注入，脚本按现有保护停止，随后使用指定 HWND 的消息验证基础事件；合成 WM_CHAR 中文字符出现错误，故这不算中文 IME 验收，也未验证实际 Ctrl+Enter/Tab/辅助功能。主应用中文字体回退和真实键盘路径仍待在完整 Pull 流程验证，不以 ASCII 定向输入冒充完成。

主应用已声明弹窗模块，仍没有可点击 Pull 入口或编辑请求 UI 分发，当前新增交互只由验证 harness 调起。下步连接 Pull 对话框、操作进度、认证/编辑分发及结果刷新，再整体验证发布。原生主程序编译检查单独执行；现有 dist 仍是 Fetch 版，没有更新安装包或总体内存结论。

## F3 进展：普通 Pull 主界面接线与预览包（2026-10-16）

再次读取 `GitPullDialog.kt` 的 createCenterPanel、默认远端/分支切换、验证、选项、设置保存及 Fetch 刷新路径（参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`）。新增 `src/pull_dialog.rs`，远端默认和切换后的分支沿已有 Targets 规则，分支输入按实际存在性校验，提供当前远端候选菜单；普通 Pull 的五个选项按已翻译互斥矩阵启用。单仓不额外显示仓库选择器。当前显式 REBASE 尚未在 UI 开放；选项持久化、完整 autocomplete/自然排序、弹窗内 Fetch/快捷键、no-verify Git 版本门槛尚未还原。候选菜单暂最多 200 项，可输入其余已有分支；完整列表与更大规模优化仍待办，不能宣称与 Rebased 完全一致。

主工具栏增加 Pull。后台读取目标和 gitdir，确认后运行已有普通命令接口；沿用同一写操作 token/task，禁重复 Fetch/Pull 和切仓，支持取消。请求按 Kind 分发登录/提交说明编辑器，文档读取放后台；helper server 随操作结束清理。完成、失败、取消后强制刷新实际 Log/refs/本地状态，并区分冲突、未完成操作和命令成功。当前只显示本地化状态摘要，错误详情脱敏/进度、typed 状态随语言切换刷新、完整中途关闭竞态仍待补；不会自动强制 checkout、stash 或回滚普通 Pull。

Windows 原生完整流程证据 `artifacts/pull-ui/`：专用 source/client 分叉仓库，启用 fixture 的 GIT_MERGE_AUTOEDIT=yes，点击 Pull 默认 origin/main，真实 helper 打开应用编辑弹窗，定向输入 ASCII Reviewed: 前缀后确认；assertions.json 验证双父提交的第二父等于源 HEAD、编辑说明真实进入提交、工作区干净且 incoming.txt 内容正确、MERGE_HEAD 消失。refreshed.png 确认拉取完成和四条历史/合并 graph，保留原有选中提交。editor.png 拍于弹窗就绪前，实际编辑器证据为 editor-ready.png。原生测试未连接真实远端或使用用户凭据。最初 fixture 脚本函数 Git 与 git 命令同名递归，已终止于 CallDepthOverflow；改名 InvokeFixtureGit 并显式 git.exe 后重建隔离 fixture，不是产品问题。

Linux no-default-features 全套通过；Windows release 构建通过。只关闭自有 gitui-pull-ui PID 31924，启动脚本 session 正常退出，用户 Rebased 未操作。HTTP Pull 后端成功/拒绝/取消已在上一轮原生验证；本轮尚未跑带可见登录的 HTTP Pull 端到端、冲突 UI 后续、英文全流程、真实中文 IME/完整键盘，启动内存样本不能替代总体验收。

打包脚本新增 gitui-editor.exe 和 editor_sha256，更新 README/Windows 说明，明确 Pull 可创建合并提交，冲突继续/中止界面尚未开放，需外部 Git 工具处理未完成操作。生成 `dist/GitReview-windows-x64-pull-preview.zip` 并更新 canonical preview.zip，保留上一版 fetch-preview.zip。ZIP CRC 通过，三程序的包内/release/原生已测宿主字节一致；证据 package-check.json。主程序 SHA256 `fa0e8cff546dcf48401dfd83c2c834849ba21d9c7d016e8696bf02a6ef6b2d1f`，AskPass `2ca5e4e3780a618a41755285683746d175a8db5e36da043f1a81ae72eee2c2aa`，editor `cd100c9587ce4ae07e199bd715b6d3f1b1ffc0d99adcb8745f339e2fd77997ae`。本轮是普通 Pull 可手测增量；显式 Rebase、Push、checkout、F1/F2/F3/F4 完整验收继续推进。

## F3 进展：合并完成／中止后端与 UI 结构依据（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，完整读取 `plugins/git4idea/backend/src/merge/GitMerger.java`、`actions/GitMergeCommitAction.kt`、`actions/GitAbortOperationAction.kt`、`actions/GitOperationActionBase.kt`、`ui/toolbar/GitMergeRebaseWidget.kt`，核对 `GitMergeAction.java::handleResult`。GitMerger 使用 commit -F MERGE_MSG，文件不存在时用当前分支和仓库路径构造说明；不自动 stage。MergeCommitAction 仅在 MERGING 且无冲突时显示；普通 Pull 的冲突 resolver 仅在 commitAfterMerge 为真时自动完成。Abort 须确认后执行 merge --abort。Widget 为顶部横向不换行操作区，继续／解决与中止使用不同按钮背景；下一步 UI 沿用此结构，不能把新功能放入临时独立页面。已读源码不等于已安装 Rebased 原生界面验收。

新增 `src/merge_operation.rs` 后端，复用 common-dir 操作队列，执行前复核 Merge 状态，完成前检查 unmerged index；使用 worktree 专属 gitdir 的 MERGE_MSG。说明文件的访问错误不当作缺失；命令失败／取消后独立读取真实状态，不自动重试或 reset。此为显式完成／中止基础能力，尚未接入 UI，也未接自动 resolver、no-commit 策略和 squash 冲突的后续处理。跨进程修改仍可能发生，队列仅约束本应用。

真实 Git 集成测试 `merge_completion_and_abort_preserve_repository_state` 覆盖六种场景：解决并暂存后完成、缺失 MERGE_MSG 回退、中止、index.lock 引起提交失败／中止失败、预取消；断言双父关系、中文多行说明、实际 Merge 状态、HEAD 和无关未暂存修改。Linux no-default-features 全套通过（64 单元、51 repository 集成及其他套件）；Windows 原生同一六场景通过（13.98s）。证据 `artifacts/merge-operation/`，脚本使用本轮实际产物 repository-b0d19709ee73427e.exe。读取上游 GitSingleRepoUpdateTest 的冲突解决片段发现属于 RESET/stash 场景，不能冒充上述 merge 完成／中止的直接上游用例；本轮用例为按已读 action 规则补充的本地验证。未更新便携包，UI 确认、刷新、可取消生命周期与原生截图仍待接线验证。

同步落实用户 UI 优先级补充：AGENTS.md 与本计划要求功能第一版即对照 Rebased 的布局、入口、信息层级和操作顺序，仅像素细节延后。

## F3 进展：合并操作顶部入口（2026-10-16）

按上一节已读 GitMergeRebaseWidget／GitOperationActionBase／GitMergeCommitAction／GitAbortOperationAction 规则，`src/ui.rs` 在仓库标题旁新增状态驱动的顶部操作区。Merge 且无 unmerged index 时显示完成合并；Merge 状态显示中止，点击打开带仓库路径的确认弹窗。后台读取 history/local changes 时同时读取操作状态，沿现有 generation 检查丢弃过期结果；读取失败隐藏操作，执行前由后端再次复核。切仓清理状态；操作共用串行 token，结束后强制刷新 Log/refs/local 状态。中英文文案集中于 i18n。UI 结构已参考上游；按钮目前采用本地 primary/默认样式，上游继续绿底、中止红底和图标待统一主题时对齐，不宣称像素完成。

Windows release 构建通过，原生专用仓库 `GitReview-validation/merge-ui/client` 验证：初始冲突只显示中止；打开确认后取消，MERGE_HEAD 保留；再次确认中止，MERGE_HEAD 消失且冲突文件恢复 main 内容；重新构造冲突后外部解决并 stage，点击刷新出现完成合并；点击完成生成正确双父提交、工作区干净、MERGE_HEAD 消失。脚本、截图和断言见 `artifacts/merge-ui/`。只关闭自有 PID 27176，启动会话正常退出。Linux no-default-features 回归通过。

限制：未提供应用内冲突编辑／暂存；外部解决后需刷新。不把显式完成按钮等同于 Pull resolver 自动 commit-after-merge；no-commit、squash、rebase 后续仍按各自上游流程接入。状态摘要仍是上次操作结果，外部更改后不自动消失（resolved.png 显示上次“已中止合并”），后续需 typed 通知／生命周期统一；详细错误、图标、颜色、确认弹窗的最终排布和英文键盘路径尚待。未更新便携包，当前发布包仍为之前普通 Pull 版。

## F3 进展：Push 原生结果解析（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，完整读取 `plugins/git4idea/backend/src/push/GitPushNativeResultParser.java`、`GitPushNativeResult.java` 和 `tests/git4idea/push/GitPushNativeResultParserTest.kt`。新增 `src/push_result.rs`，映射 porcelain 六种 flag，提取 source/target、summary、reason 和两点／三点提交范围；非快进判断沿上游对 non-fast-forward／fetch first／failed to lock 的大小写无关包含规则，stale info 单独识别。使用借用字符串的惰性 iterator，解析本身不累计第二份输出；保留 target/summary 便于后续核对请求目标与远端拒绝类别。没有结果不能视为成功，截断输出的完整性需执行层接线时处理。

移植上游八类单结果和两类含 tag 输出用例，补充拒绝分类、中文引用、CRLF、格式错误、无法识别输出和范围判断。保持上游 parser 的非空 source 约束：删除输出的空 source 仍不解析；当前基础 Push 范围未开放删除。读取 GitPushOperation 的结果选择、空结果报错和拒绝更新循环片段：自定义拒绝/错误停止；force、非跟踪目标、非当前分支拒绝不进入更新；普通拒绝按配置/对话框决定更新。后续必须继续完整追踪，不把此处 parser 当作完整 Push 策略。GitPushParams 已读；GitPushTarget 部分路径已读，默认目标及相关 tests 仍待核对，尚无本地目标实现。

Linux no-default-features 全套通过（66 单元、52 repository 集成及其他套件）。真实 Git 集成针对临时本地 remote 验证新分支、已最新、快进、分叉拒绝、receive.denyNonFastForwards 导致远端拒绝、强制更新；拒绝后断言远端 tip 未变化。Windows 同用例通过（2.44s），证据 `artifacts/push-parser/`。测试中的 force 仅用于隔离 fixture 产生相应输出，不是产品入口；未使用用户远端或凭据。Push 命令执行层、输出完整性检查、认证、目标／待推送列表 UI、拒绝恢复交互仍未接入，便携包未更新。

## F3 进展：Push refspec 与附着分支默认目标（2026-10-16）

同一参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，已读 `GitPushSupport.java::getDefaultTarget/getPushTargetIfExist/makeTargetForNewBranch/getSource`、`GitPushTarget.java::getFromPushSpec/shouldSetUpstream/parse`，完整读取 `GitPushSpecParser.kt`、`GitPushSpecParserTest.java`、`GitPushTargetTest.kt`。上游先在跟踪远端（否则 origin／首个远端）匹配 push refspec；精确匹配优先于通配符，多个通配符取首个；无匹配时取跟踪分支，再回退默认远端同名分支。refs/heads/ 和所选 refs/remotes/<remote>/ 转标准目标，其他保留特殊 ref。refspec 前导 + 不转成产品强推授权。新分支且无现有 tracking 才默认设 upstream，显式设置 upstream 另行支持，commit/tag source 不设。

新增 `src/push_target.rs` 纯模型：target_ref、附着分支 default_for_branch、Target.destination/should_set_upstream。输入复用现有 Targets 快照，push specs 独立传入；当前调用者须保证快照 tracking 对应该 source，尚未连接真实仓库配置读取、UI 编辑校验或执行命令。保留上游空／格式异常 refspec 的解析规则；最终目标有效性必须在接线层验证。detached/任意提交 source 的默认 detached 唯一名称路径未实现，不以附着分支模型宣称完整默认目标。

测试翻译上游 star 位置、完整／短引用、部分分支通配、精确覆盖通配、tag spec 忽略、特殊 refs/for、包含斜杠的远端、跟踪远端优先等场景，并补充第一个通配结果、多余冒号、显式 upstream 与非 branch source 条件。Linux no-default-features 全套通过（68 单元、52 repository 集成及其他套件），记录 `artifacts/push-target/linux-result.txt`。本轮纯模型测试不等于真实仓库配置／Windows UI 验收。PushTargetPanel 仅定位到入口，未读取具体控件逻辑；下一步读取完整目标 UI 和待推送提交收集路径再接入，发布包未变。

## F3 进展：Push 真实仓库目标快照（2026-10-16）

`src/push_target.rs` 新增 Repository::push_snapshot：从已存在本地分支读取 source OID，复用远端／跟踪引用读取，再读取所选远端的全部 push refspec 并调用上一节默认规则。未初始化分支、detached 返回无附着分支快照，不自行发明推送分支；已初始化且无远端则保留源信息、default=None。配置按 NUL 分隔读取，缺失值（exit 1 且无输出）和读取错误区分；取消、超时、非 UTF-8、不完整记录和超过 64 KiB 输出均报错，不将截断数据当成完整配置。读取沿只读 command，不访问网络，不写 config/ref。实际推送前仍需重新核对 source/target，当前快照不是跨进程原子快照。

真实 Git 集成覆盖初始空仓库、无远端、多远端、精确 refspec 覆盖通配、refs/for 特殊目标、带斜杠远端、tracking 优先、标准 refs/remotes 映射、新分支 upstream 判断、detached、预取消和损坏 config。远端均为 example.invalid 占位地址，流程不联网。Linux no-default-features 全套通过（68 单元、53 repository 集成及其他套件），Windows 同一用例通过（4.86s）；证据 `artifacts/push-snapshot/`。超大配置边界尚无直接用例；没有把损坏配置用例视为所有读取故障覆盖。

继续核对同一上游 commit 的 GitPushTargetPanel::verify/fireOnChange/getTargetNames：编辑使用 GitPushTarget.parse 校验，upstream checkbox 仅可见且勾选时显式设置，尚未移植 UI；完整布局仍待读取。完整读取 GitOutgoingCommitsProvider.java：已有目标用 destination..source，新目标用 source --not --remotes=<remote> --max-count=1000；查询错误作为 VcsError，不能显示成无待推送提交。下一步按此规则实现有界待推送查询并接推送窗口。本轮仍无 Push 产品执行入口，发布包未更新。

## F3 进展：待推送提交分页（2026-10-16）

重新核对参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GitOutgoingCommitsProvider 全文，沿 GitHistoryUtils.history、GitLogUtil.readFullDetails、GitDetailsCollector 读取调用链。新增 Repository::outgoing_commits 与 push_target::Outgoing：已有目标用本地 remote-tracking 目标到固定 source OID 的差集；新目标用 source --not --remotes=<remote>，保留上游 1000 条上限。现有目标本地按页读取，每页最多 1000，额外一条判断 more；新目标达到上限以 capped 表示“已达上游上限”，不冒充已知总数或证明仍有更多。命令末尾 -- 隔离路径，源 OID 校验并确认为 commit，已有目标先解析成 OID，远端不存在／目标消失／查询失败返回错误。

低内存适配：这里只读取摘要、父节点等列表字段，文件状态和完整消息按选择按需加载，不像上游 full-details collector 一次保留所有提交文件状态。已有目标分页超过1000不截断可访问历史；新目标仍遵循上游1000上限。上游 GitHistoryUtils 在部分详情已读后发生错误可能返回部分列表；本地目前采用整体查询失败，不把部分结果当完整，需 UI 明确错误。各页仍读取当时远端 refs，不是固定全局快照；UI 分页接线前需处理 fetch/外部引用变化引起的列表失效。shallow 边界标记、完整排序要求及选中提交详情链仍待后续核对，不宣称完整 Push 列表交付。

真实 Git 用例覆盖已有目标差集、两页/空结果、新目标仅排除所选远端、该远端其他分支参与排除、special ref 新目标、缺失目标/远端、非法源及预取消。fast-import 构造1002提交验证：新目标 offset990仅10条并 capped，offset1000停止；已有目标可跨1000继续读取且 more 正确。Linux no-default-features 全套通过（68 单元、55 repository 集成及其他套件）；Windows 两项原生用例通过（2.90s），见 `artifacts/push-outgoing/`。没有连接真实远端，尚未接 Push UI 或执行入口，发布包不变。

## F3 进展：普通 Push 单次执行后端（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 GitPushOperation::doPush/createPushSpec、GitImpl::doPush、GitPushSource.OnBranch::getRevision、GitPushRepoResult::convertType，以及 GitPushOperationSingleRepoTest 的 successful push/new branch/upstream set/upstream unchanged/rejection dialog 场景。新增 `src/push.rs`：附着分支单次普通推送，--progress --porcelain，沿源完整 ref 与 set-upstream 条件构造 refspec；选项移至 -- 前是本地参数隔离适配。复用 common-dir 队列、后台进程、可选应用认证 server；操作后重新读本地状态。执行前核对 source branch/OID 未改变、远端仍在、引用合法；这不是跨进程原子保护，检查后外部修改仍可能发生。

结果按匹配 source/target 的 porcelain 记录分类，保留有界原始命令结果；多 push URL 的任一匹配拒绝不被其他成功遮盖。中断、输出截断、非 UTF-8、成功退出却无匹配记录保留未知／失败状态，不冒充成功。非快进、stale info、其他拒绝分别记录；本模块只负责一次 attempt，未实现上游拒绝更新对话框/配置/重试编排。没有 force、tag、skip-hook 或 commit-source 产品选项，不能把当前普通路径当完整 Push。

真实临时本地 remote 测试：新分支创建并设置 tracking、已最新、快进成功、审阅后 source 变化拒绝、分叉拒绝后双方 tip 保持、推到其他新分支但不覆盖现有 tracking、未提交和未跟踪文件保留、非法目标/预取消。Linux no-default-features 全套通过（68 单元、56 repository 集成及其他套件）；Windows 同用例通过（8.15s），证据 `artifacts/push-command/`。当前认证只是接现有 Requests/AskPass 机制，尚无 Push HTTP/SSH 真认证用例；多 URL、截断、服务端拒绝的执行级测试仍需补。推送 UI、进度／详细错误、拒绝恢复和原生端到端待接入，便携包未更新。已定位 UI 后续入口 platform/dvcs-impl/.../VcsPushDialog.java、PushLog.java、PushController.java，仅路径确认，不算行为依据。

## F3 进展：Push 失败回归与窗口结构依据（2026-10-16）

在既有真实 Git 测试上补充 Push HTTP 认证拒绝／关闭提示取消（tests/fetch_auth.rs）：复用回环 HTTP Basic fixture 和真实 askpass helper，仅虚构凭据；一次用户名对话提交整组凭据，密码复用，无遗留 pending。断言认证失败为 Failed、关闭提示为 Cancelled，源 OID 和未提交内容保持，branch.main.remote 未写入。此 fixture 始终返回401，不是成功 smart-HTTP Push 的验证，成功认证、HTTPS/SSH 仍待。

tests/repository.rs 新增多 pushurl 部分失败场景：两个临时本地远端，第一处已分叉拒绝，第二处成功更新；结果保持 RejectedNonFf、Git 非零退出，分别断言两处远端最终 tip。确认不能把失败或取消描述为“远端均未变化”。当前状态保留单一拒绝类型及原始有界输出，UI 后续必须说明可能部分完成；逐 URL 详细结果尚未建模。

Linux no-default-features 全套通过（68 单元、57 repository、8 HTTP认证及其他套件）。Windows 原生2个认证用例通过（2.93s），多URL用例通过（4.52s），见 artifacts/push-failures。未使用用户远端／凭据，未更新包。

UI 依据（同一参考 commit）：已读 platform/dvcs-impl/src/com/intellij/dvcs/push/ui/VcsPushDialog.java 的构造与 createCenterPanel/createOptionsPanel、PushLog.java 的树编辑/selection及分栏构造、PushController.java 的编辑时禁用确认与 createTopPanel。窗口中部是左右 splitter（默认0.5），左侧仓库/目标可编辑树与提交节点，右侧变更浏览器及可选下方 commit details（默认0.67）；顶部用于通知，底部是选项和推送动作。单仓不显示多仓根展开柄，目标编辑中禁用推送。按此结构实现下一版窗口，不另做一套纵向表单；相关类尚未全文读取，选择聚合、完整快捷键和 Diff 行为需沿方法继续核对。功能阶段先对齐此结构，颜色/尺寸等像素细节后续统一。

## F3 进展：Push 窗口与普通推送主流程（2026-10-16）

新增 src/push_dialog.rs，沿已读 VcsPushDialog/PushLog 的左右分栏：左上 source→target，下面待推送提交；右侧所选提交文件及说明。补读 PushLog::onSelectionChanges/updateChangesView/updateDetailsPanel：单提交才展示详情，多提交为聚合路径，当前只实现单选。目标点击编辑远端/分支，Apply 在后台验证与重载；编辑/加载/错误状态禁推送。每页100条、仅保留当前页和当前详情，切换查询取消旧 token 并以 generation 丢弃旧结果，关闭实体取消后台读取。当前固定左右比例，文件为路径列表，无目录树/窗口内Diff、多选聚合、键盘完整路径、分隔拖拽、upstream checkbox/目标编辑取消和autocomplete；这些是功能待补，不属于已验收项。页切换时远端引用仍可能变化，固定快照失效检查待补。

新增 Repository::custom_push_target，先查已配置远端再校验普通分支名。补读 GitRefNameValidator.java 全文：本地 Git check-ref-format 加上游额外的首字符 -、引号、dot-before-slash 禁止条件；不自动改写输入。测试覆盖已存在/新中文分支、非法字符、缺远端及取消。UI 中默认特殊 ref 保留；手工编辑仍按上游 ordinary target parse，特殊 ref 编辑细节须继续核对。

src/ui.rs 接 Push 按钮、异步源读取、推送确认、共享写操作 token、真实认证请求分发及结束强制刷新。结果含成功/已最新/拒绝/取消/失败，取消失败提示可能已有部分远端更新；拒绝更新对话框、逐URL结果详情和完整诊断尚未接入。无 force、hook绕过入口。中英文资源集中于 i18n。首次编译 trait 路径按编译器建议仍不适用当前 facade，检查现有UI及gpui-kit导出后修正为 component::Disableable。

Windows 原生专用 push-ui/client 与本地 bare remote：打开窗口出现唯一待推送提交，单击后右侧显示 review.txt、说明和作者；点击推送后远端main、本地origin/main均等于source，工作区干净，断言见 artifacts/push-ui/assertions.json，截图dialog/details/completed。仅关闭自有PID33656，启动会话正常退出。原生验证为默认目标、中文UI、单提交路径，尚未验证目标编辑及复杂键盘。Linux no-default-features全套通过（68单元、58repository、8认证及其他套件）。原生流程之后补充上游分支名额外规则、详情读取失败后重新选择清除错误；这两项改动经Linux回归与Windows编译验证，未重跑完整UI。没有更新发布包，尚不能称Push整体完成，也不把小仓库启动内存当总体验收。

## F3 进展：Push 目标编辑取消与 upstream（2026-10-16）

完整核对同一参考 commit 的 GitPushTargetPanel.SetUpstreamCheckbox、构造条件、fireOnChange/fireOnCancel：checkbox 初始 false，仅 defaultTarget 类型 TRACKING_BRANCH 且非新分支时建立，只有合法且不同于默认远端/分支时显示；应用时仅可见且选中才请求更新 upstream。新增 push_target::valid_branch_input/show_upstream_checkbox 与单元场景，区分默认类型/新目标/合法名称/目标是否变化。引用名称原生Git校验仍为最终检查；UI 暂不做上游 on-typing 自动清理（保持用户输入并在 Apply 时校验，差异保留）。

PushDialog 增加输入观察、草稿/已应用 upstream 区分、取消编辑恢复目标与复选状态并重新加载；编辑开始清空旧提交和详情，禁分页/提交选择，加载时禁输入/远端切换，防止旧页覆盖新草稿。Apply 未改变目标时保留既有特殊 ref 身份。显示新上游提示，尚未合并为上游同款紧凑标签；快捷键和autocomplete仍待。

Windows release 构建通过。原生复用专用 push-ui/client：打开Push→编辑main为main-review，出现未勾选checkbox且推送禁用→取消恢复main→再次编辑、勾选upstream、Apply显示新目标/新上游/新分支→确认推送。assertions 验证推送前tracking仍origin/main，完成后远端main-review=source、tracking=origin/main-review；仅关闭自有PID33036。证据 artifacts/push-edit，包括定向ASCII输入脚本、截图及断言。无新提交但新分支仍允许推送，符合上游新目标 hasSomethingToPush 规则；“没有待推送提交”指集合为空，不表示不能创建分支。Linux no-default-features全套通过（69单元、58repository、8认证及其他套件）。这不覆盖真实中文IME、远端切换、加载中取消竞态及默认特殊ref手工编辑所有路径；无发布包更新。

## F3 进展：Push 文件 Diff 与 UI 结构约束（2026-10-16）

功能优先不延后 UI 结构：新增功能从第一版按 Rebased 的入口位置、面板分区、信息层级与交互顺序实现；精确颜色、间距、边框等后续细化，避免结构返工。此约束已同步 AGENTS.md。

参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 `platform/dvcs-impl/src/com/intellij/dvcs/push/ui/PushLogChangesBrowser.kt`，沿 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/ui/ChangesBrowserBase.java` 的 showDiff/showStandaloneDiff、双击和 Enter 路径确认：Push 变更浏览器默认使用独立 Diff 窗口，并构造所选或全部文件的导航链。`src/push_dialog.rs` 接入 Show Diff、双击、Enter/Ctrl+D，使用嵌套 GPUI 对话框复用 `DiffTab` 的完整文件、单/双栏和文件导航；主 Log 原有独立 Tab 行为不变。关闭子对话框保留 Push 的选择；Push 实体不持有 viewer，关闭时依赖 viewer Drop 取消读取，不增加长期 Diff 缓存。独立原生窗口与当前嵌套对话框仍有差异；文件目录树、多选聚合、完整快捷键和窗口尺寸视觉还原待补。

Windows release 构建和 Linux no-default-features 回归通过。原生专用 fixture 验证工具栏打开完整30行文件（第15行修改）、导航到第二文件、关闭返回保留 Push 选择、选中文件 Enter 再打开 Diff。`artifacts/push-diff/` 保存截图、脚本和 assertions：HEAD 未变、工作区干净、远端仍为基线，Enter 未误触推送。双击/Ctrl+D/F7 已接线但本轮未逐一原生验证；未据启动内存样本声明内存验收。仅关闭自有 gitui-push-diff PID6112，未更新发布包。

## F3 进展：Push 分页引用失效检查（2026-10-16）

重新读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/push/GitOutgoingCommitsProvider.java` 全文：上游一次查询并持有完整结果，本地为内存控制分页，因此需避免每页引用变化造成漏项。`Repository::outgoing_commits_checked` 在查询前后读取所选远端 refs 的名称/OID/symref，已有 special 目标另记录 commit OID；后续页与首轮记录比较。变化时返回错误，不返回空页。`PushDialog` 只保留 Arc 引用记录及当前页，目标 Apply 开始新查询，普通翻页及取消编辑保留原记录；错误禁推送，中英文提示重新打开窗口刷新。单次 outgoing_commits API 委托同一路径。引用读取受现有8 MiB上限和取消/超时约束。

真实 Git 测试覆盖已有目标和新目标：稳定引用可读下一页，目标前移后旧页请求被拒绝，重新读取正确更新集合，新增其他同远端分支也使旧记录失效；断言 HEAD/工作区未被浏览修改。Linux no-default-features 全套通过（69单元、59repository、8认证及其他套件），Windows 原生新增用例通过（5.39s），证据 `artifacts/push-pagination/`。这是本地分页适配测试，不冒充上游自带用例。该检查不是跨进程锁，不能捕获查询中 refs 改动后又完全恢复的 ABA 情况；shallow 边界变化未纳入，后续需补。新错误提示尚未原生截图验收；发布包未更新。

本轮 Windows release 编译通过；未启动新的 UI 测试进程。现有原生 UI 证据仍是上一轮 Push Diff，不将后端原生测试等同于本轮错误提示的 UI 验收。

## F3 进展：Push 目标编辑键盘确认／取消（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 `platform/dvcs-impl/src/com/intellij/dvcs/push/ui/PushLog.java` 的 processKeyBinding、stopEditing/cancelEditing 和 `plugins/git4idea/backend/src/push/GitPushTargetPanel.java` 的 fireOnChange/fireOnCancel。Enter 编辑时结束编辑并先校验；取消恢复已应用目标。`src/push_dialog.rs` 在目标行监听 GPUI Input 的 Enter/Escape action：无 Shift/secondary 的 Enter 走现有异步 Apply，编辑期间消费事件防止传播成 Push；Escape 走取消编辑。监听在输入框之后，保留控件自身优先处理补全菜单/输入法等状态的机会，不在捕获阶段抢键。真实 IME 尚未验收；未声称已还原完整树编辑、F2 和焦点遍历。

Windows release 构建通过。原生专用进程 PID11912：main 编辑追加 -review 后 Enter，目标显示 origin:main-review、窗口仍开且列表更新；再次编辑追加 -review 后 Escape，目标恢复 main-review、窗口保持。截图/定向消息脚本见 `artifacts/push-keyboard/`。断言 HEAD/工作区未变、main远端仍为基线、main-review远端不等于source、main-review-review未创建、tracking仍origin/main。只关闭自有 gitui-push-keyboard，启动会话退出。本轮是 ASCII 输入路径，未测连续按键、无效输入后的焦点、输入法、远端菜单打开时Esc；未增加只镜像事件实现的测试，未更新发布包。

## F3 进展：指定提交 Checkout 普通执行后端（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，完整读取 `plugins/git4idea/backend/src/actions/GitCheckoutActionGroup.kt`，读取 `branch/GitCheckoutOperation.java` 的普通尝试及 common failures、`commands/GitImpl.java::checkout`，以及 `tests/git4idea/branch/GitBranchWorkerTest.kt` 的 revision 成功/失败和 unmerged 场景。Log 单提交菜单排除当前本地分支、列出可切换分支并提供 revision 动作；revision 动作用完整 hash。普通命令不 force，失败按 unmerged/local changes/untracked 分类；本地覆盖冲突进入显式 Smart Checkout/Force/Cancel 路径，而非自动丢弃。上游 detach 通过 reference^0 实现。

新增 `src/checkout.rs`：接受所选完整40/64位 commit ID 与 expected HEAD，common-dir 队列中复核 HEAD 和 commit 对象，执行 checkout <oid>^0 --。显式 peel 也避免与同名全长hash分支混淆。执行后用独立未取消 token 读取实际 HEAD/operation state；checked_out 同时检查成功退出、未中断、detached 和目标 OID。保留命令诊断及状态，不在失败或取消后 reset。expected HEAD 是本地防陈旧适配，不是跨进程锁。当前仅普通 attempt，Smart Checkout 选项/状态分类呈现、强制路径、菜单/UI/刷新仍未接入，不能宣称 checkout 功能完整。

真实仓库用例覆盖干净切换、无关未暂存/已暂存改动保留、覆盖本地改动失败、覆盖未跟踪文件失败、陈旧HEAD、缺失commit、blob不能充当commit、预取消、重复checkout当前commit、unmerged index保留和非法参数拒绝。断言源分支不移动、HEAD/detached/工作区/index及Merge状态；用例为上游场景与本地适配组合，未移植多仓回滚或Smart Checkout测试。Linux no-default-features 全套最终通过（69单元、61repository、8认证及其他套件）。此前一次 editor 集成测试启动helper报 Text file busy，保留 linux-first-failure.txt；无代码改动重跑通过，未将其标记为已修复。发布包未更新。

Windows 原生同两项checkout集成测试通过（13.45s），涵盖上述各模式；证据 `artifacts/checkout-command/`。本轮没有新增UI或启动应用窗口，不将后端测试视为菜单/确认/恢复流程验收。下一步沿Log提交菜单接线并继续读取覆盖冲突恢复路径。

## F3 进展：Log 提交 Checkout 入口（2026-10-16）

再次读取同一参考 commit 的 GitCheckoutActionGroup.kt / GitCheckoutRevisionAction：revision 动作直接操作所选完整hash，不额外弹通用确认。`src/ui.rs` 的 Log 行增加右键“检出提交 <短hash>”，执行捕获完整OID、打开菜单时的HEAD及仓库路径；回调复核仓库未切换、共用写操作token禁重复操作。后台调用checkout后端，结果检查实际detached+HEAD，成功/失败/取消均刷新Log、refs、本地变更和操作状态。中英文摘要集中于i18n。菜单当前只有revision动作，尚未补上游同commit本地分支子项、完整上下文菜单、Smart Checkout及覆盖冲突文件列表/详细错误。右键目标绑定被点击行，但当前右键不改变先前高亮选择，需后续对齐选择语义。

Windows release 构建通过。专用checkout-ui克隆fixture并创建未跟踪local-note.txt：右键第二条Context base→检出d08ab465，实际HEAD等于main^且detached，main仍3c84581，本地便签内容和未跟踪状态保持。refreshed.png确认HEAD(detached)标签已移至目标行，详情中旧提交不再显示HEAD；completed.png是异步刷新完成前的中间帧。证据 `artifacts/checkout-ui/` 的截图与assertions。仅关闭自有gitui-checkout-ui PID31696，未操作用户Rebased。尚未原生验证失败菜单、取消竞态、英文/键盘菜单；没有更新便携包，不将普通成功路径当checkout整体完成。

## F3 进展：Checkout 失败类别及原生提示（2026-10-16）

完整读取同一参考 commit 的 `GitLocalChangesWouldBeOverwrittenDetector.java` 和 `GitUntrackedFilesOverwrittenByOperationDetector.java`，沿 GitCheckoutOperation common failures 优先顺序：unmerged→local changes→untracked。`operation_events.rs` 增加现代Git覆盖错误头的流式识别，沿既有固定尾部状态扫描，无需保留全部输出；`checkout::ResultState::status` 结合退出状态、停止原因及实际HEAD分类，成功退出但实际状态异常仍为Failed，不由错误短语伪装覆盖失败。`src/ui.rs` 将三类原因及超时映射到集中中英文摘要。当前不收集受影响路径，旧Git单行格式也尚未翻译；这只是恢复流程前的分类，Smart Checkout/Force/Cancel弹窗仍待接入。

测试在真实checkout场景断言Success/LocalChanges/UntrackedFiles/Unmerged，流式测试覆盖逐字节分片及后续10万字节诊断不抹掉标记。Linux no-default-features全套通过；Windows两项checkout集成通过（13.65s）。Windows release构建通过，专用checkout-ui fixture在detached基线上修改context.txt，启动自有gitui-checkout-failures PID8284，右键检出main tip被拒绝，界面显示本地修改覆盖提示；assertions及脚本验证HEAD仍基线、main未变、本地修改及未跟踪便签保留。证据 `artifacts/checkout-failures/`。仅关闭自有进程，未更新包；UI本轮仅验证local-changes提示，其余类别由原生后端测试覆盖，不能等同于各自UI验收。

## F3 进展：Smart Checkout 保存／切换／恢复后端（2026-10-16）

读取同一参考 commit 的 GitCheckoutOperation.smartCheckoutOrNotify/smartCheckout，GitPreservingProcess.kt::execute/load，以及 GitStashChangesSaver.java::load，核对 GitBranchWorkerTest 的 agree-to-smart-checkout 内容合并断言。关键区别：checkout 不传 autoLoadDecision，成功保存后即使切换失败也在finally中尝试恢复；Pull update 的成功条件恢复策略不能直接套用。stash pop 是否 --index 由 staging-area 选项决定，恢复只尝试一次。

`update_session::Session` 提取 restore_saved，原 finish 保持更新结果判定不变。`checkout::smart_execute` 仅供用户显式选择Smart之后调用：持有同一common-dir队列，保存前校验目标和expected HEAD，复用保存，执行不重复加锁的普通checkout，再独立非取消恢复。返回save/checkout/restore各自结果、剩余saved_oid以及最终HEAD/state；不会把切换成功等同于恢复成功。复用现有top-stash身份核对，避免pop其他应用插入的stash；保存失败不执行切换，恢复冲突保留stash。reinstate_index参数显式传入，UI尚未决定/接线。尚未接Smart/Force/Cancel弹窗、受影响文件列表、冲突解决和恢复通知；不宣称完整Smart Checkout可用。

真实临时Git测试覆盖：成功合并本地不同位置修改并恢复暂存状态、未跟踪文件阻止checkout后仍恢复本地修改、恢复冲突保留新stash和旧stash、index.lock保存失败不切换。每种场景预先建立独立旧stash，核对恢复不丢失旧记录。Linux no-default-features全套通过（69单元、62repository、8认证及其他套件），证据 `artifacts/smart-checkout/`。取消发生在保存之后、外部HEAD竞态及恢复失败通知的端到端仍待验收。此轮没有UI改动或发布包更新。

Windows 原生同一测试的四种场景通过（10.88s）；仅运行测试二进制，未启动或关闭用户GUI。没有将后端测试当作尚未接入的恢复弹窗验收。

## F3 进展：Checkout 受影响文件列表（2026-10-16）

完整读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GitMessageWithFilesDetector.java 和 GitSmartOperationDialog.java。上游用开始/结束标记提取文件，恢复弹窗采用顶部说明、中央changes browser（无Change对象时simple paths browser）、左侧Force及Smart/Cancel动作，Cancel默认聚焦；后续UI沿该结构实现。本轮仅提供文件数据，不把状态栏摘要当恢复弹窗。

新增 `checkout_files.rs`，按之前已读两种覆盖头与结束标记提取现代Git的tab缩进路径；与上游trim相比，本地保留文件名首尾空格，并解码Git引号/C转义及八进制UTF-8供显示。该列表严格为显示数据，不能直接充当Git参数或文件访问授权。`checkout::ResultState::affected_files` 只在相应失败类别下解析stderr，输出tail截断、超过200项、非UTF-8、缺标记或无法识别格式均complete=false。旧Git单行/空格分隔列表暂不完整解析。checkout命令局部设LC_ALL/LANG=C，使标记不依赖用户系统语言，界面仍本地化。

单元覆盖中文八进制、空格、tab/newline/引号、CRLF、截断和不完整列表；中文测试首次发现u8八进制计算中间值溢出，修正为先减ASCII基值再相加。真实Git新增quotePath=true中文文件/带空格路径覆盖错误，断言解码列表及本地文件和HEAD保留；原checkout覆盖场景也补充文件列表断言。Linux no-default-features全套通过（70单元、63repository、8认证及其他套件）。证据 `artifacts/checkout-files/`。路径列表尚未接UI；没有更新发布包。

Windows原生新增中文路径用例通过（1.70s）。本轮无UI变更或GUI测试进程；尚未进行恢复弹窗原生验收。

## F3 进展：Checkout 恢复弹窗与显式 Force（2026-10-16）

沿已读同一参考 commit 的 GitSmartOperationDialog（顶部说明、中央文件、左Force、右Smart/Cancel）和 GitCheckoutOperation SMART/FORCE 分支接入 `src/ui.rs`。普通checkout判定LocalChanges后打开弹窗，传捕获目标/expected HEAD与受影响列表，不完整时提示；取消只关闭。点击选择后关闭弹窗，核对仓库路径及写操作占用，后台执行且最终强制刷新。Smart当前采用stash并恢复index；各阶段失败/剩余stash/实际冲突不会宣称整体成功，状态保留stash OID。Force为显式独立后端 `checkout::force_execute`，复用校验与队列，仅此路径加--force，不自动重试。没有触碰用户仓库。

当前恢复窗口采用简单路径列表，尚无changes browser中的Diff/rollback、文件树、取消默认焦点及完整键盘遍历；Enter默认确认返回false，避免直接执行Smart/Force。Smart固定恢复index属于当前适配，后续需结合staging设置。状态栏可能截断stash ID，持久通知/可复制详情和冲突编辑入口仍待；Force失败/操作取消细分也需继续统一。未跟踪覆盖和unmerged仍使用各自提示，不误进入Smart弹窗。

Linux no-default-features全套通过（70单元、64repository、8认证及其他套件）；新增Force真实Git用例断言丢弃tracked修改、保留无关untracked、源分支不移动及拒绝陈旧HEAD。Windows原生同用例通过（1.84s），release构建通过。原生GUI专用fixture：本地context.txt修改阻止切换，弹窗正确列出文件；取消保持HEAD和本地内容；再次选择Smart，HEAD变目标tip，stash恢复冲突产生三阶段unmerged index，原始修改仍可从保留stash读取，未跟踪便签保持。restored-state.png显示未完全恢复摘要，assertions.json记录实际状态与stash。证据 `artifacts/checkout-recovery/`。仅关闭自有gitui-checkout-recovery PID32720，启动会话退出。GUI未逐一验证Force及Smart无冲突成功路径，不以后端用例代替UI验收；未更新发布包。

## F3 进展：Push 拒绝更新策略与多URL拒绝优先级（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GitPushOperation.execute、pushingToNotTrackedBranch、pushingNotCurrentBranch、update/settings/dialog相关方法，GitRejectedPushUpdateDialog.kt主要布局/选项路径，以及GitUpdateResult.java。核对GitPushOperationSingleRepoTest.kt中跟踪/非跟踪目标、过多拒绝、merge更新、自定义hook拒绝场景。上游普通non-FF且当前分支推向跟踪目标才允许更新；首次无auto-update设置需询问，后续同一次获授权循环不重复问。最多10次push；第10次拒绝后源码仍执行update但不再push。更新ResolvedConflicts/Incomplete也停止自动推送，不能仅凭isSuccess判断重试。

新增纯策略模块 `push_recovery.rs`：after_push与retry_after_update，输入明确区分source/current/tracked目标身份、force、结果和零基attempt。没有执行命令或暗含用户许可，尚未接入Push主流程；后续仍需更新流程编排、Merge/Rebase/Cancel弹窗、记住设置、rebase-over-merge警告及source新OID刷新。上游多仓更新在本产品单仓范围尚无对应实现，不宣称完整移植。

同时修正现有push结果解析：此前遇到第一个匹配拒绝立即返回，会漏掉后续pushurl的custom rejection。现在继续读取所有匹配记录，custom优先于stale，stale优先于non-FF，成功不能盖掉拒绝。新增真实Git阶段：第一个远端non-FF，第二个远端拒绝更新当前分支，返回RejectedOther并核对两处tips不变；之前部分成功场景仍通过。网络错误与多URL逐项完整诊断仍待，不能将该类别聚合当作完整多URL更新安全证明。

Linux no-default-features全套通过（72单元、64repository、8认证及其他套件），包含策略边界和现有真实Git用例扩展；证据 `artifacts/push-recovery-policy/`。未改UI或发布包，Push当前仍是单次尝试，不会因新增策略模块而自动更新/重推。

Windows原生多pushurl扩展用例通过（6.61s）。仅使用临时本地仓库，未连接真实远端或启动GUI。

## F3 进展：Merge updater 命令步骤（2026-10-16）

完整读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GitMergeUpdater.java，核对 GitImpl.merge。更新用 merge --no-stat -v <跟踪目标>，不是普通Pull选项路径；冲突交resolver，已解决返回ResolvedConflicts、未完成返回Incomplete；覆盖本地/未跟踪文件报Error。取消使用reset --merge。isSaveNeeded优先检查暂存改动，再与远端变动路径求交；该保存决策本轮尚未翻译。GitSingleRepoUpdateTest关键词匹配主要是fast-forward优化和RESET更新，不能冒充当前merge步骤用例已完整移植。

`update_session::Session::merge` 接入现有串行session，检查attached/目标引用/无未完成操作/编辑器，防重复merge或交叉rebase；使用完整refs/remotes路径和--参数隔离、应用提供GIT_EDITOR、局部C locale。MergeStep保留命令、实际state和取消reset结果；分类为Success/Incomplete/Error/Cancelled，不将有未完成状态的成功退出直接视为可重推。此处读取真实state是本地保守适配；自动resolver及merge完成仍需后续编排。运行中取消沿上游reset --merge、用新token刷新；本轮没有直接触发该分支，不能宣称取消验收完成。

真实Git场景覆盖快进、分叉双父merge、内容冲突Incomplete、本地覆盖Error和启动前取消；断言HEAD/父节点、无关本地改动、覆盖失败文件以及重复调用被拒绝。测试编辑器使用true模拟接受默认说明，不代表应用编辑器端到端。Linux no-default-features全套通过（72单元、65repository、8认证及其他套件）。证据 `artifacts/merge-updater/`。尚无fetch/save-needed/update/retry完整编排，无新UI或发布包。

Windows原生上述五种模式通过（9.43s）；没有连接网络、使用用户凭据或启动GUI。

## F3 进展：Merge updater 运行中取消实证（2026-10-16）

补充上一节未验证的 GitMergeUpdater.cancel/reset --merge 分支。新增真实Git集成 `merge_updater_running_cancel_records_reset_outcome_and_preserves_local_work`：分叉合并进入pre-merge-commit hook，输出确定性标记后watcher才取消；不使用任意延迟假定操作已开始。两种模式分别允许reset、由fixture预建index.lock阻止reset。测试断言Stopped=Cancelled、reset独立退出结果、HEAD不移动、无关未暂存内容/未跟踪文件保留；reset成功时远端新增文件和暂存条目消失，reset失败时锁文件及已经写入的incoming文件保留，禁止假称已回滚。hook及子进程由既有进程树管理取消，测试不删除未知锁或执行reset --hard。

Linux定向测试通过（0.53s），证据 `artifacts/merge-cancel/`。此次只新增故障注入集成用例，未改变实现，因此不重复上一轮已通过的完整回归；测试覆盖原生命令取消，仍不是未来Push更新UI取消流程的验收。保留运行中失败的结构化结果供后续通知接线，尚无UI或发布包更新。

Windows原生同两模式通过（4.84s）；未启动GUI、联网或操作用户仓库。

## F3 进展：Merge updater 保存判断（2026-10-16）

沿已读GitMergeUpdater.isSaveNeeded，读取同一参考commit的GitUtil.hasLocalChanges/getPathsDiffBetweenRefs：暂存diff非空即保存，否则本地affected paths与当前分支/目标分支端点diff路径求交。新增Repository::merge_update_save_needed，只读检查attached目标、暂存diff和tracked状态（不枚举untracked），远端路径用--name-only -z，保留Git默认rename行为；比较本地前后路径，Windows按小写字符串、其他平台按Path比较。读取沿现有输出上限，取消返回错误；其他读取失败保守返回需要保存。GitUtil内部diff命令失败实际返回empty，这里选择保守true而非冒充无重叠，作为明确本地差异；真正执行前仍由session再次检查仓库状态，不能将true视为操作有效性证明。

真实Git测试覆盖干净、仅untracked、无关unstaged、任意staged、重叠unstaged、staged rename、目标引用消失及取消，并确认查询不创建stash、不修改本地内容。Linux完整no-default-features回归通过（72单元、67repository、8认证及其他套件）；随后将状态枚举改为untracked=no，定向新增用例再次通过。Windows路径大小写/特殊Unicode大小写和远端rename与本地原路径重叠的边界尚未专门覆盖，不宣称所有路径等价语义已验证。证据 `artifacts/merge-save-decision/`。此规则尚未连接完整fetch/update/retry控制流程，无UI或发布包更新。

Windows原生同用例通过（3.97s），只使用临时本地仓库，无网络/GUI操作。

## F3 进展：单仓Merge更新流程串联（2026-10-16）

进一步读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GitUpdateProcess.update/updateImpl/fetchAndNotify 和 GitUpdater.isUpdateNeeded/hasRemoteChanges。顺序为检查仓库/跟踪配置→仅fetch跟踪远端→重新读取配置→rev-list -1 HEAD..target判断是否需要更新→保存判断/保存→Merge updater→按结果恢复。保留compoundResult实际恢复条件，而非方法旧注释“错误也恢复”的描述。

新增 `merge_update.rs` 单仓worker流程。Request捕获branch/HEAD/target/index恢复选项，fetch前后复核身份；fetch使用已有认证可选接口，返回后才获取update session锁避免重入死锁。Report分别保留fetch/save/merge/restore、剩余stash、错误及最终HEAD/state。NothingToUpdate不创建stash；Fetch失败NotReady且不改工作区；Merge冲突Incomplete保留stash。ready_to_push额外要求恢复成功/无剩余stash/实际attached且无未完成操作和冲突，避免“merge成功但恢复失败”继续推送。该门槛是本地适配，不把上游单一UpdateResult当完整恢复证明。

真实临时仓库六模式：无远端新提交、快进并保留无关unstaged、保存staged后双父merge并恢复index、merge内容冲突、stash恢复冲突、fetch目标不可用。断言实际parents/HEAD/文件/暂存与stash；远端均本地fixture。Linux no-default-features全套通过（72单元、68repository、8认证及其他套件）。证据 `artifacts/merge-update-flow/`。测试编辑器为true，不代表应用helper认证/编辑端到端；应用editor请求server向merge命令的配置仍需接线。尚未接Push拒绝弹窗及重试循环、Rebase同级流程、多仓、冲突resolver，发布包未变。

Windows原生六模式同用例通过（25.48s），仅测试二进制，无GUI/用户仓库操作。

## Merge 更新编辑器请求桥接（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/commands/GitImplBase.java::runCommand／prepareGeneralPurposeEditor`：启用交互回调的命令在执行范围内准备认证管理器和 `GitSimpleEditorHandler`，随命令结束关闭。编辑是否触发由 Git 命令及配置决定。

本地 `src/update_session.rs::merge_with_requests` 在 merge 子进程期间启动已有请求服务并配置子进程环境，结束后释放，再执行取消恢复／状态读取；`src/merge_update.rs::execute` 将调用方 Requests 从 Fetch 阶段传至 Merge 阶段，沿用现有 editor helper 与 Document 校验。保留无请求服务的底层 merge 入口供外部编辑器／测试使用。没有强制打开说明编辑器或改变默认 merge 参数。

`tests/editor.rs::merge_update_routes_editor_and_preserves_saved_changes_on_rejection` 使用本地 remote、分叉历史、真实 helper 和测试仓库 `branch.main.mergeOptions=--edit` 触发编辑：保存多行中文说明验证双亲、暂存修改恢复及允许继续 Push；拒绝编辑验证 HEAD 不变、MERGE_HEAD 保留、stash 内容保留、禁止继续 Push，并验证请求已清理、操作取消 token 未误触发。

验证：Linux 编辑器 5 项串行通过（1.52 秒）；原有六场景 Merge 流程集成测试通过（2.80 秒）；Windows 原生编辑器 5 项通过（13.07 秒），脚本与输出在 `artifacts/merge-update-editor/`。初次 Linux 并行运行中已有 helper 拷贝／启动测试出现 `Text file busy`，串行运行通过，但该并发测试稳定性尚未解决。初次新用例未配置 `--edit` 而没有产生编辑请求，修正测试触发条件后通过。

本轮无可见 UI 改动，未启动 GUI 或更新便携包。尚需接入 Push 拒绝后的 Merge／Rebase 选择及更新重试 UI，不能将此后端桥接视为完整 Push 恢复验收。

## Rebase 开始／继续的编辑器请求连接（2026-10-16）

继续读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/update/GitRebaseUpdater.java`、`plugins/git4idea/backend/src/rebase/GitRebaser.java::continueRebase` 和 `GitRebaseProcess.java::isRebasingPublishedCommit／askIfShouldRebasePublishedCommit`：更新开始前需已发布提交判断；继续时为 `rebase --continue` 准备 editor manager，取消继续不自动 abort。无变更时的 stage／skip 恢复是另一个处理分支，尚未接入本地流程。

本地 `src/update_session.rs` 增加 `rebase_with_requests` 和 `continue_rebase_with_requests`，为子进程配置现有请求服务，命令结束关闭，再读取状态或执行已有取消处理；保留无请求服务的入口。此为编辑器生命周期连接，不是完整 Rebase 更新调度，调用方仍需先做 published 判断及用户决策。

新增真实 Git 测试 `tests/editor.rs::rebase_continuation_editor_can_reject_then_retry_without_losing_resolution`：构造分叉冲突并先保存暂存修改；测试代码解决并暂存冲突后，真实 helper 首次取消说明编辑，断言 HEAD、已暂存解决内容、rebase 状态和保存修改均保留；再次继续时保存多行中文说明，断言新提交父节点、说明、rebase 结束，并恢复原暂存内容／清理已保存 stash。冲突解决仅由测试 fixture 执行，不表示产品已实现解决编辑器。请求取消不误取消操作 token，结束后无待处理请求。

Linux 6 项 editor 测试串行通过（2.02 秒），原有 clean／abort／continue 集成回归通过（0.98 秒）；Windows 原生 6 项 editor 测试通过（17.45 秒），证据 `artifacts/rebase-update-editor/`。本轮没有可见 UI 改动或便携包更新。完整 Rebase 更新的 fetch／快进优化／保存／published 决策／执行／恢复调度及 Push 拒绝恢复界面仍待接入；沿用前节记录的 Linux 并发 helper 测试稳定性待办。

## Rebase 更新执行链与决策顺序（2026-10-16）

本轮重读参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/update/GitUpdateProcess.java::updateImpl／tryFastForwardMergeForRebaseUpdaters`、`GitRebaseUpdater.java::isSaveNeeded／doUpdate`、`GitRebaseOverMergeProblem.java` 及 `plugins/git4idea/backend/src/rebase/GitRebaser.java`。顺序为 Fetch、重新检查跟踪配置、判断是否有更新、对有 tracked changes 的仓库尝试快进、rebase-over-merge 决策、保存修改、已发布提交判断及确认、Rebase、按结果恢复。已发布确认取消发生在保存之后，因此保存内容保留，不自动 pop。

新增 `src/rebase_update.rs::execute` 单仓后台执行链，复用 `merge_update::Request／validate`、Session、Fetch、published 查询及 editor 请求服务。Report 保留 fetch、快进、save、published 查询、Rebase、restore 的结果和最终实际 HEAD／状态；只有完整成功且无保留 stash／进行中操作／冲突才允许后续 Push。UI 等待结束后再检查源 HEAD／分支／tracking；网络过程不嵌套持有 Session 锁。`Repository::has_tracked_changes` 使用不枚举 untracked 的 NUL status 查询；读取失败时不做快进优化，保存判断保守为 true，取消仍停止。

必要接口边界：`before_save` 是调用方必须提供的 rebase-over-merge 检查／决策入口，false 停止本次 Rebase，可由上层转到 Merge；`confirm_published` 仅在已发布匹配或检查未知时调用。回调需支持取消，且不能调用另一项持有队列锁的操作。尚未实现 rebase-over-merge 的本地精确检测／选择弹窗，也未把该执行链接到 UI，不能以测试中无 merge commits 的直接放行回调替代产品实现。自定义 protected 规则由调用方提供，不在此重写设置语义。

新增 `rebase_update_flow_preserves_changes_and_obeys_decision_order` 十场景真实 Git 测试：无更新、带暂存修改快进、clean／saved Rebase、冲突、已发布取消／接受、published 查询未知、保存前取消、Fetch 失败。断言实际父节点、文件／index／stash、门控调用顺序、取消后 HEAD 和是否允许 Push。Linux 通过（6.06 秒）。

尚需补齐：具体 rebase-over-merge 与 published UI、无变更 stage／skip 后续处理、保存失败／恢复冲突及中途取消的编排层覆盖、等待决策时目标／protected refs 变化的校验、完整 Push 重试整合。原有底层相关测试不替代这些编排验收。本轮无可见 UI 变更，不更新便携包。

Windows 原生同一十场景集成测试通过（53.61 秒），脚本与结果在 `artifacts/rebase-update-flow/`；Linux 原有 Merge 更新六场景回归通过（2.84 秒）。这是执行层验证，不是界面或完整 Rebase 功能验收。

## Rebase-over-merge 检测与执行链输入（2026-10-16）

沿参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 读取 `plugins/git4idea/backend/src/update/GitRebaseOverMergeProblem.java::hasProblem` → `history/GitHistoryUtils.java::history` → `history/GitLogUtil.java::readFullDetails` → `history/GitCommitRequirements.kt::DEFAULT`，以及 `GitCommit.java`。判定是 base..current 中 merge commit 的 combined changes 非空，默认参数为 `-M -c`，不能用“存在 merge 节点”或第一父提交 diff 替代。这个条件也不等于检测任意语义冲突。

本地 `Repository::rebase_over_merge_problem` 固定当前 HEAD 与远端目标 OID，以 `log --merges -c -M --format= --name-only -z` 进行存在性查询；不加载提交说明和 patch，沿用读取层输出预算及取消。`rebase_update::execute` 在快进尝试后、保存前执行查询，Report 保留结果，必需的 before_save 回调接收 Result<bool, String>，调用方据此决定继续／停止。确认界面和转为 Merge 的上层调度仍未接入。

明确差异：上游 hasProblem 在 VcsException 时返回 false，且 history 可能保留部分已读提交；本地读取失败／超限保持 Err，传给决策调用方，不当成已确认无风险。这是有界加载／错误可见性的适配，尚无 UI 默认处理策略。默认 rename detection 参数遵循源码，但罕见 rename／octopus merge 的完整对照仍待覆盖。

真实 Git 用例 `rebase_over_merge_checks_combined_changes_not_ordinary_merge_nodes` 覆盖普通独立文件合并不触发、合并中额外文件触发、冲突解决内容不同于两父提交触发、目标已包含 merge 不触发，另验证取消／缺失目标报错、HEAD 与工作区不变。普通 merge 明确存在第一父 diff，从而排除错误判据。Linux 通过（0.91 秒），更新后的十场景 Rebase 编排测试通过（6.44 秒）。本轮没有 UI 变化或便携包更新。

Windows 原生新检测用例通过（8.71 秒），构建成功，脚本与输出在 `artifacts/rebase-over-merge/`；未把源码层通过称为选择弹窗或整个 Push 恢复已完成。

## 更新决策期间的远端引用变化保护（2026-10-16）

基于前述已核对的 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` 更新与确认顺序，本轮补充本地并发适配，不将它描述为上游已有的原子保证。`merge_update::remote_refs／validate_remote_refs` 在 Fetch 后记录按 refname 排序的 remote refs 名称与 OID，沿用读取输出上限。Rebase 在保存前决策结束后、published 确认结束后检查引用清单；Merge 在保存后执行 merge 前检查，同时重查源 HEAD／tracking／操作状态。变更报错，保持真实状态供刷新，不继续使用旧决策。

当前以整个 refs/remotes 清单作为判断依据，因此无关远端引用变化也会停止，属保守适配；目标、受保护分支移动／删除和新出现引用均不能静默沿用原确认。该机制没有跨进程原子锁，不保证发现改后又改回的 ABA，也不能消除最终检查与 Git 读取 ref 之间的竞态；命令固定目标 OID／快进过程竞态仍需后续处理。

真实 Git 测试 `rebase_update_rejects_changed_refs_after_decisions_without_losing_saved_work` 六场景，在 before-save／published 两个回调中分别修改 target、protected 或新增 remote ref。断言 Error、禁止 Push、没有执行 Rebase、HEAD／操作状态保持；保存前 index／文件不丢失，保存后 stash 内容仍保留且不自动 pop。Linux 新增六场景及既有十场景执行链测试通过（合计 6.54 秒）；随后新增 ready_to_push 断言纳入 Windows 构建验证。本轮没有可见界面变更，未更新便携包。

Windows 原生六场景变化保护测试通过（38.82 秒），脚本与结果在 `artifacts/update-ref-decisions/`；Linux 原有六场景 Merge 流程回归通过（3.16 秒）。界面决策与完整 Push 重试仍未接入。

## Push 拒绝后的更新／重试编排（2026-10-16）

重读参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/push/GitPushOperation.java::execute／findRootsWithMergeCommits／showDialogAndGetExitCode／update`：非快进拒绝且源为当前分支、目标为 tracking 时才进入恢复；首次且未配置自动更新时询问，将 rebase-over-merge 提示合并到首次选择；已作该选择后 updater 不再次询问同一类提示；更新失败、未完成或已解决冲突时停止；最多十次 Push，第十次拒绝后仍可更新但不进行第十一次 Push。

新增 `src/push_flow.rs` 后台编排，复用普通 Push、push_recovery 策略、Merge／Rebase update workers，通过 Decisions 接口接收拒绝选择及后续警告回答。只有 update.ready_to_push 与重试策略同时允许时，将成功更新后读取的 HEAD 作为下一次 Push 的 source_oid；普通 Push 仍重新校验源。保存最后一次 Push／Update 的有界结果及尝试次数，避免积累十份命令输出。本轮接口传入 auto_update 和 protected targets，尚未接设置持久化或 UI，不能暗示已授权默认自动更新。全流程复用同一 Cancel、Progress 和认证／编辑请求通道。

新增真实 bare remote 测试 `rejected_push_flow_updates_retries_or_preserves_conflict_and_cancel`：Merge／Rebase 均先真实拒绝、选择一次、Fetch 更新、第二次 Push 成功，断言远端 HEAD、新父节点及原暂存修改；取消仅一次 Push，远端／本地 HEAD 不变；Merge 冲突仅一次 Push，保留进行中状态和 stash，未跟踪文件始终保留。Linux 四场景通过（3.76 秒）。

尚未完成：GPUI 决策消息桥、Rebased 三按钮布局／默认焦点／remember 选项与设置规则、auto-update 模式下 rebase-over-merge 的 Merge Instead 分流、protected targets 随 Fetch 及设置变化重建、十次竞争拒绝的真实编排集成、期间取消及所有错误的最终状态呈现。现有十次上限策略单元测试不等于真实网络竞争验证。本轮不改可见界面，不更新便携包。

Windows 原生同一四场景编排测试通过（33.40 秒），构建成功，证据 `artifacts/push-update-retry/`。该结果证明执行链，尚不证明用户能从当前界面触发恢复。

## Push 更新决策的 UI 消息通道（2026-10-16）

完整读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/push/GitRejectedPushUpdateDialog.kt`：按钮顺序 Merge／Rebase（有问题时 Rebase Anyway）／Cancel；无 merge 问题时默认按钮跟随 update method，初始焦点在另一种更新方法；有问题时默认 Merge、焦点 Cancel。remember 只在非 Cancel 的返回路径中由调用方保存。结合已读 GitRebaseOverMergeProblem 和 published warning，分别保留它们的默认／焦点规则，不能统一用“默认按钮自动获得焦点”替代。

新增 `src/update_decision.rs`：有界单请求通道，Worker 实现 push_flow::Decisions，Ui 获取 Pending，回答使用类型化 Action／Answer（含 remember）。Pending 被丢弃视为关闭，后台等待退出；同一 Cancel 取消任务后不接受过期回答；Worker 结束令残留 Pending 失效；published 不接受 Merge，非首次拒绝提示不接受 remember。只有首次明确选择 Merge／Rebase 才保存 accepted 给后续设置层，Cancel／关闭不保存偏好。查询未知用 Problem::Unknown 表达，不在提示中复制原始 Git 输出；当前未知的默认／焦点沿用保守的 Merge／Cancel，此为本地差异。

Linux／Windows 原生各四项通道测试通过（均 0.06 秒）：默认／焦点、明确回答与 remember、关闭／任务取消／过期回答、UI 接收端消失和不适用回答。Windows 脚本与结果 `artifacts/update-decision/`。本轮未实现可见 GPUI 弹窗，也未改变 start_push，因此当前客户端仍不能触发完整更新重试；尚未进行截图验收或更新便携包。

待接线：实际三按钮与 checkbox、焦点和 Enter／Esc、i18n、主 Push 任务服务此通道及 editor 请求、结束时关闭自身弹窗、accepted 设置保存规则。自动更新过程的 OverMerge→Merge Instead 尚无 worker 方法切换实现，通道明确返回错误而非谎称已切换；接入该按钮前必须补齐，不能发布无效按钮。

## Rebase 更新中 Merge Instead 的执行分流（2026-10-16）

落实前述已读取的 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` `GitUpdateProcess.java::updateImpl` 规则：rebase-over-merge 决策发生在保存之前，MERGE_INSTEAD 替换 updater，随后按 Merge 的 isSaveNeeded／执行规则继续，不先执行 Rebase 或重新 Fetch。此次只切换当前更新方式，不改用户偏好。

`rebase_update::execute` 的 before_save 从 bool 改为 `Result<Option<Method>, String>`，明确区分 Rebase／Merge／取消。选择 Merge 后用 merge_update_save_needed 判断保存，调用同一 Session 的 merge_with_requests，Report 新增 merge 结果，跳过 published rebase 确认，再沿用 finish 与 ready_to_push 规则。`push_flow::Decisions`、`update_decision::Worker` 同步传递三种选择，移除此前 Merge Instead 未接入的错误路径。

真实 Git `rebase_update_merge_instead_uses_merge_save_rules_and_preserves_history` 三场景均构造含额外修改的 merge（检测必须为 true），然后选择 Merge：无关 unstaged 不保存、staged 保存后恢复、冲突时保留 stash。断言从未执行 Rebase／published 确认、原 HEAD 保持为第一父、远端为第二父、merge-only 内容保留，以及冲突不允许 Push。Linux 通过（2.71 秒）；原有 Rebase 十场景与决策引用变化六场景回归通过（合计 6.51 秒）。更新决策通道五项单元测试通过（0.06 秒），新增三种回答映射。

本轮无可见 UI 改动；GPUI 三按钮／checkbox／默认与焦点、start_push 与请求服务接线、设置持久化及原生弹窗验证仍待完成。未更新便携包。

Windows 原生 Merge Instead 三场景通过（24.51 秒），脚本／结果 `artifacts/update-merge-instead/`；Linux 既有 Push 拒绝恢复四场景回归通过（3.79 秒）。这些是执行层证据，不表示弹窗已可用。

## Push 恢复弹窗与主任务接入（2026-10-16）

依据前述已读 Rebased `4aec35256d765c217aa0d662b82ac4e860c396fd` GitRejectedPushUpdateDialog／GitRebaseOverMergeProblem／published warning，实现 `src/update_dialog.rs` 并在 `src/ui.rs::start_push` 接入 push_flow。拒绝提示提供 Merge／Rebase／Cancel，按状态切换 Rebase Anyway；published 提示为 Rebase Anyway／Cancel。默认样式与初始键盘选中分别来自已验证模型，支持左右／Tab／Shift+Tab、Enter、Esc；关闭请求会令后台退出等待。当前键盘焦点在容器内用 selected 管理，非各 Button 的独立可访问焦点，仍需无障碍与默认按钮语义进一步对齐。

主 Push 任务同时服务认证、editor Document 和 update_decision；结束关闭残留决策弹窗、刷新真实仓库。新增取消更新／更新未完成状态以及中文提示资源。初次构建修复 focus 借用冲突，最终 Windows release 构建成功。首次 UI 版本固定 auto_update=false，未显示 remember checkbox，未写更新策略偏好；此为未完成结构项，不归为纯像素差异。protected targets 暂用启动 Push 时已知的 main／master 集合，Fetch 后变化与自定义设置仍待完善。

Windows 原生 fixture `GitReview-validation/push-recovery-ui`，测试 PID 34256：真实点击 Push→非快进拒绝→中文三按钮弹窗（decision.png，Merge 蓝底、Rebase 初始选中圈）→Cancel，cancel.png 确认退出并显示已取消推送更新，Log 仍为原本三条提交；重新 Push→选择 Merge→自动 Fetch／Merge／重推→completed.png 显示推送完成、四条历史和合并图。verify.ps1 验证本地 HEAD 等于 bare remote main 且有两个父提交，结果 assertions.json。截图与脚本在 `artifacts/push-recovery-ui/`。已关闭自己的测试进程，启动会话确认退出；未操作用户 Rebased／IDEA，未做滚动测试。截图中的内存仅启动采样，不作为内存验收。

未完成：Rebase／published／Merge Instead 的 GUI 路径、键盘和任务取消时弹窗清理的原生验证、remember 设置联动、分支名／详细解释与警告图标、弹窗位置及视觉细化、错误详情及保存 stash 的可恢复入口。认证／editor 接线本轮只编译，未在 Push UI 中触发这些额外交互；不能沿用后端测试代替 UI 验收。便携包未更新。

## Push Rebase 原生键盘验证与 Enter 修复（2026-10-16）

沿用上一节已核对的 Rebased 拒绝对话框／选择规则，使用独立 `GitReview-validation/push-rebase-ui` fixture：本地与远端分叉，base.txt 有暂存修改。原生三按钮提示中 Right 将选中从 Rebase 移到 Cancel，Left 返回 Rebase（right.png／left.png）。首次 Enter 未执行：通用 Dialog 的确认动作先接管按键，而原 on_ok 返回 false，导致 capture_key_down 的 Enter 分支不足以提交。Git 检查确认仍为原本地 HEAD，未推送，暂存内容未变；enter-before-fix.png 保留失败证据。

`src/update_dialog.rs::show` 将 Dialog.on_ok 接到当前 selected action，沿用 answer 的一次性 Pending 消费与关闭逻辑，返回 false 防止框架再关闭一次。Windows release 重新构建成功，关闭旧测试窗口 34340，确认会话退出后复制新 exe，以 32628 在同一未修改 fixture 重测。初始 Rebase 选中时，通过目标 HWND 的 WM_KEYDOWN／UP Enter 确认，随后自动 Fetch／stash／Rebase／restore／Push 完成；completed.png 显示线性三提交与推送完成。

verify.ps1 断言本地 HEAD 等于 bare remote main、仅一个父提交且父标题为 incoming、base.txt 仍在 index 中且内容为 valuable staged、无 stash、无 rebase-apply／rebase-merge 目录。assertions.json 和脚本在 `artifacts/push-rebase-ui/`。只关闭本轮两个测试进程，最终启动会话确认结束。未动用户窗口，未做滚动或内存验收；未更新便携包。

本轮验证了 Rebase 成功路径、暂存恢复及 Enter 事件修复；尚未覆盖 Esc／Tab／Shift+Tab、不同 selected 下 Enter、published／Merge Instead／冲突的 GUI 路径和取消任务时弹窗清理。容器 selected 焦点仍是明确的可访问性差异，remember 设置工作仍待完成。

## Push 更新偏好模型与默认策略校正（2026-10-16）

本轮读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/shared/src/git4idea/config/GitVcsOptions.kt`、`GitVcsSettings.java`，重读 `plugins/git4idea/backend/src/push/GitPushOperation.java::savePushUpdateSettings／readPushUpdateSettings／showDialogAndGetExitCode`：默认 UpdateMethod.MERGE、auto update=false；BRANCH_DEFAULT 才解析分支配置。确认后更新 auto-update checkbox 值，即使未勾选也会保存 false；Cancel 不改设置；保留 BranchDefault，或在 rebase-over-merge 警告情况下保留原策略。

新增 `src/update_preferences.rs`，提供 Merge／Rebase／BranchDefault、resolve、accepted 和有界版本化文件读写。取消保持全部偏好；未知检查沿警告规则保留原策略，为本地保守差异。严格读取最多 128 字节，缺失／损坏／未知版本／重复字段回退 Merge + 不自动更新。文件接口接受明确路径，尚未选择实际仓库级存储位置；遵循上游 workspace 范围，不能把某仓库的自动更新选择写成全局授权。

`push_flow::execute` 改接 Preferences，显式策略不读取分支配置；`start_push` 暂传默认值，校正此前无条件解析 branch/pull.rebase 的偏差。尚未接 checkbox、accepted 保存、设置页面和实际 load，因此当前仍每次询问。读写函数仅在测试临时文件上使用，未修改用户设置。

Linux 三项偏好测试通过；Windows 原生三项通过（0.07 秒），证据 `artifacts/update-preferences/`。Linux Push 恢复四场景回归通过（3.70 秒），用例额外设置 pull.rebase=true，断言默认仍为 Merge，再按用户实际选择执行 Merge／Rebase／Cancel。可见布局未改，无新截图或滚动测试；便携包未更新。

Windows release 构建通过，确认 Preferences API 已正确接入桌面主任务；未将编译结果作为 checkbox／持久化 UI 验收。

## Push 更新偏好的 worktree 存储范围（2026-10-16）

延续已读 Rebased GitVcsSettings 的 workspace 级存储语义，本地单仓应用采用每个 worktree 的 Git 管理目录存放私有偏好。新增 `update_preferences::for_repository`，调用现有 `Repository::editor_directory`（`rev-parse --absolute-git-dir`）定位 `git-review/push-update-preferences`。普通仓库使用自身 .git，linked worktree 使用专属管理目录，不使用共享 common-dir。此为本地目录映射差异；不把自动更新选择保存到全局 layout preference 或受版本控制的文件。

解析路径／读取不创建目录或写文件，只有显式调用 save 才保存。新增 `push_update_preferences_are_private_per_worktree_and_not_inherited_by_clone` 真实 Git 用例验证：首次读取不创建路径；保存后往返；Git config、HEAD、原工作区 status 不变；linked worktree 默认不继承并可独立保存；新 clone 不复制选择；取消查询报错且既有设置不变。Linux 通过（0.17 秒）。

检查现有 Settings 页面仍只有语言与 Diff 偏好，下一步需增加当前仓库的更新方式／自动更新入口，以及拒绝弹窗的 remember checkbox 和成功选择时的保存。当前 for_repository 尚未在主 Push UI 调用，因此本轮未启用自动更新，也未修改用户仓库设置。无可见 UI 变动，不更新便携包。

Windows 原生同一存储隔离集成测试通过（1.16 秒），脚本与输出在 `artifacts/update-preferences-scope/`。没有 GUI 启动或内存采样。

## 确认更新选择时的偏好保存（2026-10-16）

在已核对的 Rebased GitPushOperation 保存时序上，`update_decision::Worker` 新增 `set_preferences(path, previous)`：后台收到明确 Merge／Rebase 回答后、返回执行链之前，按 Preferences.accepted 保存；Cancel／关闭不进入写入。持久化失败独立保存在 persistence_error，仍返回本次选定 Method，避免把写设置失败当作用户取消或改变更新方式。该错误尚须由主 UI 单独展示。

新增通道测试在临时目录中实际写读：Cancel 不改变自动更新状态；Merge／Rebase 的 remember=true 保存；有 merge 警告时临时 Merge 仍保留原 Rebase 偏好；把目标设置路径设为目录制造失败，断言错误可见且本次 Rebase 选择保留。Linux 六项决策通道测试通过（0.06 秒），Windows 原生六项通过（0.07 秒），证据 `artifacts/update-decision-save/`。

同时读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/config/GitVcsPanel.kt` 中 cdAutoUpdateOnPush、Push 分组和 Update 分组：自动更新是 Push 下 checkbox，更新方式是 Update 下 radio group。下一步据此组织设置控件，而非把所有选项散放在恢复弹窗。

当前桌面调用方尚未配置 Worker.set_preferences，也未提供 remember checkbox／仓库更新设置控件；这轮仅完成确认→保存的后台连接，不声称用户已能记住选择。未修改用户偏好，未改可见 UI、未启动窗口或更新便携包。

## Push 记住选择与 Git 设置原生联动（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/push/GitRejectedPushUpdateDialog.kt::createDoNotAsk`、`GitPushOperation.java::savePushUpdateSettings/readPushUpdateSettings` 及 `plugins/git4idea/backend/src/config/GitVcsPanel.kt`：恢复弹窗提供 remember，明确选择后保存；设置按 Push checkbox 与 Update method radio 分组。保留前述取消不保存、临时 Merge 不覆盖策略和 BranchDefault 规则。

`src/update_dialog.rs` 接入首次拒绝提示的 remember checkbox；`src/ui.rs::start_push` 在后台读取当前 worktree 偏好并配置 Worker，push_flow 使用该偏好。持久化错误通过 Report.preferences_error 单独返回，在操作状态后提示设置未保存，不将本次明确选择误作取消。`src/update_settings.rs` 新增 Git 设置内容：Push 自动更新 checkbox、Update 下 Merge／Rebase／跟随分支配置 radio，OK 写入、Cancel 丢弃草稿，写入错误留在界面；中英文集中在 i18n。Settings 入口限制宽度并保留在左侧，修正最初横跨全页的问题。

Windows release 构建通过（最终布局调整 12.19 秒）。原生证据 `artifacts/push-remember-ui/`：PID29740 首次拒绝勾选 remember 后 Merge／重推成功，实际偏好为 merge/true；设置取消草稿不改变文件，关闭自动更新并选择 branch-default 后保存、重开正确显示，assertions.json 校验文件。关闭该进程后以新构建 PID31752 重开同一 fixture，确认跨进程偏好保持及收窄后的布局。通过第二个本地 clone 制造新远端提交，本地另提交形成分叉；UI 将偏好改为 merge/true，再 Push，仅点击推送按钮后完成 Fetch／Merge／重推，未再回答恢复对话框。second/auto-final.png 与 second/assertions.json 验证本地等于远端 HEAD、双亲分别为 second ours 和 second incoming、工作区干净。首次 verify-auto.ps1 仅等4秒，任务仍在运行便断言失败；未重启任务，稍后 verify-auto-final.ps1 在同一次运行完成后通过，保留两次证据。

已关闭自有29740／31752并确认启动会话结束，未操作用户 Rebased／IDEA，无滚动测试、发布包更新或总体内存结论。剩余结构差异：Git 设置暂以 Settings 内嵌套弹窗承载，仍需迁入统一设置分类页；恢复弹窗 checkbox 尚未纳入容器自定义 Tab 遍历；默认按钮／可访问焦点和警告图标等继续对齐。功能待验收：自动 Rebase／BranchDefault 的原生路径、失败保存提示、取消任务弹窗清理及 protected targets Fetch 后刷新。不以本次 Merge 自动路径代表全部 Push 恢复场景完成。

## 自动 Rebase 在 Fetch 后解析默认保护分支（2026-10-16）

重新读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/GitProtectedBranches.kt` 全文及 `rebase/GitRebaseProcess.java::checkForRebasingPublishedCommits/isRebasingPublishedCommit`：按当前 repository remoteBranches 和保护设置判断是否包含待重写提交；这不是 Push 开始时固定的目标列表。原本 src/ui.rs 在首次 Push 前枚举 main/master，随后 Fetch 即使发现新保护分支也不会纳入 published 检查，存在漏提示。

`published_commits::Protection` 区分 Defaults 和 Explicit；Defaults 在 rebase_update 已完成 Fetch、保存本地修改后进入 published gate 时解析当前远端目标，再执行既有祖先关系检查。主 Push 使用 Defaults，移除 UI 预先枚举；Explicit 保留给明确提供保护目标的调用方和已有测试。查询错误继续作为 unknown 交确认，既有远端引用变更检查仍覆盖等待用户决策的区间。此次仅实现默认 main/master 规则的刷新；自定义正则／托管平台规则与设置 UI 仍待，不宣称完整移植保护设置。

新增真实 Git 测试 auto_push_rebase_discovers_protected_branches_created_before_fetch：本地不知道的新远端 master 包含 ours，main 已有 incoming，自动 Rebase 的 Fetch 后必须发现 master。取消断言一次 Push、未运行 Rebase、HEAD／远端 main 不变且 staged 内容在 stash；确认断言两次 Push、新 HEAD 父为 incoming、远端一致且 staged 恢复；新增普通 review 分支不触发提示。Linux 三场景通过（4.20秒），既有 Rebase 更新／引用变化／Merge Instead 三项回归通过（6.48秒）。Windows release 构建通过（18秒）。本轮未修改可见布局，无新 GUI 截图或便携包更新。

Windows 原生同一新增用例三场景通过（39.86秒），证据 `artifacts/push-protection-refresh/{tests.ps1,windows-result.txt}`。Linux 既有 Push 拒绝恢复四场景回归通过（3.78秒），并改用实际 Defaults 策略覆盖主调用方式。此为执行层证据，published 弹窗的 Windows GUI 路径仍需独立验收。

## 已发布提交 Rebase 弹窗原生验收与取消状态（2026-10-16）

重读参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` `plugins/git4idea/backend/src/rebase/GitRebaseProcess.java::askIfShouldRebasePublishedCommit`：Rebase／Cancel 两按钮、默认和初始焦点均为 Cancel、警告图标。现有本地两按钮和默认选中模型一致；warning icon、两段解释、标题仍有差异，本轮不宣称像素一致。

`artifacts/push-published-ui/` 新建 Windows 本地 bare remote fixture，预置应用偏好 rebase/auto-update=true（这轮不重复设置 UI）。远端 main 与本地分叉，master 发布本地 ours，但本地开始时尚无 origin/master。PID25884 实际 Push→Fetch→中文 published 提示：取消按钮默认蓝底并选中；目标 HWND Enter 取消，cancel-assertions.json 断言 HEAD 和远端 main 不变、工作区干净。再次 Push 后 Left 将选中移至“仍然变基”，Enter 确认；complete-assertions.json 断言新 HEAD 与远端 main 一致、唯一父为 incoming、master 保留原 ours、工作区干净；completed.png 为最终状态。首个请求不因 auto-update 而绕过 published 确认。

原生发现取消结果被 UI 归入通用“更新未完成，请检查冲突”。`push_flow::Update` 增加 cancelled／has_saved_changes 查询，`src/ui.rs` 在非任务取消且更新结果 Cancelled 时显示“已取消推送更新”；若 Report 仍持有 saved_oid，显示“本地修改仍保存在 stash 中”，不掩盖已保存工作。任务主动取消的远端结果不确定提示保留，错误／冲突仍走未完成状态。集中新增中文资源。

Windows release 重新构建通过（15.93秒），Linux 自动 Push Rebase 三场景回归通过（4.21秒），新增断言验证取消与保留 stash 的查询。新 PID28144 在独立 `artifacts/push-published-status-ui/` fixture 复测 Enter 取消，cancelled-fixed.png 显示“已取消推送更新”，仓库不变断言通过；保留 stash 的状态文字只验证到执行层标志，未在本轮 UI 触发。已关闭自有25884／28144并确认启动会话结束，未触碰用户 Rebased／IDEA。未更新发布包或测试滚动；尚待 Merge Instead 的原生路径、checkbox 键盘遍历、统一设置分类页和保存工作恢复入口等。

## Push 记住选择键盘路径与 Tab 事件修复（2026-10-16）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GitRejectedPushUpdateDialog.kt::createDoNotAsk/setDefaultAndFocusedActions，保留 remember 与默认／初始选中分离。读取本地 gpui-component Checkbox／Button 的 tab_stop、focus handle、键盘激活路径；控件层 action 可先于容器原始 KeyDown 回调处理 Tab/Space，不能仅靠 capture_key_down 管理选中索引。

首轮仅扩展 selected 的 Tab 循环，Windows fixture PID27468 实际复现失败：Tab 把真实焦点交给 Button，手动选中仍留在 Rebase，空格误触更新并保存未勾选偏好；sequence.ps1 的“Esc 不应生成偏好文件”断言失败，证据 `artifacts/push-remember-keyboard/`。按 diagnosing-bugs 以同一目标 HWND 按键脚本作回归；仅在本地 bare fixture 中触发，未操作用户仓库。

`src/update_dialog.rs` 新增 PushUpdate 专属 NextControl／PreviousControl／ActivateControl action，在 main 初始化绑定 Tab／Shift+Tab／Space；在现有容器焦点模型中统一遍历按钮与 remember，禁用内部控件另行 tab_stop，避免双重焦点与激活路径。remember 行显示选中圈，空格只切换勾选；按钮空间激活当前选项，Enter 保留当前按钮确认，选中 remember 时落到默认 action；Esc 仍取消。左右方向仅在按钮区域生效。鼠标勾选同步手动选中与容器焦点。

最终 Windows release 构建通过（11.58秒）。新 PID31068、独立 `artifacts/push-remember-keyboard-fixed/`：相同 Tab→Tab→Space→Esc 路径通过，space-checked.png 显示 remember 勾选与选中圈，Esc 未生成偏好；重开 Tab→Tab→Space→Tab→Enter，assertions.json 验证 merge/auto-update=true，实际合并双亲与本地／远端 HEAD 一致。已关闭自有27468／31068，启动会话结束。此为原生按键回归，没有用纯索引单测替代事件分发测试。未更新发布包。

保留差异：仍使用容器选中模型，尚非每个控件独立的可访问焦点；Shift+Tab 已接线但本轮未单独原生验收，remember 上 Enter 的默认按钮语义仍需与安装版 Rebased 对照。checkbox 选中圈、警告图标和对话框像素细节待视觉阶段，不宣称完全还原。

## 自动更新 Merge Instead 与 BranchDefault 原生验收（2026-10-16）

重新读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/update/GitRebaseOverMergeProblem.java`，以及 `plugins/git4idea/shared/resources/messages/GitBundle.properties` 的 rebasing.merge.commits 项：独立警告标题 Rebasing Merge Commits，按钮实际为 Merge／Rebase／Cancel（不是枚举名称 Merge Instead），默认 Merge、初始焦点 Rebase。`src/update_dialog.rs` 对 OverMerge 使用对应标题，并把此前 Rebase Anyway 标签改为 Rebase；Rejected 和 Published 保留各自标签规则；中文标题加入 i18n。正文仍为本地简化解释，警告图标和精确布局未完成。最终 Windows release 构建通过（15.97秒）。

原生 fixture `artifacts/push-merge-instead-ui/`：新建 side 分支后 merge，额外加入 merge-only.txt；远端 main 有分叉 incoming，本地 base.txt 有 valuable staged 暂存修改。预置应用 branch-default/auto-update=true 和 branch.main.rebase=true，以实际主任务验证跟随配置走 Rebase 更新。PID31048 Push 后进入独立 OverMerge 警告，decision.png 确认标题、三按钮、Merge 蓝底和 Rebase 选中圈；Esc 取消，cancel-assertions.json 验证 HEAD／远端不变、暂存内容原样、未生成 stash（警告在保存之前）。

再次 Push，目标 HWND Left 选择 Merge、Enter 确认，实际保存／Merge／恢复／重推完成。complete-assertions.json 验证新 HEAD 等于远端 main，双亲分别为原本含额外修改的 merge 和 incoming；merge-only.txt 内容仍在新提交中，base.txt 暂存内容恢复且无残留 stash；应用偏好仍 branch-default/true，Git branch.main.rebase 仍 true。completed.png 显示完成状态及保留 merge 拓扑。已关闭唯一自有31048并确认启动会话结束；未操作用户 Rebased／IDEA、未做滚动或新内存验收，便携包未更新。

此次覆盖自动 BranchDefault→Rebase→Merge Instead 的成功／取消路径，不等同于选择 Rebase 后可能丢弃 merge-only 修改的完整警告验收，也不覆盖冲突 UI。正文／警告图标、统一设置页、保留 stash 的查看恢复入口和更多失败状态仍待。

## 统一分类设置页与草稿生命周期（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/platform-impl/src/com/intellij/openapi/options/newEditor/SettingsEditor.java` 左 treeView／右 ConfigurableEditor＋banner／splitter 构造，以及同目录 SettingsDialog.java::createActions/applyAndClose/doCancelAction：左分类右内容，OK 在 apply 成功后关闭，Cancel 放弃未应用内容，Apply 留在窗口。沿此前 GitVcsPanel 已核对的 Push／Update 分组组织 Git 内容。

`src/ui.rs` 将 Settings 改为左侧 Language／Version Control 下 Git、Diff，右侧对应内容，共用底部 OK／Cancel／Apply。移除“设置内再开 Git 弹窗”，`src/update_settings.rs` 只提供内容 Entity 和保存方法。语言／Diff 从立即保存改为当前设置会话草稿，与 Git 一并在 Apply／OK 保存；Cancel、Esc、切出设置均丢弃未应用草稿。Apply 后再 Cancel 保留已写值。保存失败不关闭，Git 错误切回 Git 页显示；跨配置文件按顺序写，非原子事务，后项失败时前项可能已应用。

Git 页按需异步读取、重复点击不重复请求；回调检查仓库路径、设置会话编号与打开状态，关闭／重开／切仓后不把旧结果挂到新页。关闭释放内容 Entity，不增加常驻设置缓存。设置打开时禁用顶部 Git 写操作按钮，避免本应用 Push 保存偏好与编辑中的草稿竞争。loading、无仓库和操作中提示提供中英文；这些异常状态尚未全部原生验收。

Windows 原生 PID31064、`artifacts/settings-categories/`：使用独立 LOCALAPPDATA 配置目录，不改用户日常偏好。依次修改语言、Git 自动更新、Diff 位置，取消后断言三处文件均未创建；重新修改并 Apply，断言 language=en、diff-location=bottom、auto-update=true，applied/git-persisted 截图确认英文与统一分类布局。再次取消未应用的关闭自动更新草稿，保存值保持 true；最后 OK 保存 false 并关闭，ok-closed.png 确认英文主界面与底部 Diff 生效。verify.ps1／assertions.json／ok.ps1 保存证据。已关闭自有进程并确认启动会话结束，未做滚动测试或更新发布包。

结构保留差异：当前设置占主内容区而非上游独立设置窗口；分类是小型固定导航而非完整可搜索树，尚无搜索／可调分栏／Reset，导航行的对齐、字体、密度和圆角仍待视觉阶段。Apply 暂未按 dirty 状态禁用；加载失败重试需再次选择 Git。保存失败、切仓竞态、窄窗与完整键盘／无障碍路径待进一步验收，不将上述成功路径等同完整设置验收。最后补齐 loading 与 Diff 保存失败中文文案后重新构建，不重复成功路径截图。

## 设置修改状态与失败重试原生验收（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/platform-impl/src/com/intellij/openapi/options/newEditor/ConfigurableEditor.java::init/updateCurrent`，以及 SettingsEditor.java 的 ConfigurableEditor.apply 覆盖：Apply 初始禁用，由修改状态启用；仅遍历已修改配置，成功清除修改标记，失败定位对应分类并不关闭。

`src/update_settings.rs::Settings` 保存读取／成功写入后的 saved 基准，modified 比较草稿；未修改直接返回成功、不调用文件写入，成功写入更新基准，失败保留草稿与基准。`src/ui.rs` 汇总语言、Diff 和 Git 修改状态禁用 Apply，并观察 Git 内容 Entity 的通知让父页及时更新按钮。OK 在无修改时仍可关闭。只改其他分类不把 Git 的旧快照重新写回，避免覆盖外部新设置；尚未对用户确实修改过的 Git 草稿实施外部写入冲突检测。

Windows release 构建通过（11.82秒）。原生 PID29356、隔离 LOCALAPPDATA 的 `artifacts/settings-save-errors/` fixture：以普通文件占据 .git/git-review 目录位置制造保存错误。未修改 Git 页 Apply 禁用，OK 正常关闭且不触碰阻断文件；再修改英语和 auto-update，从语言页 OK 后自动切到 Git，显示实际写入错误、页面未关闭、auto-update 草稿保留，语言文件未被提前写入。确认阻断文件身份后移除它并点击 Apply，两项草稿成功保存，错误消失且 Apply 禁用。只切换 Git checkbox 时 Apply 立即启用，切回基准禁用，截图 git-only-dirty/reverted-clean。

进一步在测试文件中模拟外部将 Git 偏好改为 rebase/false；只把语言切回 system 并 Apply，断言外部 Git 值未被覆盖。exercise.ps1、retry.ps1、assertions.json 和截图保存上述证据。已关闭自有29356、确认启动会话结束，无用户配置改动或发布包更新。无 Linux UI 测试；此次使用真实 Windows 文件失败和控件路径，不以纯 modified 比较测试代替 UI 验收。跨文件部分成功、Git 加载后外部改变的可见刷新、窄窗和无障碍仍待。

## 当前设置版本 Windows 内存基线复验（2026-10-16）

使用包含统一设置分类页及修改／失败重试处理的现有 Windows release，SHA256 `837C1F36A532CD3E1631F88DC8F30D6117DF49128B8ABFCD3A373BDA8E6AE504`。启动脚本 `artifacts/current-settings-memory-launch/run.ps1` 将该构建复制到宿主测试目录，隔离 LOCALAPPDATA，使用宿主 Git；串行启动空窗口和完整 Diff 两个实例。沿用 windows-job-memory.ps1 的挂起启动→加入 Job Object→恢复执行，进程组峰值包含从启动起的 Git 子进程，稳定态各采样20次，GPU单列。未改产品代码，无需新增上游行为映射。

第一次预检发现 full-context-memory-fixture 实际为10001个提交（原10000条上另加中间行修改提交），在启动应用前退出；核对 HEAD `480ebb280b4c8f75fb8e63725f4e0c762560ed78`、仅 review.rs 一行增一行删、工作区干净后修正精确数量断言。文件975000字节，启动历史上限10000。history.png 实际显示已加载10000条；opened.png 确认独立双栏 Diff Tab 从文件首行展示未修改上下文，而非仅变更片段。

| 场景 | 稳定 Private Bytes（应用＋Git） | Job进程组峰值 | 稳定 Working Set（应用） | GPU独立／共享 |
| --- | --- | --- | --- | --- |
| 空窗口 | 53.27 MiB | 55.66 MiB | 67.40 MiB | 17.37／1.09 MiB |
| 10000条历史＋975000字节完整文件 Diff | 105.77～108.04 MiB | 144.85 MiB | 104.09～106.13 MiB | 55.40／5.07 MiB |

证据分别位于 `artifacts/current-settings-welcome-memory/`、`artifacts/current-settings-diff-memory/` 的截图、memory.json、steady.json。两场景预算断言通过（空窗口80MiB、Diff稳定150MiB、峰值250MiB）；稳定采样时Git子进程占用为0，Diff进程组生命周期共计73个进程、结束采样时仅应用存活。GPU是单次计数器读数，不与Private Bytes相加，也不是GPU峰值。该复验不是冷缓存启动计时、不代表滚动性能、连续切换压力或网络／写操作峰值验收；F4相关待办保留。

测试进程7740、32992均已退出并由宿主查询确认；未关闭用户窗口，steady.json所列其他审阅进程为空。未更新发布包。此记录仅说明当前两个固定场景仍在内存预算内，不宣称总体功能或内存验收完成。

## 当前版本连续提交选择的取消与内存回落（2026-10-16）

沿用上一节构建、10001提交fixture／10000行历史上限及975000字节文件；`artifacts/current-settings-memory-launch/selection.ps1` 在宿主隔离配置启动一次应用，以15／50／150ms间隔各投递100次首两提交往返（共600次选择请求），每批等待Job中所有Git子进程退出，最后重新打开原文件完整Diff并采样20次。不是滚动测试，也不是100个不同文件、20刷新、10重开验收的替代。

`artifacts/current-settings-selection-memory/selection-cycles.json`：三批分别启动1184／1204／1797个子进程，停止后均只剩应用，应用Private Bytes分别93.08／93.21／94.01MiB。整个Job峰值237.93MiB，单进程峰值148.60MiB；快速请求时多个进程重叠的峰值明显高于上一节静态浏览。重新打开相同完整Diff后稳定109.36～114.71MiB，最终109.36MiB，启动时Diff基线108.83MiB；Working Set107.49～112.75MiB，采样时Git子进程0，GPU独立55.40／共享5.00MiB。稳定150MiB／峰值250MiB断言通过，但峰值余量仅约12MiB，不能据此认定所有压力场景均安全或完全没有泄漏。

检查selection-15.png的最终提交／文件及selection-final.png的完整双栏Diff，确认脚本当前坐标仍命中预期目标；进程启动计数证明真实查询发生，不证明每个快速请求都完成解析或绘制。截图、memory.json、steady.json与上述分批数据保留。测试PID15268已退出并由宿主查询确认，未关闭用户窗口、未更新发布包。后续仍需不同文件／刷新／重开及网络写操作内存验收；若其他负载越过预算，应定位并发查询生命周期，而非调高预算。

## 当前版本文件列表／刷新／重开压力与验收范围修正（2026-10-16）

`artifacts/current-settings-memory-launch/combined.ps1` 使用相同 release SHA，宿主 GitReview-100files fixture（10000提交，当前历史窗口1000条），隔离配置。CombinedStress 依次投递100次文件导航（首次Up留在首项，后续Down）、20次刷新点击、通过原生目录选择器重新打开同仓10次，最后取消一次目录选择。中间截图switch-49.png确认选中file-048.rs，returned.png确认回到同仓首提交、checkpoint.txt与1000条历史。未做滚轮或滚动性能排查。

发现旧脚本在默认独立Tab模式下仅改变文件选择，未打开Diff；src/ui.rs::load_file在bottom_preview=false时不读取Diff。因此本轮仅为列表导航、刷新请求和仓库替换证据，不计作100个不同文件内容／缓存生命周期验收。scripts/windows-stress.ps1及windows-job-memory.ps1补充明确说明；后续报告附combinedStressNote，PowerShell语法解析通过。本次运行在补说明前已启动，原始memory.json保留，不回填修改。

证据 `artifacts/current-settings-combined-memory/`：列表导航应用Private Bytes73.78～75.10MiB，刷新阶段74.37～76.05MiB；重开10次采样79.43～81.62MiB（非单调），最终20次稳定采样80.76MiB，初始74.25MiB。Job进程组峰值159.72MiB、生命周期883个进程，采样结束仅应用存活。GPU独立55.39／共享2.13MiB单列。相比初始仍多约6.51MiB，本次短序列不能证明长期无增长；也不能由20次点击证明20次异步刷新全部完成。1000条窗口及无打开Diff的负载不能替代10000条＋完整Diff预算场景。

PID25552已退出并由宿主查询确认，未关闭用户进程或更新发布包。下一项应补默认独立Tab内真实文件Diff导航与加载验收，再评估重复循环的回落趋势；保留F4完整压力验收未勾选。

## 独立 Diff Tab 的100文件真实加载往返（2026-10-16）

为补上一轮仅列表选择的缺口，scripts/windows-job-memory.ps1 新增互斥模式 DiffNavigationFiles：先打开独立Diff，依次点击下一文件，再反向回到第一文件；每一步要求Job出现新子进程并等待仅剩应用，额外等待UI处理结果，超时直接失败，按步记录内存并定期截图。按钮坐标只针对当前中文／150% DPI／100文件fixture，不是通用跨语言UI测试。没有更改应用逻辑。

`artifacts/current-settings-memory-launch/file-diff.ps1` 使用当前release及宿主GitReview-100files，加载10000条历史；fixture.json确认当前提交恰有100个变更文件。先显示checkpoint.txt的1/100，99次前进到review.rs的100/100，再99次返回checkpoint.txt的1/100。所有198次导航均启动3个Git子进程并结束；diff-forward-99.png确认975000字节新增文件的真实内容，diff-backward-99.png确认返回原文本对比。不是仅发送方向键或读取列表，也没有滚动自动化。截图抽查不等于逐文件像素验收。

`artifacts/current-settings-file-diff-memory/` 保存diff-navigation.json、fixture.json、memory.json、steady.json与阶段截图。首次小文件Diff基线79.31MiB；导航阶段应用Private Bytes采样最大127.43MiB；返回同一小文件后20次采样82.19～85.94MiB、最终82.19MiB（比基线高2.88MiB）。完整Job峰值136.03MiB，生命周期667个进程，最终仅应用存活；稳定阶段Working Set81.02～84.71MiB，Git子进程占用0，GPU独立55.40／共享3.40MiB。脚本150／250MiB断言通过。此为一次完整往返，不能仅凭约3MiB残差就认定长期泄漏，也不能宣称多轮稳定性已经验证。

PID34260已退出并由宿主查询确认，未关闭用户窗口或更新便携包。当前完成独立Tab真实100文件导航的验收证据；F4网络写操作及多轮组合负载仍待，不据此勾选总体内存验收。

## 保留 stash 的查看数据层（2026-10-16）

读取参考commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/stash/GitChangesSaver.java::notifyLocalChangesAreNotRestored`、`GitStashChangesSaver.java::showSavedChanges/load`、`stash/ui/GitStashUIHandler.kt::showStashes`：未恢复提示带查看动作，进入Stash工具窗口／旧版Unstash对话框；查看不是自动恢复。读取 `GitStashUtils.kt::loadStashStack/loadStashChanges` 及 `GitStashCache.kt`：列表记录hash、parents、作者时间、reflog selector与subject；工作区变化相对第一父提交，另保留index和可选untracked提交，不把stash作为普通merge只显示单份内容。

新增 `src/stash.rs` 并导出：list限制1～1000条，额外读一条判断more，复用Git读取输出限额与取消；UTF-8＋NUL字段替代上游冒号分隔，subject仍按首个冒号拆分branch提示与message，保留message内冒号。Entry保留完整parents，selector仅用于显示；contents按不可变OID重新读取真实提交及parents，工作区Details和额外父提交Details分开返回，复用现有完整Diff读取入口。未预加载全部stash内容，无新增常驻缓存；与上游缓存／后台预加载方式不同，服务低内存约束。当前超过1000条只明确more，尚未做翻页UI。

真实Git测试 `stash_view_keeps_identity_index_and_untracked_content`：空仓列表／非法limit；中文含冒号消息；同一tracked文件的index与working版本不同，加未跟踪文件后stash -u；列表截断more；新增后续stash导致旧selector位置变化时，按原OID仍得到正确三类内容；读取前后stash列表／工作区不变；提前取消列表与内容读取均失败。首次测试发现snapshot_commits传空roots会返回空历史，修正为实际OID roots后Linux通过（0.34秒）；Windows原生同用例通过（3.00秒），证据 `artifacts/stash-read-model/{tests.ps1,windows-result.txt}`。

本轮仅实现后续查看入口所需数据层，未新增可见Stash Tab、通知链接或手动恢复动作；无GUI验收、release更新或便携包更新。下一步继续对照SavedPatchesUi布局接入查看界面，再按上游Unstash动作与冲突规则完成恢复；不将数据读取测试当作用户恢复流程完成。

## Stash 查看面板及独立 Diff 接入（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/savedPatches/SavedPatchesUi.kt` 构造／updateLayout：保存记录树与changes browser共享分隔布局，选中记录更新文件，双击／Enter打开编辑器Diff；`plugins/git4idea/backend/src/stash/ui/GitStashProvider.kt::toChangeObjects/getDiffPreviewTitle` 将主变化与带parent tag的index／untracked变化一起展示，并为stash Diff提供专用标题。延续上一节未恢复通知的查看入口规则。

新增 `src/stash_view.rs`：左记录、右分组文件，共享36逻辑像素工具栏及可拖分隔；虚拟化记录／文件行，按选中记录异步读取，generation与Cancel防止旧结果覆盖新选择，切仓释放实体。工作区／暂存区／未跟踪分组复用数据层；显示差异按钮、双击、Enter／Ctrl+D打开现有独立Diff Tab，文件方向键跳过分组标题。内容缓存仅保留当前记录的文件元数据，Diff文本由活动Tab按需持有。刷新保持选中OID，重新进入面板刷新；空态、数量截断提示与主要标签加入中英文。

`src/ui.rs` 增加Stashes入口，主刷新路由到当前面板，打开stash Diff按完整Target identity区分同名文件的不同保存版本。Push报告保留stash、Smart Checkout恢复失败有saved OID时提供“查看已保存的更改”入口；尚未原生触发这两个通知入口的端到端路径，不能算已验收。该提示标记目前直到切仓才清除，不是实时stash存在性追踪；普通Pull链路仍需审查入口覆盖。

Windows release最终构建通过（12.58秒）。临时仓库 `artifacts/stash-view-ui/`、PID31568：保存tracked.txt暂存版index version、工作区版working version及untracked.txt；loaded.png确认三组文件，working-diff／index-diff／untracked-diff分别确认相对base、相对base、根提交新增内容正确；assertions.json确认stash仍一条、工作区干净。最后补全行宽与重新进入刷新后，PID8024在final目录验证点击工作区行→Down跳过分组标题→Enter，keyboard-index-diff.png实际打开index version。两个自有进程均正常关闭、启动会话结束；无滚动测试或发布包更新。

保留结构／交互差异：单仓平面保存列表及分组文件尚无目录折叠、分支与日期标签、搜索、底部动作区；Stashes入口暂常显。Diff沿用Repository Diff标题，缺上游stash名称／parent tag及关闭后的返回焦点，需要补齐；文件链暂限各分组，未跨组导航。左树键盘导航、文件越界可见定位、双击／Ctrl+D原生路径、空态／失败／快速切仓、通知入口端到端仍待验收；这里只验证按钮与Down＋Enter路径。尚未提供Apply／Pop等恢复动作，不宣称完整恢复功能或像素级还原完成。

## Stash 显式 Apply／Pop 执行层（2026-10-16）

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/stash/ui/GitStashActions.kt`：标准Apply／Pop分别调用unstash的pop=false／true，reinstateIndex均为false；Unstash As另提供keepIndex。重读GitStashUtils.kt::createUnstashHandler/unstash：执行原生stash apply或pop及可选--index，index conflict独立报错，普通冲突转冲突处理，其余覆盖／失败显示诊断；没有成功时不能静默删除stash。读取tests/git4idea/stash/GitStashTest.kt，现有测试主要校验列表hash、selector、branch、time与message，未将其称为恢复冲突覆盖证据。

新增 `src/stash_operation.rs`：Action::Apply／Pop与独立reinstate_index参数；使用已有common-dir操作锁、有界输出及取消执行器。执行前验证selector格式与OID，并重新解析selector核对当前OID；若旧stash@{n}已移位，拒绝并要求刷新。这是本地防误操作适配，应用内锁不是跨Git客户端原子锁，仍有外部修改发生在预检后、命令前的竞争窗口，未宣称已消除。直接执行原生pop，未自行拆成apply＋drop。

Report保留命令诊断、停止原因和执行后重新读取的真实仓库状态；取消／失败不能假定回滚。状态区分成功、工作区冲突、index冲突、本地修改覆盖、未跟踪覆盖、取消和通用失败。实际untracked同名错误可能落通用Failed，仍保留输出，不伪称已恢复。没有自动继续、自动覆盖或自动删除动作；界面尚未接线。

真实Git测试explicit_stash_apply_pop_preserve_index_and_failures共9种情况：Apply保留stash、Pop成功移除、--index恢复独立暂存版本；普通冲突与index冲突分别判定并保留stash；脏tracked文件／同名untracked原内容保留且stash保留；selector移位与提前取消不修改工作区或stash。成功用例验证HEAD不变、工作区版本／未跟踪内容及暂存版本符合各选项。Linux通过1.48秒，Windows原生通过14.04秒，证据 `artifacts/stash-restore-model/{tests.ps1,windows-result.txt}`。测试只在临时仓库写入，无产品GUI改动、发布包更新或恢复按钮验收。后续需接入底部动作区、操作中禁用／取消、刷新与冲突引导，并补执行中取消及外部竞争处理。

## Stash Apply／Pop 原生界面与冲突反馈（2026-10-16）

进一步读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/savedPatches/SavedPatchesComponents.kt::buildBottomToolbar/createBottomComponent`：底部左对齐Apply主按钮、Pop次按钮、顶部边线和帮助说明；位于changes browser下方。按此结构在src/stash_view.rs右侧文件区底部加入Apply／Pop，tooltip说明保留／成功后移除stash，均传reinstate_index=false。尚无Unstash As的单独选项页。

Restore事件交给Review::restore_stash，复用全局写任务、Cancel和状态栏；本应用其他写操作／设置打开时禁用恢复按钮，执行入口再次检查，避免重复启动。执行完成后读取Report真实状态、刷新stash与主仓数据；失败展示有界Git诊断（stderr＋stdout，最多8000字符）和本地更改入口，取消文案明确先检查实际文件。为单仓轻量UI采用内嵌诊断／本地冲突列表，尚非上游冲突解决弹窗；未增加自动解决、暂存、重试或删除。

Windows release构建通过13.23秒。原生PID26680、`artifacts/stash-restore-ui/`临时仓库：点击Apply恢复working version和saved untracked，stash仍一条，index保持base；将已核对内容的测试fixture恢复原状后点击Pop，内容恢复且stash清空、按钮禁用。另造stash与新HEAD同一行冲突，点击Pop后断言unmerged index存在、stash仍一条；conflict-result.png显示中文冲突提示与Git原始诊断，本地更改按钮实际进入Conflict 1／tracked.txt（local-conflict.png）。assertions.json记录实际Git状态与内容断言。首次exercise.ps1辅助函数命名Git与内部git调用发生PowerShell递归，发生在Apply已成功后的断言阶段；改为git.exe并从实际已恢复状态继续验证，没有重复Apply或重启测试。

已关闭自有26680并确认启动会话结束，无用户仓库修改、滚动测试或便携包更新。普通Apply／Pop成功和普通冲突路径已有GUI证据；执行中取消、并发禁用、stale selector、index冲突、更多覆盖错误的GUI路径及写操作内存仍待。恢复状态与诊断位置是功能阶段适配，帮助图标、原生冲突对话框及像素细节仍未完成。

## Stash Diff 来源标题与返回文件焦点（2026-10-16）

对照参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/stash/ui/GitStashProvider.kt::getDiffPreviewTitle/prepareChangeContext`、shared/resources/messages/GitBundle.properties 与 `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/savedPatches/SavedPatchesEditorDiffPreview.kt::getEditorTabName/returnFocusToTree`：Tab 标题使用 stash 名称与文件名；内容上下文附来源和可选 parent tag；关闭预览将焦点交还保存记录的变化树。

src/diff_tab.rs 新增轻量 StashOrigin，src/stash_view.rs 打开事件携带 selector 和工作区／暂存区／未跟踪分组；Tab caption 与右侧版本标题显示来源。普通 Repository Diff 保持原标题。src/ui.rs 关闭活动 stash Diff 时返回保存面板，focus_after_diff 聚焦文件列表。未增加文件内容缓存，非活动 Diff 继续释放 viewer。上游标题为 selector: filename，本地额外将分组写入标题，以区分当前多个同名文件 Tab；右侧仍保留实际 OID，尚未复刻上游单独 parent tag 组件。

Windows release 构建通过（15.64秒）。沿用已启动的原生 PID1980、隔离配置及临时仓库 `artifacts/stash-title-ui/`，verify.ps1 点击打开 working/index 两个版本；working-diff.png 显示 working version，index-diff.png 与 enter-reopened.png 显示 index version，标题各有中文分组。escape-return.png 确认 Esc 关闭活动 index Tab、返回文件列表且保留暂存区文件选择；不再点击鼠标直接发送 Enter，成功重新打开 index version，证明返回焦点路径有效。finish.ps1 断言工作区干净且 stash 仍为一条，正常关闭自有进程并确认原启动会话结束。未改用户仓库、未做滚动测试、未更新发布包；启动内存读数不作为本版本内存验收。

剩余：关闭非活动 Tab 时现有 focus_after_diff 会转移到隐藏历史焦点，应补齐活动 Diff 焦点保持；本轮只验证活动 Tab 的 Esc 返回。selector 是打开时快照，外部重排后标签不自动更新，实际内容仍按 OID 固定；文件链仍限分组。工作区 Diff 左栏 First parent 尚未中文化，原生未覆盖未跟踪标题、普通 Log 返回、鼠标关闭、快速切仓与窄窗口。上述项不计为已完成。

## 非活动 Diff 关闭焦点及版本标题翻译（2026-10-16）

继续对照参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 `platform/platform-impl/src/com/intellij/openapi/fileEditor/impl/EditorWindow.kt::closeFile/computeIndexToSelect`：关闭非活动文件时保留当前选中文件。结合上一节 SavedPatchesEditorDiffPreview 的返回树规则，本地 focus_after_diff 先检查是否仍有活动 Diff；存在则通过 DiffTab::focus_viewer 聚焦已有 viewer，不重新加载；否则才回到 stash 文件树或历史列表。原代码在关闭非活动 Tab 后仍将焦点交给隐藏历史列表。复用该 focus_viewer 到文件选择器关闭路径，未改变加载及缓存生命周期。

独立 Diff 和底部预览的 First parent、Empty tree、Empty file、Index／Working tree、Staged、Unmerged 等版本标签接入现有 i18n；补充第一父提交、空目录树、空文件、未合并、未跟踪的工作区文件和已暂存翻译。仅翻译显示名称，revision identity 和 Git 查询不变。

证据 `artifacts/diff-close-focus/`：before PID8860 与 after PID27900 都使用上一节的已知临时 stash fixture 和隔离配置。exercise.ps1 打开工作区、暂存区两个 Diff，关闭后台工作区 Tab；after/inactive-closed.png 确认仍显示暂存区 Tab 和 index version，working.png 确认“第一父提交”中文。随后 Esc 返回保存文件列表、Enter 再次打开暂存版本（escape-return／enter-reopened.png）。首轮尝试 Ctrl+A 时前台检查失败，脚本拒绝发送到其他窗口；没有获得全选的修复前／后判据，不能将本轮称为焦点全选回归通过。当前焦点修复由调用路径检查支持，窗口截图只能证明选择保持及 Esc／Enter 返回路径，Diff 专用组合键仍待原生验证。未为此简单分支增加镜像实现的单元测试或调试日志。

首个 Windows release 构建通过21.83秒，原生验证后补齐 Staged 翻译再次构建。两个自有进程均通过校验可执行路径后正常关闭，启动会话均结束；没有操作用户窗口、执行恢复命令、做滚动测试或更新发布包。底部模式各版本文案、英文回切、普通 Log 的多 Tab 与窄窗口尚待验收；不宣称完整键盘或像素级验收完成。

## 显式 Pull Rebase 的所选目标执行入口（2026-10-17）

审查当前调用链发现 PullDialog 仍未开放 Rebase 选项，start_pull 仅调用 pull::execute_command；该函数明确拒绝显式 Rebase。已有 rebase_update 执行器用于 Push 拒绝后的更新，要求目标等于 tracking 分支。因此不能直接开放菜单并将任意 Pull 选择传给旧入口，也不能用原生 git pull --rebase 替代上游更新流程。

读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/actions/GitPull.java::perform/performRebase`：显式 REBASE 分流到 GitUpdateExecutionProcess，使用当前本地分支与对话框所选远端分支构造 GitBranchPair，不要求所选分支等于 upstream；未附着分支时不启动。读取 `backend/src/pull/GitPullOption.kt` 的 REBASE 与 FF_ONLY／NO_FF／SQUASH／NO_COMMIT 互斥规则；现有 Options 已覆盖，不另造规则。读取 `tests/git4idea/update/GitSingleRepoUpdateTest.kt` 的本地提交＋脏工作区保存、无本地提交快进优化、干净工作区不保存三项，沿用现有更新链，不重新实现 stash／fetch／rebase。

src/rebase_update.rs 增加 execute_selected，和旧 execute 共用 execute_validated 的完整处理链；仅将各个执行前／决策后检查绑定到新的 merge_update::validate_selected。新检查要求当前分支、固定 HEAD、所选远端分支存在且仓库无未完成操作；不修改 tracking 配置。旧入口仍要求 tracking 精确相等，Push 的保护不放宽。validate_source 复用 HEAD／操作状态检查；远端引用快照检查、取消、保存与恢复逻辑均保留。此为单仓适配，应用内串行锁仍不承诺与外部 Git 客户端原子隔离。

真实 Git 测试 explicit_pull_rebase_uses_selected_target_without_rewriting_tracking 使用临时本地 bare remote、发布工作仓和消费工作仓，共5种情况：无 upstream、upstream 指向 main 但选择 topic、决策期间 HEAD 改变、目标引用被删除、Rebase 内容冲突。验证旧 Push 入口拒绝非 tracking 目标；新入口成功后 HEAD 父提交是 topic 的新提交，原 tracking 配置不变，暂存与未暂存版本分别恢复、未跟踪文件保留、远端不被推送修改；过期决策在 stash 前拒绝；冲突保留真实 unmerged 状态和 stash 两种文件版本。首次断言错误要求冲突期间 HEAD 仍附着，修正为检查原分支的 upstream 配置；不改实际 Git 行为以满足错误断言。Linux 新测试通过4.98秒，原有 rebase_update_ 三项回归通过7.16秒。

本节仅补齐执行器入口，Pull 菜单和异步 UI 分流尚未接入，不称为显式 Pull Rebase 已可用。下一步固定确认时源分支／HEAD，复用 update_decision 的 over-merge／published 提示与认证／编辑器交互，展示冲突及保留 stash 的查看入口，再完成 Windows 原生端到端验收。普通 Pull 的原生 Git autostash 保留情况需另行核实，不由本节 API 推断已覆盖。

Windows 原生新用例通过47.49秒，证据 `artifacts/pull-selected-rebase/selected-windows.txt`；同目录 tests.ps1 串接共用执行链的3项回归。另核对 GitStashChangesSaver.java::load：上游在 staging area 可用时使用 pop --index；后续本地 Pull 接线应沿用已支持暂存区显示的 reinstate_index=true，保留分开的 index／working 版本。读取同文件冲突通知确认有 Show Stash 和 Resolve Conflicts 入口，本地应复用已保存更改面板，不把恢复失败只显示成“Pull 完成”。本轮未改可见 UI、未启动应用窗口、未构建新 release 或更新便携包。

Windows 共用 rebase_update_ 三项回归全部通过72.25秒（regression-windows.txt），测试进程已结束；覆盖保存／快进／发布确认顺序、决策后过期 refs 拒绝及改选 Merge 的保存规则，未将它们等同尚未接入的 Pull GUI 验收。

## 显式 Pull Rebase 界面接线与原生成功／冲突验证（2026-10-17）

继续对照参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GitPull.java::perform/performRebase，读取 `plugins/git4idea/backend/src/pull/GitPullDialog.kt::createOptionsDropDown/getOptions/isOptionEnabled/createPanel`：命令行式远端／分支选择下方放修改选项入口，Rebase 在选项菜单中，与不兼容选项互斥；显式选择交给更新流程。沿用既有 update_decision／update_dialog 中 over-merge 和 published 提示，不为 Pull 另造风险决策流程。

src/pull_dialog.rs 开放 --rebase，沿用 Options 的互斥规则；未附着分支或没有可用 HEAD 时禁用该选项，普通 Pull 仍可用。src/pull.rs 新增 Source/source，打开对话框时读取并固定分支及 HEAD；ui.rs::show_pull 将其随所选目标交给后台，execute_selected 在执行前和决策后复核。打开后外部修改源时拒绝旧操作，属于本地防过期适配；不是跨客户端原子快照。普通 Pull 保持原 execute_command 路由。

ui.rs::start_pull 加入更新决策通道，后台显式 Rebase 使用 execute_selected、默认 published 保护及 reinstate_index=true；UI 循环处理认证、Git 编辑器和风险确认，操作结束关闭仍活动的对话框。结果区分取消、冲突、未完成操作、stash 保留及完整成功；stash 保留时打开已有“查看已保存的更改”入口。完成后刷新真实仓库状态和历史，没有 Push 重试、静默强制操作或自动解决冲突。新摘要加入中英文。原始详细错误仍未在该入口完整展示，不能将短摘要当作诊断功能完成。

Windows release 最终构建通过26.77秒。新增真实 Git source 用例覆盖 unborn、已附着提交、detached 与提前取消，Linux通过0.18秒、Windows原生通过1.90秒，证据 artifacts/pull-rebase-ui/source-windows.txt。确认查询不修改 HEAD／工作区；该测试不代替不同状态下的禁用按钮 GUI 验证。

原生 artifacts/pull-rebase-ui/ 使用隔离配置、两个专用 fixture，每个含本地 bare remote、发布工作仓及消费工作仓：
- success PID34292：从默认 origin/main 手动选择 topic，再选 --rebase，点击拉取。result.png 显示“拉取完成”、新的线性历史及 origin/topic；verify.ps1 断言 HEAD 的父提交为 topic 新提交，tracking 仍为 origin/main，暂存版本为 staged local、工作版本为 working local，stash 清空、无冲突、远端没有被修改。
- conflict PID5772：本地与远端对 shared.txt 同行提交冲突，另外 local.txt 含不同暂存／工作版本。实际界面显示“拉取后存在待解决的冲突”，Git断言 unmerged shared.txt 和保留的一条 stash，stash 主版本／index父提交分别保留 working local／staged local。点击顶部查看入口后 saved-loaded.png 展示工作区和暂存区两组 local.txt；没有点击恢复按钮。两个 fixture 的 assertions.json、截图及启动／操作脚本保留。

两个自有进程均校验路径后正常关闭，启动会话已结束；未修改用户仓库或操作用户窗口，没有滚动测试、写操作内存验收或便携包更新。结构沿用远端／分支行、选项菜单、确认区及现有通知入口；菜单描述／选项标签可移除形式、选项持久化、对话框尺寸和按钮比例仍与上游不同，未宣称像素还原。此轮仅验证中文成功和普通冲突链路；显式 Pull 的 published／over-merge 原生决策、取消、错误详情、英文／无 HEAD 禁用状态、认证和编辑器交互、冲突后的继续／中止与恢复串联仍需验收及补齐。普通 Pull 原生 autostash 保留反馈仍为独立待办。

## Pull Rebase 已发布提交确认／默认取消／明确继续（2026-10-17）

再次读取参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/update/GitRebaseUpdater.java::checkForRebasingPublishedCommits/doUpdate`、`rebase/GitRebaseProcess.java::askIfShouldRebasePublishedCommit`：在执行 Rebase 前确认已发布提交，按钮顺序 Rebase／Cancel，默认与初始焦点索引均为1（Cancel）。读取 GitUpdateProcess.java 的 preserving process autoLoadDecision 和 GitUpdateResult.java::isSuccess：自动恢复要求没有 incomplete 且结果为 success 类；CANCEL 不在其中，不能由旧注释推断取消后自动恢复本地修改。当前 Session::finish 与此相符。

扩展 artifacts/pull-rebase-ui/start.ps1 的 published fixture：本地 bare remote 的 main 已包含消费仓的本地提交，topic 在另一路前进；消费仓仍跟踪 origin/main，但 Pull 手动选择 topic，并带独立暂存／工作版本。原生 PID36828 使用上一节已测 release、隔离配置。confirmation.png 确认中文“变基将改写已发布的提交”弹窗，“取消”为默认／焦点按钮；waiting-assertions.json 确认等待决策时 HEAD 尚未改变、stash 已保存两种本地版本，未发生 Rebase。

published-cancel.ps1 对该窗口定向发送 Enter，不先点击其他按钮；cancelled.png 确认默认动作取消并提示“本地修改仍保存在 stash 中”，保留查看入口。cancelled-assertions.json 确认源 HEAD 不变、tracking 不变、无 Rebase 标记、工作区干净、stash 仍含暂存和工作版本。这是风险决策取消，不是执行中终止 Git 的验收。

随后同一窗口重新打开 Pull、重新选择 topic／--rebase，再明确点击“仍然变基”。accepted-result.png 确认完成并同时显示本地新提交与 origin/main 的旧提交；accepted-assertions.json 确认新 HEAD 的父提交为所选 topic，远端 main／topic 均未被改写，tracking 仍为 origin/main。重试未创建新的 stash，上次取消保留的 stash OID 及两种内容保持完全一致，没有将历史保留记录误当本次应自动恢复的数据。结果仍保留查看入口；未执行 Apply／Pop。

无应用代码变更，不重复构建或已有数据层测试；本次为真实新增 UI 验收证据。自有36828已校验路径后正常关闭，启动会话结束，无用户仓库／窗口操作、滚动测试或发布包更新。仍需 over-merge 决策、执行中取消、确认窗口关闭路径、英文及未知检查错误状态；不将本轮默认取消／明确继续扩大为所有取消和风险分支已验收。弹窗目前位于内容区上部，标题／提示行数／图标与上游仍有视觉差异，留到结构及视觉细化核对。

## Pull Rebase 合并提交风险与改用 Merge 原生验收（2026-10-17）

参考 `4aec35256d765c217aa0d662b82ac4e860c396fd`，读取 GitUpdateExecutionProcess.kt::launchUpdate/createSpec 确认 GitPull 最后的 false 是 shouldSetAsUpstream，不是关闭风险检查。读取 GitUpdateProcess.java 的 tryFastForward→findRootsRebasingOverMerge→选择 updater→保存顺序，以及 GitRebaseOverMergeProblem.java::hasProblem/showDialog：检测 merge 提交独有变化，按钮依次 Merge／Rebase／Cancel，默认 Merge、初始焦点 Rebase。读取 GitMergeUpdater.java 的 --no-stat -v 合并与 isSaveNeeded 暂存检查；对照 tests/repository.rs::rebase_update_merge_instead_uses_merge_save_rules_and_preserves_history 的既有真实 Git 场景。未根据提示文字自行拼接另一套操作。

artifacts/pull-rebase-ui/start.ps1 新增 over-merge fixture：本地 main 与 side 合并时加入两父提交均没有的 merge-only.txt，remote topic 另有新提交；local.txt 具有独立暂存和工作版本。原生 PID30288 选择 topic／--rebase 后出现中文风险弹窗（confirmation.png），Merge 为主按钮、Rebase 有焦点边框。waiting-assertions.json 确认等待时 HEAD 保持原 merge 提交，暂存／工作修改尚在原处且 stash 为空，证明 UI 决策前没有提前保存。

明确点击 Merge 后，merge-result.png 显示拉取完成和保留合并结构的 graph。over-merge-check.ps1 的 merged-assertions.json 验证新 HEAD 的第一父提交等于原 merge OID、第二父提交等于所选 topic 新提交；merge-only.txt 的独有变化保留；暂存版本 staged local、工作版本 working local 分别恢复，stash 清空、无冲突、tracking 仍为 origin/main，远端 topic 未变。本次 Git 未请求编辑器，不能将成功结果作为编辑器弹窗路径证据。

无应用代码或新 release 变更，本次补齐已接入链路的原生证据。自有30288已校验路径后正常关闭，启动会话结束；未修改用户仓库或窗口、未做滚动／内存测试、未更新便携包。尚未原生覆盖该弹窗的 Rebase anyway、Cancel／关闭及未知检查错误、Merge 后续冲突／编辑器失败；原有数据层覆盖不替代这些 GUI 分支。按钮大体分组和决策顺序已有证据，警告图标、弹窗位置、文案排版仍有视觉差异。

## Pull Rebase 原生执行中取消（pre-rebase hook 阶段，2026-10-17）

重读参考 `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 GitRebaser.java 对 ProcessCanceledException 返回 CANCEL，以及 GitRebaseUpdater.java::doUpdate 对 CANCEL 调用 abortRebase。当前 update_session::rebase_with_requests 在执行器返回 Cancelled 后读实际状态，仅存在 rebase 标记时尝试 abort，再次读取状态；不将终止进程假设为自动回滚。上一节已核对 CANCEL 不自动恢复 stash。

artifacts/pull-rebase-ui/start.ps1 新增 cancel fixture，仍为隔离配置＋本地 bare remote＋两工作仓。在专用 client 的 pre-rebase hook 写 entered 标记并 sleep 60，末尾写 completed 标记；源含本地提交和不同的暂存／工作版本。原生 PID14596 通过真实 Pull 菜单选择 topic／--rebase。entered 标记和实际 Git／sh 进程确认已进入 Git rebase 命令，非仅等待风险弹窗；此时尚未检出重放目标，不能代表重放已开始或 abort 分支已覆盖。

首轮 cancel-running.ps1 的进程树断言误要求 sleep.exe 是当时能从应用 PID 追溯的后代，脚本在点击取消前失败。宿主只读进程查询确认应用14596→Git33100→Git11912→sh28052仍活跃；sleep26700的父进程34856已退出，快照父链断开。随后在同一原生窗口点击顶部取消一次，没有重启或重新发起操作；manual-cancel.png 显示“已取消拉取；本地修改仍保存在 stash 中”。立即查询上述4个观测PID均不再存在，hook completed 标记不存在。修正脚本仅要求可追溯 Git 和 hook shell，记录其PID及创建时间；本轮没有重新运行修改后的完整脚本，不声称修正版已验证。

cancel-check.ps1／cancel/assertions.json 确认 HEAD 和当前分支未改变、upstream仍origin/main、工作区干净，保留的一条 stash 包含 working local 与 staged local 两种内容，hook未自然完成。额外只读检查 rebase-merge／rebase-apply 均不存在、entered存在而completed不存在。这里只证明已观察进程退出，不是从启动开始的 Job 进程组完整计数或峰值内存测试；没有取消时延的可靠测量。测试窗口14596正常关闭且原启动会话结束。

无应用代码变更、release／发布包更新或用户仓库操作；没有滚动测试。尚待实际重放中取消及 abort 成功／失败反馈、网络 Fetch 取消、编辑器等待取消、取消后恢复串联。本轮证明 pre-rebase hook 阶段的执行中取消与数据保留，不能扩展为所有 Git 写入阶段均已验收。

## Rebase 继续／中止：脱离原始会话的执行入口（2026-10-17）

当前任务进入实现，尚未完成 UI 收尾。读取参考 commit `4aec35256d765c217aa0d662b82ac4e860c396fd` 的 `plugins/git4idea/backend/src/actions/GitAbstractRebaseAction.java`（基于 REBASING 状态，支持仓库级后台动作）、`rebase/GitRebaseUtils.java` 的单仓 continueRebase／abort、`GitRebaseSpec.java::forResumeInSingleRepository`（根据实际进行中状态构建恢复 spec）、`GitRebaser.java::continueRebase/handleRebaseContinueFailure`（应用编辑器、原生 continue、取消不等于自动 abort；冲突／no-change 有独立处理）及 `GitAbortRebaseProcess.java::confirmAbort`（单仓无 rollback 时 Abort／Cancel 确认）。源码版本不代表安装版版本一致。

本地新增 `src/rebase_operation.rs`，导出 Continue／Abort 执行器。使用现有仓库操作锁，重新检查 RebaseApply／RebaseMerge 状态；Continue 要求冲突已解决并暂存、编辑器非空，接入已有编辑器请求服务，执行原生 rebase --continue；Abort 执行原生 rebase --abort，调用方必须先确认。执行失败／取消后使用新 token 重新读取实际仓库状态；取消不自动中止或假称回滚。不依赖原 update_session 的 rebase_started，能处理外部 Git 留下的操作。现有 Session 保持原有生命周期，不在持锁状态下嵌套调用新执行器。

测试对照 `plugins/git4idea/tests/git4idea/rebase/GitSingleRepoRebaseTest.kt` 中 continue after resolving conflicts、unresolved conflicts、failure after continue 的场景。新增真实 Git 测试 `existing_rebase_can_continue_or_abort_without_original_session`：外部 Git 构造冲突，新 Repository 执行；覆盖无操作拒绝、未解决拒绝、解决后继续且正确父提交／分支、中止恢复原 HEAD、编辑器失败仍处于 Rebase、提前取消保留状态。每种场景都断言预存 stash OID 和内容不变、未擅自恢复工作区。Linux 通过（1.18秒）；Windows 测试构建通过（2.01秒），宿主 Git 实测通过（11.09秒），证据 `artifacts/rebase-operation/test.ps1` 与 `windows-tests.txt`。仅临时仓库，无用户仓库／窗口操作。

保留差异与下一步：这是执行层增量，尚无新可见入口，不算功能交付或原生 UI 验收。下一步接入工具栏 Continue／Abort、Abort 确认、中英文状态和应用编辑器交互，再做 Windows 原生路径。多次冲突、执行中取消及 apply backend 仍需补充。上游 no-change 的 stage／skip 恢复尚未翻译，本入口暂返回真实失败而不自动 stage／skip；后续须核对当前范围的恢复引导。脱离原 session 后不猜测旧 stash 所有权，不自动恢复；与持有 saver 的上游会话自动恢复存在差异，需通过已有保存修改入口及后续生命周期方案补齐。未构建新 GUI release 或更新便携包。此任务 UI 收尾后按上方优先级进入弹窗与顶部分支 UI/UX 对齐。

## Rebase 工具栏与继续编辑器接线（2026-10-17）

继续读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 `plugins/git4idea/backend/src/ui/toolbar/GitMergeRebaseWidget.kt`：Continue／Abort 属于顶部横向不换行的操作组，继续与中止分别使用运行／停止强调色。本地 `src/ui.rs` 新增实际 Rebase 状态下的 Continue／Abort、后台执行及编辑器请求循环；冲突未解决时 Continue 禁用，中止经过独立确认，运行中禁止重复操作。完成／取消／失败均刷新状态与历史，`src/i18n.rs` 加入中英文摘要。Abort 弹窗居中、含仓库路径和取消／中止，尚待原生验收；当前样式仍沿用通用 DialogFooter，与 JetBrains 紧凑按钮组有差异。

Windows release 构建通过，最终含翻译的构建21.28秒。独立 fixture `artifacts/rebase-operation/start.ps1` 使用外部 Git 创建冲突并暂存解决结果，再启动应用（PID11360），验证没有原 update Session 的继续入口。点击 Continue 实际打开应用提交信息编辑器（ui/editor.png），点击确定后仓库重放完成；verify.ps1 检查 HEAD 第一父提交为 incoming、重新附着 main、无 rebase-merge 标记、index／工作区干净，证据 ui/assertions.json。已核对进程路径并关闭唯一测试窗口，启动会话结束。未操作用户窗口／仓库，无新内存或滚动测试，未更新便携包。

发现必须继续处理的问题：外部 Git 用默认 # 写入的旧冲突说明，当前 Continue 强制 core.commentChar=U+0001，编辑器只过滤 U+0001，因此 # Conflicts 与路径进入最终提交正文（assertions.json 已保留证据）。不能将此次成功退出等同于完整行为验收；下一步先源码核对外部操作的注释兼容规则并修复，再验证 Abort 确认／取消及禁用状态。完成截图捕获于历史异步刷新过程中，仍显示旧 detached 标记，未证明最终 graph 已刷新，需等待稳定重新验证。新增工具栏位置／编辑器行为已原生确认；警告图标、停止色、紧凑按钮组、键盘确认／取消、错误详情等仍待，不能宣称完全还原。

## 外部 Rebase 注释兼容核对与 Abort 原生验收（2026-10-17）

修正上一节“必须修复”的归因：重新读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 `plugins/git4idea/backend/src/commands/GitImplBase.java::loadFileAndShowInSimpleEditor/ignoreComments`，确认只去掉 U+0001 开头行；`GitUtil.java::COMMENT_CHAR` 明确覆盖默认 # 及用户自定义符号，GitImpl.java 的 REBASE_CONFIG_PARAMS 与 GitRebaser.continueRebase 使用该覆盖。最小临时 Git 对照：同样外部启动冲突、解决并暂存，仅改变 continue 时是否覆盖 commentChar；原配置得到纯 local 正文，U+0001 覆盖保留 # Conflicts。结果保存 `artifacts/rebase-operation/comment-comparison.json`。因此现有翻译符合所读源码，但有外部操作兼容限制；未实测安装版 Rebased，不声称其实际版本必然相同。不新增按 # 前缀或英文 Conflicts 字样删行的猜测算法，以免丢失用户提交正文。此限制保留待后续明确兼容方案，未宣称已修复。

独立 Windows fixture `artifacts/rebase-operation/start-abort.ps1` 外部创建未解决冲突，PID5880：confirmation.png 显示 Continue 禁用和居中的中止确认。abort-exercise.ps1 点击取消，断言 HEAD／unmerged 索引不变且 Rebase 仍在；再次明确中止后断言恢复原 HEAD、main、无 rebase-merge、工作区／索引干净。等待3秒后的 aborted-stable.png 确认操作按钮消失、状态“变基已中止”、graph 当前分支高亮与 HEAD／main 图标刷新。取消与完成断言分别归档 abort-ui/cancel-assertions.json、abort-assertions.json。核对路径后关闭自有5880，启动会话结束，保留用户窗口。

当前 Rebase 基础入口已有后端 Linux／Windows 和原生 Continue／编辑器／Abort／鼠标取消证据；不扩大为全场景验收。键盘默认动作、连续冲突、外部注释兼容、无变化恢复和完整错误细节仍列待办。遵循用户新优先级，下一阶段进入现有 UI/UX 结构差异审查和共用弹窗改造，先紧凑按钮组与一致信息层级，再顶部分支菜单；这些改造也覆盖本次 Abort／编辑器，避免把已知整行按钮差异留到最后。功能总清单保持未完整勾选，发布包未更新。

## UI/UX：共用弹窗按钮取消整行拉伸（2026-10-17）

读取参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd 的 `platform/platform-api/src/com/intellij/openapi/ui/DialogWrapper.java::createSouthPanel/createButtonsPanel/layoutButtonsPanel`：左右操作分组，中间 horizontal glue，把右组推至右侧；组内水平 BoxLayout，按钮按自身尺寸与间隔排列。Windows 保留 createActions 顺序，仅 macOS 另排序；本次不改变业务按钮顺序。用户 Update Project 截图用于确认紧凑右组的视觉目标，不作为 Abort／Pull 业务规则。

查明本地 `vendor/gpui-component/src/dialog/footer.rs` 的 DialogFooter 已 justify_end，但 DialogClose 和 DialogAction 包装层 size_full 导致子项铺满整行。两者改为 flex_none，保留 DialogDispatchAnchor、原关闭／确认分发与 Button 本身。作用于应用使用这两种包装层的共用弹窗；自定义 update_dialog 的按钮行不受影响。没有新增常驻状态或资源。上游及 vendor 原许可证保留。

Windows release 构建通过（1分02秒）。独立 `artifacts/dialog-layout/` fixture PID6360 的 compact.png 与前一版 rebase-operation/abort-ui/confirmation.png 可对照：原先整行两大按钮已变为右对齐紧凑按钮。exercise.ps1 取消后仓库不变断言通过；首轮确认点击未触发，中止后 HEAD 断言失败，截图出现部分黑屏，不能据此归因应用逻辑。未重启进程；重新捕获同一窗口完整画面后在相同按钮中心附近定向点击，verify-retry.ps1 确认中止恢复原 HEAD／main、无 Rebase 状态且工作区干净。保存初次失败和重试证据，不称首轮脚本全绿。已核对路径关闭自有6360并确认启动会话结束。

仍待：最小按钮宽度、Windows 按钮顺序与默认焦点、主次强调、统一标题／图标／留白，以及编辑器／Pull／Push／认证的代表性复验。此次仅修正共用布局层，不宣称所有弹窗视觉和键盘验收通过；没有新增镜像式单元测试，以原生布局与真实 Git 行为检查为证据。发布包未更新。

## 标准确认弹窗的按钮顺序与最小宽度（2026-10-17）

读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 DialogWrapper.java::createActions（默认 OK、Cancel，帮助有条件追加）、GitPullDialog.kt（设置 OK 文案为 Pull，沿用默认动作）、GitUnstructuredEditor.java（沿用 DialogWrapper，初始焦点为文本编辑器）。读取 DarculaButtonUI.java::getDarculaButtonSize/getPreferredSize：内容尺寸加边距并满足主题 minimumSize；JBUI.java::CurrentTheme.Button.minimumSize 默认72×24，可被主题覆盖。重读 GitAbortOperationAction.kt 的 OK／Cancel 中止确认；Rebase 单仓确认依据前述 GitAbortRebaseProcess。

新增 `src/dialog_style.rs::confirmation`，统一标准确认动作在前、取消在后，两按钮 min_w=72逻辑像素，继续使用共用 footer 的右对齐与现有分发。应用于 Git 编辑器、Pull、Merge Abort、Rebase Abort，Abort 确认使用主按钮强调。自定义风险决策不强行套用；Push／认证保持待单独核对。`examples/editor_dialog.rs` 接入同一模块。72 是上游主题 fallback 的本地适配，非安装版精确测量；未改变 Enter／Esc 分发或编辑器初始焦点，不把鼠标验证当作键盘验收。

Windows release 构建通过（20.23秒）。`artifacts/dialog-standard/` 新 fixture PID27564：confirmation.png 显示右侧紧凑且一致最小宽度的“中止／取消”，主动作蓝色；exercise.ps1 点击右侧取消后断言 HEAD／冲突索引不变且 Rebase 保留，重新打开点击左侧中止后断言恢复原 HEAD／main、状态结束且工作区干净。完整脚本通过，截图及断言归档。校验路径关闭唯一自有窗口并确认启动会话结束，无用户仓库或窗口改动。未更新便携包，未做内存／滚动验收。

下一步继续默认焦点、键盘遍历与其他代表性弹窗，随后顶部分支菜单。标题图标、路径截断、尺寸／留白和整体框架仍有差异；本次仅标准按钮组织改进，不宣称完整像素还原。

## 标准确认 Esc／Enter 原生证据与顶部分支菜单源码入口（2026-10-17）

读取参考 4aec352 的 DialogWrapper.java 键盘注册：Esc 取消；默认动作来自默认按钮，特定编辑器有自己的 preferred focus。使用最终 dialog-standard release 新建 `artifacts/dialog-keyboard/` fixture，PID27100。exercise.ps1 对目标 HWND 定向发送 Esc，不借用全局键盘：escape.png 确认弹窗关闭，断言 HEAD／冲突索引未变且 Rebase 保留。重新打开后直接定向 Enter（不先点击按钮）执行默认中止，断言恢复原 HEAD／main、无 rebase 标记且干净，稳定截图归档。脚本通过，核对路径后关闭自有进程，启动会话结束。此次证明 Rebase Abort 的 Esc 和默认 Enter，不证明 Tab／Shift+Tab、按钮空格激活或所有弹窗键盘路径；不扩大验收结论，无应用改动和新构建。

为下一步顶部分支入口读取同参考源码：
- `plugins/git4idea/frontend/src/com/intellij/vcs/git/frontend/widget/GitToolbarWidgetAction.kt::createPopup`：顶部组合按钮根据仓库状态取得当前／可用仓库，再调用 GitBranchesPopup.createDefaultPopup；与 backend GitBranchWidget.kt 的状态栏入口区分，后者不能作为顶部布局证据。
- `shared/src/com/intellij/vcs/git/branch/popup/GitDefaultBranchesPopupStep.kt`：新 UI 首层动作组和通用顶层动作组之间按需分隔；单仓使用 GitBranchesTreeSingleRepoModel；选择 GitReference 创建 GitSingleRefActions 子步骤，并非直接 checkout 或改 Log 过滤。动作项只有启用才可执行，按最终选择与动作组类型决定子步骤／执行。
- `shared/src/com/intellij/vcs/git/branch/tree/GitBranchesTreeSingleRepoModel.kt`：recent 来自仓库近期分支并受 showRecentBranches 控制；local 为 localBranchesOrCurrent，remote／tags 来自仓库状态；按前缀建立分组并按需缓存节点，匹配动作与引用组合为顶层节点，匹配为空可无节点。具体顺序、收藏比较器、搜索匹配与展开状态仍需读基类，不从候选路径推断。

本地映射尚未实现：应新增独立顶部引用菜单实体及状态，复用仓库引用数据和已有 checkout 执行器，不复用 Log Branch 过滤菜单；动作先保持已授权范围，未实现的 Commit／New Branch 不做假按钮。当前只有 Log 行 Checkout Revision，尚无输入 revision 入口。继续追踪 popup 基类、GitSingleRefActions、顶部 action group 与测试后再实现。截图 titlebar-branch-popup.png 为视觉结构基准，安装版精确版本未知。下一阶段以此为主，键盘剩余项随共用弹窗改造补齐。

## 顶部分支菜单：独立引用分组模型（2026-10-17）

继续读取参考 4aec35256d765c217aa0d662b82ac4e860c396fd 的 `shared/src/com/intellij/vcs/git/branch/tree/GitBranchesTreeModel.kt::rebuild/getRefComparator/getSubTreeComparator` 与 `GitBranchesTreeModelUtil.kt::getLocalAndRemoteTopLevelNodes/match/LazyRefsSubtreeHolder/buildBranchTreeNodes`。已确认非空组依次 Recent／Local／Remote／Tags；普通引用排序先当前、再收藏、开启目录分组时含 slash 的引用、再引用名称比较；近期组保留输入顺序。特别注意 exceptRefFilter 仅在 matcher 非空时执行，因此未搜索时 Recent 与 Local 可重复，搜索时 Local 排除近期引用。树节点按引用列表顺序创建前缀分组，下级还有稳定分组排序，不能把排序后的扁平引用当作最终树渲染。

本地新增 `src/branch_popup.rs`：借用既有 Branch／Tag 字符串和 OID，构造独立 typed Section／Reference／Group；不改变 Log 的 query、collapsed 或 selection。近期输入去重并忽略已删除分支；以完整 refs 名称保留本地／远端／tag 身份。查询由独立匹配谓词传入，未冒充已经翻译 JetBrains MinusculeMatcher。单元测试覆盖 Recent 只在搜索时从 Local 去重、同名 tag 与本地分支身份／当前态不混淆、过期近期分支、当前／收藏／目录／自然序及空匹配。该模块是菜单数据准备，不是完整 popup，也不新增可见功能。

待接入：上游近期分支来源与持久化、收藏及目录设置、MinusculeMatcher 等价匹配、按需目录节点和展开状态、顶层动作及引用子菜单、标题栏锚点、Windows 原生交互。现有 snapshot tag 仅保留指向 commit 的标签，非 commit tag 仍为已有限制；不得在新菜单中假装完整支持。尚未新增 GUI 构建／原生验收或更新便携包。

## 顶部分支菜单：按层构造目录节点（2026-10-17）

再次核对参考 4aec352 的 GitBranchesTreeModelUtil.kt::LazyRefsSubtreeHolder.buildSubTree 和 GitBranchesTreeSingleRepoModel.kt::getChildren：目录按排序后的引用首次出现位置插入 LinkedHashMap；分类根保留插入序，前缀目录内部额外应用稳定 comparator，将当前／收藏引用排在目录之前、普通引用置后。不是全树统一按名称排序。

`src/branch_popup.rs` 新增 FolderKey（Section＋完整前缀）、Folder／Ref 节点及 Group::children，只构造指定目录的一层，借用引用名称／OID，不常驻完整递归树；普通前缀匹配要求 slash 边界，feature 不会误匹配 features。关闭目录分组时根直接提供完整名称的引用节点。Recent 与 Local 中相同路径拥有不同 key，后续展开状态不会混用；尚未接入实际展开／折叠或持久化。

读取 GitBranchesPopupTreeStateHolder.kt，确认上游工作区持久化的是顶层及第二层已展开路径集合，无展开路径时不保存状态；具体恢复时机、搜索时状态切换仍待沿 popup 基类核对，不擅自把整个 Log collapsed 集合复用于此。新增测试覆盖当前／收藏／嵌套目录／普通引用的稳定顺序、同名前缀的分类身份、深层按需展开、前缀边界、平铺模式。全部 branch_popup 3项测试通过；本轮为纯模型增量，没有新可见入口或 Windows UI 验收。下一步接 popup 的生命周期、搜索和操作子菜单，依然以用户的完整菜单结构为目标。

## 顶部分支菜单：展开状态与虚拟列表行（2026-10-17）

读取参考 4aec352 的 GitBranchesPopupBase.kt::applySearchPattern/traverseNodesAndExpand/selectPreferred/afterShow：更新查询后重建模型、展开当前／近期分支路径、选择首选项使其可见，再恢复会话 expandedPaths；不是无条件展开所有搜索结果。GitDefaultBranchesPopup.kt 在 afterShow 的基础选择之后应用已存 TreeState，dispose 时保存树状态并清空模型。GitBranchesPopupStepBase.kt::setSearchPattern 对 null 或精确 `/` 不过滤，其他输入 trim 后构建 `*pattern` 的 NameUtil matcher 并套 GitBranchesMatcherWrapper，不能以普通 contains 代替完整语义。

本地 branch_popup 新增独立 Expansion、reveal、toggle、saved_paths 及 visible_rows。reveal 只展开指定分类和引用的祖先；折叠父目录不删除会话中的子目录状态。saved_paths 只返回当前实际存在且可见的分类根和首层目录；不序列化全部深层展开信息。visible_rows 使用显式栈按已展开层构造可见行，保留深度／展开标记和原引用身份，供后续虚拟列表使用，避免极深引用路径递归耗尽栈。没有常驻完整递归树。磁盘存储与 popup dispose 接线仍未实现，不把返回 key 集合宣称为已持久化。

新增测试覆盖 reveal 路径、Recent／Local 同名节点不联动、可见行深度、只保存浅层路径、父目录折叠隐藏深层、展开后恢复会话状态、删除引用后的无效状态排除。branch_popup 共4项测试通过。此次仅模型层，无新增可见功能／Windows 原生验收。搜索 matcher 和动作子菜单仍待；后续必须接入真实标题栏 popup，不以这些模型测试代替产品功能验收。

## 顶部分支菜单：真实近期 checkout 数据（2026-10-17）

读取参考 4aec352 的 `plugins/git4idea/backend/src/repo/GitRecentCheckoutBranches.kt::collectRecentCheckoutBranches`：查询 checkout reflog，取最后一个 ` to ` 之后的本地分支名，按最近优先去重并排除不存在的分支；配置中最近分支未出现时追加，用于 clone 初始分支无 checkout 记录的情形。读取 backend/resources/intellij.vcs.git.backend.xml 注册值50；GitBranchesCollection.java 将显示量限制为5。读取 tests/git4idea/repo/GitRepositoriesFrontendHolderTest.kt 的 repo state updated 用例，确认当前新分支＋此前最近4个的期望顺序。

本地 branch_popup::recent_branches 使用既有有界可取消只读 Git 执行器，最多50条 checkout reflog、输出最多5个 full ref。显式 --format=%gs 只读 subject，避免用户 pretty 配置影响默认输出；结果按调用方已有本地 refs 校验。saved_recent 可选参数接受完整引用，偏好磁盘写入／读取尚未接入。读取失败返回 Err 以保留错误和取消区别，而上游失败直接空列表，此差异须在菜单层映射为非阻塞反馈，不能让整个分支菜单不可用。

真实 Git 测试 branch_popup_recents_follow_checkout_order_and_exclude_deleted_or_detached 覆盖7次新分支切换、重复切换、删除最近分支、detached hash排除、最多5个、提前取消、无checkout记录及有效／失效偏好补充。Linux通过（0.23秒），Windows测试构建通过（4.63秒），宿主Git实测通过（2.34秒），证据 artifacts/branch-popup/test-recents.ps1 与 recent-windows.txt。未启动GUI或修改用户仓库，发布包未更新。

额外读取 GitBranchesMatcherWrapper.kt：匹配本身委托 MinusculeMatcher，只在得分上增加10000减首个匹配偏移，偏好靠前出现的匹配；不能由这个 wrapper 推断普通子串匹配等价。搜索 matcher、标题栏 popup、动作子菜单和偏好持久化仍未接线，此轮完成的仅是真实近期数据入口。

## 输入 revision 的只读解析入口（2026-10-17）

读取参考 4aec352 的 GitCheckoutFromInputAction.kt：空仓禁用；GitRefDialog 输入 branch／tag／hash，确认后 GitBrancher.checkout(reference, true, ...)；GitBrancher.java 明确第二参数为 detach，因此这个输入动作即使输入本地分支也以 detached checkout 为语义。读取 GitRefDialog.java：返回去首尾空白的文本，初始焦点输入框；补全来自 Log 引用或后台 branch／tag provider。不能把此动作与分支子菜单中附着分支的普通 Checkout 混用。

新增 checkout::resolve_input 和 ResolvedInput，将输入只读解析为固定 commit OID，并固定源 HEAD，复用现有 checkout::execute 的过期 HEAD 防护、冲突与本地修改保护；通过 --end-of-options 避免选项解释、^{commit} 排除 blob／tree。此处是本地实现的两阶段适配：解析与实际写操作分开，引用在其间移动仍 checkout 已解析提交，不把用户 revision 原文作为命令选项传入。空白／多行／NUL输入拒绝；错误保留给调用方。

真实 Git 新测试 checkout_input_resolves_and_pins_refs_without_switching_or_accepting_noncommits 验证 annotated tag、首尾空白、HEAD~1、短哈希、本地分支、未知引用／选项外形／非commit／多行拒绝、取消；只读解析不改变分支，tag移动后执行仍使用固定OID并进入detached。Linux通过（见本轮命令输出）。尚未接 GitRefDialog 等价UI／补全及菜单入口，未进行该功能Windows验收；此入口不等于整个顶部分支菜单已完成。下一步输入控件与异步解析反馈，并接真实标题栏菜单；上游 MinusculeMatcher、近期偏好和操作子菜单仍待。

## 标题栏 revision 输入动作接线与 Windows 验证（2026-10-17）

进一步读取参考 4aec352 的 GitRefDialogUi.kt：上方说明、填满横向空间的文本输入；GitRefDialog.java 初始焦点输入、首尾空白修剪；GitCheckoutFromInputAction 的 detach=true 沿用前节。`src/ui.rs` 新增 open_checkout_input、resolve_checkout_input 和 repository_branch_menu：项目名旁显示实际当前分支／detached OID，入口菜单暂只接通 Checkout Tag or Revision 动作；输入弹窗采用标准确认按钮组，后台解析固定OID及源HEAD，再转入原 start_checkout 与覆盖保护流程。解析期间使用同一操作token禁止重复写入，取消不继续checkout，失败显示本地化摘要加Git原始原因。新文本加入中英文。增加 detached 提示属于本地明确操作结果的适配，上游文本框本身无此提示。

Windows release 构建通过（26.46秒）。新 fixture artifacts/checkout-input/ PID26528：菜单位于标题栏分支按钮下方；点击打开弹窗，直接发送 release 字符，无额外输入框点击，typed.png 证明初始焦点有效。定向 Enter 确认后等待3秒，confirm.ps1 验证 HEAD等于标签目标incoming、symbolic-ref确认detached、main仍为原提交、index／工作区干净；complete.png 和 assertions.json归档。核对路径后关闭唯一自有进程并确认启动会话结束，未修改用户窗口／仓库，便携包未更新。

功能阶段差异明确保留：这是首个顶部分支动作，不是完整菜单交付。当前 dropdown 还没有搜索框、Recent／Local／Remote／Tags 树、收藏和引用操作子菜单；已有 branch_popup 模型尚未挂入，必须继续替换菜单内容为目标结构，不以单项菜单结束目标。图标暂借 branchLabel（tag外形）也需改为顶部branch图标。输入弹窗补全未实现；空白确认仅留在弹窗、错误详情仍在截断状态栏，需改进反馈；取消／无效输入／补全的原生交互待验收。本次证明有效tag输入路径，不扩大为所有revision和完整视觉验收。

## Revision 输入补全与原生两阶段 Enter 验证（2026-10-17）

参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd：已读取 GitRefDialog.java 的 MySimpleCompletionListProvider／Log provider、platform/vcs-log/impl/src/com/intellij/vcs/log/ui/actions/TwoStepCompletionProvider.java、platform/lang-impl/src/com/intellij/util/textCompletion/ValuesCompletionProvider.java 与 TextCompletionProviderBase.java。引用输入使用无分隔符、忽略大小写的 PlainPrefixMatcher，前缀为开头到光标；不是顶部分支菜单的 MinusculeMatcher。上游同步分支与异步标签分阶段提供，Log 引用以名称加类型去重。

本地 src/revision_input.rs 接入 src/ui.rs 的输入弹窗，使用既有 completion_matches 匹配，虚拟列表显示候选，方向键选择，Enter 先接受候选、再次 Enter 才确认 checkout；Escape 有候选时先关闭候选。输入接受替换光标前文字、保留后缀，避免接受事件重新打开候选。首轮 Windows 截图发现候选被后续提示文字覆盖、选中背景不满行，改为 deferred priority 101 和全宽行后重新构建，release 通过（16.94秒）。

原生测试 artifacts/revision-completion/fixed：临时仓库输入 rel，显示 release；accept.ps1 定向 Enter 后 accepted.png 确认输入变成 release、候选关闭、弹窗保留，Git HEAD仍为原值；confirm.ps1 再次 Enter 后验证 HEAD等于标签目标、detached、main未移动、工作区干净，assertions.json 与 complete.png 留证。只关闭路径核验后的自有 PID32992，启动会话已正常结束。没有更新便携包。

仍未完整还原：候选暂从打开时 snapshot 取得并按名称去重，同名 branch/tag 的类型身份、异步标签及完整排序待补；鼠标接受、光标单独移动时刷新、空输入主动补全、窗口边界避让和完整取消路径尚未验收。不得把有效标签的键盘路径扩大为完整补全验收。下一阶段继续 UI/UX：完整标题栏搜索／分类树／引用子菜单，以及各业务弹窗的信息层级和默认动作；当前单动作菜单不是目标结构。

## 顶部分支 Checkout：附着本地分支执行路径（2026-10-17）

参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd：读取 shared 的 GitSingleRefActions.kt 与 intellij.vcs.git.shared.xml，确认引用子菜单通过 Git.Branch 动作组扩展；读取 backend/actions/ref/GitCheckoutAction.kt、GitSingleRefAction.kt：单仓当前引用的 Checkout 不可用，本地分支传短名称、detach=false，tag 传完整引用，remote 进入 GitRemoteBranchesUtil 的独立流程。读取 backend/ui/branch/GitBranchCheckoutOperation.kt，确认远端对应本地分支已有额外提交时有 checkout/rebase/reset 决策，不能把远端简单映射成 detached OID。读取 commands/GitImpl.java::checkout，普通分支不追加 ^0，命令末尾加 --。读取 GitBranchWorkerTest.kt 的 revision detached、覆盖未跟踪文件、覆盖本地修改和回滚用例，区分普通分支 checkout 与版本输入动作。

本地 src/checkout.rs 新增 execute_local 与 LocalResult：仅接收完整本地引用，校验 ref 格式、源 HEAD 和目标 branch tip 后按短名称 checkout；用现有操作锁、取消和有界结果收集，结束后以新 token 读取实际 HEAD、symbolic-ref 和操作状态。成功需要命令成功、附着状态、引用名和目标OID均正确；不复用 detached 的 checked_out 判断。没有强制覆盖或隐式创建／reset分支。本地锁不约束外部Git进程，因此校验后外部并发改引用仍可能影响命令，最终结果会按实际状态判定，不宣称跨进程事务。

真实 Git 测试 branch_menu_checkout_stays_attached_and_preserves_overwritten_changes 覆盖同名 tag、本地分支正确附着、保留main与tag、不干净文件拒绝并保留内容、无效／远端／tag引用拒绝、过期HEAD／目标拒绝、提前取消、相同OID从detached重新附着。修正首次测试Fixture字段拼写后Linux通过（0.52秒）；Windows测试构建通过（4.15秒），宿主Git测试结果见 artifacts/branch-popup/checkout-windows.txt。本次没有UI变化，不声称菜单交付或原生界面验收。

待接线：引用子菜单的当前分支隐藏规则、操作反馈、本地修改恢复对话框、完整顶部分支树与搜索；remote checkout及tag菜单动作单独按上游实现，不使用本入口替代。仍需覆盖工作树占用、未跟踪覆盖等分支路径。便携包未更新。

## 本地分支 Checkout：失败分类与显式恢复（2026-10-17）

继续读取参考 4aec352 的 backend/src/branch/GitCheckoutOperation.java::runCheckoutAndHandleCommonFailures、smartCheckoutOrNotify、smartCheckout、checkoutOrNotify：冲突分类顺序为未合并、本地覆盖、未跟踪覆盖；本地覆盖展示 Smart／Force／取消选择；Smart 经 GitPreservingProcess 保存、执行原引用 checkout、恢复，Force 仍保持原 myDetach 值。工作树占用另有打开既有工作树／确认忽略占用流程，不能无提示追加 --ignore-other-worktrees。

src/checkout.rs 将命令失败分类提为共享逻辑，但分别按 detached 与本地完整引用判断成功；LocalResult 增加 status 与 affected_files，复用已翻译的有界路径解析。新增 smart_execute_local、force_execute_local；普通、Smart、Force 共用本地引用与预期OID校验。Smart 在 Session 持锁期间保存、重新校验并执行、恢复，再读取实际分支／HEAD／冲突与保留stash；不再次获取同一操作锁。Force 只能由显式选择调用，不是失败自动重试。原 detached API 保持原行为。

新增真实仓库测试：成功恢复暂存内容且清除自有stash；恢复冲突时保持附着目标分支并保留stash原始内容；覆盖错误分类及路径；Force 拒绝过期目标而不损失本地内容，以及显式执行后的干净工作区／源分支未移动。Linux checkout 相关10项通过（1.68秒），Windows测试构建通过（2.55秒）；宿主测试脚本 artifacts/branch-popup/test-checkout-recovery.ps1、输出 checkout-recovery-windows.txt。本轮仅后端，无UI验收或便携包更新。菜单、恢复弹窗与刷新接线仍待；工作树占用恢复尚未翻译，继续保留Git拒绝行为。

## 顶部分支树接入与本地 Checkout 原生验证（2026-10-17）

沿用已读参考 4aec352 的 GitToolbarWidgetAction、GitDefaultBranchesPopupStep、GitBranchesTreeSingleRepoModel／ModelUtil 与 GitSingleRefActions；本轮再次查看 shared/resources/intellij.vcs.git.shared.xml、backend/actions/ref/GitCheckoutAction.kt 和 GitSingleRefAction.kt，保持引用先打开操作子菜单、本地 Checkout 附着、tag detached、当前本地分支无 Checkout 动作的区分。

src/ui.rs 的 repository_branch_menu 改为标题栏锚定 Popover；独立 repository_expansion 与虚拟列表接上 branch_popup 分组、目录展开和当前分支祖先展开。引用点击打开操作菜单，不改变Log过滤，也不直接写仓库。本地 Checkout 接 execute_local，结束清理操作状态并刷新引用／Log／作者；tag 接已有固定OID detached路径。菜单新增中英文动作文本。空分类不显示，远端暂不提供checkout（其独立流程尚未实现）。复制完整引用名是本地新增只读动作，未证实与上游同名动作一致，列为差异待统一，不作为上游行为结论。

Windows release 构建通过（23.87秒）。artifacts/repository-menu 保存 menu.png、expanded.png、actions.png、complete.png：确认 Local目录树、feature/topic展开、点击引用只开操作菜单，再点Checkout执行；verify.ps1验证symbolic-ref为refs/heads/feature/topic、HEAD等于incoming、main未移动、工作区干净。完成截图确认标题栏、graph高亮已刷新。仅关闭路径核验后的自有PID8520，启动会话已结束，便携包未更新。

功能阶段UI验收仍为部分通过：标题栏入口／目录层级／先菜单后操作／成功刷新已验证；引用操作菜单目前是下方dropdown，需改为右侧级联，属于结构差异，不能推迟到像素阶段。缺少顶部搜索、Recent异步数据、收藏、目录设置与展开持久化；键盘方向／焦点／关闭层级未验收。当前使用tag形图标、顶层动作居中与参考不符，待统一。tag当前引用判定尚未对照GitRefUtil完成；tag原生checkout、远端节点路径未验收。本地覆盖失败目前仅状态反馈，虽然后端Smart／Force可用，恢复弹窗尚未接到本地路径，须继续完成。跨仓库展开状态清理和打开期间快照变化需补生命周期验证。本阶段不是完整分支菜单交付，不勾选总验收。

## 引用操作菜单右侧展开与 Esc 层级（2026-10-17）

读取参考 4aec352 的 GitBranchesPopupBase.kt::handleNextStep：子菜单位于父菜单右边界减STEP_X_PADDING，纵向使用所选树行顶部；handleSelect先disposeChildren，最终动作关闭父菜单链。src/ui.rs 引用动作改用侧向Anchor::LeftCenter，顶层版本输入动作改为左对齐；引用行组件ID从可见行序号改成完整ref，避免折叠后序号复用保留另一个引用的dropdown状态。当前未接Recent重复项；接入时须把Section加入行身份。

Windows release通过（16.52秒）。artifacts/repository-menu/cascade/actions.png确认操作菜单已移到引用右侧；escape.ps1定向Esc，escaped.png确认只关闭子菜单，父树仍保留、引用聚焦；重新点击main打开菜单并执行Checkout，verify-cascade.ps1验证正确附着main、HEAD正确、工作区干净。仅关闭路径核验后的自有PID31340，会话已结束。此轮无滚动自动化，便携包未更新。

剩余结构差异：GPUI侧向锚点当前纵向居中，上游按行顶部定位；边界翻转、Left返回、Right打开、hover切换单个子菜单和点击外侧逐层关闭尚待核对，不宣称完整级联交互验收。搜索、Recent、收藏及本地覆盖恢复弹窗仍按前节待办继续。此轮真实验证是右侧位置、一级Esc返回及之后Checkout，不扩大为所有菜单生命周期。

## Recent 异步接入与分组状态隔离（2026-10-17）

再次读取参考 4aec352 的 backend/src/repo/GitRecentCheckoutBranches.kt：后台读取checkout reflog、只保留现存本地分支、失败返回空列表、配置近期分支用于clone初始分支补充；读取 GitBranchesPopupBase.kt 的近期路径展开逻辑。src/ui.rs 接入已测试recent_branches：每次打开独立后台读取，任务代数、仓库路径和菜单打开态阻止迟到结果；关闭／动作选择／切仓取消token，切仓清空近期及展开状态，Drop亦取消。失败时清空近期但其他分类仍可用，与上游空列表处理一致。当前未提供保存近期偏好，仍无clone初始分支补充。

branch_popup::VisibleRow增加Section身份；引用行ID为分类+完整ref，目录为分类+完整前缀，因此未搜索时Recent／Local同一分支的展开与菜单状态不会混用。Recent读取成功时展开其祖先路径，模型与Log状态独立。只保留最多5个近期名称；读取期间临时复制已有branch列表，未创建常驻第二份snapshot。

模型4项测试通过；Windows release通过（22.93秒）。artifacts/repository-menu/recent/menu.png确认Recent位于Local之前且按reflog顺序显示main、feature/topic、incoming；reflog.txt归档原始来源。verify-recent.ps1先展开Local/feature再折叠Recent/feature，recent-collapsed.png确认只改变对应分类；关闭、重新打开后reopened.png有近期分组，Git检查HEAD仍附着main且OID未变。仅关闭路径核验后的PID3624，启动会话结束。

待验收与差异：未用人为延迟任务验证取消竞态，也未做原生切仓测试；打开后异步到达的Recent会展开路径，保存的折叠状态恢复顺序尚未按上游完整实现。无Recent开关／收藏／搜索／偏好持久化；复制引用动作仍为本地差异，完整级联键盘与本地覆盖恢复接线继续待办。本轮没有新的内存规模验收或便携包更新。

## 收藏模型：默认收藏与显式排除（2026-10-17）

读取参考 4aec352 的 plugins/git4idea/backend/src/ui/branch/GitBranchManager.kt、platform/dvcs-impl/src/com/intellij/dvcs/branch/DvcsBranchManager.java：默认收藏本地main／master和远端origin/main／origin/master，tag无默认项；isFavorite先显式收藏、再排除、再默认；setFavorite对默认项取消时保存排除，重新收藏清除排除，非默认项取消则移除显式收藏。上游包含项目级全仓与指定仓库作用域，本项目先以每仓独立值映射；全仓覆盖尚未实现。尝试定位BranchStorage.java的路径不存在，不将未读文件作为已验证持久化格式。

src/branch_popup.rs新增Favorites，分别保存added/excluded，以full ref隔离同名branch/tag/remote，提供contains、set、toggle、present和最小overrides迭代接口。present只把当前快照存在的收藏交给树，保留已删除引用的偏好，重建同名分支可恢复。当前分支优先排序与收藏状态独立，不把当前分支自动视为收藏。该模型由调用方按仓库身份管理，尚未磁盘保存或挂入UI；不宣称重启持久化已完成。

新增测试覆盖4个默认项、tag／非origin远端不默认收藏、默认取消及恢复、同名类型隔离、override重建、无效伪引用拒绝、两个独立仓库值互不影响、当前与收藏排序、删除／重新出现引用偏好保留。与已有分组测试一起执行。下一步接仓库偏好存储和星标交互；本轮无新增可见功能，不需要新增原生截图，便携包未更新。

## 收藏持久化层（2026-10-17）

读取参考 4aec352 的正确路径 platform/dvcs-impl/shared/src/com/intellij/dvcs/branch/BranchStorage.kt：以引用类型为映射、repoPath和sourceName为条目身份，add去重、remove删除精确仓库项并清空空分类。沿用GitBranchManager／DvcsBranchManager的默认收藏与排除规则。本地src/favorite_preferences.rs使用单工作树管理目录git-review/branch-favorites，复用update_preferences已有的editor_directory定位；这是与上游项目级XML存储的明确差异，不写版本控制文件或Git config，跨工作树不共享收藏。读取不创建文件；仅显式保存才写入。

存储为版本化完整记录，按引用名排序，只写默认项的排除和非默认项的新增，包含结束标记；读取最多1MiB，最多4096条override，拒绝损坏、截断、重复及未知格式。不存在返回默认值，其他读取错误返回调用方，不悄悄覆盖损坏记录。保存先校验，再创建同目录随机临时文件、sync、rename替换，错误清理仅限成功创建的自有临时文件。并发独立实例为最后写入生效，没有跨实例合并／锁；目录fsync和断电持久性未承诺。上限为本地低内存适配，超限报错。

单元测试覆盖默认不创建目录、默认排除与Unicode标签回读、已有文件替换、独立路径不串用、无效写入保留旧内容、临时文件清理，以及格式版本、截断、重复、条数超限。首次getrandom错误类型转换编译失败已修正。Linux两项通过；Windows最终测试构建通过（1.45秒），原生两项通过（0.05秒），artifacts/favorites/test.ps1及windows-tests.txt留证。尚未实际linked-worktree定位集成测试，不能把路径级隔离测试称作完整工作树验收。

本轮仅持久化API，尚未接入菜单加载／保存、星标点击、错误提示或原生重启后的UI恢复；下一步完成这些接线。未启动GUI或更新便携包。

## 顶部分支收藏接线与重启恢复（2026-10-17）

读取参考 4aec352 的 shared/src/com/intellij/vcs/git/branch/tree/GitBranchesTreeRenderer.kt 与 shared/src/com/intellij/vcs/git/ui/GitBranchesTreeIconProvider.kt：图标按选中未收藏／当前且收藏／当前／收藏／tag／普通branch优先选择，主图标可作为收藏切换命中区。导入favorite、notFavoriteOnHover、currentBranchFavoriteLabel_dark、currentBranchLabel_dark原始SVG，来源与许可证记入assets/icons/SOURCE.txt并注册assets.rs。已安装截图版本与该源码图标不一定相同，当前收藏图标使用源码的标签形状而非凭截图替换。

src/ui.rs将Favorites接入分组排序；打开时与Recent后台任务读取每工作树设置，读取失败显示本地化提示且禁止覆盖。主图标独立点击切换收藏，不触发引用操作菜单；Recent/Local即时共享同一引用收藏状态。异步保存期间禁用重复切换，失败回退旧值并提示；加载期间亦禁用，避免旧读取覆盖刚完成的点击。保存任务不因菜单关闭取消；仓库切换递增epoch，旧保存回调不能改新仓库UI。当前星标的读写错误只显示摘要，完整错误详情待改善。

原生第一轮发现图标被默认按钮内边距挤压、引用菜单触发区未占满行，修正星标p_0及固定尺寸、独立弹性行容器后重建，最终release通过（18.86秒）。artifacts/favorites/fixed/defaults.png确认默认main收藏；toggle.ps1点击取消main、收藏feature/topic，saved-record.txt验证排除与显式收藏正确落盘，同时HEAD未改变、工作区干净。favorite-topic.png确认星标显示；重启新进程后artifacts/favorites/restart/restored.png确认main未收藏、topic仍收藏。关闭重开有截图，但未将即时采样作为异步加载完整时序证明，以独立重启截图为恢复证据。全部自有测试PID9528、37872、9296经路径核验关闭，启动会话结束，便携包未更新。

剩余验收：未选中图标悬停替换为空心星、Space键切换尚未接入；读取／写入失败回退、快速关闭重开及跨仓库保存完成竞态需专门验证；实际linked-worktree隔离仍待集成测试。新图标有源码出处但未完成同DPI像素对齐；图标色彩当前GPUI单色渲染，原SVG颜色处理差异保留。搜索与本地checkout覆盖恢复弹窗仍未完成，整体菜单不宣称验收完成。

## 收藏工作树隔离：真实Git验证（2026-10-17）

新增 tests/repository.rs::favorites_are_private_per_worktree_and_reads_leave_git_state_unchanged，直接通过favorite_preferences::for_repository解析实际Git管理目录，不再只靠传入两个不同文件路径推断隔离。普通仓库初次读取返回默认且不创建设置目录；保存main排除与Unicode tag后，实际linked worktree返回独立路径和默认值；给linked保存另一个收藏后，两边再次读取仍各自正确；通过--no-local创建的clone不继承设置。提前取消读取不改已有记录，linked设置截断后读取报错、损坏字节原样保留且主工作树收藏仍正确。末尾逐字节核对主仓Git config及index，核对HEAD／symbolic-ref／干净工作区。

Linux通过（0.23秒）；Windows测试构建通过（4.38秒），宿主Git通过（1.94秒），artifacts/favorites/test-worktrees.ps1与worktrees-windows.txt留证。所有写入限临时fixture。该结果补齐前节实际linked-worktree存储隔离待办，不证明GUI切仓、错误提示回退或并发保存竞态已经验收。本轮无可见界面变更，未启动GUI／更新便携包。下一步仍是顶部分支搜索及交互补齐，本地checkout覆盖恢复接线未完成。

## 顶部分支搜索：ASCII核心匹配翻译（2026-10-17）

读取参考4aec352的GitBranchesPopupStepBase.kt::setSearchPattern：null或精确`/`不设matcher，其余trim并在前方加`*`；读取NameUtil.java::MatcherBuilder：默认IGNORE_CASE、空hardSeparators、未开启typoTolerant，构造FixingLayoutMatcher并包PinyinMatcher。读取MinusculeMatcherImpl.kt的初始化、matchWildcards、matchSkippingWords、maxMatchingFragment、findLongestMatchingPrefix、word-start查找和100字符阈值，以及NameUtilCore.kt::isWordStart。确认不是普通contains或任意子序列：混合大小写、连续数字、点号与中间片段最小长度均影响结果。

新增src/branch_matcher.rs::AsciiMatcher，仅翻译空hardSeparators、IGNORE_CASE、最多100字符的ASCII布尔匹配核心；通配符、连续片段、单词边界跳跃、数字和点号规则来自上游。对非ASCII或长pattern明确返回不支持，不误报为不匹配；未接UI，不作为临时替代搜索。暂不计算片段和matchingDegree，因此略去只用于优先选择片段的improveCamelHumps，使用状态memo避免重复求解；其余排序／评分须另行翻译和核对，不由布尔通过推断等价。内存／最坏输入性能尚未验收。

读取NameUtilMatchingTest.kt及其assertMatches／assertDoesntMatch helper，确认字面量断言使用IGNORE_CASE。tests/fixtures/minuscule-ascii.tsv保存256条上游直接字面量ASCII断言（保留可能重复的原用例，排除需要转义、非ASCII和长pattern），注明commit／来源／Apache-2.0；另加分支路径、数字跳跃和不支持输入用例。3项测试均通过，其中一项逐条执行全部256条断言。没有实际运行Kotlin matcher，不将静态移植断言当作全输入差分证明。

待完成：Unicode／UTF-16边界、键盘布局fallback、拼音包装、长输入降级、匹配片段与评分、搜索后展开／选择和动作匹配，再接顶部搜索UI。当前搜索仍未交付，无Windows GUI改动或便携包更新。

## 搜索长输入降级与快速排除（2026-10-17）

再次读取参考4aec352 MinusculeMatcherImpl.kt::match/matchBySubstring/meaningfulCharsMatchAt：先按非通配字符数拒绝过短候选；处理后的pattern长度严格大于100才降级，前导*控制从任意位置或仅开头尝试，pattern里的空格/*不进入meaningful数组，候选内空格/*可跳过。降级不是普通glob；meaningfulCharsMatchAt返回消费候选长度，候选提前结束且已消费字符时仍可能算匹配，此源码边界亦保留。读取MinusculeMatcher.kt::nameContainsAllMeaningfulCharsInOrder，短输入在递归匹配前先按大小写等价字符顺序快速排除。MinusculeMatcherPerformanceTest.java用于理解长输入／大量重复字符场景，未运行其Java性能基准。

src/branch_matcher.rs缓存meaningful字节，ASCII长pattern不再报不支持；大于100不进入递归memo路径。短pattern顺序预检查失败不分配memo。新增100/101阈值、大小写、前导*、长全通配、候选通配字符和短候选等测试，保留此前256条上游字面量断言。4项测试全部通过。此处仅证明布尔ASCII规则及这些边界，未给出延迟或内存达标结论；Unicode、包装器、评分与UI接线仍待，未更新便携包。

## 搜索键盘布局修正模型（2026-10-17）

读取参考4aec352的FixingLayoutMatcher.kt、MatcherWithFallback.kt、PlatformKeyboardLayoutConverter.kt、KeyboardLayoutUtil.java及KeyboardLayoutConverter.kt。确认fixLayout只在包含字母且所有字母均非ASCII时尝试转换；转换不变不创建fallback。布尔匹配为主或备用，片段／评分优先非空主匹配。Platform转换先查运行期实际按键映射；映射为空或Linux才用固定俄文键位表，支持大小写；Windows已有任意观察项后，未知字符不再走俄文硬编码。store只接受物理A..Z范围，忽略已为ASCII字母的字符，首次观察优先。该规则不能简单做全局俄文转拉丁或按系统语言猜测布局。

新增src/keyboard_layout.rs::Layout，翻译观察／转换／备用pattern生成，区分Windows与Linux fallback。字母判定作为显式参数留给Unicode实现，避免将Rust Alphabetic（含某些组合标记）冒充Java Letter；当前测试只注入受测脚本的限定判定，不把它接入生产搜索。两项测试覆盖全非ASCII字母条件、混合ASCII禁止修正、数字符号不触发、大小写、点号物理键转换、固定/动态表优先与首次观察、无效按键范围、修正后ASCII matcher匹配String。测试通过。

尚未接Windows物理键码采集、完整UTF-16/Unicode字母属性或主matcher的fallback组合；片段评分与Pinyin亦待。本次无可见界面变化、Windows原生按键验收或便携包更新，不称完整键盘布局支持已交付。

## 中文搜索拼音fallback（2026-10-17）

完整读取参考4aec352的PinyinMatcher.kt（match、matchingDegree、create及ENCODING/DATA）和PinyinMatcherTest.kt。启用条件：仅移除一个前导*，剩余必须非空且全为小写ASCII字母，不能含i/u/v；不是全拼音、忽略大小写或任意子序列。候选必须在上游字表U+3400..U+9FFF内连续匹配，每个汉字允许编码中任一多音字首字母；返回首个匹配UTF-16区间。评分为开头500+长度、精确get/is/set前缀200+长度、其他位置长度，未匹配为Int.MIN_VALUE。作为普通matcher的fallback，不能覆盖已有主匹配。

scripts/generate-pinyin-reference.py从固定commit解析Kotlin字符串，校验27648字符与579种编码；将原DATA保留为UTF-16LE索引、ENCODING转为26位掩码，输出assets/search的两个bin及SOURCE.txt（JetBrains Apache-2.0）。静态数据共57612字节，生成时明确解析转义及Kotlin转义美元符号，不抓取外部拼音库。src/pinyin_matcher.rs按字符扫描，不构造整串拼音、UTF-16候选副本或每次查询字表；返回区间仍累计UTF-16偏移。没有对运行时整体内存作验收结论。

3项测试通过：上游你好/get你好、角色的gh不匹配与g匹配，补充j多音字、启用条件、路径／ASCII分隔符不能跨过、emoji前缀UTF-16偏移、评分、字表范围外字符以及所有生成索引范围校验。尚未运行上游Kotlin程序做完整差分。该模块未接主matcher/fallback组合或UI，Unicode主匹配与整体评分／选择仍待；本轮无可见界面变化或Windows GUI验证，便携包未更新。

## 搜索 Unicode 属性与边界基础（2026-10-17）

对照参考4aec352的 platform/util/base/src/com/intellij/util/text/NameUtilCore.kt、CodePoint.kt 及 text-matching 的 CharArrayUtil.kt：单字符大小写按UTF-16单元处理，单词边界部分使用完整codepoint，汉字与假名脚本转换另有规则。src/search_unicode.rs提供这些属性和边界基础，尚未替代主matcher或接入UI。

scripts/GenerateSearchUnicode.java使用宿主IDEA的JBR 25.0.4+1-b508.27生成静态属性与单字符大小写表，共2949个属性区间、2369个大小写记录（40755字节）。运行时不依赖JVM；生成来源和格式见assets/search/UNICODE-SOURCE.txt。此版本是实际观察到的IDEA运行时，不证明与用户安装的Rebased运行时一致，Unicode版本差异仍须保留。

devenv下search_unicode三项测试通过，覆盖UTF-16偏移、补充平面、组合标记、汉字／假名边界、大小写及生成表完整性。未运行完整Kotlin差分或Windows界面验收，不称Unicode搜索已经交付。

用户补充的两张UI/UX基准已与原附件逐字节SHA-256核对一致，继续保存在artifacts/rebased-user-reference；后续按本计划“UI/UX优先级调整”检查弹窗和顶部分支菜单。搜索基础测试通过不代表菜单交互已完成，也不替代结构与原生验收。

## 顶部分支搜索：UTF-16 主匹配核心（2026-10-17）

重新完整读取参考4aec352的 platform/util/text-matching/src/com/intellij/psi/codeStyle/MinusculeMatcherImpl.kt，并核对NameUtilCore.kt::isWordStart与NameUtilMatchingTest.kt的helper。新增src/utf16_matcher.rs，采用IGNORE_CASE、空hardSeparators，pattern与候选按UTF-16单元处理，接入前节静态属性表。顺序预检查／连续片段使用pattern的单字符大小写对；非ASCII候选位置查找使用Kotlin Char.equals规则，ASCII位置查找保留上游仅比较两种形式的分支。汉字、假名、补充平面与100单元降级阈值不按UTF-8字节或Rust字符数替代。

保留improveCamelHumps可达路径，即使目前只返回布尔值；片段回退低于最小长度时按上游停止，不继续跨过不允许的长度。使用每次候选匹配的状态memo替代重复递归，尚未加入上游maxFoundLength剪枝；性能与更广泛差分仍待。候选UTF-16缓冲及memo在调用后释放，不为每个引用常驻第二份名称。旧ASCII模块暂保留，后续统一公开入口，避免两个核心分别接UI。

三项测试通过：既有256条上游ASCII断言、中文／假名／emoji与单字符大小写／非ASCII数字边界，以及UTF-16阈值。新增Unicode用例为依据源码编写的本地用例，未运行Kotlin matcher作差分，不冒充上游原始断言。完整匹配片段、评分、布局／拼音组合及UI搜索／选择仍未接入；本轮未改变可见界面，未进行Windows GUI验收或更新便携包。

## 搜索匹配片段与评分（2026-10-17）

重新读取4aec352的MinusculeMatcherImpl.kt中matchWildcards、matchSkippingWords、matchInsideFragment、improveCamelHumps与appendRange；读取MinusculeMatcher.kt::calculateHumpedMatchingScore/evaluateCaseMatching，以及NameUtilCore.kt::nextWord。src/utf16_matcher.rs::find返回有序UTF-16区间，区分无匹配和空成功；递归逆序构造、相邻合并、最终反转，保留尾空格实际命中的区间及大写词首优先。补入maxFoundLength剪枝；memo仅记录失败状态，成功区间沿调用栈移动，避免缓存每个状态的片段副本。

matching_degree根据片段计算词首、大小写、跳过词首、起始／结尾等分数，空hardSeparators对应固定加2，未匹配返回i32::MIN，空片段返回0。search_unicode::next_word按codepoint分类并返回UTF-16偏移，评分仍把每个数字视作独立词首。该评分接口的片段前提为同一matcher对同一候选的find结果；未作为外部任意区间输入接口。

六项测试通过，包含原256条布尔断言、上游MatchingFragments／Sorted／PreferCaps／CamelHump用例、尾空格／空成功／Unicode偏移及阈值；tests/fixtures/minuscule-score.tsv保存25条上游显式IGNORE_CASE排序／同分断言，另验证上游valueStartCaseMatch=true用例。未执行Kotlin全量差分或性能基准，未宣称所有Unicode评分与边界已穷尽验证。

主matcher片段和评分基础已补齐，NameUtil布局／拼音包装、分支菜单专用分数、搜索后树展开／默认选择、动作匹配及输入UI仍待组合。未新增可见功能，未启动Windows GUI或更新便携包；整体顶部分支搜索仍未交付。

## 分支搜索包装与优先选择模型（2026-10-17）

读取4aec352的MatcherWithFallback.kt、FixingLayoutMatcher.kt、PinyinMatcher.kt::create、GitBranchesMatcherWrapper.kt及GitBranchesPopupStepBase.kt::setSearchPattern。src/branch_search.rs::Search将原始名称匹配、键位修正、原pattern拼音备用匹配串联，非空主片段优先；同一次find返回片段及其所属matcher的分数，不将备用片段配到主评分。分支评分增加10000减首片段UTF-16偏移；空片段不加权。None或精确“/”不创建过滤器，其余按Java空白规则trim再加前导*，包括空串创建空成功matcher，与UI何时传None的决策分开。

重新读取KeyboardLayoutUtil.java，keyboard_layout的观察入口拒绝无法表示为Java Char的补充平面标量，大小写分类与转换改用生成的Java属性表；Windows动态映射优先／首次观察规则保持。Windows物理键事件采集尚未接入，不能称实际键盘布局检测完成。

读取GitBranchesTreeModelUtil.kt::match/getPreferredBranch/LazyRefsSubtreeHolder及SingleRepoModel的选择入口：分支先按原比较器排序，匹配只过滤、不按分数重排；同组最高分作为topMatch，同分保留首项；优先组为Recent、Local、Remote、Tags，动作topMatch在引用之前。Search::filter_groups接入引用部分，搜索时Local排除Recent重复引用，保留显示顺序、去除空组、返回首选Section+完整引用身份。动作优先仍待UI层实现，返回值不宣称最终菜单选择已完成。

branch_search四项测试通过：主／键位／拼音优先级及对应分数、UTF-16偏移加权、空串／斜杠／NBSP处理、动态键位与平台差异、分组优先／同分／稳定顺序／去重与空结果。keyboard_layout原两项回归通过。当前已组合搜索模型，未接输入框、树展开恢复、焦点／键盘、可见高亮或动作过滤；未启动Windows GUI或更新便携包。

## 搜索后的动作选择与展开状态（2026-10-17）

读取4aec352 GitBranchesPopupBase.kt::applySearchPattern/traverseNodesAndExpand/selectPreferred/addTreeListeners/onSpeedSearchPatternChanged，GitBranchesTreeSingleRepoModel.kt::getPreferredSelection、GitBranchesTreeFilters.kt及GitVcsOptions.kt的filterByActionInPopup=true默认值。搜索不是全部展开：当前与Recent引用路径自动展开，首选结果makeVisible；已有展开路径继续保留，显式折叠从expandedPaths移除。动作匹配优先于引用；不按分数重排显示。刷新用TreeState保留状态与输入变更重新选择是两个流程，不能混用。

src/branch_search.rs新增popup_pattern（去尾部Java空白并将空串变None）及apply_to_popup，接收有序引用组、稳定动作ID／本地化文字和既有Expansion，返回可见动作与Selection。匹配动作优先；禁用动作过滤时仍显示动作，但不将其当作匹配动作抢占引用；无过滤优先调用方提供的已保存／当前引用；无首选则退回首个动作或引用。自动展开当前、Recent和首选引用祖先，保留其他展开键，不维护一份会覆盖用户折叠的“搜索前快照”。切仓清理和打开／关闭仍由UI所有者负责。

branch_search五项测试通过，新增路径覆盖动作与引用同时命中、关闭动作过滤、首选引用祖先可见、不展开无关目录、清空后恢复当前引用、原展开键保留、无结果及尾空白／斜杠归一化。此为模型验证；尚未调用于GPUI弹出菜单，输入去抖、实际选择／焦点、鼠标／键盘、高亮、刷新选择保持以及Windows原生验收仍待。没有新增可见功能或更新便携包。

## 顶部分支搜索输入接线与原生验证（2026-10-17）

沿用前节已读4aec352 PopupBase／SingleRepoModel／MatcherWrapper，并核对GitBranchesTreeUtil.kt::FILTER_DEBOUNCE_MS=100。src/ui.rs新增菜单内搜索输入、100ms可取消任务、当前matcher及首选身份，输入变化过滤动作和引用、展开首选祖先、定位首选行；打开时清空输入，关闭／切仓取消任务并释放matcher，Recent到达后重新应用搜索。未添加仓库查询或Diff缓存，仍从现有引用快照过滤。i18n.rs加入搜索占位及空态中英文，切换语言时更新占位与动作匹配。

Windows release首版构建通过（24.26秒）。artifacts/branch-search-ui归档原生截图与脚本：opened显示搜索在菜单顶部；topic显示仅Recent/feature/topic并蓝色标记首选；empty显示无结果；cleared恢复引用及当前main；reopened记录关闭重开；action-unicode显示输入“检出”只保留版本输入动作，revision-dialog确认点击后真正进入已有版本输入对话框。首次action截图因脚本PostMessage默认ANSI入口导致中文乱码，unicode.ps1改用PostMessageW及明确UTF-16码点后验证通过，不把该失败截图作为中文匹配证据。测试核对HEAD未变、工作区干净。路径确认后关闭自有PID37804，启动会话已结束，未关闭用户窗口。

功能阶段UI验收为部分通过：入口位置、动作／引用过滤、首选可见、空态、清空与鼠标动作已验证。仍缺方向键／Enter打开首选子菜单、鼠标移动同步选择、片段文字高亮、子菜单过滤变化时主动关闭、输入法真实组合态、英文原生验证及物理键位观察；当前直接聚焦搜索输入，上游以树为preferred focus，需在键盘交互阶段统一。动作选中使用GPUI默认样式，与引用蓝色行不同；无结果／仅动作时列表仍留一行高度；菜单工具图标及尺寸密度待完善。渲染仍重复计算过滤组，尚未做大引用集延迟／内存验收。最后补齐语言切换占位刷新后重新构建，截图来自此前同搜索逻辑构建；未更新便携包，不宣称完整搜索或像素验收完成。

## 分支菜单搜索文字高亮（2026-10-17）

读取4aec352 GitBranchesTreeRenderer.kt::getTreeCellRendererComponent、GitBranchesTreeUtil.kt::canHighlight、PopupStepBase.kt::getNodeText，以及platform/platform-api的SpeedSearchUtil.java：对显示文字重新匹配，不将完整引用名称的匹配区间直接套到分组叶子；分类标题不参与高亮，目录、引用和可过滤动作参与。SpeedSearchComparator.java确认使用NameUtil构造matcher，但其可配置camel-case／hardSeparators等选项尚未沿完整WizardPopup链核实，本地暂复用已实现的分支matcher，特殊通配输入的高亮差异需保留。

src/branch_search.rs::byte_ranges将有序UTF-16区间转换为UTF-8字节边界；区间触及代理项时覆盖整个标量，相邻区间合并。src/ui.rs::repository_search_label使用GPUI StyledText，动作、目录和叶子分别基于显示文字匹配。读取SimpleColoredComponent.java第二遍绘制，深色搜索命中前景为Gray._0；背景暂取已读intellijlaf.theme.json的SearchMatch.start/endBackground #ffd042b3。当前GPUI是纯色矩形，未还原UIUtil.drawSearchMatch形状／主题运行时覆盖，不宣称Islands像素一致。

branch_search六项测试通过，新增目录显示文字匹配、中文UTF-8转换、emoji代理项边界及相邻合并。最终Windows release通过（18.88秒）。artifacts/branch-search-ui/highlight/partial.png确认“top”只高亮topic前三字母；folder.png确认搜索feature高亮目录文字而不错误高亮topic。测试核对HEAD不变、工作区干净；自有PID24860（补前景色前启动）、36700均已路径核验关闭，启动会话终止。中文／emoji原生高亮尚未验证，单元测试不能替代字体渲染验收。未更新便携包，键盘导航与子菜单状态同步继续待办。

## 引用子菜单生命周期接线（2026-10-17）

读取4aec352 GitBranchesPopupBase.kt::handleSelect/disposeChildren/handleNextStep，以及overrideTreeActions/processKeyEvent；本地检查vendor/gpui-component的DropdownMenuPopover、PopupMenu及gpui-base PopoverState关闭／焦点恢复。src/ui.rs由父Review保存当前PopupMenu弱引用，打开另一个引用时关闭旧菜单并同步Section+完整ref选择；搜索输入变化立即关闭旧子菜单，不等100ms过滤任务；目录折叠和父菜单关闭／切仓也关闭子菜单。关闭事件延后发送，避免菜单动作回调内重入正在更新的PopupMenu；只捕获弱引用，不引入父子实体持有环。未修改通用组件API。

最终Windows release通过（19.03秒）。artifacts/branch-search-ui/child-lifecycle记录搜索topic打开子菜单、重新聚焦输入改为main后旧菜单消失。该路径同时包含外侧点击关闭行为，不能仅凭截图证明所有程序化取消分支。首轮鼠标checkout点击后菜单关闭但HEAD未变，原因未确定，不计通过；retry.ps1重新打开同一子菜单、Down/Enter确认checkout，验证附着refs/heads/feature/topic、目标OID正确且工作区干净，证明菜单动作中关闭父菜单没有重入崩溃。关闭路径核验后的自有PID27652并确认启动会话结束，随后以宿主Git将隔离fixture恢复main及原OID。用户仓库和Rebased未改动。

待办：首轮鼠标checkout未触发原因需复核；多个引用间直接切换、目录折叠／切仓取消、迟到Recent更新仍需专项验证。这里验证的是已打开PopupMenu自身的Down/Enter，顶部分支树的方向键／Enter打开子菜单尚未接线，不宣称完整键盘导航完成。悬停选择与定时展开、位置对齐、内存生命周期测量及便携包更新仍待。

## 修复父菜单提前吞掉子菜单鼠标动作（2026-10-17）

按diagnosing-bugs流程，将前节鼠标失败缩减为打开菜单→输入topic→打开子菜单→点击检出，artifacts/branch-search-ui/mouse-checkout/check.ps1在修复前返回“Checkout not attached to topic”，复现此前失败；没有改变Git执行路径或测试坐标。前节已确认的4aec352 PopupBase父子菜单生命周期作为目标，进一步读取本地gpui-base Popover::render：默认on_mouse_down_out立即关闭父层。右侧引用菜单是独立DropdownMenuPopover，位于父层边界外，因此mousedown移除父子层，子菜单mouse-up/click处理器无法执行；键盘确认绕过此路径，解释了此前差异。

src/ui.rs关闭顶部分支Popover默认外侧处理，改在菜单主体处理：落在当前子菜单可见bounds内保留父层，真正落在父子两层之外才关闭。vendor/gpui-component PopupMenu增加只读contains_position接口，复用已有窗口坐标bounds；未扩大点击目标、不跳过确认、不修改checkout实现。该原生端到端脚本就是回归测试边界，不用只测试矩形contains的浅层单测冒充鼠标链路覆盖。无临时日志插桩。

Windows release通过（1分02秒）。同一check.ps1修复后通过，retry-result.png显示当前feature/topic，Git断言attached／目标OID／干净工作区均正确；关闭进程后恢复临时fixture main与原OID。mouse-outside/check.ps1进一步验证有子菜单时点击两层之外关闭整组菜单、仅父菜单时点击外侧仍关闭，已查看outside-both／outside-parent截图。自有PID33616、15652、30924均经路径核验关闭，启动会话结束；未关闭用户程序或更新便携包。前节鼠标checkout未触发待办由本节解决；树键盘导航及其余交互待办仍保留。
## 过滤后分支键盘导航：模型完成，原生接线待修复（2026-10-17）

对照4aec35256d765c217aa0d662b82ac4e860c396fd的plugins/git4idea/shared/src/com/intellij/vcs/git/branch/tree/GitBranchesTreeUtil.kt：过滤时方向导航跳过组标题和目录，遍历可选动作及引用叶子，包括折叠祖先下的匹配项；选中后显示祖先路径。platform/platform-api/src/com/intellij/util/ui/tree/TreeUtil.java的isCyclicScrollingAllowed受屏幕阅读器、Registry和UISettings共同控制，尚未核实安装版默认配置。

src/branch_navigation.rs实现Next／Previous／First／Last及可选循环，保留Section与完整引用身份；临时借用叶子列表，仅克隆最终选择。两项模型测试通过，覆盖目录遍历顺序、隐藏叶子、动作、重复引用身份及边界。src/ui.rs尝试在搜索输入聚焦时捕获方向键与Home／End；先应用最新查询并取消去抖任务，避免迟到任务重置选择。本地暂不循环，仍保持输入框焦点；这些与上游配置／树焦点的差异尚待统一，未过滤树导航与Enter／Right展开仍未实现。

最终Windows release构建通过（18.11秒）。artifacts/branch-search-ui/navigation归档原生脚本与截图；脚本确认HEAD不变、工作区干净并关闭自有PID25316，随后确认该进程不存在。但人工检查home.png与down.png发现选择仍停在incoming，未得到预期的main及feature/topic：原生按键链路未通过，脚本退出成功仅代表截图和仓库断言成功，不能作为导航通过依据。尚需区分事件分发／焦点接线与注入方式，修复后重新验证。未更新便携包，不宣称本项或完整分支菜单交互完成。


## 修复搜索输入吞掉分支导航键（2026-10-17）

前一轮为有进展：明确原生导航未通过并保存失败截图。本轮按diagnosing-bugs建立更严格的原生回归：artifacts/branch-search-ui/navigation/check.ps1在固定窗口与fixture下断言选中行背景，修复前明确失败“Navigation home failed”，不再将截图脚本退出成功等同导航成功。

重新读取4aec352的GitBranchesTreeUtil.kt选择规则，业务导航模型不变。检查本地GPUI 0.3.9 window.rs::dispatch_key_event发现快捷键action先于原始key-down分发；输入组件gpui-base 0.7.3 input/base/state.rs将up/down/home/end绑定MoveUp/MoveDown/MoveHome/MoveEnd并消费。临时日志证实原capture_key_down处理器未进入。src/ui.rs改为父菜单capture_action调用repository_search_move；仅搜索输入聚焦且过滤生效时接管，其他情况propagate给输入框。仍先取消去抖并应用最新查询，避免稍后覆盖导航选择。临时DEBUG-nav日志全部删除。

Windows release通过（17.58秒）。同一check.ps1修复后通过：Home→Recent/main、Down→Recent/feature/topic、End→Recent/incoming、Up→Recent/feature/topic，跳过目录行；down.png人工核对选择与目录结构。此断言只适用于该固定原生fixture的尺寸／主题，不冒充通用像素验收。HEAD与干净工作区断言通过。自有PID12404、15636、22320均经路径核验关闭，各启动会话已结束；用户程序未关闭。截图在navigation/，未更新便携包。

本节解决前节过滤状态下四个导航键未生效的问题。未过滤时树导航、Enter／Right打开首选引用子菜单、悬停选择、循环配置与树焦点还原仍待；不宣称完整分支菜单或UI/UX验收完成。

## 分支引用子菜单的Enter／Right入口（2026-10-17）

读取4aec35256d765c217aa0d662b82ac4e860c396fd的plugins/git4idea/shared/src/com/intellij/vcs/git/branch/popup/GitBranchesPopupBase.kt::overrideTreeActions/processKeyEvent/handleSelect/handleNextStep：叶子toggle允许最终动作，Right只走子步骤；没有子步骤的最终动作在Right下不执行；同一路径已显示子菜单时不重复创建。子菜单在选中行右侧显示，上游采用行顶部对齐。

src/ui.rs新增repository_activate_selection与Section+完整ref的child selection，捕获输入Enter／MoveRight动作，先应用未完成的查询；引用打开现有菜单，Enter在revision动作上进入现有输入对话框，Right不执行该最终动作。vendor/gpui-component/src/menu/dropdown_menu.rs新增可选受控open接口，传给现有Popover；鼠标和键盘复用同一builder、焦点、DismissEvent和弱引用生命周期。父层关闭、搜索变化和方向导航清理child selection；关闭回调仅清理身份仍匹配的菜单。Selection增加Clone，只保存一份当前子菜单身份，不复制引用快照。未改Git执行策略。

Windows release通过（24.36秒）。artifacts/branch-search-ui/keyboard-child/check.ps1验证输入topic→Enter展开→Esc关闭子菜单→Right再次展开→鼠标检出；人工检查enter.png和escape.png确认展开层、原查询保留及焦点回到输入框。Git断言attached refs/heads/feature/topic、OID正确及工作区干净通过，随后恢复main原OID。mouse-checkout/check.ps1再次验证纯鼠标打开／检出通过，排除受控开关破坏旧入口。自有PID22940、31060经路径核验关闭，各启动会话结束，用户窗口未动。未更新便携包。

功能阶段UI验收仍为部分通过：当前子菜单依旧LeftCenter而非上游行顶部，行按钮选中背景还覆盖部分整行蓝色；需在结构／样式整理时统一。revision动作Enter／Right分流、无过滤状态、键盘连续选择执行、嵌套Left返回、悬停展开与迟到刷新尚待原生验证。原搜索输入焦点策略与上游tree preferred focus的差异仍保留，不宣称完整键盘或像素还原完成。

## 分支菜单最终动作与全键盘检出验收（2026-10-17）

前轮实现为有效进展，本轮继续验证其未覆盖路径，使用同一Windows release，不新增业务实现。artifacts/branch-search-ui/keyboard-complete/check.ps1：PostMessageW输入中文“检出”，Right后人工核对action-right.png仍在动作菜单且HEAD未变；Enter后action-enter.png为已有版本输入弹窗，输入框获得焦点；Esc取消后重新打开分支菜单，输入topic，Enter展开、Down选中检出、Enter执行。Git断言attached refs/heads/feature/topic、目标OID正确及工作区干净通过。关闭自有PID18648后恢复fixture main原OID；启动会话已结束，用户窗口未关闭。

脚本首次调用时startup.json尚未生成，读取失败，未进入交互；轮询同一启动会话确认已生成后重跑，没有重复启动进程。截图保存原始尺寸，未将这一初始化竞态当作产品失败。

上节revision动作Enter／Right分流和键盘连续选择执行的待验收项由本节覆盖。仍未覆盖未过滤树导航、Left返回、悬停展开、刷新状态保持等。仅动作时列表留白和动作灰黑选中态与分支蓝色选中态不统一，action-right.png作为后续UI结构／样式修正依据；本次没有扩大像素验收结论、重新采样内存预算或更新便携包。

## 分支菜单选中态与空列表布局统一（2026-10-17）

读取4aec35256d765c217aa0d662b82ac4e860c396fd的GitBranchesTreeRenderer.kt::getTreeCellRendererComponent：动作和引用文字使用同一CurrentTheme.Tree选中颜色，文字组件非opaque；GitBranchesTreeSingleRepoModel.kt::getTopLevelNodes只在引用组存在时追加动作分隔符。src/ui.rs的repository_menu_body据此将动作行改为28逻辑像素并使用现有分支蓝色选中态；引用名称和收藏按钮使用透明custom variant，避免Dropdown的open样式遮住父行选中背景。引用数量为零时不创建uniform_list，不再强行保留一行高度；分隔线只在动作和引用同时存在时显示。这里只统一本地现有颜色，未重新验证安装版Islands主题色值。

Windows release通过（17.31秒）。artifacts/branch-search-ui/row-style记录action-right.png仅动作结果收紧、蓝色选择保持；branch-enter.png子菜单打开后整行蓝色连续，无此前名称按钮黑色覆盖。复用全键盘路径验证版本动作Right不执行／Enter打开、Esc取消、分支Enter→Down→Enter检出；目标分支、OID和工作区断言通过。路径核验关闭自有PID31468并恢复fixture main原OID，启动会话已结束。没有变更Git操作逻辑或更新便携包。

功能和局部视觉验收通过；整体菜单仍待行顶部锚定、鼠标移动同步选择／延时展开、未过滤树键盘、工具图标和主题尺寸统一。透明按钮不自行绘制局部hover块，完整悬停反馈应由后续整行选择逻辑提供。无匹配空态与仅动作状态采用不同内容高度，空态本轮未再次截图；不宣称完整像素还原。

## 分支动作子菜单Left返回父层（2026-10-17）

对照4aec35256d765c217aa0d662b82ac4e860c396fd：GitBranchesPopupBase.kt::overrideTreeActions中嵌套树Left取消；实际引用动作子菜单还需遵循platform/platform-impl/src/com/intellij/ui/popup/list/ListPopupImpl.java的Left action／isClosableByLeftArrow／goBack与WizardPopup.java::goBack。存在父层时Left返回；如果子菜单自身持有speed-search过滤，先清除该过滤。当前本地动作子菜单没有独立搜索，因此实现返回分支父菜单，不清除父查询。

vendor/gpui-component PopupMenu新增默认false的close_on_left opt-in，仅src/ui.rs引用子菜单builder启用。适用于父层是外部树、没有PopupMenu parent指针的场景，调用原dismiss路径恢复焦点及发出关闭事件，并停止继续传播；普通菜单行为保持默认。未新增强持有关系，也未修改checkout实现。

Windows release通过（1分01秒）。artifacts/branch-search-ui/left-return/check.ps1验证topic→Enter展开→Left返回→Right重开，再重复两次Left／Right后Down／Enter检出。人工核对left.png：子菜单消失，topic查询与蓝色选中项保留，搜索框重新聚焦；最终Git attached分支、目标OID、干净工作区断言通过。路径核验关闭自有PID36172，恢复fixture main原OID，启动会话终止；未关闭用户程序或更新便携包。截图验证首轮返回，后两轮以连续键盘检出成功为端到端证据，未单独逐轮测内存。

此项补齐当前无独立搜索的引用动作子菜单Left返回；未来若添加子菜单搜索，需补齐先清子过滤的上游规则。未过滤树选择、悬停与延迟展开、行顶部锚定及刷新生命周期仍待，完整UI/UX验收未完成。

## 动作／引用行鼠标移动选择及自动展开依据修正（2026-10-17）

读取4aec35256d765c217aa0d662b82ac4e860c396fd的GitBranchesPopupBase.kt::SelectionMouseMotionListener/onChildSelectedFor、GitDefaultBranchesPopup.kt继承入口及platform/platform-impl/src/com/intellij/ui/popup/WizardPopup.java计时器实现。树监听忽略首个和相同坐标事件，屏幕阅读器启用时不改变选择；鼠标移动更新路径并通知父菜单。虽有restartTimer，WizardPopup::onAutoSelectionTimer为空，检索该版本git4idea无覆盖实现；因此之前“延迟展开”待办不能直接当作已证实行为，当前不添加自动展开。已安装版行为仍需另核对，版本一致性未知。

src/ui.rs为菜单增加一个最近指针坐标，在关闭时清理；动作和引用行on_mouse_move更新共享repository_selection，不开子菜单或执行Git动作。首个／重复坐标及window.is_a11y_active时不更新。该GPUI检测表示无障碍客户端活跃，比上游ScreenReader条件更宽，需后续专门验证。未给目录／标题增加虚假的引用身份；这些行的选择以及子菜单鼠标进入通知父选择仍待完整树选择模型补齐。

Windows release通过（17.25秒）。artifacts/branch-search-ui/hover-selection/check.ps1输入i（首选incoming），发送窗口鼠标移动到main再到topic；人工核对topic.png蓝色行已切到topic，停留800ms后无自动子菜单；Enter／Down／Enter最终检出refs/heads/feature/topic，目标OID、附着分支、干净工作区断言通过。首次脚本的Move函数名被PowerShell别名截获，未进入移动步骤；改SendPointer并重置同一窗口菜单后通过，未重启程序冒充重试。路径核验关闭自有PID428，恢复fixture main，启动会话结束。

本轮只验收引用行悬停接键盘的路径；动作行移动、静止指针与键盘竞争、真实指针穿越子菜单、屏幕阅读器以及目录／组选择继续待验收。未更新便携包、未宣称完整悬停交互或像素还原。

## 分支子菜单改为行顶部对齐（2026-10-17）

重新读取4aec35256d765c217aa0d662b82ac4e860c396fd的GitBranchesPopupBase.kt::handleNextStep：x取父content右边缘减STEP_X_PADDING，y取选中路径顶部屏幕坐标。现有GPUI Popover的LeftCenter将两者中心相对，不能表达该布局。新增vendor/gpui-base（来源crates.io gpui-base 0.7.3，原LICENSE-APACHE／Cargo元数据保留，GITUI-PATCH.md说明改动），Cargo.toml patch与Cargo.lock指向本地副本。不是修改机器包缓存。

Popup／Popover及gpui-component Popover／DropdownMenu逐层传递默认false的side_start；启用时LeftCenter改用popup TopLeft定位到trigger top-right，RightCenter对应TopRight定位到trigger top-left，其余anchor及默认行为不变。定位仍读取每帧trigger canvas实测bounds，沿用Positioner corner裁限和8px窗口margin，不新增长期缓存。src/ui.rs仅引用子菜单启用。横向仍按行按钮右边缘加原gap，非上游父content右边缘减2px；该差异保留待视觉细化，尚未实现侧边不足时自动翻转。

Windows release通过（1分23秒）。artifacts/branch-search-ui/top-alignment/check.ps1保存enter、narrow、left、right；人工查看narrow.png确认子菜单顶部与选中行顶部对齐，尺寸变化后仍随行定位。SetWindowPos请求850×700受应用最小宽度限制，实际截图较宽且子菜单仍有足够侧向空间，因此本次只证明缩窗后的几何与可用性，不能作为边界裁限／翻转验收。重复Left／Right后键盘检出成功，attached分支、OID和工作区断言通过。路径核验关闭自有PID8516、恢复fixture main原OID，启动会话结束。

未更新便携包；其余popover默认路径本轮未逐个原生回归，新参数默认false但仍需在后续公共UI回归覆盖。子菜单水平间距、极窄／底部边缘、自动翻转及完整视觉验收仍未完成。

## 未过滤分支树可见行导航与折叠展开（2026-10-17）

读取4aec35256d765c217aa0d662b82ac4e860c396fd platform/platform-impl/src/com/intellij/ui/tree/ui/TreeAction.java::lineDown/lineUp/selectChild/selectParent/selectFirst/selectLast及DefaultTreeUI.java::installKeyboardActions，结合已读GitBranchesPopupBase对过滤模式的覆盖：普通上下键包含可见目录／分组而跳过分隔符；无选择时上下均选首项；Right先展开，再进入下一可见行；Left优先折叠，否则选择父路径，隐藏根需回退之前展开路径或首行。叶子Right仍由分支Popup覆盖为子菜单。

src/branch_search.rs::Selection增加Folder(FolderKey)，身份包含Section与目录前缀。src/branch_navigation.rs新增row_matches／row_selection与navigate_visible，借用可见行，仅克隆最终身份；保留过滤叶子遍历，并支持过滤时Left选中父目录后继续上下导航。src/ui.rs按是否有matcher选择导航策略，目录／分组可显示蓝色选中行，未过滤时鼠标折叠同步目录选择；Enter切换目录展开，Right展开／前进，Left折叠／返回父层。目录动作不触发Git写入。循环仍固定关闭，与上游设置的差异保留。

三项branch_navigation测试通过，新增可见行包含折叠分组、首项回退、目录身份和过滤父路径恢复。Windows release通过（17.63秒）。artifacts/branch-search-ui/unfiltered-tree/check.ps1在空搜索框下Home→Down选Recent→Left折叠→Right展开→Down两次选feature→Left折叠→Right展开→Right选topic→Enter／Down／Enter检出；collapsed.png人工核对分组折叠与选择，Git attached refs/heads/feature/topic、OID、干净工作区断言通过。路径核验关闭自有PID12116并恢复fixture main原OID，启动会话结束；未更新便携包。

本节覆盖普通树主要键盘路径，仍非全部TreeAction复刻：目录悬停选择、无选择Left／Right首项回退、隐藏根复杂边界、过滤状态目录Right细节、PageUp／PageDown、多选及循环设置未完整验收；输入框持续焦点和左右键文本编辑的冲突仍需按上游preferred tree focus统一。过滤模式本轮模型测试通过，但未重新跑全部原生过滤脚本，不扩大回归结论。

## 目录悬停选择与过滤时逐级Right（2026-10-17）

重新核对4aec352 GitBranchesPopupBase.kt::SelectionMouseMotionListener：任何有效树路径先更新selection，再判断是否为可执行节点；TreeAction.java::selectChild在展开目录上选下一可见行，不使用过滤叶子遍历。src/ui.rs目录／分组on_mouse_move接入共享指针选择，点击折叠在过滤和未过滤时均保存Folder身份；新增repository_visible_move复用已有可见行模型，目录Right在过滤状态也逐级进入，不跳过子目录。无选择Right与无法定位当前行的Left回退首个可见动作／行，Enter无选择仍不执行动作。

Windows release通过（17.80秒）。artifacts/branch-search-ui/folder-pointer/check.ps1输入topic，鼠标移动选Recent，Right选feature目录（人工核对right-folder.png），继续Right到topic、Left回父目录、Left折叠、Right展开／进入，然后键盘检出。attached refs/heads/feature/topic、目标OID、干净工作区断言通过。路径核验关闭自有PID31812，恢复fixture main原OID，启动会话结束。

本节补齐目录／分组指针选择和过滤目录Right实际路径；无选择左右键回退已实现但未构造原生失选专项用例。焦点仍停留输入框、子菜单指针通知父选择、Page键、循环及更新保持等差异仍待；未更新便携包或扩大整体视觉验收。

## 当前菜单构建的核心回归与大仓内存复测（2026-10-17）

本轮无应用逻辑改动，验证近期选择模型与组件定位补丁后的构建。Linux/devenv运行cargo test --no-default-features --lib：116项通过；--test repository：86项真实临时Git仓库测试通过（10.77秒），包括查询／过滤／完整Diff、Fetch／Pull／Push、checkout、stash恢复及冲突／取消等现有用例。记录artifacts/memory-menu-current/tests.json；这些结果不是Windows全部Git集成或vendor组件测试的替代。

Windows脚本artifacts/memory-menu-current/run.ps1复制当前release与两个helper到独立验证目录，使用独立LOCALAPPDATA，校验GitReview-benchmark恰好10000条提交且干净；调用windows-job-memory.ps1 -HistoryLimit 10000 -LargeDiff -MemorySamples 10 -AssertBudget。构建SHA256为5F21D6066010D7DD280E393FAA28F0A906CE4F647668B0D896ADF15FE8571528。history.png人工确认已加载10000条、offset 0；window.png确认独立Tab打开review.rs。现有固定benchmark的review.rs为975000字节／13000行；本轮没有滚动交互测试。

run/memory.json：Job内核峰值156139520字节（148.91 MiB），包括从挂起启动起继承Job的Git子进程；累计73个进程，末态仅应用存活。任意单进程峰值130.59 MiB，不代表Git独立峰值。10个稳定采样应用Private Bytes均95.89 MiB、Git子进程为0，末态Working Set 93.04 MiB，通过稳定150 MiB／Job峰值250 MiB预算。GPU单独一次计数为DedicatedUsage 58118144字节、SharedUsage 3756032字节、TotalCommitted 62664704字节，不与进程Private／Job峰值相加或称为同刻总峰值。采样期间otherReviewProcesses为空，不包含用户Rebased／IDEA的占用。

脚本正常退出，核实自有PID9420已不存在；benchmark HEAD与干净工作区保持。原始采样、截图、可执行文件hash及运行脚本均在artifacts/memory-menu-current。当前场景通过预算，但不证明大量refs打开／搜索菜单时的延迟与峰值、反复菜单生命周期、冷启动、网络操作峰值或长期无增长，也不与旧版不同Diff实现的采样直接作回归百分比比较。未更新便携包，整体目标仍未完成。

## 5001分支菜单生命周期采样与详情溢出发现（2026-10-17）

本轮建立独立Windows fixture：1个空提交、main加5000个本地分支，按feature/team000..099/topic0000..4999分组，使用update-ref事务批量创建并pack-refs。artifacts/branch-scale/start.ps1保留生成与启动步骤；初次PowerShell管道向update-ref传入CRLF导致start命令解析失败，未启动GUI，改为.NET进程StandardInput写LF，在独立v2目录成功完成。未改用户仓库。

当前release运行check.ps1十轮打开菜单→输入topic4999→关闭，每阶段采样应用Private Bytes／Working Set及累计CPU；不是Job内核峰值，未纳入Git子进程或GPU，也不是精确输入延迟测量。人工核对filtered-1.png确实定位到feature/team099/topic4999，并自动显示祖先路径；保留第10轮及末态截图，未逐轮视觉检查中间状态。

samples.json：基线Private Bytes 69.92 MiB，所有阶段采样最高75.48 MiB；十轮关闭样本为74.86、70.64、74.09、70.56、70.12、70.76、70.06、70.24、74.24、74.54 MiB；末轮关闭后再等3秒为69.87 MiB。该短轮次没有单调增长，不能证明长期无泄漏或异步瞬时峰值。脚本验证HEAD未变且工作区干净，关闭路径核验后的自有PID15636，启动会话已结束。无滚动自动化。

新增明确UI缺口：5001个引用指向同一提交时，右侧详情引用名称连续换行，超出可用面板高度；filtered-1.png是复现证据。下一步应对照Rebased提交详情的引用显示／展开实现，限制初始可见内容并提供真正的展开交互，不能仅CSS裁掉名称。该问题优先于继续微调菜单间距。菜单规模采样通过当前观察范围，不等于详情面板、搜索延迟或完整低内存验收通过。未更新便携包。

## 大量直接引用的详情边界修正（2026-10-17）

重新读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/vcs-log/impl/src/com/intellij/vcs/log/ui/details/commit/ReferencesPanel.java、CommitDetailsPanel.kt，以及上层CommitDetailsListPanel.kt和platform/util/resources/misc/registry.properties。纠正上一节下一步推断：直接引用按branch/tag各限100，剩余为普通数量标签，不是Show all链接；ContainingBranchesPanel才有Show all／Hide。上层详情使用按需垂直滚动、禁用水平滚动。不能把两者合并或为直接引用另造展开流程。安装版Rebased版本仍未确认，本次规则来自上述固定源码。

src/ui.rs详情外框增加overflow_hidden，引用组容器禁止flex收缩，保留现有内部overflow_y_scroll、100个上限和独立包含分支区。此前大量换行内容可绘制到面板／窗口底部之外；这次修正布局边界，不减少引用展示数量，不新增缓存或查询。

Windows release构建通过（17.72秒）；复用5001本地分支、单提交fixture，artifacts/details-overflow/start.ps1启动独立profile，自有PID19804。人工查看window.png确认详情文字不再越过底部边界；仍需滚动才能看到长引用组后面的内容，这是上游有界详情视口的结构，不把首屏未显示误认为数据缺失。遵循用户约定未做滚动自动化，末尾数量、包含分支展开在此规模下的原生手测尚待验收。close.ps1验证HEAD未变／工作区干净后关闭自有进程，启动会话正常结束。启动Private 74.05MiB仅作启动记录，不是新的内存预算结论。未更新便携包，像素级验收及整体UI/UX计划仍未完成。

## 本地分支检出的Smart／Force恢复弹窗接线（2026-10-17）

重新读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的plugins/git4idea/backend/src/branch/GitCheckoutOperation.java::runCheckoutAndHandleCommonFailures／smartCheckoutOrNotify，以及GitSmartOperationDialog.java。本地覆盖冲突进入Smart／Force／取消决策；未跟踪覆盖和既有未合并状态不使用该恢复路径。Force在左，Smart与取消在右，上游取消获初始焦点。已安装版本仍未确认。

src/ui.rs::start_local_checkout保留后台结果和固定引用／OID／原HEAD，取消后不再打开恢复弹窗；普通本地覆盖失败将affected_files传给现有show_checkout_recovery。共享弹窗及执行入口增加可选本地完整引用：Some走smart_execute_local／force_execute_local，None保留原detached路径；本地Smart成功要求实际HEAD、attached引用、checkout结果、恢复及冲突状态同时满足。保留stash提示和现有结果刷新机制，未将local恢复退化成detached。无自动Force或新增Git流程。

Windows release通过（18.55秒）；Linux repository local_筛选6项通过，包含本地Smart恢复index／冲突stash与Force陈旧目标／attached测试。artifacts/local-checkout-recovery使用独立临时仓库：main与topic修改同一context.txt，main有本地修改及未跟踪note.txt。原生中文菜单搜索topic→子菜单检出弹出问题对话框，problem.png人工确认文件和三种动作。点不检出，断言HEAD与修改保持；再次进入点Smart，断言attached refs/heads/topic、目标OID、unmerged index、stash保存原local edits、note.txt保持；result.png人工确认恢复冲突提示，assertions.json保存结果。关闭路径核验自有PID33764，启动会话已结束；冲突fixture保留用于复查，未动用户仓库。

本轮补齐此前本地分支只有状态提示的功能缺口。功能阶段UI验收仍部分：共享弹窗目前偏上、固定大尺寸，默认焦点／Enter、Force GUI路径、中英文对照、文件列表的ChangesBrowser行为尚未完成；不把后端Force测试等同原生交互验收。未更新便携包，无新内存预算结论。

## 检出恢复弹窗的初始焦点与键盘动作（2026-10-17）

参考4aec35256d765c217aa0d662b82ac4e860c396fd，重新读取GitSmartOperationDialog.java构造与createLeftSideActions，并读取platform/platform-api/src/com/intellij/openapi/ui/DialogWrapper.java::createJButtonForAction／OkAction：Smart为默认OK动作，取消为FOCUSED_ACTION，Force在左侧；初始焦点和默认动作不是同一概念。已安装Rebased的具体版本与Enter平台分发仍未独立验证，不宣称所有平台键盘语义完全一致。

vendor/gpui-component/src/button/button.rs新增可选focus_handle；未提供时沿用原use_keyed_state生命周期。src/ui.rs::show_checkout_recovery为每次打开创建cancel／force句柄，builder首次调用才聚焦取消，重绘不抢焦点；Smart使用primary，三按钮最小72逻辑像素，保留左右分组。弹窗on_ok按取消／Force焦点分发，否则走Smart默认动作；执行复用固定仓库／引用恢复路径。只在该弹窗使用新句柄API，无新的常驻实体。

第一次Windows原生试验发现仅设置焦点不足：Dialog Confirm先截获Enter，导致取消聚焦时执行默认Smart。artifacts/checkout-dialog-focus保留失败截图与first-failure.json；专用fixture实际转topic并产生恢复冲突，原local edits仍在stash，未操作用户仓库。修正on_ok后重建通过（17.52秒，初次组件重编1分08秒）。第二次试验取消Enter通过，但测试脚本把两个Tab误当Smart，实际经历Close→Force，Space执行了Force；smart-focused.png明确显示Force焦点，first-failure.json记录topic内容、attached分支、未跟踪note保留。该脚本的Smart断言失败是错误的焦点路径，不将其记为Smart通过。

最终artifacts/checkout-dialog-keyboard独立新fixture验证：初始Enter取消，等待2秒后断言原HEAD／local edits保持；重新打开后三次Tab依次经过Close、Force、Smart，smart-focused.png人工确认Smart焦点；Space执行后断言attached topic、目标OID、恢复冲突index、stash中的原修改及未跟踪note均保持，assertions.json通过。三个测试自有进程16320／11424／28236均核验路径后关闭，启动会话结束。中文截图确认初始取消焦点环和Smart主按钮样式；没有滚动测试或新内存结论。

仍待：弹窗位置／尺寸、关闭按钮Enter、Force Enter与Smart Enter专项、英文界面、文件列表交互和同条件Rebased截图。不能把本次主要路径等同全部键盘或像素验收；未更新便携包。

## DialogClose的Enter错误确认修复（2026-10-17）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/platform-api/src/com/intellij/openapi/ui/DialogWrapper.java：CLOSE_EXIT_CODE等于CANCEL_EXIT_CODE，doCancelAction仅取消关闭；沿用前节GitSmartOperationDialog默认／焦点规则。检查本地vendor/gpui-base DialogClose与Dialog宿主：宿主Enter绑定Confirm先于原生按钮激活，关闭trigger只有on_click，导致Confirm冒泡到业务on_ok。

修复前专用Windows fixture原生复现：取消初始焦点→Tab到右上角关闭→Enter，实际执行Smart，HEAD转topic并产生恢复冲突。artifacts/checkout-close-enter/close-focused.png和first-failure.json保留证据，原local edits仍在stash，未跟踪note保持。只操作专用测试仓库，核验路径关闭自有PID14372。

vendor/gpui-base/src/dialog.rs::DialogClose现在在自身分发路径拦截Confirm，stop_propagation后经既有anchor派发Cancel，继续尊重宿主on_cancel及关闭生命周期；未更改其他确认动作、鼠标／Space路径。GITUI-PATCH.md记录修改。该补丁用于通用关闭控件，避免每个业务弹窗单独识别关闭按钮焦点。

Windows release通过（1分22秒）。artifacts/checkout-close-enter-fixed在新fixture中重复关闭按钮Enter，等待2秒断言HEAD与local edits保持，close-focused.png人工确认实际焦点；随后重开、三次Tab到Smart并Enter，断言attached topic、目标OID、恢复冲突index、stash保存原修改和note保持，assertions.json全部通过。自有PID9312核验路径后正常关闭，启动会话结束。只证明该弹窗的Close与Smart Enter路径；其他通用DialogClose使用点、嵌套弹窗／取消veto、Force Enter及英文场景仍待进一步回归。未更新便携包、未声称完整像素或内存验收。

## 检出问题弹窗居中与英文Force Enter验收（2026-10-17）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/platform-impl/src/com/intellij/openapi/ui/impl/DialogWrapperPeerImpl.java初始定位流程：优先保存位置／尺寸与显式初始位置，否则setLocationRelativeTo(owner)。同时读取GitSimplePathsBrowser.java及ChangesBrowserWithRollback.java：文件区是带工具栏／分组的滚动树，并非按文件数量缩成提示框。当前源码不足以确定安装版该弹窗精确尺寸，本轮不凭单文件留白任意缩减高度。

vendor/gpui-component/src/dialog/dialog.rs新增默认false的centered选项，开启时Positioner使用LeftCenter锚点，以已计算横向位置和父视口中心定位实际测量后的弹窗，保留max_height及margin裁限；无需估算弹窗高度或下一帧测量缓存。src/ui.rs仅Checkout Problem启用，其他弹窗保持现有定位。弹窗保存位置／尺寸与可调整大小仍未实现，明确保留差异。

Windows release通过（1分02秒）。artifacts/checkout-dialog-centered新建专用fixture、独立profile显式语言en；problem.png人工确认英文文案完整、弹窗在父窗口中部、Smart主动作和取消初始焦点正确。先Enter取消并等待2秒断言HEAD／local edits未变；重开两次Tab到Force后Enter，断言attached topic、目标OID、tracked context.txt为目标内容、无unmerged index、源main不移动、无关note.txt保持。assertions.json通过，核验路径后关闭自有PID13912，启动会话结束。

本轮补齐Force Enter与英文主要决策路径，没有把Force误当Smart测试；前一轮已分别验证中文Close／Smart Enter。完整视觉仍未通过：窗口尺寸／DPI变化的居中及裁限、原生尺寸持久化、文件树工具栏／分组、同条件Rebased截图均待。未更新便携包，无新内存预算结论。

## Pull／版本输入弹窗共享实际居中（2026-10-17）

重新读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的GitCheckoutFromInputAction.kt、ui/branch/GitRefDialog.java及pull/GitPullDialog.kt，确认后二者使用DialogWrapper；沿用已读DialogWrapperPeerImpl默认相对父窗口定位规则。src/ui.rs::show_pull／open_checkout_input改用centered(true)，移除260／190px的高度估算，不更改操作选项或执行流程。

Windows release通过（17.80秒）。artifacts/operation-dialog-centering使用新临时clone与英文独立profile，打开Pull并将窗口改为1200×750，pull-narrow.png人工确认命令栏、选项与右侧操作区可见且居中；取消后从顶部分支搜索checkout打开版本输入，revision-narrow.png确认输入获得焦点、内容与OK／Cancel可见，再调整为2182×1361并保存revision.png。PNG尺寸经头部读取核验。Esc取消后断言HEAD、全部refs、干净工作区保持，自有PID36028核验路径后正常关闭，启动会话结束。未执行Pull或checkout，不把此布局测试当Git执行回归。

视觉发现：Pull的Modify options仍是内容区整行按钮，与上游不同。已进一步读取GitPullDialog::createOptionsDropDown／createSouthPanel、GitMergeDialog.kt::createSouthPanelWithOptionsDropDown及DialogWrapper::createSouthPanel：该控件为DropDownLink，置于底部左侧辅助区，与右侧确认／取消分组分离。下一步按此结构修正，不能仅缩短原内容区按钮。窗口位置持久化、极端小高度、多DPI仍待验收。

## Pull底部选项链接结构修正（2026-10-17）

依据上一节已读4aec352的GitPullDialog::createOptionsDropDown／createSouthPanel、GitMergeDialog::createSouthPanelWithOptionsDropDown及DialogWrapper::createSouthPanel，src/pull_dialog.rs将既有选项菜单构建提取为options_button，使用link与下拉箭头；src/ui.rs将其放入footer左侧，中间弹性空白，右侧保留Pull／Cancel。移除内容区整行按钮，选中选项仍显示在命令栏下。菜单选项及互斥条件未变，无新增Git命令。

首次编译发现dropdown_menu返回包装元素而非Button，改用AnyElement返回后Windows release通过（17.86秒）。artifacts/pull-options-footer使用新临时clone／英文profile，pull.png人工确认底部左右分组与紧凑链接。点击链接打开菜单，Down／Enter选择--rebase；缩为1200×750后narrow.png人工确认选项显示、链接仍在底部左侧、确认取消仍可见、弹窗按新高度保持居中。取消后HEAD／refs／工作区断言通过，自有PID17416路径核验后关闭，启动会话结束。

结构已修正，保留差异：选中选项当前仍为文字串，尚未按GitOptionsPanel制作可移除的选项控件；菜单说明、快捷键、链接颜色及帮助入口未完全还原。本次只验证一个选项可选、状态更新与取消，不声称互斥全回归或实际Pull执行通过；既有命令逻辑未改变。未更新便携包，无新内存结论。

## Pull已选选项标签与移除交互（2026-10-17）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的plugins/git4idea/backend/src/merge/dialog/GitOptionsPanel.kt、OptionButton.kt及platform/platform-api/src/com/intellij/execution/ui/TagButton.java：空选项隐藏区域；已选选项以带Close的TagButton呈现，标签焦点下Backspace／Delete移除；已有标签保持顺序，新选项追加。src/pull_dialog.rs将UI选择存储改为去重Vec，保持添加顺序，提交时仍交给既有Options构造／兼容性校验；渲染独立标签、关闭按钮与可聚焦删除路径，最后一个移除时隐藏区域，删除后焦点返回分支输入。仅每个弹窗保留6个轻量焦点句柄，无额外查询或缓存。i18n添加中英文移除提示。

首次编译因GPUI不支持(&str,&str)元素ID失败，改为稳定枚举序号后Windows release通过（17.63秒）；Linux pull::tests三项通过，覆盖互斥矩阵、默认策略／rebase分派与参数校验。artifacts/pull-option-tags新临时clone／英文profile：菜单选择--rebase，selected.png人工确认标签；点击标签正文后Delete，deleted.png确认区域消失；再次打开菜单after-remove-options.png核对去选中状态，再选--rebase，点击标签Close，closed-tag.png确认再次移除。退出后HEAD／refs／工作区断言通过，自有PID13640路径核验后关闭，启动会话结束。

本节验证单选项鼠标关闭和Delete路径，不扩大为多标签顺序／Backspace／键盘Tab遍历的完整原生验收。选项菜单描述与禁用原因、标签精确配色／尺寸、可访问描述仍有差异；选项命令不翻译，工具提示本地化。未更新便携包，无新内存结论。

## Pull选项菜单连续选择与说明（2026-10-17）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的merge/dialog/GitOptionsPopupBuilder.kt、OptionListCellRenderer.kt、pull/GitPullOption.kt及shared/resources/messages/GitBundle.properties。上游handleSelect直接切换后repaint而不关闭；isSelectable由实时兼容性决定；行按勾选、描述、间隔50逻辑像素、参数组织，禁用说明变灰。修正此前普通菜单选择即关闭的行为。

vendor/gpui-component PopupMenu增加默认true的close_on_confirm及set_item_state，在原实体更新checked／disabled而不丢失当前键盘行；confirm增加禁用项检查。src/pull_dialog.rs仅选项菜单关闭自动dismiss，仍使用现有Options校验；动作后通过弱引用defer更新6行，不在已借用menu回调中重入，不创建新菜单实体或强引用循环。描述与参数分列，英文说明按上述资源对应，中英文资源在i18n集中提供；原始Git参数保持不翻译。常规菜单默认行为不改。

Windows release通过（1分02秒）。artifacts/pull-options-live新临时clone／英文profile：Down／Enter选rebase后rebase-live.png确认菜单保持、勾选出现且互斥项变灰；再次Enter取消，随后Down／Enter选ff-only，再Down跳过no-ff／squash并Enter选no-commit。two-options.png人工确认两项勾选、两个标签、说明及参数列；Esc先关闭菜单，再Esc退出弹窗。HEAD／refs／干净工作区断言通过，自有PID14496核验路径后关闭，启动会话结束。没有运行Pull或改动用户仓库。

剩余差异：菜单标题、按最长描述动态测宽、selected参数颜色、说明的可访问名称、中文原生／鼠标连续切换与小视口边界仍待；当前宽度550–680逻辑像素。普通菜单默认路径及通用禁用Confirm变更尚未全面原生回归，不以本例替代公共组件全验收。未更新便携包，无新内存预算结论。

## 弹窗与连续选项菜单后的核心／内存回归（2026-10-17）

本轮无产品代码改动，验证当前公共组件与Pull UI改动后的构建。devenv运行cargo test --no-default-features --lib --test repository，116库测试与86真实Git集成测试通过，后者11.64秒；概要artifacts/memory-dialog-current/tests.json。覆盖现有核心场景，不是Windows GUI、vendor组件或完整认证套件的替代。

artifacts/memory-dialog-current/run.ps1复制当前三个release可执行文件到独立验证目录／profile，使用既有干净GitReview-benchmark，确认10000条提交，调用windows-job-memory.ps1 -HistoryLimit 10000 -LargeDiff -MemorySamples 10 -AssertBudget。SHA256 EB126571A22D0007FD8258C749B3CFF967C61E66731BDBFD32C4C1FBA03224A8。history.png人工核对已加载10000、offset 0；window.png确认review.rs完整文件独立Tab。fixture规模沿用975000字节／13000行，无滚动自动化。

memory.json：Job内核峰值154144768字节（147.00MiB），包含自挂起启动起继承的Git子进程；累计73进程，采样末态1进程。任意单进程峰值127.21MiB不是Git独立峰值。steady.json十个Private Bytes样本均91.16MiB、Git子进程样本0、Working Set 90.83MiB。通过稳定150MiB／Job峰值250MiB预算。GPU单次计数Dedicated58114048、Shared3969024、TotalCommitted62873600字节，单独报告，不与Private／Job峰值相加。otherReviewProcesses为空，不包含用户Rebased／IDEA。

脚本退出0，验证benchmark HEAD与工作区不变；后续Win32_Process按gitui-review.exe查询无返回，确认自有PID35964对应程序已退出。截图、采样、执行脚本留在artifacts/memory-dialog-current。结果证明本基准未超预算，不证明连续菜单打开／关闭、写操作网络峰值或长期无泄漏；不能因数值小于先前一次样本就宣称性能改善。未更新便携包；功能与完整视觉目标仍未完成。


## Pull 中文与鼠标连续选择补验（2026-10-17）

本轮无产品代码改动，沿用前节已读取的 GitOptionsPopupBuilder、GitOptionsPanel、TagButton 规则与当前 release。artifacts/pull-options-chinese 使用独立 zh-CN profile 和临时 clone，Windows 原生打开 Pull：options.png 人工确认六项中文说明与原始参数分列；鼠标先选 no-verify，再选 ff-only，two.png 确认菜单持续打开、两项勾选、互斥项禁用，标签按选择顺序而非枚举顺序展示。Esc 关闭菜单后点击首标签正文，Backspace 移除 no-verify，backspace.png 确认保留 ff-only 并回到分支输入焦点。取消后 HEAD／refs／干净工作区断言通过；核验路径后关闭自有 PID24856，未运行 Pull。中文小视口、移除后重添顺序和通用菜单全面回归仍待，不宣称完整视觉验收。

用户补充的 Update Project 与顶部分支菜单原图已核验归档文件逐字节一致，见 artifacts/rebased-user-reference/README.md。当前功能验证收尾后优先执行上方 UI/UX 优先级调整清单：先逐入口对照交互、弹窗结构与分组，再统一样式。Update Project 与显式 Pull 分别核对，不能把参考截图当作同一种操作。当前 Pull 截图仍可见标题区、尺寸、字体、按钮及顶部动作布局差异，功能补验不代表这些差异已解决。


## UI/UX 结构复核与 Push 底部动作（2026-10-17）

本轮重新检查当前实现，参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd，安装版精确版本仍未知。读取 plugins/git4idea/backend/src/update/GitUpdateOptionsDialog.kt、GitUpdateOptionsPanel.kt：Update Project 的 Merge／Rebase 是绑定 settings.updateMethod 的单选组，Reset 是单仓库额外左侧动作，先取消当前窗再独立确认；因此不能将此截图直接套用到显式 Pull 或 Push Rejected。读取 plugins/git4idea/backend/src/push/GitRejectedPushUpdateDialog.kt：使用 warning MessageDialogBuilder，Merge／Rebase／Cancel 按序，默认与初始焦点随设置及 merge 风险分别指定，Remember 通过 DoNotAskOption 处理。

读取 platform/dvcs-impl/src/com/intellij/dvcs/push/ui/VcsPushDialog.java 的构造、createCenterPanel／createSouthPanel／createActions：Push 主动作标记 DEFAULT_ACTION，动作数组为 Push、Cancel、Help；顶区、提交列表、选项区分层，列表 preferred size 为800×450逻辑像素，并保存窗口尺寸。重新读取 DialogWrapper.java::createSouthPanel／createActions，确认 Windows 保留动作顺序；DialogWrapperPeerImpl.java 默认相对父窗口居中。

本地 src/ui.rs::open_push 从 Cancel／Push 改用 dialog_style::confirmation 的 Push／Cancel，复用72逻辑像素最小宽度；启用 centered(true)，不改 spec、执行或关闭语义。Windows release 成功（18.36秒）。artifacts/push-dialog-layout 新临时clone、中文独立profile，dialog.png 原生核对居中、右下方 Push／Cancel 顺序；narrow.png 为1200×750，按钮仍可见但分页与空状态被固定400逻辑像素内容区挤掉，明确小窗口完整验收失败。Esc 取消后 HEAD、refs、干净工作区断言通过；核验路径关闭自有PID8764，启动会话退出0，未执行 Push。保留差异：Help入口、Push分裂按钮、尺寸持久化、标题仓库信息、提交树与列表布局、精确样式，未更新便携包。

当前结构差异清单（本轮检查，不是完整视觉验收）：

| 区域／本地位置 | 已有证据与差异 | 后续修正顺序 |
| --- | --- | --- |
| Push：ui.rs::open_push、push_dialog.rs::render | 本轮源码及原生截图确认按钮顺序和居中修正；400逻辑像素固定内容高度在小窗遮住分页／状态 | 优先修复内容区尺寸约束，保留底部动作与状态，再补列表／选项结构 |
| 风险提示：update_dialog.rs::show／render | 已读 GitRejectedPushUpdateDialog；动作与 Remember 仍在内容 render 中、未居中；自有逻辑焦点与默认动作分别保留 | 先核对 MessageDialogBuilder 下层布局和键盘行为，再迁移共用底部结构，不能套用 Update Project 单选窗 |
| 顶部分支：ui.rs::repository_menu_body | 当前有搜索、版本检出动作、Recent／Local／Remote／Tags和树；搜索输入仍拦截 Left／Right／Home／End，文字编辑与树导航混用。与用户原图相比缺上方工具组织，只有已实现动作 | 优先核对树与搜索的焦点路由；分组／工具入口按真实功能添加。上游对应树类需逐项重读，不能由本轮截图推断键盘规则 |
| 认证：auth_dialog.rs::show | 本地仍 Cancel／Log In，使用240高度估算居中，与新共用结构不一致 | 读取认证对应源码后修正，不直接照常规 OK 弹窗推断焦点和默认行为 |
| 编辑器、中止Rebase、日期、路径 | editor_dialog.rs、ui.rs、paths_menu.rs 仍各自估算居中高度 | 逐个读取业务源码，迁移实测尺寸定位并验证实际内容／取消行为 |
| 通用外观 | 用户原图与当前 Pull／Push 截图仍有标题图标／字重、背景弱化、按钮边框与留白差异；缺同DPI同状态并排证据 | 基于统一组件调整，区分结构问题和精确像素；保留单独视觉验收 |

下一步先处理本轮发现的 Push 小视口内容约束；本清单尚未覆盖所有设置、错误态及菜单，顶部总验收项不勾选。


## Push 小窗口内容约束修正（2026-10-17）

针对前节1200×750原生失败截图，重新读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/dvcs-impl/src/com/intellij/dvcs/push/ui/VcsPushDialog.java::createCenterPanel／createSouthPanel：中心列表使用preferred size并处于BorderLayout中心，选项与南侧操作区独立；不是固定不可收缩的整块内容。检查本地Dialog默认children进入整块滚动容器，Push内部固定400逻辑像素高度使分页和状态落到可视范围外。

src/ui.rs::open_push改用已有DialogContent入口，给弹窗540逻辑像素期望高度，仍由通用max_height按父窗口裁限；content min_h_0并隐藏外部溢出。src/push_dialog.rs::render改为受父容器约束的满高布局，提交／文件双栏占剩余高度，header与分页不收缩，状态留在双栏之外。无需视口高度减魔法常量或新增测量缓存；列表继续使用原有内部滚动，不改变数据加载和Git动作。540为本地期望尺寸，并非声称上游精确高度。

Windows release通过（18.39秒）。artifacts/push-dialog-resize在新clone／中文独立profile中验证：dialog.png核对默认窗口；narrow.png在同样1200×750下确认上一页／下一页、没有待推送提交、Push／Cancel均可见，前节空状态遮挡已修正。Esc取消后HEAD／refs／工作区断言保持，路径核验后关闭自有PID33576，启动会话退出0。未执行Push、未进行滚动自动化。本次覆盖空状态和两种窗口尺寸；有待推送提交／文件详情、目标编辑、长错误、多DPI与更小高度仍待原生核对，不能扩大为全部小窗口验收。未更新便携包，未复测内存预算（本轮仅布局变更）。


## Push 小窗口详情与目标编辑补验（2026-10-17）

本轮无产品代码改动，验证前节布局变更。artifacts/push-details-resize/start.ps1创建专用本地bare remote与client：Base已推送、Review implementation仅在本地，两个提交、单个review.txt。独立中文profile启动当前release，选中待推送提交后调整为1200×750；details-narrow.png人工确认提交行、文件、显示差异入口、提交说明／作者、分页、Push／Cancel均可见。edit-narrow.png确认目标编辑的remote、branch、应用和取消完整可见，Push在草稿状态禁用。

尝试SendKeys被前台检查拒绝，未向其他窗口输入；改用验证过路径的自有HWND定向End／WM_CHAR追加-draft。draft.png核对main-draft草稿；点击编辑区取消后restored.png确认恢复origin:main及原待推送提交。随后Esc退出Push。脚本断言HEAD、refs和干净工作区保持，bare remote main仍为HEAD父提交，未实际推送；核验路径后关闭自有PID16052，启动会话退出0。此次证明单提交／单文件详情和目标草稿取消路径，不替代目标应用、多提交分页、多文件、长文案、多个DPI或更小尺寸验收。未更新便携包，无新内存结论。


## 更新风险提示居中与默认／焦点复验（2026-10-17）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/platform-api/src/com/intellij/openapi/ui/messages/MessageDialog.java：继承DialogWrapper，createActions分别标记DEFAULT_ACTION与FOCUSED_ACTION，消息区在中心、图标在西侧顶部，取消close(-1)。重新读取GitRejectedPushUpdateDialog.kt的Merge／Rebase／Cancel顺序及设置／风险对应的默认和焦点规则、DialogWrapperPeerImpl默认相对父窗居中、JBUI.CurrentTheme.Button.minimumSize的72×24逻辑像素fallback。

src/update_dialog.rs::show启用已有centered(true)，render动作统一最小宽72；没有改Prompt映射、默认动作、选中焦点和取消生命周期。Windows release通过（18.28秒）。artifacts/push-warning-centered创建本地bare remote及专用client，两端从base分叉为incoming／ours，英文独立profile。原生实际Push遭non-fast-forward拒绝后warning.png确认提示居中，Merge为默认蓝色、Rebase为初始焦点；Tab后cancel-focused.png确认Cancel焦点，再Enter取消。脚本验证client HEAD／干净工作区和remote main保持，路径核验关闭自有PID27928，启动会话退出0。只在专用本地remote运行，没有访问用户远端。

仍有明确结构差异：warning图标缺失、Remember仍在内容区、按钮仍随业务实体render而非宿主footer、缺Help入口、分支上下文说明和原生真实焦点树；后续应统一这些结构并回归默认／焦点及复选框键盘行为，不能将当前居中视作完整风险弹窗还原。其他Prompt分支、中文和小视口尚未在本构建重新验证。未更新便携包，无新内存预算结论。


## 更新提示底部区域分离（2026-10-17）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的DialogWrapper.java::createSouthPanel(leftSideButtons,rightSideButtons,addHelpToLeftSide)：DoNotAsk复选框与Help／附加控件位于南侧左区，右侧动作组经弹性空白分隔；消息内容不负责承载底部操作。GitBundle.properties中的push.rejected.remember.checkbox含说明与设置入口文案。结合前节MessageDialog的消息中心区规则，本地src/update_dialog.rs将复选框和动作组移到宿主footer，Remember在左并可换行，动作保持右对齐且不收缩；没有Remember的Prompt保留弹性左空白。

新增按弹窗创建的UpdateFooter视图，仅持有UpdateDialog弱引用并观察其notify，键盘选中项／复选框变化同步刷新底部，不复制Pending或建立强引用循环。原业务实体仍保留键盘上下文和决策状态，默认／初始焦点与取消映射不变。此处仍是逻辑选中态而非各控件独立原生焦点，保留差异。

Windows release通过（18.38秒）。artifacts/push-warning-footer使用新本地分叉client／bare remote和英文独立profile，实际触发推送拒绝；warning.png确认底部左右组织、说明换行和Rebase初始焦点。两次Tab到Remember、Space选中，remember-checked.png确认勾选与焦点边框；再次Space取消，缩为1200×750后继续三次Tab到Cancel，cancel-focused.png确认新底部状态随键盘刷新、内容完整，Enter取消。HEAD／干净工作区与remote main保持断言通过，自有PID26204路径核验后关闭，启动会话退出0。未涉及用户远端。

尚未完成：中文、多风险Prompt、鼠标复选框／动作、记忆选项实际持久化的本构建回归；Warning图标、Help、真实焦点树、上游分支上下文说明及精确样式。新增footer生命周期经弱引用设计检查，本轮未做长时反复打开的内存验证。未更新便携包，整体视觉与功能目标继续。


## 更新提示 Warning 图标与中文补验（2026-10-17）

参考4aec35256d765c217aa0d662b82ac4e860c396fd，读取MessageDialogBuilder.kt::asWarning → UIUtil.java::getWarningIcon，确认OptionPane.warningIcon／AllIcons.General.WarningDialog；读取IdeaLaf匹配项、AllIcons.java资源映射，确认New UI使用expui/general/warningDialog.svg及深色资源。读取MessageDialog.java::createIconPanel：图标在西侧顶部，消息居中，间隔15逻辑像素。进一步核对GitRebaseUpdater→GitRebaseProcess::askIfShouldRebasePublishedCommit与GitRebaseOverMergeProblem::doShowDialog，另外两种Prompt也使用warning图标。

从上游导入platform/icons/src/expui/general/warningDialog_dark.svg到assets/icons/warningDialog_dark_4x.svg，仅将固有尺寸28×28改为112×112，保留viewBox／路径／颜色／copyright，沿用项目彩色图标缓存渲染方式；现有Apache-2.0许可证保留，SOURCE.txt新增来源。src/assets.rs注册资源，src/update_dialog.rs消息区增加28逻辑像素彩色图标、顶部对齐和15逻辑像素间隔，消息可伸缩；没有更改Git操作和决策规则。

Windows release通过（18.04秒）。artifacts/push-warning-icon为独立中文profile与新本地分叉client／bare remote；实际Push拒绝后warning.png人工确认黄色图标、中文消息和底部控件；Tab／Space勾选及取消Remember，缩到1200×750后cancel-focused.png确认图标与完整中文控件、Cancel焦点，Enter取消。脚本验证HEAD／干净工作区和remote main保持，自有PID24872核验路径后关闭，启动会话退出0。仅验证Rejected分支的中文主要路径；另外两种Prompt虽源码确认图标，本构建仍待原生回归。未更新便携包，未声明像素级完成或新增内存结论。


## HTTP 认证窗共用定位与按钮组（2026-10-17）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的plugins/git4idea/backend/src/commands/GitHttpLoginDialog.kt：DialogWrapper派生，Log In替换OK标题；说明、用户名／密码标签与填充输入列；空用户名优先用户名否则密码焦点；空值按字段校验，另有Remember／helper／provider路径。重新读取DialogWrapper标准动作顺序OK／Cancel；定位沿用已读DialogWrapperPeerImpl规则。

src/auth_dialog.rs::show删除240逻辑像素高度估算，使用centered(true)；改用dialog_style::confirmation的Log In／Cancel顺序和统一最小宽。未更改凭据请求、脱敏目标、密码掩码或认证流程。Windows release通过（17.99秒）。artifacts/auth-dialog-layout使用新空仓库、独立英文profile及仅Loopback监听的401测试服务；Fetch真实触发认证。login.png核对用户名初始焦点、居中与按钮顺序；Enter空提交后缩为1200×750，empty-narrow.png确认校验文案与实际增高后的居中；输入虚构fixture-user／fixture-password，masked.png确认密码掩码。点击Cancel，等待操作结束，断言无refs和干净工作区。自有PID20588核验路径后关闭；测试HTTP服务核验命令行包含本轮脚本后关闭，启动会话退出0。没有使用实际凭据或外部远端。

保留差异：当前空值错误为整体提示且输入后仍显示，尚非上游逐字段errorOnApply；Remember持久化、helper选择、外部provider、预填用户名场景未完整映射。成功认证／中文／密码单字段模式未在本构建新增回归；仅本轮布局、空值与取消路径通过，不能等同完整认证验收。未更新便携包，无新内存结论。


## 认证字段分别校验与编辑反馈（2026-10-17）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的GitHttpLoginDialog.kt::buildCredentialsPanel：用户名／密码各自errorOnApply，非空白约束；CellImpl.kt::errorOnApply／addValidationRule和CellValidationImpl.kt::installValidationOnApply／guessValidationRequestor确认校验关联具体组件、文本变更触发重新校验。进一步读取DialogPanel.kt及DialogPanelValidator.kt::applyValidations／ApplyValidationRequestor：提交时激活apply规则，普通输入使用input规则，不能假定空值错误始终即时常驻。

src/auth_dialog.rs将单error拆为username_error／password_error，确认时分别验证；仅订阅InputEvent::Change清除对应提交错误，不用每帧／焦点notify清除；输入框下显示各自提示，标签列固定不收缩、输入列填充。i18n添加分别的中英文文案。仍使用内联错误文本，而非JetBrains ComponentValidator提示外观，明确保留视觉差异。

Windows release通过（24.57秒）。artifacts/auth-field-validation新本地Loopback401服务／空仓库／独立英文profile：真实Fetch触发认证，Enter空提交后empty-narrow.png在1200×750确认两项分别提示；输入虚构用户名后username-fixed.png确认只清除用户名错误，密码错误保留；输入虚构密码后masked.png确认密码掩码且两项提示消失，弹窗随高度变化保持居中。点击Cancel，断言无refs／干净工作区，核验路径关闭自有PID29616及命令行匹配的HTTP服务，启动会话退出0。未发送实际凭据或访问外部远端。

仍待：中文本构建截图、空白字符串重复提交、密码单字段模式、成功认证，以及错误提示的可访问关联／焦点跳转和ComponentValidator视觉。校验本身没有扩大认证范围；未更新便携包，无新内存预算结论。


## 顶部分支搜索文字编辑与导航分流（2026-10-17）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的plugins/git4idea/shared/src/com/intellij/vcs/git/branch/popup/GitBranchesPopupBase.kt：getPreferredFocusableComponent返回tree；processKeyEvent中Up／Down／Enter转交tree，ACTION_FIND聚焦搜索编辑器，搜索框实际聚焦时其余按键由文本编辑处理；树Right叶子打开子菜单，Home／End在过滤时选择首尾。此前本地将搜索Input上的Left／Right／Home／End无条件截获为树操作，不能编辑查询中间字符。

src/ui.rs新增每个Review实例一个repository_search_editing标记：打开菜单默认树导航语义，点击搜索框／菜单打开时Find启用文字编辑；此时Left／Right／Home／End交回Input；Up／Down／Enter恢复原树动作。Find在菜单打开时指向菜单搜索并关闭其子菜单，不再离开当前上下文跳到Log过滤输入。底层仍保留Input平台焦点负责文本和IME，逻辑状态区分语义；不是已经实现独立JTree式真实焦点／可访问树。没有新缓存或后台任务。

Windows release通过，清理重复赋值及补注释后的最终构建18.33秒。artifacts/branch-search-editing新clone／英文profile：打开顶部分支菜单，点击搜索输入，输入main，Left后插入x，left-insert.png为maixn；Home插入a、End插入z，home-end.png确认为amaixnz而非误展开／跳转树节点。Esc取消，HEAD／refs／干净工作区断言通过，自有PID26480路径核验后关闭，启动会话退出0。

本次原生证明鼠标进入搜索编辑后的Left／Home／End路径；Ctrl+F、Right、Up／Down返回树、子菜单恢复、中文IME和真实辅助技术焦点仍待进一步验收。初始Input视觉焦点与上游tree实际焦点仍有差异，不能宣称完整键盘导航还原。未更新便携包，无新内存结论。


## 分支搜索编辑返回树导航补验（2026-10-17）

本轮无产品代码改动，验证前节分流状态。artifacts/branch-search-navigation新clone／英文profile，打开顶部分支菜单并点击搜索输入：输入main，Left再Right后插入x，right-edit.png确认为mainx，说明编辑模式Right回到文字末尾而非打开子菜单。Backspace恢复main，Down返回树导航并选择origin/main，Right后right-tree.png确认打开Copy Reference Name子菜单；Left后left-return.png确认只关闭子菜单，保留父菜单、查询和选中分支。再点击搜索框，Home插入z，editing-restored.png为zmain，确认能再次进入文字编辑。

Esc退出，HEAD／refs／干净工作区断言通过，自有PID36768核验路径后关闭，启动会话退出0。未执行checkout或剪贴板动作，没有滚动自动化。Ctrl+F、中文IME、Up／Enter切换、多个子菜单状态以及真实可访问焦点仍待；本例只证明Right编辑、Down切回树、Right展开／Left返回和鼠标重新编辑的组合路径。未更新便携包，无新内存结论。

## 顶部 Git 图标与左侧工具栏尺寸（2026-10-17）

参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd（安装版本仍未知）。读取 intellij.vcs.git.backend.xml 的 Git.Pull／Git.Fetch 声明、GitPull.java 的 GitPullDialog 入口、intellij.platform.vcs.dvcs.impl.xml 的 Vcs.Push 声明以及 AllIcons.java；从 platform/icons/src/expui/vcs 导入原始 fetch_dark.svg／push_dark.svg，注册 src/assets.rs 并在 assets/icons/SOURCE.txt 保留来源及 Apache-2.0 许可。显式 Pull 声明未指定图标，保留文字入口，不能将 Update Project 箭头直接映射成 Pull。前轮 artifacts/git-action-icons 已验证本地 Fetch 完成、Push 对话框打开与取消、HEAD／refs／工作区不变，自有 PID36236 已关闭。

本轮进一步读取 platform/platform-impl/resources/idea/PlatformActions.xml：MainToolbarVCSGroup 属于 MainToolbarLeft，位于项目入口后；MainToolbarRight 包含搜索和设置。MainToolbar.kt::createActionBar 统一按钮尺寸，ToolbarFrameHeader.kt 将标题内容 CENTER 填充；CustomWindowHeaderUtil.kt 按 normal／compact 模式取高度，CustomHeader.kt 的 normal 为40逻辑像素。ActionToolbar.java 委托 JBUI.CurrentTheme.Toolbar：按钮 fallback 30×30、图标20。主题或运行设置可以覆盖，不能把 fallback 当作已安装版本测量值。

src/ui.rs::toolbar 将 Fetch／Push／Pull 移到仓库名之后、分支菜单之前，设置保留右侧；外层 TitleBar 设40，内层填充并垂直居中。顶栏按钮从24改为组件标准32，字体14、主要图标20；分支触发器同步调整。vendor/gpui-component/src/button/button.rs 增加可选 icon_size builder，使用原有尺寸字段，避免 Button 重设 Icon 的尺寸，默认行为不变。用户指出仍偏上后，移除 Log Tab 上方4逻辑像素同色留白以及内层重复边线；该留白原本扩大了视觉上的顶栏底部空间。

保留差异：按钮32而非上游fallback30，Open／仓库名仍未合成项目选择器，Fetch／显式Pull 与上游 Update Project 组合不同；精确字体、完整顶部动作组织、长仓库／分支名以及操作进行中窄窗溢出待验收。本轮没有改 Git 流程，也没有新缓存；不更新便携包、不作新的内存预算或完整像素级还原结论。

最终 Windows release 构建通过（18.23秒）。artifacts/top-toolbar-layout 使用独立clone／英文profile：window.png 和1200×750的 narrow.png 已人工核对左侧动作、放大图标及顶部中心线，去除底部留白后图标与窗口控制同处顶栏中心。点击左侧 Fetch 对本地源完成获取；Push／Pull 对话框及分支菜单均打开并 Esc 取消，截图分别保存。HEAD／refs／干净工作区断言通过；初版自有 PID34908 和最终 PID6352 均核验路径后正常关闭，启动会话退出0。未执行 Push／Pull／checkout，未关闭用户 Rebased。窄窗截图另暴露右侧提交详情底部裁切，属于既有内容区域问题，仍待修复；本轮仅验收顶部正常状态的布局与入口。

## 显式 Pull／Fetch 回归 Git 菜单（2026-10-17）

用户再次指出顶部不应有 Pull 文字按钮。本轮按真实入口修正：src/ui.rs::main_menu 新增左上主菜单的 Git 子菜单，按已支持范围提供 Push／Pull／Fetch；顶部保留 Push，删除直接 Pull／Fetch。处理器继续调用原流程，构建菜单时禁用忙碌／设置状态，并在执行时重查。中英文主菜单提示加入 src/i18n.rs。新增主菜单图标取自参考 commit 4aec35256d765c217aa0d662b82ac4e860c396fd 的 platform/icons/src/expui/general/windowsMenu@20x20_dark.svg，原样保留及记录 Apache-2.0 来源。

源码依据：intellij.vcs.git.backend.xml::Git.MainMenu 中 Push／UpdateProject／Pull／Fetch 分立；Git.Experimental.Branch.Popup.Actions 注释说明 UpdateProject／Push 在 Rebased 移到顶栏。MainMenuButton.kt 默认 AllIcons.General.WindowsMenu_20x20，AllIcons.java 对应 expui/general/windowsMenu@20x20.svg。安装版本未知。GitUpdateEnvironment.java::performUpdate 构造 GitUpdateProcess 并启用 merge 风险／tracking 检查；GitUpdateOptionsDialog.kt 的 Reset 先关闭当前窗，再检查 attached branch／tracking 并独立警告确认。

Update Project 尚未接入，不使用 Fetch／Pull 或空按钮冒充。下一步需完整读取 GitUpdateProcess 的 readiness、tracking、fetch 后重新读取、updater 选择、保存／恢复流程及测试；本轮只检索到了这些入口，尚未完整验证实现。现有 update_session 可作候选复用，update_preferences 当前存储的是 push-update-preferences，不能直接把其中自动更新推送授权当作 Update Project 的 Don't show again。主菜单目前只有已支持的 Git 子菜单，其余主菜单功能、图标、快捷键及可访问性仍需继续还原。

Windows release 构建通过（24.82秒）。artifacts/git-main-menu：新本地clone与英文profile，鼠标从主菜单 Git→Pull 打开原Pull窗后Esc取消，再从Git→Fetch对本地源获取，fetch-finished.png确认Fetch completed；HEAD／refs／工作区断言通过，自有PID24320核验后正常关闭，启动会话退出0。初次定向键盘Right／Down／Enter没有打开Pull，仅展开了子菜单（pull.png实际仍为菜单，不能作为成功证据）；有效证据为pull-click.png，键盘导航仍待修复／复验。未执行Pull或Push、未更新便携包、无新内存结论。右侧详情窄窗问题本轮仅源码阅读，未修改，继续待办。

## Update Project 只读前置检查（2026-10-17）

继续读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的GitUpdateProcess.java::update／updateImpl（142–275）：readiness→tracking→Fetch→重新读取tracking→选择updater；Rebase可先fast-forward，merge风险决策在保存改动之前；按updater决定保存，只有非Incomplete且成功结果才自动恢复。另读checkTrackedBranchesConfiguration／fetchAndNotify：单仓库detached或无实际tracking拒绝，Fetch只针对目标remote。读取GitUpdateMethodTest.kt的默认Merge、branch.rebase优先级、interactive／preserve与pull.rebase场景；这些属于现有update_method的依据，不是新前置检查测试的直接上游用例。

新增src/project_update.rs::prepare：读取operation_state和pull_targets，拒绝未完成操作／冲突、detached、无现存远程tracking；固定当前branch／HEAD／target并调用已有merge_update::validate复验，返回供更新器使用的Request，不执行Fetch、stash或工作区写入。保留差异：单仓库范围；沿用本地更广的operation_state拦截（含cherry-pick等），没有上游多root同步／子模块跳过流程；当前request沿用reinstate_index=true策略。UI、独立Update Project设置及执行调度仍未接入，不能宣称按钮已可用。merge_update与rebase_update现有worker会在Fetch前后复验，后续接线不能绕过它们。

新增两个真实Git集成测试覆盖非origin且不同名upstream、没有tracking不回退、远程ref消失、detached、取消、实际merge冲突，以及前置读取不改变HEAD／冲突index／staged／unstaged／untracked或创建stash。仅用临时仓库和本地bare目标，无外部网络；Linux定向测试2项通过。没有可见界面改动，无本轮截图或便携包更新。
Windows原生定向测试同样2项通过（4.10秒），使用宿主Git，输出artifacts/project-update-preflight/windows.txt；测试二进制构建5.52秒。这里只证明前置检查及其只读性，未覆盖完整Update Project端到端流程。

## Update Project 单次更新调度（2026-10-17）

继续读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的GitUpdateProcess.java::updateImpl剩余恢复决策、findRootsRebasingOverMerge、tryFastForwardMergeForRebaseUpdaters、defineUpdaters与fetchAndNotify：依据选定方式创建updater，Rebase本地改动可先fast-forward优化，风险决策先于保存；单次更新完成后没有推送。重新读取GitRebaseOverMergeProblem.java::hasProblem：检查range内merge的changes非空，而非只检查存在merge节点；上游查询异常返回false，本地沿用已有Unknown需确认的差异。

src/project_update.rs新增execute与Report，固定用户确认前的Request，按Merge／Rebase分派既有更新器。Rebase未预先经过Push拒绝提示，因此必须接收风险callback；明确无风险时才略过确认，published callback独立保留。没有调用push_flow或复用其自动重试／偏好写入。Report::complete复用更新器完整恢复检查，避免merge/rebase命令成功但stash恢复冲突被宣称为完成。认证／编辑器仍由调用方服务；本轮尚未接入UI。

新增真实Git测试场景：不同于origin的本地bare目标；两端分叉后的Merge与Rebase；NothingToUpdate；确认后切换分支必须在Fetch前拒绝；带merge-only改动的Rebase风险取消在保存前生效。各场景验证staged／unstaged／untracked、stash清理和bare remote未被推送。初版风险fixture只是普通merge，按上游规则不会警告，已修正为merge-only内容，不改变产品判断迎合测试。Linux本模块3项测试通过（4.19秒）。端到端UI、独立设置／Don't show again／Reset等仍待，不宣称Update Project完整完成。
Windows原生3项定向测试通过（39.22秒），包括上述五种调度模式，使用宿主Git与临时本地bare remote；输出artifacts/project-update-worker/windows.txt，测试构建2.44秒。无本轮可见UI或便携包更新。

为后续弹窗读取GitUpdateOptionsPanel.kt（Merge／Rebase单选绑定settings.updateMethod）、GitUpdateConfigurable.java（apply/reset/dispose）、platform/vcs-impl/src/com/intellij/openapi/vcs/update/UpdateOptionsDialog.kt：Don't show again对应StandardOption.UPDATE，setToBeShown仅onOk保存；不能借用Push自动更新授权。基础UpdateOrStatusOptionsDialog的完整生命周期仍待读，不将未读实现当作证据。

## Update Project 顶部入口与首次原生接线（2026-10-17）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的UpdateOrStatusOptionsDialog.java：单VCS中心面板、reset初始化、OK逐个apply，取消不保存options；前节GitUpdateOptionsPanel的纵向Merge／Rebase单选结构保持。VcsActions.xml中Vcs.UpdateProject使用AllIcons.Actions.CheckOut并位于MainToolbarVCSGroup首位；AllIcons.java映射expui/vcs/update.svg。原样导入update_dark.svg，登记assets与Apache-2.0来源，安装版版本仍未知。

src/project_update_dialog.rs提供纵向两种更新方式；src/ui.rs新增open_project_update／start_project_update，将顶部左下箭头与Git菜单Update Project接入前置检查、固定Request的确认和单次worker。对话框居中、560逻辑像素宽、OK／Cancel右组；确认前切仓保护、准备期间取消、执行期间认证／Git编辑器／风险确认走现有通道。完成后刷新仓库与saved stash入口；取消／不完整恢复不能显示成功。状态与新增文案支持中文。src/main.rs注册模块。

明确尚存差异：更新方式暂时每次默认Merge，独立设置持久化及Don't show again尚未实现；Reset to Remote Branch、Help和Ctrl+T未接；当前Radio选中用勾而非参考圆点，字体／窗口装饰／间距与焦点还待还原。没有放置无行为控件，不能把当前弹窗视为完整上游实现。认证／风险／编辑器接线虽复用已有通道，本轮未新增这些路径的Update Project原生验证。

Windows最终release构建通过（25.03秒）。artifacts/project-update-ui独立英文profile、本地bare remote：点击更新箭头，dialog-ready.png确认两种方法与Merge默认；Esc取消断言HEAD和origin/main均未改变（未Fetch）。再打开并点击OK，真实Fetch与fast-forward完成，updated.png显示Project updated；断言HEAD达到incoming、本地valuable local edit保留、无残留stash、bare main未改。自有PID37176核验路径后正常关闭，启动会话退出0。没有用户仓库写入、没有Push、便携包未更新。
中文补验artifacts/project-update-rebase-ui使用另一套本地bare／seed／client与中文独立profile：两端分叉且client带本地未提交改动；rebase-selected.png人工确认第二项已选，点击确定后真实Rebase完成，updated.png显示项目已更新、本地提交位于incoming之上。断言HEAD^等于incoming、local.txt内容保留、无残留stash、bare main保持；自有PID37680路径核验后关闭，启动会话退出0。这里只验证无冲突私有提交的Rebase，复杂风险、认证失败／成功、取消执行和冲突恢复仍需Update Project专门原生回归。中文截图也显示已有部分元数据英文（committed on等），不宣称整个客户端已完成中文覆盖。

## 顶栏图标比例与右下操作状态（2026-10-17）

用户指出名义居中后仍不统一，并要求操作信息移到右下、移除固定底栏文字。核对本地Button显式icon_size与TitleBar::ControlIcon：此前顶栏20逻辑像素图标、约14文字、窗口控制Icon::small约14，视觉重量不匹配。参考仍为4aec35256d765c217aa0d662b82ac4e860c396fd；读取IdeStatusBarImpl.kt的centerPanel／rightPanel及createInfoAndProgressPanel（信息／进度由底部状态栏承载），InfoAndProgressPanel.kt的进度控件与状态栏注册。精确右对齐是本次用户要求，不声称源码所有消息都在右下。

src/ui.rs顶栏主菜单／Open／Update／Push／分支图标缩为16，Settings缩为14，保持40栏高和32按钮点击区域；不改窗口系统按钮。删除底栏Git review和固定快捷键说明（快捷键绑定保持），fetch_status及Cancel／Show saved changes移到底栏右侧。空闲无状态时底栏留空，未将错误诊断顶部banner一并迁移。仍有SVG实际绘制边界、字体字重、不同图标笔画及状态过长时截断等细节待统一，不能仅按名义尺寸称像素级完成。

Windows release通过（25.67秒）。artifacts/toolbar-balance-status新clone／英文profile：原生顶部与1200×750的narrow.png已查看，设置与窗口按钮比例改善；Git菜单Fetch对本地源完成，右下Fetch completed，顶部无操作消息／底部无固定提示。HEAD／refs／工作区断言通过，自有PID26532核验路径关闭，启动会话退出0。快速Fetch截图未捕获进行中的Cancel，不声称本构建已原生验证取消／saved stash入口点击；未更新便携包，无新内存结论。

中断前的“不再显示”工作：新增src/project_update_preferences.rs及lib注册、中英文文案暂保留为未接入存储实现；本轮撤去尚无重新启用入口的弹窗复选框、跳过弹窗与保存接线，恢复上一版Update Project确认行为。该存储模块尚未运行专项测试；后续连同设置页恢复入口、取消不保存、写失败留窗与原生持久化验证一并完成，不将其标记已交付。

## Update Project 确认偏好与共用更新方式（2026-10-17）

重新读取参考4aec35256d765c217aa0d662b82ac4e860c396fd：platform/vcs-impl/src/com/intellij/openapi/vcs/update/UpdateOptionsDialog.kt::setToBeShown只在OK保存；plugins/git4idea/backend/src/update/GitUpdateOptionsPanel.kt绑定GitVcsSettings.updateMethod；plugins/git4idea/shared/src/git4idea/config/GitVcsSettings.java及backend/src/push/GitPushOperation.java::savePushUpdateSettings/readPushUpdateSettings确认Push与Update Project共享更新方式。因此替换上一轮未接入的独立method文件实现，不引入两套互不一致的Merge/Rebase偏好。安装版版本未知。

src/project_update_preferences.rs读取已有push-update-preferences的strategy，BranchDefault通过仓库配置解析；仅把show-options存入单独project-update-options。确认保存方式时重新加载并保留auto_update；只有方式写入成功才保存隐藏选项。缺失／截断／无效记录恢复确认。src/project_update_dialog.rs、src/ui.rs接入已保存方式、底部左侧Don't show again、OK保存／取消不保存，保存错误留窗。每次点击Update仍先检查仓库状态和目标，隐藏选项不绕过前置检查。src/update_settings.rs沿用唯一更新方式选项，新增Show Update Project options恢复入口。

Linux定向验证3个存储测试与4个真实Git测试通过（repository 4.17秒），覆盖共用方式、BranchDefault、读取不写、取消草稿、Push授权不混用、错误记录与写失败，以及既有更新调度。Windows release 24.87秒。原生artifacts/project-update-options：本地clone英文profile，勾选后取消没有生成偏好文件；选Rebase并确认后保存且更新完成，再次触发跳过弹窗；设置页显示共享Rebase，勾选恢复后重新打开弹窗且保留Rebase选中。ready／remember-selected／skipped／git-settings-ready／restored.png为证据；首次git-settings.png误点到导航空白，实际仍是Language页，不作为Git页验收。HEAD／工作区未改变，自有PID30728核验后关闭，启动会话退出0。

差异和待验收：按worktree保存而非IDE project；设置恢复项暂在Git页Update区而非上游完整Version Control确认选项页。BranchDefault打开弹窗时显示解析后的二选一，OK转为显式方式；无弹窗时保持BranchDefault。两个小文件非事务保存，第二步失败会留下已保存方式但弹窗仍保留错误，不能宣称原子保存。原生写失败／损坏配置／重启后恢复及中文新增控件尚待专门验证；Reset、Help、Ctrl+T、Radio圆点及完整视觉还原继续待办。未更新便携包，无新内存结论。

## 底部状态栏高度与文字中心（2026-10-17）

用户指出单行文字无需如此高的底部区域。检查src/ui.rs发现24逻辑像素状态栏上方还叠加内容容器pb(8)，同色背景视觉合计32，文字自然偏下。对照参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/platform-impl/src/com/intellij/openapi/wm/impl/status/IdeStatusBarImpl.kt：minIconHeight=18+1+1，getPreferredSize按文字／图标高度和insets取最大值，文本widget在中心布局。此依据用于紧凑居中，不声称本地20就是所有Islands缩放下的上游最终尺寸。

本地删除内容容器底部8留白，状态栏24→20，12号文字明确16行高；底部Cancel／Show saved changes改XSmall（20高），保持状态文字与控件居中，避免24高按钮溢出。没有改变Git操作或快捷键。

Windows release通过（19.17秒）。artifacts/statusbar-height：本地clone英文profile完成Git→Fetch，fetch-finished.png及1200×750的narrow.png人工检查，底栏紧凑，文字上下留白接近对称；HEAD／refs／工作区断言通过。自有PID29416核验路径后关闭，启动会话退出0。操作进行中和saved stash按钮点击仍未在本构建专门复验；中文字形／其他DPI待验收，便携包未更新，不宣称全面像素还原或新内存预算达标。

## Update Project 偏好异常与中文原生补验（2026-10-17）

本轮不改产品代码，复验上一节新增功能。artifacts/project-update-options-failure复用自有临时clone，使用最新底栏构建和中文profile重新启动：restarted.png确认重启后保留Rebase且恢复的确认开关生效。将私有project-update-options文件暂换成目录模拟拒绝写入；勾选不再显示后确认，save-failed.png显示中文“无法保存设置：拒绝访问”，弹窗保留；FETCH_HEAD时间戳断言不变，未执行Fetch。随后移除测试障碍、写入截断记录并取消重开，corrupt-restored.png显示确认恢复且不再显示未勾选。再次确认可保存有效记录并完成更新（recovered.png显示已是最新）；git-chinese.png确认设置恢复入口、更新方式和Push自动更新选项中文显示、Rebase仍选中。

验证在自有临时仓库完成，HEAD／工作区断言未改变；自有PID13896核验路径后关闭，启动会话退出0。这里的恢复是取消重开后的成功确认，未测试同一错误弹窗内直接重试；不将它当作已覆盖。目录写失败模拟不等价于所有磁盘故障；两个偏好文件部分写入的既知差异仍保留。中文主界面还可见Branches、committed on等英文，完整本地化继续待办。本轮未更新便携包或重新评估内存预算。
Windows原生专项测试同样通过：3个存储单元测试（0.06秒）＋4个真实Git集成测试（39.84秒），使用宿主Git与临时本地bare remote，输出gitui_review-tests.txt／repository-tests.txt。测试二进制构建4.73秒；没有扩大到整套回归或由此声称其他功能已验收。

## 提交详情中英文即时切换（2026-10-17）

对照参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/CommitPresentationUtil.java::getAuthorAndCommitterText/getCommitterHtml（250–295）及resources/messages/VcsLogBundle.properties的四个author/committer消息：作者时间始终显示；提交者或时间不同才增加说明，并分别处理只换人／只换时间／两者都变。沿用本地相同绝对时间不重复显示的规则；本地人物比较仍为完整identity字符串，不等同上游VcsUserUtil全部同人语义。安装版版本未知。

src/i18n.rs新增类型化AuthorDate／Committer／ShallowParents消息，按中文语序组合；src/commit_presentation.rs继续先转义仓库数据及编码mailto，再插入消息，中文日期去掉英文at。本地仅对当前选中提交保留中英文两份短metadata，不复制正文／文件列表／Diff，也不为切换语言重新读取Git；src/ui.rs按当前语言取对应metadata。旧实现只缓存一份英文，因此全窗口刷新也不会翻译它。替换选中提交时两份一起释放。

3项presentation单元测试通过，覆盖四种提交者／时间组合、中英文内容、相同时刻不重复、HTML转义、恶意邮箱编码及正文不改变。Windows release通过（24.84秒）。原生artifacts/commit-language用临时clone并仅在fixture准备阶段amend产生不同Reviewer提交者：chinese.png显示中文作者／提交者说明，english.png显示同一hash和正文的英文说明，往返无需重新选择提交；脚本验证语言偏好文件和HEAD／干净工作区。首次英语坐标误点空白，已修正坐标并重跑，最终截图才作为语言切换证据。

分支竖条仍为Inter英文轮廓SVG，未借此改成不同布局；需继续按ExpandStripeButton旋转文字语义补中文资源或文字绘制。日期仍采用本地时区的固定年月日格式，没有完整地区日期格式化。整体F1未完成，其他DPI／窄窗详情仍待验收；无新内存预算结论，未更新便携包。
chinese-returned.png已查看，往返后仍选中同一提交并恢复中文说明；断言通过。自有PID5440核验路径后正常关闭，启动会话退出0。

## 设置改为模态弹窗（2026-10-17）

用户指出设置直接替换Log区域缺少清晰退出入口，提供JetBrains独立Settings窗口截图，允许采用Tab或弹窗；本轮选择与参考更接近的模态弹窗。原图归档artifacts/settings-dialog/rebased-reference.png。读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/platform-impl/src/com/intellij/openapi/options/newEditor/SettingsDialog.java（85–270）：IDE模态、中心editor、COMPACT布局、OK/Cancel/Apply动作顺序、apply成功才关闭；最小900×700和尺寸服务是独立窗口规则。本地采用900×650逻辑像素、按宿主可用区域收缩的应用内弹窗，不能等同原生独立系统窗口；安装版版本未知。SettingsEditor.java的树/搜索/分隔结构只检索定位，未将其完整行为当作已验证依据。

src/ui.rs::open_settings使用现有Dialog承载：标题和右上角关闭、左侧分类、右侧内容、底部确定/取消/应用，点击遮罩不关窗；on_close清理草稿，on_ok仅保存成功关闭。保留settings_session异步Git偏好加载保护。欢迎页和仓库页均接此入口，原内容区域不再替换成设置；打开设置不调用leave_diff_tab，后台Tab选中态保留。左侧分类改左对齐、版本控制分组缩进和蓝色选中条，内容标题改14号半粗，按钮最小72逻辑像素。现有Apply/Cancel/保存错误逻辑复用，没有重写Git设置。

初次Windows release19.77秒；检查后进一步调整分类列表，最终release20.05秒。artifacts/settings-dialog：初版修改语言草稿后右上角×关闭，偏好仍en；最终版Apply切中文后弹窗保持打开（applied.png），Cancel返回Log，重开保持中文，Esc关闭；Git页异步加载可见（git-ready.png）。用户参考图与本地主要结构一致，但本地仍缺搜索/完整分类树、帮助、项目名标题、独立系统窗口和尺寸持久化；这些是明确的剩余结构差异，非像素级完成。未放置无行为搜索框/帮助按钮。欢迎页、Diff Tab背景保持和保存失败的弹窗专门回归仍待补验。
最终补验：Git草稿修改后Cancel未生成偏好文件；OK关闭设置；1200×750原生narrow.png确认按窗口收缩后标题×及底部三按钮仍完整可见。HEAD／工作区断言通过。自有初版PID33764及最终PID37424均核验路径后关闭，两次启动会话退出0。未更新便携包，无新内存预算结论。

## 设置细节对齐，暂不提供搜索（2026-10-17）

用户明确目前设置少，不需要搜索；已撤回刚开始的settings_search模块、输入状态、监听及文案，不再把搜索作为当前阶段交付要求。新参考图归档artifacts/settings-details/rebased-reference.png。沿用已读4aec35256d765c217aa0d662b82ac4e860c396fd的SettingsDialog.java::COMPACT/createActions，并重新读取145–195；核对新截图的普通字重窗口标题、分类缩进、面包屑和底部分隔按钮区。

src/ui.rs设置标题增加当前仓库名；取消容器叠加的底部padding，按钮区固定50逻辑像素、水平16及12间距。分类宽240、行高26，子分类加深缩进，内容标题改Version Control › Git/Diff；语言与Diff位置使用单选控件，替代按钮选中态。仍沿用应用内弹窗，不复制参考图中未支持的Appearance选项和搜索占位。

读取上游platform/jewel/int-ui/int-ui-standalone/src/main/resources/themes/expUI/icons/dark/radioSelected.svg及radioSelectedDisabled.svg：16直径圆中6直径圆点，普通白色、禁用灰色。vendor/gpui-component/src/radio.rs以同一比例绘制圆点，替换错误复用的checkbox勾号，保留BaseRadio事件、焦点和可访问语义；颜色仍跟随本地主题。此视觉修正覆盖已有Radio，包括Update Project和Git设置，并非只给新语言控件单独换样式。安装版本仍未知，不宣称所有字体/颜色/焦点态已经像素级匹配。
Windows release通过（1分08秒，包含Radio组件重编译）。artifacts/settings-details：opened.png核对英文标题仓库名、分类与圆点；切中文并Apply的偏好文件断言通过；Git和Diff页截图核对面包屑、选择标记、按钮区；Cancel关闭后重开保持中文。1200×750的narrow.png确认三按钮、关闭入口和内容未被截断。HEAD／干净工作区断言通过；自有PID33948核验后正常关闭，启动会话退出0。未自动测试滚动；Radio禁用/焦点/键盘状态和新Diff单选保存路径尚未本轮专门原生复验，不能从语言单选成功推断全部状态完成。独立系统窗口、精确字体与焦点选中颜色仍有差异；搜索按用户要求暂不做。未更新便携包、未复测总体内存预算。

## 设置错误归属与Diff背景保留（2026-10-17）

原生artifacts/settings-lifecycle从shared.txt的Diff Tab打开设置，模拟语言偏好文件拒绝写入，发现同一错误同时出现在弹窗和后台全局banner，取消后仍残留。读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的SettingsEditor.java::apply（278–330）：逐configurable收集ConfigurationException，经编辑器context更新错误并选择失败分类，返回false阻止关闭。不是将设置错误写到Git审阅视图。

修正src/ui.rs：set_language/set_bottom_preview的保存错误写入SettingsDraft.error，并分别选择Language/Diff；apply开始只清理设置错误，不再清掉原本独立的仓库诊断；settings_panel只呈现自身错误。取消丢弃草稿及其错误，失败保存保留草稿和弹窗。Git偏好继续由原Settings实体管理自己的错误。

修改前已验证Diff Tab可从设置返回、Bottom单选可保存，但旧save-failed.png明确展示重复banner，不能作为修复后验收。初版自有PID37336已核验关闭，启动会话退出0。随后以新fixture进行修复后验证。

修复后Windows release通过（19.69秒）。artifacts/settings-errors原生复验：save-failed.png中错误仅在设置弹窗内，取消后cancel-after-error.png返回原Diff且没有残留全局错误，语言文件仍为en。Bottom选择经Apply保存，log-bottom.png确认底部预览显示；重新选择Tab并OK后tab-saved.png确认底部预览关闭，diff-location为tab。HEAD不变、工作区干净的断言全部通过。自有PID14764核验路径后正常关闭，启动会话退出0。未更新便携包；本轮不扩大为全部设置状态或像素级验收。

## 分支竖条中文标签（2026-10-17）

继续F1，修正收起分支面板后固定英文轮廓的缺口。重新读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的plugins/git4idea/backend/src/ui/branch/dashboard/ExpandStripeButton.kt（动态text supplier、small label font、逆时针90度绘制）、BranchesInGitLogUiFactoryProvider.kt（messagePointer及点击展开属性）和ExpandablePanelController.kt（展开/收起可见性）。安装版版本仍未知。

src/ui.rs::branches_rail按现有解析后语言选择英文或中文轮廓，不改变展开状态或Git查询。scripts/generate-branches-stripe.py增加可重现的中文生成参数及字体SHA-256检查；中文两字使用Noto Sans CJK SC Regular 2.004，字体元数据确认Adobe版权和SIL OFL，来源、哈希与许可证保存在assets/fonts，随现有打包规则分发。仅新增1977字节SVG，不打包完整中文字体或增加后台字体实例。保留差异：上游运行时旋转系统字体，本地是固定12逻辑像素轮廓；字体与用户截图未宣称完全一致。竖条点击范围保持原有尺寸；中文轮廓按两字长度24显示，英文仍60。Windows验证待本轮完成。

Windows最终release通过（19.22秒），测试副本与最终exe的SHA-256一致。artifacts/branches-language原生验证英文→中文→英文立即更新竖条，中文点击标签展开分支树、箭头收起均正常；chinese.png、expanded.png和english-returned.png已人工检查。语言文件往返、HEAD不变和工作区干净断言通过。自有PID34952核验路径后关闭，启动会话退出0。当前只验证此DPI与普通主题，未重测总体内存预算，也未更新便携包；F1其余文案与键盘/焦点验收仍需继续。

## Rebase 连续冲突与双后端补验（2026-10-17）

核对当前实现与旧记录后，确认原单次冲突测试不能证明连续冲突及apply后端行为。重新读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的plugins/git4idea/tests/git4idea/rebase/GitSingleRepoRebaseTest.kt::test multiple conflicts、test continue rebase after resolving all conflicts和test warning notification if conflicts were not resolved；以及backend/src/rebase/GitRebaser.java::continueRebase/handleRebaseContinueFailure、GitAbortRebaseProcess.java::confirmAbort。上游按每次重放结果再次进入冲突处理；单仓中止独立确认。本轮不改执行流程或UI，安装版版本未知。

tests/repository.rs新增existing_rebase_multiple_conflicts_continue_or_abort_both_backends，覆盖merge/apply×最终继续/中止四组临时仓库：两次修改同一文件均产生冲突；第一次解决并继续后命令失败但已经生成第一条提交，真实状态仍为对应Rebase后端且有第二次冲突；再次未解决Continue拒绝并保持HEAD/索引。第二次解决后继续完成，或明确Abort回到原始main与原文件内容。每步新建Repository，不依赖旧会话；最终断言父链、分支、操作状态、干净工作区，以及事先stash的OID、暂存和工作两份内容保持，未擅自恢复stash。

Linux定向测试通过（1.93秒）；Windows测试构建通过（2.81秒），宿主Git原生同一测试通过（18.63秒），记录artifacts/rebase-multiple-conflicts/windows-tests.txt。这是执行层补验，不是连续冲突UI端到端验收。未新增可见功能，不重复启动GUI或更新便携包。旧计划中的外部注释兼容、no-change恢复引导和错误详情仍未解决；本次检查src/ui.rs::start_rebase_operation确认它仍将底层错误丢弃，仅显示通用摘要，下一步需要补可排查的失败详情，不能据本测试宣称Rebase全流程完成。

## Rebase 失败详情与重试反馈（2026-10-17）

重新读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的plugins/git4idea/backend/src/rebase/GitRebaser.java::handleRebaseContinueFailure/collectErrorOutputLines、commands/GitHandlerUtil.java::isErrorLine与GitImplBase.java::ERROR_INDICATORS：优先收集stdout/stderr的错误前缀行，无匹配时返回stderr，再回退最后非空stdout行；通用失败交给VcsNotifier。GitAbortRebaseProcess.java中中止失败也保留错误输出。安装版版本未知。

src/rebase_operation.rs新增diagnostic，按上述Continue规则提取文本，同时保留命令启动错误和状态读取错误。传输仍为各64KiB有界尾部，显示最多8000字符加省略号；URL移除userinfo、query和fragment，过滤普通控制字符，不将原始输出写入日志。不是任意hook内容的通用秘密检测器。src/ui.rs保存最近一次Rebase诊断，在右下角提供中英文“变基详情”按钮，点击居中纯文本详情窗，关闭可再次打开；新Rebase尝试及切仓清理，刷新不丢失。中止命令失败优先显示操作失败，避免被尚存冲突覆盖为解决冲突提示。

保留差异：上游通知气泡/错误列表，本地复用底部状态入口和按需详情窗，符合用户信息放右下的要求；没有完整通知中心、复制按钮或上游所有通知操作。Abort目前也复用Continue的错误行选择，未复制其完整HTML错误输出；原始英文Git输出不翻译。本轮不补自动stage/skip、外部注释兼容及其他Git操作的诊断。

2项单元测试通过，覆盖上游优先级、stdout回退、原文、启动与状态双错误、URL隐藏、中文边界和长度；现有真实Git测试增加editor-failure有诊断、成功无诊断断言，Linux通过（1.24秒）。最终Windows release19.69秒。artifacts/rebase-details临时中文仓库：外部Rebase冲突后放置专用index.lock，原生点击中止，details.png显示具体锁文件及fatal原因，右下显示变基操作失败；确认HEAD仍在incoming、冲突和Rebase标记仍在。移除测试锁后再次明确中止，recovered.png显示变基已中止且详情入口清除，断言回到原main/HEAD、无操作标记、工作区干净。初版PID28560与最终PID36184均核验路径关闭，启动会话退出0。首次检查重启窗口时startup.json尚未更新，旧PID查找失败后等待实际新启动记录重试，没有重启活进程。只验证中文鼠标路径；英文/键盘/切仓原生补验仍待。未更新便携包、未复测内存预算，不宣称全面视觉还原。

## Rebase 空补丁恢复（2026-10-17，实施中）

读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的plugins/git4idea/backend/src/rebase/GitRebaseProblemDetector.java、GitRebaser.java::handleRebaseContinueFailure/stageEverything及GitUtil.java::hasLocalChanges：明确No changes - did you forget to use 'git add'?后，有cached diff则continue，有tracked工作区diff则git add --update后continue，两者都没有则skip。tests/git4idea/rebase/GitSingleRepoRebaseTest.kt中的unstaged changes while stopped for editing stage and retry实际测试独立StagingAreaHelper，不把它冒充本路径完整测试；本地另外构造真实apply后端空补丁。

src/operation_events.rs流式检测新增NoChanges，跨pipe分块并在输出尾部截断后仍保留。src/rebase_operation.rs增加持锁调用的continue_with_recovery，供独立操作及update_session::continue_rebase_with_requests共用，避免二次获取操作锁。仅Git失败、非取消、明确NoChanges且实际索引无冲突时恢复；读取HEAD、REBASE_HEAD和staged/unstaged标记组成一个前次状态，重复则返回真实失败，避免上游递归重试在异常输出下不终止。只保留一个状态，不累计提交列表；stage使用--update，不添加untracked。读取失败、stage失败和取消停止后续恢复。

兼容差异有真实测试依据：Git 2.55.0在apply空补丁失败时额外输出Resolve all conflicts manually通用hint。上游按该文字优先判冲突会挡住NoChanges恢复；本地在明确NoChanges时以实际unmerged索引判断冲突。最初测试暴露此交叉，已修正并重新通过。另一个测试发现现代Git在未暂存编辑存在时先报You must edit all merge conflicts，不产生NoChanges；该情况按真实失败保留修改，不自行扩大stage触发条件。无中断状态/无明确marker均不自动skip。安装版Rebased版本未知；未声称它当前运行版也存在同样问题。

新增existing_rebase_apply_no_changes_recovers_without_adding_untracked_files覆盖空补丁跳过、暂存内容正常提交、非NoChanges未暂存错误保持index/工作区、untracked从不加入。Linux最终3项existing_rebase测试通过（1.98秒）；流式事件测试通过（0.03秒）。更广的rebase定向运行中其余14项仓库测试、5项库测试及1项编辑器测试通过，唯一空补丁失败在上述hint兼容修正后定向复验通过。Windows三项真实Git测试通过（32.02秒），证据artifacts/rebase-no-changes/windows-tests.txt。legacy NoChanges同时伴随tracked变化的自动stage分支已按源码实现，但当前Git前置拒绝导致本次真实fixture未触发，不宣称已运行覆盖；重复状态保护也尚未专门故障注入。原生继续按钮路径待本轮构建后核验。
Windows release通过（35.56秒）。artifacts/rebase-no-changes中文原生fixture在apply冲突后将文件解决为上游相同内容，并另留untracked.txt；点击现有继续变基后自动完成空补丁恢复。completed.png确认状态变基已完成、顶部恢复main且操作按钮消失，graph与详情均更新；verify.ps1断言HEAD等于incoming、main附着、rebase-apply已移除、唯一状态为untracked且内容完全保留。自有PID17644核验路径正常关闭，启动会话退出0。没有新UI控件，本轮验证了新增恢复行为的实际入口；其他DPI、legacy自动stage及失败重试保护专门注入仍待，未更新便携包或重测总体内存预算。

## Rebase 恢复异常分支补验（2026-10-17）

继续上一节明确未覆盖项，本轮不改产品代码。重新读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的GitRebaser.java::handleRebaseContinueFailure/stageEverything，核对cached重试、tracked自动暂存及add失败结束规则。tests/repository.rs新增rebase_no_change_recovery_stage_failure_and_repeated_error_stop：真实merge后端Rebase冲突解决后，经Git实际编辑器脚本注入NoChanges错误，覆盖当前Git不会自然产生的legacy恢复状态。该方式只控制编辑器失败与临时仓库内容，不替换Git命令执行器，也不声称错误由现代Git自然生成。

四组场景：编辑器首次将解决内容从index退回工作区，恢复器add --update后再次继续成功；同样退回后创建index.lock，add失败即停止，编辑器只调用一次；保留cached内容、首次报错后第二次编辑器成功；始终报同一错误时编辑器只调用两次，Rebase保持中间HEAD且已暂存解决内容不丢。重复脚本设第五次退出成功兜底，使重试保护回归时测试失败而不是永久挂起。每组断言未跟踪文件内容与未入index，成功父链/提交内容，失败HEAD、工作区/index、操作状态；测试锁仅在专用fixture移除。

Linux最终专项通过，Windows测试构建1.66秒，宿主Git专项通过14.95秒，记录artifacts/rebase-recovery-faults/windows-tests.txt。覆盖了上一节legacy自动stage、stage失败和重复状态保护的执行分支；不扩大为这些故障的原生GUI逐项验收，也不替代自然NoChanges的上一节真实Git/原生证据。无新可见功能，未重新构建发布exe、更新便携包或启动测试GUI。

## 编辑弹窗重复焦点边框修复（2026-10-17）

用户报告提交信息弹窗输入框左右出现多线。原生2182×1361截图可重复检出左边缘蓝线段631–634与636，中间分离；artifacts/textarea-border/check.py在修复前失败。定位到vendor/gpui-component的Input同时绘制focused border与styled.rs::focus_ring外扩环，非截图缩放问题。

源码参考4aec35256d765c217aa0d662b82ac4e860c396fd：platform/platform-impl/src/com/intellij/ide/ui/laf/darcula/ui/DarculaEditorTextFieldBorder.java::paintBorder与同目录上级darcula/DarculaUIUtil.java::paintOutlineBorder/doPaint，聚焦轮廓与边缘采用连续几何；安装版Rebased版本仍未知。本地Input/Textarea增加external_focus_ring选项，默认保持原行为；src/editor_dialog.rs仅关闭编辑器额外外环，保留focused蓝边、非聚焦边及布局，不改其他控件。差异：采用轻量单线轮廓，未声称复刻上游精确线宽及所有DPI。

Windows release构建通过（1分13秒）。同一临时Rebase仓库重开提交消息编辑器，artifacts/rebase-dialog-centered/border-fixed.png及原始border-focused.png构成修复前后证据；同一像素检查左右三个高度均通过，现在左636、右1545各单组连续蓝线。视觉确认文字、圆角、按钮仍在；尝试Tab失焦验证因目标非前台被脚本拒绝，未向其他程序发送按键，失焦态/其他DPI待单独验收。只关闭自有验证进程，不触及用户Rebased/IDEA。

同时保留此前进行中的弹窗居中修正：src/editor_dialog.rs与src/ui.rs::confirm_abort_rebase改用centered(true)，不再估算高度设置margin_top。依据GitUnstructuredEditor.java、GitImplBase.java的编辑器入口及DialogWrapperPeerImpl.java的父窗口居中规则；稳定截图确认编辑器居中，初次动画截图不作为位置验收。错误文案增高、窄窗口及Abort弹窗的居中回归仍待验证。

## Rebase 弹窗居中与取消路径原生补验（2026-10-17）

上一轮为有效进展：重复边框已修复且原生像素检查通过。本轮沿用该release，不新增产品代码；使用已有临时仓库及自有PID21480继续核验。编辑器打开后，mutate-message.ps1记录HEAD/index tree并在COMMIT_EDITMSG追加外部修改，点击OK得到保存失败提示。save-failure-stable.png确认弹窗随错误文本增高后仍居中，输入保留；resize.ps1缩至1200×750，save-failure-narrow.png确认输入、错误和两个按钮完整可见。未进行滚动自动化。

点击编辑器Cancel后verify-state.ps1通过：HEAD、index tree、resolved工作区内容及外部消息修改均保留，rebase-merge仍在。打开Abort Rebase，abort-stable.png确认1200×750下居中，路径省略显示，Abort/Cancel可见；点击Cancel再次通过相同仓库状态断言。再次打开并确认Abort后，verify-state.ps1 -Aborted通过：HEAD恢复fixture原始提交，main附着、rebase-merge移除、工作区干净；abort-complete.png确认原生页面返回main并显示Rebase aborted，诊断入口清除。证据与可重放操作脚本均位于artifacts/rebase-dialog-centered；本次fixture已完成abort，重跑编辑器测试需重新准备Rebase状态。

这完成上一节的错误增高、1200×750布局、编辑器取消、Abort取消和确认路径验收；其他DPI、失焦边框及键盘切换仍未验收。没有更新便携包或重新测量总体内存预算。只关闭本次自有验证进程，保留用户程序。

## 当前操作说明与便携预览包同步（2026-10-17）

上一轮完成原生状态断言与截图，为有效进展。本轮核对README、docs/WINDOWS-PREVIEW.txt及src/ui.rs实际菜单/键绑定、src/i18n.rs系统语言解析、src/fetch.rs参数，发现交付文档仍将Push、checkout、Rebase继续/终止描述为不可用。更新当前工作流：主菜单Git与顶栏Update/Push入口，Log过滤与顶部分支检出的区别，全文Diff Tab、设置语言和应用/取消、外部解决暂存后继续Rebase、Abort确认与取消不保证回滚。保留历史验证数据，将旧Pull能力描述明确标注为早期快照；未扩大为完整提交/暂存/冲突编辑能力。不改产品业务行为，无需新增上游行为结论。

生成dist/GitReview-windows-x64-preview-20261017-rebase-dialogs.zip（17,231,986字节），包含上一轮验证过的release、AskPass/editor两个助手、当前中文说明和许可/manifest；exe SHA-256为8e092a0a7386b8bc1d5d03cbee74f3b09638c421d0bd0fcc3521c70308d007a1。Windows Expand-Archive解压到独立验证目录，逐个校验三个exe哈希及README.txt与源文档相同，全部通过；启动解压后的exe，用未写语言偏好的独立profile加载三提交fixture，原生截图确认系统默认中文、Log/graph/文件/详情显示正常。证据artifacts/package-rebase-dialogs/{verification.txt,startup.json,window.png,verify.ps1}。仅关闭自有PID14596，未改用户应用。

本轮为文档与实际预览包交付补齐；不是完整功能回归或发布验收。启动采样不用于整体内存预算结论，现有DPI/键盘及更广场景验收缺口仍保留。

## 核心全量回归与编辑器测试执行竞态（2026-10-17）

上一轮文档/包同步与Windows解压启动为有效进展。本轮执行devenv shell -- cargo test --no-default-features，发现tests/editor.rs两个直接spawn助手的用例偶发Linux ETXTBSY（Text file busy），发生在业务断言前。完整套件两次触发；单失败用例100轮、缩小为两个直接启动用例30轮均未触发；完整六用例并发重复可复现。证据artifacts/editor-exec-regression/full-suite.txt与parallel.txt。strace显示各Fixture的PID+序号路径独立、copy写入使用O_CLOEXEC；加跟踪时未失败，未确定失败瞬间的具体句柄持有进程，不能声称已完全证明内核时序。

tests/editor.rs::Fixture::helper仅在Unix改用指向已构建助手的symlink，保留中文目录和带空格的启动路径，避免测试并发spawn过程中临时写入可执行inode；Windows保持原先fs::copy，不改产品进程执行、错误重试或编辑器行为。推断与并发子进程/临时可执行文件写句柄生命周期有关，采用消除临时写入的测试准备方式，而非给产品添加吞错重试。现有六用例构成真实调用链回归，修正后连续20轮全部通过（120次测试执行），专项初次构建运行也通过。未改产品UI，不新增原生界面验收或重发预览包。

随后完整no-default-features套件通过：122库测试、4 AskPass、6编辑器、8 HTTP认证、4操作进程、4进程树、93真实仓库测试，共241项报告通过（含测试进程fixture入口）；仓库组13.05秒。完整日志full-suite-fixed.txt。该模式不包含desktop组件测试，也不是Windows全量复验；现有完整验收缺口保留。无遗留产品调试日志；跟踪文件仅留在明确诊断artifact目录，repeat.py保留重复并发用例入口。

## Windows 核心全量回归（2026-10-17）

上一轮Linux回归与测试准备修正为有效进展，本轮补齐对应Windows执行证据。cargo xwin test --no-default-features --target x86_64-pc-windows-msvc --no-run --message-format=json构建通过（2.98秒）；从本次Cargo artifact提取测试程序清单，不靠猜测旧hash。将测试程序及两个debug助手复制到独立Windows验证目录，用宿主Git运行、每组--test-threads=4，HTTP仅回环测试服务，写操作使用测试自建临时仓库和本地远端。

全部241项报告通过：93仓库（275.68秒，含启动与日志写入）、122库、4 AskPass、6编辑器（16.11秒）、8 HTTP认证（7.91秒）、4操作进程、4进程树。Windows编辑器沿用fs::copy分支，未采用Unix symlink修改；中文/空格助手路径与编辑取消/错误保存用例均通过。进程组测试验证后代管道关闭、leader先退出后清理及不误杀无关进程。check-cleanup.ps1确认专用验证目录的程序无残留，不据此扩大为GUI关闭生命周期的全部验收。

证据artifacts/windows-core-regression：build.jsonl、tests.json、run.ps1、各组输出、results.json、summary.txt、cleanup.txt。运行结果均exit=0；没有产品代码变更或额外UI截图，未重打包。本次与Linux同样关闭desktop feature，untracked_preview因此为0项；不将241项通过解释为桌面组件、所有DPI、真实外部认证环境、长期内存或完整发布验收完成。后续仅在相关改动/新风险出现时重跑受影响测试，避免无变化反复全量。

## Desktop feature 的 Diff 模型测试补验（2026-10-17）

上一轮Windows核心241项通过为有效进展，但no-default-features明确排除了tests/untracked_preview.rs。本轮启用默认desktop feature，cargo xwin test --target x86_64-pc-windows-msvc --test untracked_preview --no-run --message-format=json构建通过（首次桌面测试依赖1分57秒），随后将本次生成的测试exe复制到独立Windows目录运行，2项通过，进程退出0。

覆盖：未跟踪Unicode/CRLF/末行无换行的Added状态及三行新增计数，空文件仍为Added/has_changes；只有100644→100755权限变化且增删计数为0时仍has_changes并保留扩展header。证据artifacts/windows-desktop-regression/build.jsonl、run.ps1、tests.txt。这两项执行真实gpui-component DiffFile构造/解析模型，不建立窗口，不代表GPUI渲染、文本选择、Tab生命周期或键盘验收；没有修改产品代码或重打包，也不重复前一轮核心测试。

## 当前预览包空窗与万条历史内存基线（2026-10-17）

前轮补齐desktop Diff模型测试为有效进展。本轮因设置弹窗/Rebase变动后尚无对应基线，使用当前预览包相同release SHA256 8e092a0a7386b8bc1d5d03cbee74f3b09638c421d0bd0fcc3521c70308d007a1，独立profile执行windows-job-memory.ps1；沿用挂起启动即加入Job统计子进程的方法，10次稳定采样，GPU单列，不做滚动自动化。

空窗Private Bytes十次均60.48MiB，Working Set66.22MiB，Job峰值63.07MiB。万条历史+975000字节review.rs完整Diff Tab，Private Bytes十次均99.26MiB，Working Set101.89MiB，Job峰值155.23MiB（162770944字节），累计73进程，采样末态1进程。通过空窗80MiB、Diff稳定150MiB、Job峰值250MiB预算。history.png确认已加载10000、offset0；window.png确认目标review.rs完整文件独立Tab。Git cat-file复核blob975000字节，基准HEAD/工作区前后不变。

GPU：空窗Dedicated18210816、Shared1146880、TotalCommitted29044736字节；Diff为58118144、3756032、62664704字节。均为同次单点计数，与进程峰值不相加；otherReviewProcesses为空。没有执行网络/写操作，未测其峰值；不代表冷缓存时延、长期交互增长或所有图拓扑规模验收，也不把相对上次单次样本变化归因为性能回退/改善。证据artifacts/memory-rebase-dialogs/{welcome,run}，check.ps1确认自有PID23080/9356已退出。没有改产品代码或更新包。

## 设置弹窗重复开关生命周期采样（2026-10-17）

上一轮当前包内存基线为有效进展，本轮补齐此前未覆盖的设置弹窗重复开关。scripts/windows-job-memory.ps1新增SettingsCycles（0默认、不改变现有路径），限制与其他交互模式互斥，在固定2182×1361/150%DPI基准上保存每次打开/关闭截图及内存样本。当前release、独立profile、10000提交和975000字节全文Diff背景，20轮仅打开设置并点关闭，不修改偏好或Git状态；沿用Job统计和稳定采样。无产品代码修改，无滚动自动化。

首轮人工截图确认设置打开、关闭返回相同Diff。artifacts/settings-cycle-memory/check-frames.py逐一检查20对原生截图的尺寸与设置内容区/底层绿色Added Diff区域，全部通过；这些区域断言只证明对应面板开关，不代替全部控件和文字视觉验收。每次关闭后Job activeProcesses=1，整个运行总进程数73与基础读取一致。基线95.70MiB，各轮关闭后94.37–96.00MiB，最后一轮95.58MiB，随后稳定94.15MiB（Working Set93.21MiB）。Job内核峰值149.48MiB（156737536字节），稳定与峰值预算均通过；此短循环无持续单调增长，不宣称长期无泄漏。

GPU单次Dedicated58122240、Shared4079616、TotalCommitted63123456字节，单独报告不并入Job峰值。benchmark HEAD与工作区不变；验证进程10328正常结束，check-cleanup.ps1确认专用目录无程序残留。每轮截图、settings-cycles.json、memory.json、steady.json和脚本位于artifacts/settings-cycle-memory。Git设置页异步读取、应用偏好、错误弹窗生命周期及写操作峰值不在此次覆盖内。


## F1/F2 验收状态核对与剩余本地化缺口（2026-10-17）

上一轮设置生命周期采样为有效进展。本轮按当前代码和落盘证据核对首页清单，避免已完成子项永远留空导致重复实施。仅勾选两项，不把局部测试扩大到整个F1/F2：

- F1语言选项/默认系统语言/持久化：src/i18n.rs的Language、GetUserDefaultUILanguage与主语言位判断；src/main.rs启动load_language+apply；src/layout_preferences.rs有界读取及save_language；src/ui.rs三单选项及Apply/OK。单元测试覆盖各中文LANGID（含繁体区域）、英文/日文/0、显式覆盖与编码往返，最近Linux/Windows全量均通过。artifacts/settings-details/assertions.json和check.ps1确认真实中文选择写盘；artifacts/branches-language保留中英往返文件断言和截图；artifacts/package-rebase-dialogs/window.png证明独立无偏好profile按宿主中文启动。所有中文区域使用简体资源，其他/0回退英文，README及便携说明已明确。
- F2浏览为主+基础操作的说明更新：README Current workflow、docs/WINDOWS-PREVIEW.txt已覆盖Fetch/Pull/Update/Push/checkout及外部冲突处理边界，旧Pull记录标为历史。src/ui.rs不再显示全局Read-only产品承诺；artifacts/package-rebase-dialogs/verification.txt与verify.ps1证明当前zip中的说明与源文件相同、三个程序哈希一致，并已实际启动。

仍不勾选：Settings图标项的全部键盘可达性；集中本地化覆盖全部可见状态；语言切换保留所有筛选/Tab/错误状态的完整矩阵；F2执行生命周期、认证与取消后全部状态/峰值范围。当前找到的明确代码缺口为src/ui.rs底部Diff工具栏expand按钮直接输出Restore/Expand，以及子模块提示直接输出英文句子；这些属于产品文案，不是仓库数据，下一步应对照对应上游入口后接入集中翻译并原生确认。未用本次核对宣称整体目标完成。

## 底部 Diff 展开动作与子模块说明本地化（2026-10-17）

完成上一轮明确定位的两处遗漏。读取参考4aec35256d765c217aa0d662b82ac4e860c396fd：platform/platform-impl/src/com/intellij/openapi/wm/impl/MaximizeToolWindowAction.java按maximized状态切换ActionsBundle消息；plugins/git4idea/backend/src/diff/GitSubmoduleDiffRequestProvider.kt构造before/after文本并使用GitBundle子模块标题，GitSubmoduleContentRevision.kt读取已提交gitlink哈希或当前子仓HEAD。本地src/ui.rs底部预览展开/还原按钮和子模块说明改调用t，src/i18n.rs集中增加三条翻译，不翻译路径/哈希/Git补丁正文。差异保留：本地动作仅展开底部预览，不等同上游整个ToolWindow最大化；子模块说明是本地显式补充，不声称上游有同句提示。

Windows release通过（1分12秒）。artifacts/preview-language创建专用两提交仓库，第二提交新增160000 gitlink，设置底部预览与中文。window.png确认展开及中文子模块说明；expanded.png确认点击后变为还原且内容保留；通过设置改English后english-restored.png确认还原为底部布局、按钮Expand与英文说明恢复，路径和哈希保持原样。自有PID3264核验路径后关闭。仅文案改动，无新增镜像实现测试或重复核心全量，无便携包更新。

此次原生核对进一步发现Compared with比较标题和组件内Added file仍为英文，保留为下一批本地化缺口，F1全界面本地化不勾选。组件状态文案应通过组件现有本地化机制接入，不直接修改或翻译Git原始内容。其他DPI与像素级外观未由此补验。

## Diff 比较标题与组件语言同步（2026-10-17）

上一轮两处文案修复及原生证据为有效进展。本轮处理截图中的Compared with与Added file遗漏。参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/vcs-impl/src/com/intellij/openapi/vcs/changes/actions/diff/ChangeDiffRequestProducer.java::getRevisionTitleOrEmpty/getBaseVersion/getYourVersion：用户界面版本名走DiffBundle，具体revision保留短哈希；GitSubmoduleDiffRequestProvider.kt沿用版本标题。保留本地差异：底部预览继续显示本地比较摘要和完整base ID，不声称其排版等同上游。

src/ui.rs原base英文String改为Option<i18n::ComparisonTitle>，保存静态语义或原始base+merge标志，绘制时按当前语言生成；src/i18n.rs集中覆盖普通/第一父/浅边界及本地各area比较标题。这样切换语言不依赖重新选择或重新查询Git。查明vendor/gpui-component/locales/ui.yml已有Diff.AddedFile/DeletedFile/RenamedFile等中文、diff/mod.rs使用其t!，问题是应用未同步组件locale。src/main.rs初始化及src/ui.rs::set_language同步zh-CN/en到组件set_locale，沿用其现有消息资源，无重复硬编码状态翻译。

现有i18n三项测试通过，Windows release通过（32.02秒）。artifacts/preview-comparison-language/window.png确认中文比较标题及新增文件；english.png确认设置切English后，同一选中提交/文件/底部Diff立即切换英文，哈希和Subproject commit原始内容不变。专用gitlink fixture，未执行用户仓库写入；自有PID6836正常关闭。该原生验证覆盖普通新增gitlink和即时切换，不冒充删除/复制/merge/浅边界全部状态验收。没有更新预览包；全界面本地化仍未整体勾选。

## 底部 Diff 空状态与偏好错误摘要本地化（2026-10-17）

继续上一轮未完成的F1文案覆盖。读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/diff-api/src/com/intellij/diff/requests/NoDiffRequest.java（IdeCoreBundle.message.nothingToShow）及platform/diff-impl/src/com/intellij/diff/impl/DiffRequestPanelImpl.java（空request归一NoDiffRequest后更新视图），并核对DiffBundle资源。提取规则是空状态文案属于本地化UI，不是Git内容；本地保留比上游通用无内容消息更详细的原因说明，未改变空状态分支和查询。

src/ui.rs渲染empty_diff_message时调用t，src/i18n.rs覆盖八种已有提示：消息筛选无结果、干净工作区、浅边界、第一父无变化、普通空提交、无历史、无patch、未选择文件。同时Diff外观/布局偏好保存失败摘要接入t，底层error保持原文。不是翻译原始Git输出，也未修改失败处理逻辑。

Windows release通过（29.44秒）。专用两空提交fixture、底部预览中文启动，artifacts/preview-empty-language/window.png确认空提交中文说明；local-clean.png确认本地工作区干净中文提示及此前的本地比较标题；english.png确认当前Local Changes不切换仓库就即时恢复英文提示。自有PID37692关闭。八分支中仅两代表状态本次原生核验，其余文案映射和错误摘要已接入但未逐一注入/截图，不把此次扩大为全空态验收或全部本地化完成。未重复核心Git测试或重打包。

## Log 定位动态提示本地化与原生验证（2026-10-17）

前轮边框复核仅再次确认已有修复，本轮收尾实际未验收的定位提示变更。参考4aec35256d765c217aa0d662b82ac4e860c396fd，platform/vcs-log/impl/src/com/intellij/vcs/log/ui/AbstractVcsLogUi.java的tryJumpTo、handleCommitNotFound、getCommitNotFoundMessage及VcsLogBundle.properties：区分目标存在但不匹配当前视图与仓库中未找到，引用名作为消息参数保留。src/i18n.rs新增Locating/HiddenInLog/NotInHistory/MoreReferences类型消息，src/ui.rs在绘制时生成当前语言文本，原始引用名和查询状态不变。保留差异：本地使用内联提示并提供清除筛选动作，上游这里使用VcsNotifier warning；本轮不声称通知结构已还原。安装版Rebased版本未由此次确认。

Windows release构建通过（29.70秒）。artifacts/navigation-language使用独立三提交分叉仓库：main上下文菜单设置分支筛选，side-tag上下文定位触发隐藏提示。hidden-zh.png确认中文；english.png确认设置切换英文后同一提示即时更新，main筛选与选中引用保留；located.png捕获英文定位中状态，located-stable.png确认清除筛选后恢复三条历史，选中Side review commit，右侧显示side及side-tag。核验路径后关闭自有PID12508，未触及用户Rebased。当前验证覆盖隐藏提示、语言切换与恢复定位；未找到目标、更多引用数量及中文加载瞬态未逐一原生验收。未重复核心Git测试、未更新便携包，F1全界面本地化仍不整体勾选。

## 设置键盘入口与宿主热键冲突定位（2026-10-17）

上一轮Log动态消息原生验收为有效进展。本轮按F1键盘可达性补齐实际缺失的设置快捷动作。读取参考4aec35256d765c217aa0d662b82ac4e860c396fd的platform/platform-impl/src/com/intellij/ide/actions/ShowSettingsAction.java（齿轮及本地化名称，欢迎页使用默认project）与platform/platform-resources/src/keymaps/$default.xml（ShowSettings绑定control alt S）。src/ui.rs添加OpenSettings动作及Review上下文Ctrl+Alt+S，欢迎页与仓库页均调用既有open_settings，复用草稿、取消、会话和释放机制；两个齿轮tooltip补充快捷键，accessibility label保持本地化名称。没有新增搜索或设置项，也未复制新的设置实体。

Windows干净release构建通过（39.21秒，诊断日志已移除）。首次实体组合键和SendKeys均未打开，但Ctrl+F正常、鼠标正常。artifacts/settings-shortcut/check-open.py对失败截图报缺少设置语言选中区；按diagnosing-bugs流程加入仅S键修饰符及动作入口的临时日志。宿主RegisterHotKey探针注册Ctrl+Alt+S失败，Win32错误1409（已注册）；未解除/修改宿主热键。临时日志和截图证明定向WM_KEYDOWN配合真实Ctrl/Alt修饰状态可到达OpenSettings。保留差异/限制：宿主全局热键仍拦截实体组合键，不添加非JetBrains替代键、不宣称此环境实体快捷键已通过。

最终无诊断日志构建在原生仓库窗口通过定向键打开、Esc关闭和再次打开；final-open-stable.png、final-escape-stable.png、final-reopen-stable.png及像素断言通过。欢迎页同一动作打开设置，welcome-open-stable.png人工确认标题无仓库名及原有语言/版本控制布局。定向键验收覆盖GPUI键位匹配、动作分发和弹窗生命周期，不代替未占用环境的真实组合键验证，也不覆盖全部Tab焦点链或辅助技术。脚本发送全局修饰键前检查进程路径、窗口命中及前台HWND，finally释放按键；仅使用独立fixture。自有PID12800/31340/8128/20812均在重启或结束时核验路径后关闭。未更新便携包、未重跑Git全量；F1设置入口完整验收仍保持未勾选。

## Fetch 写操作进程组内存基线（2026-10-17）

上一轮设置快捷动作是有效进展。本轮补齐F4写操作内存的首个实际样本，不修改Fetch产品逻辑。scripts/windows-job-memory.ps1新增可选ScenarioScript（默认空，与现有交互模式互斥），在原有挂起启动/加入Job/恢复执行之后、稳定采样之前调用场景脚本，记录场景路径和最终截图。Job从应用创建开始包含所有继承子进程，GPU另报。场景自身用于观察refs的Git轮询由测试PowerShell启动，不是产品子进程，不纳入Job；没有将外部造数/clone/push计入应用峰值。

artifacts/fetch-job-memory/run.ps1从10000提交基准创建独立bare remote及client/source两个clone，源clone再生成5个新提交、5个各1MiB随机二进制blob并推送本地远端；client启动前保持原HEAD及旧origin/main。运行当前干净release（SHA256 217fcc13658dc6c72beb91c6b10140a7f7f445f703a9d349ced2009de1765410），historyLimit10000，不打开Diff。scenario.ps1打开主菜单后等待真实远端跟踪ref变化；按main-menu.png/git-menu.png确认菜单，通过原生定向点击Git→获取执行（同次点击命令整理为trigger.ps1，须在readiness后另行调用）。不是后台直接执行git fetch代替UI验收。

预期和实际origin/main均85219a8aec459edfbdd3669b6a9c66d7739bddba；后续断言HEAD不变、工作区/索引保持干净。scenario-final.png确认获取完成、main保持旧提交，刷新为offset5以保留原选中提交；未滚动。Job进程总数从70增到158，等待连续2秒activeProcesses=1后采样10次：Private Bytes84.05→81.27MiB，后7次81.27；Working Set末态78.41MiB，所有采样childPrivateMiB为0。Job内核峰值227975168字节（217.41MiB），通过250MiB进程组峰值预算；稳定最大84.05MiB通过150MiB预算。预算断言由场景run.ps1执行并落盘assertions.json，通用memory.json的assertBudget=false不代表未检查。

GPU单点Dedicated58089472、Shared2248704、TotalCommitted61128704字节，独立报告不与Job峰值相加；otherReviewProcesses为空。cleanup-check.ps1确认自有PID34844退出。此结果仅覆盖本地传输成功、10000历史+5MiB新对象规模；未覆盖HTTP/SSH认证、取消、失败、多远端、长时重复或Pull/Push峰值，不据此勾选F4整体内存验收。未更新便携包，无像素还原结论。

## Push 写操作进程组内存基线（2026-10-17）

在用户新增主线前已执行完成，本轮补录收尾。artifacts/push-job-memory从10000提交基准建立临时bare remote与client，在client新增5提交和5个各1MiB随机blob；通过当前release（SHA256同Fetch基线217fcc…65410）原生顶部Push打开弹窗。push-ready.png确认main→origin:main与五条待推送提交，push-details.png确认选中提交5及对应文件；原生点击推送后，远端main更新为6238578c5f411ff2d02c35f22e4687c0af53e714，client upstream同步。HEAD不变、index/工作区干净，scenario-final.png确认推送完成和refs刷新。

沿用Job挂起启动、ScenarioScript及10次稳定采样，historyLimit10000（实际10005提交），未打开Diff。Job总进程70→246，结束active=1；Private Bytes82.34→81.74MiB（后9次81.74），Working Set末态79.64MiB。Job峰值258682880字节=246.70MiB，低于250MiB预算但余量较小；此单次样本不证明更大传输、不同并发或网络后端也能通过。GPU另报Dedicated58109952、Shared2543616、TotalCommitted61444096字节；otherReviewProcesses为空。assertions.json通过，cleanup-check.ps1确认自有PID37868退出。仅本地传输成功样本，不覆盖认证、拒绝、取消、多目标或重复增长；未更新包。

## 新主线范围落盘（2026-10-17）

保存用户两张原图，更新本计划顶部的侧栏／只读文件浏览／Commit／Amend／多选Squash及右键动作验收，并修正旧版不包含Commit的范围冲突。AGENTS.md仅保留当前范围指针和新写操作源码约束；源码读取情况详见docs/REBASED-REFERENCE.md新增段落，区分已读规则和候选路径。本轮是范围、执行顺序和源码依据落盘，未声称新工具窗口、提交或Squash已经实现。下一实施项为左侧工具窗口壳与只读Project浏览，既有性能预算不降低。

## 只读 Project 文件浏览的数据层（2026-10-17）

上一轮新范围与源码依据落盘为有效进展；本轮开始实现新的主线。对照ProjectViewImpl的单击预览选项、树展开恢复，以及ProjectViewDirectoryHelper的目录枚举/过滤/节点构造，读取范围与差异见docs/REBASED-REFERENCE.md。现有src/path_tree.rs是Paths过滤选择器，会排除未跟踪文件，因此不直接拿它充当Project树。

新增src/project_files.rs与lib导出：以canonical工作区根和相对PathBuf为身份，目录展开只列直接children（包括未跟踪文件），自然排序；.git管理路径不展示/读取，拒绝绝对路径和上级跳转。链接/reparse可见但不跟随；普通文件按需读取，5MiB正文上限及4096项/512KiB目录路径预算，预取消和读取循环取消检查。UTF-8保留CRLF/无末行换行，BOM移除；二进制、不支持编码、超限、IO、常见外部变化返回类型状态，供未来UI本地化。无Git子进程、写操作、后台索引或常驻全仓库扫描。

Linux6项真实文件测试通过（0.24秒）。Windows xwin测试构建通过；首次运行被PowerShell的UNC当前目录警告/NativeCommandError打断（不是产品读取断言失败），将测试cmd工作目录固定为fixture并用独立stdout/stderr捕获后重跑：6项全过（2.37秒），包括真实Windows junction拒绝、Unicode文件名/内容、BOM/行尾、文件外部替换与删除后重读、二进制/编码/超限、取消、Git管理文件及大目录明确失败。证据artifacts/project-files-core/windows-tests.txt与run.ps1；无GUI修改，本轮没有原生窗口截图或新内存结论。

未完成范围明确保留：目录预算目前超限报错，尚无大目录分页；ignored/隐藏项与Project设置映射、链接内部安全展开、其他编码、读取瞬间变更的原子身份保证、树UI及只读文件Tab、全局缓存/请求生命周期仍待。当前模块不宣称原子文件快照或对敌对并发文件系统的隔离边界。下一步把读取层接入左侧Project实体和按需Tab，同时迁移原本地变更/已保存的更改；本节不勾选完整Project功能。

## Project 工具窗口与只读 Tab 接线（2026-10-17）

src/project_view.rs 新增 ProjectView/FileTab，src/ui.rs 接入左侧 Project 工具按钮和中央文件 Tab。目录逐级展开，包含未跟踪文件；普通文件双击/Enter 打开，显式重新加载，关闭按钮回到原视图。正文使用 readonly EditorState 和 readonly Editor 双层约束，无保存操作；只保留活动 Tab 正文，退出取消请求并以 generation 丢弃过期结果。目录最多 16384 节点，单目录和文件仍遵循读取层预算。状态用类型化静态标签/IO 数据，在绘制时按当前语言翻译。

Windows release 首次构建通过（30.41 秒）；原生独立 fixture PID7604，tree.png 确认 Project 与 Log 并排、目录和未跟踪文件可见；file.png 确认 notes.txt 独立只读 Tab 和完整正文。readonly.png 中 Ctrl+A 选中正文后输入未替换内容，但宿主输入法出现组合候选，此截图只验证该输入路径，不扩大为所有编辑快捷键验收。关闭 Tab 后截图 closed.png。发现树选中背景未铺满行，已添加 w_full 并再次构建通过（24.14 秒）。证据目录 artifacts/project-browser-ui。

剩余：本地/已保存更改迁移、上下分区、尺寸拖动、双击和完整键盘边界验收、编码扩展与预览状态恢复；Commit/Amend/Squash 尚未实现，完整 Project 和视觉验收不勾选。此次启动采样不是内存预算验收，未更新便携包。

最终构建自有 PID31024：tree-final.png / expanded.png 确认修复后整行选中背景及 src 目录展开显示 sample.rs。verify.ps1 断言 HEAD、受跟踪文件和 notes.txt 内容均未改变，assertions.json 已落盘；两个自有测试进程均核验路径后关闭，未操作用户 Rebased。未自动化滚动。

## 左侧工具窗口改为独占模式（2026-10-17）

上一轮实际完成 Saved Changes 侧栏迁移、保留中央 Diff，以及宽度/上下分隔拖动的 Windows 验证（artifacts/tool-window-sidebar/both.png、diff.png、wider.png、split.png）；随后用户明确取消上下分区。顶部验收已改为独占侧栏，历史双窗口截图仅作当时事实，不再是目标或必需验收。

src/ui.rs 删除上下分区状态；Project/已保存的更改互斥显示，重复点击当前入口收起，保留各实体的选择和展开状态，中央文件/Log/Diff 不因侧栏切换关闭。Saved Changes 不再占中央主 Tab。侧栏宽度仍可调整，并将横向分栏容器约束在图标栏剩余宽度内。后续本地更改/Commit 仍需从共用 Log 状态拆出，再迁到此独占槽位；本轮不宣称 Commit 已迁移。

Windows release 23.47 秒通过。自有 PID28240，exclusive-stash/project/stash-restored.png 验证独占切换且 stash 及文件选择保留。用户同时指出旧双窗口截图右侧 Diff 齿轮被裁切：根因是横向 ResizablePanelGroup 的 size_full 未约束到图标栏以外的剩余宽度。src/ui.rs 为其增加 flex_1/min_w_0/h_full 外层，中央区域随侧栏变化收缩。

按 diagnosing-bugs 建立 artifacts/tool-window-sidebar/check-toolbar.py：在同一 2182×1361 原生截图内匹配完整 22×22 齿轮（包含边界），旧 split.png 误差35.28，断言明确失败；exclusive-diff.png 与拖宽后的 exclusive-wider.png 均误差0、位置(2114,136)，断言通过。该修复已随独占布局构建，因旧截图准确记录了相同触发路径，直接使用旧/新截图回归，不另添加诊断日志或重建旧版。证据仅覆盖当前 DPI/视口与此次侧栏拖动，不扩大为全部窄窗口/Tab溢出验收。

exclusive-project-diff/hidden-diff/restored-diff.png 进一步确认：切换到 Project、收起侧栏、恢复 Saved Changes 时中央 stash Diff 保留，完整齿轮检查均误差0；exclusive-settings.png 确认实际点击齿轮后菜单在窗口内打开。用户原图保存为 user-clipped-toolbar.png。verify.ps1 断言 HEAD、tracked diff 和 notes.txt 均未改变，关闭自有 PID28240；先前 PID6020 在换版时已核验路径关闭。未触发 apply/pop、未操作用户 Rebased，未新增像素级完成或内存验收结论。

## 本地变更侧栏的状态拆分准备（2026-10-17）

上一轮独占侧栏与右侧图标溢出修复均有原生证据，属于有效进展。本轮读取 ChangesViewPanel、NonModalCommitPanel、LocalChangesListView 及其三项按路径选择测试，开始拆分被 Log/Local Changes 共用的状态。

新增 src/local_changes.rs（lib 导出）：纯元数据变更树模型，不持有文件正文；Area/path/old_path 稳定身份，刷新重排保留目标，目标删除或 rename 来源变化时清除选择；折叠保留选择、键盘跳过不可见文件；清理已不存在的目录/分组状态。失败/加载期间不调用 replace，因此保留原有选择；异步取消及 generation 仍需由下一步独立 UI 实体实现。该模型只管理审阅选择，未定义 Commit 勾选，不能据此开始 git add。

Linux 5 项单测通过（测试构建5.22秒），Windows 测试构建通过（4.02秒）。覆盖暂存/未暂存同路径及Unicode刷新重排、rename来源改变、折叠选择/导航、空列表、不继承已消失目录的折叠状态。真实 Git local_changes 读取层未修改，本次不重跑 Git 写操作全量。尚未接 UI、未改变可见界面或更新便携包；本地更改独立实体、中央Tab迁移和原生验收继续待办。
Windows 原生执行同5项模型测试全部通过（0.01秒），日志和脚本位于 artifacts/local-changes-model；本轮未启动GUI窗口。

## 本地更改接入独占侧栏（2026-10-17）

src/local_view.rs 新增独立异步实体，使用上一轮 Model；任务取消/generation、刷新失败保留旧树但禁用打开比较，切仓销毁实体。树选择、展开/收起、上下键及 Enter/Ctrl+D 绑定在 LocalChanges 上下文；工具栏提供刷新/Show Diff/Expand All/Collapse All。src/ui.rs 新增第三个互斥工具入口，移除中央本地更改Tab，Saved Changes 的 ViewLocal 也进入此侧栏。比较通过既有 Target/Chain/DiffTab 路由；外部Git操作结束调用的refresh_impl同步刷新已创建本地实体。

初次 Windows release 通过（18.33秒），专用 fixture 同一 sample.rs 同时含 staged/unstaged 修改，另有两个未跟踪文件。PID6072 原生 local.png 确认新结构；unstaged.png 与 staged.png 分别只有对应的新增行，两个完整target身份独立Tab；project-with-diff/local-restored/local-refreshed/log-retained.png 覆盖切换Project再返回、刷新保留staged选择及返回Log仍选中原Side提交。首次尝试Enter被前台保护拒绝，没有向其他应用发键；随后使用定向点击Show Diff验证，键盘通路尚未本轮验收。补齐Unstaged/Untracked中文后再次release通过（25.65秒）。

仍未完成：底部预览偏好在新的本地侧栏重新接线（当前显式比较走Tab）、键盘/双击及错误状态完整原生验收、旧local中央模式死分支清理、Commit/Amend勾选和消息区。四种Area组仍是现有审阅语义，尚未改成Commit change list，完整侧栏/Commit验收不勾选。没有更新便携包或新增低内存预算结论。
最终 PID37428 的 final-zh.png 确认四组中当前出现的未暂存/已暂存/未跟踪标签均中文。verify.ps1 实际比较HEAD、index tree、sample.rs工作区hash及未跟踪正文，全部未改变，assertions.json落盘；两次自有测试进程均路径核验后关闭，未操作用户Rebased。

## 本地底部预览与键盘动作恢复（2026-10-17）

上一轮独占本地侧栏已真实接入，现恢复迁移遗漏的bottom偏好。实现映射与源码见docs/REBASED-REFERENCE.md。复用DiffTab embedded模式，单击/上下选择产生片段预览但不抢树焦点；显式Show Diff/Enter仍打开完整独立Tab。预览与Log比较数据互不覆盖，空选择不自动读取，切换侧栏/活动Tab、读取刷新和切仓失效释放。底部模式的中央区域在本地侧栏激活时显示本地预览，切换其他侧栏回到Log预览。

Windows release 24.64秒通过，artifacts/local-preview独立fixture沿用同路径暂存/未暂存数据并配置bottom。PID8432，empty-selection.png显示空选择提示；unstaged-ready.png显示暂存区到工作区；定向WM_KEYDOWN/UP的Down切到staged并更新HEAD到index预览（down-staged-preview.png），Enter打开对应独立Tab（enter-tab.png）。这证明GPUI键分发与预览不抢焦点，不宣称已验证宿主实体快捷键的所有组合。没有操作用户窗口或自动滚动。工具栏差异按钮可用，embedded文件链导航暂隐藏以确保侧栏选择为唯一来源。

完整像素/多状态验收尚未完成；未修改Git写入或提交勾选逻辑，未更新便携包。下阶段继续Commit/Amend的上游index保留、勾选与消息生命周期，不以继续像素微调延后该主线。
补充finish.ps1：log-preview-restored/project-log-preview/local-preview-restored/refresh-ready.png覆盖返回Log、切换Project、恢复本地窗口及刷新；最后仍为选中的staged预览。verify.ps1断言HEAD、index tree、tracked工作区hash和未跟踪正文未改变，assertions.json通过，自有PID8432已关闭。采样仅辅助观察，不作为Job峰值或GPU内存验收。

## Commit 暂存状态保存与恢复准备（2026-10-17）

前一目标轮仅复核已修复图标，未产生新增实现；本轮恢复功能主线。读取的 Rebased commit、文件和规则已记录 docs/REBASED-REFERENCE.md“Commit／Amend 的暂存区保留”。默认新后端以原 blob/mode 恢复未选文件的暂存状态，不重新 git add 工作区内容。

新增 src/commit_index.rs::ExcludedIndex 与 tests/commit_index.rs。只构造准备/恢复的 NUL index-info 输入，元数据有8MiB输入预算，不保留正文；冲突和异常记录明确拒绝。未选文件的暂存新增、删除、修改、rename 两端及文件/目录互换可暂时回到HEAD，再恢复原index；所选rename须传入两端。保留symlink/gitlink/executable模式、SHA-1/SHA-256身份和原始路径字节。没有接入可见按钮或生产写执行器，不将其宣称为Commit/Amend已完成。

Linux真实Git六项测试通过（0.56秒），涵盖普通提交/Amend/失败后的未选暂存blob及工作区保留、AD/DA/rename/file-directory、unborn HEAD、冲突拒绝、symlink/executable/换行路径、SHA-256/gitlink/intent-to-add。新增组合测试首次因fixture嵌套SHA-256仓库被外层git add拾取而失败，显式先销毁内层fixture后通过，非产品回退。Windows xwin测试构建通过；原生结果见 artifacts/commit-index。

下一步接入共享operation queue/process执行器，处理stdin、有界消息文件与编码、失败/取消后的强制恢复及恢复失败报告，然后实现Commit勾选、消息、Amend和Commit and Push。部分行、大小写rename、hooks/GPG仍需专门覆盖；当前无UI更改或新截图、不更新便携包、不新增内存验收结论。
Windows原生最终五项全部通过（4.69秒）；Unix专属symlink/换行路径用例未计入Windows覆盖，另外五项包含Windows真实Git的SHA-256、gitlink、Unicode、文件/目录互换和Amend。日志 windows-tests.txt / windows-stderr.txt 与 run.ps1 已保存。测试仅创建并删除自有临时仓库，无GUI进程或用户仓库写入。

## Commit 标准输入与取消通路（2026-10-17）

上一轮暂存计划及真实Git测试构成有效进展。本轮在共享operation_process增加run_with_input，沿用自有进程树、取消/超时、stdout/stderr有界读取；输入借用不复制，独立writer在线程scope中发送并关闭stdin。取消和超时先结束进程树，确保不读stdin的子进程不会让writer永久阻塞。输入错误单独记录，不丢Git stderr/退出状态/取消原因；旧run保持null stdin。参考源码及本地差异追加docs/REBASED-REFERENCE.md。

暂存计划真实Git测试改走此通路。Linux commit_index 6项、operation_process 7项、process_tree 4项通过，分别0.67/0.53/0.01秒（含两个fixture入口，不把它们称独立行为场景）。Windows交叉测试构建1.90秒通过，原生执行脚本与日志在artifacts/commit-input。可见界面未变，无截图或便携包更新。

仍待主线：持有仓库queue guard的完整Commit执行器，消息文件/编码、校验和所选文件暂存、提交结果识别、取消后的恢复以及恢复错误明确报告；然后接Commit/Amend UI与Commit and Push。当前新增传输不代表这些产品行为已实现。
Windows原生 operation_process 7项（1.00秒）、commit_index 5项（5.83秒）、process_tree 4项（0.15秒）全部通过，包含阻塞stdin的取消/超时及真实Git暂存恢复；未运行Unix专属链接路径测试。仅使用自有测试子进程和临时仓库，没有启动或关闭用户GUI。

## Commit／Amend Last 执行器接线（2026-10-17）

上一轮输入并发与取消通路及双平台测试均已落地，本轮继续接入实际提交事务。新增src/commit_operation.rs：显式所选add/remove、消息和预期HEAD；仓库guard覆盖整个流程；校验后按Rebased顺序保存排除项、暂存所选、git commit -F [--amend]、恢复排除项；失败/取消同样尝试恢复，恢复使用新token。返回各阶段结果、恢复错误/数据、实际HEAD及operation state，提交成功但恢复失败不能succeeded。临时消息文件创建/删除，literal NUL路径及Unicode消息均接入。源码对应/保留差异见docs/REBASED-REFERENCE.md。

Linux12项真实Git测试通过（0.76秒），其中六项为执行器新增：正常/Amend排除index不泄漏、hook拒绝、预检/锁失败、提交成功但恢复失败、hook中取消，以及unborn/仅消息Amend/大小写rename/带特殊字符路径/所选删除。原有六项暂存计划回归也通过。所有hook/锁均在测试自有临时仓库创建；取消留下的Git锁由测试在确认所有自有writer结束后清理，产品不会自动猜测锁归属。Windows测试构建通过，原生验证见artifacts/commit-executor。

尚未接UI或宣称Commit功能交付。下一步必须补恢复数据持久化与显式恢复路径、GPG/空提交结果分类、非UTF8消息编码策略及其测试，再接勾选/消息/Amend与Commit and Push；目录/submodule选择与部分行提交另待支持。当前只返回内存恢复数据，不能据此承诺进程退出/崩溃后可恢复；未改可见UI，不需本轮界面截图，未更新便携包。
Windows原生最终11项全部通过（13.83秒），包括Windows真实Git的大小写rename、中文消息/路径、hook取消/拒绝、恢复失败和仅消息Amend；Unix专属symlink/换行路径不计入Windows覆盖。测试脚本及stdout/stderr已保存于artifacts/commit-executor；无用户仓库操作或GUI进程。

## 提交结果分类（2026-10-17）

上一轮执行器及双平台真实Git验证是有效进展。本轮补src/commit_result.rs和Report::status，按已核对的Rebased匹配规则区分stdout空提交、stderr签名与pinentry问题；检测过程中保留有限状态，原始日志仍受64KiB尾部限制。恢复失败优先报告，保留底层commit结果；正常成功才允许succeeded。阶段间取消单独保留，不把未启动的commit误归普通失败。

Linux检测单测2项、真实Git commit_index 13项、operation_process 7项通过（含fixture入口）；新增空提交/签名失败真实场景确认不自动unsigned retry、HEAD不变、排除index恢复。Windows交叉构建通过（3.72秒），原生结果及脚本artifacts/commit-result。未接UI/Push，尚无可见功能验收或便携包更新。

接下来仍需持久恢复记录及显式恢复交互、消息编码策略，再接Commit/Amend消息与勾选，不能因新增错误分类就将完整功能勾选。
Windows原生检测2项（0.01秒）、进程7项（1.10秒）、真实Git提交12项（13.41秒）全部通过。Unix链接/换行路径用例未计入Windows覆盖；无用户仓库、密钥、GUI操作。

## 提交恢复元数据持久化（2026-10-17）

上一轮结果分类已双平台验证。本轮新增commit_recovery模块，将原HEAD、prepare与restore的mode/OID/path元数据先独占写入每工作树Git目录并sync_all，再允许index变更。恢复成功删除；恢复失败保留；删除失败单独状态，不能报普通成功。新Commit遇到现存标记直接停止，不覆盖旧恢复信息。数据不包含文件正文/提交消息；有版本、长度校验和读取上限，未知/损坏记录拒绝读取且继续阻止提交。源码依据及本地扩展说明写入docs/REBASED-REFERENCE.md。

Linux18项测试通过（0.82秒，含一个fixture入口）。新增覆盖释放Report后从磁盘恢复、旧记录阻止覆盖、成功/拒绝hook后清理、损坏/过长记录拒绝；强制结束拥有的测试进程树前确认index已prepare，结束后从完整磁盘记录在测试仓库重放，原暂存blob与未暂存工作区均恢复。不是只模拟对象drop；但不扩大为断电测试。Windows交叉构建1.84秒通过，原生执行日志/脚本在artifacts/commit-recovery。

尚未完成生产显式恢复API/UI：下一步核对受影响index身份、防止覆盖记录创建后的外部暂存修改，再提供恢复动作；对象保活与消息编码仍待。当前记录读出不直接执行Git，测试手动重放不能作为用户恢复交互已完成。Commit/Amend按钮仍未接入，不更新便携包或视觉验收。
Windows原生17项全部通过（16.89秒，含fixture入口），包含真实强制结束进程树后读盘恢复、拒绝覆盖旧记录及所有已有Commit回归。Unix专属链接路径场景未计入Windows覆盖；测试仅处理自有进程和临时仓库。

## 显式暂存恢复 API（2026-10-17）

上一轮持久记录和真实进程终止验证有效。本轮实现commit_recovery::restore，串行guard内校验记录结构、受影响路径集合、当前index模式/OID、冲突及祖先/后代路径碰撞；只接受原始或prepared身份，保留不相关暂存项。恢复后再次核对index与记录，成功才清理。失败/取消/锁存在保留原记录；不自动启动恢复、不改HEAD或工作区、不删除Git锁。外部Git并发窗口与8MiB当前index读取上限明确记录，不声称全局事务隔离。

Linux完整20项通过（1.37秒，含fixture）；随后加强descendant场景，使prepared中该路径真正不存在，初版fixture多余git rm失败，改为ignore-unmatch后定向2项通过（1.36秒）。验证保留无关新暂存、拒绝受影响文件新内容、新子路径碰撞、取消/锁失败保留记录及index。Windows构建及原生测试证据在artifacts/commit-recovery-action。

恢复API已可供UI调用；下一步推进Commit侧栏的独立勾选、消息、Amend草稿与恢复提示，按Rebased入口/布局接线。尚未进行这部分原生界面验收；UTF-8以外消息等明确限制不隐藏、不把其余范围勾选完成。
Windows首轮运行了加强fixture但尚未修正多余rm的构建，同样因fixture命令失败（windows-fixture-failure.txt）；修正后重新构建并原生19项全部通过（21.66秒，含fixture入口）。最终日志windows-tests.txt。未使用用户仓库或GUI进程。

## Amend 草稿与异步消息状态（2026-10-17）

上一轮恢复API和双平台验证构成有效进展。本轮深入读取Rebased Amend handler、message policy和精确空白规则，新增src/commit_draft.rs：草稿独立于审阅选择，Amend异步状态/请求generation、未编辑自动消息还原、手动编辑保留、加载失败禁用提交、成功后清理；支持无勾选的仅消息Amend。读取固定OID完整UTF-8消息，前后核对HEAD，过期请求不回写。源码规则与本地补充差异详见docs/REBASED-REFERENCE.md。

Linux3项模型用例通过；真实Git完整中文多段消息/HEAD变化/取消/只读index用例通过（0.15秒）。Windows xwin构建2.77秒通过，原生日志artifacts/commit-draft。无可见UI变动，不将后台模型称为侧栏已交付。下一步文件勾选和真实侧栏接线、恢复提示及Commit and Push仍待，需Windows原生结构交互验收。
Windows原生3项草稿模型测试与1项真实Git消息读取测试全过（0.00/1.19秒）；只执行相关过滤用例，不将编译出的其他用例计入本轮通过数。没有操作用户GUI或仓库。

## 提交文件包含状态（2026-10-17）

上一轮Amend草稿已验证。本轮补commit_selection独立整文件勾选模型；暂存/未暂存同路径合并成一个提交项，审阅选择不修改包含状态。稳定身份包含rename来源，刷新保留有效已有勾选但不静默纳入新文件；冲突禁止提交。生成执行器Request处理删除、删除后重建、rename来源、copy来源保留及add/remove去重，源码依据和差异记录REBASED-REFERENCE。

Linux3项模型测试通过，2项真实Git读取status→选择→执行器测试通过（0.27秒）：同时暂存/未暂存的所选文件提交工作区版本，未选文件的index与工作区分别保留；已暂存删除后重建提交新文件内容而非错误删除。Windows构建/原生过滤用例日志artifacts/commit-selection。

当前仍是状态与执行器接线，不是可见Commit侧栏完成。接下来的UI必须把这套勾选与消息模型一起接入，保持独占侧栏、中央Diff和失败后草稿，不新增无行为的Commit占位按钮；需要Windows原生截图/操作验收。未更新便携包。
Windows原生3项包含状态模型与2项真实Git提交联调用例全部通过（0.00/2.28秒）。只运行相关过滤用例，其他编译出的用例未重复计入。无GUI改动、未操作用户仓库。

## Commit 侧栏实际接线与原生操作（2026-10-17）

本轮不再只停留模型：LocalView接入勾选、消息Textarea、Amend异步加载、Commit、Commit and Push、取消与恢复按钮，后台操作完成通知Review刷新Log及本地树。失败保留草稿/包含集合，成功清理；Push仅在提交全部成功后打开既有预览。移除侧栏父容器抢输入焦点，文件行单独管理审阅焦点。源码入口和差异见REBASED-REFERENCE。

Windows release首构建25.29秒通过。隔离fixture/PID36368，artifacts/commit-sidebar保存open/checked/message/amend-loaded/draft-restored：同路径staged/unstaged勾选同步，Amend开启加载Main review commit，关闭恢复原Review selected changes草稿。commit.ps1原生点击后断言新HEAD、消息、仅src/sample.rs被提交、包含其未暂存行、notes未被加入；committed.png确认Log新增记录、草稿清空。

仅消息Amend原生操作保持提交数量3，HEAD更新；首次脚本试图用Home/Delete替换消息，实际Home落在末尾空行，变成追加，因此精确消息断言失败，未将其写成产品失败。脚本改为明确追加Amended review并验证消息后缀/HEAD/数量，重跑通过，amend-assertions.json记录实际消息。failure.ps1在自有fixture安装拒绝pre-commit hook，HEAD不变、所选notes暂存保留；failure-retained.png实际确认草稿/勾选和错误可见，随后删除自有hook。

push-dialog.ps1创建本地bare remote，点击提交并推送；本地notes提交成功后Push对话框列出main→origin:main，截图push-dialog.png，取消后remote无refs，断言未在确认前写远端。未执行真实推送或触及用户仓库。补whole-file提示和按钮换行后再构建/重启验证。

尚未验收/实现差异：保持四Area审阅分组（同路径两行共享勾选），最终Changes/Unversioned组织、目录/组勾选、原生取消和恢复按钮交互、英文/窄窗完整状态、完整错误查看、持久消息草稿、非UTF8编码等仍待。不是像素级完成，也未更新便携包或重做Job内存预算。
最终补提交前再次读取/验证Textarea，避免超过消息限制的无效编辑仅报错却仍使用旧Draft提交。后两次Windows release分别25.44/19.27秒通过。自有PID32812中间重启、最终PID27920原生final-sidebar.png已核对；finish.ps1确认HEAD保持最后成功提交、tracked/index干净、未选binary仍未跟踪、本地bare remote无写入，final-assertions.json落盘。36368/32812/27920均按路径核验正常关闭，三个启动会话结束0；未关闭用户Rebased。新增提示hover和英文换行状态尚未单独截图验收，不扩大为完成全部状态。

### Rebased 原生交互驱动对齐（2026-10-17）

已成功操作用户打开的 Rebased 1.1.20，保存 Commit 的 Amend 开关、组勾选、文件选择与 Ctrl+D 独立 Diff Tab 截图到 `artifacts/rebased-commit-interaction/`。源码依据与安装版本区别见 docs/REBASED-REFERENCE.md 同名记录；没有提交/推送用户仓库，用户窗口保持打开。

本轮修正并经 Windows 原生截图核对：Amend 自动消息替换/恢复时聚焦并全选，两颗提交按钮同步切换 Amend 文案（中文已验证，英文像素效果待验收）。Windows release 构建通过；没有重复 Git 写操作回归，因为本轮只改变消息焦点和操作标签。

接下来 Commit UI/UX 的顺序：先按源码统一 Changes/Unversioned 文件呈现并实现组/目录勾选，避免同一文件因暂存状态重复；再实现 Amend 原提交及文件展示；随后核对 Commit Diff 的包含控制与现有只读审阅范围。每项继续先操作原生参考、读对应源码、再实现与同条件截图验收。保持用户要求的左侧工具窗口独占，不复制参考中的上下分区。仍不宣称完整 Commit UX 或像素级还原已完成。

### Commit 组／目录三态勾选（2026-10-17）

已按ChangesTree的computeNodeStatus/toggleChanges补组与目录勾选：部分横线→全选→清空；折叠后包含隐藏后代；不改变审阅选中项。冲突不允许包含，范围外勾选保留，重复Area行共享整文件身份。实现和源码依据见REBASED-REFERENCE对应记录。

Windows release 首次因 Rems/Pixels 混算编译失败，改为相对中心加像素偏移后构建通过（1m00s）。Linux local_changes 6项、独立group_inclusion测试通过；此前selection原3项也通过。Windows自有PID36124原生验证：mixed.png为单文件勾选后的组横线，all.png为点击混合组后全选，none.png为再次点击清空；collapsed-included.png确认折叠目录勾选仍作用到隐藏文件，并同步同路径另一Area行，Log/预览未改变。artifacts/commit-group-inclusion/finish.ps1核对HEAD/index树/工作区文件hash与启动前一致，assertions.json落盘，随后正常关闭自有窗口。未做滚动测试，未关闭用户Rebased。

尚待：统一Changes/Unversioned及相应HEAD→工作区完整Diff、Amend原提交文件组、部分行范围决策、目录图标与缩进精调；英文、键盘Mixed激活和大树性能未进行原生专项验收。本轮未更新便携包，不标记完整Commit UX完成。

### 统一默认预览逻辑（用户补充，2026-10-17）

把本地变更、Log提交文件、已保存更改、Push预览的默认展示作为同一交互专项：有可预览候选时不长期停留在“选择文件以查看差异”。优先保留当前文件，无有效当前项时按参考候选顺序回退；空候选才显示空态。具体焦点/刷新例外和Push初始提交选择按源码确认，不用“一律选第一个”替代。默认预览、树选择、提交勾选和打开独立Tab分别处理。源码已确认的基类规则与未读入口记录于REBASED-REFERENCE；实现及Windows原生验收待做，随当前分组/完整工作区Diff一起推进。

### Changes 分组的完整比较前置（2026-10-17）

WorkingTreeChange与HEAD→工作区净列表/全文Diff接口已实现，保留现有Area审阅API，尚不切换UI。已对照GitChangesCollector与GitChangeProviderVersionedTest，确认MM还原、AD消失、修改before/after版本语义。Linux真实Git3项通过；Windows原生测试与记录在artifacts/working-tree-review。此阶段无可见界面变化，不用构建通过代替后续UI验收。

下一步仍需完整Commit收集器：status中的冲突/未跟踪与净变更合并，删除重建/重命名语义测试，通过后再接统一文件树、Diff Target及默认预览。不得直接用净diff原始列表取代所有状态处理。用户要求的默认展示与焦点保护继续按上一节推进。

Windows原生net_working_tree三项全部通过（2.19s）；本轮没有启动或关闭用户GUI程序。

### Commit 状态合并模型（2026-10-17）

新增commit_changes::Snapshot，按Rebased status规则分出Tracked/Unversioned/Conflict，保留原始暂存/工作区身份；MM还原/AD省略，RD展示旧路径删除，未知组合报错。Linux真实Git3项通过，覆盖重命名删除、删除重建和实际冲突；Windows结果在artifacts/commit-collector。

尚未接侧栏：同路径的已暂存删除和未跟踪重建不能共用现有path-only勾选身份；RD也需关联原始index目标。因此下一步先把可提交项身份/端点映射与新模型接通，再迁移文件树和默认预览。保留原计划范围，尚不宣称分组或默认预览已完成。

Windows宿主Git的commit_collector三项全部通过（2.71s），stdout/stderr位于artifacts/commit-collector；测试只写自有临时仓库，本轮无GUI变化。

### Commit 新勾选身份与范围映射（2026-10-17）

commit_inclusion已实现Kind+path+before/after身份、范围勾选和快照HEAD请求。与commit_changes结合可区分同路径删除/未跟踪重建项。重新读stageAndCommit和index-info saver后修正RD设计：只提交旧路径删除，未选rename目标暂存内容按上游保存/恢复，不能自动清理目标。详情见REBASED-REFERENCE。

Linux真实提交3项通过，验证删除/重建分别勾选、同时勾选、RD未选暂存目标保留，以及其他未选暂存/工作区内容不变。Windows验证日志在artifacts/commit-inclusion。下一步用新收集器与勾选模型迁移LocalView、支持完整工作区Diff Target，再统一默认预览；旧侧栏尚未切换，完整UI验收未完成。

Windows宿主Git的commit_inclusion三项全部通过（4.71s），输出已保存；仅操作独立临时仓库，本轮没有GUI变化。

### Commit 统一分组已接入 UI（2026-10-17）

LocalView现已使用新Snapshot/Inclusion，显示更改/未版本化文件（另有冲突组），普通暂存+未暂存文件不再重复。新增WorkingTree比较区域接到Target/Chain，Diff以HEAD→工作区展示完整范围；底部预览未显式选择时回退首项，不自动勾选或抢开Tab。

Windows release与原生验证通过：artifacts/commit-unified-ui的preview-ready.png显示默认预览及两部分修改；组勾选后真实Commit仅提交sample.rs，断言见commit-assertions.json；full-tab.png验证显式打开全文。Linux桥接测试和local_changes六项通过。具体源码及限制见REBASED-REFERENCE。

下一步继续统一各入口默认预览、保留当前文件与焦点/刷新生命周期，处理LocalView notify引起预览重建的问题；之后补Amend原提交文件展示。Log/Stash/Push尚未完成统一；Tab名称和版本标题布局、图标缩进、双语言完整状态仍待视觉验收，不视为完整Commit功能交付。

### Commit 无关交互不再重建预览（2026-10-17）

预览现在按文件身份和刷新代次复用；输入草稿、组勾选、展开折叠不重建viewer/重复克隆Chain，真正刷新或切文件才更新。Windows原生GIT_TRACE验收通过：diff调用数普通交互1→1、追加fixture内容后刷新1→2，截图确认新内容、草稿/勾选保留和切文件有效，证据在artifacts/commit-preview-lifecycle。源码及差异见REBASED-REFERENCE。

下一步继续Log/Stash/Push默认预览规则及刷新焦点/视口保护；本轮仅解决Commit普通交互重载，未宣称统一工作完成。

### Push 默认展示源码核对与合并基础（2026-10-17）

确认Rebased默认选择仓库节点，展示其待推送提交的合并文件列表，而非首条提交；单提交详情仅在选具体commit时显示。已翻译上游zipChanges元数据算法到combined_changes.rs，保留重命名链、删除重建和碰撞顺序，100,000输入变更上限；Linux四项通过，Windows结果在artifacts/combined-changes，源码/许可证记录齐全。

下一步Push接仓库级聚合范围和复合版本Diff Target，特别处理分页不能只合并当前100条却标为全部。现有Push界面尚未切换默认展示；其余默认预览/焦点规则继续保留原范围。

Windows原生combined_changes四项全部通过（0.08s），输出保存于artifacts/combined-changes；本轮未操作用户GUI或仓库。

### Push 复合版本端点转换（2026-10-17）

已按 GitChangesParser 源码增加 combined_changes::from_file，保存每个文件的 before/after OID 与路径；根提交新增、重命名链、删除可进入已实现的 zipChanges 算法。Linux 合并模块五项测试通过；引用与保留差异见 REBASED-REFERENCE。后续仍需完整 outgoing 范围收集、merge 规则核对、复合 Diff 内容读取及 Push UI 接线，不能把当前分页误作完整推送范围。本轮没有 GUI 变化。

本轮 Windows 原生同组五项测试亦通过（0.08s），日志：artifacts/combined-changes/gitui_review.txt。

### Push 全范围元数据读取（2026-10-17）

增加 outgoing_preview_commits，跨 UI 分页收集固定 source/remote 快照的全部可展示待推送提交；保持新分支上游 1000 上限，预算超限显式失败。真实仓库 105 条记录的跨页、预算、引用过期和取消测试 Linux 通过。确认 Rebased Push merge 使用 combined diff，不能复用 first-parent 文件列表；后续先实现对应文件收集，再接 zipChanges、复合 Diff 和仓库默认选择。尚未有 UI 变化或完成默认预览验收。

Windows 原生相同跨页集成测试通过（7.39s）；日志 artifacts/outgoing-preview/repository.txt。仅操作隔离临时仓库，未执行网络推送。

### Push combined 文件收集 API（2026-10-17）

已新增 push_commit_changes，遵循 Rebased 默认 root/-M/combined 状态与首字符解析规则；真实仓库验证根新增、普通 rename、merge 冲突解决与双方独有文件排除。Linux 通过，原生 Windows 测试见 artifacts/push-combined-files。后续将全范围元数据、逐提交文件收集与 zipChanges 串联，并补浅边界来源验证、复杂 merge rename、复合 Diff 内容读取及默认仓库 UI；尚未宣称默认预览功能交付。

本轮 Windows 原生 combined 文件集成测试通过（1.72s），日志 artifacts/push-combined-files/repository.txt；没有界面改动。

### Push 仓库级聚合查询接通（2026-10-17）

push_preview::load 已串联全范围记录、逆序 combined 文件收集和 zipChanges，输出不带正文的合并端点及范围信息。加入真实浅边界识别、完成后 shallow/remote 快照复验、输入数量预算与取消；Linux 两项相关集成测试通过。接下来仍需复合文件正文读取、Diff Target/Chain、Push 默认仓库选择及 Windows UI 验收；当前仅数据层可用，不视为默认预览已交付。逐提交子进程成本与内存需在 UI 接通后按实际加载规模测量。

本轮 Windows 原生两项聚合/浅边界集成测试通过（2.79s），日志 artifacts/push-preview-snapshot/repository.txt。无 GUI 变化。

### 复合 Diff 全文读取（2026-10-17）

combined_changes::read_contents 已按独立 OID+path 读取两侧全文，区分缺失版本和空文件，不受工作区内容影响；二进制/编码/读取上限沿用现有明确报错策略。Linux 真实仓库测试通过，源码规则与差异见 REBASED-REFERENCE。下一步扩展 Diff Target/Chain 与 viewer 内容入口，再接 Push 默认仓库合并展示及原生 UI 验收。本轮没有界面变更。

Windows 原生复合正文集成测试通过（1.13s），日志 artifacts/combined-contents/repository.txt。

### 复合 Diff 身份与文件导航（2026-10-17）

Target/Chain 已支持独立前后版本与路径，完整端点用于身份及同路径删除/重建区分，元数据共享而游标独立；复合正文读取接入 Target。Linux 三项相关单测通过，Windows 日志在 artifacts/combined-target。下一步接 DiffTab 两侧文本入口、标题与 Push 仓库默认选择，再进行原生 GUI 验收。本轮无界面变化。

Windows 三项 Target/Chain 单测通过（0.01s）；Linux 经 Target 读取全文的集成测试通过（0.14s）。

### 复合全文 Diff viewer 接线（2026-10-17）

DiffFile 增加双侧全文构造，DiffTab 接复合目标正文与真实版本标题；不经临时 patch，保留纯重命名完整两侧及缺失/空文件区别。Windows 原生文档测试通过，见 artifacts/combined-document。Push 尚未使用此入口，实际 GUI 截图/交互验收仍待下一步仓库默认选择接线后完成；算法对齐和内存差异见 REBASED-REFERENCE，不视为功能已完整交付。

本轮 Windows release 构建通过（1m02s）。原生窗口截图与复合 Push 交互仍待入口接线后验收。

### Push 默认仓库预览功能接线与 Windows 验收（2026-10-17）

默认选中仓库并展示完整待推送范围的合并文件；Show Diff 默认首文件、单提交范围切换、返回仓库范围和复合全文导航已接通。Windows release 与自有窗口验证通过：artifacts/push-aggregate-ui 中 ready/diff/single/rename/added/repository-restored 截图及 assertions.json，确认重命名端点正确、新增后删除抵消、纯重命名全文和首项默认打开。测试窗口已关闭，HEAD/远端未改动。

此项从纯数据层推进到可见功能，但暂不标整体 Push/默认预览专项完成：Rebased 同条件原生 Push 对照、仓库树/文件目录图标布局、详细失败反馈、跨页和大规模性能、快速切目标等仍待；Log/Stash 的统一默认预览与焦点保留仍在原范围内。精确差异和本轮源码依据见 REBASED-REFERENCE，便携包尚未更新。

### 已保存更改刷新选择保留（2026-10-17）

按 SavedPatchesChangesBrowser/SavedPatchesDiffProcessor 的 patch identity 和 tag 规则，Stash 刷新保留同一固定 OID 下的文件比较身份；Index/Working tree 同路径不串，已加载同 stash 重复点击不重置选择。Windows release 和原生刷新、stash 下标移动、Index Diff 打开验证通过，证据 artifacts/stash-selection-refresh；测试窗口已关闭。

此项仅处理 Stash 选择恢复。加载中连续刷新、原 stash 消失、默认底部预览、焦点/视口保护与 Log 全量统一仍待；不称默认预览专项已全部完成。源码映射与差异见 REBASED-REFERENCE。

### Stash 连续刷新不丢失待恢复选择（2026-10-17）

pending_selection 跨刷新两阶段保留固定 stash/分组/文件身份；用户显式选择替换偏好，空列表清理。Windows release 和原生 8 次快速刷新、旧 stash 被删除后的首项回退、全部移除后的空态验证通过（artifacts/stash-refresh-burst）。测试窗口已关闭，仅改变隔离 fixture 的 stash 栈；HEAD 与工作区断言通过。焦点/视口保护和其他入口仍继续原计划，未标默认预览专项完成。

### Log 跨提交保留对应文件（2026-10-17）

按 Rebased Change.equals/ChangeWrapper 的前后路径与分组规则，提取 preview_selection；Log 切换提交保留仍存在的对应文件，否则自动首项，空列表无选择。正文身份仍包含固定版本，避免保留选择时复用错误内容。Linux 定向单测和 Windows release 通过；原生两提交 b.txt 保留及根提交首项回退截图见 artifacts/log-preview-selection，HEAD/工作区断言通过，测试窗口已关闭。源码路径、版本与差异见 REBASED-REFERENCE。

默认预览专项仍待：加载中快速切换的偏好、焦点/视口保护、Stash 底部模式及同条件 Rebased 原生对照；底部预览视觉结构尚未统一，不将本轮作为全部完成。

### Log 快速切换期间保留预览偏好（2026-10-17）

已将待恢复文件从临时文件列表中独立保存，避免详情加载期间连续选提交后回到首项；成功模型、显式文件选择、空历史及切仓库按对应生命周期消费/清理，过期 generation 不覆盖新偏好。依据 VcsLogAsyncChangesTreeModel 的独立待处理状态与既有 ChangeWrapper 选择规则，详情见 REBASED-REFERENCE。Windows release 通过（19.09s）；原生 12 次快速切换及刷新后 b.txt 预览保留，截图和只读断言见 artifacts/log-preview-burst，测试窗口已关闭。默认预览专项的焦点/视口、Stash 底部模式与 Rebased 同条件原生对照仍未完成。

### Stash 底部默认预览（2026-10-17）

已接入设置中的底部模式：打开已保存更改自动预览首文件，单击其他文件更新，显式 Show Diff/双击仍打开独立 Tab。底部和 Tab 共享固定版本 Target/Chain 构造；同目标刷新保留 viewer，侧栏或 Tab 切换按单一预览槽释放。Windows release 通过（20.06s），原生默认 Working tree a.txt、切 Index b.txt、刷新保持均验证，证据 artifacts/stash-bottom-preview；测试窗口已关闭。源码依据、元数据复制及尚未进行同条件 Rebased 原生对照等差异见 REBASED-REFERENCE。默认预览全入口焦点/视口和键盘验收仍待。

### Stash 预览导航按需共享（2026-10-17）

移除每次界面通知时对整组文件导航数据的重复构造。当前 stash 各分组首次使用时创建 Chain，后续预览和独立 Tab 共享其 Arc 元数据及独立游标，刷新清理缓存，不缓存正文。依据 GitStashCache/GitStashUtils，映射及保留差异见 REBASED-REFERENCE。Linux 3 项共享/释放测试及 stash 分组真实仓库测试通过；Windows release 通过（19.92s），默认预览、切 Index、刷新原生回归见 artifacts/stash-shared-preview，测试窗口已关闭。尚未进行大规模进程内存验收，不以小 fixture 启动数据宣称内存目标达成。

### Log 多选的逐父状态合并基础（2026-10-17）

按功能主线推进 Log 多选。源码确认 collectChanges 逆序合并选中详情，但 Log provider 使用 DIFF_TO_PARENTS 后按 VcsStatusMerger 合并，不能直接复用 Push combined 第一状态字符的解析。新增 log_changes::merge_parents，覆盖共有路径、重命名来源、混合修改/删除语义与数量上限；Linux 和 Windows 原生各 4 项测试通过，日志 artifacts/log-parent-status。源码路径和许可证记录见 REBASED-REFERENCE/THIRD-PARTY。

尚待真实逐父 Git 查询、多个选中提交聚合、OID 选择模型、Ctrl/Shift/右键集合、右侧多详情和 Diff 接线。Squash 尚未实现；本轮仅完成必要数据基础，没有新 UI，不勾选多选或 Squash 验收。

### Log 多选聚合接通真实 Git 查询（2026-10-17）

新增 log_commit_changes 逐父文件查询及 log_changes::load 固定 OID 选择聚合；遵循行序逆序 zip，不扩展不连续选择为整个区间。校验重复/无效/数量，浅边界前后复验，取消和上限失败不返回部分结果。Linux 与 Windows 原生集成验证根/rename/merge、未选 gap 文件排除、新增后删除抵消及完整版本正文正确，Windows 日志 artifacts/log-selection-query（3 项，2.99s）。源码依据与差异见 REBASED-REFERENCE。

下一步仍是 OID 多选模型、Ctrl/Shift/右键与右侧详情/文件/Diff 同集合接线；复杂 merge rename、逐父分组展示及最终性能验收仍待。当前无 GUI 改动，多选和 Squash 不勾选完成。

### Log OID 多选模型（2026-10-17）

新增 log_selection::Selection，统一普通选择、Ctrl 切换、Shift 范围、Ctrl+Shift 锚点状态应用及键盘 lead；OID 集合按显示顺序导出，刷新/过滤/换页按身份交集恢复，不保留失效行号。右键选中项保留集合、未选中项单选。依据 VcsLogGraphTable/SelectionSnapshot 与 JTable API 契约，版本和待原生核对项见 REBASED-REFERENCE。Linux/Windows 原生各 4 项模型测试通过，日志 artifacts/log-selection-model。

模型尚未接界面：下一步与 log_changes 聚合、右侧详情、Diff、命令可用性同步接线，再做 Windows 实际 Ctrl/Shift/右键验收；不能把模型测试当多选 UI 完成。Squash 仍未实现。

### Log 多选界面、聚合文件与 Diff 接线（2026-10-17）

已接 Ctrl/Shift 点击、Shift+上下、右键选中集合保留和 OID 刷新恢复；多选右侧采用真实聚合文件，底部与独立 Diff Tab 使用同一复合端点/导航链。首项自动预览，切文件和复制多个 hash 均对应当前集合；Copy Revision Number 按上游旧→新、空格连接，多选禁用单提交 Checkout。多条消息/作者详情最多 50 条并设总展示文本预算，中英文提示同步。

Windows 最终 release 通过（20.11s），自有窗口 Ctrl 加入/移除、Shift 扩展、右键保持/Checkout 禁用、复制内容、刷新集合、b.txt 底部/独立 Tab 合并版本验证通过，证据 artifacts/log-multiselect-details；HEAD/工作区断言通过，窗口关闭。首轮 artifacts/log-multiselect-ui 发现详情区被单选条件隐藏，已修正后重验。

不勾选完整 Log 多选验收：多详情 refs/包含分支/父链接、Ctrl+Shift 实际窗口、键盘加载中保护、过滤/跨页/快速重选和空选择专项、复杂 merge rename 与低内存实测仍待；保留结构/视觉差异见 REBASED-REFERENCE。同条件 Rebased 原生对照未完成，Squash 尚未实现，便携包未更新。

### 多选详情 refs 与独立 metadata 查询（2026-10-17）

commit_metadata 只读取消息/身份/日期，多选详情不再重复查询文件 diff；details 复用此入口保持原单选行为。单/多选共享 commit_refs，逐卡片展示对应分支与 tag；补齐多选文字前景/链接样式，并禁止历史加载期间普通上下键重启选择查询。Linux/Windows 真实仓库 metadata 测试通过，最终 Windows release 20.88s。原生两卡片分别显示 HEAD/main/review-tag 与 review-base，截图 artifacts/log-multiselect-refs-final；复制及只读断言通过、窗口关闭。

多详情包含分支/父链接、复杂边界与实际大规模性能仍待；metadata 尚逐提交读取，未宣称批量或完整内存验收。其余多选与 Squash 主线保持原计划，源码和首次截图发现/修正的文字颜色问题见 REBASED-REFERENCE。

### 默认文件预览统一验收补充（2026-10-17）

按用户要求先核对源码，后续统一 Log、本地变更、已保存更改与推送预览：有效原选择优先，否则在当前范围候选中选首文件；无变化、加载和错误各用对应状态，不在有候选的正常预览中长期停留“选择文件以查看差异”。不要因刷新抢焦点或自动增开 Diff Tab。Push 默认范围是仓库待推送提交的合并变化，而非首个 commit。Local Changes 空选择与聚焦保护有上游特例，不能无条件套用首项。

验收应分别检查列表默认选择、当前预览正文、显式打开 Diff 的回退、刷新后的选择/焦点和范围切换；现有 Push 的 Show Diff 首项回退不能单独代表整项完成。源码依据与本地映射见 REBASED-REFERENCE.md“默认预览统一规则复核”。本轮仅记录规则；全入口统一与原生交互验收仍待完成。

### Log 多选包含分支详情（2026-10-17）

多条提交详情已接各自包含分支异步展示，共用串行查询，随选择释放并限制结果保留量；不会为每张卡片同时启动一个 Git 查询。Windows release 和原生两提交 fixture 验证通过，截图/trace/只读断言在 artifacts/log-containing-selection；源码映射与差异见 REBASED-REFERENCE。外部 refs 变化的查询中复验、父提交导航与 Squash 仍待，未宣称 Log 全部完成。

### Squash 重放计划与真实 Git 验证（2026-10-17）

新增 squash_plan：按上游 unite 将选中提交移到最旧选择之后，以 reword/fixup 合并，未选提交顺序保留；消息草稿按上游去重与 autosquash subject 规则生成。Linux 纯模型及真实 Git 最近连续／中间连续／不连续／冲突后中止四场景通过；详情与差异见 REBASED-REFERENCE。

此为 Squash 后端基础，不勾选 Squash 功能完成。下一步必须补操作前完整线性路径与 protected ref/HEAD/状态复验、真实 todo 对照及消息映射生命周期，再接操作执行/继续/中止/Undo 和多选右键消息弹窗；不能把测试直接调用 rebase 当作产品写操作入口已完成。

Windows 原生 Squash 计划测试也通过：真实 Git 四场景 8.99s，模型/消息两项 0.01s，证据 artifacts/squash-plan。当前没有可见 UI 变化，UI 验收等待写操作入口接通。

### Squash HEAD 路径及保护分支准备（2026-10-17）

只读 prepare 已接真实 Git：基于 expected HEAD 验证完整选择在线性重写路径内，检查根/merge/浅边界和最旧选择的保护远端 ancestry，读取前后复验引用，生成同一 Squash Plan；保留 detached 与 operation state 供执行流程处理。Linux 两项集成测试通过，验证已暂存/未暂存内容未改动。源码映射与限制见 REBASED-REFERENCE。

仍待执行器在操作锁内重新验证、进行中状态/本地修改决策、实际 todo 快照匹配、编辑器消息映射与冲突/取消/Undo，再接右键及消息弹窗。当前 prepare 成功不能直接视为允许写入；不勾选 Squash 功能交付。

Windows 原生两项 Squash 准备集成测试通过（9.18s），日志 artifacts/squash-check/repository.txt。仍为只读后端准备，无新增可见 UI，未更新便携包。

### Squash 实际 todo 校验接入编辑器测试（2026-10-17）

Plan 保存原始重放顺序，公开 rewrite_todo 验证 Git 实际输出的完整范围/顺序、唯一缩写与动作后才生成新 todo；不再公开绕过核对的 raw todo 方法。正常四场景与实际编辑器拒绝缺项/重排/额外指令的 Linux 集成测试通过，拒绝时原 HEAD/index/worktree 保持且无残留 rebase。源码、性能边界和通用 todo 动作尚不支持的差异见 REBASED-REFERENCE。

下一步仍为执行器与持久 reword 消息映射（onto/原消息身份匹配、冲突继续、取消恢复），不能直接把所有 editor 请求覆盖成同一消息。UI 和完整 Squash 功能尚未交付。

Windows 原生同组验证通过：两个真实 Git 集成测试（含四种重放/恢复与三种拒绝场景）13.23s，三项模型测试 0.01s；日志 artifacts/squash-todo。未更新便携包，无新增 GUI 验收。

### Squash 消息持久映射基础（2026-10-17）

新增随 rebase-merge 生命周期保存的原消息→新消息映射，验证 onto/orig-head/消息前缀且只应用一次；sequence 请求不受影响，匹配失败留给普通编辑器。正常 Squash 真实 Git 测试已改用该映射，身份不符/损坏/外部文件修改/重复使用与手工回退测试通过。源码、崩溃窗口和存储差异见 REBASED-REFERENCE。

尚需将 prepare、todo、消息映射接成完整执行器，并接 Continue/Abort 路由与恢复提示、Undo、右键及消息弹窗。不能将本轮磁盘映射测试算作完整冲突继续或应用重启验收。

Windows 原生三项 editor 集成测试通过（15.62s），三项计划测试通过（0.01s），证据 artifacts/squash-message。额外断言 sequence 请求不消费消息映射，以及 Git 完成/中止后映射所在目录确实被清理。未新增 GUI 或更新便携包。

### Squash 后台执行器接通（2026-10-17）

新增 squash_operation::execute：在操作队列内重查弹窗快照、启动 interactive rebase，自动处理已验证 todo 与持久消息映射，结束后按真实 HEAD/operation/conflict 状态出具报告；不自动回滚。真实仓库直接调用执行器验证三类成功选择、冲突、过期 HEAD/切分支/保护 refs 变化及本地修改拒绝。源码规则、差异和测试详情见 REBASED-REFERENCE。

仍未开放 UI：本地修改保存/恢复、Continue 映射与错误反馈、Undo/焦点定位、右键消息弹窗和原生可见验收尚未完成。当前 dirty state 拒绝是接线期间限制，不改变计划中最终保存/恢复能力要求。

最终 Linux executor 十场景通过（5.89s）。Windows 原生四项 editor 集成回归通过（59.77s，包含 executor 十场景、消息映射、重放恢复和 todo 拒绝），日志 artifacts/squash-executor。仅临时仓库，无可见 UI 改动，便携包未更新。

### Squash Undo 后端接通（2026-10-17）

从成功结果生成绑定 worktree/分支/新旧 HEAD 的 Undo，重复检查 HEAD 未变化和首个重写提交未进入保护远端后执行 reset --keep；结束后读取真实状态。Linux 八场景通过，包含原提交完整顺序恢复、本地内容保留、保护分支/HEAD/分支变化拒绝及锁失败；prepare 共享保护查询回归通过。源码与暂存划分语义见 REBASED-REFERENCE。

Undo 通知按钮、成功/撤销后定位和原生 UI 验收仍待；本地修改保存/恢复、Continue 接线与 Squash 右键/消息弹窗继续按原计划推进，不勾选整体功能完成。

Windows 原生 Undo 八场景通过（55.86s），共享保护查询的两项 preparation 回归通过（12.22s），日志 artifacts/squash-undo。测试仅使用临时仓库，没有 GUI 变更或便携包更新。

### Squash 持久消息接入已有 Continue（2026-10-17）

已有 rebase Continue 后端现先尝试持久 reword 映射，未匹配继续走普通编辑器；保持取消→Git 失败→可重试/中止的原语义。真实两轮 Squash 冲突逐次解决后 Continue 成功，最终 tree/消息顺序正确；新桥接恢复、匹配失败、已消费、取消/损坏与显式中止均有测试。源码映射及一次发现并修复的取消回归见 REBASED-REFERENCE。

跨继续阶段的成功通知/Undo/focus 汇总、本地修改保存恢复、Squash 右键与消息弹窗仍待。没有将外部冲突解决测试冒充内置冲突编辑功能，也未完成原生可见 UI 验收。

Windows 原生验证全部通过：消息恢复/普通取消重试两项 14.04s，真实两轮 Squash 冲突 Continue 6.55s，原有独立 Continue/Abort 回归 8.77s；证据 artifacts/squash-continue。没有新增 GUI 截图或更新便携包。

### Squash 首次保存与恢复（2026-10-17）

已移除 tracked dirty 的临时拒绝：首次 Squash 前保存本地修改，复验后重写；完成或未启动时恢复暂存/未暂存划分。保留既有 stash，冲突时留下 owned stash 与 worktree 恢复记录。Linux 七项 Squash 集成回归通过（9.00s），新增四场景与源码/差异见 REBASED-REFERENCE。

尚未完成 Continue/Abort 后的自动恢复和残留记录处理，Squash UI/通知/Undo 定位仍待，不勾选整体完成。没有新增可见 UI 或更新便携包。

Windows 原生验证通过：本地保存/恢复四场景 25.84s，执行器十场景回归 57.30s；日志与运行脚本在 artifacts/squash-save。仅临时仓库测试，无可见 UI 改动或便携包更新。

### Squash 继续／中止恢复接线（2026-10-17）

现有 rebase Continue/Abort 后端已接入 Squash 保存内容恢复：先绑定当前 rebase 身份，成功结束后恢复 index 与工作区；错配记录或栈顶变化保留内容并给出诊断。两轮冲突继续、显式中止以及拒绝误恢复的真实仓库测试通过，详见 REBASED-REFERENCE。

剩余包括外部 Git 结束后的显式恢复入口、恢复冲突/残留记录处理、跨继续阶段 Undo/成功定位和 Squash 消息弹窗/右键入口。现有状态提示的原生可见验收仍待，不标整个 Squash 完成。

Windows release 构建通过（32.63s）。原生测试通过：编辑器继续/取消重试两项 20.34s，两轮 Squash 冲突继续与恢复 13.61s，保存/恢复六场景 43.08s，原有独立 Continue/Abort 9.84s；证据 artifacts/squash-resume-save。Linux 诊断两项亦通过。未更新便携包；现有失败提示分支未做窗口截图验收。

### Squash 残留记录接入手动 stash 恢复（2026-10-17）

已接通已有 Apply/Pop 路径：成功恢复对应 stash 后清理 Squash ownership，操作其他 stash 或失败不清理；记录损坏时允许恢复 Git 内容，单独提示记录仍需处理。Linux 十场景与普通 stash 九场景通过，源码、语义与差异见 REBASED-REFERENCE。

仍需终端恢复/删除后的显式确认、恢复冲突及损坏记录处理入口；Squash 菜单/消息弹窗/跨继续 Undo 和焦点恢复尚未完成。本轮不勾选整体功能。

Windows release 通过（32.00s）；原生保存/恢复十场景 85.08s、普通 stash 九场景 13.52s，日志 artifacts/squash-manual-restore。自有 PID18696 原生窗口点击 Apply 后，artifacts/squash-manual-restore-ui/restored-warning.png 确认中文“已恢复保存的更改；恢复记录仍需处理”及损坏记录诊断可见；脚本断言文件内容恢复、HEAD/stash 不变、index 保持未暂存和异常记录保留，测试窗口正常关闭。首次截图显示被缩放，修正原始像素点击坐标后验收通过；无滚动自动化，未操作用户窗口。没有更新便携包或宣称整体 Squash UI 已完成。

### Squash 右键与消息弹窗初次接线（2026-10-17）

Log 多选右键新增 Squash：后台准备和完整消息草稿、非空消息确认、执行前复验及真实执行器/普通编辑器回退已接线。源码与当前非模态/工具栏/隐藏规则等差异详见 REBASED-REFERENCE。Linux 完整消息顺序及取消/无效选择测试通过（0.11s）；Windows 构建与原生交互待本轮记录。

尚待 Undo/成功定位、跨 Continue 结果汇总、恢复残留入口和完整异常交互验收，不勾选整体完成。

Windows release 通过（23.63s）。原生 artifacts/squash-ui 验证：多选右键入口、预填完整消息、输入聚焦、空消息禁用/不改历史、确认后两条提交压成一条、tree 不变及工作区干净。首轮 SendKeys 被当前中文输入法消费空格，屏幕与实际提交一致；改用粘贴后消息精确断言通过。第二次自有 PID34956 等待刷新完成，completed.png 显示 Reviewed squash/base 两行，测试窗口正常关闭，未操作用户仓库/窗口。

原生发现的待办：初次 4 秒截图尚处异步刷新，15 秒等待后列表更新；没有将延迟视为性能验收。重写后原 OID 选择清空，详情区为空，必须继续接 focusCommitWhenReady 等效行为及 Undo，尚未满足完整功能交互验收。取消/保护分支变化/冲突继续等 UI 路径仍待，现有后端用例不代替窗口验收。便携包未更新。

### Squash 新提交定位与 Undo 入口（2026-10-17）

直接成功后捕获 Undo，使用新 refs 定位压缩后的范围首提交并加载默认文件预览；状态栏 Undo 复验后执行 reset --keep，再定位原范围首提交。来源和 Git fallback/通知位置/过期机制差异见 REBASED-REFERENCE。编译和原生交互验证结果待本轮补充。

剩余跨 Continue 完成汇总、过滤/跨页专项、即时入口过期、残留恢复处理和完整 UI 验收继续保留，不勾选整体完成。

Windows release 通过（21.60s）；Undo 八场景 Linux 回归 9.36s、Windows 原生回归 75.21s，增加新/旧定位目标断言。自有 PID6668 在 artifacts/squash-ui-undo/completed.png 中自动选中 Reviewed squash，右侧消息与 a.txt 默认预览正确；undone.png 显示原历史三行、选中 First edit both files、默认 Diff 恢复且 Undo 入口消失。脚本通过复制 OID 验证新旧定位，断言原 HEAD/完整 rev-list 恢复、工作区干净，并正常关闭测试窗口。未操作用户 Rebased，未做滚动自动化；15 秒等待仅为测试稳定等待，不代表性能结论。

### Squash Continue 完成结果汇总（2026-10-17）

新增 scoped origin，将本产品发起的 Squash 与普通 rebase 区分。成功 Continue 且恢复完成后返回 Undo，UI 复用新提交定位和预览；中间冲突、取消、Abort 和来源不匹配不会生成 Undo。Linux 两轮冲突四变体通过，源码与持久会话差异见 REBASED-REFERENCE。

剩余外部结束后的恢复记录管理、异常与过滤/跨页 UI 专项、入口过期和整体功能验收仍保留；未勾选整体完成。

Windows release 通过（31.01s）；原生编辑器继续/取消重试两项 16.22s、两轮 Squash 冲突及来源四变体 41.74s、独立 Continue/Abort 回归 11.09s，日志 artifacts/squash-continue-undo。上述为后端真实 Git/桥接验证，窗口交互结果另记。

原生 UI 验证 artifacts/squash-continue-undo-ui：自有 PID14896 从 Log 选择 first/last 发起 Squash，发生首轮冲突；正常关闭后以 PID21088 重新启动。测试脚本在隔离 fixture 外部解决/暂存 shared，应用两次 Continue 与普通消息编辑确认后完成，completed-after-restart.png 自动选中非 HEAD 的 squashed、显示详情/默认 Diff/Undo。点击 Undo 后 undone-after-restart.png 选中原 first，脚本断言原 HEAD/完整 rev-list 恢复、工作区干净且 rebase 目录清理，测试窗口正常关闭。没有内置冲突编辑器的完成声明。

自动化两次因窗口遮挡/前台保护拒绝操作，未向其他窗口发送点击或按键；最终定位用截图核对，Undo 历史用 Git 断言，不把被拦截的剪贴板检查计为通过。用户 Rebased/IDEA 未关闭，未进行滚动自动化。便携包未更新。

共享 Undo capture 重构后的直接成功八场景 Linux 回归也通过（9.24s）。

### Squash 确认时复验与提示修正（2026-10-17）

已接异步只读复验，失败不关闭消息弹窗，取消会取消查询；执行器仍在队列内重新验证。按用户反馈改为 Rebased 式按钮上方单条本地化错误、禁用确定，移除额外说明和英文底层诊断。源码与差异见 REBASED-REFERENCE 对应小节。Linux 新增快照复验真实仓库测试通过（0.90s），覆盖同 HEAD 切分支、发布至保护分支、进行中操作、取消、HEAD 变化以及 index/工作区不变；执行器十场景回归通过（7.52s）。修正后 Windows 构建与截图验证待补充，不将整个 Squash 验收标完成。

修正后 Windows release 通过（31.18s）。artifacts/squash-confirmation-ui/stale-error.png 显示单条中文校验提示、禁用确定和可编辑草稿；隔离 fixture 原生验证 HEAD 变化后没有重写历史，剪贴板精确断言 Keep this draft 保留，Esc 取消后 HEAD 不变，测试窗口 PID10864 正常关闭。上一轮脚本误将先全选后点击造成的追加文本当作丢稿，已修正为点击后全选粘贴并重跑，不将误报作为产品失败或通过证据。仍待：检查进行中取消的专门竞态验收、其他失败提示和安装版同条件对照。

同一修正版原生正常流程回归通过：空消息不能确认，输入 Reviewed squash 后异步校验并压缩成功，提交数为 2、tree 与原 HEAD 相同、工作区干净；证据 artifacts/squash-confirmation-ui/assertions.json 与 completed.png。自有 PID20984 已正常关闭；未操作用户 Rebased/IDEA。

### Log 父／子导航前置查询（2026-10-17）

已按 GoToParentOrChildAction/VisibleGraphImpl 读取可见图邻接规则，新增 history::adjacent，复用 filtered_graph 投影，保留过滤范围、完整 OID 与固定 roots。不能直接使用 Commit.parents，否则隐藏节点和多父／多子选择会偏离 Rebased。五项投影测试和真实仓库导航测试通过（0.22s）；未加载文件正文。下一步接单选菜单启用状态、单目标定位、多目标选择 popup 与键盘焦点行为，随后 Windows 原生验证；此处仅数据层完成，菜单功能仍待接通。

### Log 父／子导航接入菜单和键盘（2026-10-17）

右键菜单增加转到父提交／子提交，History 焦点下 Right/Left 对应相同动作；单目标直接定位，多目标弹出候选，跟随 Log 选择详情与默认文件预览。查询和弹窗选择都核对代次/OID，切换模型取消查询。Windows release 通过（20.78s）；原生验证进行中。保留差异和剩余验收见 REBASED-REFERENCE：无目标菜单预先禁用、数字选择与键盘锚点、大图预算/延迟、跨页与过滤 UI 尚待，不能标整个导航完成。

原生隔离 merge/fork fixture 验证通过（artifacts/log-adjacent-ui）：parent-popup.png 显示 Main/Side 两个父候选，点击首项后复制 OID 精确匹配 main 父；Left 直接返回 merge，复制 OID 匹配原 HEAD；base 上 Left 显示两个子候选（child-popup.png），Esc 后复制 OID 仍为 base。HEAD 和工作区未变化，assertions.json 已记录，自有 PID27728 正常关闭。未进行滚动自动化，未操作用户 Rebased/IDEA；右键入口的独立点击及复杂过滤/跨页仍待验收。

### 导航补充验收与 Compare Versions 前置（2026-10-17）

导航数字1–9/0、默认首项及键盘锚点已接入；原生数字2、Enter、右键单子跳转和多选禁用通过，证据 artifacts/log-adjacent-ui。源码与剩余差异见 REBASED-REFERENCE。测试仅隔离仓库，自有 PID14596 收尾关闭，不操作用户窗口。

Compare Versions 已核对上游完整入口与承载形式：两条同仓库记录、下→上两个版本直接比较，默认 Changes 文件浏览器及预览；不要用 Log 累计变更或 merge-base 替代。新增 revision_changes 返回固定端点，真实分叉仓库测试通过（0.22s）。下一步接两选菜单、Changes浏览器、默认文件选择与完整Diff导航，之后Windows原生验收；目前仅数据层可用，不勾选完整功能。

### Compare Versions 可见入口与 Changes 侧栏（2026-10-17）

已接双选右键“比较版本”、独占左侧 Changes 文件树、默认首文件、刷新/关闭及 Enter/双击/Show Diff 完整文件查看。读取 ChangesBrowserToolWindow 后明确默认 LEFT，采用现有独占侧栏；固定比较身份独立于Log选择，正文通过已有DiffTab按需加载。源码和预览Tab等保留差异见 REBASED-REFERENCE。Windows构建及初次主/侧分支窗口验证已进行，最终修正版证据待补。整体 Compare Versions 仍不勾选完成。

修正版 Windows release 通过（29.97s）。原生 artifacts/compare-versions-ui：Main/Side 两选菜单可见比较版本，comparison-ready.png 默认 a.txt；first-diff-ready.png 显示 side→main 的 a.txt 删除行；second-diff-ready.png 显示 b.txt 新增行；refreshed-selection.png 保留 b.txt 并显示整行选择，closed-comparison.png 关闭左侧后已打开固定 Diff 仍正确。脚本断言原 HEAD/工作区不变（assertions.json）；比较内容及选择以截图核对，不把 expectedFiles 字段当自动内容断言。测试 PID36064/17608 均正常关闭。多目录、空比较、错误、预览Tab联动和完整键盘/生命周期验收继续保留；便携包未更新。

### Compare Versions 预览 Tab 身份与返回（2026-10-17）

已按 EditorTabDiffPreview/TreeHandlerEditorDiffPreview 接线同一比较窗口复用一个 Diff Tab、Escape 返回文件树，以及关闭/替换比较窗口释放所属预览；具体源码、实现位置和保留差异见 REBASED-REFERENCE。Windows release 构建通过（21.62s），原生行为验证待补。树选择自动联动、Diff 导航同步、复杂目录/错误/取消验收继续保留，未勾选整体完成。

原生验证通过（artifacts/compare-versions-ui/preview.ps1）：preview-reused.png 确认 a→b 显式打开仅保留一个比较 Tab；Escape 后 Up/Enter 打开 a.txt，preview-reopened-first.png 确认树选择及完整比较内容正确；preview-owner-closed.png 确认关闭比较窗口释放所属 Tab 并保留 Log 多选。preview-readonly.json 记录 HEAD/工作区未变；UI 行为按截图核对，不冒充自动像素断言。自有 PID30864 正常关闭，未操作用户 Rebased/IDEA。替换比较窗口、保留其他来源固定 Tab 的组合专项尚待；没有更新便携包或宣称完整交互/性能验收通过。

### Compare Versions 文件树与预览联动（2026-10-17）

按 TreeHandlerChangesTreeTracker/TreeHandlerDiffRequestProcessor 接入双向选择：树单击/上下键更新已打开预览且保留焦点；预览未打开不新增 Tab，非活动预览不加载正文；Diff 前后文件/选择器同步树选中与祖先展开。来源、实现和即时更新/防抖等差异见 REBASED-REFERENCE。构建及原生截图验证待补；刷新异常、复杂目录、快速切换与完整功能验收继续保留。

Windows release 通过（21.41s）；原生 artifacts/compare-versions-ui/linked-preview.ps1 验证单击 b.txt 自动更新完整 Diff，Up 回到 a.txt 且树焦点保留（linked-key-up.png）；Diff 下一文件同步树选中 b.txt（linked-next-ready.png）；Escape 后 Up 只改变树选择、不重开预览（linked-remains-closed.png），Enter 可重新打开。脚本检查原 HEAD/工作区不变（linked-readonly.json），自有 PID28844 正常关闭。截图核对行为，不把 expectedFiles 当内容自动断言。复杂目录展开、文件选择器反向联动、非活动 Tab 延迟读取和快速取消仍需专项验证；便携包未更新，整体计划未完成。

### Compare Versions 刷新失败不保留旧正文（2026-10-17）

已对照 LoadingChangesPanel/CompareWithLocalDialog 接入模型刷新通知：失败清理关联 Diff viewer，禁用文件导航并隐藏旧标题；成功刷新重载已有预览，保持 Tab 身份。比较面板显示中文摘要与复制错误详情，复用刷新重试。源码/呈现差异见 REBASED-REFERENCE。Windows 构建及故障恢复原生验证待补；快速连续刷新、空结果及非活动 Tab 组合验收仍待。

修正版 Windows release 通过（21.99s）。原生 artifacts/compare-versions-ui/error-preview.ps1 在专用 fixture 中临时移走 .git，刷新后 error-ready.png 显示中文摘要/复制详情和清空后的原预览 Tab，旧标题不可见、文件导航禁用。还原元数据并刷新，error-recovered.png 同一 Tab 恢复完整 a.txt 比较。复制详情断言包含固定 side OID；初次脚本误要求错误必含 git/repository，实际为 Could not access <OID>，修正断言后通过，不将脚本误报记作产品缺陷。所有尝试均通过 finally 恢复 .git；最终 HEAD/工作区不变（error-readonly.json），自有 PID9876 正常关闭。未操作用户仓库/Rebased，未更新便携包；整体错误处理、本地化及性能验收仍未完成。

### 当前审阅功能版本核心回归与内存基线（2026-10-17）

因 Commit/Project/Squash/Compare 等新增功能后尚无统一当前构建证据，本轮运行 no-default-features 全套：338 项报告通过（156 库、4 AskPass、33 commit_index、14 editor、8 fetch_auth、7 operation_process、4 process_tree、6 project_files、106 repository；repository 11.67s）。日志 artifacts/current-review-regression/linux.txt。该配置不包含桌面 Diff 模型测试，不等同 Windows 全量回归或 UI 验收。

当前 release SHA256 `90905E1E289889F1BA14AC79ED1AAAC9DC8F17B5FC5B6E1ED2468B2088D0E0B1`，Windows 从挂起启动即加入 Job Object，稳定态各采样 10 次：空窗 Private Bytes 54.32–54.34MiB、进程组峰值 56.93MiB；10000 历史 + 975000 字节 review.rs 全文 Diff，Private Bytes 95.73MiB、峰值 152.34MiB，末态 1 进程、累计 73 进程。原生 history.png 确认已加载10000、window.png 确认目标全文 Diff；宿主 Git cat-file 复核正文大小。满足本场景空窗80/稳定150/峰值250MiB预算。

GPU 单独报告：空窗 Dedicated18210816/Shared1146880/TotalCommitted29044736 字节；大Diff Dedicated61403136/Shared4096000/TotalCommitted66420736 字节，不与进程峰值相加。检测到另一 gitui-review PID16304，未操作也未计入专用 Job；自有 PID7188/29380 已退出。基准 HEAD/工作区保持。证据 welcome/run 的 memory.json、steady.json 及 memory.ps1。该采样不是冷启动时延、长期无增长或新增写操作峰值验收；比较预览循环另记。

同一 release 新增比较预览专项：四提交、两个小文本文件，固定 side→main 比较，20轮 b→a（40次实际文件读取），每次要求 Job 新子进程出现并回到仅应用1进程后采样，无滚动自动化。原生 after-cycles.png 确认最终 a.txt 内容及单个复用Tab，closed.png 确认关闭比较释放Tab。基线71.51MiB，20轮71.62–72.11MiB，末轮72.07MiB；关闭后10次72.00–72.11MiB。短循环未见持续增长，不推断长期无泄漏或大文件峰值。含全部子进程的Job峰值140.31MiB、累计349进程；GPU单列Dedicated59727872/Shared1859584/TotalCommitted62377984字节。fixture HEAD/工作区未变，自有PID18604已退出。证据 artifacts/current-review-regression/comparison 下 cycles/memory/steady.json 与截图、comparison-memory.ps1/scenario脚本。全部本轮新脚本只操作验证进程，用户已有PID16304保持运行；未更新便携包，整体性能/发布门槛仍未全部验收。

### 当前功能便携预览包同步（2026-10-17）

README 当前流程及 docs/WINDOWS-PREVIEW.txt 更新独占工具窗口、只读 Project、文件级 Commit/Amend/Commit and Push、Log 多选/Squash/Undo、Compare Versions 直接树比较及复用预览交互，保留外部冲突解决与恢复记录限制。移除“没有独立提交”的过时说明；未宣称任意 hunk 暂存或完整冲突编辑器已实现。

生成新归档 dist/GitReview-windows-x64-preview-20261017-review.zip（17808540 字节），保留旧包。799 个归档项、501 个锁定依赖记录，CRC 和三个 exe SHA256 均检查通过；主程序 SHA256 90905e1e289889f1ba14ac79ed1aaac9dc8f17b5fc5b6e1ed2468b2088d0e0b1，与上轮内存实测构建相同。Windows Expand-Archive 到全新目录，三个 exe 哈希及包内使用说明逐一匹配，原生启动已成功；比较预览交互验证进行中。这里只同步开发预览，不视为正式发布/干净系统/全部功能验收。

解压包原生交互复验通过：artifacts/package-review 的 linked-b-ready、linked-a-ready、linked-next-ready 确认树单击/键盘选择及 Diff 导航联动，linked-remains-closed 确认 Escape 返回后选择不重开Tab，Enter可重开。linked-readonly.json 检查原HEAD及工作区不变。自有PID36688正常关闭，未影响用户已有窗口。证据 verification.txt/startup.json/原生截图；此轮未重复执行无代码变更的核心测试，也未把新目录解压等同干净Windows环境验收。

### 外部恢复责任记录的显式确认基础（2026-10-17）

新增 squash_saved::pending/acknowledge_external_recovery，支持用户确认已在外部处理后仅清理应用恢复记录。先绑定有界原始字节及worktree，执行队列内拒绝进行中操作/冲突、过期记录、跨仓库确认与取消；不改Git数据、不自动确认、不重复应用stash。上游与本地持久化差异见 REBASED-REFERENCE。Linux真实Git六场景通过（1.08s），HEAD/stash/index/工作内容逐项不变。Windows对应测试构建/运行待补；界面入口和确认交互尚未接入，外部恢复闭环未完成，不更新便携包。

Windows 对应六场景真实 Git 验证通过（1项测试函数、10.53s），构建清单与运行日志在 artifacts/squash-external-recovery。尚未覆盖 linked worktree、未合并index及非普通/超限记录专项；当前仅后端API，未新增GUI，没有将后端通过视为用户可见恢复入口交付。下一步接入已保存更改面板的显式确认和状态刷新。

### 已保存更改侧栏的外部恢复确认（2026-10-17）

StashView 后台检查待恢复记录并提供“已在外部处理…”；确认窗口说明只清理应用记录、保留stash/文件，默认取消，Enter不直接清理。显式确认后复用公共后台写队列及精确记录复验，结束刷新侧栏与本地化结果。记录读取失败显示摘要/复制详情。来源与本地持久化差异见 REBASED-REFERENCE；Windows初版可见入口已检查，发现新增提示挤压底部Apply/Pop，正在修复剩余高度约束。取消、过期记录和确认三路径待最终构建原生验收，未勾选完整恢复闭环。

外部恢复确认最终原生证据已核对：`artifacts/squash-recovery-confirm-ui/verify.ps1` 与 `assertions.json` 确认 Enter 取消不清理、确认期间记录改变会拒绝、重新确认只清除应用记录；HEAD、stash、index/工作区均不变。completed.png 显示恢复提示已消失、stash 仍可浏览、底部 Apply/Pop 保持可见，修复了提示占高挤出动作区的问题。测试只关闭自有进程；首次前台保护拒绝发送按键不计为通过。该记录补充上一节的待验收描述，不代表整个 Squash/恢复矩阵或像素还原完成，便携包尚未同步此增量。

### 外部恢复确认的 worktree 与冲突边界（2026-10-17）

继续核对 GitStashChangesSaver::notifyUnresolvedRemain 的未解决冲突保留与后续动作规则；上游没有同名磁盘记录清理动作，本地适配边界未变。本轮新增 tests/repository.rs 三项真实仓库回归：同 common-dir 的两个 worktree 即使记录字节相同也拒绝交叉确认，仅清理正确 worktree；真实 stash apply 产生未合并 index、但无进行中 merge 元数据时仍拒绝清理并保持 refs/index/冲突正文；8KiB 边界、超限、目录替换、记录消失和 Unix 符号链接均保持原数据。既有六场景一并运行，Linux 四项测试通过（1.18s）。Windows 对应结果另补。

Windows xwin 测试构建通过（2.69s），宿主 Git 对应四项全部通过（11.73s），证据 artifacts/squash-external-recovery/windows.txt 和 run.ps1；Unix 符号链接分支不计入 Windows 覆盖，Windows reparse-point 专项仍待。此轮只新增后端边界测试和证据记录，无新增可见界面，不重复运行 UI 或内存采样；前述截图仅作为上一轮原生验证的补记。完整恢复验收、英文确认交互及跨进程最终检查/删除之间的竞态限制继续保留。

### 独立 Diff 错误页与重新加载（2026-10-17）

按 ErrorDiffTool/ErrorDiffRequest/CacheDiffRequestProcessor 接入 src/diff_tab.rs：失败显示居中本地化摘要、重新加载和复制详情，不铺开原始 Git 输出；释放失败 viewer、禁用差异块跳转并隐藏版本标题。重试固定原 Target，不跟随后台 Log 选择；既有代次/取消隔离保留。Diff 偏好保存失败改为摘要+复制详情。来源与适配差异见 REBASED-REFERENCE，英文资源同步。

Windows release 通过（35.77s）。artifacts/diff-error-ui 的专用仓库中，先打开 merge 的 a.txt 独立 Tab，再暂移 .git 触发读取失败，finally 还原元数据；error-ready.png 确认居中中文摘要、重载及复制详情、正文/标题清空。retry.ps1 复制诊断断言包含 not a git repository，重新加载后 recovered.png 恢复相同版本的完整 a.txt；HEAD/工作区未变（assertions.json），自有 PID30140 已退出。首次切回 Log 导致旧错误条改变 Tab 坐标，修正定位重跑，不将第一次未点中目标算通过。未操作用户仓库/窗口，无滚动自动化。

发现并保留明确后续项：旧 Log 底部预览与顶部全局错误仍由 Review 独立渲染原始 Git 文本，本轮尚未统一；DiffTab 嵌入模式/偏好保存失败/英文/键盘重试需专项原生核对。本轮不宣称全局错误交互、像素或整体计划完成，未更新便携包。

### Log 底部预览与全局错误提示分离（2026-10-17）

src/ui.rs 新增 preview_error，将文件读取错误与全局操作错误区分。底部 Diff 采用摘要/重载/复制详情，隐藏失败版本标题并禁用差异导航；重试只读取选中文件，不自动触发写操作。全局 error 从顶部原文行移至右下状态栏，提供摘要、复制详情与关闭，不再撑动 Tab/工具栏。全局加载失败的底部空态提供只读刷新。偏好保存失败同样改用简短摘要。来源与暂用状态栏代替上游 sticky balloon 的差异见 REBASED-REFERENCE。

修正编译括号错误后 Windows release 24.96s 通过。artifacts/log-error-ui 原生隔离 fixture：移走 .git 后单击已加载文件触发 error-ready.png，底部短摘要可复制诊断，恢复 .git 后点击重新加载得到 recovered.png，同一 b.txt/原比较端点恢复；随后全局刷新故障在 global-error-ready.png 显示右下操作失败，顶部高度不变，复制详情断言通过，点击刷新后 global-recovered.png 恢复原提交/文件。finally 始终还原 .git；HEAD/工作区不变，自有 PID37764 正常关闭。未操作用户窗口，无滚动自动化。截图发现刷新按钮缺中文，已补集中资源，最终构建另记；英文、键盘、窄窗多状态叠加及通知浮层尚待，未更新便携包。

中文 Refresh 资源补齐后的首轮构建被终止（退出码143，无编译诊断），确认进程已结束后重跑，最终 Windows release 24.78s 通过。本轮原生截图为补文案前的同逻辑构建；最终中文按钮截图仍待补，不把构建通过当语言全矩阵验收。

### Project 大目录分页读取层（2026-10-17）

原 children 对超过4096项目录整页报错，现新增有界 children_page：每页最多256项，目录优先/自然排序，游标绑定目录与版本；重新扫描当前层选择下一页，内存不随目录总项数增长，不引入全仓索引。目录变化、跨根/跨目录及取消明确失败，调用方需刷新而非拼接不同版本。源码对照及PSI/文件系统适配差异见 REBASED-REFERENCE。

Linux 8项 Project 真实文件测试通过（1.30s）：新增4113个Unicode文件+两个目录的完整多页顺序/无重无漏断言，空目录、游标作用域、显式目录时间变更及取消；保留只读正文/边界/链接/旧API超限等覆盖。Windows原生结果另补。当前仅数据层完成，Project树仍使用旧children API，下一步接可见分页/键盘入口/刷新及节点总预算；本轮没有可见UI改动，不宣称用户已经能浏览大目录，也不更新便携包。

Windows 最终测试构建1.83s通过，8项原生测试全过（8.45s），含显式修改目录时间戳的过期游标检查和真实junction拒绝。日志/运行脚本在 artifacts/project-pagination。测试耗时包含fixture创建与清理，不作为分页响应时延或产品内存验收；运行进程已正常结束，没有启动或操作用户GUI。

### Project 文件树接入分页（2026-10-17）

src/project_view.rs 已改用 children_page，每目录保存游标，末尾“加载更多文件”可单击/Enter/Right；加载后选中新页首文件，Home/End定位已加载树首末项。折叠释放子树与游标、取消请求，刷新根重置；检测到目录变化清除该目录旧分页并提示刷新。总16384节点预算保留，预算不足不追加半页。源码规则及本地分页适配差异见 REBASED-REFERENCE，中文资源同步。

Windows release 34.96s通过，artifacts/project-pagination-ui 专用4202文件fixture已确认首批显示、End定位加载更多、Enter追加第二页、首项file0254.txt被选中并可Enter打开只读全文。没有进行滚轮/滚动性能自动化；这些按键用于选择和动作入口验证。目录外部变化/刷新/折叠/鼠标路径结果待补，整体Project功能与大目录性能仍未勾选完成，便携包未更新。

原生后续路径通过：verify.ps1 在独立fixture外部新增文件后再次加载，stale-result.png 清除旧目录分页并显示中文刷新提示，中央已打开file0254只读Tab保持；刷新后 expanded.png 显示新增文件，Home/Left 折叠、Right展开恢复首批，鼠标点击更多后 mouse-loaded.png 选中新页首项file0253（新增文件占用一个排序位置，符合预期）。assertions.json 检查HEAD、受跟踪工作区、index及预览文件原文不变；外部测试新增文件已清理，自有PID36292正常关闭。未操作用户窗口。截图人工核对UI行为，脚本不冒充像素断言；嵌套目录分页、预算耗尽与执行中折叠的竞态专项、大目录响应/Job内存仍待验收。

### Project 分页的 Windows 进程组内存（2026-10-17）

同一 release SHA256 `52C657915899EAEE25AD4B577F3466C8F0D5017B1B7CF1A508B86413865878E7`，专用单提交仓库4202个文件，三个完整17页加载→打开末尾file4199→关闭Tab→折叠根目录循环。每轮复制只读正文精确断言 preview 4199，确认实际加载到末页；截图 loaded-1/2/3 与 collapsed-3 证明文件身份及折叠后无文件Tab。键盘仅用于加载动作/选中目标，无滚轮或滚动性能自动化。fixture HEAD和状态保持，readonly.json记录；用户已有PID16304未操作。

从挂起启动加入Windows Job，包含所有子进程：首批Project基线65.73MiB；三轮完整加载Private Bytes分别79.82/79.43/79.52MiB，折叠后78.27/77.94/77.87MiB；最后10次稳定采样77.97–78.02MiB，Working Set81.92–81.97MiB，子进程稳定态0。Job峰值132.61MiB，累计46进程，末态仅应用。GPU独立报告Dedicated58765312/Shared2023424/TotalCommitted61710336字节，不与Job峰值相加。证据 artifacts/project-pagination-memory/run2 的cycles/steady/memory.json及原生截图，binary.json和运行脚本保留。

三轮未见持续增长，但折叠后比初次基线仍高约12MiB；本轮只证明该小文件/单目录规模短循环表现，不宣称长期无泄漏、16384节点预算极限或整个性能门槛完成。脚本含显式等待和截图，17秒/轮不是实际目录读取延迟。首轮run在完成首个末文件检查后被前台保护中断，未完成内存报告，不计验收；测试进程已由其专用Job清理。修正归属核验后的聚焦，在run2三轮通过；自有PID4404正常退出，已确认两次测试PID均不再存在。便携包未更新。

### Project 分页与恢复/错误提示预览包同步（2026-10-17）

README 当前包链接及 docs/WINDOWS-PREVIEW.txt 更新：Project每页256项/总16384节点/目录变化刷新，Diff错误重载/复制诊断和右下操作错误，以及Squash“已在外部处理”只清应用记录/保留stash文件的明确确认。保留非普通/超限恢复记录需外部检查及未完成发布验收的边界。

新归档 dist/GitReview-windows-x64-preview-20261017-project-recovery.zip（17845658字节），保留旧包。799项、501依赖记录，CRC及三个exe哈希校验通过；主程序SHA256 `52c657915899eaee25ad4b577f3466c8f0d5017b1b7cf1a508b86413865878e7` 与Project三轮内存构建一致。Windows解压到全新验证目录，核对三个exe和使用说明后原生启动；artifacts/package-project-recovery 的more-ready/second-page/file-ready截图确认分页、选择新页首项和只读全文。assertions.json检查HEAD/index/受跟踪工作区及目标文件不变，测试PID21568正常关闭，用户窗口未操作。

这次验证可解压分发的当前开发预览，不等同干净Windows系统或完整发布验收。恢复与错误交互沿用同一SHA构建此前的专项证据，本轮没有重复未改代码的全套测试/内存采样；英文、键盘边界、嵌套目录预算、长期资源释放及逐像素一致性仍按原计划继续。

### 计划当前状态入口整理（2026-10-17）

新增文首“当前状态与剩余主线”，区分现有实现、完整验收缺口、首版后功能和当前可手测包；保留全部历史记录及未完成验收要求，不因局部通过批量勾选。核对Project/Diff设置/Commit恢复代码及近期分页、恢复、错误和交付证据后，修正“首版不加入写操作”和“语法高亮/字符强调排除”的过时表述。后续入口明确为Project预览/固定Tab及生命周期对齐，再收尾跨入口审阅与错误/本地化、剩余Git恢复矩阵和发布门槛。本轮仅文档校正，未修改二进制，不重复运行原生或内存验证。

### Project 普通文件 Tab 排列与关闭（2026-10-17）

对照 EditorWindow 的 addComposite/computeIndexToSelect 和 UISettingsState 默认值，确认可选预览默认关闭。本轮保留文件单击选择、双击/Enter打开普通Tab；src/ui.rs 新文件插在当前文件Tab之后（Log激活时插文件组首部），关闭活动文件回左邻，关闭非活动文件仅修正索引、不重读活动正文。左邻按需读取，关闭文件仍取消任务/释放正文。源码路径、规则及与previousSelection/混合Tab组的剩余差异见 REBASED-REFERENCE；本轮不代表可选预览和完整Tab生命周期已经完成。

Windows release 构建22.89s通过，SHA256 `63da5f68f32f2bf4bb8e0ff5335b503e866c5b0dcac5bd0bae26b86c05f7b6bd`。原生 artifacts/project-tab-order：依次打开a/b、回a再打开file0000，inserted.png确认a/file0000/b顺序；关闭file0000后复制正文精确为base a；重新打开file0000后关闭非活动a，复制仍为preview 0（inactive-closed.png），后续剩余b正文为base b，all-closed.png确认最终回Log。assertions.json检查HEAD、index、受跟踪及预览文件未改；只关闭自有PID36596，启动会话正常退出。首次检查早于startup.json生成，等待原进程就绪后继续，无重复启动。未做滚动自动化、重复内存采样或便携包更新。

功能阶段核对：保持原Tab入口/横向布局和关闭图标；活动关闭后正文与焦点可复制验证，非活动关闭保留当前内容；仍按需激活/释放。待验收/实现：跨Log/Diff/File统一顺序与previousSelection、可选预览模式、Esc行为及视口恢复、英文/窄窗口；未宣称像素还原或整个Project验收完成。

### 普通文件 Esc 与 Ctrl+F4（2026-10-17）

按 EscapeAction/DefaultDiffFileEditorCustomizer 区分普通文件和Diff：src/ui.rs 的Back不再关闭普通文件，src/project_view.rs 只在正文聚焦时清除文本选择；无选择时保留文件/焦点。按默认keymap及CloseTarget加入ProjectFileTab上下文内Ctrl+F4，复用已有文件关闭及左邻激活路径。Diff的Esc未改。来源和多光标差异详见 REBASED-REFERENCE。

Windows release 22.92s通过，SHA256 `b7bcd87a2023b0fad9b050ce5d90e53ab28285daa965dabd389f28b4607bbd39`。artifacts/project-file-escape/verify.ps1 原生键盘验证：全选后Esc，再Copy保持剪贴板哨兵，证明选择已清除；第二次Esc后Ctrl+A/C精确得到base a，证明Tab与焦点保留；打开b后Ctrl+F4返回a且复制base a，最后Ctrl+F4回Log。截图escape-selected/retained/left-active/log-return及assertions.json记录结果，HEAD/index/受跟踪工作区保持，测试PID34688关闭。未操作用户窗口，无滚动自动化。

功能阶段核对：不新增布局/文案，沿用已有关闭入口；普通正文选择、无选择、关闭左邻/末页键盘路径已核对。未覆盖输入法/多光标/菜单叠加、英文与窄窗专项，不宣称所有焦点情况完成；可选预览模式、混合Tab组previousSelection和视口恢复仍待推进。未更新便携包或重复性能采样。

### 文件与 Diff 的统一 Tab 顺序及跨类型关闭（2026-10-17）

进一步读TabLabel::mousePressed/mouseReleased，确认previousSelection是鼠标操作期间临时状态而非永久打开来源。落实默认左邻规则需要统一Tab顺序，不能按文件/Diff两组分别排序。src/ui.rs新增仅存EntityId的editor_tab_order，Project和Log/Commit/Stash/Compare新建Tab统一插在当前Tab之后；重复打开不重排。标题栏关闭活动文件/Diff返回实际左邻，可跨类型；关闭非活动Tab保持当前正文/焦点；程序关闭、Compare销毁和切仓同步移除身份。Diff Esc继续走来源面板返回路径。源码依据与尚存差异见REBASED-REFERENCE。

Windows release 22.60s通过，SHA256 `0362caad1e7411594fb68be83baddf3fbd328f53fc2f35b7ede96e78d7d904a6`。原生artifacts/mixed-editor-tabs：mixed-ready.png确认Log/Diff/文件顺序，关闭文件后returned-diff.png返回左侧Diff；关闭非活动Diff保留文件焦点并精确复制base a；reverse-order.png确认从文件打开新Diff得到Log/文件/Diff，关闭Diff后复制base a，returned-file.png确认回文件；Diff Esc回Log并保留普通文件；再打开Diff、关闭非活动文件，diff-remains.png保持当前Diff，最终关闭回Log。assertions.json确认HEAD/index/受跟踪内容不变，自有PID27020正常退出，未操作用户窗口或进行滚动自动化。

功能阶段已核对默认混合顺序、双向标题关闭、非活动关闭、文件Ctrl+F4及Diff Esc基本路径；代码覆盖各创建入口，但本轮原生仅验证Project和Log组合，Commit/Stash/Compare混合、切仓、英文/窄窗仍须专项核对。保留按需加载及关闭释放，不增加正文缓存；未重复内存采样、未更新便携包。可选预览模式、光标/视口恢复和鼠标按下释放间临时previousSelection行为尚未完成。

### 普通文件的轻量阅读状态恢复（2026-10-17）

对照TextEditorProvider::getStateImpl/setStateImpl及TextEditorState，src/project_view.rs在非活动正文释放前保存单个活动选择的逻辑行列、方向与视口偏移；再次激活/重新加载时读取新正文并恢复，越界行列由现有Rope转换限制。vendor/gpui-base新增set_selected_range_with_direction，沿用UTF-8边界限制并保留活动端。保存状态没有正文副本或Editor实体引用，失败保留位置供重试。源码与GPUI视口偏移/上游相对光标距离的差异见REBASED-REFERENCE。

Windows release（含vendor重编译）1m27s通过，SHA256 `5d913000c530867992ac533216fcd56401beb900ff7cf891f62c7b195b22c630`。artifacts/project-reading-position原生fixture：a第二行“中文🌿 end”反向选择，打开b后切回a复制内容一致；Shift+Right后复制精确为“文🌿 end”，证明选择活动端恢复；点击重新加载仍保留该选择。非活动时外部将a缩短为x换行，再切回时选择收敛到有效位置，Copy保留空选择哨兵，Ctrl+A/C精确为x。finally恢复原字节并重载；HEAD/index/受跟踪内容不变。截图restored/direction-restored/reload-preserved/shortened/original-restored与assertions.json记录证据，测试PID22788正常关闭。

功能阶段已核对切换、反向Unicode选择、同文件重载与外部缩短边界；未执行滚动自动化，视口偏移恢复的滚动体验留手测，DPI/字体变化相对位置、多光标、失败后重试及切仓组合仍待验收。未更新便携包或重复内存采样，不将本轮单选择恢复描述为完整编辑器状态还原。

### 可选文件预览 Tab 与固定打开（2026-10-17）

对照AutoScrollToSourceHandler/TaskManager、ProjectViewImpl、EditorWindow预览生命周期、EditorTabbedContainer双击及EditorsSplitters斜体规则，加入设置“编辑器标签页→启用预览标签页”，默认false，中英文并持久化。开启后Project单击预览、键盘选择100ms合并且检查焦点；只保留一个可替换预览，不抢树焦点，Enter/文件双击/Tab双击转普通。已有普通Tab不降级；预览替换释放旧任务与正文。关闭开关将现有预览转普通并保留文件。源码与少数排列/类型差异见REBASED-REFERENCE。

最终Windows release 24.05s通过（首轮32.46s后补Home/折叠取消旧预览请求再构建），SHA256 `c5afcbb3dc6de4a90b2a29be9393e9f3e08d435ece63fa077b49236cae756bf5`。artifacts/project-preview-tabs：default-no-preview确认默认单击不打开；中文设置开启并保存true，a-preview-ready/b-preview-ready确认键盘下移复用斜体预览且树焦点保留；Enter固定b并复制base b，继续浏览file0000/file0001只替换预览，b保持普通Tab。首次用WM_LBUTTONDBLCLK模拟未触发GPUI内部连续点击计数，promote.ps1截图不作为双击通过证据。

关闭测试PID18656后用同一profile重启（PID9296），仅将测试profile语言改为en；restart/persisted-preview确认开关读取生效。restart-verify.ps1使用两组连续按下/释放验证：Tab双击固定a、文件树双击固定b，fixed-and-preview保留a/b和单个斜体file0000；英文设置入口可用，关闭开关保存false，disabled-keeps-content确认file0000变普通、选择file0001不替换中央正文。HEAD/index/受跟踪内容不变，restart/assertions.json记录；两个测试进程均正常关闭，用户窗口未操作，无滚动自动化。

功能阶段核对：设置承载形式沿用现有弹窗并新增真实Editor Tabs分类；默认、开启、单击、键盘、Enter、双击、普通/斜体、关闭开关和重启中英文路径已核对。待补：快速键盘/离焦取消竞态、切仓与加载失败组合、旧预览非活动时明确打开新文件的精确插入位置、设置保存失败、目录分页/预算交互及本功能内存证据。未宣称完整Project或像素验收；便携包仍未同步本轮及近期Tab增量。

### 非活动预览的明确打开位置与快速按键（2026-10-17）

按EditorWindow先插入再trim规则修正src/ui.rs::open_project_file：明确打开新文件不沿用远处的旧预览位置，先插入当前Tab后再清理旧预览；普通预览替换仍保留槽位。Windows release22.87s通过，SHA256 `ca4d299f5e3089177e6bfcd7e0d8e9211c7b6532d2f122ed5ce17c3ceb435478`。

artifacts/project-preview-order：构造普通b/file0000及非活动预览a，回b后连续投递End/Enter；explicit-order.png确认b/file0001/file0000，新file0001为普通Tab，旧a已移除，复制正文精确为preview 1。回b后连续End/Home，cancelled-by-home.png确认根目录被选中、b正文仍在，延迟预览未覆盖。按键用于选择和预览请求，不是滚动自动化。HEAD/index/受跟踪内容保持，自有PID11000正常关闭。该证据覆盖Enter取消和Home取消，不推断所有失焦/切仓/慢I/O竞态完成。


### 文件 Tab 增量预览包同步（2026-10-17）

README及包内使用说明更新可选文件预览、Enter/双击转普通、文件Esc/Ctrl+F4、混合Tab排列和活动选择恢复；滚动/缩放及完整性能限制继续明确。生成 dist/GitReview-windows-x64-preview-20261017-file-tabs.zip（17869390字节，799项、501依赖记录），保留旧包，CRC与manifest校验通过。主程序SHA256 `ca4d299f5e3089177e6bfcd7e0d8e9211c7b6532d2f122ed5ce17c3ceb435478`，与本轮排列/取消原生验证相同。

Windows解压到新目录，artifacts/package-file-tabs/verification.txt确认三个exe哈希与包内README逐一匹配；从解压程序启动，单击a产生预览、Enter转普通且Ctrl+A/C精确得到base a，fixed-ready.png确认正文及普通Tab。HEAD/index/受跟踪内容不变，assertions.json记录，测试PID36492正常退出。仅验证开发预览的解压/启动/代表性行为，不等同干净系统、所有功能回归或最新构建内存验收；此前专项证据及剩余验收继续保留。

### Project 文件读取错误摘要与恢复（2026-10-17）

按FailedEditorPane的短消息/下方动作规则整理src/project_view.rs：I/O诊断不再直接拼进正文，文件失败页面显示短摘要、重新加载和复制详情；树底部错误增加刷新/复制链接。二进制、编码不支持及读取超限仍各自说明。源码规则、本地映射和47%定位/通知差异见REBASED-REFERENCE。

Windows release 23.88s通过，SHA256 `96dd1ae8bd32ceb64ed47d3e624b24e1422d23c7a79fa5a535942b5039e57204`。artifacts/project-error-ui用隔离fixture打开a.txt，临时移走后点击重载；error-ready.png确认旧正文移除、中央中文摘要及两个动作。finally恢复文件后，verify.ps1点击复制详情，断言得到系统缺失文件诊断（os error 2）；点击中央重新加载后Ctrl+A/C精确为base a，recovered.png确认正文及选择恢复。assertions.json核对HEAD/index/受跟踪工作区不变；仅关闭自有PID15432，启动会话已正常结束。

功能阶段已核对中文文件缺失、摘要/详情分离、鼠标重载、成功后正文复制焦点。目录错误链接尚未做Windows原生专项，英文/全键盘、权限失败及失败中切仓组合仍待验收；不是整个Project或逐像素验收完成。未重复内存测试、未更新便携包，文首file-tabs包不含本轮错误页面改动。

### Project 目录错误英文与键盘刷新补验（2026-10-17）

目录I/O错误改用独立文件夹摘要，避免与普通文件读取失败混淆，中文资源同步。Windows release32.94s通过，SHA256 `283504b9f34e1cc195a23f42c01ae7337deada12f61c982e17b4997fd3c9b8c6`。artifacts/project-folder-error英文隔离fixture中先打开a.txt，再临时移走整个repo并点击Project刷新；folder-error.png确认树旧列表移除、短摘要/Refresh/Copy error details可见，中央文件Tab保留。finally恢复repo，复制诊断得到系统错误。初次按错误焦点假设进行Shift+Tab只到达文件/全局刷新控件，keyboard-refresh/tree-recovered-ready等早期截图不能作为目录重试通过依据。

改从根行明确聚焦后Tab到Project工具栏刷新，再Tab到错误区链接；retry-link-focus.png确认该Refresh焦点框，Enter后retry-link-ready.png确认文件树恢复且错误区消失。verify.ps1从恢复树打开b并复制base b，再返回原a Tab复制base a，证明目录恢复可用及中央Tab保留；assertions.json同时核对HEAD/index/受跟踪工作区不变。仅关闭自有PID6952，启动会话正常结束。

功能阶段补齐英文目录缺失/复制详情/键盘错误区重试；当前刷新仍回到根节点，展开/选择恢复尚未完成，权限失败/加载中切仓/全部键盘组合仍待核对。不做滚动自动化，不将普通启动内存数字当性能验收；便携包仍为file-tabs旧构建，未在此次重复打包。

### Project 刷新恢复展开目录及路径选择（2026-10-17）

依据Async/CoroutineProjectViewSupport与RestoreSelectionListener实现有界刷新快照。src/project_files.rs记录旧展开目录的最后已加载排序边界，重新分页读取至该边界；不扫描关闭/新增子树，总节点仍上限16384，预算耗尽保留未消耗的分页cursor。src/project_view.rs刷新取消旧任务/预览，结果通过generation复验；重建时保留当前路径选择，已删除目标回最近可见祖先，折叠根保持折叠。原生失败恢复入口沿用同一refresh方法。源码规则、内存方式和失败后状态等差异见REBASED-REFERENCE。

新增真实文件系统测试覆盖嵌套展开、边界之前插入文件导致跨页、关闭子树不加载、目录替换成文件、预算耗尽后下一页无跳项、取消和空展开状态。首次测试错误假设被替换的目录在变为普通文件后仍在第一页，按自然排序及分页规则改为从正确cursor验证下一页；没有放宽实现。Linux project_files全10项通过；Windows xwin测试构建后原生运行同10项通过（9.40s，artifacts/project-refresh-state/windows-tests.txt）。Windows最终release23.42s通过，SHA256 `e989227a0d75d72ef5a1390164793f3b43964e8fe23d68a27a4b55087093fa11`。

Windows原生artifacts/project-refresh-state：展开src/deep并选b.txt，外部加入排序更靠前的a.txt，刷新后refreshed-selection.png确认展开保留且仍选b，Enter打开后复制精确为nested original。临时移走b后刷新，deleted-selection-ancestor.png确认选择退回deep；Enter折叠deep，finally恢复b再刷新，collapsed-deep-preserved.png仍保持该目录折叠。折叠根再刷新，collapsed-root-preserved.png保持根折叠。assertions.json核对HEAD/index/受跟踪工作区不变，自有PID33852正常关闭，未操作用户窗口或测试滚动。

功能阶段已验证英文嵌套刷新/新增行/删除回退/键盘打开与折叠保持。根失败后的展开恢复、慢读时交互与切仓、视口、中文路径UI/预算耗尽UI仍待专项；首次新实现的内存峰值尚未采样，不能沿用旧构建性能结果。文首便携包仍为file-tabs，不含近期错误页及刷新增量，未更新交付包。

### Project 连续刷新失败后的展开与选择恢复（2026-10-17）

src/project_view.rs保留轻量refresh_restore/restore_selection，跨失败/重复重试保存展开路径和排序边界，不留旧正文。成功后释放；新的鼠标/键盘选择使旧选择记录失效，实际展开/分页/折叠清除旧展开计划。重试期间不覆盖新选择。首次目录尚在加载时刷新也把请求路径纳入计划，避免空entries误当作折叠。来源与适配边界见REBASED-REFERENCE，仍沿用此前有界读取、取消和generation规则。

最终Windows release24.82s通过，SHA256 `268116c35faf53c30cec3d9d8a1d8b6f0c385d25b816773ed51baf7767cc221c`。artifacts/project-refresh-retry隔离fixture展开src/deep并选b，临时移走repo后连续刷新两次产生真实错误，finally恢复repo后重试；restored-nested-selection.png确认原展开/选择恢复，Enter打开后Ctrl+A/C精确为nested original。再次移走repo，在失败页主动选根节点，恢复后new-selection-preserved.png确认目录展开恢复但选择留在根，没有回抢b。第三轮失败后通过Right明确重新展开根，explicit-expansion-retained.png确认随后刷新只保留新的根展开，src/deep没有被旧计划打开。验证脚本的初始聚焦点击也展开了测试窗口内的Log Branches区域，与Project恢复结果无关，不把该变化当作恢复行为。

assertions.json确认HEAD/index/受跟踪工作区保持；只关闭自有PID25264，启动会话正常结束，用户窗口未操作。功能阶段已核对连续失败/恢复、原选择打开、新选择优先和明确展开覆盖；首次加载中刷新及慢读时取消的原生竞态仍待专项，不以这组快速本地读取推断完成。此次未改读取层，不重复上一构建已通过的10项文件系统测试；UI变更已做原生验证。未做滚动自动化或新的内存验收，便携包仍未更新。

### Project 延迟预览跨入口取消与旧版对照（2026-10-17）

按AutoScrollToSourceHandler的100ms选择合并、执行时可见/焦点检查和cancelAllRequests规则，src/project_view.rs增加取消序号，src/ui.rs增加当前ProjectView实体/序号校验，并在Tab、侧栏和设置入口取消待发预览。直接选择预览/键盘正常预览和显式固定仍保留。详细映射及适配差异见REBASED-REFERENCE。

Windows最终release27.30s通过，SHA256 `84b9119081a2070564bc964535bd8d9ea60fc9ee0b11304c80bb130dbaacf7b5`。用旧SHA `268116c35faf53c30cec3d9d8a1d8b6f0c385d25b816773ed51baf7767cc221c` 独立fixture复现：固定a后在树内End紧接两次Project图标点击，旧版after-hide-reopen.png错误打开file0001预览，正文复制为preview 1。新版本相同队列保留a，复制base a。证据分别在artifacts/project-preview-cancel-baseline/current，两个复制出的exe哈希已实际核对。

新版本另覆盖End后立即切Commit再回Project，仍复制base a；固定b后回树a，End立即点b Tab，after-file-tab-switch.png保留b并复制base b；不加取消的End对照仍打开file0001斜体预览并复制preview 1，证明没有通过关闭全部预览绕过问题。脚本断言及截图记录结果，HEAD/index/受跟踪内容均不变，自有PID20920/37364正常关闭，用户窗口未操作。所有End用于选择触发预览，不是滚动自动化。

功能阶段已核对快速收起/重开、切Commit、切普通文件Tab与正常预览；切仓、模态和全部Diff来源的慢读取竞态仍待专项，未宣称整个异步验收完成。未改读取层，不重复文件系统全测；新版本未重新采样内存，旧样本仍仅对应其原构建。

### Project 生命周期增量便携包交付（2026-10-17）

README当前包及docs/WINDOWS-PREVIEW.txt同步Project目录刷新/失败重试的展开与选择恢复、明确操作优先、延迟预览取消，以及文件/目录的摘要/重试/复制诊断。生成dist/GitReview-windows-x64-preview-20261017-project-lifecycle.zip（17916113字节，799项，501依赖记录），保留旧包；CRC与manifest校验通过，主SHA256 `84b9119081a2070564bc964535bd8d9ea60fc9ee0b11304c80bb130dbaacf7b5` 与本轮原生取消验证一致。

artifacts/package-project-lifecycle：Windows Expand-Archive到全新验证目录，verification.txt确认三个exe哈希与包内README逐一匹配。解压程序中文界面启动，Project单击a预览，Enter固定，Ctrl+A/C精确base a；fixed-ready.png确认正常正文与普通Tab。assertions.json确认HEAD/index/受跟踪内容不变，仅关闭自有PID29432，启动会话正常结束。近期新增行为沿用同版本源码与对应专项原生证据，不将这次代表性启动检查描述为所有功能回归。干净Windows环境、当前构建完整性能/长期释放及未覆盖异步组合继续按原计划执行。


### 当前生命周期构建内存证据核对（2026-10-17）

核对现有原生采样产物，未重新启动重复测试、未修改产品代码。artifacts/project-lifecycle-regression和artifacts/project-refresh-memory的memory.json均绑定当前包主程序SHA256 `84b9119081a2070564bc964535bd8d9ea60fc9ee0b11304c80bb130dbaacf7b5`。采用Windows Job Object从挂起启动起统计进程组峰值，含继承的Git子进程，不含GPU；稳定态为应用Private Bytes，每场景10次。记录中另有用户GitReview进程16304，该进程未纳入测试Job，不能把桌面所有窗口占用相加后当作单实例数据。

- 空窗：53.21–53.23MiB，Job峰值55.49MiB，末态1进程。GPU独立计数Dedicated18210816、Shared1146880、TotalCommitted29028352字节。
- 10000条历史＋975000字节review.rs全文Diff：10次均96.66MiB，Job峰值153.35MiB（160796672字节），累计73进程、末态1进程，稳定采样子进程0。人工核对history.png底部已加载10000和window.png目标全文Diff；宿主Git cat-file -s再次确认975000字节。GPU独立计数Dedicated61296640、Shared3756032、TotalCommitted65843200字节。脚本AssertBudget启用，以上场景满足空窗80/稳定150/峰值250MiB预算。
- Project：4202文件、17页，首批64.95MiB，加载全列表并打开文件78.64MiB。三轮分别更换末尾文件名后刷新，再打开新文件并复制核对正文；cycles.json三次lastFileVerified均true，避免仅以点击刷新推断完成。刷新采样77.62/77.95/77.96MiB，打开文件77.86/78.46/77.84MiB，Job峰值131.85MiB，累计46进程、末态1进程。收起后10次77.64→77.13MiB（后9次稳定），没有回到首批基线，不据此宣称全部内存已归还操作系统。GPU独立计数Dedicated58765312、Shared1810432、TotalCommitted61497344字节；readonly.json确认HEAD/status未变。

GPU计数不与Private Bytes或Job峰值相加解释为同刻总峰值。三次刷新没有单调增长，只证明本次短序列，不证明长期无泄漏；此场景未覆盖16384节点耗尽、复杂嵌套目录或万条历史同时展开大目录。脚本耗时含固定等待、按键和截图，不是响应时延数据。原生截图只验证工作负载与可见状态，不代表像素验收。当前构建的长期会话、复杂宽图、启动/反馈延迟、其余写操作峰值及干净Windows环境仍待，避免继续重复同一基线采样延后功能主线。


### Project 延迟预览与设置模态专项（2026-10-17）

继续核对同参考commit的AutoScrollToSourceHandler.kt：SingleAlarm绑定默认模态状态，执行检查可见/焦点，cancelAllRequests可主动撤销。当前src/ui.rs::open_settings在打开弹窗前调用cancel_project_preview；本轮验证该既有实现，无产品代码修改。

artifacts/project-preview-modal新增独立Windows fixture与start/verify脚本，当前主SHA84b9119081a2070564bc964535bd8d9ea60fc9ee0b11304c80bb130dbaacf7b5。固定a.txt后在Project树发送End选择末项并紧接点击设置：settings-with-pending-preview.png确认设置已打开、树选中file0001、中央仍只有a.txt而无迟到预览；Esc关闭后复制正文精确base a。正常对照再次选择末项，normal-preview-control.png确认file0001.txt斜体预览及正文preview 1，剪贴板断言一致，未以禁用预览绕过问题。assertions.json确认HEAD/index/受跟踪内容不变；自有PID22272正常关闭，启动与验证会话均exit0，用户窗口未操作。End用于选择事件，不进行滚动自动化。

只补齐英文设置打开/关闭与待发预览组合；其他模态、快速开关、切仓和慢I/O完成期间竞态继续保留，未宣称全部模态验收完成。此次未改二进制，不重复构建、内存基线或打包。


### Project 默认隐藏名称与 Git 忽略文件区分（2026-10-17）

按REBASED-REFERENCE新增源码证据，将FileTypeManagerImpl默认忽略名单翻译为src/project_files.rs::hidden_in_project；普通与分页枚举均在计算页容量前应用，刷新沿用相同规则。隐藏版本控制管理目录、__pycache__等缓存、*.pyc/*.pyo/*.rbc/*.yarb及备份后缀~；扩展名不区分大小写，精确名称按上游默认区分。Windows .git仍保留原有大小写不敏感行政路径保护。普通点文件和Git忽略文件保持可浏览，不启动Git ignore查询，不增加全仓扫描；读取API只保留原有.git/路径/链接保护，名单只影响树展示。

新增真实文件系统测试验证完整名单、.env/.hidden/.gitignore/ignored.log保留、非匹配扩展名保留、普通/分页/刷新一致和已有文件仍能读取。Linux Project测试11项通过（1.16s）；Windows release通过（32.53s），主SHA256 `61bb0016409208b284770846a105f4125bc5a871ab3ba6ad67d600d33fb8625e`。

Windows原生artifacts/project-name-visibility：visible-names.png确认.env/.gitignore/ignored.log及普通文件可见，fixture中的缓存目录/.hg/.DS_Store/cache.PYC/backup~/vssver.scc不出现；单击.env复制精确visible dot file，键盘End选择ignored.log预览并复制精确visible git ignored file。脚本断言HEAD/index/受跟踪内容不变，自有PID25096正常关闭，两个会话exit0。End用于选择，不执行滚动自动化。源码来源及Apache-2.0许可已保留。

本轮没有引入可编辑全局名单、IDE模块/插件排除规则或OS hidden开关，这些差异已明确记录；没有宣称完整Project View项目模型一致或像素验收完成。文首便携包仍为project-lifecycle旧构建，不含这次默认名单增量；未重复内存采样。


### 现有功能集中回归与审阅基线包（2026-10-17）

按用户要求停止继续拆Project边缘组合，现有主功能证据集中收尾。当前源码cargo test --no-default-features报告347项通过：156库、4AskPass、33commit_index、14editor、8fetch_auth、7operation_process、4process_tree、11project_files、110repository；repository11.72s。日志artifacts/project-name-visibility/core-regression.txt。覆盖已有提交/index恢复、Squash、Fetch/Pull/Push更新/取消/冲突、编辑器与读取层真实Git路径；不包含desktop专属Diff模型，不称为完整Windows/网络/视觉验收。当前改动只涉及Project名称过滤，不重跑未受影响的全部原生操作窗口。

生成dist/GitReview-windows-x64-preview-20261017-review-baseline.zip，17910273字节，CRC通过，主SHA61bb0016409208b284770846a105f4125bc5a871ab3ba6ad67d600d33fb8625e。README和包内说明同步默认名称过滤，并修正README刷新重置分页的旧说法。Windows在新目录解压，三exe哈希及说明匹配；中文原生Project打开.env与Git忽略文件，复制内容精确匹配，ignored-file-preview.png已人工核对；HEAD/index/受跟踪内容不变，仅关闭自有PID28676。证据artifacts/package-review-baseline，启动与验证会话exit0。

该包作为继续开发和用户手测的基线，不代表所有历史验收框已通过。下一主线固定为merge-base分支改动审阅，再做显式冲突预检；已记录的非阻断Project/像素/罕见组合不再逐个追加为前置任务。


### 用户九项 UI/UX 集中修复（2026-10-17）

本轮围绕用户列出的九项集中完成，未增加分支审阅/冲突预检或其他Git操作。参考commit/源码路径/规则/本地映射与差异集中记录于REBASED-REFERENCE“UI/UX 集中修复：图标、筛选、菜单与欢迎页”；六张用户截图保存artifacts/uiux-round-reference/1.png～6.png。

| 用户项 | 实现与验证 |
| --- | --- |
| 1 侧栏图标 | Commit/Project采用Rebased 20px资源；Stash使用Shelve，Changes使用工具窗口Changes图标。显式Button.icon_size(20px)，32px按钮/36px侧栏，消除被默认尺寸覆盖；原生截图见uiux-round-final/commit-icons、stash-icons、project-open。 |
| 2 文件图标 | Log/Commit/Stash/Project/Compare/文件Tab共用file_icon与彩色img，补Rust/Python/C/C++/Shell资源；原生核对JSON/Rust/Markdown/Text在各面板一致，Rust普通Tab与树一致。许可证与来源保留。 |
| 3 条件清除 | Branch/User/Date/Paths各自×；清除只替换对应过滤模型、走原查询。uiux-round/head-user-filter显示HEAD＋me；clear-branch-keeps-user保留me并恢复Side提交；date-open已显示User清除；date-applied/date-cleared及path-filter-applied/paths-cleared确认分别应用/清除，路径从4条到2条再恢复。早期脚本焦点点击会关闭已打开菜单，误点击不作为通过；修正helpers后完成日期/路径验证。 |
| 4 Checkout选择 | 有同提交其他本地分支时显示子菜单，排除当前分支；分支与Checkout Revision复用现有执行器。checkout-assertions精确验证refs/heads/feature/review与随后detached HEAD，OID未变，未提交修改保留。 |
| 5 Tab关闭 | Close/Other/All/Right/All Read-Only，Log的Close禁用。混合Diff/普通文件，非活动cache右键Close Other后保留并激活cache；Close Right保留左侧；其余关闭回Log。tab-assertions含两次正文复制精确匹配；close-all-tabs/close-all-readonly/close-single-tab截图人工核对。 |
| 6 详情滚动条 | 实际overflow_y_scroll容器绑定details_scroll，超出时持续显示垂直Scrollbar；60行说明原生截图显示thumb。遵循用户约定，没有滚动自动化。 |
| 7 最近项目 | 最近优先/路径去重/最多20项及64KiB文件上限；仅成功仓库读取后记入，不扫描所有历史仓库。无路径启动中文欢迎页显示1条、重复打开无重复；点击后Ctrl+Shift+C精确匹配目标HEAD。最终欢迎页上方布局/深色背景修正；English包验证移除记录持久化且项目文件仍在。 |
| 8 HEAD双击 | 现有源码已绑定双击/Enter到HEAD过滤，本轮对照上游Head节点映射和原生验证；4条历史变3条HEAD可达提交，Side被排除。未将既有功能描述为新增。 |
| 9 文件高亮 | FileTab根据扩展名设置现有EditorState语言，未知类型plaintext；Rust/JSON原生截图显示语法颜色。复制全文精确匹配，输入尝试后正文仍相同；非活动正文/高亮实体沿用释放生命周期。无语义索引或编辑。 |

构建与证据：最终Windows release通过（25.09s），SHA ef594d4dff278cc46df4f92b678f6a70867092014d356e5bedd9968083ad53d9。recent_projects存储测试通过（Unicode路径、损坏/截断与大小/数量上限），日志uiux-round-reference/recent-tests.txt；未重复未改Git后端的347项全套与内存基线。

原生证据分构建记录：uiux-round覆盖过滤/HEAD/Checkout（仅临时fixture写入）；uiux-round-final主SHA a60df1ae89aae330b74b3dc57dd47700c61efbddc668006011caecd73fd7a64a覆盖最终批量关闭、图标与代码高亮；其后仅欢迎页布局/背景/字体修正，uiux-welcome-final和uiux-package绑定最终ef594d4。用户窗口未操作；自有PID35948/8412/27620/31084/34296均正常关闭，启动及验证会话exit0。readonly-assertions逐次检查工作区patch/status保留、HEAD OID不变；Checkout的分支/游离状态变化为测试预期。

生成dist/GitReview-windows-x64-preview-20261017-uiux.zip（17886510字节），CRC通过；Windows全新目录Expand-Archive，三exe哈希和包内说明匹配，英文欢迎页可见最近项目，移除后偏好记录为空且仓库正文/状态不变。package.json与assertions.json在artifacts/uiux-package。功能阶段已核对上述入口、主要布局、默认/禁用、关闭/返回和状态反馈；尚未宣称所有DPI/字体渲染的逐像素一致，也不将此次启动内存样本当作完整性能验收。当前仅待用户常规手测反馈，避免继续把边缘组合拆成无限前置任务。

### UI/UX 后续：编辑器释放与所有权错误（2026-10-17）

- 已修复 GPUI 窗口焦点登记对编辑器的强引用，关闭 Project 文件后对象可释放；回归测试 `tests/focused_input_release.rs` 修复前失败、修复后通过。未关闭代码高亮、全文复制或压缩文件内容。上游编辑器生命周期与 Log 错误源码依据见 `docs/REBASED-REFERENCE.md` 同名记录。
- Windows 同条件实测：10000 条历史、975000 字节 Rust 文件、全文复制、关闭 Tab；关闭后 10 次 Private Bytes 从旧构建稳定 150.63 MiB 降至新构建 130.32–130.35 MiB，Git 子进程均为 0；GPU 另计（新构建 dedicated 93.53 MiB，shared 3.31 MiB）。启动至结束内核 Job 峰值新构建 199.47 MiB（包含继承的 Git 子进程，不含 GPU），旧构建 200.07 MiB。文件内容复制校验通过，基准仓库 HEAD／状态未变。文件打开前新构建 87.87 MiB，关闭后尚未回到基线，因此这里只确认对象释放及本场景约 20 MiB 改善，不宣称全部缓存回收或完整内存验收通过。证据 `artifacts/file-highlight-memory{,-fixed}`。
- Git dubious ownership 由 Git 拒绝访问，不能当作普通空历史；改为 Log 内简短本地化错误、刷新及详情，详情弹窗保留完整 Git 输出／复制／关闭。未成功加载仓库时隐藏底部 Diff 重复错误。Windows 原生已复现 UNC 路径错误并核对空态、详情及复制；全局 safe.directory 未修改。JetBrains 源码确认的是通用 Log 错误截短及刷新机制，安装版专门所有权流程尚未实测，不能混同 IDE 项目信任。证据 `artifacts/ownership-error`。
- 新手测包 `dist/GitReview-windows-x64-preview-20261017-uiux-fix.zip`，17937550 字节，主程序 SHA256 `48c44671e8e45768ae89110a6edd645efdfa6ceb25a27b986e46d6889636d427`，Windows release 通过，归档 CRC/清单/许可证检查通过。本次没有新增提交；未进行滚动自动化或关闭用户的 Rebased／IDEA。

### 文件历史、选区历史与跳转源码（2026-10-18）

按用户最新要求优先实现以下审阅功能，覆盖此前回归收尾的执行顺序；merge-base 分支审阅和合并预检仍暂缓。用户三张 Rebased 原图保存于 `artifacts/file-history-reference`。

- 文件只读 Tab 的右键菜单提供 Show History，使用独立可关闭历史 Tab，显示版本／日期／作者／说明，选中版本加载与上一文件版本的 Diff；支持追踪重命名，不把同名文件重建前的无关历史混入。
- Show History for Selection 使用独立历史窗口；无选区时使用光标所在行，按 Rebased Block 规则跨版本回溯行块；默认 Changes only，可切回全部版本，包含工作区未提交的选区变化。上半部块 Diff、下半部版本表、底部提交说明，中英文同步。
- Diff 工具栏铅笔按钮和 F4 触发 Jump to Source；按当前侧／源行定位到工作区只读 Tab，考虑历史与当前文件之间的行位移。不可导航文件禁用或给出具体错误，不打开外部编辑器、不修改文件。
- 查询／内容／缓存有界且可取消；文件历史首批100、最多1000条，超限显示还有历史，按需扩展；单版本5MiB，选区内容预算16MiB。合并内重命名、重建、分页、取消和工作区行移位需真实 Git 测试；Windows 原生检查入口、首项默认展示、菜单、选区范围、关闭与复制，未完成前保持待验收。

上一轮回归收尾：当前工作树349核心测试、3桌面数据／预览专项及焦点释放测试通过。`scripts/windows-job-memory.ps1` 修正生命周期循环结束时 Diff 已关闭却仍按 LargeDiff预算验收的问题：AssertBudget 时重新打开 Diff、等待 Git 结束并保存 budget-active-diff.png。当前48c4467包实际显示975000字节Diff、10000条历史时稳定98.87–101.61MiB（Git子进程采样为0），内核Job峰值153.11MiB，GPU独立61,374,464／共享4,329,472字节另计；5轮开关Diff回Log稳定87.36MiB。旧坐标失败运行及关闭状态采样仅作诊断，不计为打开Diff预算通过。证据 `artifacts/uiux-final-regression`；窗口均为自有测试进程，仓库HEAD/status未变。

本轮已实现历史预览／选区窗口的源码跳转，和 Diff Tab／底部预览使用同一只读文件导航逻辑；弹窗Diff使用自身不可变target，Push预览跳转不使用底层Log的选择。历史Tab停用只保留版本ID、模式和加载规模等轻量状态，恢复时按ID重新选中。额外比较预算为两侧合计200000行、约1秒，超过预算明确失败；相同源文本直接映射，不建立行diff。真实Git4项（重命名/重建/分页/取消、选区本地变化和无关修改过滤、合并内rename、循环rename/未跟踪/路径校验）及行映射/预算2项单元测试通过。Windows原生最终验收与交付记录见本节后续记录。

文件历史的现代默认查询起点已补读确认是HEAD，本地一致；独立Branch过滤、graph与精确视觉仍有明确差异，本轮不宣称完整现代文件历史界面已还原。浅仓库、复杂合并与大规模选区的完整矩阵仍待后续验收，不将普通仓库场景扩写为完整发布验收。

交付与原生验收（2026-10-18）：最终R2主程序SHA256 `a5e87102c9fea6dd09dc079fc27dea2ae34b1d06676fa44d41dda02982cc2954`，预览包 `dist/GitReview-windows-x64-preview-20261018-file-history-r2.zip`，18099393字节；CRC、Windows解压和3个exe哈希检查通过，包含新的中英文入口及中文使用说明。

- 当前工作树完整核心回归355项通过，包含本轮2项行映射／预算单元和4项真实Git历史测试；日志 `artifacts/file-history-tests/core-final.txt`。Windows release通过，未新增提交。
- 原生 `artifacts/file-history-source-fix` 验证：文件右键菜单、默认首版本、重命名前原始文件全文复制、Changed only排除无关插行/重命名、关闭过滤显示全部版本、最初选区块复制、F4与铅笔按钮（含原版本侧）映射到有本地插行的第5行、历史预览与独立选区窗口跳转、返回历史Tab保留最初版本。`history-assertions.json` 与 `history-navigation-assertions.json` 记录实际复制内容断言。初版工具栏失焦首行回退和旧测试坐标失败图仅为诊断，不计通过。
- 仓库保护：临时fixture共6提交、63字节当前源码及未提交修改；HEAD、status、完整工作区patch检查未变。没有操作用户Rebased／IDEA窗口或运行滚动自动化。
- 内存样本：上述6提交小文件、主窗加选区历史窗口同时打开时，应用10次Private Bytes均84.46MiB，Git子进程采样0；GPU dedicated 86,683,648／shared 2,539,520字节单列。此为小仓库稳定样本，不包含未采到的Git瞬时峰值，不代表万条历史或大选区的完整内存验收。
- 明确保留：文件历史当前版本表布局尚无现代graph／独立Branch过滤入口；选区比较用块内行号，标题给出真实行范围。精确像素、复杂拓扑与浅仓库等完整矩阵继续按主计划验收，不把本轮功能验证宣称为完整发布验收。

### 文件历史：graph 与独立 Branch 筛选补齐（2026-10-18）

- 文件历史 Tab 改为 graph／提交说明／作者／日期列表，复用主 Log 的绘制和已翻译的虚线投影；真实拓扑跨过未修改该文件的提交，不按相邻行伪造连线。选择版本继续按需加载全文 Diff；合并按第一父语义比较，重命名读取旧路径，原版本侧源码跳转携带真实父版本身份。
- 独立 Branch 条件默认 HEAD，下拉分本地／远程／标签，× 清除后包含所有引用。不更改主 Log 筛选、不 checkout；Tab 停用释放正文，回来恢复条件、版本ID及模式。刷新／更多保留选中的版本。选区历史仍沿用独立窗口的版本表，不加入这个分支筛选。
- 来源、提取规则、预算及保留差异见 `docs/REBASED-REFERENCE.md` 本轮条目。现代文件历史还有多选比较、完整详情组件的结构差异，像素验收未完成；本轮不继续扩展其他大方向。最多100000临时祖先、128起点、16000／16MiB元数据；超过预算明确失败，不将截断图当成完整图。拓扑查询结束即释放，未新增常驻仓库索引。
- `cargo test --no-default-features` 357 项通过，`artifacts/file-history-tests/core-graph.txt`；新增真实Git测试覆盖HEAD／全部／单分支／annotated tag、分叉及隐藏边、真实父版本Diff、rename旧路径、分页、失效引用与浅克隆。Windows release通过；本轮未新增提交。
- 最终Windows原生包 `dist/GitReview-windows-x64-preview-20261018-file-history-graph.zip`（18042245字节），主SHA256 `bf6e715109eeffd233a7cffbe1097f0291f53da3eefe49c829193753457d1fc2`；CRC通过，宿主全新目录解压，3个exe与清单及release哈希一致。证据 `artifacts/file-history-graph`。最终构建重跑：HEAD默认6行、清除×后7行且呈分叉、单分支5行；切换源码Tab返回恢复选中的创建版本；原版本侧F4经历史映射复制到 `selected local`。截图由人工核对列表、条件、graph、Diff内容和菜单，不将截图行数描述为自动断言。
- 临时fixture删掉当前过滤分支后刷新，出现短错误／复制详情、旧图清空；恢复fixture引用再刷新可恢复。仅自有fixture引用被测试脚本修改，应用没有执行checkout或写操作；HEAD／patch／status断言均未变。只关闭自有验证进程，保留用户窗口；没有滚动自动化，没有以小fixture推断完整性能或内存验收。

### 文件历史焦点／关闭与性能验证（2026-10-18）

- 性能场景发现历史从源码右键打开后立即 Ctrl+F4 不关闭，点击历史行后却正常。小仓库原生 `artifacts/file-history-close/repro.ps1` 修复前失败（`repro-before.json`／`closed-before.png`），点击列表的 `focus-probe.ps1` 通过；说明是焦点／绑定上下文问题，并非大文件慢或快捷键完全没发出。原始性能 run 中所谓关闭后的截图仍是历史，已标记无效，不能作为内存生命周期通过证据。
- 对照已读 Rebased CloseEditorAction 的当前窗口回退与 FileHistoryPanel 焦点遍历规则，`src/ui.rs` 的 Ctrl+F4 在 Review 上下文关闭当前活动源码／历史／Diff，复用原关闭函数；存在 dialog/sheet 时不关闭背后 Tab。`src/project_view.rs`／`src/history_view.rs` 激活历史一次性聚焦列表，后续渲染不抢焦点。修复后原生同脚本通过，刚打开即可 Down 选择第二版本、Ctrl+F4 返回源码且全文复制匹配；设置弹窗内 Ctrl+F4 保留背景历史，Esc 返回后可正常关闭；再关闭源码、Diff 回到 Log。
- 最终构建 `b027d58c4e29e31b4a7edc90d9d17e69e14797ef9e21432319329e34e7b16818`，Windows release通过，357项核心回归通过。便携包 `dist/GitReview-windows-x64-preview-20261018-file-history-fix.zip` 18044060字节；CRC、宿主全新解压、三exe清单／release哈希一致。小仓库 HEAD／patch／status 未改，已正常关闭自有验证进程与 Git 子进程。像素／完整性能验收仍未完成，未新增提交。
- 设置验证中先前的快捷键打开尝试没有出现弹窗、后续复用失败状态的尝试也没有正确重建历史，均未计为通过；最终从全新进程使用齿轮打开实弹窗，截图 `settings-{open,after-close-key}.png`、`history-preserved-after-settings.png` 和最终回Log的 `diff-closed.png` 人工核对。自动剪贴板断言核对返回源码全文，截图核对模态保护与页面结构，二者范围分别记录。
- 新构建内存证据 `artifacts/file-history-memory/run-fixed3`：10000条主Log历史、review.rs 975000字节／13000行；此文件只有1条修改历史，因此覆盖“祖先拓扑10000、单文件全文预览”，不冒充10000条文件修改历史或复杂宽图验收。先校验源码全文，再打开历史，3轮Ctrl+F4关闭／精确复制源文件／重开，最后关闭两Tab回Log。原生截图与复制确认每轮页面确实切换；HEAD未变、status干净。
- 每个阶段5次Private Bytes（约500ms间隔，另有采样查询开销）：历史首次138.38–142.34MiB，三轮重开结束单样本139.49／139.73／139.58MiB；采样时Git子进程为0。末次历史采样139.58–142.41MiB，单场景低于150MiB；包括源码复制／重开过程的内核Job峰值226.30MiB，包含自挂起启动加入Job的全部继承Git子进程，不含GPU，低于250MiB。GPU末次历史单独记录dedicated 93.53MiB、shared 3.43MiB；不与Private Bytes相加。Git独立瞬时峰值仍未单列测量。
- 关闭全部文件后最终5次130.79–130.81MiB，仍高于初始约82.59MiB；普通源码返回后的结束单样本约151MiB，前几个样本还包含复制／高亮活动，不能把它包装成低于150MiB的稳态。当前只确认历史Diff本场景预算、3轮历史重开未呈持续线性增长，未宣称所有缓存释放、源码视图全面合格、100轮长会话、启动／反馈时延或完整发布验收通过。未修改内存预算。
- 失败／无效运行全部保留并标注：原run没有实际关闭历史；run-fixed被窗口遮挡保护拒绝，未发送误点；run-fixed2在大文本异步复制完成前读到空剪贴板，改用10秒有界精确内容等待后run-fixed3通过。它们均不纳入通过样本。滚动仍由用户手测，不进行滚动自动化。

### 文件历史多选版本比较（2026-10-18）

- 按已读 Rebased FileHistoryModel／FileHistoryPanel／详情 listener 规则，文件历史支持Ctrl切换、Shift范围及Shift上下扩展；Diff比较显示顺序中的首末所选版本，独立于鼠标最后点到的lead。跨rename读取各端真实路径。单选沿用第一父语义；独立选区历史继续单选表，不扩大编辑权限或增加Git写操作。
- 复用已有OID选择模型，Tab停用只保存集合／anchor／lead，刷新与重新激活按ID恢复并与当前图求交，不按旧行号转移到别的提交。两端正文按需加载，选择变化取消旧任务，原／新侧Jump to Source分别对应真实端点；空选择清空预览且禁用源码跳转。详情前50条并显示超限说明，溢出滚动条与主Log一致；滚动仍由用户手测。
- 新增2个真实Git测试验证跨rename的非连续端点比较、点击顺序与显示顺序区分、删除内部／端点选择、空选择、刷新新提交插入及窗口裁剪按ID恢复；现有选择模型测试覆盖Ctrl/Shift组合。当前完整核心回归359项通过（文件历史真实Git8项），日志 `artifacts/file-history-multi/core-tests.txt`。来源commit／路径／规则及本地映射见REFERENCE本轮条目，第三方声明已更新。
- 保留差异：单选多父合并处理仍未复制完整upstream changes merger；完整CommitDetailsPanel／详情布局及逐像素验收延后。显示与祖先／正文预算不变，不缓存每个选中版本全文；上一轮b027d58的完整内存证据仍只属于其构建，不冒充本次多选构建完整性能验收。
- 最终 Windows release 主SHA256 `09beb05fb367b756813cf58e6912c5cde27d7ddc2423e390a92287658af4619b`；便携包 `dist/GitReview-windows-x64-preview-20261018-file-history-multi.zip` 18128432字节，CRC／清单通过，Windows全新目录解压3个exe与release／清单一致。当前359项核心回归通过，未新增提交。
- 原生证据 `artifacts/file-history-multi`：Shift选6个版本、原侧复制selected v1／新侧selected v3，F4按原版本映射到源码selected local；返回Tab仍保持6选及端点；Ctrl取消HEAD后新端变selected v2；Shift Down范围可比较；全部取消及切Tab后空选择保持、源码按钮禁用、Diff正文清空。前述内容由剪贴板精确断言，选区色块／端点标题／详情滚动条／空态由截图人工核对，未把截图数量当自动断言。设置切English并检查独立profile偏好为en，英文多选结果和关闭后源码全文校验通过；提交正文、路径及作者未被翻译。参考与原生精确像素差异仍未全量验收。
- 结束检查：最终原生fixture的HEAD／patch／status均未变；自有进程及Git子进程正常关闭（final-assertions.json），用户Rebased／IDEA未操作。旧首轮5db48d2构建只作功能检查，最终09beb05构建完整重跑多选与英文检查，不混用不同构建的验收证据。未继续扩展其他主线或追加边缘组合。

### 阅读面板错误反馈统一（2026-10-18）

- 对照Rebased的Log短错误空态／刷新、SavedPatchesChangesBrowser错误区域，以及MessageDialog／DialogWrapper底部动作组，新增 `src/error_ui.rs` 复用短说明、Refresh和Details。完整诊断在有界高度、可复制、有明确Close／X／Esc／Enter入口的详情框中；复制和关闭在底部右侧，不把复制按钮拉成整行。来源commit、实际读取路径、本地适配与差异见REFERENCE本轮条目。
- Stash不再直接铺原始error或固定100px原始操作诊断；列表／正文失败放到对应区域，同一底层诊断导致的恢复记录提示仅去重显示（状态保留，独立错误不隐藏）。文件历史／版本比较有相邻刷新和详情；Project保留原有语义及重试规则，IO诊断从仅复制升级为详情；主Log也复用该详情框。未改变Git命令、写操作、信任配置、选择恢复或取消策略。
- 359项核心回归通过，Windows release通过。仅新增共享UI及本地化文本，不为静态按钮结构编写镜像测试；原生故障场景检查实际错误、复制和恢复。完整通知／写操作诊断矩阵、所有错误类型和逐像素验收仍未通过，本轮不扩大范围。
- 最终主SHA256 `f317b5f5d1e84735084e8494900fe17ff6fbd3736f5106a17b79ca79c40d6448`；包 `dist/GitReview-windows-x64-preview-20261018-error-feedback.zip` 18120628字节，CRC／manifest及Windows解压3个exe、说明文件的哈希核对通过。证据 `artifacts/error-feedback`，未新增提交。
- 原生验收：在自有复制fixture中暂移.git并finally恢复，历史／Stash／比较分别出现本地化短说明、相邻Refresh／Details，恢复后查询正常。历史和Stash复制完整`fatal: not a git repository…`，比较复制其实际对象访问诊断（Git在仓库外diff给出Could not access+原OID，未将其错误包装成另一种诊断）；每次复制前写入哨兵，避免相同旧剪贴板让测试假通过。重复的Stash恢复检查错误消失，记录与正文恢复可见；截图人工核对位置、默认禁用与无长原始输出。
- 对自有源码文件加FileShare.None只读锁，Project读取失败显示短说明及详情；释放锁后重载并精确复制全文匹配。中英文详情均可复制os error 32完整诊断，切English后原始诊断逐字相同，英文UI与偏好en已核对。Esc、Enter、显式Close分别走过关闭；X入口可见。Project目录入口复用同工厂及原重试规则，但本轮未重跑目录IO全部矩阵；Stash写操作诊断入口代码接入，未宣称本轮完成其写操作矩阵。未滚动自动化，未重复未受改动影响的内存基线／宣称新构建完整性能合格。
- 诊断跑次不计为通过：首轮按钮区域点击未命中；早期Compare脚本错把对象访问错误期望为not-a-repository，按实际Git诊断纠正；说明校验中的中文PS源码编码匹配改为文件哈希。复用旧源码跳转准备脚本的定位断言未稳定通过，保留截图；错误面板验收改从已打开源码直接定位，不将该准备步骤算作源码跳转的新验收或假定其原因。最终实际故障／复制／恢复记录绑定上述f317b5f构建。
- 收尾：fixture HEAD、patch、status及refs/stash均未变化，临时.git移动和文件锁已恢复；自有app／子进程已关闭（final-assertions.json），用户窗口未操作。当前仅更新本轮验收范围，不把共享入口推广成所有通知、认证、写恢复及像素验收完成。

### Diff 正文跳转与首次源码定位修复（2026-10-18）

- 继续文件／选区历史与Jump to Source的公共交互验收，不扩展产品范围。修复前 `artifacts/source-navigation-audit/code-click.ps1` 稳定复现：点击正文后F4复制得到`# Header`，行号选择后同操作得到`selected local`。Rebased从caret取当前侧／行，单独保存行号选区不足；已读来源、commit与本地差异见REFERENCE最新条目。
- 公共Diff保留轻量源行导航位置，正文按下、文本选择游标和行号选择共同更新；替换文件清空，工具栏失焦保留。DiffTab／底部／历史共用getter，Push预览沿已接入的DiffTab回调生效。本轮没有重新跑完整Push矩阵。只记录行，不宣称支持完整编辑器光标／列导航。
- 原生复测又隔离出首次加载问题：中间构建91429cd3的正文跳转到已打开源码通过，但冷打开源码仍回首行。`FileTab::render`的tuple模式在viewer为空时也执行`jump_line.take()`；改为viewer就绪后才消费。保留中间失败证据 `artifacts/source-navigation-fixed/repro.json`，不将其计为通过。
- 最终Windows原生 `artifacts/source-navigation-final`：冷打开源码、正文F4、正文后铅笔按钮，均精确复制`selected local`；工作区有本地插入和修改，用来核对历史版本到当前行的映射。截图人工核对当前行与只读Tab。359项核心测试通过，新增两项GPUI回归通过（单／双栏正文源侧、无整行选择、替换清理；编辑器加载前保留目标行／加载后应用）。测试准备早期固定横坐标误落原侧／默认单栏，改为明确Split及按viewport比例定位，不将那些预期不符算产品回归通过。滚动未自动化，内存基线未重测，像素验收仍未完成。
- 最终Windows release通过，主SHA256 `b786cfb6cc7fecb3c7efa15189b1615f80e4769fe3365ca8fb6fbf917c88ca42`。包 `dist/GitReview-windows-x64-preview-20261018-source-navigation.zip` 18137152字节；CRC／manifest通过，Windows解压核对见同目录package-verified.json。未额外提交。
- 收尾核对：Windows解压三个exe及说明文件哈希均与manifest／release一致；fixture HEAD、patch、status未变化，自有原生测试app与Git子进程已退出。用户Rebased／IDEA未关闭。

### 文件历史详情结构与信息补齐（2026-10-18）

- 本轮推进首页“文件／选区历史”的完整详情区域，未展开延后池。实读FileHistoryPanel确认文件历史详情位于列表下方，与列表构成一组，再与Diff拆分；不是直接搬主Log右侧，也不是放在Diff之后的固定消息栏。来源commit／类、实际读取规则和差异见REFERENCE最新条目。
- 普通文件历史改为列表→详情的可调整区域及独立下方Diff；完整说明、hash、作者／提交者与时间、分支／Tag及包含分支，复用主Log现有格式与图标组件。独立选区历史继续保留Diff／版本表／消息布局。多选只加载前50条元数据并提示；不缓存这些提交的文件全文。详情加载、空态及短错误／重试／详情位于本区域，Diff读取独立；切选择／刷新取消旧metadata与包含分支任务，generation拒绝旧结果，关闭释放。
- 359项核心回归通过，Windows release通过。新增主要为既有查询API及卡片组件接线，不为静态布局写镜像测试；原生用独立仓库副本核对作者／提交者不同、分支与annotatedTag、多选及关闭恢复。没有完成前不勾选整项或宣称逐像素合格。外部状态徽章、主Log哈希导航、详情开关、分割比例持久化仍为差异，详见REFERENCE。

- 最终原生证据 `artifacts/file-history-details-final`（bd2392f3构建）：独立fixture副本的HEAD有不同作者／提交者、两个本地分支与annotatedTag。单选卡片逐项核对说明、hash、作者／提交者／邮箱／日期、HEAD／本地分支、独立Tag行和包含分支；包含分支显示HEAD、main、review-main，Tag未混入。复制元数据检查两组身份／邮箱，卡片菜单复制完整HEAD精确匹配；两选按显示顺序生成两张卡片，取消HEAD后只剩另一提交，清空选择时卡片及Diff清空、源码入口禁用。详情区域可调整高度，溢出显示滚动条；未进行滚动自动化。
- 切English后偏好en、英文author/committer元数据复制检查通过，提交原文／邮箱／引用名称不翻译；Ctrl+F4关闭历史返回源码，全文复制与磁盘文件精确相同。最后HEAD、patch、status与fixture启动基线核对，正常关闭自有窗口及Git子进程；不关闭用户Rebased／IDEA。
- 中间93f7264构建发现普通flex_basis会被ResizablePanel尺寸管理覆盖，详情初始偏小；最终改用size初始接口，后续用户拖动仍由原组件管理。首轮复制测试误点消息卡片，只复制了标题，改点metadata后断言通过；该准备失败不计为元数据验收通过。完整50条上限／外部状态错误矩阵和新构建性能基线未在本轮原生全量重跑，不扩大验收结论。
- 最终主SHA256 `bd2392f397938f705cf7494475c0644e695121100c1f5a60bf208987630c599b`，包 `dist/GitReview-windows-x64-preview-20261018-history-details.zip` 18185057字节，CRC／manifest通过；宿主全新解压三个exe和说明文件与清单／release哈希一致。当前359项核心回归通过，像素与完整发布门槛仍未完成；未新增提交。

### 文件历史 graph 父版本及合并请求模型（2026-10-18）

- 按Rebased FileHistoryModel→FileHistoryUtil→VcsStatusMerger读取并翻译，普通文件历史单选比较graph父版本，多选仍比较显示顺序首末；两父版本完整身份保留，rename同源／异源和missing／deleted状态区分，比较端点只持有OID／路径，避免按父数复制长消息／正文。已知边缺少元数据要求加载更多，不把cache缺项当新增；已加载older可用作边界。旧previous_revision第一父接口不再用于普通历史UI，选区块历史不变。
- 沿FileHistoryDiffProcessor和MergedChangeDiffRequestProvider实读确认：恰好两个有效sourceChanges应优先三栏（parent / result / parent），缺少一侧才降两栏，其他父数采用合成状态的普通两栏。请求模型已按该顺序翻译并保留所有父版本。**三栏renderer仍未接入**；当前三侧请求暂以明确标注“父版本”的第一个graph父版本→合并结果两栏显示，不把它宣称为完整合并Diff还原。下一项沿这个模型接三栏，不继续新增目录／焦点边缘组合。
- 状态merger回归先失败（不同rename源错误保留old-a.txt），按上游规则转selectedPath modification后通过。新增4项专项（实际Git clean merge、图父版本状态规则、missing／rename状态、请求三侧／缺侧顺序），旧merged-rename实际Git测试补断言：graph父路径a.txt正常，而旧第一父查找无before，能区分整文件误新增。完整核心363项通过；最终实际Git请求同时核对左右两个graph父版本身份，非只有一个手工构造request测试。
- 分页测试初期错认为一个截断HEAD查询必然保留缺失父边；实际投影只有已知父边且older可读。按实际数据改为核对已知父有效、投影仍命名未保留父时拒绝假新增，未为错误假设增加拒绝正常读取的分支。完整无限历史／复杂merge矩阵及本构建性能峰值不因此通过。
- Windows前后对照：旧bd2392f3构建 `artifacts/history-parents-before` 在独立临时仓库中，合并时把a.rs重命名并修改为b.rs，graph历史含feature版本但Diff整段新增（截图人工核对）；唯一可见列复制到merged而没有two。新9b1ac0c3构建 `artifacts/history-parents-final` 单选原侧精确复制two、新侧merged；从历史a.rs行F4映射当前b.rs得到merged。切回历史Shift选到创建版本，原侧one、新侧仍merged；关闭历史返回源码，全文复制与磁盘一致。每次复制前置哨兵，避免旧剪贴板假通过。
- 旧版第一轮按双栏原侧的x110找行号，但旧版只有新增列，其行号实际偏右，复制仍是哨兵；改按其真实唯一列行号x142重跑，明确记录“整段新增”对照，不把准备点击失败当产品结论。最终前后两轮均核对HEAD／patch／status不变，正常关闭自有app及Git子进程，未操作用户Rebased／IDEA。
- 本轮原生验证覆盖合并内rename的graph父路径、正文比较、两端Source及多选端点；双父三侧请求的临时“父版本”标注仅做编译和模型验证，未在本轮额外跑双父窗口专项，随下一项三栏renderer一起验收。中英文新状态／更多父数UI及完整复杂拓扑仍未全量验收，不把普通rename对照泛化为完整合并UI合格。
- 最终主SHA256 `9b1ac0c38bc3eea5561b4cdd504a3db4c79889c878c4e18d92caad4e9de193c9`；包 `dist/GitReview-windows-x64-preview-20261018-history-parents.zip` 18174654字节，CRC／manifest通过。完整核心363项通过，强化后的文件历史12项单独重跑通过；Windows release通过。宿主解压／清单核对见同目录package-verified.json；新构建没有冒用旧内存测量，三栏和完整像素／发布门槛仍未完成，未新增提交。

### 双父三栏合并结果 Diff（2026-10-18）

- 本轮接入上一轮保留的三侧请求，不再临时丢弃right parent。新三栏显示left parent／merge result／right parent，标题有真实OID／历史路径／只读图标；三侧逐一读取实际blob，独立来源导航。多选或缺少一侧仍按已确认请求规则降普通两栏，保持原单／双栏偏好。文件历史增加变更上下导航及F7/Shift+F7，动作由本历史处理，避免跳到背后的主Log；三栏变更导航取center真实行，清理旧选择。
- 源码、commit、原生结构规则与本地位置见REFERENCE最新条目。共同Equal对齐、片段颜色分类及MERGE_RESULT方向已翻译；不将三份文本各自滚动或复制人为补齐空行。三列共享虚拟行列表与水平offset，每列独立裁剪。真实源索引必须按顺序且只出现一次；padding不生成复制内容。当前行级强调，BY_WORD词内强调、连接曲线及独立滚动开关等保留差异，不宣称像素级完成。
- 新增7项核心测试覆盖相同／空文本、单父修改、合并结果新增／删除颜色方向、相同／不同父修改、长短片段补齐、中文／CRLF、512种小序列的源顺序守恒、取消及100000行预算。370项核心回归通过。新增2项GPUI专项覆盖三列点击／复制／来源、跨栏拖选锁定、padding不进复制、Ctrl+A活动列、F7center来源及关闭后viewer弱引用失效，以及无效重复／缺失行拒绝；已有2项源导航／焦点注册释放专项一同通过。源码判定及行为测试分别记录，不用布局镜像测试代替交互。
- Windows原生 `artifacts/three-way-native`：真实双父merge含左右不同插入与修改，三列全文逐字匹配对应Git内容；每列行号F4都映射工作区相应行，F7与铅笔按钮导航通过；Shift多选还原两栏并精确比较base／merged两端，单选merge恢复三栏；切English后偏好en、真实正文不变，标题／结构截图人工核对；关闭历史返回只读源码，全文复制一致。未做滚动自动化。HEAD／patch／status未变，自有app与Git子进程已关闭，用户Rebased／IDEA未操作。
- 预算与生命周期：每来源5MiB、对齐最多100000行，行收集先限量；相似性匹配有阶段／总时间预算及取消检查。三份正文仅在活动请求中持有；metadata仍按原50条独立加载，切选择拒绝迟到任务；停用／关闭释放。SourcePosition索引使用内部唯一列身份，导航携带真实Revision/path，内部标识不绘制为标题。
- 新构建三栏内存证据 `artifacts/three-way-memory/run`：实载4条提交（启动上限参数10000，不伪称万条已加载），left/result/right各961689／961716／961690字节，即约0.917MiB，13001／13002／13001源行。三列全文分别精确复制校验，1次首次打开＋3次关闭／重新打开；截图人工核对每次确为三栏及最终所有文件Tab关闭回Log。未滚动自动化。
- 稳定Private Bytes（采样时Git子进程为0）：首次三栏111.04–112.01MiB，随后三次138.98–139.69、139.01–139.95、139.41–139.52MiB。整个流程Kernel Job峰值217.58MiB，从挂起启动即纳入应用与Git子进程；不是只截取稳态的进程快照。独立budget-audit.json核对已打开各阶段≤150MiB、流程Job峰值≤250MiB；通用脚本assertBudget=false是因Scenario模式禁止LargeDiff断言，未将最后较低的关闭态冒充已打开预算通过。
- GPU另记：最后三栏打开采样Dedicated约83.22MiB、Shared约4.68MiB，不混入上述CPU Private/Job阈值。启动基线Private65.29MiB，全部文件关闭后119.49MiB；未宣称所有缓存释放或整个长会话达标。此前进度口头“0.96MiB”单位不精确，此处以原始字节和约0.917MiB为准（约0.962MB）。该场景规模与旧万条历史基线不同，不用它替代复杂宽图、真实万条三栏、长会话和干净Windows验收。
- 小仓库与内存场景均绑定主SHA256 `aa9d92f891ac358db2dc18dd1b78c46e19ef4b6e3fd206c20c8aca1724c97e6c`。Windows release通过；小仓库及大文件fixture HEAD／状态未变化，测试自有应用／Git子进程已退出，用户窗口保留。当前功能与有限规模内存场景通过，完整像素及发布门槛仍未完成，未新增提交。
- 当前手测包 `dist/GitReview-windows-x64-preview-20261018-three-way.zip`，18136328字节；Windows重新解压核对三个exe与release／manifest一致、使用说明一致，见 `artifacts/three-way-native/package-verified.json`。强化后的三栏GPUI两项测试再次通过，见desktop-three-final.log。

### 当前构建 Windows 启动响应测量（2026-10-18）

- 补已有“固定等待后截图”的证据缺口；新增可重复执行的 `scripts/windows-startup.ps1`，指定exe／输出目录／可选仓库／历史上限。禁止覆盖旧输出，隔离应用profile，轮询当前测试进程的可见主窗口并用SendMessageTimeout(WM_NULL)检查响应；正常关闭自己启动的进程，不枚举关闭用户窗口。未修改产品Git或UI行为，故不产生新的Rebased实现规则。
- 当前AA9主程序（SHA256同首页）Windows原生各5次：欢迎页首次可见1002.09ms、可响应1036.39ms；随后四次可响应389.30–427.20ms。10000提交线性基准仓库五次可响应358.26–414.78ms，中位数398.71ms。计时从Start-Process之前开始，包含启动调用开销；10ms轮询及Windows调度误差、每次响应探测最多50ms须计入解释。
- 证据 `artifacts/startup-aa9/{welcome,repository}/startup.json`；settled首／末截图另在计时结束后等待3秒采集。人工检查repository/settled-4.png确实显示10000 commits loaded、默认首提交与两文件树，不把传入limit参数冒充实际加载规模。该截图只能证明后续观察时已加载，不证明上述窗口响应时已经加载完成，也不是首次可见帧的精确呈现时间。
- 首轮没有清理OS磁盘／GPU驱动缓存，不称为冷启动或重启Windows样本；后续为同一机器连续启动。用户Rebased／其他窗口保持打开，非干净Windows环境。未测本轮点击到选择／加载反馈100ms目标，不将WM_NULL回包替代交互反馈。完整冷热启动、数据就绪、复杂宽图及长会话仍待验收。
- 临时基准HEAD／status未变，10次自有应用正常退出；Git子进程退出检查见cleanup.json。未进行滚动自动化；程序未变，无需重新打包或借本次测量更改现有内存结论。

### 三栏合并结果的词内变更强调（2026-10-18）

- 继续文件历史已确认差异，未扩展新Git操作或暂缓的分支审阅／冲突预检。按REFERENCE最新实读规则在整段变更块中比较两／三侧，中心列合并两次词差异；未变化侧、空侧和padding不生成强调。行级颜色、语法高亮、选区覆盖层共同保留；插入行前后不按显示行错误配对。
- 后台计算稀疏真实源字节范围，不复制所有文件正文进常驻缓存；每块每侧32KiB、源累计约512KiB、100000范围、每次20ms／总体250ms，超预算保留行级颜色，取消过期请求。GPUI安装时验证真实行／UTF-8边界，换源清理，Tab展开使用display映射，不改变复制与F4来源。
- 本地similar Unicode word分词与上游完整ByWordRt仍有差异，不宣称词级区间全面一致。连接曲线、独立滚动与三栏设置仍保留；功能与精确像素分别验收。源码commit／类／规则／本地位置详见REFERENCE最新条目，许可来源已更新。
- 新增4项核心测试覆盖center差异并集、整块跨插行比较、中文／Tab／CRLF、仅一侧及超大块／取消；完整核心374项通过。3项三栏GPUI专项（新增UTF-8／越界／未变化侧拒绝与换源清理）、源码定位和焦点释放专项通过。日志 `artifacts/three-way-inner/{core-tests,desktop-tests,windows-build}.log`；Windows release通过，主SHA256 `3024f5df9047a71ef24c1fe52075295bfe9c3fbd89de8da973333b5f7a928360`。旧构建内存／启动测量不直接作为新构建性能验收。
- Windows原生 `artifacts/three-way-inner/history-loaded.png`人工核对数字、字符串、插入注释的词级强调，未变化侧保持原样。三侧全文复制与对应Git源一致（exact-copy-audit.json仅规范CRLF、不trim尾部空白）；三列F4、F7和铅笔跳转通过，HEAD／patch／status未变，自有应用与Git子进程已退出。滚动仍交用户手测，未操作用户Rebased／IDEA。
- 手测包 `dist/GitReview-windows-x64-preview-20261018-three-way-inner.zip`，18192374字节；CRC／manifest打包检查通过，Windows新目录解压后3个exe匹配release与manifest、说明文件一致，见package-verified.json。未新增提交。当前启动／内存证据仍分别绑定旧构建，本轮只确认功能与原生显示，不称完整性能或像素验收通过。

### 万条提交／32轨道宽图的预算失败与修复（2026-10-18）

- 按首页性能缺口做当前构建宽图验收，发现真实失败：10000提交、32并行分支、32父octopus merge、975000字节Rust Diff（约0.930MiB、13000行），旧3024构建全Log显示读取失败。快照单独读取10000成功，但同UI的history::read布局失败；核心10000／32lane回归先失败，真实原仓库harness也记录相同geometry budget错误（artifacts/wide-3024/probe-layout-red.log及regression-red.log）。不是Git损坏或Windows所有权问题。
- 旧布局638107条半段超500000预算。src/graph.rs合并同列／同样式／不经过当前节点的两半垂直边，src/ui.rs绘制完整行高，保持节点／父边和虚线相位；新329099存储段，展开后仍638107半段，最大32轨道。500000存储段、512轨道和150／250MiB门槛均未提高。Rebased源码版本、类、规则及内部适配差异见REFERENCE最新条目。
- 新增1项10000提交／32lane布局回归，强化已有随机DAG上下连续样式和真实父边检查；分页／过滤导航几何对照包含full标记。完整核心375项通过，Windows release通过；临时CLI探针已移到artifacts/wide-3024/wide_probe.rs，未遗留debug插桩或新产品入口。
- 同时修复测量假阳性：原windows-job-memory的AssertBudget仅检查数字，错误空页也能通过。旧run内NOT-ACCEPTED明确拒绝该证据；新增ExpectedDiffFile在采样前从实际Diff复制全文并与源文件比较，只规范CRLF不trim空白。rejected-frame-check.json确认旧错误页被新校验拒绝，不将assertBudget=true旗标单独当验收。按当前固定150%DPI坐标验证，非通用UI定位器。
- 正式证据artifacts/wide-3024/fixed-focused：人工核对history.png确实10000 commits loaded且32分支合并／贯穿轨道可见；window.png为独立review.rs全文Diff，content-verified.json确认975000字节全部可复制。只读HEAD／status未变，自有应用／Git子进程已退出，用户Rebased／IDEA未操作。滚动未自动化。
- 当前E422构建（SHA256同首页）10次500ms间隔稳态Private Bytes合计119.34–127.07MiB（直接Git子进程采样均0）；整个启动／加载／全文复制流程内核Job峰值183.83MiB，含Git子进程，不含GPU。此次32轨道超过普通目标≤20，但仍低于150／250MiB门槛。Working Set／GPU详见steady.json，独立显存约74.19MiB、共享约4.13MiB，不并入Private预算。此前口头约119MiB指收敛值，完整采样范围以本记录为准。
- 非干净Windows、非真正冷缓存，不证明数百轨道、长会话、全部写操作峰值或滚动帧率达标。初次fixture生成因注释行字节长度断言中止，保留-incomplete目录，重新生成并检查真实commit数／merge父数后才运行；首个修复原生run因键盘前台保护中止，fixed/NOT-ACCEPTED保留诊断，之后仅在确认测试窗口标题未被遮挡时激活自己的窗口并重测。未绕过前台保护向用户窗口发送快捷键。
- 当前手测包 `dist/GitReview-windows-x64-preview-20261018-wide-graph.zip`，18188883字节，主SHA256 `e4229f06bc87c83c149b7589caa8735d9e40ed85edf037a817b2ca171aaf1dd9`。打包CRC／manifest通过，Windows新目录解压3个exe与release／manifest一致、说明文件一致，见artifacts/wide-3024/package-verified.json。未新增提交；完整发布、像素、长会话门槛仍未完成。

### Commit／Amend 失败反馈与英文主流程（2026-10-18）

- 按首页“常用流程／错误与中英文”继续收尾。发现Commit失败仍将原始stderr前2000字符塞入72px裁切区；改为本地化短摘要、Details、Dismiss，复用有滚动条／复制／关闭的详情弹窗。诊断包含有界stdout与stderr、恢复／记录／HEAD/state错误，最多32000字符并显式截断；失败保留消息和勾选，原Commit／Amend按钮就是显式重试，不自动忽略hook、关闭签名或Push。读取失败用Refresh／Details，Amend消息加载失败用Retry／Details。源码规则／上游版本／本地通知适配差异见REFERENCE最新条目。
- Windows临时repo与本地bare remote进行英文Commit→hook失败→详情→中英文切换→修复hook→原草稿重试→无文件选择的消息Amend。Hook输出5119字符，详情复制包含开头、末尾、拒绝标记和第119行，不再被2000字符或72px裁掉。中文summary和控件即时更新，Git/hook正文保持原文；两个语言截图人工核对，滚动未自动化。
- 失败HEAD未变，重试只提交selected.txt且保留原消息，Amend加载前条消息、改变HEAD但提交数量仍2；选中文件正文正确、未勾选excluded.txt在HEAD仍base，index仍excluded staged、磁盘仍excluded unstaged。bare remote的main始终为初始HEAD，未隐式Push。证据artifacts/commit-feedback/{failure,details,retry,amend}-assertions.json与cleanup.json；自有app／Git子进程已退出，用户窗口保留。
- 最初中文断言在Windows PowerShell 5读取无BOM脚本中文literal时错误，原生截图已显示正确中文；改用Unicode码点断言并从当前弹窗续测通过，保留早期截图及retry-resume.ps1，不将这次测试脚本编码问题写成产品修复。所有写操作仅限此fixture，本项目源仓库未提交。
- 未改提交执行和恢复事务，按影响范围重跑commit_index 33项、i18n 3项通过；Windows release通过。完整核心375项证据仍为前一轮相同核心行为，不声称本轮重新跑全套。新主SHA256 `2e97614d4bed87a43a272027909e33e8019ab7c72d00f257514a9a77c127695f`；未重测内存／启动，旧E422性能数字仍绑定旧构建。Hooks/签名/认证全部组合、通知浮层、精确像素和完整发布验收继续保留，当前只确认所列常用流程。
- 当前手测包 `dist/GitReview-windows-x64-preview-20261018-commit-feedback.zip`，18200268字节；CRC／manifest通过，Windows独立目录解压核对3个exe与release／manifest一致、使用说明一致，见package-verified.json。未新增提交。

### 万条历史的选中态反馈延迟（2026-10-18）

- 补性能目标“点击后100ms内显示选择反馈”的实际绘制证据。新增scripts/windows-selection-latency.ps1，只观察已加载Log前两行，不滚动、不修改产品或Git数据。当前2E976构建SHA256同首页，独立profile／Windows原生／10000提交线性仓库；before/after截图人工确认确实10000 commits loaded。
- 固定当前150%DPI布局，先校准Log聚焦选中色#2a4371，再向受身份／前台／遮挡／Ctrl/Shift状态保护的自有窗口PostMessage鼠标消息；从入队前开始Stopwatch，观察屏幕两行空白点同时完成“目标行变蓝、原行不再蓝”。每次先验证相反行已选中的基线，不能把静止蓝像素当成功。屏幕DC读色／窗口遮挡检查／1ms请求睡眠与调度开销均包含在报告值中，非物理鼠标设备延迟，也非内部handler计时。
- 20次交替选择（间隔250ms），视觉反馈24.90–39.22ms、最近秩p95=38.76ms；20/20低于100ms。证据artifacts/selection-latency/run/latency.json记录每次前后两点COLORREF、轮询次数和原始double耗时；测到的是保守观察上界，屏幕读取本身会增加耗时，不以本样本反推精确GPU帧时间。
- 此项仅证明普通线性万条场景的选中背景反馈；没有把它扩写为数据就绪、加载文字可见、首批1000条2秒、冷启动、数百轨道或滚动帧率验收。加载反馈与Git内容读取时延仍在首页剩余项。不是性能全项完成。
- 临时基准HEAD／status未变，自有app与Git子进程已退出，见readonly.json／cleanup.json；用户Rebased／IDEA未操作，未自动化滚动。未修改产品源码，无需重编译、全套回归或重新打包，继续沿用commit-feedback预览包。

### 文件历史详情显示选项（2026-10-18）

- 按已读Rebased FileHistoryUiProperties／FileHistoryPanel／ShowDetailsAction及菜单XML实现Show Details。现代源码默认false，本地改为默认隐藏；普通文件历史工具栏的显示选项使用GroupBy实际映射的show_dark.svg图标，提供中英文checked菜单项。选区历史独立窗口不添加此入口。列表／详情内部分区按开关显示，外层列表／Diff 0.7比例不变，没有以隐藏详情偷偷改变预览布局。
- 按项目根路径保存到应用配置目录，不写工作区／.idea；路径key与文件索引相互验证、读取最多64KiB。切Tab重新读取同一项目的最新值，不保留旧Tab私有覆盖；刷新／分支条件保持该值。隐藏取消metadata／包含分支任务、递增generation并释放卡片，显示时只加载当前所选OID。低内存适配与上游隐藏但保留组件的差异、源码commit／路径／规则见REFERENCE最新条目；精确像素仍未完成。
- Windows原生artifacts/history-options：新profile默认无详情卡片；选择非首条parent后，显示／隐藏、刷新、离开回到历史Tab均保留同一OID及完整7行目标Diff（逐字复制，仅规范CRLF）。重启同项目恢复已开启的详情；第二个复制仓库仍默认关闭，随后其true→false不改原项目true。配置文件验证两个项目key／值隔离；英文菜单、中文勾选菜单及详情布局截图人工核对，滚动未自动化。
- toggle-assertions.json／project-assertions.json与main-restart-cleanup.json／other-project-cleanup.json记录功能、HEAD／status未变、自有测试进程退出及Git子进程检查；原用户Rebased／IDEA保留。启动／重启选中的默认首提交与Tab切换保留原OID分别检查，不把重启后默认首项当作版本选择持久化承诺。
- Windows release通过，3项既有layout_preferences专项通过（本轮未把它们声称为新增偏好全矩阵测试）；完整核心375项仍为历史全套证据。主SHA256 `97958b648d60c4f7cfd5cf74865f04aacc9865772881cc3bd00ca90fd7d8e07c`。未重测内存或数据读取时延，旧构建性能记录保持绑定，不宣称因隐藏详情已量化节省内存。设置写异常／路径别名／多窗口同步等不扩大成完整IDE属性系统；其余显示选项不放无行为占位。未新增提交。
- 当前手测包 `dist/GitReview-windows-x64-preview-20261018-history-options.zip`，18198215字节；CRC／manifest通过，Windows新目录解压核对3个exe与release／manifest一致、使用说明一致，见artifacts/history-options/package-verified.json。

### 连续审阅验收中的 Windows 剪贴板所有权修复（2026-10-18）

- 重新检查100文件压力入口，旧CombinedStress在默认独立Tab模式仅切列表选择，不能证明实际加载100个Diff；本轮改用原生完整正文复制断言，每次要求新的Git读取及子进程结束，再核对磁盘预期全文（仅规范CRLF）。仓库有10000提交，正常加载规模1000条；未把总提交数冒充当前加载数。100文件往返、20次刷新、10次换仓完整结果见后续记录，未通过前不记为性能验收。
- 旧97958b64构建在有效原生截图中，第42个文件file-040.rs全选正确但连续3秒剪贴板仍为空，证据artifacts/lifecycle-9795/capture-guard-run。先前同文件分离Ctrl+A/C的30次循环通过，不能将这项局部通过替代整个流程。独立Win32 100轮NULL／有效HWND对照中，NULL路径一次SetClipboardData错误1418、读取无Unicode格式；另有外部占用导致OpenClipboard失败，未混作相同错误。API参考：https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-openclipboard 。短40次探针实际为39次写入成功、一次OpenClipboard被占用，修正此前中间进度的粗略统计。
- vendor/gpui-pre-windows/src/clipboard.rs写入改为显式所有者HWND；platform.rs传入平台已有的常驻隐藏窗口handle，其生命周期结束时销毁。不新建窗口／缓存／后台服务，读取仍使用NULL检查；无所有者提前拒绝，不清空剪贴板。既有GlobalAlloc成功移交系统的释放规则保留。上游gpui-pre-windows 0.3.9 / zed@93d745668805029476e6d7fbb19652116fed51de，许可证及本地映射见THIRD-PARTY；没有改变Rebased入口／菜单／复制交互。
- 正确API接缝的Windows原生test_clipboard新增GetClipboardOwner断言：先将所有者参数接入但仍保留NULL打开，实际运行1项失败；修复后1项通过，并保留Unicode、metadata、NUL替换往返检查。日志artifacts/lifecycle-9795/clipboard-test-{red,green}.txt；首次只编译vendor测试未启用其依赖test-support，失败属于构建配置；最终同时选中root与vendor、root启用desktop-tests后构建成功。精确过滤曾运行0项，不算测试通过。Windows release成功，主SHA256 `3e97db796d8d86b4a7d5e3463372f266e73914fe4f0705347f3da616911da63e`。
- 截图有效性单独修复：scripts/windows-capture-validation.ps1检查采样全黑，capture-window.ps1拒绝将其保存为成功证据，另存.invalid.png且finally释放Bitmap／Graphics并撤销临时topmost。真实黑屏和此前有效原生截图、合成黑／Islands深灰四项检查通过，PS5语法检查通过。该检查仅排除采样全黑，不能代替人工内容核对或确认未被其他窗口遮挡；copy-probe-run明确标记NOT-ACCEPTED，不据此推断应用复制失败。没有自动解锁／操作用户窗口或滚动测试。

本轮交付与验收边界：

- 修复版Windows原生 `artifacts/lifecycle-clipboard/run` 已完成100个不同文件的199次查看与20次刷新，共219次全文复制精确核对；每次读取要求新Git工作并等待子进程结束。`partial-assertions.json`重新检查实际actions和clipboard-reads记录，不以运行意图代替通过证据。代表性截图forward-file-49／99人工核对文件身份、正文与语法高亮。首批1000条历史（源仓库总10000提交）、最大文件975000字节／13000行，正常独立Tab模式，无滚动自动化。
- 10×500ms稳定采样：初始小文件84.85–85.74MiB；看过全部100文件时，大文件100.30–104.64MiB；返程回到小文件88.68–89.48MiB；20次刷新后88.62–88.66MiB。各采样时Git子进程为0；前述阶段内核Job累计峰值150.45MiB，含从挂起启动纳入Job的Git子进程，GPU不含在内。GPU大文件阶段dedicated61374464／shared4096000字节，刷新后61374464／4554752字节另列。只证明这些有界阶段，未宣称无限长会话无泄漏或全发布性能通过。
- 完整串联流程在首次换仓时停止，10次换仓未验收。原生选目录自动化没有可靠选中目标，后续目录框实验仍未成功；这些运行不是产品复制失败，也不是换仓通过证据。正式windows-pick-folder.ps1已恢复原状，实验保存在明确NOT-ACCEPTED的诊断目录；下一步先读取实际原生控件状态，不再猜控件／焦点。部分阶段与完整串联验收明确分开，本轮不因此勾选全性能门槛。
- 新包 `dist/GitReview-windows-x64-preview-20261018-clipboard.zip`，18195634字节；主SHA256 `3e97db796d8d86b4a7d5e3463372f266e73914fe4f0705347f3da616911da63e`。ZIP CRC、第三方说明一致，Windows新目录解压核对三个exe与manifest／release一致、README一致，证据package-verified.json。未新增提交；完整核心375项仍是此前全套证据，本轮新增验证是Windows原生剪贴板回归及脚本检查。
- 最终cleanup.json核对两个临时仓库HEAD与原始fixture完整OID一致、status仍干净；自有lifecycle测试程序和其Git子进程已退出，用户Rebased／IDEA保留。未修改用户仓库的信任配置。


### 原生目录选择与连续审阅复测（2026-10-18）

- 目录选择脚本的实际失败原因已确认：原生shell先创建对话框／子控件HWND，再显示初始化后的窗口；旧脚本向不可见对话框提前输入。`scripts/windows-pick-folder.ps1`改为等待本测试进程可见的目录框，核对输入框／确认按钮可用，并在WM_SETTEXT后通过跨进程WM_GETTEXT逐字回读目标路径，再确认。没有更改产品打开仓库逻辑、没有增加键盘／复制重试。API依据：https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getwindowtextw 。本条取代上一轮“正式脚本已恢复原状”的当时状态。
- `artifacts/picker-inspection/visible-native`独立原生场景完成10次大小仓库往返，每次要求新的Git读取结束并精确复制首文件全文；readonly记录确认两个fixture HEAD／status未变、自有程序及Git子进程退出。稳定Private基线80.12–81.32MiB，最后85.23–85.67MiB；全流程Job峰值164.43MiB含Git，GPU另记。这只证明独立换仓场景，不替代完整串联。
- 完整串联第一次在第3次刷新后复制仍为哨兵，记录保存在`artifacts/lifecycle-complete/run`。第二次增加已绘制选区前置检查、仍只发送一次Ctrl+C，在文件往返途中因前台窗口遮挡保护中止，用户确认当时操作了其他窗口；`painted`不记为产品失败或通过。用户停止操作后再跑同一场景到`painted-r2`，15次全文核对通过，在第16个文件file-014.rs失败：选区已绘制，单次Ctrl+C后3秒内剪贴板仍为25字符哨兵，预期42890字符。截图确认文件身份和蓝色选区；这次没有前台保护异常，但不能据此排除输入丢失或剪贴板占用。`partial-assertions.json`明确完整流程未通过，刷新及换仓阶段尚未到达。
- 当前“哨兵未更新”与此前SetClipboardData失败后“剪贴板为空”是不同症状，原因尚未确认；没有用重复Ctrl+C跳过失败，也没有声称旧所有权修复已经解决所有复制问题。本轮程序／手测包仍绑定3e97db79构建，无产品代码更改或重新打包。PowerShell5目录脚本语法检查通过；不将这次部分数据作为完整长会话性能验收。未做滚动自动化，未新增提交。


### Windows 剪贴板短暂占用修复与当前构建复测（2026-10-18）

- 最小化复现：3e97正式构建只在同一个17字符小文件反复全选／单次复制，也出现旧哨兵未更新，已排除“必须换文件或Git读取才触发”。临时边界记录区分原生Ctrl+C、Diff动作焦点／选区长度和底层写入；诊断06466构建101次同文件及229次完整串联均通过，没有捕获自然失败原因。不能把日志改变时序后的通过当作修复，诊断构建不交付。证据`artifacts/clipboard-delivery/{same-file,traced,traced-full}`。
- 受控真实Win32回归确认另一个原生窗口短占用5ms会导致旧实现放弃单次写入、保留哨兵。按Rebased ClipboardSynchronizer.java／WindowsClipboardHandler已实读规则，写入最多获取5次；本地间隔5ms，成功前不EmptyClipboard，不缓存或排队重放正文；持续占用有界返回并保留旧内容，读取仍单次。源码commit／路径／本地映射／差异及许可见REFERENCE与THIRD-PARTY最新条目。上游warning通知尚未映射，保留原日志。短占用回归旧实现真实失败，修复后与所有权／Unicode／metadata／NUL共2项原生测试通过。首次用NULL holder不能建立预期互斥、一次fixture获取锁被外部占用以及编译所有权错误均保留为失败实验，不冒充正确回归的红／绿证据；最终holder使用真实HWND、布置锁有500ms获取上限。
- 已移除全部DEBUG-copy-delivery临时记录，产品无新轮询服务／窗口／正文缓存。Windows release通过，主SHA256 `60569835b32948346953807ac7be10d61fed05e853a8d2d621691b04fae4eae7`。完整核心375项仍是此前证据，本轮只重跑受影响的Windows原生接缝，没有声称重新跑全套。
- 无诊断正式构建`artifacts/clipboard-delivery/final`连续226次全文核对通过：100不同文件共199次查看、20次刷新、7次换仓。每次要求新Git工作结束、选区绘制后仅一次Ctrl+C；仅规范CRLF。第8次换仓等待20秒未观察到预期新Git工作／quiescence，完整串联未通过，原因未确认，未声称复制重试修复了所有自然偶发问题。增加超时现场采集后，当前相同构建独立10次换仓／11次全文核对在`repositories-final`全部通过，未复现超时；两段不能拼成一次串联验收，不重复无依据地刷测试求绿。
- 内存范围：源仓库10000提交，实际加载1000，100文件，最大975000字节／13000行；正式部分流程每阶段10×500ms采样（另有查询开销），初始87.62–88.46MiB，大文件102.85–106.89MiB，返程91.28–91.67MiB，刷新后91.29–91.66MiB；采样Git子进程0，GPU另记，刷新后dedicated61374464／shared4608000字节。完成阶段记录中的Job峰值最大169.82MiB含Git，不冒称失败后全流程最终峰值。独立换仓基线80.05–81.08MiB、最后83.00–83.98MiB，全Job峰值162.31MiB含Git，GPU最后dedicated61374464／shared2895872字节；该独立场景150／250MiB核对通过，整体发布及无限长会话仍未验收。partial-assertions／budget-audit从实际记录核对数量、内容和测量边界；大文件及返回小文件截图人工核对，不做滚动自动化。
- 当前包`dist/GitReview-windows-x64-preview-20261018-clipboard-retry.zip`，18198276字节。ZIP CRC／manifest通过，Windows解压核对3个exe与release／manifest、README及第三方说明；详情见package-verified.json。cleanup核对原始fixture HEAD／status、自有程序和已知Git子进程退出；保留用户Rebased／IDEA，未改用户信任配置，未新增提交。复制缺陷有实质修复与原生证据，换仓超时仍是未确认事项；保持完整目标与暂缓范围，不扩展新的细粒度专项。


### 当前预览构建的完整连续审阅与主要内存预算（2026-10-18）

- 超时现场`artifacts/clipboard-delivery/complete-capture`确认：第二次换仓已经显示正确大仓库Log，原生目录框已关闭，Git Job只有应用本身；超时发生在后续OpenDiff等待新Git工作，before／total同为787。不是目录框仍打开或Git子进程卡住。此前笼统“换仓超时”的阶段定位据此修正。未捕获点击当帧的焦点／禁用状态，不能进一步断言忽略点击的唯一原因。
- 验收入口`artifacts/file-ready/complete.ps1`在OpenDiff前确认测试窗口前台并等待首文件选中行实际绘制，再只点击一次。像素取自本构建、150%DPI固定布局，源代码`files_toolbar`的can_diff同样依赖有效selected_file；不是增加固定等待、重复点击或替换Git断言。Git子进程结束＋150ms原来不能证明界面已就绪。11次前置绘制检查均通过（采样约2.47–10.58ms），本次没有观察到长绘制等待，故不将通过夸大为已证实此前所有失效原因或产品换仓代码修复。
- 无诊断605构建原生完整串联`artifacts/file-ready/complete`：100个不同文件199次查看、20次刷新、10次换仓，229次全文逐字核对均通过（只规范CRLF）；每个操作要求新Git工作并结束，复制前确认选区，仅一次Ctrl+C。scenario-assertions、actions、clipboard-reads、first-file-readiness与独立budget-audit互相核对，不用意图／进程计数替代正确正文。代表性大文件与最终小文件截图已人工核对；未做滚动自动化。
- 源仓库10000提交，实载1000；最大975000字节／13000行。各阶段10×500ms采样（另有查询开销），初始86.24–87.89MiB，大文件101.56–111.82MiB，返程89.86–90.81MiB，刷新后89.86–90.23MiB，最终92.09–93.13MiB；10次换仓结束单点91.59／93.47／95.77／92.56／92.30／93.90／95.79／93.24／92.88／92.70MiB，没有本次范围内的逐次持续上涨。所有稳定采样Git子进程0；从挂起启动纳入应用及Git的全Job峰值168.81MiB，150／250MiB预算通过。GPU单独记录，最终dedicated61374464／shared4554752字节。本次约217秒有界场景，不称无限会话无泄漏，也不外推其他写操作会话。
- 同一605构建另外完成`artifacts/current-605-performance`的两项已有主预算：新profile欢迎页无Git子进程，10个稳态样本均53.20MiB、Job峰值55.56MiB，80MiB预算通过；10000提交实载、32父合并／32轨道、975000字节只读Rust Diff，稳态120.44–128.01MiB、全Job峰值184.84MiB，150／250MiB预算通过。截图确认10000 commits loaded与实际32轨道、正确大文件，全文复制与磁盘源一致。GPU另记：欢迎dedicated18210816／shared1146880字节，宽图Diff dedicated77762560／shared4325376字节。未将CPU预算与GPU相加或将稳态快照冒充峰值。
- 仓库HEAD／status未变化，自有应用及已知Git子进程退出；原用户窗口保留，非干净Windows／冷缓存测量。程序、许可证与现有预览包未变，仍为`60569835b32948346953807ac7be10d61fed05e853a8d2d621691b04fae4eae7`，无需重新打包或重复核心回归；未新增提交。当前固定规模的连续审阅与主内存预算证据已补齐；后续按接续顺序集中处理UI/UX与发布剩余项，不继续新增Project边缘专项或反复跑本轮通过的基线。完整像素、认证组合与清洁环境等门槛仍未全部验收，保持目标进行中。

### 主工具栏按运行中的 Rebased 校正（2026-10-18）

用户反馈外观奇怪后，实际操作其 Rebased 主窗口的项目、分支和齿轮菜单并保存原图；主窗口及 History for Selection 均保留。源码 commit／路径／规则／本地映射／保留差异已记入 docs/REBASED-REFERENCE.md“主工具栏原生对照”，原图索引 artifacts/rebased-live-toolbar/README.md。

本轮：标题栏项目入口合并为图标＋名称＋下拉菜单；打开项目与最近项目分组、名称／路径两行展示，用户目录缩写，Windows verbatim 路径去重及图标颜色身份统一；项目和分支文字调整为 13px normal、项目图标20px；正常分支使用 fork，精确 tag 使用标签图标及短名称，其余非正常状态用上游 Warning。欢迎页复用项目标识、齿轮14px（欢迎页本轮尚未原生验收）。不修改 Git 写操作或用户仓库配置。

Windows release 构建通过。中英文菜单／项目切换原生验证：artifacts/project-toolbar/native-r3-en、native-r3-zh；final tag 名称去除 refs/tags/ 后以 native-r5-zh 验证，SHA256 0be87feb724178c9a59e2c2070d68dc80a124f50b156f068cf108ac33f0fed3d。截图确认标签状态、项目菜单及1080物理像素窄窗顶部设置／窗口按钮仍可见；Log 过滤栏窄窗仍有裁切，未宣称全界面窄窗通过。菜单选中最近项目触发真实新 Git 查询并正常结束；专用临时仓库 HEAD／状态复验通过，自己启动的 app／Git 子进程已退出，cleanup-r3-*.json／cleanup-r5-zh.json 可查。r4 因输出目录预先存在被验证脚本拒绝，未启动应用，已用新目录 r5 重测。

功能入口已验证，完整分支菜单、分隔线／控件渲染、欢迎页等仍待视觉验收，不称像素级完成。本次未重做性能预算验证，既有605构建性能数据不转用于此hash。现有便携包仍是 clipboard-retry 版本，本轮只有原生 release 构建，未重新打包或提交。

### 顶部分支菜单 Commit 与工具栏预览包（2026-10-18）

继续现有功能／明显 UI/UX 主线，不推进暂缓功能。根据运行中的 Rebased 菜单及源码确认 Commit 是分支菜单首组动作、入口激活工具窗口并聚焦说明框。上游 commit 4aec35256d765c217aa0d662b82ac4e860c396fd；读取 GitDefaultBranchesPopupStep／GitBranchesPopupActions／backend XML 分组、AbstractCommitChangesAction／CheckinActionUtil／ChangesViewCommitPanel，规则与本地映射详见 docs/REBASED-REFERENCE.md 新条目。

`src/ui.rs` 将单动作可见状态换成筛选后的动作列表，Commit 与 revision 共享搜索、选择、上下／Home／End 导航及 Enter；入口复用既有 open_local，`src/local_view.rs` 聚焦既有说明框，草稿不清空。未新增 Git 提交执行路径或常驻视图；不放尚无行为的 New Branch。精确间距、快捷键提示及其余完整分支菜单差异继续保留。

Windows release 编译通过；`cargo test --no-default-features branch_` 的32个匹配用例通过（分支／导航／搜索及相应仓库测试），不是新增全量回归。原生英文鼠标打开、聚焦／输入、搜索 Commit 后 Enter、revision 搜索弹窗与 Esc、Home／Enter导航，中文搜索“提交”及重复进入后保留草稿，均已查看截图：artifacts/project-toolbar/native-commit-entry-r2-{en,zh}。首次 SendKeys 受系统 IME 候选干扰，不能作为搜索验收；重测改用限定自有 HWND 的 WM_CHAR，未改用户全局输入设置。临时仓库 HEAD／状态复验未变，自己的 app／Git 子进程均退出（cleanup-commit-entry-r2-*.json）；未关闭用户 Rebased／IDEA。

新包 dist/GitReview-windows-x64-preview-20261018-toolbar.zip，18,208,501字节，主程序SHA256 48e0ba8dbd26cc84836eae1a551a4275595a77cdcec4d1a33ca865731a20e7f1。包含上轮项目菜单／状态图标与本轮 Commit 入口；Windows Expand-Archive 和三个exe哈希／资源来源检查见 package-verified.json。原生交互运行与包内主程序同hash；本轮不重跑未受影响的完整性能压力，605构建历史数据仍仅归属605。未额外提交，完整视觉与发布验收仍未完成。

### 菜单／弹窗边界样式统一（2026-10-18）

沿用现有 UI/UX 修复主线，未扩展功能。本次核对 ManyIslandsDark.theme.json 的 Menu／Popup 颜色链及 DarculaMenuSeparatorUI 的1px条纹规则，参考4aec35256d765c217aa0d662b82ac4e860c396fd，具体路径／规则／本地映射／差异见 docs/REBASED-REFERENCE.md“菜单与弹窗共用边界”。

修正原因：主页面分隔线颜色被用作控件／菜单分组边界，且框架浮层轮廓另取文字10%透明色、分组线2px，导致不同浮层的线宽与对比不一致。src/ui.rs 将页面 BORDER 与 POPUP_BORDER 分开，后者采用上游 #33353B；theme.border、自绘分支弹窗边界和分组线统一；vendor/gpui-component 的 popup_menu.rs 改1px分隔，styled.rs 的既有单层轮廓读取主题border，不另加第二圈。无数据加载／缓存／写操作变化。

Windows release构建通过，主程序SHA256 8e0198dfbb50eccf0939e664342c53862bd929f22fe4aacede4e10ce91a458bb。artifacts/project-toolbar/native-menu-surface-en 已查看项目菜单／分支菜单／revision-dialog／settings-click 截图：菜单分组线清晰，弹窗外缘未叠加第二圈；项目／分支菜单关闭、revision搜索打开及Esc取消完成。settings.png 为 CtrlAltS 尝试未打开的主界面，不能作设置截图；settings-click.png 才是鼠标打开的设置。设置之后的脚本因自有窗口标题被遮挡而拒绝继续发送输入，安全退出；不宣称设置关闭／整套集中交互通过。cleanup-menu-surface.json 确认专用app及Git子进程退出并记录临时仓库状态。未关闭用户Rebased／IDEA。

这是共用样式调整，未新增镜像样式测试，也未重复完整性能测试。保留GPUI阴影／圆角与字体差异，中文及其他DPI视觉仍待集中验收。当前可手测zip仍为48e0工具栏版本；本次只更新release构建，未重新打包／提交。

### Windows 系统字体、设置关闭及最近项目回归（2026-10-18）

按现有UI/UX主线核对Rebased JBUIScale／IdeaLaf／AppearanceConfigurable，Windows默认使用系统消息框字体；本地主窗口、欢迎页、metadata及包含分支TextRun之前硬编码Inter，现由src/ui_font.rs统一读取并缓存有界名称；代码仍JetBrains Mono，非Windows保留Inter。源commit／路径／规则／实现／字号与fallback差异见docs/REBASED-REFERENCE.md“Windows UI 系统字体”。宿主机采样为Microsoft YaHei UI Regular 9pt（artifacts/system-font/host-font.json），本地本轮只改字体族、保留13逻辑px，不宣称精确字体／DPI验收完成。

Windows原生英文与中文的项目／分支菜单、搜索Commit、Enter打开、说明框焦点和输入、中英文设置打开／Esc关闭已完成并查看截图：artifacts/project-toolbar/native-system-font-{en,zh}，交互构建9472cd62339a148993f8ce3cbe86d68d494605867c1622a074b1ff71d96c5eac。上轮设置关闭因遮挡而未验收的项目已补验；未验证所有快捷键和更改后Apply／Cancel组合。本轮窄窗仍暴露既有过滤栏与右侧详情裁切，已保留，不能宣称窄窗布局通过；字体切换后的普通全窗未见该裁切。自己启动的app／Git子进程退出，专用仓库HEAD／状态复验通过。

欢迎页首次检查发现普通／verbatim Windows路径导致同一仓库重复。真实recent-file读取用例修复前5项而非2项，修复后2项，红／绿结果见artifacts/system-font/recent-{red,green}.txt。统一identity到recent_projects的decode／opened／remove，UI菜单／配色也委托同一入口；加载不扫描／canonicalize离线仓库，保留20项／64KiB预算。Windows最近模块2项、Linux原有1项通过，非全量核心回归。源码RecentProjectsManagerBase.loadState对照及默认大小写身份等差异已记参考文档。最终构建651c0d9eb615503a0de959cfaac80b184c78fec4218a2444061f75a5fbd19461使用原样含重复记录配置原生重开欢迎页，window.png确认只剩2个项目，原配置字节未改；见artifacts/system-font/welcome-final及welcome-final-cleanup.json。

新预览包dist/GitReview-windows-x64-preview-20261018-system-font-recent.zip，18,212,285字节，主程序SHA256为最终651c构建；包含上轮浮层边界样式。Windows解压、三个exe哈希、说明与资源许可检查通过（artifacts/system-font/package-verified.json）。中途9472候选system-font.zip保留为证据，不是当前交付包。未额外提交；没有重复既有完整性能压力，605构建历史数字不转用于此包。源码及记录已通过git diff --check。


### 窄窗布局与筛选子弹层验收（2026-10-18）

已对照Rebased Splitter／MainFrame／AutoLayoutStrategy／ActionToolbarImpl，路径、规则、本地映射及差异见docs/REBASED-REFERENCE.md。修复中央最小宽度之和超出实际空间造成右侧详情／按钮裁切，搜索允许收缩，筛选条件进入溢出入口。原生排查发现Paths Select关闭父菜单后编辑器消失，增加可选子弹层命中保护，User与Paths补全／编辑继续复用原实体。

674e6176构建的Windows原生英文／中文记录在artifacts/project-toolbar/native-narrow-r3-{en,zh}，已核对Paths Select输入main.rs并应用／X清除、User Select输入Reviewer并应用、外部点击关闭、宽窗恢复全部筛选且保留条件；中文HEAD筛选应用、展开分支树及宽窗恢复通过。英文R2另验Tab后Enter打开分支子菜单、日期预设应用。宿主请求1080px宽时实际PNG为1102px（Windows窗口最小尺寸／框架约束），记录以实际截图为准；固定fixture分隔线扫描原148px详情宽度失败，修复后三栏展开192px，通过记录见artifacts/narrow-layout/{red,green-zh-tree}.txt。此扫描只确认该布局的可见空间，不替代全部文字与DPI验收。

R1点击清除的脚本坐标错误、R2等待期间超时均由测试Job退出清理，不作成功证据；R2实际发现的子编辑器消失已在R3补验。cleanup-narrow-r3-{en,zh}.json确认仅自有app及Git子进程退出、临时仓库HEAD与清洁状态复验，保留用户Rebased／IDEA。溢出列表结构、阈值、完整键盘导航与上游仍有差异，已记录；未新增功能范围或重跑完整性能压力。

### 项目字母图标字体与留白（2026-10-18）

按用户反馈先修复图标内双字母贴边。源码AvatarUtils明确New UI字母用独立JetBrains Mono demi-bold，之前错误继承了系统UI字体。src/ui.rs统一显式字族／半粗字重／整数比例字号，并加四周10%留白、独立居中行框及边界裁剪；20px顶部／项目菜单和24px欢迎页共用。内嵌上游原版SemiBold字体，来源与OFL已保留。规则、源码路径、AWT与GPUI基线差异见参考文档，未把padding适配声称为上游原样算法。

Windows release构建通过，主程序SHA256 581228d85b9cbab9b78fc85098fe23fc9c78c763ebd86cceffa3e383951a1698。原生顶部与中文欢迎页已查看：artifacts/project-toolbar/native-avatar-r1-en/project-closed.png、artifacts/project-avatar/welcome/window.png，TM／TT双字母有明确留白、没有贴边。welcome-cleanup.json确认原最近项目文件字节未改、自有窗口退出。项目菜单已补验：artifacts/project-toolbar/native-avatar-r4-en/project-menu.png，两行图标留白一致，Esc关闭，cleanup-avatar-r4-en.json确认专用app及Git子进程退出／临时仓库不变。R1—R3截图未打开菜单，未作为菜单通过证据；R4先完成窗口捕获激活再点击，避免抓图准备关闭已弹出的菜单。仅对本轮头像做可见样式检查，没有镜像样式测试；特殊Unicode／其他DPI与全界面视觉验收仍待继续。

本轮预览包dist/GitReview-windows-x64-preview-20261018-avatar-layout.zip，18,408,391字节，主程序为上述581228构建，同时包含已验收的窄窗修复。Windows解压、三个exe哈希和字体／图标来源与许可证检查见artifacts/project-avatar/package-verified.json。没有额外提交或重新宣称完整性能压力通过；既有性能数据仍属于原构建。


### 256轨道存储修复与宽图渲染反证（2026-10-18）

按现有性能主线补数百轨道，而非新增产品功能。独立临时fixture为10000提交、256分支并行／256父merge、975000字节Diff。旧581228构建graph regression失败、原生Log为空错误页；GeometryPool按完整切片内容共享重复行，只计唯一物理存储，500000段／512轨道上限不变，行、边、颜色与虚线均保留；过滤路径也接入。源码对照／差异见参考文档最新条目。

Linux核心377项通过（原375+256lane回归+共享释放回归），原生Windows release构建通过。新0a721457a679e044f0c0c94e4c44f220b96bf174adc6c0fd0d34ee5c12371686构建真实加载10000行、可打开并全文复制核对975000字节Diff。artifacts/wide-256/native-green稳态10次Private Bytes为135.53–140.21MiB，采样直接Git子进程为0；从挂起启动纳入的Job峰值238.17MiB，包含Git，不含GPU，低于原150／250门槛。独立GPU Dedicated约96.03MiB、Shared约4.84MiB，详见steady.json。非冷缓存／非干净Windows，不证明长期增长、所有拓扑或全部反馈延迟。readonly-green.json确认HEAD／status不变、自有app及Git子进程退出。

**宽图视觉验收未通过**：用户指出当前效果不合适；应请求实际用Rebased打开同一fixture，收起Project截图artifacts/wide-256/rebased-default.png。上游按行图形预留文字宽度，长边默认以箭头缩略中间段，作者／日期列独立；本地仍把全窗最大256lane宽度用于每行，顶部线性记录也看不到文字。完整源码路径／常量／规则已记录；下一步优先修复此渲染结构，再更新预览包。共享存储修复保留，不能以加载成功和内存通过代替UI验收。本轮不发布0a721候选，当前手测包仍为581228的avatar-layout；未额外提交。

Windows原生graph／filtered_graph专项14项通过，输出artifacts/wide-256/windows-graph-tests.log。未修改用户仓库内容；Rebased保留在本次压力仓库Log，便于用户自行查看。


### 宽图默认显示按Rebased修复（2026-10-18）

承接用户同仓库截图反证，src/ui.rs已把graph／标题／refs放入Commit单元格裁剪，作者／日期独立；src/graph.rs按当前行实际node／edge中点计算文字留白，取消全窗最大轨道对所有行及整表的撑宽。随后新增src/graph_print.rs翻译默认长边30／1的可见投影、相邻位置映射、DFS layoutIndex排序及箭头；Git父关系／原始分页frontier保持完整，过滤后的虚线也进入同一显示路径。计算仅保留相邻三行位置表，不复制全部轨道矩阵。上游commit、全部路径、提取规则、本地映射和边界差异见参考文档“按行Commit单元格与长边显示投影”。

原版Rebased manyNodes／longEdges测试fixture原样保留并注明许可证；按照原测试7／2／10策略，逐node位置及逐edge端点／虚线／箭头比较通过，不比较测试专用颜色或输出顺序。其他覆盖默认／展开策略、短边、边界、取消、39组随机DAG的过滤／分页位置、真实父关系恢复及10000行256轨道。初始测试曾错误要求所有行都变窄；源码及原生参考都证明合并端点箭头行仍可很宽，改为断言全部端点保留、内部区段正确压缩，未提高任何数据／内存上限。

最终C289构建release通过，Linux完整核心382项通过，Windows原生graph／filtered_graph／print专项19项通过。artifacts/wide-row-layout/native-print/history.png已查看，顶部线性提交文字恢复、作者／日期可见、长边箭头及后续分支标题结构接近Rebased同仓库截图（artifacts/wide-256/rebased-default.png）；窗口尺寸不同、间距／颜色／节点密度仍有差异，不声称同尺寸逐像素通过。native-print/content-verified.json核对975000字节Diff全文，清除选择后的window.png保留真实源文件预览。native-filter/filtered.png确认Post merge为14条，清除后restored.png为首批1000条及Load more，遵循现有重置筛选回首批规则，不能把它写成恢复了10000条。

最终主程序SHA256 c289e2e4d2e7d488d08432f4c0c1b38ca77b4fefdb06e1b2d6141298cb26918c；10000提交／256轨道／975000字节Diff的10次Private Bytes采样为135.16–140.71MiB，采样直接Git子进程均0；从挂起启动起Job峰值203.06MiB，含Git、不含GPU。GPU Dedicated约86.71MiB、Shared约6.75MiB，独立记录steady.json；非冷缓存、非干净环境、不外推长期增长或全部操作。readonly-{print,filter}.json确认仅自有app及Git子进程退出、fixture的HEAD／status不变；保留用户Rebased，未做滚动自动化。之前0A721及F319数据不冒充最终构建证据。

保留差异：推荐留白未移植上游永久图加权采样；跨窗两端未知的贯穿连接保留原线，不猜测边长；布局heads依据当前窗口而非IDE永久图分支优先服务。Show Long Edges策略已在核心测试但可见入口未接入，箭头点击导航未接入；文件历史graph尚未切到同一print投影。以上继续在现有UI/UX范围处理，完整视觉与发布验收未完成，未额外提交。

新预览包dist/GitReview-windows-x64-preview-20261018-graph-print.zip，18,424,727字节，主程序为C289构建。Windows新目录解压、三个exe哈希、字体／图标来源及Graph print许可说明已检查，见artifacts/wide-row-layout/package-verified.json。当前交付仅包含已验证的主Log默认投影；内部Show Long Edges策略不等于已有可见开关。


### Long Edges入口、持久化与屏外绘制（2026-10-18）

收尾上轮已列出的开关，不扩展其他显示功能。已读ShowLongEdgesAction／BooleanPropertyToggleAction／VcsLogToolbarPopupActionGroup、VcsLogUiPropertiesImpl、CommonVcsLogUiImpl及PresentationSettings分组，默认false、按Log属性保存、空图禁用、改变仅重绘。源码commit／路径／规则／映射及保留差异见参考文档最新条目。实际操作用户Rebased同fixture并恢复Long Edges关闭，确认Toggle后菜单保持打开；保留原进程，没有Git写操作。

src/ui.rs显示选项眼睛菜单提供Long Edges／长连线，勾选实时更新、确认不关闭、Esc关闭；空图／在途禁用。graph_print::Model缓存紧凑拓扑，独立后台重绘只换显示rows，不重新查Git或加载Diff；refresh／切仓取消和Model身份门禁防迟到。设置按项目写app data、Windows身份统一，失败不改变当前状态。新增raw geometry释放／切换等价及偏好身份／损坏边界回归；Linux库170项、Windows相关24项通过（graph命令21含一个偏好项，另一个布局命令4有一项重叠），没有把上轮382项全量核心回归冒充本轮重复执行。

D1候选原生英文切换、中文重开保存值／取消通过，见artifacts/long-edges-toggle/native及reopen，但无Diff展开采样141.62–152.69MiB，不计作稳定150预算通过。增加content mask屏外整段路径剔除，穿越区段及箭头余量保留，没有删除拓扑或降低存储上限；随后按Rebased原生行为补保持菜单打开及实时勾选。最终B312构建SHA256 b3122774e0079b287b46516eae1598f504aa4bb800b46b54ddb44beab6703e35。

最终native-r3/live-checked.png证实鼠标切换后菜单仍显示新勾选，raw-enter.png证实单独Enter再次启用；collapsed.png／unchecked.png与expanded-final.png确认双向显示。组合SendKeys Down+Enter最初仍为未勾选，未计为通过；拆分Enter后成功，不据此宣称任意快速组合均通过。脚本动作newProcesses均0，主选择及详情未变；raw工具仅给已核对exe／前台HWND输入并直接截图，避免截屏准备改变弹层。展开图10次Private Bytes均139.29MiB；此前候选数据不冒充最终版本。

最终同配置重开，中文budget-expanded/history.png为已保存展开状态下的10000提交；window.png及content-verified.json核对975000字节Diff全文。10次Private Bytes为134.02–146.96MiB，直接Git采样均0；Job内核峰值211.45MiB，包含Git不含GPU。GPU Dedicated约86.69MiB、Shared4.38MiB单独记录；原150／250门槛未改。证据位于artifacts/long-edges-toggle/budget-expanded。非冷缓存、非干净Windows、不证明长期无增长／全部发布条件；readonly-r3／readonly-budget确认临时仓库HEAD／状态不变、仅自有应用与Git子进程退出，保留用户Rebased，未做滚动自动化。

开关现已可用；箭头点击导航、文件历史graph共用投影及完整视觉／发布验收仍未完成。未新增提交。


### 菜单生命周期收尾与同仓库视觉复核（2026-10-18）

PopupMenu 自身持有实时观察 Subscription，关闭菜单立即移除回调，不等待 Review 下一次 notify。Windows 原生 tests/menu_subscription_release.rs 1项通过，owner 保持存活但不 notify，回调捕获资源仍释放。正式 desktop 特性 release 重新构建，与 desktop-tests 构建分开；本轮包为 long-edges-final.zip，18,450,553字节，SHA256 0f8b617d015901383227f6c4486364dcd6e017bac3699954f9d69bd9f32e245d。中间 lifecycle.zip 为测试特性构建，不是交付包；之前 B312 证据继续绑定其自身构建。

原生同仓库截图基准为 artifacts/rebased-live-toolbar/long-edges-final-restored.png。Rebased 默认长边箭头、逐行文字留白及独立作者／日期列均已确认；其首屏下半部的分支头较快向左收拢，而本地 native-r3/collapsed.png 仍有较多贯穿线及被裁文字。不同窗口／列宽／行数尚未逐行统一，轨道排序、颜色、线宽与分支标题可读性仍待修复，不能称像素验收完成。

最终包 budget-final3 完整复制校验975000字节Diff通过；10次 Private Bytes 稳态最大153.65MiB，超过150MiB门槛，Job峰值217.40MiB（含Git、不含GPU）低于250MiB。本次内存验收未通过，不调高预算，也不以B312的134–147MiB替代。budget-final 首次未打开Diff，全文校验拒绝；final2在输入前构建哈希拒绝，两者均不是有效性能结果。最终测试文件／截图和manifest保存在artifacts/long-edges-toggle；此预览可用于功能手测，完整内存与视觉仍待验收。未额外提交。


### 长边双向验收更正与几何内存收缩（2026-10-18）

上一轮把 native-r3/collapsed.png、unchecked.png 当作默认模式的证据有误：unchecked.png仍有勾选，因此“默认轨道排序导致标题不收拢”的判断不成立，也不算双向切换通过。未据此擅改轨道算法。本轮 artifacts/long-edges-state-audit/native-v2 使用原始截图及分离Down／Enter，等待真实保存值，验证false→true→false、实时勾选、Esc关闭和零新增Git子进程；最终collapsed.png有可读分支标题，与Rebased默认状态的结构一致。完整视觉仍待验收。native初次HOME没有导航绑定，保存值断言拒绝，未记作通过。只读与自有进程清理见readonly-v2.json。

定位到展开模式独立几何276484段、每段32字节，占8.44MiB。src/graph.rs以内部紧凑坐标及位标志改为16字节（4.22MiB），完整颜色身份及拓扑／绘制规则保留。上游commit／类与提取规则、本地映射及差异详见参考文档同名条目。sizeof回归先红（32字节）后绿；保留全样式、原版上游fixture、随机DAG、边界及实际Git用例。Linux完整核心386项通过；没有把仅库172项充作完整核心。

Windows release SHA256 d72cb27e7679d06d3acd1c04388adb52d2b8ba5f985f9187caae5f17cb3cd379。budget-compact为10000提交／256轨道、已保存展开状态、975000字节Diff；全文复制核对通过，10次Private Bytes 135.93–149.09MiB，直接Git采样0；Job峰值212.71MiB，含Git、不含GPU。原150／250门槛通过，未延长等待或改变采样次数；上轮0F8B的153.65MiB失败仍保留。readonly-compact确认临时仓库HEAD／status不变、自有app与Git均退出，用户Rebased保留。该宿主暖缓存单次场景不能证明所有拓扑、长期无增长或干净Windows正式发布验收。箭头点击跳转、文件历史共用投影及其他主线继续保留，未新增提交。

本轮 D72C Windows graph 专项23项通过；native-compact重复验证英文双向切换、菜单实时勾选与项目保存值一致，零新增子进程。查看collapsed.png确认默认分支标题收拢；中文展开及完整Diff见budget-compact。本轮预览 compact-graph.zip，18,450,956字节，哈希为本节D72C；Windows解压和三个exe及来源检查记录package-verified.json。完整视觉验收与所有发布门槛保持未完成。

compact-graph 包 Windows 解压验证已通过（artifacts/long-edges-state-audit/package-verified.json）：三个exe SHA256一致，字体／图标来源、Apache改编声明与使用说明齐全。菜单生命周期订阅的上一轮专项仍按其0F8B构建记录，本轮未重跑未修改的桌面订阅专项。


### 主Log长边箭头导航（2026-10-18）

按现有主线接入箭头实际行为。实读同一4aec352的VisibleGraphImpl.kt::performArrowAction、SimpleGraphCellPainter.kt命中规则、GraphCommitCellController.java以及VcsLogGraphTable.java::jumpToGraphRow；路径／规则／本地映射／差异见参考文档最新条目。上下方向分别取真实投影边端点；普通节点和较早绘制边优先命中，不用斜线外接矩形当点击区。主Log可见箭头悬停有手形／目标摘要，点击不先选源行，使用现有select_commit、History焦点与Nearest定位，不清筛选或查询另一套导航数据。切窗／重绘中的旧回调用Arc身份及loading门禁拒绝。

导航目标不放入共享线段；最初方案导致256lane的500000几何预算测试失败，已改用每行可空稀疏索引。Segment仍16字节，Row多一个可空Arc描述符，只有有已知箭头目标的行拥有小数组。共享几何但不同目标的回归验证导航身份互不污染；原版上游fixtures、过滤真实目标、UP／DOWN／expanded及unknown frontier均覆盖。Linux完整核心389项通过，Windows graph专项26项通过；sizeof及5MiB几何压力上限保持。

原生命中曾失败：测量canvas未指定四边，默认静态位置落在绘图canvas后，命中矩形低28逻辑px。probe trace明确给出event y162.67与错误bounds y168；改为控件内部absolute＋inset_0后通过。临时GITUI_ARROW_TRACE代码已移除；artifacts/graph-arrow-navigation/hit-trace.txt为诊断历史，不属于正式产品日志。初始脚本别名Move及一次误点节点均被断言拒绝，没有把复制出的源行ID记成成功。

最终release SHA256 2330e04b679bf27bd6069d3793f5c54b95eb2c68d42bd2810d5a5728e6426daf。100提交临时fixture的native-v5：HEAD向下箭头到root，再由上箭头返回HEAD；完整复制ID分别与真实Git root／HEAD核对，down-tooltip／up-tooltip及root-selected／head-selected已查看。没有发鼠标滚轮或继续滚动自动化排查，定位是本次点击导航的实际结果。1100提交／加载首1000的boundary-native中文tooltip说明目标不在窗口，点击保留原HEAD ID且newProcesses=0；不是错误横幅，也不自动清筛选、加载更多或假造目标。两套readonly记录确认HEAD/status未变，仅自有app与Git退出。

同最终构建budget为10000提交／256轨道、已保存展开模式与975000字节全文Diff复制核对；10次Private Bytes 131.52–144.61MiB，直接Git采样0；Job峰值207.29MiB（包含Git、不含GPU），原150／250门槛通过。GPU Dedicated约86.69MiB／Shared4.38MiB单记steady.json。不是冷缓存或干净Windows，不以单次波动宣称又节省固定几MiB。用户Rebased保留。

主Log已加载目标的箭头跳转可用；图外frontier缺上游永久图targetId，只给通用说明，未显示准确hash／元数据；hover尚未移植整条边的黑轮廓高亮，文件历史显示投影／箭头交互及全部视觉验收仍待推进。所有精确视觉差异继续记录，未新增提交。

本轮 graph-arrows.zip 为18,450,578字节；Windows原生解压、三个exe哈希及字体／图标来源／Apache声明检查通过，记录 artifacts/graph-arrow-navigation/package-verified.json，哈希2330E04B与上述原生功能／预算场景一致。未额外提交。

## Plico、时间与项目窗口增量（2026-10-18）

本轮范围固定为用户提出的品牌／图标布局、提交日期、新窗口和状态栏调整，不另行扩展功能清单。源码依据与保留差异见 docs/REBASED-REFERENCE.md 的同日记录。

- [x] 名称改为 Plico：主窗口／欢迎页、Windows 产品元数据及便携包入口；使用用户 SVG，保留原图，去除外层黑底，生成多尺寸 Windows icon。Cargo 构建目标及旧 GitReview 配置目录继续保留，已有设置不丢失。
- [x] 顶部应用 logo 与项目徽标分开，logo 固定区域中心对齐左侧栏按钮，工具栏在标题栏垂直居中。继续沿用 Rebased 工具窗口／文件图标，不替换为应用 logo。当前只确认默认 Windows 比例下结构；多 DPI 精细差异未宣称像素级完成。
- [x] 主 Log／文件历史显示作者日期和时间，使用本地时区、今天／昨天与最近时间段，中英文共用规则；跨年昨天、未来时间、时区转换及分钟舍入有确定性测试。
- [x] 已有项目时，选择另一个项目提供当前窗口／新窗口／取消，回车确认当前窗口。新窗口使用同一 GPUI 运行时和独立 Review；欢迎页直接进入当前空窗口。没有增加 Attach、搜索或其他未实现入口。
- [x] 移除 Log 常驻提交数量、偏移和历史已到末尾文案；部分历史／shallow 提示进入左下状态栏；真实加载／历史窗口导航保留。

功能与结构验证：Windows 原生 `artifacts/plico-branding/native-verified` 记录 Today／Yesterday／较早日期与时间、项目选择弹窗、同一 PID 两个原生窗口以及关闭一个后另一个仍能浏览项目文件；`windows-resource.json` 与 associated-icon.png 验证 PE 产品名称与实际提取图标。测试仅触碰自建验证仓库／测试配置，保留用户 Rebased。构建候选与最终版的区分、中文验证及发布包 hash 在本节后续验收记录中注明。

文件历史图收尾：`file_history.rs` 接入与 Main Log 共用的默认长边打印规则，`history_view.rs` 使用共享箭头命中／跳转并修正表头布局，原 parent links 继续用于比较。Linux 393 项核心用例（含 3 个日期规则测试）与 Windows release 编译纳入本轮；文件历史新增核心用例在此前 Windows 13 项原生 Git 测试通过。`artifacts/file-history-graph-print/native-verified` 在 Plico 候选上确认文件历史 down→root／up→HEAD 的实际预览内容，Main Log 往返通过完整 OID 复制核验。删除 28px 底部调试条后，自动检查的 Main Log up 箭头坐标随视口底端调整，初次旧坐标失败记录保留，调整后的复测通过；没有继续做滚动自动化排查。

低内存证据边界：此前文件历史实现候选 EF7A7D… 的主窗口 10000 commits／256 lanes／975000 字节 Diff 采样 Private Bytes 131.75–144.90 MiB，Job peak 207.28 MiB（含 Git，排除 GPU），保留在 artifacts/file-history-graph-print/budget。该结果不冒充新 Plico 最终包或多窗口内存门槛；本轮未扩展内存验收范围。整个计划尚有原已列出的视觉／发布验收项，不将本轮功能完成等同于全部计划完成。

最终验收：`dist/Plico-windows-x64-preview-20261018.zip`，18,409,136 字节；exe SHA-256 `a9d986e1febbb8421e08251b3f62e5ab6fc701dfcae84cf73c7f521e29104cfd`，zip SHA-256 `20212009ed7b4b4e8861083b4c8749b13d355c28c5c82a14fc6ddf7a297ddd46`。393 个 Linux 核心测试全部通过，Windows release 构建通过。Windows Expand-Archive 解压后的主程序与两个 helper hash 均与 manifest 一致，产品元数据为 Plico，系统提取图标正确，解压包原生启动通过。1100 条 fixture 加载 1000 条的 `artifacts/plico-branding/packaged-native/history.png` 已确认“加载更多”继续可用，部分历史提示位于左下，数量／offset 没有常驻。最终二进制 `native-zh-verified` 确认中文欢迎页→最近项目→中文日期，仓库 HEAD／status 不变。

构建证据区分：双窗口与文件历史箭头完整验证使用 `5b23b4e3…` 候选，最终 `a9d986e1…` 只再修正 UtilBundle 的 Moments／A minute／1 hour 消息分段，窗口、图形和布局实现保持相同；对应最终版核心测试、中文界面及 zip 启动独立通过。早期点击因缩小截图导致坐标错误、测试框架未允许欢迎页导航等失败日志保留；框架现在允许显式 Welcome Scenario，而空欢迎页无场景时仍要求没有 Git 子进程。所有本轮测试进程由各自 Job Object 清理，未关闭用户 Rebased／IDEA。功能及默认 Windows 结构验收完成，精确多 DPI／像素级视觉和已有发布门槛仍待后续，不新增功能细分任务。

### 现有箭头悬停反馈收尾（2026-10-18）

沿用现有功能／明显 UI 问题主线，补齐已记录的 arrow hover 轮廓，不新增功能清单。源码核对发现旧记录“黑轮廓”不准确：JBColor.BLACK 深色变体来自 Label.foreground。`ui.rs::graph_cell` 复用 hovered segment，按未选元素→浅色加粗轮廓→原彩色线／箭头顺序绘制，与提交行选择保持独立；主 Log／文件历史共享。未增加图数据、缓存或 Git 查询，保留普通描边几何与 Java2D 设备像素对齐差异，完整源路径／规则／差异记录在参考文档。

Windows release D132EF692DBF530E1FB9D5885444CE82725DD2D99F6C89069809782F347085D0 编译通过。`artifacts/graph-hover-outline` 原生截图确认两处悬停／移开，限定箭头区域亮色像素分别38→0和40→0；文件历史 hover-only 无新增进程，root／HEAD 预览文本与主 Log 完整 OID 双向导航通过，HEAD/status不变、自有Job退出。没有进行滚动自动化，保留用户 Rebased。此变更不影响数据层，未重复完整393核心或内存压力，将之前数据与本轮证据分开；DPI描边精度和其他既有视觉／发布验收继续保留。首页手测包／多窗口状态已从旧 graph-arrows 信息同步为 Plico，不重新实施已有功能。

本轮手测包 `dist/Plico-windows-x64-preview-20261018-hover.zip`（18,486,831字节），exe D132EF…；Windows原生解压、主程序和两个helper哈希、Plico资源及启动已检查，`artifacts/graph-hover-outline/package-check.json` 记录zip SHA-256 `ac2d899f3e39eca1dc1aa00e3fbbfe07ea90afc0fe3dd575219a37318df40721`。本轮仅补当前箭头反馈，其他原有视觉与发布门槛保持原范围，未新增提交。

### HEAD 节点和行背景收尾（2026-10-18）

继续现有graph默认显示集中修复：Main Log／文件历史原先只有普通实心节点，现按Rebased GraphTableModel／HeadNodePainter显示真正HEAD的外圈和中心，中圈用实际行背景。完整参考路径、规则、本地位置和几何差异见参考文档最新节。没有把过滤首条当HEAD，没有改变共享图模型；ScopedHistory复用既有HEAD解析结果，不多启动Git查询。

新增真实Git用例检查文件历史首条不是HEAD、游离HEAD在较早行和HEAD过滤，Linux／Windows各14项通过。Windows release BDBF8D058C5FBE3B0196E256D1B3B5C7905F69933DDAB1ABF9F36A5E0E893FD0 构建通过。`artifacts/graph-head-marker/native` 已核对四种行背景下的圆环，marker-pixels.json限定区域检查通过；箭头悬停与双向导航继续通过。用户运行中的Rebased已截图、保留，截图fixture不同仅用于行为确认，不当同条件像素验收。既有全面核心393项与内存数据继续绑定原构建，本轮仅运行受影响的文件历史专项，不重复扩大验证。

本轮手测包 `dist/Plico-windows-x64-preview-20261018-head.zip`（18,492,421字节），主程序BDBF8D…；Windows解压、三个exe哈希／Plico资源及原生启动通过，包SHA-256 `b818f405472df40f1910a01a002ae26e6b213a27778aba634ba9f08885806f19`，证据 artifacts/graph-head-marker/package-check.json。未扩大内存／功能验收范围，也未额外提交。

### 多窗口语言一致性与自然退出（2026-10-18）

按集中常用流程验收推进，发现并复现另一窗口搜索提示不随全应用语言变化：其他标签已中文但Text or hash仍英文。`ui.rs`与`local_view.rs`对保留的搜索／提交说明输入框增加一次性语言同步；现有输入对象和值／选择不重建，只有实际语言改变才更新，没有新增后台查询／订阅。上游LanguageAndRegionConfigurable要求重启，与本地F1即时切换的有意差异已记录。

Windows release 049297377d612f5f5a82863c382522764f289d2797f95985fda973bf8effc17c编译通过。artifacts/multiwindow-review/verified确认两个窗口英→中→英，已创建Commit面板同步，含空格提交草稿与筛选输入切换前后逐字复制匹配，两次应用语言不新增进程。首次SendKeys丢空格的输入路径未算通过，改用WM_CHAR并先确认初始值；这不代表物理键盘／IME完整验收。未新建镜像实现的单元测试，不重跑未改变的数据层／内存压力。

补齐自然退出证据：ExpectExit先等程序退出并读取原生退出码，再确认Job中无活跃进程，最后释放Job。exit-verified检查关闭一个窗口后另一个仍能显示；关闭最后一个窗口退出码0、应用及Git均退出，不再用Job清理当作应用退出证明。产品沿用GPUI Windows默认LastWindowClosed，无需新的强制退出代码。已有精确视觉、其他中英文状态与发布门槛保持原范围，未增加功能细分清单。

本轮手测包 `dist/Plico-windows-x64-preview-20261018-language.zip`（18,492,134字节），主程序049297…；Windows解压验证三个exe哈希、Plico资源与启动通过，包SHA-256 `ecaee1990d2762d46a2335097ba2a582c7aae045b4649bbdfeb3670f3b7a29ac`，证据 artifacts/multiwindow-review/package-check.json。repositories-verified.json补记两个审阅仓库的HEAD、提交数量及干净状态，持久语言文件为en；没有改用户真实偏好。原发布门槛仍保留，未新增功能清单或额外提交。

### 键盘输入前提与 Unicode 验证工具（2026-10-18）

继续F1／常用输入验收，最小复现将上轮SendKeys空格差异定位到输入法：新窗口输入a b得到啊b；在自有窗口切到英文后，普通／分步／原生虚拟键、WM_CHAR和粘贴均完整保留a b，切回后恢复中文选词。源码产品没有丢普通英文空格，不以中文输入模式下的结果作为产品失败。微软说明拼音Space选词、Shift切中英文：[官方输入法说明](https://learn.microsoft.com/en-us/globalization/input/simplified-chinese-ime)。本机旧IMM状态字段为false但实际行为仍转换，验证以真实输入／读回为准。

增加 `scripts/windows-literal-text.ps1`，将测试填表的字面数据与真实键盘测试分开：检查目标PID／前台窗口，拒绝控制键，按UTF-16发送；中文／表情回归抓到了C#默认ANSI入口的转换问题，已显式绑定PostMessageW，依据[WM_CHAR说明](https://learn.microsoft.com/en-us/windows/win32/inputdev/wm-char)及[字符集互操作说明](https://learn.microsoft.com/en-us/dotnet/standard/native-interop/charset)。多窗口语言验证脚本也复用此助手，不改产品输入模型。

原生 `artifacts/keyboard-space/verified-4` 在现有049297…程序上14项逐字复制核对全部通过，含Review draft及审阅 a b(🙂)，覆盖Log过滤与Commit输入。两个输入模式均复测恢复。仓库HEAD/status不变、自有app/Git退出；收尾包装脚本一处旧报告路径失败后，使用已完成的正确memory.json独立核对清理通过，没有再次启动整轮。诊断全过程、失败记录与有效证据边界见DIAGNOSIS.txt；没有产品调试日志，没有以此宣称所有IME组合或整个F1完成。

验证工具独立提交 `aadca33`，仅含该脚本及根因／测试说明；其他产品功能工作区改动保持原状。产品二进制与当前language预览包未变化，未重建或重发软件包，也未重复无关核心／内存压力验收。

### 首批历史实际可交互计时（2026-10-18）

补原有启动验收缺口，不新增功能清单。按参考文档中 GraphTableModel 的实际可见数量规则，增加仅显式启用的 --startup-report；报告一次实际首批数据／图数量，正常启动无报告。Windows release C2CE1DCF8A3F2A16688EAFE109C99F6C885CF25A457CE074AC700C894CAE4450 构建通过；独立 std 单元测试确认只写一次、保留已存在文件。

artifacts/startup-data/native 的5次原生验证：本地NTFS仓库10000提交，实际1000 commits／1000 graph rows，max_columns=1、offset=0、more=true、无过滤；启动→prepaint为1390.25–1480.68ms，启动→可见节点为1437.18–1541.42ms，启动→点击后完整HEAD OID读回为1524.42–1643.94ms，全部满足普通仓库首批1000条2秒目标。后者包含测试框架轮询、激活、输入、诊断文件和复制开销；不是将 prepaint 冒充呈现时间。节点截图已查看，文件列表和提交详情可用。

缓存条件：测试前Git读取HEAD／数量／status，因此是预读后的热／未控制缓存；冷缓存、干净Windows和其他拓扑／加载反馈门槛继续待验收，既有内存数据不冒充该构建。5个测试进程正常关闭，原生exit code均0，无存活自有Git子进程，仓库HEAD/status不变；保留用户Rebased。初次测试前台保护拒绝、第二轮汇总器错误均未计通过；第三轮每次样本独立落盘并完整汇总通过。

本轮手测包 `dist/Plico-windows-x64-preview-20261018-startup.zip`（18,490,520字节），zip SHA-256 BE5B03FDC1041C4E446D180C6CE2CEF4C6F9839EAE0477491A71A31906105851。Windows Expand-Archive 后主程序及两个 helper hash、Plico 产品资源与默认无诊断参数的原生启动通过，截图已查看，证据 artifacts/startup-data/package-check.json／packaged-native。硬件／系统／Git信息独立保存在 environment.json。没有将包启动采样当作完整内存门槛，也没有重跑未改变的数据层完整核心回归；完整计划保持进行中。

### 设置常用流程集中验收（2026-10-18）

沿用当前C2CE1D…预览，未改产品或新增功能清单。源码核对SettingsDialog／SettingsEditor的OK先apply再close与Cancel取消未应用修改，记录见REFERENCE。本轮Windows native-r3实跑29步，8次偏好文件断言：键盘Tab/Space选中文→Esc丢弃，Apply中文→改English→Esc保留中文，底部Diff Apply→改Tab→Cancel保留底部，重新打开后Enter确认Tab／English并关闭。截图已查看；自有进程自然exit0、Job为0，仓库未变。证据 artifacts/settings-workflow/verification.json。

设置快捷键未触发的诊断确认宿主Ctrl+Alt+S已有全局注册（Win32 error1409），鼠标入口正常，弹窗内Tab／Space／Esc／Enter正常；不修改用户其他软件／系统热键，也不因其占用改产品默认快捷键。无占用环境的快捷键仍待验收，未勾选完整F1。前台保护／命令等待超时的早期尝试不算通过。产品二进制和startup预览包保持不变，无需重复构建、核心回归、内存或打包；完整plan继续进行。

### Push 面板、强推确认与底栏对齐（2026-10-18）

参考 Rebased 4aec352（宿主安装1.1.20），对应 VcsPushDialog／ForcePushAction、GitPushOperation／GitImpl、GitPushOptionsPanel／TagMode、PushLog 文件树与 Islands 控件样式，详细源码路径、规则、本地映射和差异见 REBASED-REFERENCE 的同名记录。单仓库Push调整为source→target树根／提交列表、右侧目录文件树和紧凑工具栏；底部左侧标签选项、右侧Push分段按钮／Cancel。All为固定宽度带箭头组合框；源码控件高度28与south边距8／12作为共同行约束。Checkbox保留自然高度，由父行居中，避免内部items_start令标签贴上沿。Push箭头展开保持蓝色，添加内缩分隔线，菜单打开选择首个可用动作。

Force Push显式从下拉进入，默认显示目标及覆盖远端风险确认；取消保留预览，仅确认后保存“此目标不再提醒”。普通Push保持普通动作；强推使用显式force-with-lease，已知remote-tracking OID作为期望值，无已知引用时为空，不自动Fetch／Pull重试。main/master默认受保护；标签All／CurrentBranch真实映射--tags／--follow-tags。强推确认复选项关闭整条焦点外框，Tab／Space及勾选状态保留。目标记忆按项目+remote+branch隔离且无常驻缓存。

397项Linux核心测试通过。临时仓库+本地bare集成覆盖普通拒绝、已知lease成功、远端新提交stale拒绝、显式fetch后成功、新目标、未知存在目标、保护分支、HEAD／未跟踪文件不变；标签模式区分附注／轻量标签；偏好测试覆盖目标隔离、碰撞及损坏。Windows原生lease／tags两项集成也通过。底栏与focus修正仅涉及界面，未重复未改变的核心数据测试。

Windows footer-en／footer-zh完成29步原生操作：目录折叠展开、Ctrl+D默认文件Diff、标签启用、Force菜单／确认、取消不记忆、确认记忆并实际推到自有bare、再次显式Force不重复警告、编辑目标main后Force禁用，全部自有进程自然exit0、Job归零，原仓库HEAD／status不变且远端OID／全部标签断言通过。final-en较早构建的编辑目标点击未生效，不算保护菜单验收；后两轮已修正输入步骤并看图确认目标main。用户Rebased未关闭。

保留差异：单仓库、无自定义保护规则／provider、无plain force降级／skip hook配置、标签模式未跨弹窗记忆；工具栏未复制未实现配置菜单。精确字体、焦点图标细节、各DPI仍待视觉细化，不称完整像素级还原。Push准备失败同时修正为统一短错误+详情入口，原始Git错误与未检出本地分支分开；有效证据为 push-preparation-error/verified-en、verified-zh，旧clipboard假绿尝试已标无效。本轮没有新增大方向或重跑无关内存门槛。

最终C3A5AECBD0E53FCC7BBA8957D65EA1436C182CD680B7D7637AE5599C10EFBAC0构建再次完成 checkbox-final 的29步Windows原生验证；已查看底栏自然居中、Force菜单、勾选后无整项焦点框的原始截图，保留Tab／Space与目标记忆的文件断言，实际bare推送及标签断言通过，自有进程自然exit0／active0。标题栏logo与菜单painted bounds同为y中心28.5，仅symbol视觉重心有0.4物理像素差异，未改布局；详见logo-alignment.json。前一footer-zh也确认中文底栏对齐并完成同样流程。

新版便携包 dist/Plico-windows-x64-preview-20261018-force-push.zip（18,520,384字节），zip SHA-256 49C40E3DCC5CF6419293E7A0CB510C3827BD4F8D8B5A8A9F84FFA518432371CE。Windows Expand-Archive、三个exe hash、Plico产品资源与中文原生启动通过；证据 artifacts/force-push/package-check.json／packaged-native，截图已查看。主程序为上述C3A5…；打包前中间包未交付，已用最终构建替换。三次启动采样不作为本轮完整内存验收，完整plan仍在进行。

用户要求进一步调整SVG视觉重心：彩色symbol组下移22 SVG单位（20px标题栏图标约0.50逻辑像素），tile/viewBox及标题栏共同中心不变，原始SVG保留；同步生成16–256px Windows ICO和PNG。最终logo构建73D95C46F24753748B79DA2D4217EB803E981D4B7B96DCA0DB3059AD28F7D572，Windows原生启动截图已查看，包主程序／helper哈希与品牌资源通过，仓库未变。新增最终包 dist/Plico-windows-x64-preview-20261018-push-ui.zip（18,520,442字节），SHA-256 187EDC7162E5E7659F91D1FADE2FDA918BEA34E5C1AF14058584DE622F4F9D0A；证据 logo-package-check.json／logo-packaged-native。原C3A5…的29步完整Push测试继续作为行为证据，本次仅品牌资源变化，未重复无关Git写操作与核心回归。

### 默认Log变更区加载反馈与100ms验证（2026-10-18）

继续现有常用流程／性能验收，不增加功能边缘矩阵。发现默认独立Diff Tab模式中，选择提交后文件树清空却没有加载反馈；已有Reading Git data仅在底部Diff。读取Rebased同一4aec352的MainFrame／VcsLogCommitSelectionListenerForDiff与ProgressUIUtil后，在src/ui.rs::files复用busy于变更区显示本地化轻量提示，无额外Git／缓存／计时器；原generation、取消与结果路径保留。为本计划100ms目标即时显示文字，记录与上游300ms延迟spinner的有意差异。

Windows release6425B9AF5EFD9F6CE0C01FAB38644E8B16F83F90D389410CD326CC32192B7E99构建通过。native-v4在10000提交仓库／history-limit10000、默认Tab布局下，通过仅测试目录的Git包装器给9次show --no-patch元数据查询增加2000ms延迟，然后仍调用真实宿主Git；发布程序无延迟注入。8次交替选择同时检测两个提交行的选择色切换与64个已校准加载文字像素，最慢／p95 51.4568ms，全部≤100ms。loading.png原图确认加载提示，after.png确认任务完成后恢复文件树；非物理硬件输入延迟／Git就绪／冷缓存测量，无滚动自动化。早期3次验证工具路径／校准问题被断言拒绝，不算通过；详见VALIDATION.txt。

HEAD/status未变，自有程序自然exit0、Job active0，保留用户Rebased。低影响可逆界面改动未写镜像单元测试，也不重跑未改数据层的397项核心基线／已通过内存压力。完整发布、干净环境、冷缓存和精确视觉门槛仍未全部完成。

便携包 dist/Plico-windows-x64-preview-20261018-loading-feedback.zip（18,520,925字节），SHA-256 88B3370F4FA8B0C472264A6CCE50C316A26064D8AE467081F1463C1C6D525E3D。Windows解压、三个exe哈希、Plico资源与中文原生启动通过。Job验证脚本增加可选StartupReport参数（默认不启用、拒绝覆盖旧报告），包启动报告明确commits=10000／graph_rows=10000／max_columns=1／offset=0；补足规模证据，不将首帧报告当作交互计时。本轮不宣称冷缓存／干净系统或完整内存门槛；原始延迟结果的row标签有笔误，verified-latency.json只修正行号，原timing/pixel数据保留不改。截图与说明见 artifacts/loading-feedback。


### 基础编辑：文件保存执行器与 Commit Diff 范围（2026-10-18）

按用户补充，Commit Diff 中对应工作区文件的侧栏与源码 Tab 共用同一份文档状态；不根据“右侧／Current Version”标题直接授予编辑权限。自动保存只写工作区，不自动暂存；已暂存内容保持不变。保存后刷新本地变更和 Diff，保留光标／视口。此项与源码编辑同批交付，不留为另一个独立编辑实现。

新增 `src/file_save.rs`，使用 Project 原有路径规则拒绝穿越、Git 管理路径及链接；基于原始字节检查外部修改，写完整同目录临时文件并 sync 后复查原文件，再替换；Windows使用ReplaceFileW且不回退到删原文件，Unix用rename。保存失败不推进文档基线，取消／删除／外部修改分支清理临时文件。`Project::read_bytes`供文档保留原始BOM／换行，原只读read继续相同解码行为。

Linux文件相关16项测试通过。Windows release测试程序原生运行5项实际保存测试、6项文档状态测试通过，证据 `artifacts/basic-edit-save/windows-tests.txt`。覆盖成功保存、写完临时文件后同长度外部修改、替换前取消、文件删除不重建、只读与Git管理路径拒绝、BOM／CRLF和草稿保留。测试使用独立系统临时目录，未改用户仓库或 index，未操作用户Rebased。

边界：这是乐观复查，不是文件系统级原子比较交换；复查与替换之间仍有外部进程竞争窗口，不宣称防御恶意目录替换或完整并发保证。文件IO已实现但尚未接入UI；共享文档注册、自动保存触发、关闭／切仓／退出及Git操作前保稿、Commit Diff 编辑、集中Windows窗口验收仍待，当前便携包不变。

### Tab 栏清理与静默保存（2026-10-18）

按用户反馈删除 Tab 公共栏右侧重复刷新；保留 Ctrl+F5、面板刷新及文件 Reload。去掉源码栏“自动保存”等常驻保存状态文案，保存失败反馈和未保存 Tab 标记保留。源码依据与差异记录见 REBASED-REFERENCE 同名条目。Windows release 构建通过，artifacts/quiet-editor-ui 原生截图确认入口移除，修改后磁盘内容断言确认静默自动保存有效；测试窗口正常关闭。便携包未更新，Commit Diff 工作区侧编辑仍待完成。

### Commit Diff 编辑：文档接入准备（2026-10-18）

已对照 DiffContentFactoryImpl/SimpleDiffViewer；增加工作区端点判定及回归矩阵，隔离暂存/历史/冲突快照。共享 Documents 新增 get_or_create，安装异步加载结果时复查已有文档；Source Tab 统一复用/新建的观察订阅，避免旧磁盘快照替换编辑稿，以及复用时漏发保存事件。端点测试通过。Commit Diff 仍未提供编辑交互，后续继续接自定义 Diff 渲染器，不以跳源码按钮替代；此阶段未更新便携包。

### Commit Diff 编辑：完整端点读取（2026-10-18）

新增 working_copy 读取层，为后续编辑 Diff 提供原始工作区 Document 和只读 HEAD/index 基线，不从 patch 重建保存正文。固定 blob 对象后读取，保留工作区 BOM/换行规则，覆盖重命名、空文件、无差异文件。Linux 四项集成测试通过；Windows 原生验证记录见 artifacts/working-copy-endpoints。尚未接入 DiffTab，直接编辑、实时差异重算及两侧行对齐仍需继续，便携包不变。

上述 working_copy Windows 原生测试四项全部通过（1.30s，隔离临时仓库）；未执行远程写操作，未启动或关闭用户窗口。

### Commit Diff 双栏编辑首次接线（2026-10-18）

工作区侧已接共享 Editor/自动保存，左侧为只读完整基线；输入后后台重算差异颜色，离开清理装饰。pending 文档持有到保存完成或显式解决错误，避免切文件丢稿。Windows 原生验证右侧实际落盘、左侧拒绝修改、Jump to Source 内容一致、HEAD/index 不变，artifacts/editable-diff-first 留有截图和断言。

仍属实现中，不能交付为完整 Rebased 编辑 Diff：继续补增删行对齐、单栏映射、差异导航/Jump位置、Ctrl+S、保存后列表刷新、完整关闭/多视图 Reload 生命周期；背景目前只覆盖文字，外观选项未全接。保持目标范围，不用双栏首版替代完整需求。便携包不变。

### 编辑 Diff 操作闭环（2026-10-18）

接入 Review Ctrl+S、真实编辑器 F7/Shift+F7、工作区侧 caret Jump、统一保存后 Commit 列表刷新；同目标底部刷新和独立 Tab 离开保留编辑文档。修复快速 Jump 同步 owner 回调导致 DiffTab 重入崩溃，GPUI 回归先红后绿；差异锚点 UTF-8/纯增删测试通过。Windows artifacts/editable-diff-navigation-fixed 验证导航、显式保存、撤销重做、Jump 行位置、基线恢复后列表清空/撤销后重新出现，HEAD/index 不变。

本阶段仍未完成整个编辑 Diff：行对齐、同步滚动、单栏编辑、完整外观选项及共享 Reload 生命周期继续推进；历史侧映射到未保存文档也需补。没有更新便携包或宣称像素级验收完成。

行对齐进度：已读取 AlignedDiffModel 的 block inlay／实际视觉高度／视口恢复逻辑，增加两侧变更区间及稀疏空白行投影，区间和双向坐标测试通过；源码与差异见 REBASED-REFERENCE“编辑 Diff 行对齐：坐标基础”。投影尚未启用到界面，下一步一起接绘制、命中与光标移动，再做 Windows 原生验证；不将模型测试算作行对齐完成。

行对齐已接入双栏编辑器：按实际视觉行数补齐短侧，绘制/选区/鼠标/光标共用投影，返回源码时清除空白。Linux 7 项相关测试、Windows 原生插入/删除/替换场景通过，保存、Undo/Redo、Jump 正常且 HEAD/index 未变，见 artifacts/editable-diff-alignment。继续处理同步滚动与视口保持、单栏编辑和共享文档生命周期；尚未更新便携包。

同步视口已按 Rebased 对齐模式接入：同步改变的轴，接收侧限制位置后不回传；直接释放 Diff 也会清除共享源码视图的对齐空白。3项相关集成/生命周期测试及 Windows 编辑保存/跳转回归通过，见 artifacts/editable-diff-sync；滚动交互按约定待用户手测，没有滚轮自动化。重算视口锚点、单栏编辑和共享 Reload 继续推进，便携包不变。

共享 Reload 已改为原地更新同一文档，修复 Source 重载后旧 Diff 仍持有旧内容的问题；读失败/期间新输入保留草稿，成功后更新保存基线并通知 Diff 重算。GPUI 与 Windows 外部修改→Reload→返回已编辑 Diff→再次保存验证通过，见 artifacts/shared-document-reload。自动 VFS 重载未实现；接下来继续单栏编辑和重算时阅读位置保持，便携包不变。

双栏重算阅读位置保持已接入：当前侧以可见光标或顶部源码行为锚点，另一侧静默跟随；首次打开保留开头删除行。3项Editor测试和Windows开头删除场景、保存/撤销回归通过，详见 REBASED-REFERENCE 及 artifacts/editable-diff-position。该实现只覆盖 padding 重建，未宣称任意文本变化/滚动交互完全验收。单栏工作区编辑仍未实现，继续按上游 UnifiedDiffViewer 的受保护删除区、来源映射和共享撤销规则推进；没有缩小为只支持双栏，也未更新便携包。

单栏编辑开始接入来源映射基础：已读 UnifiedDiffViewer 的 guarded block、master 文档转写/共享撤销，以及 UnifiedFragmentBuilder 和上游测试。新增不可变 UnifiedEditProjection，保护删除区并严格映射工作区选区，覆盖 Unicode/空文件/末尾无换行。尚未连接输入与共享Undo、实时映射更新及单栏渲染，不算单栏编辑已完成；下一步在输入事务层接入，随后Windows原生验收。详见 REBASED-REFERENCE 同名记录。

单栏映射增加即时编辑更新：合法输入后同步移动后续来源与删除区坐标，不等待后台重算；拒绝修改不改变模型。六项测试通过，其中64对来源各24步连续操作验证只读原文与双向位置不变量。上游依据为 UnifiedDiffViewer/LineNumberConvertor 及 CorrectorTest，详见 REBASED-REFERENCE。输入转写、共享Undo/Redo、外部变化失效和界面接线仍待实现，当前不是可用单栏编辑交付。

单栏编辑继续完成输入层接口：修改前整批保护、修改后精确同步通知（区分IME）、可委托工作区的Undo/Redo；程序重建不回传。真实双Editor集成测试验证映射后输入与工作区历史，输入保护/IME/粘贴、原行对齐及投影回归一并检查。正式单栏入口仍未启用，继续接生产视图、IME事务与外部更新生命周期，随后Windows验收；不是已交付单栏编辑。

### 单栏 Diff 编辑接线与原生验证（2026-10-18）

已启用 Commit 工作区单栏直接编辑，删除区保持只读；单栏、双栏、源码共用工作区文档与撤销历史。输入即时校正映射，后台重算前复验来源，外部文本变化使旧映射失效；输入法预编辑期间不重建展示。保存只写工作区来源，不写含删除行的展示正文。连续输入合并撤销的问题已修复。实现与上游依据见 REBASED-REFERENCE 同名条目。

相关11项编辑器／映射测试及1项生产 UnifiedEditing 测试通过；生产测试覆盖连续输入合并Undo、Redo、外部修改保护及中文IME事务。Windows release SHA256 `79D53C24A2E5259804FF9D0B6B56A6763C7BB1DB98220D3582D615685F3D2A7A`，`artifacts/unified-editing/assertions.json` 确认删除区拒绝输入、工作区保存、Undo/Redo、Jump源码与切换双栏文本一致，HEAD/index未变。单栏、保存后、源码和双栏截图已查看，stderr为空，自有测试窗口正常关闭；未操作用户Rebased，没有滚动自动化。

尚未完成：单栏双来源行号、整行差异背景、关闭／输入法组合期间的完整保稿审计及编辑内存验收。当前单栏显示投影行号，背景覆盖字形，重算保留原视口偏移；不宣称像素级还原或所有生命周期场景完成。原生Windows输入法交互仍待验证（本轮IME为GPUI测试）。便携包未更新。

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

## Preview 使用阻断修复（2026-10-19）

仅在 `/home/echoyn/code/plico` 实施。旧 `gitui` 代码与未提交内容已移到工作区外备份，避免误改；未丢弃其改动。

- 修复真实 Tab 在文件编辑器中被渲染为零宽的缩进问题，保留原文件内容及光标字节映射。
- F7／Shift+F7 按 Rebased 的文件内导航 → 边界提示 → 再次按键跨文件处理，首尾不循环。
- 历史“加载更多”固定紧凑高度。
- 标题栏交互按钮与原生拖动命中隔离，覆盖设置、更新项目、推送、项目／分支／主菜单；保留悬停说明。
- 欢迎页打开项目按原窗口中心定位，成功创建项目窗口后再关闭欢迎页；已有项目窗口切换项目保留尺寸。

源码依据、映射和保留差异见 `docs/REBASED-REFERENCE.md` 同日记录。Tab 字节／坐标回归测试和两个 F7 边界测试已通过，Windows release 构建通过。Windows 原生验收证据见 `artifacts/fixes-20261019/`：

- `physical-before.png` / `physical-after.png`：真实左键点击欢迎页设置，修复前无响应，修复后打开；`main-settings-left.png` 覆盖主窗口设置。
- `update-ready.png`、`push-ready.png`、`push-tooltip.png`：标题栏更新／推送真实左键打开弹窗，推送悬停提示正常。仅使用临时仓库和本地 bare remote，未执行更新／推送确认。
- `window-bounds.json`：欢迎页与项目窗口横向中心均为屏幕 x=1920，纵向中心相差 5 物理像素（原生边框尺寸），不再以固定左上角放大。
- `source-tabs.png`、`source-tabs-end.png`：复制 Spring Framework 的 IOConsumer.java 到临时仓库验证，嵌套 Tab 正常显示。编辑、自动保存、撤销、再次保存后，`source-roundtrip.json` 的 SHA-256 与初始内容一致。
- `f7-boundary.png`、`f7-next-file.png`、`f7-last-no-wrap.png`：独立 Diff Tab 边界提示、二次按键跨文件、末尾重复按键不循环；`load-more-compact.png` 验证紧凑分页条。未做滚动自动化。

已知验证限制：桌面测试总计 8 通过、1 失败，失败为既有 `releasing_diff_clears_padding_from_retained_source`。临时恢复全部 Rust 文件到未修改 `cbde5aa` 后，同一测试仍以相同断言失败（实际 0，期望 2），记录为独立问题，不宣称全套测试通过。Tab 默认四列，未新增 `.editorconfig` 解析；本轮跨文件 F7 原生验收限独立 Tab，旧式底部 Log 预览尚未接入跨文件提示。

本轮作为修复提交推送至 main；不创建发布标签或更新现有 Release。

## Log Reset 增量（2026-10-19）

- [x] Log 单选右键“将当前分支重置到此处…”；Rebased 风格目标说明、Soft/Mixed/Hard/Keep 单选、默认 Mixed、记忆确认的模式、Reset/Cancel；Hard 描述明确说明本地改动丢失。
- [x] 写操作前保存编辑器，在工作树锁下重新核对 HEAD/分支/操作状态；后台执行，结果刷新 Log、本地变更、项目树和共享编辑文档；复用底部状态与展开错误详情。
- [x] Rebased 源码与保留差异已记录在 `docs/REBASED-REFERENCE.md`。Keep 拒绝覆盖时采用 FAIL 策略，无自动 Hard / Smart Reset。
- [x] 5 个临时仓库集成测试通过，覆盖四模式、staged/unstaged/untracked、Keep 拒绝覆盖、游离/同 HEAD、状态变更、取消、锁失败及已有 merge；桌面 cargo check 通过。
- [x] Windows 原生功能阶段 UI 验收：菜单、四模式/默认 Mixed、取消、Hard 选择与执行通过；临时 repo 的 HEAD 从 `828b1031` 回到 `6a79084f`，原有 sample.txt Tab 从 Second content 自动刷新为 Base content，底部提示重置成功，工作区干净且模式保存为 Hard。截图 `artifacts/reset-20261019/menu.png`、`hard-selected.png`、`source-before.png`、`source-after.png`；执行证据 `result.json`。Windows release 构建通过。未完成像素级验收。

## Preview.2 使用反馈修复（2026-10-19）

- [x] 修复可编辑 Diff 在比较前规范化换行导致 CRLF/LF 差异消失；原始格式比较、LF 编辑坐标映射、单/双栏差异高亮及“仅换行符不同”提示。查看不改写文件。
- [x] 修复 Reset 底部取消直接杀 Git 导致半完成状态：准备阶段可取消；执行阶段不可取消并明确提示，完成后刷新。无需也不进行不可靠的自动反向 Reset；已有损坏状态不自动修改。
- [x] 两个缺陷均先复现失败测试；工作区/Diff 6 项和 Reset 6 项回归通过；源码依据和保留差异见 REBASED-REFERENCE。
- [x] Windows 原生截图确认纯换行差异单/双栏，以及 Reset 执行阶段无取消按钮：`artifacts/eol-reset-20261019/eol-open.png`、`eol-split.png`、`reset-running.png`、`reset-complete.png`。Hard Reset 完成后 HEAD/文件内容一致且无 index.lock；功能验证通过，未做像素级验收。
- [x] WSL UNC 保存 code 50：原生临时目录复现并修复不支持的 ReplaceFileW，保存/冲突/取消等 5 项 Windows WSL 测试通过。
- [x] Git 标准错误超过 64 KiB 误报数据超过 8 MiB：分离限额并持续排空诊断；194 项 backend 测试通过。
- [ ] SecideaAiSourceSecurity 真实仓库的状态输出统计：用户确认仓库不在本机，当前无法测量，也不能认定其真的超过 8 MiB。

- [x] 嵌入 Diff 预览统一完整文件上下文；300 行失败复现修复、8 项 Diff 查询测试和 30,000 行完整内容/差异范围测试通过。字节上限与超长单行策略未修改，大文件原生 UI 待验收。
- [x] 修正标题栏主菜单、项目及分支下拉的事件遮挡；Windows release 构建通过，三个入口真实左键打开截图均已核对（`artifacts/eol-reset-20261019/*-menu-fixed.png`）。源码规则及事件映射见 REBASED-REFERENCE 同日记录。

- [x] 窗口右边缘缩放后详情自动选中文字：原生复现后修复底层文本选择残留状态，无左键移动结束手势。Windows release 及原生重放通过；主动拖选和释放后选区稳定也已确认。证据见 REBASED-REFERENCE 同日记录。

- [x] 文件浏览编辑光标偶发消失：文件页改走编辑器聚焦生命周期，取消重复聚焦；Windows 构建与首次打开/切换 Log 后返回的 12 帧闪烁验证通过，证据 `artifacts/eol-reset-20261019/caret-{before,fixed,tab-return}.png`。

## Preview.3 只读 Diff 滚动范围修复（2026-10-19）

- [x] 500 行真实 Diff 布局复现首次打开只有 15px 可滚动距离；修复底层首次/宽度变化时清空行高估算的问题。F7 不再承担逐步补全滚动范围的作用。
- [x] 双栏首次打开、改变宽度、切换单栏的滚动范围测试通过；连同既有 F7/源码跳转回归共 4 项通过。源码依据与本地差异见 REBASED-REFERENCE。
- [x] Windows release 构建及原生新 Tab 首次打开核对，截图 `artifacts/diff-extent-20261019/{before,after}.png`。
- [ ] 实际鼠标滚动由用户手测；默认滚动条为空闲自动隐藏，静态截图不作为滚动范围已修复的证据，范围由上述渲染测试验证。本次尚未发布新包。

## 双栏 Diff 中线调整（2026-10-19）

- [x] 对照 Rebased DiffSplitter/Splitter，实现中线拖拽及双击恢复等宽；历史只读 Diff 与本地变更可编辑 Diff 共用比例，标题/正文/横向滚动条同步。
- [x] 比例限制 10%–90%，当前页面保留；释放/丢失左键/退出页面清理拖动，单栏不显示拖动区域。三栏合并分隔和跨启动持久化不在本轮范围。
- [x] 2 项鼠标交互/比例测试、4 项 Diff 导航/滚动范围测试通过，Windows release 构建与历史/本地 Diff 原生拖拽、双击及对齐截图核对通过。详见 REBASED-REFERENCE；未发布新包。

### 2026-10-19 Diff 观感与字体设置

- 按 Rebased 源码调整新增／删除／修改颜色与行内强调，修复可编辑双栏 Diff 只给文字着色的问题；统一只读 Diff 与编辑器字号计算。
- 设置新增独立“外观”“编辑器字体”：字体、字号、预览、恢复默认；Apply/OK 持久化，Cancel 丢弃。
- 源码依据、现场截图和保留差异见 `docs/REBASED-REFERENCE.md` 同日条目；本轮不扩展为全部 Diff 像素级重写。

- Diff 中栏贯穿着色与工作区块级还原：已翻译 Rebased 可编辑判断／有效性校验／单次撤销规则；双栏箭头接入自动保存，纯历史对比保持只读。块边界及过期状态测试、Windows 原生替换和撤销验证已通过。源码、截图与保留差异见 REBASED-REFERENCE 的 2026-10-19 记录；不扩展追加模式等额外操作。

## 文件操作候选增量（2026-10-19，暂缓实施）

用户要求仅记录计划，当前不实施；待后续明确启动，不作为本轮交付或封版阻断项。

- [ ] 项目浏览：文件／目录的复制、剪切／粘贴、移动／重命名、删除。
- [ ] 本地变更区：删除文件、回滚变更、复制路径、定位到项目树；磁盘文件移动复用项目浏览的操作。删除磁盘文件与回滚 Git 变更使用独立入口，不混淆语义；纯历史记录不提供磁盘删除。
- [ ] 两处共享文件操作逻辑，处理目标同名冲突、Windows／WSL 路径、打开的文件 Tab 和 Git 状态刷新。重点协调待保存内容与文件生命周期，避免删除／移动后旧自动保存任务重新创建原路径；批量操作部分失败或取消时明确报告实际结果。
- [ ] 删除按平台能力考虑回收站；不可回收时明确确认。回滚的 staged／unstaged／untracked 范围需在实施前继续核对上游，不能把删除或回滚默认解释为清空暂存区。
- [ ] 验证使用临时仓库，覆盖文件／目录、同名冲突、打开且待保存的文件、失败／取消及 Windows／WSL；实施后做 Windows 原生菜单和弹窗验收。

已核对 Rebased commit `4aec35256d765c217aa0d662b82ac4e860c396fd`（安装版 1.1.20 与该快照不保证一致）：

- `platform/vcs-impl/resources/META-INF/VcsActions.xml::ChangesViewPopupMenu`：删除、回滚和复制引用分开；`ChangesView.Move` 对应 `MoveChangesToAnotherListAction`，含义是移入另一个 Changelist，不能照名称实现成磁盘路径移动。
- `platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ui/ChangesListView.java`：所选文件通过 `VirtualFileDeleteProvider` 提供删除能力。已读 `platform/platform-impl/src/com/intellij/openapi/fileChooser/actions/VirtualFileDeleteProvider.java`：回收站行为取决于平台支持与设置，无法全部回收时确认删除，并汇总失败结果。
- `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/actions/RollbackAction.java`：保存文档后按所选变更／缺失文件进入对应回滚处理。具体 Git 回滚实现与测试尚未核对，启动时补读。

范围保持基础文件管理，不引入 IDE 的引用重构、安全删除分析或 Changelist 管理。当前没有本地功能实现；入口、控件及操作生命周期在启动实施时继续按 Rebased 对照。

## 代码质量：主窗口职责拆分（2026-10-19）

- [x] `src/ui.rs` 从 5,877 行拆至格式化后的 1,022 行，只保留窗口状态、构造、快捷键绑定和顶层组合；设置、Tab、仓库、远端操作、checkout、历史写操作、Log 及预览按职责放入 `src/ui/` 私有模块。
- [x] 提取 `ui_actions.rs`、`ui_style.rs`、`ui_graph.rs`；其他视图直接依赖共享动作／样式／绘制，不再反向依赖主窗口。Graph 绘制通过回调交还导航，不拥有查询任务。设置页数字编号改为私有枚举，草稿字段封装在设置模块。
- [x] 搬移前后方法核对无缺失，除设置页枚举替换外保留原方法行为；cargo check、194 项库测试、15 项桌面测试及 Windows release 构建通过。已知基线失败 `releasing_diff_clears_padding_from_retained_source` 明确跳过，未宣称全套通过。
- [x] Windows 原生独立测试仓库核对提交侧栏、Diff Tab、设置字体页和返回 Log；截图 `artifacts/diff-fonts-20261019/quality-{diff,settings,log}.png`。本轮没有重测全部 Git 写操作或进行滚动自动化。

边界：这是第一轮职责整理。`src/ui/` 私有实现仍共享 `Review` 的状态与异步生命周期，尚未成为各自独立的组件；后续质量工作优先收拢状态所有权与取消／刷新接口，不按文件行数继续机械拆分。本轮不扩展新功能、不实施上面的文件操作候选。源码映射与保留差异见 REBASED-REFERENCE 同日记录。


## 代码质量：操作生命周期与 Tab 状态收拢（2026-10-19）

- [x] `src/active_operation.rs` 统一操作身份、阶段、取消与完成；取消请求不提前释放占用，过期完成不能解锁后续操作。保留 Reset 准备可取消、执行不可取消的既有策略。
- [x] `src/ui/operation.rs` 集中保存前置流程与文档锁同步；开始、结束和本地提交状态变化时更新锁，不再依赖 render。保存失败保留草稿并阻止后续 Git 操作。
- [x] `src/editor_tabs_state.rs` 成为编辑器 Tab 顺序和活动身份的唯一来源。Diff／文件实体列表仅存放各自视图；菜单、点击和延迟关闭以实体 ID 定位，不保存易失效的数组下标。
- [x] 200 项库测试、18 项桌面测试、46 项 Git 集成测试通过；桌面套件仍明确排除既有基线失败 `releasing_diff_clears_padding_from_retained_source`。cargo check 与 Windows release 构建通过。
- [x] Windows 原生独立实例核对混合 Tab、关闭前面的非活动 Tab、重新打开、右键关闭全部，以及编辑后立即关闭的保存结果。截图 `artifacts/diff-fonts-20261019/architecture-{close-inactive,tab-menu,close-all,saved-close}.png`；临时仓库同 HEAD Mixed Reset 成功。取消策略由自动化测试覆盖，本轮未原生验证进度取消。

本轮到此结束：不继续重写 Log 状态，不扩展文件管理。各 Git 操作的结果解释、刷新及恢复流程仍留在对应功能模块；公共生命周期不把取消请求解释成回滚成功。未进行全量内存基准或所有操作的原生回归，不宣称像素级验收。源码规则及保留差异见 REBASED-REFERENCE 对应记录。
