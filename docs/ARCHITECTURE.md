# 架构审查与本轮整理（2026-10-20）

目标是让维护者通过少量明确的 Interface 使用 Module，由 Module 持有任务和状态的不变量；不按文件行数拆分，也不建立全项目通用任务框架。领域用语见 ../CONTEXT.md。

## Standards

审查固定范围 `git diff dad3cb7...2a3371a`，即 preview.5 后的项目树路径操作与本地更改刷新反馈。依据 AGENTS.md、REBASED-REFERENCE.md；仓库无 issue tracker 配置，需求采用功能实现记录，未声称查阅外部 issue。

未发现明确违反项目约定的新增功能。两个架构判断：LocalView 直接维护查询、取消、排队、提示等成组状态；同时负责提交执行报告解释和树渲染，职责容易相互影响。均为重构机会，不是凭字段数量判定的功能缺陷。

## Spec

发现一项性能回归：事件即时分发后 Documents::observe_disk 对所有打开文档检查 stamp，无关事件也会触发磁盘访问；同一计时器还被每个事件重置。修复为受影响路径立即检查、全体打开文档独立每两秒兜底，连续事件不加速或推迟此周期。30 秒仓库补偿仍仅用于 UNC／监听不可用等既有情况。

审查统计：Standards 无硬问题、两项架构判断；Spec 一项性能回归。路径菜单捕获点击时的路径，树重排不会换对象，保留此行为。

## 已落实的 Module

- `changes_refresh::ChangesRefresh`：具体的本地更改查询 Module。Interface 是 request、只读查询状态与 Started／Loaded／Failed 事件；Implementation 持有真实 Git 查询、取消令牌、一次运行加一次合并请求、300ms 延迟反馈和任务释放。LocalView 保有树模型、提交范围、草稿与写操作，不再直接维护查询任务的多个开关。
- `worktree_events::Schedule`：独立的打开文档检查和仓库补偿期限。Interface 接收当前时间与事件批次，产生本次检查决定；不查询 Git，不把普通定时检查转为全仓刷新。
- `Documents::observe_disk`：把事件影响范围与周期检查明确区分。只有命中事件或到期兜底的文档才启动 IO，继续由共享文档保护未保存草稿与迟到结果。

ChangesRefresh 的加载器是内部测试 seam：生产 Adapter 使用真实 Git，测试 Adapter 使用受控完成时机。没有将这套机制泛化到所有后台查询；项目树的分页、Log 的引用查询、远端检查各自有不同生命周期。删除该 Module 会把取消、排队和延迟提示复杂度重新散回视图，因此它不是仅转发的封装。

## 后续优先级

1. 本地提交结果的错误解释可单独整理为有界诊断 Module；本轮不改变写操作与恢复语义。
2. Review 的 Log 查询／过滤／分页仍共享较多状态，下一轮应先以查询身份和结果接纳规则建 seam，再移动代码。
3. ProjectView 中项目树与文件 Tab 有独立生命周期，可分离源码文件，但仅移动代码的收益低于上述状态所有权整理。

暂不引入事件总线、依赖注入框架或全仓命名重写。已知 Diff `releasing_diff_clears_padding_from_retained_source` 基线失败不混入这轮刷新重构。

## 验证

206 项库测试、2 项文件监听集成测试及 38 项桌面测试通过；上述已知 Diff 基线失败仍单独排除。Windows release 构建和原生慢刷新验证通过：更新提示出现／消失不挪动文件行，空闲八秒 status 调用保持 2 次，文件修改后变为 3 次。证据见 `artifacts/refresh-architecture-20261020/`。未测量异机 Windows→WSL 耗时，不能据此声称解决所有刷新性能问题。
