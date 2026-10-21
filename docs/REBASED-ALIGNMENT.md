# 已实现功能对齐检查（2026-10-21）

基线：Plico `f97486a`（preview.7）及本轮未提交修改；源码参考 Rebased
`4aec35256d765c217aa0d662b82ac4e860c396fd`，宿主机安装版 1.1.20。
二者不保证是同一版本。范围是已实现功能的实现与界面核对，不以补齐整个 IDE 为目标。

本表是覆盖范围与差异清单，不是“所有场景已验收”证书。**本轮源码**表示本次实际阅读上游对应方法；**既有证据复核**表示核对当前本地入口及已有实现记录，未重新执行该功能的全部上游测试／原生流程。旧记录是阶段快照，例如“缺少多块提交”已经由后续实现取代，不能直接当成当前缺口。

## 覆盖与结论

| 功能 | 核对依据与本地入口 | 当前结论与保留差异 |
| --- | --- | --- |
| 欢迎页、项目、窗口、标题栏 | 既有证据复核；`ui/chrome.rs`、`ui/repository.rs`，参考记录“主工具栏原生对照”“欢迎页定位” | 已有独立项目窗口、品牌图标、项目与分支入口。此次原生打开同一仓库核对布局；不同 DPI、窗口位置、多窗口操作不作全量通过声明。 |
| Log / graph / 详情 / 日期 | 本轮 `VcsLogUiImpl.java`，本地 `ui/log_history.rs`、`ui/log_selection.rs`、`detail_cache.rs`；其他 graph/date 规则使用既有记录 | 已有按需详情、日期时间、当前 HEAD 范围高亮。保留有界历史及缓存，不复制 IDE 全量常驻图；新增结果与当前选择不是一回事。没有为了界面对齐取消缓存和取消令牌。 |
| 文本、分支、用户、日期、路径过滤 | 本轮 `VcsLogClassicFilterUi.kt` 的模型集合及事件；本地 `ui/log_history.rs`、`history.rs`、各 filter 模块 | 入口次序和独立模型对应；聚合过滤与 UI 操作已有。未重新穷举所有正则／路径组合的上游语义，不将按钮齐全当作语义穷举通过。 |
| 自动刷新与分支更新 | 本轮 `GitBranchIncomingOutgoingManager.java` 调度、远端策略和未知计数；本地 `ui/auto_refresh.rs`、`branch_updates.rs` | 本地引用 5 秒轮询；当前分支远端 5 分钟只读 ls-remote。上游还支持 FETCH 策略、更多分支与事件驱动。本地未获取的新提交需要显式 Fetch 才进入 Log，箭头与已获取数量分别表示。 |
| 分支、Tag、Checkout | 本轮 `GitCheckoutAction.kt`、`GitRefUtil.java::getCurrentReference`；本地 `ui/reference_checkout.rs`、`reference_checkout.rs`、`branch_popup.rs` | 当前分支禁用；其他同 OID 分支仍可 checkout；游离 HEAD 取一个当前 Tag 与上游规则对应，不能把同 OID 全部标签禁用。远端同名冲突和本地独有提交已有决策；远端／Tag 的 Smart Checkout 仍未统一，覆盖风险由 Git 拒绝。 |
| Fetch / Pull / Push | 本轮 `VcsPushDialog.java` 动作及后台执行；其余完整操作语义沿用既有记录；本地 `pull_dialog.rs`、`push_dialog.rs`、`push_flow.rs`、`ui/operation.rs` | 已有 Fetch loading、Pull 选项、Push 拆分按钮与 Force 确认。没有重写这些写操作。本轮未执行真实远端写入，认证失败／断网／保护分支 UI 完整矩阵仍需专项原生验证。 |
| Commit / Amend / Changes 列表 | 本轮 `ChangesBrowserFileNode.java`、`ChangesBrowserChangeNode.java`；本地 `local_view.rs`、`commit_scope.rs`、`commit_operation.rs` | 勾选范围与浏览选择分离，Amend、分组、忽略文件已有。本轮修复父目录挤压短文件名；改为有序的文件名＋灰色路径单文本。未添加命名 changelist、Shelf 或无行为占位入口。 |
| Commit Diff 多块／部分行选择 | 本轮 `SimpleLocalChangeListDiffViewer.kt`、`LocalTrackerDiffUtil.kt`、`DocumentTracker.kt::RangeExclusionState`；本地 `editable_diff/partial.rs`、`partial_selection.rs`、`diff_tab.rs` | 本轮修复标题总勾选、差异／已包含计数、Islands 中性勾选图标、上下文菜单可用状态。变更内容改变时保守失效，不自动迁移旧 mask；Unified 尚无逐块／逐行选择，新增／删除整文件和 rename 仍为整文件选择。 |
| 编辑器行标记与单块提交 | 既有证据复核；`source_changes.rs`，参考记录“单块提交” | 已有修改／新增／删除标记、块撤回、单块提交。仍缺 inline Amend、消息历史与上游自适应输入布局；未做滚动自动化。 |
| Rollback | 本轮 `RollbackChangesDialog.kt`；本地 `rollback_dialog.rs`、`local_view/rollback.rs` | 空选择禁止执行、新增文件删除选项只在有新增项时可用，选择可调整。UI 仍为平面列表、简化数量说明，取消文案及 Help 与上游不同；文件树与分类统计尚未像素对齐。 |
| Reset / Reword / Squash | 本轮 `GitNewResetDialog.kt`、`GitCommitEditingActionBase.kt` 入口与禁用检查；本地各 dialog、`reset_operation.rs`、`reword.rs`、`squash_check.rs` | Reset 四模式及目标摘要对应；历史编辑保留保护分支／HEAD／操作状态检查。执行时复验不能被 UI 资格缓存替代；本轮未在用户仓库执行历史重写。 |
| Stash | 本轮 `stash/ui/GitStashActions.kt`；本地 `stash_view.rs`、`ui/saved_changes.rs` | 已有 Commit 工具窗口内 Stash 页签、单条 Apply／Pop／Drop，Drop 需确认。缺 Unstash As（index／新分支）、Clear All、完整创建对话框。Shelf 是另一个能力，不能把 Stash 页签改名为 Shelf 伪装完成。 |
| Diff、导航、预览归属 | 既有源码证据＋本轮桌面回归；`editable_diff.rs`、`diff_scroll.rs`、`diff_tab.rs`、`ui/preview.rs` | 已有中栏曲线、对侧空区间细线、内侧行号、拖拽分栏、首次差异定位和 F7 边界。Unified 显示、逐块控件及排除区域淡化仍有差异；滚动视觉连续性留用户手测，不把静态截图当作帧率验证。 |
| 源码编辑／自动保存／外部变化 | 既有源码证据＋本轮桌面回归；`editing/`、`working_copy.rs` | 已有共享文档、自动保存、撤销、外部修改冲突保护。历史与 index 快照保持只读；文件预算、有界读取与明确错误是保留的产品约束，不等于 IDE 全功能编辑器。 |
| 项目文件树、历史、Tab、设置与字体 | 既有证据复核；`project_view.rs`、`history_view.rs`、`ui/editor_tabs.rs`、`ui/settings.rs`、`ui_font.rs` | 已有项目树、文件／选区历史、预览与普通 Tab、Apply/Cancel 草稿、UI／编辑器独立字体。字体字重／连字／备用字体／独立行距未复刻；设置页布局是轻量客户端范围，完整键盘和 DPI 矩阵仍待验收。 |

