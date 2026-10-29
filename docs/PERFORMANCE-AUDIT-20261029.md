# Windows 原生操作速度对照（2026-10-29）

结论：在本地 bare remote 的小仓库中已复现 Plico 的额外等待。Git 传输本身没有更慢；主要差异在串行查询、Push 预览逐提交查询，以及操作后刷新。已实现无变化 Fetch 保留历史、Push 变化批量读取和刷新观察基线同步，并完成原生复测与完整回归，结果见下文。

## 环境与方法

- Plico：`0.1.0-preview.13`，`b30b13871110a7d38e3f6aafc5419ad76a23e932`，Windows CI release 包。
- 安装版 Rebased：1.1.22 / `262.10968.SNAPSHOT`。源码参考 `245042348669114adb8e24e5b0fc0ce73763f3c4`，不能视为与安装版完全一致。
- 同机 Windows，Git for Windows `2.56.0.windows.3`；应用预热后操作。Rebased 使用独立 config/system/log，保留用户已有实例。
- `%TEMP%/plico-speed-20261029` 下两套独立工作区、producer 和本地 bare remote。相同初始 50 个提交、200 个小文本文件，main 跟踪 origin/main。隔离 Git global/system 配置，禁用签名，排除 `.idea/`。
- 原生 UI 点击 Fetch、工具栏 Update Project（默认 Merge）、Push。Update Project 与普通 Pull 弹窗是不同入口，本次拉取结果仅覆盖前者。
- 两应用均继承 `GIT_TRACE2_EVENT`；点击前记录 UTC marker。按 sid 区分顶层 Git 命令与 Git 自己的子进程，后者不重复计数。退出状态、最终 HEAD/bare remote 及干净工作区均核验。
- Fetch 各 3 次；一次单提交 Update；分别一次 1/20 提交 Push 预览和实际 Push。不是统计基准，也未测公网、认证、冲突、取消、submodule、大仓库或冷启动。

原始证据在 `artifacts/speed-audit-20261029/`：`environment.json`、`setup.ps1`、`advance.ps1`、`markers.jsonl`、两份 `*-trace.jsonl`。`analyze.mjs` 可用 Node 执行，生成 `analysis.json`。初步 `summary.json` 包含空闲轮询，不作为最终计时依据。

计时口径：以下“查询链”从首个顶层 Git start 到连续命令簇最后 exit，间隔至少 500 ms 划为下一簇，观察窗口最多 15 秒。这是原生操作触发的 Git 工作时间线，**不是按钮点击到最终画面的精确 UI 延迟**。并行与同时发生的轮询仍可能混入同一簇；marker 到首条命令另含输入注入和调度时间。截图用于确认状态，不能据相邻工具调用时间推算响应速度。

## 实测

| 场景 | Plico 查询链 | Rebased 查询链 | 顶层 Git 次数（Plico / Rebased） |
| --- | ---: | ---: | ---: |
| Fetch，无新提交，3 次范围 | 1.98–2.04 s | 0.48–0.57 s | 39–40 / 5 |
| Update Project，拉取 1 提交 | 7.57 s | 3.23 s | 130 / 46 |
| 打开 Push，1 提交 | 2.14 s | 0.016 s | 29 / 1 |
| 打开 Push，20 提交 | 2.93 s | 0.021 s | 47 / 1 |
| 确认 Push，1 提交 | 3.06 s | 1.49 s | 50 / 13 |
| 确认 Push，20 提交 | 3.29 s | 1.67 s | 50 / 13 |

预览行的 Rebased 数值只是那一次批量 Git 查询的 start→exit，不能解读为弹窗 16/21 ms 内可用。对应 marker→最后 Git exit：1 提交 Plico 2.285 s、Rebased 0.407 s；20 提交 Plico 3.067 s、Rebased 0.208 s。Plico 的单提交簇尾还包含两条同期 refs 轮询，20 提交簇内包含一条 git-dir 轮询，因此不把整个差值都归因于预览。

Plico 打开 Update Project 前另有 27 条查询，持续 1.716 s；Rebased 打开选项弹窗没有触发 Git 查询。两者执行耗时表不包含用户停留在弹窗中的时间。

