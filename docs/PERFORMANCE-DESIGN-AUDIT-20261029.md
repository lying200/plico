# 性能设计审计：从查询数量到状态所有权

本轮结论：当前慢并非一个控件或某条 Git 命令的问题，而是**无共享状态的细粒度查询、
按操作扩散的刷新、逐对象查询**叠加 Windows 每次进程调用的固定成本。值得做有边界的
底层调整，但没有证据支持整体替换 Git CLI、移除进程树保护或重写整个 UI。

审计基线为 Plico `6f2b454`，Windows / Git 2.56.0.windows.3。以下“方法与覆盖范围”和
“量化结果”描述最初只增加诊断 example 的审计阶段；后文按顺序记录已经实施的产品变更、
各自构建、原生验证与测试限制，不能将基线数字视为当前构建结果。原生 Fetch / Update / Push 对比及截图见
[上一轮审计](PERFORMANCE-AUDIT-20261029.md)，不把旧构建数据当作本轮新测量。

## 方法与覆盖范围

- 新增 `examples/performance_audit.rs`，调用真实核心入口，分别测普通 spawn/wait、
  ProcessTree spawn/wait/drop、生产查询层和上层工作流。耗时由应用外层单调时钟记录；
  同时输出墙钟起点，供独立 Trace2 文件关联命令。没有毫秒级 CI 成败阈值。
- 两个独立临时工作区及本地 bare remote：小仓库 72 提交 / 200 文件；规模仓库
  2000 提交 / 10000 文件，首屏实际读取 1000 条。只改测试仓库，最终两者内容均干净。
- 进程层 20 次、上层入口 5 次；顺序执行，测量期间没有构建或测试负载。预热缓存，
  不是重启机器后的冷启动，不测内存、GPU、滚动或公网。
- 分开记录 Trace2 开启与关闭；下表时间使用关闭 Trace2 的中位数，命令数来自同输入
  的追踪运行。不是按钮点击到绘制完成时间，不能把各行直接相加当作 UI 总耗时。
- 当前 UI Update 调用 `Prepared::read_registered`；旧 `Prepared::read` 另列诊断，
  后者 51 条查询不能冒充当前 UI 的 26 条。Log 多选模拟 `log_changes::load` 后读取
  最多 50 条 metadata；单选 UI 自有 details/cache 路径，不使用多选 1 条测量代表它。
- 初始审计阶段没有重新操作原生 UI：当时没有产品行为改动。尚未新测 Commit/Checkout/Rebase 等
  写入全过程，它们在下文是源码级定位，不是已量化的性能结论。

本机原始材料：`artifacts/deep-performance-20261029/`，包括 `fixture.json`、
`small*.jsonl`、`extended*.jsonl`、`large-untraced.jsonl`、两份 Trace2、`analysis.json`、
`summary.txt`、分析脚本及规模 fixture 脚本。该目录为忽略的本机证据，不随源码提交。

通用复测入口（在独立、具有正常 upstream 的测试仓库执行）：

```powershell
cargo build --locked --no-default-features --release --example performance_audit
& target/release/examples/performance_audit.exe <测试仓库绝对路径>
```

测量时隔离 Git global/system 配置；不在真实用户仓库执行后文的时间戳/cache 实验。
需要命令数量时设置 `GIT_TRACE2_EVENT` 到独立绝对路径，计时对照再清除此变量。

## 量化结果

| 核心路径 | 小仓库中位数，无 Trace2 | 规模仓库中位数，无 Trace2 | 小仓库顶层 Git 数 |
| --- | ---: | ---: | ---: |
| 读取操作观察标记 | 41 ms | 42 ms | 1 |
| 完整 operation_state | 124 ms | 124 ms | 3 |
| refs + shallow 观察 | 124 ms | 134 ms | 3 |
| Changes Snapshot | 168 ms | 423 ms | 3 |
| 首屏历史及 graph | 358 ms，72 条 | 364 ms，1000 条 | 8 |
| 普通提交详情 | 88 ms | 88 ms | 2 |
| 一个小文件完整 patch | 41 ms | 46 ms | 1 |
| 多选 20 条及说明 | 2189 ms | 2342 ms | 50 |
| 多选 50 条及说明 | 4765 ms | 5112 ms | 110 |
| Push 目标快照，不含 outgoing/执行 | 271 ms | 258 ms | 6 |
| 当前 Update 预检，不含偏好/弹窗/执行 | 1089 ms | 1092 ms | 26 |
| 旧单仓库 Update 预检，仅诊断 | 2165 ms | 2184 ms | 51 |

规模仓库文件很小，历史线性且没有复杂 merge。这证明本组场景的多秒等待主要随查询数
增长，不能推断复杂 graph 或大文本解析永远便宜。Changes 的增长也说明真实扫描成本
不能全部用进程启动解释。

### 进程层：优先级需要纠正

小仓库无 Trace2 的同轮 20 次中位数：普通 spawn+wait **38.90 ms**，ProcessTree
spawn+wait+drop **38.88 ms**；ProcessTree spawn **2.73 ms**，drop **0.03 ms**；
生产 head 查询 **44.54 ms**。两种 spawn 控制均丢弃输出，生产层另有管道、线程及
5 ms 轮询，不能把差额全归于某一个调用。

因此 `src/process_tree.rs` 的 Job、挂起分配和线程快照**不是本组主要瓶颈**。
先前看到的命令间空隙不能当作 PssCaptureSnapshot 耗时。保留取消时子孙进程回收和
管道退出保护。生产 read 使用 5 ms、operation_process 使用 10 ms 轮询，写队列争用
时使用 50 ms 重试；改事件唤醒可能消除尾部等待，但排在减少几十次查询之后。

Trace2 开启后，同类生产 head 查询从约 **44.54 ms** 变成 **65.43 ms**，普通
spawn+wait 从 **38.90 ms** 变为 **61.14 ms**。这是追踪开启/关闭的观测差，不归因于
单个内部 API。Trace 中有 Windows ancestry 采样。上一轮 Trace2 时间线适合证明
命令数、顺序及重复刷新，不能直接当作无追踪用户耗时，也不能据此承诺同样的提速比例。
另做前置 mingw64/bin PATH 实验，没有得到稳定收益，不将换 Git 路径作为优化建议。

## 根因一：Repository 只是命令入口，没有统一的状态快照

`src/git.rs::editor_directory` 每次启动 rev-parse；operation_state 又调用它，随后
symbolic-ref 和 ls-files。`operation_state::Observation` 即使只比磁盘标记，也每次
重新启动 Git 找目录。refs 轮询和 operation observer 各有自己的五秒调度。

`pull_targets` 读 remote、远端 refs、symbolic-ref、当前分支 upstream；`push_snapshot`
在其上继续读 config/ref 信息。历史读取也重新读 git-dir、refs、remote、HEAD、shallow
等。模块边界清楚，但“逻辑上独立的读取函数”变成了“每次都独立的昂贵查询”。

Rebased 的已读实现：

- `plugins/git4idea/backend/src/repo/GitRepositoryImpl.kt`：repoInfo 存当前分支、HEAD、
  remotes、tracking、状态等，getters 取已存在的值；update 比较旧/新状态再通知。
- 同目录 `GitRepositoryReader.kt`：按条件直接读 refs 文件/packed-refs，或一次
  for-each-ref 读取分支及 HEAD；支持非文件式存储的回退。不是每个 getter 启动 Git。
- 同目录 `GitRepositoryUpdater.kt`：按 config/index/HEAD/refs/rebase/tag 等分类触发
  repository、dirty scope、tag/config cache 的失效。

Lazygit `pkg/commands/git_config/cached_git_config.go` 将配置缓存绑定具体仓库目录，
显式 DropCache，而不是每次 Get 都启动 Git。这是另外一条可用证据，不表示其缓存
策略能原样满足 Plico 的外部配置变更和失败语义。

建议：在已有 ProjectRoots/Repository 体系内增加具体的仓库上下文，缓存 git-dir、
common-dir、worktree 身份，统一提供有版本的 refs/config/operation 展示快照。
目录身份变化、gitfile 替换、仓库重建必须失效；配置要覆盖 includes、global/system
及 worktree 配置来源。优先继续使用 Git CLI 批量读 refs/config，避免自行不完整解析
reftable、符号引用或条件 include。写操作仍在关键边界重新读实际状态。

## 根因二：安全校验与数据读取绑在一起，嵌套组合放大调用数

当前 Update 入口 `src/ui/remote.rs` 先等待项目 inventory，再调用
`Prepared::read_registered`。下层调用链为：

`Group::prepare_registered → Repositories::from_registered → Roots::from_inventory`
`→ operation_state + pull_targets → project_update::prepare → operation_state +`
`pull_targets + HEAD → merge_update::validate → pull_targets + HEAD + operation_state`。

实际单仓库 26 次：rev-parse 8、symbolic-ref 6、ls-files 3、remote 3、for-each-ref 6。
这些全发生在一次只读预检内。当前执行器还会在锁内、fetch 后、真正写入前复验；
后者有真实必要，不能用“重复”一词一起删除。

Rebased `GitUpdateProcess.java` 用 repository 状态生成 trackedBranches/updaters，
fetch、保存本地修改、更新和恢复是明确的阶段；它也做校验与失败恢复，并非无检查。

建议将“获取阶段快照”和“纯校验快照”拆开：同一预检阶段复用一次数据；进入锁、用户
确认后、fetch/restore 等状态变动后建立新阶段。不要把 UI 缓存直接授权给写操作，
也不要复用已跨过 await/用户编辑/外部写入边界的检查结论。

其他路径的源码定位（尚未新计时）：

| 操作 | 已确认的成本来源 | 可共享与必须保留的部分 |
| --- | --- | --- |
| Fetch | 配置校验、操作锁身份查询、完成后状态/历史刷新 | 共享配置与身份；失败/取消后仍检查真实 refs |
| Push | push_snapshot、outgoing 预览及结束复验、操作后刷新 | 批量 refs/config；保留 lease、远端目标及 HEAD 复验 |
| Pull / Update / Rebase | 多层 validate 重读 targets/state；保存恢复阶段额外读取 | 同阶段共享；fetch、stash、冲突继续前后重新取样 |
| Checkout | 引用格式/目标验证、HEAD/operation 检查、完成后 state/head/branch | 复用一次结束快照；保留预期引用/HEAD、冲突与恢复保护 |
| Commit / Amend | git-dir/状态/HEAD/config、排除 index 捕获、部分提交准备及恢复 | 合并稳定元数据读取；保留选中范围、index 保留和恢复记录 |
| 历史重写/冲突恢复 | 每阶段对操作文件、refs、index 的检查 | 先测阶段成本，不能将检查数少直接作为正确性目标 |

依据分别为 `src/git.rs::fetch_inner`、`src/push.rs::execute`、`src/merge_update.rs`、
`src/rebase_update.rs`、`src/checkout.rs::attempt_local`、`src/commit_operation.rs`。
公网、认证、hooks、签名、冲突停留时间必须单列，不能算成客户端计算慢。

## 根因三：已批量化的 Push 没有消除 Log 的 N+1 查询

`src/log_changes.rs::load` 逐 commit 调 `log_commit_changes`，后者逐 parent 调
diff-tree；`src/ui/log_selection.rs` 再逐 commit 调 commit_metadata。
本次线性 20/50 条实测分别是 **20/50 条 diff-tree + 20/50 条 show + 10 条公共查询**。
它既不是一次昂贵 diff，也不是渲染 50 行很慢，而是串行启动 50/110 次进程。

Rebased `GitDetailsCollector.kt::readFullDetailsForHashes` 把 hashes 送入 stdin，
在单次 handler 中读取 metadata/name-status；DIFF_TO_PARENTS 分支明确收集每父记录。
`platform/vcs-log/impl/src/com/intellij/vcs/log/data/AbstractDataGetter.kt` 先找缓存，
只加载缺失项并回填；不是对每个已选提交重新计算全部内容。

建议把已存在 `git_push_changes` 的有界批量传输经验用于 Log，但**不能直接沿用 Push
的 combined merge 语义**：Log 当前按父 diff + merge_parents，必须保留。按 root 和
需求（metadata/name-status/per-parent）分组，复用一次读取结果；保持 root/add/delete、
rename、NUL 路径、shallow、输出上限、取消和返回 OID 校验。复杂 octopus merge 单独测试。

## 根因四：刷新已经有局部去重，但失效范围仍然过大

`ChangesRefresh`、`ProjectRoots` 已有 in-flight 合并；不要重造它们。
问题在于 `Review::refresh_impl` 每次非翻页刷新都刷新 roots、所有 branch-compare tabs、
清 history_count、清 details_cache，并刷新 LocalView，再加载历史。
`sync_repository_worktree` 虽然收到带 paths/index/refs 的 Batch，主仓库本地列表仍走
完整 refresh。局部调度各自正确，组合后仍会产生大量不必要的工作。

`detail_cache` 已按 repo/OID/parents/shallow 区分身份，32 项/2 MiB 有界；普通 refs
更新时整清会丢掉仍有效的不可变提交数据。应拆分不可变内容与动态 refs/归属信息，
保留提交内容、更新动态归属；不能把 shallow 边界也视作永不变化。

Rebased `VcsLogRefresherImpl.kt` 合并请求，先加载 recent block 并尝试 join 到已有
graph，无法 join 才扩大范围/回退；并有 load/join/build 分段计时。Lazygit
`pkg/gui/controllers/helpers/refresh_helper.go` 按 scope 调度，捕获仓库代次和命令实例，
无依赖任务并行，worktrees→branches 等依赖明确等待。

建议复用 `worktree_events::Batch` 演化为明确的失效路由：文件内容只更新相关文档/状态；
index 更新状态和本地基线；refs 更新 labels/count/history；membership 更新 roots。
同一根、同一域共享在途查询，显式操作后的更新与 watcher 更新合并。可见交互优先于
后台轮询，独立只读任务有限并行，写锁范围不放宽。按需刷新隐藏视图，不制造新一轮
重复请求。保留事件丢失、watcher 溢出、外部 Git 操作的定期复核退路。

低内存限制仍有效：借鉴 Rebased 的增量规则，不复制其全历史索引/全量常驻 graph。
Plico 应保持有界窗口、元数据 LRU 和按需内容读取。

## 根因五：全树 status 和只读策略之间有真实成本

`Snapshot::load` 用两次 HEAD 包围一次全树 status；MM 情况还需净工作树差异。
`local_changes` 固定 `--untracked-files=all --ignore-submodules=none`；当前 Batch 的
脏路径信息没有传到这个加载器。所有命令还显式 `core.fsmonitor=false`，只读层设置
`GIT_OPTIONAL_LOCKS=0`。这避免后台改 index，却也不能持久化本次刷新后的 stat 信息。

独立实验只 touch 10000 个测试文件，内容不变：只读 status 五次 **475–637 ms**；
允许一次可选 index 更新后，后续只读五次 **90–96 ms**。前一组 index hash 不变，
允许更新后 hash 改变，内容/HEAD 不变。证据 `status-cache.json`；没有修改产品或用户配置。
这是一次受控极端输入，不能推断所有仓库都会改善五倍。