## 本轮修正依据

上游文件均位于上述固定 commit：

- `platform/vcs-impl/src/com/intellij/openapi/vcs/changes/actions/diff/lst/SimpleLocalChangeListDiffViewer.kt::createTitles/getStatusTextMessage`：总勾选嵌入右侧版本标题；工具栏显示可见差异数，未全选时追加已包含数。
- 同目录 `LocalTrackerDiffUtil.kt::ExcludeAllCheckboxPanel`：全选／未选／部分选择三态，无变化不显示图标；部分选择点击全选。`PartiallyExcludeSelectedLinesFromCommitAction.update`：无变更禁用，部分选区按当前侧 mask，完整块按整块判断是否还能包含／排除。
- `platform/diff-impl/src/com/intellij/openapi/vcs/ex/DocumentTracker.kt::RangeExclusionState.countAffectedVisibleChanges`：整块计一处；拆分块按删行与增行计数。本地新增独立 `visible_counts`，**不修改用于合成提交内容的原有行计数**。
- `platform/icons/src/expui/diff/gutterCheckBox{,Selected,Indeterminate}@14x14_dark.svg`：原样使用 14px Islands 轮廓图标，保留资源版权与来源。标题是 GPUI button、gutter 是 canvas 命中区，不复制 Swing InplaceButton；业务状态来自同一个 scope。
- `platform/vcs-impl/shared/src/com/intellij/openapi/vcs/changes/ui/ChangesBrowser{File,Change}Node.java::render`：先 appendFileName，平铺时再 appendParentPath。本地改为同一 StyledText 的两个颜色段，窄栏优先保留文件名。

本轮未复制：菜单按“块／行／待拆分块数”的完整动态文案与快捷键、悬停单行 checkbox、排除区域淡化／部分块拆色、Unified 细粒度入口。版本标题仍采用 Plico 的 Working tree 文案。文件总勾选与状态计数仅适用于已有部分提交 scope，不向历史 Diff 添加提交控件。

## 下一步优先级

1. Unified 与并排 Diff 的变更选择能力一致，以及已排除区域的视觉反馈：已有功能的主要体验差异。
2. 远端／Tag Checkout 的 Smart Checkout 及错误内联反馈：现有操作的恢复体验差异，必须独立测试 stash／冲突恢复边界。
3. Rollback 树／统计、菜单动态文案、键盘焦点与各 DPI：继续 UI 对齐，不顺带扩张 Git 写操作范围。
4. Stash 高级恢复、命名 changelist、Shelf、字体高级项：功能扩展，单列安排。

## 验证记录

验证结果与原生截图在本轮 `REBASED-REFERENCE.md` 末尾记录。仅使用自建 Plico 验证进程；用户的 Rebased/Plico 保留。对用户已打开的 fixture 只读取视图，未执行 commit/reset/rollback/push，未编辑文件。写操作集成测试均使用临时仓库。