实际传输/合并命令的 Trace2 `t_abs`：

| 命令 | Plico | Rebased |
| --- | ---: | ---: |
| 空 Fetch，3 次 | 137–145 ms | 186–245 ms |
| Update 内 Fetch | 242 ms | 311 ms |
| Update 内 Merge | 47 ms | 60 ms |
| Push，1 提交 | 287 ms | 362 ms |
| Push，20 提交 | 499 ms | 570 ms |

因此这组实验支持“客户端查询编排增加等待”，不支持“Plico 的网络传输更慢”。公网问题仍需独立场景。

另一个现象：Plico 两次 Push 后分别在 marker 后约 5.54 s、4.57 s 再出现一簇 35/34 条查询，持续约 1.66/1.72 s。它们不计入上表首簇，但会继续占用后台。与五秒 refs 观察再次触发整轮刷新相符；没有应用内调用关联 ID，暂不将全部后续命令唯一归因于该路径。

## 源码核对与优化顺序

### 1. 先合并操作后的刷新

本地 `src/ui/remote.rs` 的 Fetch/Push/Update 完成路径调用 `refresh_impl`；`src/ui/repository.rs::refresh_impl` 同时刷新项目 roots、分支比较、历史、详情缓存及已有本地视图。空 Fetch 即使 refs 不变也走该路径。Trace 中可见多次 git-dir、HEAD、shallow、refs、历史和详情查询。

`src/ui/auto_refresh.rs::start_auto_refresh` 每五秒读取 refs，发现与 `updates.references` 不同又调用 `refresh_impl`。操作完成刷新的结果没有在该入口直接合并为观察基线，有重复刷新的可能。

上游 `plugins/git4idea/backend/src/fetch/GitFetchSupportImpl.kt::doFetch` 成功后调用 `repository.update()`；本次空 Fetch 实机仅 5 条命令。不能只因上游也有 update 调用就认为两边刷新成本相同。

建议：同一仓库使用一次有代次的完成快照，按 refs/HEAD/worktree/config 的变化刷新消费者；结果同时更新观察基线，合并 in-flight 请求。refs 未变化时保留历史和详情缓存。失败或取消仍要复验实际状态，不能直接跳过刷新。

### 2. Push 预览批量读取，消除逐提交进程

本地 `src/push_preview.rs::load` 对每个 outgoing commit 调用 `src/git.rs::push_commit_changes`，每条启动 `git diff-tree`。实测 1/20 提交确实分别出现 1/20 次 diff-tree。另有重复 outgoing log、remote、OID 解析及 refs 复验。

上游已读取的调用链：

- `plugins/git4idea/backend/src/push/GitOutgoingCommitsProvider.java::getOutgoingCommits`
- `plugins/git4idea/backend/src/history/GitHistoryUtils.java::history`
- `plugins/git4idea/backend/src/history/GitLogUtil.java::readFullDetails`
- `plugins/git4idea/backend/src/history/GitDetailsCollector.kt::readRecordsFromHandler`

规则：范围历史和 `--name-status` 元数据在同一条 log 中流式读取，结合 rename/merge 规则解析。安装版 Trace 同样显示 1/20 提交都是一条 `git log -M -c ... --name-status`。

建议：采用有数量/字节上限的批量元数据读取，让列表与聚合文件预览共享结果；文件内容继续按需加载。保留 shallow 边界、merge combined diff、rename 链、refs 变化检测与取消行为。不能为了速度把聚合语义替换成简单端点 diff。

### 3. 合并同一阶段的状态查询，保留写入前复验

本地 `src/project_update.rs::prepare`、`src/merge_update.rs::validate/execute`、`src/push_target.rs::push_snapshot`、`src/push.rs::execute` 会在不同阶段重新读取 targets、HEAD、operation state。部分复验是防止审阅后仓库变动所必需；单次 Update 的 130 次调用不能全部视为冗余。

上游 `plugins/git4idea/backend/src/update/GitUpdateProcess.java::update/updateImpl` 同样在 fetch 前后校验配置并检查未完成操作。应以阶段边界为单位共享只读快照，保留取得写锁后、fetch 后、真正写入前的必要复验，而非删掉保护检查。