Git 官方 [status 的后台刷新说明](https://git-scm.com/docs/git-status#_background_refresh)
解释了 stat 缓存写回与后台锁争用的取舍。Lazygit `file_loader.go::gitStatus` 仅在前台
status 允许可选锁；Rebased `GitImplBase.doRun` 也按配置抑制可选锁。因此不能简单宣称
“成熟客户端都写 index，Plico 应直接跟随”。

Rebased `GitChangeProvider.java` 按 dirty scope 更新，`GitStagingAreaHolder.java::refresh`
只替换对应路径记录；其 collectDirtyPathsPerRoot 会额外包含新增/删除及重命名两端，
避免路径过滤破坏 rename 检测；未跟踪文件由独立 holder 维护。

优先做同样的脏范围增量与分离未跟踪扫描，保留全树兜底。是否允许显式维护 stat cache、
何时支持用户已有 fsmonitor，需要作为独立语义设计；不能为了跑分放开所有 optional
locks，也不能直接删除 fsmonitor=false（涉及 hook/daemon 和 Job 生命周期）。

## Diff 与进程后端的取舍

普通提交 patch 本次只需一个命令；本地可编辑 Diff 的 `working_copy::read_base` 则按
HEAD→ls-tree→cat-file 读取；combined Diff 的 `read_contents` 按两端各启动 cat-file。
可以评估按请求批量读 blob 或有界常驻 `cat-file --batch`，但它不是本次多秒等待的首因。
常驻方案要定义单仓库所有权、取消/超时后的协议复位、对象大小上限、进程回收与空闲退出。
当前没有该方案的原型数据，不建议作为第一步。

同样，替换成 libgit2/gix 能减少某些启动成本，却不自动解决重复刷新和状态所有权。
还会引入 hooks、credential helpers、filters、worktree、reftable 等兼容性工作。
先修查询图，保留 Git CLI 的实际行为；只有剩余分段数据证明必要时再选定局部替换。

## 建议推进顺序与验收

1. **观测基线**：补生产层可选、低开销的 request/root/phase/query 关联及
   queue/spawn/run/drain/parse/publish 计时；默认不记录正文、凭据和完整命令参数。
   区分首次响应、内容可用、写操作完成、后台收敛，不再用单个 busy 时间混为一谈。
2. **仓库上下文和阶段快照**：先用 Update 预检 + refs/operation 观察两个真实场景落地，
   从 getter 重读转为纯校验。同一阶段 targets/state 各采集一次，写入边界明确重采。
   验收命令数下降和外部 HEAD/index/config 变化拒绝；不设置依赖本机的毫秒硬门槛。
3. **统一失效路由及不可变缓存**：复用现有 Batch/ProjectRoots/ChangesRefresh。
   空 Fetch 不读 log；普通文件修改不发现整个项目；操作完成不再由 watcher 重读同一代。
   refs 改变不清无关 OID 内容，shallow/仓库替换仍失效；缓存规模和内存预算不提高。
4. **Log 批量 details**：验证 1/20/50/跨批次提交的进程数按批次增长，避免线性 N+1；
   保留 per-parent merge 语义，内容继续惰性加载。
5. **工作树增量**：单文件编辑不触发全树 untracked 扫描；rename、目录删除、ignore、
   submodule、watcher 丢失等场景与全量结果一致。再决定 stat cache/fsmonitor 策略。
6. **剩余进程与计算热点**：根据低开销 trace 再评估事件等待、blob 批处理、复杂图布局、
   大 Diff 解析。后台有限并行必须同时验收峰值内存和取消后进程数。

这不是一次全仓重写。阶段 2、3 是共用机制，阶段 4、5 是它们的消费者；每段单独保留
源码依据、原生前后对比和回归，不再用一个入口变快代替架构问题解决。

## 参考版本与验证

- Rebased：`245042348669114adb8e24e5b0fc0ce73763f3c4`，长期 checkout
  `~/projects/rebased`；安装版此前实测 1.1.22 / 262.10968.SNAPSHOT，二者不等同。
  上文列出的文件均已读取相关实现；Refresher/DataGetter 的稀疏缺失文件通过 git show 获取。
- Lazygit：`4e1eebe278790f257a875da6633234391a5a85b3`，checkout
  `~/projects/lazygit-reference`，本轮仅源码对照，未运行该客户端，不宣称实测速度胜负。
- Git 官方文档仅用于解释 status/cache 机制，不把配置选项当作已经实施的优化。
- 诊断 example release 编译、实际两种规模运行通过；`cargo fmt --package plico -- --check`
  与 `git diff --check` 通过。没有修改产品代码，因此没有重复约半小时的全量功能测试。
  没有提交或推送，没有新的 CI 或产品提速宣称。

## 推进记录：阶段 1 后端观测基础

上述审计完成后，开始按六项结构实施，整体目标仍在进行中。当前首先接入后端观测，
尚未将阶段 1 的 UI 首次响应/publish、跨异步请求关联列为完成；阶段 2–6 尚待推进。

`src/performance_trace.rs` 通过启动前设置 `PLICO_PERF_TRACE` 启用，默认关闭。输出是
新建的 JSONL 文件，不覆盖已有文件，最多 16 MiB，容量耗尽或写入失败停止记录。
记录 request/query/operation/queue、parent、进程内加盐 root 标识及微秒耗时；不输出
仓库路径、参数、正文或错误字符串。调用名称使用静态白名单。root 当前按传入路径标识，
尚不是跨路径别名的 canonical repository 身份，后续与仓库上下文一起完善。

Git 读取层记录 spawn/run/drain，输出线程另记 `output_read_parse_us`；后者包含等管道
和消费/解析，与 run 并行，不是纯 CPU parse，也不能与 total 再相加。operation_process
记录相同生命周期，operation_queue 记录锁等待。失败、取消、超时仍执行原清理流程。
单个 span 的文件记录成本不计入其自身 total，嵌套子记录成本包含于父请求；需通过
开关对照验证观测扰动，不把日志值直接当作零开销真值。

Changes、Log 聚合和当前 Update 预检已在实际阻塞入口创建请求范围；线程内作用域
不可 Send，不能跨 async suspension 或自动继承到线程池。还没有接入的上层请求在
query 中 parent=0，不能据此声称全链路关联已经完成。没有增加通用任务调度框架。

依据是同版本 Rebased `GitHandler.runInCurrentThread/logTime` 的 command span，及
审计已读的 VcsLogRefresher 分阶段计时。此步仅加入默认关闭的诊断，不改变界面或 Git
语义，没有将历史原生操作截图冒充本次性能优化验收。后续各行为改动仍执行原生前置与
实现后同条件验证。

定向隔离测试 `artifacts/test-runs/27f0-18e2fd8ca2e74398-0/`：17 项通过、940 项被
显式过滤，包含新增计时上下文/敏感输入/容量/文件不覆盖测试、写队列取消及既有完整
process_tree/operation_process 流程。不能称作全量通过；完整核心验证与观测开销实验
另行记录。

### 后端观测开销实测

同一 release example、小仓库、无 Trace2，先关闭后开启计时，未并行运行构建或测试。
证据：`artifacts/deep-performance-20261029/phase1-{disabled,enabled}.jsonl`、
`phase1-events.jsonl`，分析脚本 `phase1-analysis.cjs` 与结果 `phase1-summary.json`。
HEAD 各 20 次，其余请求各 5 次，中位数如下：

| 请求 | 关闭 ms | 开启 ms |
| --- | ---: | ---: |
| HEAD | 44.682 | 44.794 |
| Changes | 137.322 | 137.547 |
| 当前 Update 预检 | 1090.157 | 1090.172 |
| Log 选择 20 项（含元数据） | 2138.089 | 2145.707 |
| Log 选择 50 项（含元数据） | 4813.422 | 4717.804 |

这是单轮顺序对照，存在缓存和时间漂移，不能将微小变化视为固定开销或性能提升。
没有重现之前 Trace2 对小查询的大幅放大；后续优化仍使用关闭诊断的端到端测量。
1425 条事件、366291 字节，parent 引用完整，root 均为不透明标识。
各轮 Changes 3 个子查询、当前 Update 26 个；Log 聚合选择 1/20/50 项分别为
11/30/60 个子查询，外层元数据读取不在该 span 内，不能用后者替代完整请求成本。

格式检查、diff 空白检查、核心 clippy（存在既有警告）、doctest（0 项）通过；桌面
编译通过。完整核心回归已启动，记录目录 `artifacts/test-runs/1a74-18e2fdf8c95dc7ec-0/`，
尚待最终结果，不标为全量通过。此阶段仍未改变产品操作行为或宣称完成提速。

### Changes 跨线程诊断关联

补充 `Request/RequestContext`：请求身份显式从 UI 传到阻塞 worker，仅同步 `scope`
期间安装线程局部上下文；跨 await 不持有上下文，panic 后恢复之前的 worker 上下文。
`ChangesRefresh` 在现有调度点记录 started-event、worker-start/finished、feedback-notify、
result-event；未接纳即销毁的请求记录 abandoned，已接纳的失败记 error。既有排队合并、
300 ms 反馈、结果发布及取消逻辑保持原样，测试 Loader 只增加诊断上下文参数。
快照读取、恢复状态和可选 ignored 查询现在可归到同一 changes-refresh 请求。

这些 milestone 是调度/事件发送时刻，**不是实际绘制帧时间，也不证明订阅者完成处理**；
其他入口尚未接入，阶段 1 仍未全部完成。日志中的 milestone 使用 elapsed_us，异步 request
使用 total_us，不伪造同步 spawn/run/drain 阶段。RequestContext 可跨线程，Span 仍不可 Send。
来源仍为上述 Rebased 分阶段观测；本次只接入诊断，未更改可见行为，没有新的原生验收宣称。

验证过程保留：桌面定向首次构建 `3e40-18e2fe39fc9a15fc-0` 暴露 local_view 的测试
Loader 仍为 4 参数，已同步为 5 参数；核心定向 `5478-18e2fe3c6279da54-0` 遇到
Windows LNK1104（全量测试占用 editor.exe）。此前全量运行期间有重编译，不能将其作为
一致构建的最终验收。后续完整验证需在冻结本轮修改且不并行重编译的条件下执行。

新增验证：`57ac-18e2fe516fe41cf8-0` 核心 `--lib` 定向 4/4 通过（352 项过滤），
覆盖跨线程显式归属、panic 恢复和 abandoned；`3ac8-18e2fe4f0f8ebc70-0` 桌面定向
1/1 通过（187 项过滤），启用真实诊断文件，结果见
`artifacts/deep-performance-20261029/changes-lifecycle-events.jsonl`。三个请求分别为
ok/error/abandoned，只有慢请求产生 feedback-notify，已核对父请求 ID。此测试使用
GPUI 虚拟时钟与测试 Loader，只验证生命周期和诊断路由，不用其中微秒数评价实际性能，
也不冒充生产 worker 或原生帧时间验证。格式及 diff 空白检查通过。

## 推进记录：阶段 2 的 Update 预检消费者

原生前置、源码及差异记录见 `REBASED-REFERENCE.md` 的“Update 预检阶段共享”。
实现先消除同一预检调用链里的重复状态读取：root eligibility 已读 operation_state，
已读 pull_targets，现在将 targets 作为值交给私有 prepare_observed 消费，而不是再次
通过 getter 读一遍。单仓库 prepare 同样构造本次 request，不立即重跑 validate。
这不是跨请求缓存；没有增加常驻仓库状态，也尚未完成阶段 2 的第二个观察消费者。

Group 的执行入口仍对全部 branches validate 后才 Fetch；Fetch 后、保存/更新前的原有
复验未删除。确认期间任意 root 状态变化仍应拒绝；新测试
project_update_shared_preflight_revalidates_every_root_before_fetch 覆盖第二 root 的
tracking/branch/operation 变化，并核对第一 root 的 HEAD/FETCH_HEAD 及第二 root 的
index/工作区内容。结果、命令数与原生 Plico 对照待本轮验证后补充。

阶段 2 Update 验证：8/8 定向通过（`4e0c-18e2ff256e631468-0`）；原生 Plico 同仓库
完成预检/取消/空更新/无上游拒绝，截图、构建 hash、结构差异和未验证状态见参考记录。
生产事件证实单根成功预检 26 → 11 个 Git 查询。关闭 Plico 测试进程后，无并行构建或
测试、无 Trace2、关闭诊断，在与阶段 1 相同 small fixture 上复测：5 次当前 UI 使用的
registered-preflight 中位数 1090.157 → 499.689 ms；旧非 UI read 路径 2133.289 →
1323.937 ms，不能把后者用于当前 UI。未改的 Log 50 项 4813.422 → 5101.519 ms，
说明存在时序/系统波动，不宣称其他操作已提速，也不把此数作为跨设备硬门槛。
原始 `phase2-disabled.jsonl`、分析 `phase2-analysis.cjs`、汇总 `phase2-summary.json`
均在 `artifacts/deep-performance-20261029/`。当前版本全量 core 将在 fmt/clippy/doc
通过后顺序运行；不再并行重编译其测试可执行文件。

### 下一消费者的调用链核对与字段实验

本轮冻结产品代码等待当前版本全量回归，读取了 `ui/repository.rs::refresh_impl`
后台路径、`git.rs::snapshot_query/shallow_boundaries/operation_state_at`、
`branch_updates.rs::HistoryObservation/References`、`operation_state.rs::observe`、
`project_operations.rs::Observer::poll` 和 recovery record 存在性查询的实际实现。

| 当前同阶段重复 | 具体调用位置 | 下一步所有权与不能省略的检查 |
| --- | --- | --- |
| for-each-ref / HEAD | HistoryObservation::read → References::read，随后 snapshot_query 再读 | 单次历史请求持有不可变 refs 输入；计数与标签各自从它投影，取消/过期结果不发布 |
| shallow 判定/边界 | HistoryObservation::read 与 snapshot_query | 同次请求共享完整边界集合，分页 fixed_tips 不推进 observer baseline；不能只共享布尔值 |
| per-worktree git dir | operation_state 加 read_primary_recovery 内三个恢复状态入口 | 同步阶段解析一次目录，再交给各记录所有者读取；错误仍为 unknown/保守禁用，不能变为无恢复记录 |
| operation observe 中目录 | Observation::read → operation_state → Observation::read | 首次采样与 state 可共享路径，末次采样仍需验证 .git 指向；不能把路径缓存永久化或把 linked worktree 的 common-dir 当作 index/HEAD 目录 |

`References::read` 的 7 字段为 refname/objectname/HEAD/upstream/remotename/remoteref/push；
`snapshot_query` 的 7 字段还需要 peeled object、object type 和 peeled type，不能只复用
已有 References（它没有这些字段）。候选共享输入应是两个格式的并集；读取上限继续
沿用 OUTPUT_LIMIT，不另建无界全仓缓存，保留现有深层 tag fallback 兼容路径。

真实 Git 字段实验（不是产品或原生 UI 验收）：
`artifacts/deep-performance-20261029/reference-fields-experiment.cjs` 和同名 JSON。
在新独立 `%TEMP%/plico-ref-fields-SEbgBl` 上读取 11 字段并集，覆盖 attached、
unborn（同时存在其他 refs）、detached、local upstream、annotated/nested tag、tree tag。
本机 Git 2.56.0 下：unborn 与 detached 都没有 HEAD=* 的 ref 行，但 symbolic-ref
前者成功、后者失败，因此不能用“没有当前 ref 行”代替 detached 判定。本地上游的
remotename 是 `.`，不能按普通 remote 分支强制解释。nested tag 的对象确实指向 tag
（额外 cat-file -p 核对），本版本 for-each-ref 的 peeled type 已为 commit；保留旧版本
fallback，不基于单版本实验删除兼容逻辑。tree tag 保持 type=tree，不得加入 log tips。

后续回归入口已核对：branch_updates 的 unborn/tag/cancel 和 triangular push 计数；
repository 的 shallow observation；project_operations 的外部 merge、index resolve、
abort、失效仓库与取消。新增测试需补“共享输入的不同投影保持现有结果”、unborn 分支名、
linked worktree 与 .git 指向变化，而不是只断言某个 getter 不再被调用。原生需另外复现
refs/操作状态变化，再实施对应行为调整；Update 的旧截图不能替代这个消费者的前置。

### Update 基线完整回归与历史输入共享实现

冻结的诊断 + Update 版本完整 core 回归：`5fb4-18e2ff61382914c4-0`，
959/959 通过，0 skipped，1111.338 秒；运行期间没有重编译其测试可执行文件。
此结果不覆盖随后新增的历史输入共享代码，不作为当前全部改动全绿的声明。

历史共享的本次 Rebased 原生前置、截图和源码映射见 `REBASED-REFERENCE.md` 的
“refs 共享读取前置（2026-10-29）”。实现位于 `git_history_inputs.rs`：单次请求持有
refs 字段并集、HEAD、symbolic HEAD、remote names 和完整 shallow 边界；普通历史、
筛选及分页重建复用该输入。UI 从同一输入获得刷新观察基线，仅成功且未过期的当前请求
可发布；固定 roots 翻页仍不推进基线。后台 References 轮询保留原来的轻量字段查询。
深层 tag fallback 使用捕获的对象 ID，避免解析期间 ref 移动混入新目标。

这不是跨命令原子快照，也不是跨请求缓存；不会消除 Git 自身或外部写入造成的所有竞争。
输入受现有输出上限约束并绑定仓库路径，不交给写操作复用。操作目录观察共享、定向失效、
批量详情、dirty-path Changes 等后续阶段尚未完成。

当前新增验证：

- `4c60-18e300ae94e58924-0`：2/2 新测试通过，356 过滤。覆盖读取后 refs/tag 移动、
  仓库隔离、取消，以及 unborn/detached/本地 upstream/triangular push 的观察等价性。
- `3170-18e300b696b64c90-0`：repository 定向 37/37 通过，98 过滤；包含历史、筛选、
  分页、shallow、标签及多根图形导航相关回归。
- `3730-18e300d03ff2ccd4-0`：branch_updates 3/3 通过，无过滤。

性能实验使用 example 的 `--history-inputs` 模式：在当前版本中交替比较独立观察读取与
共享观察读取，隔离重复查询本身的成本；它不是旧版完整历史实现的二进制对比，不能用于
宣称分页或全 UI 的总体提速。

桌面定向 `3178-18e300d32745d4bc-0`：7/7 通过，181 过滤，覆盖请求替换/迟到丢弃、
显示身份、刷新期间打开 Diff 及 Changes 合并。诊断 example 首次编译发现引用私有 Span，
改用已有公共 Request/RequestContext 后 release 编译通过，没有扩大 Span 的可见性。

性能结果：large fixture（2000 commits / 10000 files），每次读取 1000 commits；无并行
构建/测试，关闭 Trace2 与 Plico 诊断，交替顺序各 10 次。独立观察中位 525.141 ms，
共享观察 372.029 ms。随后单独启用诊断核对：每轮 query 数分别为 11 和 8。
证据 `artifacts/deep-performance-20261029/history-inputs-{disabled,enabled,events}.jsonl`。
这是当前 reader 内重复观察的受控对比，不是旧二进制端到端对比；分页重建额外复用的
收益及原生画面可交互延迟没有在这组数据中测量。

原生 Plico 同 fixture 的新增/删除 refs 自动刷新已验证，选择、文件列表及详情保持，
截图和具体差异见参考文档。release 构建、fmt check、diff whitespace check 通过。
核心 clippy 输出存在既有警告；首次 PowerShell 重定向返回 1 但日志显示 Finished 且无
编译错误，随后显式检查 Cargo LASTEXITCODE 为 0（`history-inputs-clippy-recheck.log`）。
当前历史共享版本没有重新跑完整 core/desktop/navigation，不能使用之前 959/959 结果
替代当前版本全量验收。下一步继续操作状态目录共享审查及后续审计阶段，最终冻结后
顺序执行完整回归，避免运行中重编译导致验证证据不一致。

### 操作状态与恢复记录：共享工作树目录

本次原生 Rebased 前置与源码核对见参考文档“操作状态目录共享前置”。
`operation_inspection.rs` 聚合操作状态和三种恢复记录的展示信息；目录在本次读取中
解析一次，结束重新解析确认指向仍一致。恢复记录的格式/存在判断仍由 merge_saved、
revert_operation 各自拥有，读取错误保留原有保守阻止操作的行为。
`operation_state::observe` 使用首次 Observation 的工作树目录读取 state，末次
Observation 仍重新解析目录并检查 index/markers；未变化的观察路径保持不变。
写入口继续自行读取，绝不使用 Inspection 授权执行。

界面普通刷新原来的 state + 三个 recovery getter 为 6 条 Git 查询；现在即使多保留
一次结束目录复验仍为 4 条。项目导航只需要 recovery 时使用 RecoveryPresence，三次
重复目录读取合为一次。不是常驻目录缓存，不混用 linked worktree 的 common-dir。
这也不是全仓状态原子事务，不保证检测外部 ABA；状态变更后的写复验仍不可省略。

`project_operations` 测试 `3ab4-18e301b4d540d260-0`：6/6 通过，无过滤。新增 linked
worktree 恢复记录隔离、.git 在两次请求间改指向、损坏记录、missing repo、取消测试，
并在外部 merge/解决 index 流程中核对 Inspection。此测试不声称已覆盖读取中途的 ABA。

example 新增 `--operation-inputs`，交替调用旧的独立 getter 组合与共享实现。
fixture 为本次原生操作测试的三提交仓库（已跟踪文件 clean，Rebased 生成 `.idea/`），各 10 次，无并行构建/测试，
关闭 Trace2 和 Plico 诊断的中位数：

| 入口 | 独立读取 | 共享读取 | Git 查询数 |
| --- | ---: | ---: | --- |
| 操作状态 + 恢复记录 | 269.197 ms | 174.077 ms | 6 → 4 |
| 无旧 baseline 的操作观察 | 228.342 ms | 175.007 ms | 5 → 4 |

随后独立启用诊断核对各轮查询数。原始数据在
`artifacts/operation-context-20261029/{disabled,enabled,events}.jsonl`。
这里只测后端组合路径，不包含 UI 绘制；未变化轮询没有本次收益。恢复/序列操作回归及
Plico 同条件原生验证继续执行，尚不将阶段 2 或整个目标标为完成。

后续结果：`29dc-18e301d7a7748188-0` 的 revert_operation/sequencer_operation 共
23/23 通过，无过滤，含 1 项 slow（68.883 秒，未重试）。桌面定向
`1fb0-18e301f7a3932540-0` 7/7 通过，181 过滤；实际匹配 log_query 和 sequencer
测试，history_operations::tests 过滤词未匹配用例，不据此声称其已验证。
Plico 原生同条件操作状态验证通过，细节见参考文档；release 构建、fmt、diff check 和
core clippy 通过（既有警告保留，`operation-context-20261029/clippy.log`）。

### 阶段 3 开始前：校正实际失效范围

继续读取当前 `ui/auto_refresh.rs`、`ui/worktree_sync.rs`、`ui/project_roots.rs`、
`worktree_events.rs`、`worktree_events/project.rs`、`ui/remote.rs::start_fetch` 及
`ui/repository.rs::refresh_impl` 后，不能把笼统的“所有事件都会全量刷新”当作现状：

- 普通文件事件已有路径分发；完成的 membership 快照下，普通文件改名不会无条件重新
  发现项目。index、目录新增/删除、.git/.gitmodules、fallback all 仍会触发 membership。
- 单根、完整渲染且 baseline 可用的成功空 Fetch 已比较 refs+shallow 并保留 Log；
  失败/取消、unshallow、多根和查询代数变化仍退回完整刷新。下一步需测通知重入与多根，
  不应再给现有单根快路径重复叠加一层开关。
- refs 轮询检测变化后仍调用宽泛 refresh_impl；它同时刷新项目发现、比较 Tab、Changes，
  清 details cache/count，再读历史。这是已确认的耦合入口。
- watcher 浅克隆变化同样进入宽泛 refresh_impl；child refs 则有独立 project Log 重读。
  消费端不一致，不能简单把一个布尔值改成“不刷新”而丢掉 shallow 或 child 失效。
- 原生 watcher 的首次 all、linked worktree/network 的周期 all，以及进行中的 catalog
  对路径事件保守 follow-up，需要分别区分初次加载、失效与读取结果发布的代数。

下一阶段应建立具体事件→消费者映射及验证矩阵，保留原有安全 fallback；先做新的
Rebased 原生前置，再修改失效路由和有界 OID 缓存，不增加无界常驻状态。

### 阶段 3 依赖实证：OID 并非详情查询的完整键

读取 `detail_cache.rs`、`ui/log_selection.rs::select_commit_with_file/apply_commit_details`
及 `git.rs::commit_metadata/details/command`：缓存键为 path/OID/parents/shallow；
Details 不含引用标签，展示层从当前 Commit 重新组合标签，containing branches 另查。
但 Git 命令仍尊重 replace refs 和 diff 配置，不能因此直接删除 refresh 中的 clear。

新增两项真实 Git 测试（tests/repository.rs）：

- `detail_content_can_change_without_oid_parent_or_shallow_key_changes`：同树/同父替换
  提交改变正文；OID、parents、shallow 不变，最初的 HistoryObservation 也未变化。
  现有 Cache 仍命中旧 Details，只有显式 clear 后重读才得到新正文。这是在证实当前
  缓存的外部失效责任，不是将过期显示规定为产品行为；后续补 replace 观察时应更新
  该测试的观察断言，并增加 UI 自动失效断言。
- `detail_rename_detection_depends_on_config_even_for_fixed_commit_ids`：两个非精确
  重命名，diff.renameLimit=1 时为四条增删，=1000 时为两条重命名，正文相同。

第一次编译 `1344-18e302743e4682f4-0` 漏传 snapshot 的 fixed 参数，修正后
`3784-18e30286f9b7ecdc-0` 2/2 通过，135 过滤。fmt 和 diff check 已执行。
这两个测试仅证明依赖，当前没有缓存提速或新失效策略的交付声明。

对照 Rebased 当前缓存生命周期、config 通知和原生 branch/replace 场景，详见参考文档
“详情缓存与引用失效前置”。安装版对本次 replace 后手动刷新仍显示旧正文，不据此复制。

下一实现应按下表分配责任，并在实施后逐项验证：

| 输入变化 | 必须更新的消费者 | 可以保留的内容与边界 |
| --- | --- | --- |
| 普通 heads/remotes/tags | Log roots/标签、计数、包含分支、比较目标 | 同仓库、解释环境未变的 Details；不无条件重扫项目 |
| HEAD/index | Changes、操作状态、工作区比较；HEAD 还更新 Log | 固定提交内容可保留，分支/index 派生值不可复用 |
| replace/grafts | 历史拓扑、消息、详情、Diff、计数 | 当前普通 refs fingerprint 不覆盖，需新观察依据 |
| local/common/worktree config | 配置依赖的详情/比较、remote、eligibility | common config 必须正确路由到 linked worktree；包括 include/global 的保证范围需明示 |
| shallow | 图边界、比较基线、计数、详情 | 不能只看 HEAD，不能让子根走另一套漏失效路径 |
| .git/根替换、watcher rescan、失败 | root identity、membership、相关缓存和异步结果 | unknown 保守全量；仍保持规模上限与迟到结果丢弃 |
| 普通文件/目录 | 路径相关 Changes/文档/文件树，声明或根祖先变化才重扫 membership | 不把普通正文改动误当作所有 OID 内容改变；attributes 等解释文件需单列 |

已确认缺口：`GitPaths::route` 对 linked worktree 的 common config 不路由；
`Pending::push` 对 .git/config 走 continue 没有分类；refs/replace 虽触发 refs 事件，
primary Log 依赖不包含 replace 的轮询，child 路径另有仅重读历史的处理。先修全链路
依赖，再缩小刷新；不能只在 cache.get 前加一个命中开关，或用提高缓存预算掩盖问题。

### 阶段 3 首项落地：替换对象的完整失效链路

在已有 Rebased 原生前置和源码核对后，先修复缓存正确性，再讨论减少刷新：

- `branch_updates::References` 和 `git_history_inputs::HistoryInputs` 在原有 for-each-ref 中加入
  refs/replace，不新增 Git 子进程。历史解析跳过该命名空间，不把替换对象当额外分支、标签或历史起点。
- `detail_cache::invalidate_repository` 按规范化根路径清理，重新计算字节预算；其他根的数据保留，
  32 项 / 2 MiB 上限不变。子仓库历史事件也经过此入口，普通 index 事件不清理固定提交详情。
- 第一轮原生验证暴露第二层缓存：日志标题更新而右侧正文仍旧。原因是
  `loaded_log_selection` 依据 root/OID/parents/shallow 直接复用结果，绕过已经清空的 Cache。
  因此 `ui/log_selection::invalidate_loaded_details` 同时失效当前加载选择；watcher 的 all、shallow、
  interpretation 事件以及主根轮询的 replacement fingerprint 变化均接入，避免只修一层。
- `worktree_events::Batch::interpretation` 区分替换引用；packed-refs/reftable 无法从文件事件辨别
  命名空间，保守按解释变化处理。主根轮询补充比较替换引用，覆盖遗漏 watcher 通知的路径。

真实 Git 回归现在断言 replacement 会改变 observation，轻量观察与共享观察一致，且不会扩大历史起点。
桌面子根回归断言 index 保留缓存，替换事件同时清子根 Cache 和已加载选择，保留另一根缓存。
最终定向核心 `3f68-18e3039061bab0a8-0` 14/14 通过（482 过滤）；桌面
`36a0-18e303710e91acac-0` 6/6 通过（182 过滤）。之前 branch_updates 3/3 通过
（`5978-18e3032f55da75e0-0`）。release 构建通过。

最终原生复验：替换和撤销替换均无需手动刷新，日志与右侧正文同步更新，三条历史和 a.txt 选择保持。
失败首轮截图及最终前后截图都保留，详见 REBASED-REFERENCE.md。测试仓库 HEAD 未变，
替换引用已按 expected OID 删除，仅保留 Rebased 创建的未跟踪 .idea/；只关闭自建 Plico 进程。

本项是后续缓存保留优化的正确性基础，不能单独宣称速度提升。普通 refs 仍有保守清理；config、
grafts、linked common config 路由、全局/include 配置、root identity 和异步代数仍待继续审计。
当前版本尚未执行最终完整 core/desktop/navigation，不能引用之前 959/959 代替这一版本全量验收。

最终检查：cargo fmt --package plico -- --check 通过；core clippy 退出码 0，既有警告保留，日志位于 artifacts/invalidation-context-20261029/clippy.log。

### 配置依赖闭包与剩余消费者审计

继续读取 `GitRepositoryFiles.java`：common/mainDir + config 为配置路径，worktreeDir 单独保存
HEAD/index 等。`GitRepositoryUpdater.kt` 在 configChanged 后 update、清配置缓存并通知
GitConfigListener；不能把这个通知理解为所有详情消费者均已刷新。继续读取
`GitCommitTemplateTracker.kt`，它对配置中指定的模板路径另建 watcher，内容变更重读、路径变化
替换 watcher、dispose 清理；说明“配置文件变化”和“配置指向的外部文件变化”有不同生命周期。
源码仍固定于 245042348669114adb8e24e5b0fc0ce73763f3c4。

扩展真实 Git 用例 `detail_rename_detection_depends_on_config_even_for_fixed_commit_ids`：
创建 detached linked worktree；common config 的 include.path 指向 rename-settings。仅改被包含
文件中的 diff.renameLimit，common config 字节、引用观察和提交 ID 全部不变，详情由 4 条增删变为
2 条重命名。开启 extensions.worktreeConfig 后，仅 linked 的 config.worktree 覆盖为 1，
linked 返回 4 条而主工作树仍为 2 条。独立隔离运行 `5d08-18e3040941059cc8-0`，1/1 通过，
136 过滤，无重试；结束删除自建 worktree。本次仅补依赖实验，没有绕过原生前置修改产品行为。

本地完整消费链仍有三处边界，需在对应原生场景后一起处理：

1. GitPaths::route 排除 common config；Pending::push 忽略 config/config.worktree，包含文件
   也没有依赖注册。主根 refs 轮询无法补偿这些变化；不能仅增加一个 config 布尔值就宣称修复。
2. 主根 watcher 当前只有 shallow 主动触发历史刷新。新增 interpretation/config 事件必须有
   明确刷新入口与代数取消规则，不能只清缓存却继续展示旧内容。
3. `diff_tab.rs::disk_changed` 对 Area::Commit 和 stash 无条件返回；
   `ui/branch_compare.rs::refresh(false)` 在 Pair 不变时仅刷新标签。
   因此上一节已验证的 replacement 修复范围只是主 Log 和所选详情，并不覆盖所有已打开 Diff/
   比较 Tab。配置、替换对象及 shallow 的消费矩阵必须涵盖它们，尤其是事件在加载过程中到达的情况。

下一实现前先补 Rebased 原生 config/已打开比较场景，再确定有界依赖观测方案：识别生效配置来源与
缺失文件的创建、includeIf 条件变化及 worktree 差异；复用读取上下文，避免每个详情/轮询增加一次
全量 git config。未能覆盖的外部依赖须有明确保守回退，不能把缓存命中当作内容未变化的证据。

### 配置原生前置完成与端点语义校正

已完成本仓库 diff.renameLimit 1→1000 的 Rebased 原生对照及截图，见
REBASED-REFERENCE.md“配置与已打开 Diff 前置”。低限下四项增删、Diff 全文新增；Git 已识别
两条 R083 后，参考程序的列表和已打开 Diff 仍旧，手动刷新列表也未变。仅据此不推断所有上游路径。

继续读 GitContentRevision 与 GitLogDiffHandler 后，校正下一实现边界：详情列表的重命名识别可以
重新配对路径，但已打开的固定 Diff 代表用户当时选定的两个端点。仅配置 renameLimit 改变，不应
擅自把“空→new-x”Tab 改成“old-a→new-x”。因此不能直接把所有 config 变化等同 replace 并对
每个固定 Tab 重建目标；应分别处理列表解释、固定内容读取和工作区基线，保留 revision/path 身份。
replacement 改变同一 revision/path 的内容时仍须失效，普通 refs 移动则不需要。

测试分支 audit-config-20261029 保留以便 Plico 同条件复验，顶端
1608a1318a1acab52e5701a39f6eb711bf148810；原 main 已重新签出、临时 diff.renameLimit 配置删除，
原 HEAD 8c8998262f6b8e2b76969be1b8cb515cb942aceb，只有 Rebased .idea/ 未跟踪。
没有关闭任何 Rebased 窗口。本轮仅获得原生/源码证据并更新设计记录，无产品行为变更；
配置依赖观测与消费端实现继续待办，不能将这项前置验证算作阶段 3 完成。

### 阶段 3 配置观察首版接入

原生前置完成后新增 `worktree_events/configuration.rs`，每个已注册根保留 SHA256 摘要和下一检查
时间，不保留/输出配置值。后台通过 Git 自身 `config --null --list --includes` 解析 includeIf、
全局和 worktree 配置，复用现有取消及 8 MiB 输出上限；本地 config/config.worktree 事件及时检查，
其他来源每 30 秒检查。GitPaths 现将 common config 路由给 linked worktree，仍排除其他工作树的
HEAD/index/config.worktree。项目观察根上限 4096，撤销注册时释放状态。

通知与采样分开：首次读、摘要变化、读取失败/恢复标记 pending；重复读取不能清 pending，只有 UI
接受后 acknowledge。取消不更新当前根摘要/期限；拒收后下一轮继续交付。此处没有逐详情查询配置，
也没有自行实现 Git 配置语法或扩大 watcher 到整个用户目录。检查仍有每根每 30 秒一个 Git 进程的
成本，必须纳入后续空闲/多根基准；目前不声称净性能收益。全量配置摘要会对无关配置保守失效，
后续按已验证依赖细化，而非漏掉 include 或把解释环境当常量。

`Batch.configuration` 与 replacement interpretation 分开，触发详情缓存、已加载选择、历史及
比较消费者更新；固定提交 Diff 保持原端点。分支比较需强制重读同 Pair 内容，强制刷新安排在
refresh_impl 的普通 Pair 解析之后，避免任务被后者覆盖。历史仍沿用现有保守刷新入口，缩小刷新
范围尚未实施；replacement 的固定端点内容失效、grafts/root identity 等仍待继续完成。

核心定向 e94-18e304a3889b1d5c-0：9/9 通过，351 过滤；覆盖 include 从缺失到创建、文件损坏、
恢复、取消、未确认通知保留、事件分类和 linked 路由。桌面定向
36f8-18e304d744f4ea04-0：1/1 通过，187 过滤；同一子根配置事件清除详情和已加载选择，保留
另一根缓存。core/desktop check、release 构建通过，原有 sequencer 死代码警告仍在。
原生 release SHA256 8DB1F3195924F42302C4B09488686B18E5DFB80A6DEE96B51B863759A1C0895F。
同一测试分支、同一提交配置 1→1000 后四项自动变两项，截图 config-context-20261029 下
plico-limit1.png、plico-limit1000.png；无需重新选择或手动刷新。没有测绘制延迟，尚非全量验收。

原生 include 文件单独变化亦自动恢复四项，截图 plico-include-limit1.png；主配置与测试文件已清理，main 恢复。core clippy 退出 0，日志 config-context-20261029/clippy.log；fmt/diff check 通过。仍需多根开销、交付竞态和比较端专项验收。

### 配置观察多根交付与新增成本审计

新增真实 Git 的 ProjectEvents 集成测试，使用两个独立根，验证初次通知、只确认其中一个根、
拒收重投、取消后正常重试、移除根释放 pending。定向
5a50-18e30528e8f325ac-0：10/10 通过，351 过滤。进一步发现 pending 通知重投本身不需要
后台任务；configuration_due 现在排除这类仅待交付的状态（仍正常向 UI 发通知），避免忙碌拒收后
每 100 ms 发起一次无查询的 background_spawn。新增断言覆盖此区别。

example 增加 --configuration，直接调用生产 ProjectEvents::poll_configuration；watcher/reconcile
安装在计时外。独立创建 16 个空仓库，关闭 Plico/Trace2 诊断，固定测试 global config，无同时运行
的构建/测试。每种规模 10 次，合成推进 Instant 触发 30 秒到期，不声称真实观察延迟：

| 根数量 | 中位数 | 最大值 |
| --- | ---: | ---: |
| 1 | 41.39345 ms | 66.6679 ms |
| 16 | 683.31105 ms | 837.3672 ms |

证据 config-context-20261029/scale-one.jsonl、scale-sixteen.jsonl，fixture 路径在 scale-roots.txt。
所有仓库只用于自建测试，无网络。未测 4096 根，不能线性外推成实测数字；本数据也不是端到端 UI
延迟，更不是相对 Rebased 的性能结论。

设计结论：当前 worktree observation loop await 整批配置检查，然后才交付 notices/poll operations。
即使 UI 线程未阻塞，普通文件事件与操作状态仍会被多根配置查询拖延。必须继续将配置采样与通知
交付解耦或采用公平、有界的分批调度，并测试无饥饿、取消、根集替换与拒收重投。不能把“后台”或
“30 秒一次”当成已解决成本。尚未开始保留普通 refs 跨刷新的 Details，因此仍不能宣称缓存提速完成。

去除无查询后台任务后的定向回归 1200-18e30547c96115f4-0：1/1 通过，360 过滤；fmt/diff check 通过。多根串行等待尚待下一步修正。

### 配置调度改为单查询轮转

将 ProjectEvents::poll_configuration 改为每次最多一次 Git 查询，Configuration 增加 requested，
与 pending 分别表示“仍需采样”和“仍需 UI 接收”。本轮未处理的文件事件保存为 requested，下一轮
无需重复收到 OS 事件；成功读取才清 requested，取消保留。ProjectEvents 持有路径 cursor，从上次
根之后寻找 eligible root，再绕回，避免持续变动的前排根挤占其他根。移除 cursor 所指根仍能继续。
既有工作树循环每轮后执行通知/操作状态及 100 ms timer，因而不再一次 await 全部根配置查询。
没有增加线程或并行 Git 数，也没有扩大配置值缓存。

核心定向 `668-18e3057381193c50-0`：11/11 通过、351 过滤。新增真实 Git 测试覆盖三个根，
持续向首根发事件时后两根仍依次采样；延后事件无需重发；删除 cursor 根后继续；既有按根确认、
取消与拒收回归仍通过。fmt/diff check、release example 构建通过。桌面实际多根通知交错仍待专项
验证，不以这些同步测试替代原生验收。

同一 16 空仓库、相同诊断关闭环境、无并发构建/测试，连续执行 10 轮、每轮 16 个生产批次：
`scale-sixteen-batched.jsonl` 中 160 次 batch 中位 41.67955 ms、最大 70.2888 ms；10 次完整
cycle 中位 720.81015 ms、最大 789.9509 ms。之前整批中位 683.31105 ms。本次没有减少 Git
查询数或总工作，且 example 的逐批记录也在完整 cycle 时间内；只能支持“单次 await 不再累计
16 根”的结论，不能宣称总体吞吐提升或端到端延迟等于 42 ms。真实 UI 每轮还会执行其他任务和
100 ms 等待，完整覆盖时间会更长。30 秒是每根再次到期的最短周期，不是数千根均能在 30 秒内
完成的承诺；单个慢 Git/操作状态查询仍能延后工作树循环，后续继续检查剩余串行链路。

此实现对应已有配置原生前置和来源解析语义，不改入口、文件对或配置优先级。主 Log 配置更新的
上一版原生验收仍保留，新调度多根原生复验待完成；不得据此将阶段 3 或全项目验收标为完成。

### 多根原生证明与下一失效范围

单查询轮转版本三根原生验证已完成，详见 REBASED-REFERENCE.md 对应小节。子根配置自动更新，
同 OID 主根仍使用自己的配置，选择和聚合历史保持。Trace2 证据为 multi-trace2.jsonl，
变更起点 multi-change-start.txt，10 秒窗口查询分组 multi-change-query-summary.json。
该窗口共 107 次 start，包含正常周期观察，不能全部归因于配置变更；其中三个根均有历史查询，
--show-toplevel 各 3 次，共 9 次。仅此数字不代表每根完整扫描了三遍，因为目录查询存在多个调用口。

源码调用链核实：worktree_sync 的 configuration 仍走 refresh_impl→project_roots.refresh，
后者 Mappings.refresh_persisted→Repositories::from_registered→submodules::list，再读取每根
shallow。ProjectRoots 已有发现中 pending 合并，不会每次杀掉扫描，但仍需后续扫描，且 checking
暂时失效项目历史 snapshot。刷新同一根配置因此会扩大到全部根和 Changes；这是下一步明确要收窄的
消费路径，不能因为配置采样已分批就宣布刷新架构完成。

后续需要区分影响仓库成员/路径解释的配置与仅影响详情呈现的配置，保留未知/失败的保守回退；
不能不加区别地跳过所有 config 的成员发现，因为 core.worktree/bare 等可改变仓库识别。
submodules::list 当前直接读取 .gitmodules 并检查 index，配置、声明与 gitlink 事件需分别映射。
本轮补原生证据与消费链核对，无新增功能代码；未把诊断开启时的数据冒充无诊断性能对照。

### Diff 配置定向失效落地

配置观察在同一次已存在的 Git 输出上计算完整摘要与排除 diff.* 的摘要；只有 diff.* 变化且已建立
基线时发 Batch.diff_configuration。其 changed/baseline_changed 为真，history_changed 为假。
首次观察、读取错误/恢复、任何非 diff 配置改变仍走 broad configuration；pending 的 broad 范围
只允许升级，不能被随后 diff 变化降级。include.path 自身改变仍保守，包含文件内仅 diff 值改变可
走窄路径。不新增 Git 查询，不保存配置明文。当前 history 查询未使用 --follow/pickaxe；配置来源
和 Rebased GitRepositoryUpdater/Details 生命周期依据沿用既有前置，未将上游旧详情行为作为目标。

UI 只清对应根详情缓存与 loaded selection，refresh_diff_configuration_details 仅重读受影响
选择。已有 Details 请求被替换并拒绝旧结果；History/Navigation 请求保留，由其完成后的 reconcile
加载新详情。主 Log 行、项目成员和计数不因纯 Diff 配置重新查询；比较消费者仍按对应根刷新。
固定 Diff 原端点不改变，replacement 端点内容失效仍是后续独立项。普通 refs 缓存跨刷新的优化尚未做。

首次新增范围测试 1868-18e3063e6f6443bc-0 为 11/12：测试将 Windows 初始已为 true 的
core.ignorecase 再设为 true，却期待范围升级。改用新增 core.abbrev=12 来制造真实非 Diff 变化，
不改产品逻辑迁就测试。随后核心 587c-18e30658f91cae54-0 为 12/12（351 过滤）；桌面
5e68-18e3065985dd9df8-0 为 1/1（187 过滤），含取消旧详情、保持历史行和多根选择重新加载断言。
桌面 check、release 构建、fmt/diff check 通过；尚未跑最终完整套件。

原生使用同一三根 fixture、相同配置初值与选择流程；release SHA256
D52F9671261CA44AB6EE17D387B007609E0A515C12CFD42652D989AF0400D635。repo-2 配置 1→1000 后
四项变两项，12 行 Log 与选择保持。narrow-before.png/narrow-updated.png；诊断 narrow-events.jsonl、
narrow-trace2.jsonl、起点 narrow-change-start.txt、分组 narrow-change-query-summary.json，均在
config-context-20261029。变更后 10 秒共 26 次 Git start（上版同场景 107），log 0、show-toplevel 0。
窗口包括正常轮询且未锁定轮询相位，两版均启用诊断；不将 107→26 比率宣称为端到端时间提速。
它直接证明此次不再因 diff.renameLimit 改变读取所有根历史/目录。仍有正常操作状态/refs 观察和
目标详情查询。仅关闭本次 Plico，清除三根测试配置，保留 fixture 与全部 Rebased 窗口。

### 定向配置失效与进行中查询核对

扩展桌面 fixture：分别在 History/Navigation 状态下使子根详情失效，确认不取消其 token、不改变
当前行；模拟成功发布调用 reconcile 后重新加载多根详情。与真实项目导航回归一起运行
4394-18e306c6320b016c-0：3/3 通过，185 过滤，61.048 秒，无重试。此新增断言是发布边界测试，
不是完整异步导航未命中/失败交错测试，不得扩张其证明范围。

源码检查发现仍有缺口：project_navigation.rs::navigate_project_commit 和
log_branches.rs 的导航完成回调，Found 会 select_commit；Hidden/OutsideRoots/错误只更新
navigation_notice/error。repository.rs 和 project_navigation.rs 的历史失败分支也未普遍 reconcile。
因此“History/Navigation 完成自然处理期间失效”的上一版注释只对成功路径成立；若配置变化发生在
失败/未命中的请求期间，现有选择可能保持旧正文，直到下一次主动操作。此路径不能标为已完成。
下一修正需要显式记录待处理的详情失效，在请求终止后补发，同时保留导航未命中/错误提示，不能
通过取消导航或吞掉错误来换取刷新。应补对应原生参考状态与失败交错测试，再修改产品生命周期。

### 阶段 4 批量读取源码进一步核对（尚未实施）

同一参考 commit 下继续读取 GitLogUtil.java::collectMetadata(List<String>,Consumer)：以
--no-walk 和 stdin 批量提交 hash，pretty/encoding 一次设置，GitLogOutputSplitter 流式分割并
报告解析错误；不是每个元数据 getter 一次 show。GitDetailsCollector.kt::readFullDetailsForHashes
同样通过 stdin 批量，DIFF_TO_PARENTS 时由 recordCollector 收集父比较，核对记录数与父数，
再构建 GitCommit；未达到父数量不能默默按普通提交处理。

本地 ui/log_selection.rs 仍对前 50 条逐个 commit_metadata；log_changes::load 在选择快照后
反序逐提交调用 log_commit_changes，浅边界前后复验仍必要。下一批量实现应保留按根分组、用户选择
顺序、选择不扩展成 revision range、输出/对象数量上限和 cancellation；元数据与逐父状态的优化
可分别验证，不能把降低 Git 次数当作允许改变 merge 语义。原生多选前置及同条件性能复测仍待执行。

### 延后详情失效的交付闭环（2026-10-29）

对照 Rebased GoToHashOrRefPopup、VcsLogNavigationUtil、AbstractVcsLogUi 的异步导航与独立通知，
并先完成原生不存在 hash 的操作/截图，详见 REBASED-REFERENCE.md。本地性能优化不能只缩小
查询范围，还必须保证失效交付：导航/历史尚未结束时不取消其请求，但明确保存一个待处理标记；
四条有效完成回调（单根/多根历史、单根/多根导航）发布结果后补发。成功选中新内容时消费待办，
切换项目清除待办，旧 generation 仍在原有 finish 检查处拒绝，不能触发补发。

实现位于 Review.pending_log_details_refresh、ui/log_selection.rs 的配置详情刷新/flush、
project_navigation.rs、log_branches.rs、repository.rs。只对受影响的当前选择置位；反复事件
合并成一次补发，没有无界任务队列。后台重新加载保留原 error/navigation_notice，包含配置在
导航完成后才到达的情况；主动选择仍按既有 reset_log_presentation 清提示。不增加 Git 查询
类型或缓存预算，不宣称降低此次导航查询耗时，作用是让收窄失效的设计在失败分支上也成立。

验证过程保留两次测试建模错误：363c-18e3076ae27cee08-0 为 1/2，错误地将未知仓库当作
OutsideRoots；5190-18e30785296f0d90-0 为 2/3，错误地将未知 hash 当作 Git 错误。本地 locate
对未知仓库返回错误、已知仓库中的未知 hash 可正常未命中。修正 fixture，使用 commit-tree
创建无引用的真实对象测试 OutsideRoots，使用未知仓库测试错误；不改变产品结果迁就断言。
d40-18e30796b80ef388-0 为 1/1；最终 5d30-18e307c864a6a7a4-0 为 3/3，185 过滤，46.830 秒。
覆盖真实项目导航完成回调、期间失效、详情重新加载、未命中提示/错误保留、根身份，以及历史
失败发布边界/成功发布边界、立即配置失效保留已有错误。历史失败仍为边界 seam，不冒称完整
Git 失败竞态；尚未运行最终全量套件。此次只补阶段 3 生命周期缺口，后续批量/增量阶段未完成。

最终原生构建 59AB5D0F1CB07A114DFEE3CC7203DD6CF310ABA2B12D89DCDE65312E5DEEA5C1，
同一 fixture 过滤后选择 1608a131，导航 main 得到隐藏提示，之后配置 renameLimit 1→1000，
文件四项自动变两项，原选择、正文与导航提示均保持。证据 navigation-invalidation-20261029/
plico-final-hidden.png、plico-final-config-updated.png；不声称该次先后操作命中了进行中竞态。
没有新增布局/入口；原生/测试/格式结果分别记录，全部审计阶段仍未完成。已停止自有测试进程，
恢复临时配置，保留 Rebased。

### 固定 Diff 失效前的端点审计（尚未实施）

继续阶段 3，先完成 Rebased 固定 Diff 的原生操作、截图与源码核对，详见 REBASED-REFERENCE。
发现需要修正先前“为 Commit Diff 加定向重读即可”的实现假设：Target.identity 保存 parents，
但普通 Target::read/full_patch 使用 git show，父关系在每次加载时重新解析；多选 combined
target 才按保存的左右端点读取。UI 的身份和实际查询输入不一致，因此直接加重读可能改变比较
对象，而不只是更新对象解释下的正文。

真实 Git 实验：原端点 base b5d38eb2 → ours 8c899826；将右端替换为内容 theirs、父也是内容
theirs 的提交。git show 原右端得到空 patch，而 git diff 原左端 原右端仍得到 base→theirs。
完整 OID、实际输出保存在 artifacts/fixed-diff-invalidation-20261029/endpoint-experiment.json，
不是模拟输出。Rebased 已打开页签此时仍为 base→ours；其 GitLogDiffHandler 的 producer 固定
左右 hash/path，但本次没有证明重开后采用何种缓存。本地应保持固定端点，并明确更新解释后的
正文，不能复制参考中的未自动更新表现。

后续实施边界：先统一普通/combined 的固定端点语义，覆盖 root/merge/rename/delete/binary/
shallow，避免为了统一接口将单个 Git patch 查询无条件扩大为多次进程；再令解释变动触发受影响
根的活动只读 Diff 重读。普通分支移动不重读固定内容；非活动页签依靠重新激活读取，不保留全文；
加载中失效必须重试或拒绝旧代数结果，不能继续直接 return。错误重试、连续替换、恢复替换、工作区
可编辑 Diff 的共享文档/锁都需要分别验证。本轮未改产品，尚未有新实现/测试全绿声明；已恢复临时
替换，所有用户窗口保持。该证据改变下一实现步骤：端点正确性先于 UI 刷新开关。

### 固定父端点读取实施与核心回归

git.rs::patch_context 的提交路径改为显式 git diff 保存的第一父ID 提交ID，不再让 git show
重新决定父关系；root 使用当前仓库格式的空树 ID，复用抽出的 empty_tree_id（hash-object
不带 -w），原 shallow 拒绝、输出限制、取消、metadata-only 正文补全和 gitlink 选项保持。
普通非根提交仍一次 patch 查询；root 从一次增加为空树查询+patch 两次，此为正确性代价，
尚未优化格式级复用，不将本项宣称为加速。尚未解决 rename 配置改变后的固定路径对语义，
也未接入 DiffTab 的 interpretation 失效，不能宣称阶段 3 完成。

新测试 captured_diff_preserves_parent_and_empty_base_after_object_replacement 通过真实 Target
入口，在 SHA-1/SHA-256 两种独立仓库中捕获普通/root target，再替换为自身父比较为空的对象。
断言保存的父仍产生 base→theirs，root 仍为空→theirs，移除替换后恢复原 patch；取消报错、
HEAD 与工作区保持。修改前 4c84-18e30890dff1a56c-0 为 0/1，失败输出确为空 patch，非测试环境
故障。修改后 1adc-18e3089dca6a17d0-0 为 10/10（128 过滤），覆盖新回归、完整上下文、本地
index/working 区分、Unicode/root、rename、merge 首父、shallow 和 submodule。
另外 5f9c-18e308a9e0a2a5ec-0 为 3/3（135 过滤）：mode-only、binary/oversize、stash 内容身份。
fmt/diff check 通过。此前 Rebased 原生前置仍适用；本次核心修改的 Plico 原生重读验证尚待完成，
没有新发布包或完整套件通过声明，未提交。下一步继续固定文件端点与页签失效的交付验证。
### 固定版本内容的批量读取（2026-10-29）

沿用固定 Diff 原生前置验证与 Rebased GitLogDiffHandler/GitContentRevision 的源码依据：
保留捕获的 hash/path，按需读取内容；本地采用一次 cat-file 批量读取两个端点，不复制上游缓存框架。
combined_changes::read_contents 原先每个存在的端点各启动一次 Git，现在通过
git.rs::read_with_input 执行一次 `git cat-file --batch -z`。普通 Target 的固定路径转换和
DiffTab 对解释变化的失效交付仍未完成，不把这项优化称为完整 Diff 修复或提交元数据批量化。

协议依据已读取本机 Git 官方 git-cat-file.adoc：NUL 分隔输入，成功输出按对象类型和字节长度解析；
不以正文换行分割对象。拒绝 missing、非 blob、截断、多余输出、二进制及非 UTF-8，保留不存在端点
与空文件的区别。输入和输出均有 8 MiB 上限；输出上限包含协议头，比原先纯内容总量边界略严格。
沿用 30 秒超时和整个自有进程树终止；stdin writer 与两个输出 reader 均 join 后才完成请求，
避免大输入、提前退出或取消留下阻塞线程。没有启动外部 filter/textconv，也没有跨请求常驻 Git 进程。
`-z` 用于 NUL 输入兼容，未声明其他 Git 版本已验收；Unix 换行文件名测试在 Windows 未执行。

定向结果：388c-18e3090c2e687790-0 为 13/13；补充 stdin 阻塞取消、提前退出、输入上限、
目录类型拒绝及 SHA-1/SHA-256 替换端点后，a8-18e30921abbeec50-0 为 4/4（499 过滤）。
这些不是全量通过。fmt/diff check 通过，Plico 最新原生构建尚未包含此增量。

release performance_audit --revision-content，在原 operation-context 临时仓库无诊断运行，
snapshot 的最新 all-refs 提交为 1608a1318a1acab52e5701a39f6eb711bf148810（config audit rename）。
同一文件端点做 20 组交替读取并逐组比较内容：两次单端点调用中位数 82.412 ms，
一次双端点调用 41.012 ms；范围分别 80.916–104.076 ms、39.750–46.540 ms。
这是当前同一 reader 的两进程与一进程控制实验，非旧版二进制或 Rebased 的端到端 UI 对比，
也不代表大文件、冷缓存及所有操作有相同比例收益。采样时没有构建/测试并行运行。
原始数据与汇总：artifacts/fixed-diff-invalidation-20261029/blob-batch-untraced.jsonl、
blob-batch-summary.json。支持的设计结论是合并独立的小对象查询能直接减少 Windows 子进程开销；
下一步仍需检查多选元数据 N+1、失效扇出和 CPU/内存，不能仅靠这一处替换结束审计。

### 多选批量化的实现边界复核（未实施）

再次逐项核对 Rebased 同一参考 commit 的 GitLogUtil::collectMetadata：空输入直接返回，
一次 log --no-walk --stdin，统一 UTF-8 pretty 格式，流式 consumer，最后报告命令及解析错误。
GitDetailsCollector::readFullDetailsFromHandler 的 DIFF_TO_PARENTS 分支核对实际记录数与父数；
不能将缺失父记录当成空差异。Lazygit 参考 4e1eebe278790f257a875da6633234391a5a85b3 的
pkg/commands/git_commands/commit.go::GetCommitMessagesFirstLine/GetCommitsOneline 同样把
多个 hash 交给一次 show；commit_loader.go::getLogCmd 在一次 log 中读取展示字段，并支持 -300 限制。
这些仅为源码对照，没有新增 Lazygit 原生实测，也不能照搬其摘要字段或数量限制。

Plico ui/log_selection.rs 的多选 presentation 循环在 selection::load 后最多调用 50 次
commit_metadata；log_changes::load 另有逐提交/逐父文件状态查询。这是两个不同的批量化点。
建议先按 root 分组元数据，使用完整 OID 与 --no-walk，校验返回集合、重复和缺失记录，再按原选择
顺序组装卡片；不得把离散选择改成 revision range，也不得以 Git 输出时间顺序替代显示顺序。
保留当前 UTF-8 转码、完整 message、8 MiB 总展示预算、shallow base 标记和 generation 接纳规则。
流式或有界解析还需处理正文分隔符冲突，不能简单按换行切块。逐父文件状态另立验收，保留 merge
父关系和 shallow 前后复验。实施前仍需补该多选场景的原生对照，不因已有 stdin 基础就直接改 UI 链。
### 本批冻结后的完整核心回归

隔离入口 `cargo run --locked --manifest-path tools/test-runner/Cargo.toml -- core`：
运行 2b90-18e309762798f3d0-0，973/973 通过，0 skipped，1154.317 秒，无自动重试。
覆盖本批累计核心变更，替代早期 959 项基线作为当前核心结果。fmt/diff check 通过。
`cargo test --locked --no-default-features --doc` 通过（当前 0 个文档测试）。
这不等于完整桌面或跨设备验收；最新完整 desktop/navigation、Plico 固定 Diff 原生验收、
其余审计阶段仍待完成。原始结果保存在 artifacts/test-runs/2b90-18e309762798f3d0-0。

### 固定父端点原生闭环与下一项约束

release 14B51B6123B920D0EC0118B267EF3A82376E9B1A23E7EA20C73B317A4B1ABC2A 的原生同仓库
验证完成：固定 b5d38eb2→8c899826 的旧页签，在对象替换改变父关系后重新激活，仍正确显示
base→theirs，而不是替换对象相对新父的空差异；删除替换并重新激活恢复 base→ours。
步骤与截图见 REBASED-REFERENCE.md“固定父端点的 Plico 原生验收”。release 构建通过，
保留既有 sequencer dead-code 警告。本轮没有修改产品代码，因此不重复跑上一轮 973 项核心套件。

同一次实验确认活动 Commit Diff 不自动重读；Log 已完成解释失效，Diff 消费端仍落后。
下一步不能只删除 Area::Commit 的排除条件：普通 Target 仍通过树 diff 重新识别路径配对，
而 read_contents 仅用于 combined/working comparison。直接统一成 from_sources 会丢失
模式变化、binary/gitlink 信息。应先明确固定端点内容与文件元数据的共同结果模型，再接入失效。
事件需区分对象解释变化与普通 refs 移动，后者不应重新加载固定 OID 内容；加载中事件必须记录并
在接纳旧结果前处理，非活动页签保持释放内容、激活重读。现有 disk_pending 的合成 all/index 批次
也不足以保存解释失效原因。以上仍为待实施项，不能以父端点原生通过代替完整 Diff 生命周期验收。

### 阶段 4：提交说明批量查询落地（2026-10-30）

原生前置和源码规则见 REBASED-REFERENCE.md“多选提交说明批量读取前置验证”。新增
git_metadata.rs::commit_metadata_batch：最多 50 个完整 OID，经 stdin 一次 log --no-walk=unsorted；
显式 UTF-8/no-signature/no-patch，NUL 字段协议保留换行、制表符和控制分隔符正文。
以返回 OID 对齐调用顺序；拒绝重复/缺失/意外记录、超限及取消，不按 revision range 扩展选择。
显式 %n 保留旧 show --format 的末尾换行，大小写 OID 归一化；base 继续使用捕获的父/shallow。
至多解析 301 个字段，输出沿用 8 MiB 限制，避免异常 NUL 内容制造无界字段数组。
正文中嵌入 NUL 会作为协议歧义报错，不尝试展示部分结果。批量输出预算包含协议字段，较旧逐条
读取加 UI 展示预算更保守；这是有界预览的限制，不宣称任意大小提交均可展示。

project_log::selection::metadata 按 root 分组并恢复原行顺序，跨根累计元数据也限制 8 MiB；
ui/log_selection.rs 用该结果替代最多 50 次 commit_metadata，保留现有 presentation 总预算与
generation 接纳规则。单选缓存路径、逐父文件差异、选择合并算法、branch containment 不改。
equal OID 的不同根保留独立 Git 解释，不新增跨根缓存或常驻子进程。通用性能 example 的多选
测量入口同步到当前实现；旧文档数值仍是基线，不是现构建成绩。

核心定向 57e8-18e30b65e4d98748-0：2/2（138 过滤），真实 Git 核对完整正文、作者/时间、
base、顺序、空输入、重复、缺失、计数上限、取消，以及两个根相同 OID 的不同 replace 正文。
桌面 397c-18e30b4c4895243c-0：1/1（187 过滤），实际多根选择异步链和详情/预览所有权通过。
该桌面结果在最后增加 OID 大小写与解析后取消检查之前；最后核心定向已覆盖大小写。
fmt/diff check 通过。此前 973/973 发生在本增量前，不能作为此增量全量通过的声明。

无 Trace2/性能诊断、无并行构建测试，对原 small（72 提交/200 文件）fixture，release example
--metadata 分别 1/20/50 条，各 5 组交替单条/批量查询并逐项核对结果。中位数：1 条 40.241→41.648 ms，
20 条 983.213→46.805 ms，50 条 2394.263→47.230 ms。原始数据与范围在
artifacts/metadata-batch-20261029/untraced.jsonl、summary.json（沿用跨午夜实验目录名）。
这是同版本核心说明读取对照，不是 Rebased UI 或整个多选完成时间；仍有逐提交/逐父状态 N+1。
当前没有新的 CPU/峰值内存或冷缓存结论，阶段 4 尚未整体完成。

最终 release 04411F424A384F86788C169635B53753E3D5A997CFB90576DABA1865A80ADA36 原生双提交
选择已通过：同仓库 ours + theirs，合并文件 a.txt 与说明卡片顺序保持。截图、布局和加载期差异
见 REBASED-REFERENCE.md“多选提交说明批量读取实施后验收”。构建通过，仅既有 sequencer 警告。
这完成了本次 metadata 批量链的实现/定向测试/计时/原生闭环；没有宣布剩余审计或全量桌面完成。

### 阶段 4：逐父文件状态批量查询（2026-10-30）

继续读取 Rebased GitLogRecordCollector.java：log -m 可能省略某个空父结果，上游用另一条批量
tree 查询匹配并补空记录；不能简单用输出记录数猜父顺序。原生两个 merge 场景与源码映射见
REBASED-REFERENCE.md“逐父批量查询前置验证”。Git 官方 diff-tree.adoc 确认 stdin 支持
显式 commit/parent 列表；本机命令实验确认 --always 与 -z 为每个显式父比较保留空记录头。

git_log_changes.rs::log_commit_changes_batch 用最多 32 个显式父比较组成一个进程，root 使用
--root，每条返回头按输入 OID 核验。按状态字段消费一/两个路径，十六进制文件名不能被误当头；
拒绝缺失、多余、截断和非法状态。按原父序合并，跨批次保留尚未完成 merge 的组，完整提交合并后
立即释放原父记录，仍用原 merge_parents/from_file。log_changes::load 保留逆显示顺序组合、
离散选择、浅边界前后复验、跨根隔离与 zip_changes。原逐条 API 保留用于其它入口和结果对照。
非空选择最多 HISTORY_MAX，父比较总数新设 100000 上限；每提交原始状态数及全部合并状态数仍
受 MAX_INPUT_CHANGES 约束。每次读 8 MiB；仅遇该输出上限错误且批次大于一条时二分重读，
维持顺序与单条读取能力，不吞掉取消、超时或 Git 错误，不创建并行查询或常驻 Git。

1200-18e30cf23442424c-0：14/14（499 过滤），包括空父、root、重复 OID 文件名、换行路径协议、
merge 跨 32 条边界、选择顺序、跨根身份与 shallow 拒绝。44a8-18e30d0a1be5e7a4-0：2/2
（140 过滤），补真实 4000 长路径树、12 个比较超过 8 MiB 后拆分，与逐条结果逐项相等。
5e90-18e30d10fdd89cf0-0 桌面多根异步选择链 1/1（187 过滤）。fmt/diff check 通过。
这些是定向验证，尚未更新完整 core/desktop/navigation；SHA-256、octopus 和实际 Unix 路径
本增量尚无专项运行结论，不能扩张已有测试范围。

同 small fixture、无诊断/并行构建测试、release --parent-changes，每种规模 5 组逐条/批量
交替读取并核对完整结果。中位数：1 条 41.684→45.795 ms，20 条 978.766→47.168 ms，
50 条 2396.964→97.579 ms；50 条走两个批次，符合分批设计。原始数据及范围在
artifacts/parent-batch-20261030/untraced.jsonl、summary.json。这是逐父状态部分的核心控制
实验，不是 UI 整体耗时或 Rebased 性能比较；不能直接相加不同实验中位数作端到端结论。
Plico 最新原生同条件 merge 验收已完成，release SHA256 为
0BCDB6D6BCB37971664C0F42EE30A039A889CC93BAEA1C449C8488C2FC4B7A7D。
两条 merge 多选后保持一个 a.txt、两张按选择顺序排列的说明卡片；双击可打开只读 Diff。
截图与步骤见 REBASED-REFERENCE.md“逐父批量查询实施后验收”。单选仍沿用第一父差异规则。

额外发现既有内容模型差异：同一 merge 文件 Rebased 展示父一/结果/父二三栏，Plico 为父一/结果
两栏。上游 VcsLogAsyncChangesTreeModel 的 MergedChange 分支及 MergedChangeDiffRequestProvider
明确保留两个 sourceChanges；本地 merge_parents/from_file 当前将目标压为两个端点。
本次批量化与原逐条结果一致，但不能据此认定 merge Diff 语义完整。后续固定 Diff 内容模型应保留
父来源，再处理三栏呈现与生命周期；本增量不追加未经完整设计的三栏界面。阶段 4 与整体审计仍未完成。

### 阶段 5 补充源码核对：增量替换边界（2026-10-30，尚未实现）

冻结上述代码进行提交前全量检查期间，重新读取参考 245042348669114adb8e24e5b0fc0ce73763f3c4 的
plugins/git4idea/backend/src/status/GitStagingAreaHolder.java、GitChangeProvider.java，及
repo/GitUntrackedFilesHolder.kt、index/GitIndexStatusUtil.kt。这里仅补设计依据，不替代新增行为
所需的原生前置验证，也不把源码中的刷新策略宣称为本机测量结果。

StagingAreaHolder 查询 tracked 状态时关闭 untracked/ignored，并将已有 A/D/R 候选和 rename 两端
加入 dirty scope；替换记录时 path 或 origPath 命中范围均需删除旧记录。ChangeProvider 对范围边界
跨越的 rename 再标记两端，同时在子模块 dirty 状态变化时补父仓库检查。UntrackedFilesHolder
独立维护 Everything/Files 范围，成功结果仅替换覆盖的子树，保留其它路径；更新中收到的新 dirt
继续保留。其 refreshFiles 捕获 VcsException 后返回空集合，这是上游行为，但本地不应照搬为
失败时删除旧列表：现有 ChangesRefresh 的 Failed/Loaded 区分应保留，失败范围须可重试。

本地实际调用链是 Batch → Review::sync_repository_worktree → LocalView::refresh →
ChangesRefresh::request → Snapshot::load，目前在 UI 路由处丢失 paths，flight 只保留 pending bool。
Snapshot 的 review_versions 又承担 staged/unstaged 和 rename 身份，collect 对 MM 使用 HEAD 的
净差异过滤，故不能直接将局部 status 结果当成完整 Snapshot 交付。

下一步设计需由 ChangesRefresh 同时拥有本次范围、期间累计范围和最后成功状态；首次、未知/溢出、
HEAD/index/config/ignore 语义变化及写操作后采用明确的全量路径，其它路径按原记录扩展 rename 范围，
再原子合并成功结果。外部 exclude、目录删除、嵌套根、子模块和结果到达前的新事件都必须有明确
处理及回归，不能仅加 status pathspec 或开启 Git 配置开关。上述为待验证方案，不是已落地承诺。

### 本批提交前完整验证（2026-10-30）

代码冻结后，以 tools/test-runner 的 all 入口在 Windows 执行，结果目录
artifacts/test-runs/3670-18e3259d35f30d10-0，三个 status 文件退出码均为 0：

- core：978/978，0 跳过，1067.158 秒。
- desktop：188/188，0 跳过，792.826 秒。
- navigation：10/10，入口按配置过滤另外 178 项，7.910 秒；使用 test profile 的
  debug-assertions=false，并非完整 release 构建。完整桌面套件仍开启调试断言。

cargo test --locked --no-default-features --doc 成功（当前 0 用例）；
cargo clippy --locked --no-default-features --all-targets 成功，仍有非阻断警告，包含本批新增的
参数数量、chunks_exact 和测试中临时 vec 建议，不宣称警告清零，也未添加全局 allow。
cargo fmt --package plico -- --check 与 git diff --check 通过。
原生验证范围及截图见前述各增量记录；CI 尚未运行，本次未推送。
本结果覆盖本批已实现代码，不代表上述剩余设计、三栏 Diff 或整体性能审计已经完成。

### 提交后的阶段 5 命令实验（2026-10-30）

上述实现已提交为 130da87；继续在独立临时仓库和本地 bare remote 上验证查询范围，未操作用户仓库。
Git 2.56.0.windows.3，禁用 system config、独立空 global config/模板，显式本地作者与 autocrlf=false。
记录及 fixture 路径：artifacts/dirty-scope-20261030/probe.json。已提交 old.txt 后执行 git mv 到
new.txt，status --porcelain=v1 --untracked-files=all --renames 的实际结果：

| 查询范围 | 结果 |
| --- | --- |
| 全仓库 | R old.txt → new.txt |
| 仅 old.txt | D old.txt |
| 仅 new.txt | A new.txt |
| 同时 old.txt/new.txt | R old.txt → new.txt |

另创建规则 untracked-dir/ 的 .gitignore 及目录内文件。带 --ignored=matching 的全仓库查询包含
ignored 目录，限制为 .gitignore 时仅返回规则文件本身，不返回受规则影响的目录。
这直接验证了范围不仅影响性能，也影响记录语义；普通事件路径不能直接用作增量替换的完整范围。
rename 两端扩展、ignore 规则依赖失效和全量回退仍是实施前提。本实验只覆盖 ASCII 路径的命令行为，
不是原生 Rebased/Plico UI 验收，也不是性能计时；尚未开始阶段 5 功能代码修改。

### 阶段 5 核心范围与合并实现（2026-10-30，UI 尚未接入）

原生前置已补齐，见 REBASED-REFERENCE.md“工作树增量刷新原生前置”。新增 changes_scope::Scope：
包含全量/有界路径范围、目录覆盖折叠和范围并集，最多 512 个范围、6000 单位参数预算；超限不拆分
rename 候选，而是回退一个全量查询。未知/基线/成员事件、规则文件、非法相对路径也回退全量。
局部范围补上原 review_versions 中的 rename 两端与 A/D/R/C 候选，独立于 Commit 勾选范围。

Repository::local_changes_in 使用 literal pathspec、NUL 状态协议及原有 untracked/submodule
选项。Snapshot::refresh 保留范围外原记录，仅替换本次覆盖的 path/origPath，使用同一个状态排序
与 collect/MM 净差异规则；前后 HEAD 复验保留，HEAD 已改变则全量重建，失败不修改传入快照。
累计记录超过 100000 时重新全量读取，避免部分更新无限累加旧记录。

定向隔离运行 2470-18e3280d73e0b308-0：6/6（974 过滤），4.243 秒。真实 Git 测试将局部结果与
全量 Snapshot 的 head、每条 review_versions 和展示 files 逐项比较，覆盖已暂存重命名后再编辑、
字面量方括号路径、范围外修改保留、MM 被工作区反向恢复、未跟踪目录创建/删除、规则变化、恢复干净、
HEAD 前移、预取消和查询失败；检查 index 未被读操作修改。另验证范围折叠/边界与四个旧 collector
回归。最后增加达到全量回退后停止枚举的短路；最新版本在
6160-18e3282f1240c01c-0 再跑范围及真实增量对照，2/2（978 过滤），2.781 秒通过。fmt/diff check 通过。

尚未接入 ChangesRefresh/LocalView，所以本次不能宣称界面提速或增量刷新已经交付。后续必须将
成功快照绑定具体仓库，限制额外保留内存，合并在途/失败范围；在接入前补 ignore/exclude 外部依赖、
周期全量校验及大小写/子模块边界，避免将当前全量扫描偶然覆盖的变化变成永久漏报。
本次仅有核心定向结果，没有本增量全量或 Plico 原生验收结论。

### 阶段 5 刷新任务所有权（2026-10-30，文件事件尚未接通）

ChangesRefresh 现持有本仓库的最后成功 Snapshot、本轮 Scope 和期间累计 Scope。
request 仍代表显式全量刷新；request_scope 接受有界失效范围。成功后发布共享 Arc<Snapshot>，
LocalView 的勾选/列表按原流程从事件消费，不增加一份缓存克隆。失败保留旧快照，并把本轮范围
并回 pending；只有本轮期间收到新失效才立即继续，否则等待下次请求，避免错误重试热循环。
新的 LocalView 创建新的 ChangesRefresh，repository 在该刷新器生命周期内不替换，不跨根共享缓存。

额外快照缓存按拥有的 Vec/路径/String capacity 加结构体与 Arc 控制块估算，预算 2 MiB；超额结果
仍交付列表，但不保留为下次增量基线。此数字不是 RSS，不含 allocator 元数据、UI 列表副本、Git
子进程或 GPU；查询期间可能同时持有前后快照和有界 Git 输出。首次、无缓存、ignored 展示开启、
距上次成功全量至少 30 秒时，下一次请求转为全量。没有新增常驻定时查询；该机制不等同于已经
解决外部 ignore 文件无事件时的主动观察，文件事件接通前仍需补相应依赖。

桌面定向 30d8-18e32859c9c50d68-0：3/3（186 过滤），0.164 秒。覆盖在途范围合并、失败后与
新范围并集重试、没有新事件时不循环、成功快照 Arc 复用、超额快照交付但不缓存、过期全量重建，
以及既有取消、延迟 loading 反馈与 LocalView 树几何保留。这里调度边界使用受控 loader，真实 Git
语义由上一节核心对照覆盖；尚未完成生产事件到局部读取的端到端运行。
目前所有既有 LocalView 调用仍走全量 request，未启用局部事件入口；无本增量 UI 提速或原生验收声明。

### 阶段 5 忽略规则依赖（2026-10-30）

补充外部 core.excludesFile 和 common Git directory 的 info/exclude 内容观察；源码/原生前置
见 REBASED-REFERENCE 的同日外部忽略文件记录。单独 status_rules 失效只触发 Changes 等工作树
消费者，不触发历史、分支比较、提交详情缓存失效。通知直到 UI 接纳才确认；读取不代表交付。
linked worktree 的共享 info/exclude 事件路由到该根的合成 .git 地址，主工作树 index 仍不路由。

沿用配置轮询的 30 秒周期与逐根公平调度，不新建定时器或每次文件编辑解析配置。每次 eligible
root 仍读一次有效配置；首次、配置变化或上次解析失败时额外调用一次 config --null --path
和一次 common-directory 查询。稳定状态复用解析路径，只读最多两个规则文件，每文件 1 MiB，
8 KiB 缓冲流式摘要，不缓存正文。不能继续把每次 poll_configuration 称为严格只有一个 Git 进程。
规则缺失可作为稳定状态，读取失败、非普通文件、超限保守使 Changes 失效；同长度修改可识别。
取消不推进摘要或配置检查期限。Git 负责路径展开，支持显式空值、相对路径、波浪号及默认路径。
默认 XDG/HOME 分支仍依赖进程环境；未在测试线程中修改全局环境。

定向核心 4f2c-18e3290275448d00-0 为 15/15（967 过滤），新增交付边界后
32c8-18e32910fcbbdde8-0 为 4/4（979 过滤）；桌面 950-18e32914aa04f3d8-0 为
4/4（185 过滤）。未重跑全量，未启用普通路径的生产局部查询；Windows 大小写、子模块路径范围
及端到端提速仍待下一步验证，不以这些定向结果宣称阶段 5 全部完成。

真实 OS 事件集成 2d44-18e3292854dc71b0-0：5/5、0 过滤，包含 linked worktree 共享
exclude 通知。release 构建通过（既有 sequencer 未使用 variant 警告）；原生 Plico 同仓库
自动发现外部规则与 info/exclude 内容变化，列表 5→6→7，保留已打开 Diff，截图与操作记录见
REBASED-REFERENCE。fmt、diff check 和核心 all-targets clippy 通过，clippy 仍有既有警告。

### 阶段 5 大小写与子模块查询边界（2026-10-30）

独立实验 artifacts/dirty-scope-20261030/boundaries.json 证实：Windows 工作树通过小写
casefile.txt 写入后，全量 status 返回 CaseFile.txt；literal 小写 pathspec 返回空。
子模块内部文件改变时，全量父根 status 返回 child，而 child/CaseFile.txt 范围返回空。
这两种空结果都不是干净状态。Rebased 实机与 RootDirtySet/GitStagingAreaHolder 对照见参考文档。

核心局部查询现对 Windows ASCII 路径使用逐路径 :(icase,literal)，快照范围剔除、纳入和目录合并
使用相同 ASCII 忽略大小写匹配，保留分量边界。不能把全局 --literal-pathspecs 与 --icase-pathspecs
一起传给 Git（实测报不兼容）；本地显式关闭全局 literal 后使用逐路径 magic，括号/星号仍为字面量。
非 Windows 保持原字面量和大小写敏感语义；Windows 非 ASCII 失效路径暂全量回退，避免自造 Unicode
大小写规则造成漏项。命令长度预算计入 magic 前缀。配置规则文件大小写别名也必须全量回退。

定向 4904-18e329ca2f325504-0：4/4（981 过滤），包含真实 Git clean→dirty→clean→dirty、
大小写别名、括号字面量、原重命名/删除/MM 语义。fmt 与 diff check 通过。大小写分支仍未接入
生产 UI，因此没有本次 Plico 原生后置或端到端性能通过声明。
子模块下一步应复用 project_repositories::SubmoduleRelation 的 parent/path 映射，传播子仓库
失效并扩大父根范围到 gitlink；不能只依据磁盘 .git 或任意路径祖先猜测已跟踪关系。

### 阶段 5 生产事件入口接通（2026-10-30，验收进行中）

Scope::for_event 使用现有 project_repositories catalog：父根内 child/file 范围扩展为已声明
gitlink child；子根独立通知沿 SubmoduleRelation 向祖先传递，父根只查询对应 gitlink。
无关的注册根返回空范围；catalog 缺失、出错或关系超出 4096 上限时全量回退。关系遍历维护 visited，
不根据任意磁盘 .git 猜测关系，不增加每事件 Git 发现查询。Review 在事件触发成员重新发现前捕获
有效 catalog，将范围交给 LocalView::refresh_scope / ChangesRefresh；显式刷新仍保持全量。
这次仅改变 Changes 查询失效范围，未扩大既有父子 Diff Tab 通知行为。

真实 Git 回归 2df4-18e329fcac1c4e58-0：5/5（981 过滤），包含父/子两种事件地址均形成 child
范围，修改和恢复后的快照与全量结果相同，无关根不触发查询，未知成员回退。桌面定向
5a70-18e32a006efb2734-0：8/8（181 过滤）。新增 performance_audit --changes-scope 模式以交错
顺序比较 Snapshot 全量/局部读取，预热一次后五组，保留 HEAD 复验并校验结果；它不测 UI 或 RSS。

原生生产链已验证：同 Rebased 边界 fixture，父 Changes 自动出现/消失 child gitlink，大小写别名
写入自动显示 CaseFile.txt；Git trace2 确认有范围 status 真正执行，详见 REBASED-REFERENCE。
这不覆盖本地原有未聚合子仓库 Commit 列表、父子 Diff Tab 通知或刷新中双击的全部行为。

单独万文件基准：100 个目录、10,000 个已跟踪小文本、一个修改，Windows 当前机器，release
读取 Snapshot（含 Git 启动和 HEAD 复验），预热后交错五组。全量中位数 170.3914 ms，局部
136.4897 ms，减少约 19.9%；两组结果完全相同。fixture 与原始结果分别在
artifacts/dirty-scope-20261030/benchmark-fixture.json、benchmark.jsonl。
这不是冷缓存/网络仓库/UI 响应时间测量，没有采集 RSS、GPU 或 CPU；进程启动仍占主要固定成本，
不能由此宣称全部操作提速 20%。没有在构建或测试并行运行时计时。

### 阶段 5 提交前完整回归（2026-10-30）

受测代码冻结指纹见 artifacts/dirty-scope-20261030/frozen-worktree.json，结束后复核全部一致。
隔离入口 all 的运行 60c4-18e32a4d1ebae2d0-0 退出 0：core 986/986、0 跳过，1204.825 秒；
desktop 189/189、0 跳过，850.193 秒；navigation 10/10、按入口过滤 179，8.058 秒。
navigation 是关闭 debug assertions 的 test profile，不能称为完整 release 构建测试。
fmt check、diff check、doc test（0 示例）与 no-default-features all-targets clippy 均通过；
clippy 保留已有警告，日志 artifacts/dirty-scope-20261030/clippy-final.log。没有本次 CI 通过声明。

本阶段交付有界路径查询、成功快照所有权、失败范围保留、重命名候选扩展、忽略规则依赖、Windows
ASCII 别名及已声明子模块映射。非 ASCII Windows 范围、未知/错误成员、超额快照、ignored 显示等
保留全量回退。源码依据、原生结果和查询计时分别记录，未以定向测试代替上述完整回归。
整轮审计仍有统一刷新路由、不可变详情缓存的精确失效，以及剩余进程/计算热点评估待完成。
## 阶段 5 提交后的刷新调度复查（2026-10-30）

阶段 5 实现提交为 `13688b8`。以下是后续只读源码审计，不是已实施的优化或新的耗时测量。

确认的放大链路：`ui/auto_refresh.rs::start_auto_refresh` 发现 References 改变后，
进入 `ui/repository.rs::refresh_impl(false, true, ...)`。同一入口也用于 Push 完成
（`ui/remote.rs`）、Pull/项目 Update 完成、需要刷新时的 Fetch，以及 shallow/config 通知。
它无条件刷新项目根、全部分支比较、历史计数、清空全部提交详情缓存，并刷新 Changes。
`ProjectRoots::refresh` 确实启动 mappings 发现和逐根 shallow 查询，同时发出 Checking；
因此这不是几个廉价 UI notify，而是具有任务、查询和下游状态成本的放大。
已有 discovering/pending 合并防止反复取消，但不能消除不必要的发现任务。

对照 Rebased `245042348669114adb8e24e5b0fc0ce73763f3c4`，本轮重新读取
`platform/vcs-log/impl/src/com/intellij/vcs/log/data/VcsLogData.kt` 的持有者初始化与
refresh：commitDetailsGetter 独立于 refresher，refresh(roots, optimized) 转交按根刷新。
结合 REBASED-REFERENCE.md“详情缓存与引用失效前置”的原生记录，不能将刷新 Log 翻译为
清空全部详情。源码结构也不能证明上游完整处理了 replace/grafts，保留先前限制。

下一批修改的范围限定为“引用观察到消费方的刷新决策”，不引入通用事件总线或提高并发：

| 已确认的变化 | 拟刷新范围 | 实施前必须守住的边界 |
| --- | --- | --- |
| 普通分支/标签/远端引用 | 相关历史、计数、标签与符号分支比较 | 不推导为工作树或项目成员已变化；配置/解释另行失效 |
| HEAD/index/worktree | Changes 基线及相应比较 | 写操作失败或取消仍可能有部分结果，不能只看成功码 |
| 成员或仓库身份 | 项目发现及对应根消费者 | 同路径仓库替换不能复用旧身份缓存 |
| replace/shallow/配置解释 | 对应根详情及依赖解释的消费者 | 补查 grafts、已加载选择与迟到结果，不能只清 Cache |
| 手动刷新/未知变化 | 保守刷新 | 保留用户主动修复未知状态的入口 |

Push 不能仅按操作名直接跳过所有本地观察：hook、并行外部修改及部分完成都须由实际观察
决定后续刷新。Fetch 已有 unchanged/stamp 判断，拆分时应保留，而非重新无条件刷新。
普通引用保留详情缓存要与 root identity/grafts 的审计一起完成，不能单独删 clear()。

另一个独立缺口：`diff_tab.rs::disk_changed` 在 Commit/Stash、loading、无 viewer 或文档锁定时
提前返回；disk_pending 只覆盖已进入 disk_refreshing 的通知。固定端点解释失效不能直接复用
该入口并声称完成；其 read_contents 分支从文本构造 DiffFile，还需保留二进制、mode、gitlink
等语义。下一批刷新决策不顺带重写整个 Diff 生命周期，此项独立跟踪。

验收将用临时仓库和本地 bare remote，对普通引用变化、Push up-to-date/部分失败、HEAD 移动、
配置与 replace 分别记录实际 Git 命令及消费方启动次数；用原生同条件截图确认选择、已打开 Diff
和失败恢复。当前仅建立调用链与验收边界，未测得这批改动的收益，未实施上述策略。

### Grafts 前置缺口的实证与修复进行中

全新两提交仓库实测：refs 不变时 info/grafts 仍改变父关系，删除后恢复。
证据 artifacts/refresh-dependencies-20261030/probe.ps1/result.json；原生 Rebased 的
同条件操作和三张截图见 REBASED-REFERENCE.md“Grafts 与刷新依赖前置”。
已修改 Pending 的 grafts/父目录删除与重命名识别、linked worktree 的 common 路径路由，
并让解释通知直接触发历史重载，避免只等引用轮询。尚未放宽详情缓存清理策略；
仓库身份替换、有界轮询兜底、固定 Diff 解释失效仍未完成。

定向验证：b0c-18e32cdc1f73ddf8-0 单元 16/16（971 filtered）；
6178-18e32ce7a48adeb4-0 真实观察器 5/5；56fc-18e32ce8bb2f052c-0 桌面仓库详情隔离
1/1（188 filtered）。格式与 diff 检查通过。发布构建与 Plico 同条件原生复验待完成，
当前结果不代表此工作树已通过全量测试或此批已可提交。

后续发布构建通过（保留 sequencer.rs 的既有 dead_code 警告），Plico 原生创建/删除 grafts
均自动更新历史父关系及当前详情、保持选择；截图与构建身份见参考文档的新复验小节。
本批功能代码冻结，开始完整核心回归；完成前不标为提交就绪。

提交前结果：`2f78-18e32d2c718fa554-0` 完整 core **987/987、0 skipped**，
1140.702 秒，core-status.txt 为 exit code 0。随后
`cargo test --locked --no-default-features --doc` 与
`cargo clippy --locked --no-default-features --all-targets` 均成功退出；clippy 保留已有警告，
不声称 warning-free。冻结文件摘要复核一致，格式和 diff 检查通过。
本批桌面范围为上述 1 项仓库详情隔离定向回归及 Windows 原生 grafts 创建/删除复验；
没有重跑完整 desktop/navigation，也没有推送或取得新的 CI 结果。

### 引用轮询的刷新消费拆分（进行中）

grafts 修复已提交 `93271c0`。随后按参考文档“普通引用刷新范围”的原生前置记录，
将引用轮询的结果接纳集中到 ui/auto_refresh.rs::apply_observed_references。
原有取消、操作锁、文档锁和查询 stamp 门禁保留在调用处；只有前后 HEAD、当前分支、
replace 相同且处于 Log 模式，才调用 repository.rs::refresh_reference_changes。
该入口按当前根通知分支比较、刷新历史/计数，保守失效当前根详情缓存，省去该轮询额外
发起的项目发现和 Changes 请求。手动刷新及未知/checkout/解释变化仍用完整入口。

这不是整个刷新重构完成：文件观察器仍会保守响应 refs，Push/Fetch 完成的刷新还未拆分，
详情缓存也未改为跨 refs 长期复用。本批只处理有前后快照依据的重复查询来源。
首次定向测试因测试模块缺少 PathBuf 导入编译失败（12c4-18e32ea4539caa14-0），已修正；
51c8-18e32eb0d45d6b64-0 桌面回归 1/1、189 filtered：真实 Git 新分支保持成员代数，
同 HEAD 切换分支增加发现代数，普通引用仍启动历史查询。发布构建与 Plico 原生复验待完成，
尚未测量本批查询减少或延迟收益，未提交。

原生后验已完成：parent/child 的分支创建/删除均更新标签和归属，保留选择、文件详情与 Changes。
首次 Trace2 与子模块周期性 fallback 重叠，保留为混合样本，不作减少扫描的证明。
单根隔离样本 12 秒内只有 1 次 status，没有 show-toplevel/.gitmodules 项目发现查询；
历史读取正常发生。证据及剩余差异见参考文档“普通引用刷新实施后的原生验收”。
本批没有旧版本同条件耗时对照，不推导百分比提速。功能代码冻结，完整 desktop/navigation
回归待完成；核心代码未变，沿用上一提交 core 987/987 的证据，不冒充本批重新跑过 core。

提交前结果：63a4-18e32f358c1a5e7c-0 完整 desktop **190/190、0 skipped**，860.146 秒；
42b0-18e32ffe482a97f8-0 navigation **10/10、180 filtered**，8.002 秒。
navigation 使用隔离入口规定的 test profile / debug-assertions=false，并非完整 release profile。
两个 status 文件均为 exit code 0；随后 clippy 成功退出，日志为
artifacts/reference-refresh-20261030/clippy.log，既有警告保留。冻结摘要复核、fmt 与 diff 检查通过。
本批核心源文件未改动，完整核心依据仍是 93271c0 前的 987/987；没有新 CI 或推送记录。

### 空闲后台查询与监视器绑定审计（2026-10-30）

本节仅分析和记录，不改变运行行为。基线为已提交的 `0686b81`；复用上一节同构建的
parent/child 原生 Trace2。取 UTC 02:50:20（含）至 02:52:20（不含）的 120 秒，
位于测试分支创建之后、删除之前；期间未执行新的 Git 写操作，可能包含截图和只读观察。
只统计 event=start 且 sid 不含 `/` 的顶层 Git，排除 Git 自己的子进程。
全部 416 个 start 均能匹配 exit，退出码均为 0。

| 互斥分类（按 argv 特征） | 次数 | Trace2 进程内耗时之和（秒） |
| --- | ---: | ---: |
| `--show-toplevel` 根验证 | 32 | 0.634926 |
| `.gitmodules` 成员读取 | 4 | 0.076558 |
| `--date-order` 历史/图查询 | 52 | 1.103044 |
| `for-each-ref` 引用读取 | 40 | 0.863470 |
| `status` | 4 | 0.277925 |
| 其他查询 | 284 | 5.752396 |
| 合计 | 416 | 8.708319 |

证据：`artifacts/reference-refresh-20261030/plico-git.jsonl`、
`idle-120s-commands.json`、`idle-120s-summary.json`。耗时使用 Git 自报 `exit.t_abs`，
不包含完整的父进程所见启动开销；累加值不是 CPU 时间，也不是用户等待时间。此样本没有
旧版本对照，且包含引用、操作状态、配置检查等不同来源，不能把 416 次全部归因于子模块
fallback，更不能宣称删除该 fallback 就能消除全部查询。

四次成员读取发生于 02:50:29.779375、02:50:59.853373、02:51:29.912912、
02:52:00.006576，间隔约 30 秒。源码确认存在以下放大链，而非 Git status 单条命令慢：

1. `worktree_events/project.rs::reconcile` 把所有 `.git` 为文件的根列入 fallback，
   不区分已成功安装外部管理目录监视器的本地子模块和确实无法监视的根。
2. `worktree_events.rs::Schedule::advance` 每 30 秒产生 `Batch.all/index`。
   `all` 同时被解释为历史变化、详情失效和项目成员可能变化。
3. `ui/worktree_sync.rs` 将该通知交给 Changes、详情、比较、项目发现；子根还触发聚合
   Log。`project_roots.rs` 的 Checking/Ready 生命周期保护在途查询，成员扫描期间的历史
   请求进入 pending，扫描完成再加载。这里的保护本身必要，问题是上游没有变化也反复触发。

另外，“现有观察器已监听外部目录”不等于可以直接删掉 fallback：

- `ProjectEvents::reconcile` 遇到相同根路径直接 continue；`Events::install` 的闭包持有
  安装时解析出的 GitPaths，后续不重新解析。管理目录绑定是快照，而非持续更新的身份。
- UI 只在 observation_roots 的 Arc 改变时调用 reconcile；项目发现结果路径相同又保留
  原 Arc。仅在 reconcile 内增加重绑定检查仍不够，检查入口本身也需要覆盖绑定变化。
- 所以 `.git` 指针改变而工作目录路径不变后，旧观察器可能继续监听旧管理目录。
  这是源码确认的生命周期缺口，尚未做原生指针重定向实验，不冒充已复现的界面故障。

独立的固定开销也已定位：该样本包含 68 次 `rev-parse --absolute-git-dir`。
`ui/worktree_sync.rs` 每 5 秒调用项目操作观察，`project_operations::Observer::poll`
逐根进入 `operation_state::Observation::read`；即便 marker 未变，该入口也先调用 Git
重新解析管理目录。状态缓存只省掉后续状态读取，没有省掉绑定解析；68 次也包含其他调用方，
不能全部算作此轮询。引用读取另有两个 argv 格式各 20 次，本节不据此断言两者完全等价可合并。

#### 有界实施决策与验收条件

下一批只解决“本地已成功监视的 gitfile 根”，不扩大到通用事件总线或重写全部 Git 后端：

1. 明确工作目录、worktree 管理目录、common 管理目录组成的绑定及其生命周期。后台检查
   `.git`/`commondir` 等绑定依据；文件读取有大小上限，超限、不可读或身份不确定进入保守路径。
   单靠长度/mtime 不能作为指针内容永不改变的证明。不要在 UI 线程进行解析或安装监视器。
2. 真实绑定变化先安装新观察器，再替换旧观察器，并发出一次完整失效；取消或安装失败不确认
   成功。运行期 watcher 错误/rescan、网络根、未知布局继续保守恢复，并允许后续重试安装。
3. 只有绑定已验证且监视正常，周期检查才可以不产生 `Batch.all`。检查到期和内容变化必须
   分开表示。保留手动完整刷新；不靠无限延长周期掩盖失效缺口。
4. 先完成上述生命周期，再评估让操作观察复用同一代绑定。写操作前置复验仍保持新鲜读取；
   不把后台缓存直接用于允许写操作。每轮限制工作量，避免大量子模块同时到期形成查询峰值。

实施前必须补原生 Rebased 的 gitfile/commondir 变更场景及截图，再映射到本地。
回归需覆盖正常外部 index/ref 事件、同路径指针改向后新旧目录事件、安装失败后恢复、取消、
linked worktree 的私有 index 与共享 refs 边界。实现后同一 parent/child 条件重采 120 秒，
比较完整命令分类和发现/历史启动次数；健康空闲根不应周期性重读成员和历史，真实变化仍须
自动更新。剩余固定轮询单独统计，不将总数目标武断设为零。

本轮停止在证据与实施边界收敛：没有修改产品代码、没有重跑测试、没有新增原生指针改向验收，
也没有得到 Pull/Push 网络耗时或整体提速百分比。操作完成刷新与固定 Diff 解释失效保持独立待办，
不顺带扩展此批范围。

后续前置实验已完成 gitfile 改向：原生 Rebased 两次新绑定提交均需手动刷新才在本次
观察窗口内出现，第二次自动观察约 37 秒；手动刷新保持选中提交。具体截图、源码依据和
实验恢复见 REBASED-REFERENCE.md 的“gitfile 改向原生前置与失败探针”。这不构成复制
上游缺陷的理由，而是明确本地需要覆盖的恢复边界。
Plico 真实 OS 探针在正常旧目录通知通过后，改向并 reconcile 相同工作目录仍漏掉新 index
通知；隔离运行 36e4-18e3311d0c52e0e4-0 为 0/1，目标断言失败。探针已归档 artifacts，
未留在正式测试集合中。下一步具备 gitfile 场景的原生前置、源码映射和可重放失败条件，
可以开始补绑定生命周期；尚不放宽 commondir/未知布局的兜底，也不宣称性能改动已完成。

### 绑定生命周期实施中

在原生前置之后实现 `worktree_events/binding.rs`：保留工作目录、gitfile、worktree/common
管理目录和 commondir 的有界证据；指针文件直接比较内容（每项最多 8 KiB），目录比较创建
时间，不能取得该信息或遇到间接路径时保守兜底。目录 mtime 不参与比较，避免 index.lock
导致每次操作误判身份变化。安装阶段复读 GitPaths 并检查安装前后证据，拒绝不稳定结果。

`ProjectEvents` 增加独立到期时间、round-robin 游标与待确认失效。每次后台检查一个根，
稳定时不启动 Git；变化或 watcher rescan 时先安装新监听，成功后释放旧监听，失败保留旧
监听并在 30 秒后重试。取消不确认结果。失效通知持续到 UI 接收后显式 acknowledge；
不能因已读取或暂时被操作/文档锁拒收就丢失。`ui/worktree_sync.rs` 在根列表 Arc 不变时
也执行到期检查，重新绑定会触发解释及全量刷新，避免仅依赖普通 refs 比较。

健康本地无 commondir 的根关闭周期 all；网络路径、linked worktree 和未知绑定继续保守
兜底。未共享操作状态轮询的管理目录缓存，本批不同时改变写操作前置条件。
定向验证：61c8-18e331679d731a68-0 的首版 OS 测试 6/6；
48c-18e331775d7b42a4-0 观察器单元 16/16、973 filtered；补充失败恢复和取消检查后，
5e1c-18e3318729ecad64-0 真实 OS 测试 **7/7**。正式回归新增 repository_binding.rs：
自动检查（不重新 reconcile）后新目录 index 通知、旧目录事件隔离、未确认重复投递、取消、
健康空闲无 all，以及初始绑定失败后无成员变化的恢复。尚待发布构建、Plico 原生后验、
同条件空闲追踪与完整回归，不属于提交就绪或全绿结论。

发布构建及 gitfile 改向/后续提交/恢复原绑定的 Plico 原生后验通过，截图见参考文档。
同一 parent/child fixture、相同测试分支和选中提交、Changes 打开，稳定后采样 UTC
03:44:32.878 至 03:46:32.878 的 120 秒；未运行构建或测试。结果
`artifacts/binding-refresh-20261030/idle-120s-summary.json`：顶层 Git **100 次**，
全部匹配 exit；根验证、成员读取、历史/图、status 均 **0 次**，引用 **23 次**、其他
**77 次**。基线对应为 416、32、4、52、4、40、284，顶层启动数减少约 76.0%。
仍存在引用和操作状态等必要/待优化轮询，不把总数视为全可消除。
Trace2 进程内累计耗时 2.058396 秒（基线 8.708319 秒），仍不是 UI 等待或 CPU 耗时；
本轮仅一次同条件采样，没有方差、RSS、网络操作提速结论。idle-before/after.png 保持选择、
详情与 Changes；测试分支按 expected OID 删除，仅停止自建 Plico 进程。
功能代码现冻结，接续完整 core/desktop/navigation、doctest 和 clippy；完成前不提交功能改动。

提交前验证完成：隔离运行 `5d68-18e33210aa753238-0`，完整 core **989/989、0 skipped**
（1236.624 秒），完整 desktop **190/190、0 skipped**（867.650 秒），navigation
**10/10、180 filtered**（8.351 秒），三个 status 文件均为 exit code 0。导航使用项目
隔离入口规定的 test profile / debug-assertions=false，不冒充完整 release profile。
随后 doctest 成功（当前 0 个示例），`cargo clippy --locked --no-default-features --all-targets`
成功退出，既有警告保留，日志为本批 artifacts 下 clippy.log。代码及文档冻结摘要核对一致；
补充此验证记录后再次执行 fmt/diff 检查。没有推送或本批 CI 结果。

此批完成绑定生命周期和健康本地 gitfile 根的空闲刷新收敛。后续独立问题仍是操作完成后的
按实际结果刷新、操作状态轮询的目录复用、固定 Diff 解释失效；本批不以这些尚未实现的
收益作宣传，也不将网络/linked worktree 的保守兜底算作已经优化。

### Push/Fetch 完成路径复核（6a4c9f5 之后，只读审计）

本轮没有改动产品行为。复核 `ui/remote.rs::start_push` 的 worker、完成回调和
`push_flow::execute` 全流程：UI 在任何完成结果后均进入 `refresh_impl`，重新发现项目根、
刷新全部分支比较、清详情缓存、刷新 Changes 并读取历史。`finish_operation` 只处理操作
身份和文档锁，不提供“工作区未变化”的证明。

Rebased `245042348669114adb8e24e5b0fc0ce73763f3c4` 对照：

- `GitPushOperation.java::execute` 的 finally 对推送根执行 repository.update；真正执行
  Update 的 `update` 另外对相关根做 VFS refresh 和 repository.update。不是一律不刷新，
  也不是所有 Push 都按完整工作区更新处理。
- `GitFetchSupportImpl.kt::doFetch` 在成功后 repository.update；上层聚合每个 remote 的
  结果，并调用成功 Fetch 的处理器。这里只核实了这些已读入口，未据此断言失败路径绝无
  其他刷新机制。Plico 的失败/取消后复验属于明确保留的保守差异。
- Rebased 的 Push 重试中可能执行 Update。Plico 同样可能 Merge/Rebase、保存与恢复本地
  更改；最终 Status::Success 不能推出 HEAD/index/worktree 没变。`push::ResultState.state`
  没有 HEAD OID、完整 refs/config/绑定身份，不能单独承担窄刷新证明。

Fetch 已有一定收敛：只有已完整渲染的单根 Log、非 unshallow、成功且未取消、历史观察
相同、查询 stamp 仍有效时，才跳过完整刷新。多根、失败、取消、浅边界改变和查询被替换
继续刷新。`HistoryObservation` 覆盖 refs 与 shallow，不是整个工作区/配置/绑定的快照；
不得直接把它扩展成 Push 全场景的唯一判断依据。

一个更小而明确的查询放大点：`push_flow.rs` 在每次 push 后无条件调用 pull_targets，
而 `push_recovery::after_push` 首先根据 status/force 判定 Stop。正常附着分支的
pull_targets 包含 `remote`、远端 `for-each-ref`、`symbolic-ref`、当前分支 upstream
`for-each-ref` 共 **4 次** CLI；成功、已是最新、force、非快进以外的拒绝/失败均不会使用
读取出的 Targets。该数量由完整调用路径确认，尚未新增 Trace2 实测，不能套用旧进程
中位数宣称实际节省多少毫秒。读取失败还可能在远端已成功后将 report.error 置为错误，
因此移除时也要验证结果呈现，不能当作完全无语义变化的格式调整。

后续优先顺序收敛为：先将“是否可能进入恢复”这一纯策略判断放到目标读取之前，仅在
非 force 的 RejectedNonFf 等确实需要当前 tracking 身份时读取；保留 push 本身的写前
源/lease 校验、写后 operation_state 与 UI 完成刷新。随后才评估可接纳的完成观察快照，
避免为省刷新反而每次追加更多查询。不新增通用事件总线，也不把 UI 缓存当写入授权。
已有真实 Git 恢复测试位于 tests/repository.rs 的
rejected_push_flow_updates_retries_or_preserves_conflict_and_cancel 等，覆盖 merge/rebase、
冲突、取消与草稿保留；下一批须补终止结果不读取恢复目标、真正拒绝仍复验 tracking 的证据，
并按项目约定先做同场景 Rebased 原生前置。当前没有实施这两项后续改动。

### Push 终止结果的恢复查询按需执行（2026-10-30）

先完成本地 bare fixture 的 Rebased 正常/已是最新原生验证及源码核对，再修改
`push_recovery::needs_targets` 与 `push_flow::execute`。状态、force、attempt 的纯
门禁由恢复策略模块拥有，执行器在读取 Targets 前调用；原 after_push 共用门禁，
只有普通 non-FF 拒绝仍进入实时 tracking/current branch 检查。没有缓存恢复目标，
没有弱化写前源 OID、force lease 或写后 operation_state，更没有调整 UI 完成刷新。

普通终止结果省去一次 pull_targets，即 attached 分支上的 4 条 Git 查询。另一个
行为变化是成功/终止结果不再被不相关的恢复目标读取错误附加 report.error。此处不
承诺网络耗时或端到端百分比；本次原生验收与回归并行，不作为性能计时样本。

新增真实 Git 回归通过子进程启动前设置 PLICO_PERF_TRACE，覆盖 NewBranch、Success、
UpToDate 和拒绝后取消恢复，校验状态、远端 HEAD、本地修改以及实际 remote 命令数。
前三者各 1 次（写前快照），拒绝为 3 次（写前、恢复目标、合并风险检查各一次）。
首次实验误把拒绝路径期望设为 2，运行 `2e34-18e334d1775d05b0-0` 新测试失败；
跟踪并读取 rebase_over_merge_problem 后修正测试期望，未为通过测试删去风险检查。
随后 `5370-18e334e4a614d618-0` 定向 4/4 通过，另 986 项过滤；包括既有
merge/rebase 重试、冲突、取消与草稿保留。之后补充 Forced 终止策略覆盖，进入完整回归。

Plico 同条件正常/已是最新原生结果通过，截图、构建摘要、保留差异见 REBASED-REFERENCE
相应节及 `artifacts/push-terminal-20261030/`。自建 Plico 已停止，fixture 远端保留
已推送的 HEAD；不触及产品仓库远端。代码摘要 frozen-code.json 已冻结，当前等待完整
core、doctest、clippy 与 Push 对话框定向桌面回归。此次没有修改桌面生命周期，不重跑
整套导航，不将定向桌面结果冒充全量。

本批范围止于该查询门禁。完整刷新按实际副作用收敛仍待独立设计：必须区分直接 Push
与中途 Update/保存恢复，且考虑 hooks、外部写入、失败和取消。最终成功状态本身不能
证明工作区未变，本批没有用该状态直接跳过 Changes 或成员刷新。

提交前验证完成：完整 core `3b9c-18e3350474e63378-0` **990/990、0 skipped**，
1219.487 秒，status exit 0；Push 对话框定向 desktop
`4f68-18e3362cf402a744-0` **2/2、188 filtered**，status exit 0。
doctest 成功（0 示例）；clippy 成功，保留既有警告，日志位于本批 artifacts。
最终源码摘要与 frozen-code.json 一致，fmt/diff 检查通过。没有运行本批 CI 或推送。

### 操作状态轮询的目录解析复核（5e623cb 后，只读审计）

当前工作区起点 5e623cb。重新拆分已有 binding-refresh-20261030 的 Trace2 原始样本，
仍使用 03:44:32.878Z 至 03:46:32.878Z 的 120 秒窗口、顶层 start（sid 无斜线）：
100 次启动中，46 次 `rev-parse --absolute-git-dir`，23 次 `rev-parse --verify HEAD`，
23 次 for-each-ref，8 次 config --null --list --includes。目录解析在 parent 和 child
各 23 次。此为旧样本再分析，不是新版本性能测量，46% 是调用数占比，不是提速预期。

完整读取本地 ui/worktree_sync.rs、project_operations.rs、operation_state.rs、
operation_inspection.rs 及 watcher 的 binding/git_paths/project 生命周期。调用链为
每 5 秒（或根/事件变化）Observer::poll → observe → Observation::read →
editor_directory，每根即使 11 项操作标记完全不变也要启动一次 Git。改变时再读取
operation_state_in，并在末尾重新读取 Observation 复验。只读 Inspection 则把状态与
恢复记录合在一次目录解析中，最后重解析防止混用目录，不能把两条调用路径混为一谈。

Rebased 245042348669114adb8e24e5b0fc0ce73763f3c4 再次读取
GitRepositoryUpdater.kt 全文、GitRepositoryImpl.kt::update/readRepoInfo、
GitRepositoryReader.kt::readRepositoryState/findRebaseBranch：长寿命 repositoryFiles
与 reader 持有目录，VFS 按 HEAD、refs、config、merge/rebase 分别触发更新，操作标记
从对应文件读取。不能据此宣称 Rebased 所有状态读取均零 CLI，也不能复制其 VFS 生命周期。

下一优化不能简单使用 ProjectEvents 当前保存的目录：绑定轮询每 30 秒、每次一根，
操作轮询每 5 秒、遍历所有根；take 之后才提出的重建请求可能尚未执行。重建失败时为保留
观察能力还会暂留旧 watcher。必须在读取操作状态的同一请求内验证绑定，且前后检查、
失败为 unknown、取消不接纳、未送达重试都保留，不能把“监听器存在”当新鲜目录证明。

额外发现了只用 Binding::current 仍不充分的边界。独立临时仓库实验记录
`artifacts/operation-binding-audit-20261030/config-probe.json`：仅追加损坏的 .git/config，
未改 .git 指向，`rev-parse --absolute-git-dir` 从 exit 0 变为 128；恢复原配置又返回 0。
现有 Binding 记录目录创建时间、指针文件和 commondir，不记录 config。若缓存命中直接
返回旧 Observation，就会绕过当前实现的配置解析错误，并可能把不可读仓库继续视为 idle。
这是 CLI 实验加源码推导，尚未实现缓存版本或运行其负例；不能称已复现产品回归。

因此下一批需先明确目录身份与 Git 可读性的共同失效条件，再决定抽取 watcher/observer
可共用的有界证据，避免新增一套互不同步的目录缓存。外部 include/global config、路径
间接层、linked worktree 与网络/未知布局要有明确回退；不得仅延长轮询间隔掩盖开销。
已读既有 tests/project_operations.rs 的 gitfile 改向/损坏/取消、tests/conflicts.rs 的
merge --quit（refs/index 不变）与嵌套操作标记测试。新增实现还须覆盖配置损坏/恢复、
稳定轮询 CLI 数量、绑定变化后旧标记不串用及 UI 未接纳重试。

本轮未修改功能代码，没有新的原生验收，也不拿旧 merge 截图替代未来实现前置；测试
基线沿用 5e623cb 的记录，不称本轮重跑。临时配置已恢复，仅新增上述审计证据与设计约束。

### 配置失效边界回归与收益判断（15aa1fc 后）

读取 Configuration::poll 及其 missing include/error/recovery 测试后确认：当前配置观察
每 30 秒依靠 Git 解析 includes、计算摘要，错误和恢复都触发失效；不保存配置依赖文件
集合，也不提供每 5 秒操作状态读取所需的新鲜可读性证明。直接引用它上一次成功摘要会
改变 unknown 的发现时机，单独缓存 .git/config 则遗漏外部 include 与条件 include。

新增 tests/project_operations.rs::operation_observer_does_not_hide_config_errors_behind_unchanged_markers，
直接经过 Observer::poll 和 Inspection::read，覆盖本地 config、仓库外含空格路径的
include、onbranch:main 条件 include。后两项从不存在变为损坏，主配置、HEAD/index
字节保持不变；目标根仍发布错误、另一个根正常、恢复记录为保守 unknown；修复后一次
取消不吞掉恢复，下一次观察回到正常，旧发布快照保持错误。没有修改功能代码或缓存策略。
隔离定向运行 `2cc4-18e3368bf3759e80-0`：10/10 通过，981 filtered，包括整个
project_operations、两个 operation_observation 和配置 missing include 回归；不是全量。

对同一旧 Trace2 窗口按目录查询 start.sid 匹配 exit：46/46 匹配，均 exit 0，t_abs
合计 0.937601 秒/120 秒。该值不包含完整父进程启动等待，不是 CPU、UI 延迟或新版本
计时。收益证据与复杂度结合后的决定：暂不为了省这部分轮询引入配置依赖解析器，保留
上述测试作为以后共享生命周期优化的约束。下一优先级回到用户触发操作的端到端查询与
完成刷新；目录缓存候选保留为未实施，不用提高轮询周期或放宽错误语义冒充优化完成。

本批仅测试与审计记录：cargo fmt 检查、git diff --check、doctest（0 示例）及
`cargo clippy --locked --no-default-features --all-targets` 成功；既有 clippy 警告保留，
日志为 operation-binding-audit-20261030/clippy.log。没有运行全量核心/桌面、原生 UI 或 CI。

### 已是最新 Push：原生查询时间线与首屏重复读取（22fdfca 后）

使用 computer-use 操作 Windows Plico；release SHA256 仍为
06565C639B77F84FB0C45EBBF0EE9BD343E6CBD7279F680C8E2D715B23195F58，最近两批只有
测试/文档，功能与该构建一致。使用已有 parent/child 临时仓库及本地 bare remote，
选中 child 提交、展开 Changes；正常打开 Push 并执行已是最新检查。没有并行构建或
测试，没有改变远端引用、HEAD 或工作区内容。结果、选中行、详情、1 项修改/4 项未跟踪
文件保留。仅关闭本次自建 Plico，Rebased 窗口保留。

证据 `artifacts/push-completion-20261030/`：build.json、times.json、before/after.png、
backend.jsonl、git.jsonl、execute-queries.json、summary.json。times 是操作工具调用
边界，不能把 execute 到 observed 的约 10 秒当实际 UI 延迟。以下使用 Git Trace2
顶层 start/exit，嵌套子进程排除；单次样本，无方差、无 Rebased 同条件计时比较。

本地 Push 进程 05:14:39.621805Z 启动，39.709688Z 退出，t_abs 0.095012 秒。
其后的操作状态复验仍保留。39.997455Z 至 44.411202Z 是明显后续查询突发段：
85 次顶层 Git 启动，进程内 t_abs 累计 1.924361 秒，首 start 到末 exit 约 4.414 秒。
执行按钮到最终观察的整个窗口共有 105 次；末尾包含常规轮询，不能全归因到 Push。
这些量不是帧绘制时间或 CPU 时间，也不把本地 bare 的 Push 时长外推到公网。

源码定位到确定的重复，而非仅根据时间邻近推测：

- ui/remote.rs 完成后调用 refresh_impl；repository.rs 刷新成员、Changes 与历史。
- ui/project_navigation.rs::reload_project_history 在 fresh 分支执行
  `project_log::read(&queries, limit, token)?.scope`，丢弃已经读取的首屏 page 和 offsets。
- project_log::read 实际逐根读取历史、合并 page、记录固定 tips 并 advance_offsets，
  绝非只建立查询范围。之后 navigation::window_at 从空 offsets 开始，即使 offset=0
  也调用 next_page(limit)，逐根重新读取同一首屏。Trace2 中父子根先各读首屏、主根
  再读 limit=1 元数据，随后父子根又各读首屏，与此流程相符。
- ProjectRoots 的 Checking/Ready 已有 pending_project_reload 协调；当前证据不足以
  归咎于重复通知启动了两次刷新。真正可确认的是同一次 fresh worker 内的数据被丢弃。

再次读取 Rebased 245042348669114adb8e24e5b0fc0ce73763f3c4
plugins/git4idea/backend/src/log/GitLogProvider.kt::readFirstBlock：collectMetadata 得到
commits/refs，按需补 refs/tag 数据后排序并返回这些 commits；没有为取得 scope 而丢弃
整块 metadata 再读相同首屏的步骤。保留 Plico 的 tag-only history 与多根预算差异。

下一批优先处理这处结果所有权交接：首屏应从已读 Read 构造导航 Window，保留每根
offsets、固定 tips、错误根、more 和 graph frontier，避免 next_page 从零重复读取。
先覆盖 offset=0 的 fresh 首屏，同时明确非零窗口与 continuation 的路径；不能在
已经 advance_offsets 的 Read 上直接 next_page 而跳过首屏。检验跨根交错排序、空根、
失败根、分页接续、取消和选择保留。先完成对应 Rebased 原生场景记录，再修改功能，
随后用同 fixture/trace 做后验。这与按结果缩小 Push 刷新范围是两件事，本批发现不依赖
放宽 hooks、失败或恢复检查。当前仅新增测量及源码证据，没有修改功能或声称提速完成。

### 首屏结果所有权交接：实现与同条件后验（cb99ef1 后）

完成 Rebased 原生前置与源码映射后，新增 navigation::Window::from_first_page，直接
消费 project_log::read 返回的首屏、每根 offsets、固定 scope 与 more，再构建图 frontier。
ui/project_navigation.rs 的 fresh、offset=0 入口保留这份 Read，不再丢弃后调用
window_at 从零读取。没有跨刷新缓存；非零 offset、continuation、主根 limit=1 快照、
写前复验及操作后刷新范围保持原路径。本批不扩展为刷新策略重写。

真实 Git 新回归 project_first_window_consumes_loaded_rows_without_rereading_and_continues_fixed_sources：
两根含相同提交 OID，另有空根及失败根；读取后暂时移走两根 .git，首屏转换仍成功，
证明该转换不依赖重新访问 Git。恢复后移动一根分支，按 1/3/100 页大小接续，逐项验证
根限定身份、固定 tips、游标推进、元数据转出后的 offset、错误保留、空页及取消。
定向 core 3/3（989 filtered，3ccc-18e3371d92f54334-0）；实际桌面入口
project_navigation 两项 2/2（188 filtered，2e18-18e3374907ac21b8-0），覆盖初始成员
加载、选择/过滤/导航、失败根恢复及过期任务取消。定向结果不算全量。

原生证据 artifacts/project-first-page-20261030/：visible-build.json、times.json、
plico-before/refreshed/push-entry/push-result.png、visible-backend.jsonl、
visible-git.jsonl、execute-queries.json、summary.json。release SHA256 为
4FF8CCC64F790C0F5BB7F468210F78B974D3C618A3C29422DB99C7320965706A。
同一 parent/child 临时仓库与本地 bare，选择 child 提交并展开 Changes，Ctrl+F5 后
仍为 3 行、原选择、右侧两项文件和详情、1 项修改/4 项未跟踪。随后执行已是最新 Push，
结果及以上状态同样保留。采样时没有并行构建/测试；仅关闭自建 Plico，保留 Rebased。
首次用 Hidden 启动没有可操作窗口，该进程已停止；build.json/backend.jsonl/git.jsonl
是该次启动记录，不用于本次对比。对比使用 visible-*，避免混淆样本。

与上一节同条件单次采样比较（均为顶层 Trace2 start/exit，排除嵌套 Git）：

| 指标 | 修正前 | 修正后 |
| --- | ---: | ---: |
| 父子根完整首屏 metadata log 查询 | 4 | 2 |
| Push 后查询突发段启动数 | 85 | 67 |
| 突发段首 start 至末 exit | 4.414 秒 | 3.242 秒 |
| 突发段 Git 进程内 t_abs 合计 | 1.924361 秒 | 1.550228 秒 |
| 本地 Push 进程 t_abs | 0.095012 秒 | 0.096769 秒 |

后验 Push 为 05:28:38.435657Z–38.525575Z；操作状态复验后，突发段
05:28:38.763636Z–05:28:42.005588Z，execute-queries.json 的索引 18–84。
执行工具到最终观察的窗口共 89 次，后段仍有正常轮询；不能全部归于操作刷新。
同样没有把工具调用边界当 UI 延迟，也没有把小型本地仓库单样本外推到大仓库或公网。
确定性收益是消除同一请求的重复首屏查询，时间差只作该样本证据。

剩余成本明确保留：主根 limit=1 快照仍查询 refs/metadata、每根图父关系读取及刷新中的
其他状态/详情查询仍存在。下一阶段如继续，应先检查这些数据的所有权和一致性边界，
不要因查询名称相同就直接缓存或合并。本批到首屏交接为止，不继续扩大实现范围。

按完整命令进一步核对：消失的每根首屏重读包含 6 条仓库/引用元数据查询、metadata log、
rev-list 及 2 条图父关系 log，即两根共 20 条。后验突发段恰多重叠 1 次轻量 refs 轮询
和 1 次 HEAD 读取，因此该窗口净差为 18，不能将 85→67 当无噪声固定预算。
完整首屏 log 从每根 2 次变 1 次与实际调用路径一致；其他定时轮询仍正常运行。

本批最终验证：完整 core 992/992、0 skipped，耗时 1271.261 秒，运行目录
artifacts/test-runs/3524-18e3378851bdec48-0。上述 desktop 定向 2/2；另 navigation
10/10（180 filtered，3f68-18e337ad6da69b24-0）。doctest 成功但当前 0 示例；
cargo clippy --locked --no-default-features --all-targets 成功，既有警告保留，日志
project-first-page-20261030/clippy.log。cargo fmt --package plico -- --check 与
 git diff --check 通过。冻结代码哈希位于同目录 frozen-code.json，验证期间未再改功能代码。
本批未跑完整 desktop 或 CI；没有推送，也没有发布包。