### 4. 进程启动成本作为后续专项

小查询自身仅十几到几十毫秒，Plico 连续串行调用常相隔约 60–75 ms。`src/process_tree.rs` 在 Windows 上为每个子进程创建 Job、挂起启动、捕获线程快照再恢复；`src/git.rs` 还包含管道线程与轮询。当前 Trace 无法分解启动前、调度、清理、杀毒扫描等成本，不能断言某个 Windows API 是主因。

先减少调用总数，再加分段 instrumentation 判断进程层是否值得优化。保留取消时后代清理、无关进程隔离、管道关闭等保障，不直接绕过 Job Object。

## 原生状态与截图

以下文件均相对 `artifacts/speed-audit-20261029/`：

| 场景 | Rebased | Plico | 对比 |
| --- | --- | --- | --- |
| Fetch 入口/执行/完成 | `rebased-fetch-entry.png`、`rebased-fetch-running.png`、`rebased-fetch-success.png` | `plico-fetch-entry.png`、`plico-fetch-running.png`、`plico-fetch-success.png` | 均在分支弹出菜单提供 Fetch；运行有反馈，最终成功 |
| Update 选项/完成 | `rebased-update-entry.png`、`rebased-update-success.png` | `plico-update-entry.png`、`plico-update-success.png` | 工具栏入口，默认 Merge；一提交更新成功 |
| 单提交 Push 预览/完成 | `rebased-push-entry.png`、`rebased-push-success.png` | `plico-push-entry.png`、`plico-push-success.png` | 提交列表在左、文件在右、确认按钮在底部 |
| 20 提交 Push 预览/完成 | `rebased-push-twenty-entry.png`、`rebased-push-twenty-success.png` | `plico-push-twenty-entry.png`、`plico-push-twenty-success.png` | 两边均得到 file-0.txt 聚合结果并成功推送 |

Plico 部分完成截图是在下一轮 fixture 提交创建后保存，主列表已显示下一轮内容；以 Trace 和最终 bare ref 核验动作结果。此次保持默认焦点，不测试快捷键、滚动、失败、取消和禁用态；没有进行像素级验收。Rebased 的成功反馈为通知，Plico 为底部状态文字；文件树层级与弹窗尺寸仍有差异。

两套工作区最终均干净，且各自 HEAD 等于各自 bare main：Plico `0ed3b0a58d1913068a123bc98bf25bda9d9ffbe3`，Rebased `323364a63fa8a42b5453d5174e466fff1eed59b3`。只操作测试仓库。

本轮优化限定为“刷新去重 + Push 预览批量化”，没有扩展到 Git 写操作执行器或 Windows 进程管理重构。时间数字只作本机比较，不设置依赖单台设备速度的硬 CI 门槛；正确性继续覆盖 refs 变动、取消、浅克隆、merge 与 rename。

## 优化实现与原生复测

实现基于上述 preview.13；优化版身份见 `optimized-build.json`，release 可执行文件 SHA-256 为
`66FFBEF08E8FA4F4F51EC2DDDDEF4FC915ED5DF512FF8CBD92B85CC963CC4247`。
另从同一 seed 建立 optimized 工作区、producer 和本地 bare remote。正式计时前，构建与桌面测试均已结束。
先前 `smoke-fetch` 样本有后台负载，仅作行为冒烟，不纳入本表。

| 场景 | 原版查询链 | 优化版查询链 | 原版 / 优化版顶层 Git 次数 |
| --- | ---: | ---: | ---: |
| 空 Fetch，3 次范围 | 1.98–2.04 s | 0.912–0.984 s | 39–40 / 12 |
| Update Project，拉取 1 提交 | 7.572 s | 5.883 s | 130 / 99 |
| 打开 Push，1 提交 | 2.137 s | 1.752 s | 29 / 28 |
| 打开 Push，20 提交 | 2.934 s | 1.828 s | 47 / 27 |
| 确认 Push，1 提交 | 3.059 s | 3.359 s | 50 / 53 |
| 确认 Push，20 提交 | 3.294 s | 3.469 s | 50 / 53 |

保持前文首个连续 Git 命令簇口径，不把查询时间等同于精确 UI 延迟。空 Fetch 中位数从
2.037 s 降至 0.950 s（约 53%）；20 提交预览约减少 38%；单次 Update 约减少 22%。
单提交预览没有减少实际的逐提交读取次数，较小时间差还包含运行波动及同期轮询，不能全算作批量化收益。

Push 确认后的首簇增加了完整刷新前的观察采样，两个样本均略慢。收益体现在后续工作：
原版两次 Push 还各有 35/34 条查询、约 1.7 s 的整轮刷新；优化版之后只有 refs、远端提示等
5–6 条查询约 0.36 s，以及常规空闲轮询，没有第二次历史 log 重载。因此不宣称推送确认立即变快。
与 Rebased 的差距仍存在，剩余阶段校验和查询编排留作后续有界专项，不能直接删除写入前复验。

实现映射：

- `src/git_push_changes.rs`：按明确 OID 顺序，每批最多 128 个，流式读取 `show --no-walk=unsorted`
  的 name-status；由 `src/push_preview.rs` 调用。保留 combined merge、rename、shallow 拒绝、
  NUL 路径、每提交 8 MiB 和总变化数量上限、取消及输出 OID 复验。1/20 提交均只需一次变化批量查询。
- `src/branch_updates.rs::HistoryObservation`：比较 refs 与完整 shallow 边界；
  `src/ui/repository.rs` 在完整历史读取前采样，仅成功接纳当前查询时发布。
  `src/ui/auto_refresh.rs` 同步轮询基线并丢弃跨代次结果；筛选和翻页不增加采样查询。
- `src/ui/remote.rs`：仅已显示完整单仓库历史、成功且 refs/shallow 不变、查询代次不变的普通 Fetch
  保留历史。失败、取消、读取错误、多仓库、unshallow 等路径继续刷新。

优化版原生截图（均在同一 artifacts 目录）：`optimized-fetch-success.png`、
`optimized-update-entry.png`、`optimized-update-success.png`、`optimized-push-entry.png`、
`optimized-push-success.png`、`optimized-push-twenty-entry.png`、`optimized-push-twenty-success.png`。
菜单、默认 Merge 选项、提交列表及聚合 file-0.txt、成功状态与原版保持一致；没有改变布局、焦点或快捷键。
更新后主列表从 50 增至 51 条，最终 72 条；工作区干净，HEAD 与 bare main 均为
`57bdae10e9aaac15c79e0809232747a14d6c87ee`。本轮原生复测未触发取消、失败、认证和冲突；
这些边界依靠相应自动回归，不能把成功截图视为它们的原生验收。

最终验证：`cargo run --locked --manifest-path tools/test-runner/Cargo.toml -- all`
默认配置、无额外筛选/重试，退出码 0。结果目录为
`artifacts/test-runs/848-18e2e99993d5c200-0/`，含阶段日志、退出状态及 JUnit。

| 套件 | 结果 | 说明 |
| --- | ---: | --- |
| core | 952/952 | 零跳过，含批量读取、字节限制、refs/shallow、Git 写操作和取消/恢复 |
| desktop | 188/188 | 零跳过，含 Log 代次、观察基线、选择与生命周期 |
| navigation | 10/10 | 按固定入口运行导航；其余 178 项由 desktop 覆盖 |

`cargo fmt --package plico -- --check`、`git diff --check`、
`cargo test --locked --no-default-features --doc`（目前 0 个文档测试）和
`cargo clippy --locked --no-default-features --all-targets` 均退出 0。
Clippy 仍有既有警告，日志为 `clippy-final.log`，不宣称警告清零。release 构建和上述原生场景通过；
本轮未提交/推送，因此没有新的 CI 结果。

首轮核心 950 项有两项编辑器等待超时（948 通过）；无并行构建的定向运行也有复现，不能仅
归因于并行负载。原版对照、当前不重新编译的定向运行及最终完整套件通过。未放宽时限、
忽略或删除测试，早先失败记录仍保留；具体运行目录和未确定的时序问题见
[源码对照记录](REBASED-REFERENCE.md#fetch--update--push-速度对照2026-10-29)。
