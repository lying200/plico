# Plico

Windows 优先、低内存的 Git 审阅客户端，支持 Git Log/Graph、Diff、文件历史和基础 Git 操作，以及文件编辑与自动保存。

本目录为从 gitui 工作目录复制的独立初始仓库，版本 0.1.0 预览。Cargo 包和主程序名为 `plico`；构建：`devenv shell -- cargo build --release`，Windows 交叉构建：`devenv shell -- cargo xwin build --release --target x86_64-pc-windows-msvc`。

兼容性保留：GitReview 用户配置目录、gitui-askpass / gitui-editor 内部辅助程序名称。原工作目录及历史验证产物留在 `../gitui`，本仓库不复制构建缓存、测试截图、便携包或旧 Git 历史。历史记录中的旧 crate 名、路径和构建哈希描述的是当时版本。

第一版已冻结功能范围，已知差异与验证边界见 PLAN.md 的封版决定。以下保留既有项目说明和开发记录。

---

# Plico

Windows-first, low-memory Git review desktop built with Rust, GPUI and GPUI Kit. The interface follows IntelliJ's branch / log / files / diff arrangement. Development is in progress; see [PLAN.md](PLAN.md) for the complete scope and [GPUI-ASSESSMENT.md](GPUI-ASSESSMENT.md) for the framework assessment.

## Development

The Nix/devenv environment pins Rust and includes the Windows cross-build toolchain. Git must be installed on the machine running the application.

```sh
devenv shell
cargo test --no-default-features
cargo xwin build --release --target x86_64-pc-windows-msvc
```

The Windows executable is `target/x86_64-pc-windows-msvc/release/gitui-review.exe`. Run it with a local repository path, or use **Open…** inside the application. Cross-building downloads the Microsoft SDK/CRT through cargo-xwin. `devenv.lock` and `Cargo.lock` pin the environment and dependencies.

```powershell
.\gitui-review.exe C:\code\my-repository
```

## Windows preview package

After the Windows release build, run:

```sh
devenv shell -- python3 scripts/package-windows.py dist/Plico-windows-x64-preview.zip
```

The script packages the existing application, AskPass and editor helper executables, Chinese usage instructions, asset attribution, available dependency license files and a SHA-256/version manifest. It refuses to overwrite an existing archive. Extract the whole directory and run `Plico.exe`; Git for Windows must be on PATH. This is a development preview, not a completed release acceptance gate. Dependencies are recorded from locked Windows-target Cargo metadata; the list also includes resolved build dependencies.

The initial archive (11,232,246 bytes) was extracted with Windows Expand-Archive, its executable hash checked against the manifest, then started natively against the six-commit fixture. History, graph and selected-file diff rendered successfully; evidence is in `artifacts/portable-package`. The test window was closed afterward.

The current local preview is `dist/Plico-windows-x64-preview-20261018-loading-feedback.zip`, with a Rebased-style Push tree and tag options, a split Push / Force Push button, target-specific confirmation and explicit force-with-lease. The preceding startup preview includes Plico branding, localized commit times, project windows, a compact status bar, themed graph-arrow hover outlines and the actual HEAD ring marker. It also includes the file-history graph and arrow-navigation update. The preceding graph-arrows build adds direction-aware graph-arrow navigation and target tooltips, halves graph segment storage while preserving all geometry and adds the per-project Long Edges toggle, reuses a compact loaded graph model without new Git queries and skips offscreen graph paths. It uses Rebased-style per-row graph spacing and default long-edge arrows, keeps author/date columns visible, and shares repeated geometry for wide histories. It fixes project-avatar initials with the Rebased Mono semibold face and explicit insets, keeps Log panes within narrow windows and provides filter overflow with working child editors. It adopts the Windows system UI font, shares popup border styling and fixes duplicate recent repositories caused by ordinary/verbatim path spellings. It aligns the project dropdown and branch-state icons with a live Rebased reference and adds the branch-menu **Commit…** entry, which opens the existing Commit tool window, focuses its message and preserves drafts. It also fixes native Windows clipboard ownership and retries brief acquisition contention, following Rebased’s Windows policy. It includes Rebased default Project name filtering, Project refresh/retry state restoration, cancellation of stale file previews and file/folder error recovery, alongside optional previews, shared file/Diff tab ordering, directory pagination, Commit / Amend, Squash and Compare Versions. Two-parent file-history merges now use a readonly three-column view with the merged result in the center, aligned source lines, per-column copying and source navigation. File history groups the commit list and resizable details above the Diff, with complete author/committer metadata, references and containing branches for up to 50 selected commits. This UI/UX update adds consistent Rebased icons, individual filter resets, contextual checkout and tab-close menus, details scrollbars, recent repositories and read-only file syntax highlighting. See the current workflow below; the initial-package evidence above describes an older build.

Previously verified Windows measurements for clipboard preview build `60569835` (these are not measurements of the newer toolbar build) (10 steady samples per stage):

| Scenario | Application Private Bytes | Job peak, including Git |
| --- | --- | --- |
| Empty welcome window | 53.20 MiB | 55.56 MiB |
| 10,000 loaded commits, 32 graph lanes, 975 KB file Diff | 120.44–128.01 MiB | 184.84 MiB |
| 100 files, 20 refreshes, 10 repository replacements | 92.09–93.13 MiB at the final small file | 168.81 MiB |

The continuous review run loaded 1,000 commits and verified 229 complete file copies. GPU memory is recorded separately and excluded from these CPU figures. These bounded native runs pass the applicable 80/150/250 MiB budgets; they do not establish cold-cache startup, indefinite leak freedom, or complete release acceptance. Evidence and remaining gates are recorded in PLAN.md.

## Current workflow (2026-10-17)

- Choose **Log** for committed changes. The left rail opens **Local Changes** (the Commit tool window), **Project**, or **Saved Changes**, one at a time. Project opens files read-only; Saved Changes reviews stashes and offers explicit Apply / Pop. The **Text or hash** field and Branch / User / Date / Paths filters query history; **Ctrl+F** focuses the text field. These filters are not limited to the currently loaded rows.
- The Log branch tree filters the history being viewed; selecting there does not check out a branch. Clear the Branch filter to return to all branches. The separate branch menu in the top bar offers branch actions, **Commit…** and **Checkout Tag or Revision…**. Log commit context menus also offer revision checkout, which may leave HEAD detached.
- Select a commit and file; double-click a file, press **Enter** in the file list or **Ctrl+D** to open its Diff. Independent Diff tabs are the default and show full text context within the file-size limits. Settings can switch to bottom preview. **Esc** closes the active Diff tab; **F7 / Shift+F7** navigate differences. Merge commits compare against their first parent.
- **Ctrl+F5** refreshes local state; **Ctrl+Shift+C** copies the selected commit ID. The right-hand details panel shows commit metadata and containing references. Graph divergence alone does not establish a conflict; Local Changes reports actual index conflicts.
- Use the top-left main menu → **Git** for **Fetch**, **Pull**, **Push** and **Update Project**. Update Project and Push also have top-bar icons. Update Project follows the current tracking branch; explicit Pull allows remote/branch and option selection. Push previews its source, target and outgoing commits.
- During an unfinished rebase, the top bar offers **Continue Rebase** and **Abort Rebase**. Resolve and stage conflicts with an external Git tool first; continuing can open the commit-message editor. Abort requires confirmation. Cancelling an operation does not promise rollback of changes Git already made.
- In the Commit tool window, file selection controls the preview while checkboxes control what is committed. Enter a message and choose **Commit** or **Commit and Push**; **Amend last commit** also supports changing only the last message. This is file-level selection, not an interactive hunk-staging editor. Commit and Push opens the Push flow only after a successful commit.
- **Ctrl-click / Shift-click** select multiple Log commits. The context menu provides **Squash Commits…** when applicable, with a message editor and repository-state checks. Successful squash offers **Undo Squash**, subject to fresh state checks; conflicts use the existing Continue / Abort flow after external resolution. After resolving saved Squash changes externally, Saved Changes offers **Already handled externally…** to clear only the application recovery record. Confirmation preserves the stash and files and refuses stale records or unresolved conflicts. Non-regular or oversized records still require external inspection.
- Select exactly two Log commits and choose **Compare Versions** to compare their trees directly (not their merge base). The Changes side panel selects its first file; Enter, double-click or Show Diff opens a reusable full-file preview. Once open, tree selection and Diff file navigation stay linked. Esc closes that preview and returns focus to the tree. Closing the comparison panel also closes its preview.
- **Settings → Editor Tabs → Enable preview tab** is off by default. When enabled, selecting a Project file reuses one italic preview without taking focus from the tree. Enter or double-click keeps the file open as a regular tab. Ordinary file tabs retain the active text selection when switching or reloading; inactive file bodies are released. Esc clears selection in ordinary files; Ctrl+F4 closes the focused file tab. Diff keeps its own Esc return behavior. File and Diff tabs share insertion and left-neighbor close order.
- Project directories load in pages of up to 256 entries; choose **Load more files…** or press Enter / Right on that row. Home / End navigate the loaded tree. Folding a directory releases its loaded descendants; refreshing restores expanded directories and their loaded range. The tree keeps at most 16,384 entries; collapse other folders to free room. If the directory changes during pagination, refresh it before continuing.
- Failed Diff reads show a short message with **Reload diff** and **Copy error details**. Other operation errors appear in the bottom status area with details and dismissal. Reloading a Diff does not repeat a Git write operation.
- The settings gear opens a dialog with **Follow system** (default), **中文** and **English**, plus Diff presentation preferences. On Windows, Chinese UI-language variants select the Simplified Chinese translation; other languages use English. Apply / OK persist preferences; Cancel discards unapplied changes.

- In a read-only Project file, right-click **Show History** to open a closable file-history tab, or **Show History for Selection…** for an independent block-history window. Selection history follows the block across revisions and renames, includes local changes, and defaults to **Changes only**; with no selection it uses the caret line. Revision content loads on demand. File history starts at 100 entries, supports loading more up to 1,000, and indicates remaining history.
- The pencil button / **F4** in a Diff or history preview jumps to the current working file and maps the selected historical line to its current location. Source files remain read-only. Returning to a file-history tab restores the selected revision. Selection-history windows retain their originating repository; navigation cannot cross into a subsequently opened repository.

Browsing remains read-only. Explicit Fetch, Pull, Update Project, Push, checkout, Commit / Amend, Squash / Undo and stash Apply / Pop can modify repository state; ordinary refresh does not fetch. Git work runs in background tasks with bounded output and cancellation. Only the active file's Diff is loaded on demand. This is a review-focused client with basic write operations; it does not include general file editing or a conflict-resolution editor. Keep **gitui-askpass.exe** and **gitui-editor.exe** beside the application. Credential and editor prompts are operation-scoped; this application does not provide a password vault. Full authentication, DPI, long-session and release acceptance remain tracked in [PLAN.md](PLAN.md).

## Validation

`tests/repository.rs` creates disposable real Git repositories, including merges, conflicts, Unicode paths and linked worktrees. The layout engine has a pagination consistency test.

```sh
python3 scripts/create-fixture.py /path/to/new/disposable-repo
```

On Windows, `scripts/windows-smoke.ps1` starts the release executable, captures its window and records a startup memory sample. `scripts/capture-window.ps1` captures a particular process window and optionally sends keyboard input. These are development tools; startup samples do not establish the full memory acceptance criteria. Captures with entirely black samples fail validation and are saved as `.invalid.png`; a non-black image still requires content review.

`scripts/windows-resize.ps1` exercises a running window at 1200×800, 1800×1100 and 1200×800 physical pixels, saving screenshots and process samples. `scripts/windows-memory.ps1` records steady samples, direct child processes and separate GPU counters.

Windows observations on 2026-10-15: closing the other test processes left the original full-size empty repository at about 112 MiB Private Bytes. Lazily allocating GPUI's path surfaces reduced the same empty-repository startup sample to about 53 MiB. A six-commit graph and diff rendered correctly at about 119 MiB on startup. Resize screenshots retained graph and diff rendering, but Private Bytes rose to about 181 MiB and remained there after shrinking; a second resize cycle stayed around 181 MiB. Restricting path surfaces to their clipped coverage subsequently reduced the same six-commit startup to about 68 MiB and the post-resize sample to about 79 MiB; the scripted 150 MiB post-resize check now passes. This reduces allocation demand, while the cause of driver/runtime retention has not been isolated. The empty-window improvement does **not** establish overall memory acceptance. These are process samples, not combined transient peaks or GPU-inclusive totals.

The Windows UI now follows the system message-box font, as Rebased does by default; explicit details-width measurement uses the same face. Bundled Inter remains available on other platforms, while code uses JetBrains Mono and icons use JetBrains SVG assets; provenance and licenses are in `assets/fonts` and `assets/icons`. The reference is native Windows IntelliJ IDEA 2026.2.3 with Islands Dark (`artifacts/intellij-islands-2026.2.3.png`). Its Git log was unavailable because IDEA reported Git version detection “no response”, although Windows Git runs directly; the screenshot currently validates the shell, not the populated Git view. The application still needs closer layout and interaction matching.

A Windows cold-launch capture after adding the DiffState observer confirms that the split diff fills the available width without a click or resize. The previous capture reproduced the half-width initial layout. The six-commit fixture startup measured 68.05 MiB Private Bytes and 77.19 MiB Working Set; this is not a peak or GPU-inclusive measurement (`artifacts/islands-pass3`).

The Git tool-window header, tab switching and Enter-to-search were verified on Windows (`artifacts/islands-tabs2` through `islands-tabs4`). The final capture verifies Ctrl+F and hash-prefix search after activating the Log tab. An attempted no-click startup shortcut test did not reach the app because Windows rejected foreground activation; it does not establish an application shortcut defect. The capture script now checks the actual foreground window before sending keys and fails rather than typing into a different application. Initial no-click shortcut behavior remains unverified.

Branch search was validated with 6 unit tests and 5 real-Git integration tests on both Linux and native Windows. Windows captures (`artifacts/islands-branch-search`) show `main` narrowing the tree without changing the six-commit graph, selecting local `main` loading its five reachable commits, and clearing the query restoring the full tree while retaining the selected branch.

Commit details now separate Git metadata from the message using NUL-delimited fields. Linux and Windows tests cover Unicode and a multiline message containing a misleading `Author:` line. Native Windows checks verified the short-ID button copies the full ID, scrolling reveals the complete parent ID, and Ctrl+A / Ctrl+C in the details copies the complete message and metadata (`artifacts/islands-details3`). The text viewport is bounded to prevent content-sized layout from clipping its scroll extent; details occupy at most half the file pane, capped at 180 logical pixels.

Ref-decoration validation passes 6 unit and 6 real-Git integration tests on both Linux and Windows, covering same-named local/remote refs, comma-containing names, annotated/lightweight tags, non-commit tags and detached HEAD. The Windows capture shows all four decoration types (`artifacts/islands-refs`); the annotated `review-v1` tag was added only to the disposable validation fixture. Decoration colors are an initial differentiation pass and have not been verified as a pixel match to IDEA.

The history table now scrolls horizontally with its header while retaining virtualized vertical rows. Each axis is restricted to its own gesture; Windows captures verify horizontal input preserves the top commit and vertical input preserves header x positions (`artifacts/wide-merge-history3`). `scripts/create-wide-fixture.py` creates 1,026 commits with a 32-parent merge to exercise 32 graph lanes; ordinary disconnected branch tips did not force a wide graph under topological ordering. The final responsive layout passed 1200×800 → 1800×1100 → 1200×800 resize checks, with final Private Bytes 117.46 MiB (`artifacts/wide-merge-history4`). This is a process sample, not peak/GPU-inclusive acceptance. Final small-window wheel automation was rejected by the foreground-window guard, so that interaction was not verified on the last build.

## Current limitations

With bounded path surfaces, the generated 10,000-commit repository plus the approximately 1 MiB / 13,000-line `review.rs` diff measured 81.98–82.08 MiB Private Bytes over ten steady samples, with 79.29–79.34 MiB Working Set and no live Git child at sample times. Separate GPU counters were 56,471,552 bytes dedicated and 3,284,992 bytes shared. The screenshot confirms the history count and selected large diff. This is a linear-history steady-state observation, not proof of transient peak, wide-graph or long-session budgets.

The initial implementation still requires visual/interaction validation and the full performance acceptance suite. History loads up to 10,000 commits per window; **Older window** replaces it with the next batch, carrying only bounded graph frontiers. **Back to latest** opens a fresh snapshot. Refresh keeps the current window offset, reconstructs graph lanes against fresh roots in bounded batches, and restores the selected commit and top visible commit when they remain in that window. The selected file is restored by path and comparison area. **Newer window** returns to the preceding 10,000-commit window using the same fixed history roots and reconstructing its graph frontier in bounded batches. Large offsets can require Git to traverse earlier history again, under the existing cancellation/timeout limits. Graph layout rejects excessive frontier/parent complexity before expensive lane searches (a conservative 512-lane bound), and rejects windows exceeding 500,000 segments. Independent Diff tabs now load full text context within bounded file-size limits; bottom preview remains available. Large output and unsupported encodings produce visible errors. GPU memory, high-DPI behavior and long-session stability have not yet been accepted. This is not yet a completed release.

### 本地变更分组与空状态验证（2026-10-15）

本地文件列表按 Conflict、Unstaged、Staged、Untracked 分组，显示数量并可点击折叠。虚拟列表保存文件索引；上下键只选择展开组内的文件，跨组跳过标题，选择仍对应原始 Git 变更记录。分组不触发文件内容读取，diff 仍按需加载。

Windows release 构建通过。实际宿主机窗口验证了未暂存组折叠、选择暂存 README 显示 HEAD → Index，以及向下键跨组选择未跟踪文件并显示新文件预览。截图见 `artifacts/local-groups/{local,collapsed,staged,keyboard-down}.png`。小仓库启动 Private Bytes 68.43 MiB，属于单次启动采样，不是完整性能验收。冲突组和全部折叠后的键盘行为仍需专门运行验证。

此前的空状态验证截图已保存至 `artifacts/empty-states-merge`、`artifacts/empty-states-unborn`；比较基准截图见 `artifacts/local-comparisons`。分别覆盖多父提交相对第一父无变更、干净工作区、无首个提交，以及 staged / unstaged / untracked 比较栏。

### 实际 index 冲突与刷新验证（2026-10-15）

`scripts/create-conflict-fixture.py NEW_DIRECTORY` 创建独立测试仓库，拒绝覆盖已有目录，通过两个分支修改同一行产生真实 merge 冲突。Windows 界面显示 `Conflict 1` 分组、红色 `UU review.txt` 和未合并 index 提示，截图见 `artifacts/conflict-groups/conflict.png`。从外部 Windows Git 执行 `merge --abort` 后，点击应用刷新按钮，冲突组和旧提示均清除，显示干净工作区（`refreshed.png`）。应用自身没有执行合并或取消操作。

扩展 `conflicts_are_index_states_not_graph_guesses` 测试，覆盖普通分叉时工作区干净、合并后只有一条冲突路径、冲突不能当普通 diff 读取、外部取消后重新读取为空；该测试在 Linux 和 Windows 均通过。自动化键盘尝试被前台保护拒绝，没有据此认定全部折叠时的键盘行为已验证。

### 浅克隆边界（2026-10-15）

修复浅克隆边界被当成根提交、相对空树产生误导性新增文件的问题。通过 Git 的 shallow 文件识别具体边界（读取限制 8 MiB），提交行显示 `Shallow boundary`；提交说明可读，但文件比较明确显示父历史不可用，并建议在外部补齐历史后刷新。普通根提交仍使用空树比较。读取层拒绝对边界提交生成 patch。

Linux 的 6 个单元测试和 7 个仓库集成测试均通过；Windows 同样通过 6 + 7 个测试。新增真实浅克隆用例验证边界、正常后续提交、外部 unshallow 后移除标记及真正根提交的空树比较。Windows release 实际选中边界的截图为 `artifacts/shallow-boundary/boundary.png`，正常后续提交的 diff 见同目录 `window.png`。外部操作与读取之间的竞态仍需单独验证。

### 重复浏览与刷新诊断（2026-10-15）

修复刷新重入导致选中文件丢失：读取历史／文件列表期间再次刷新，现在保留正在进行的请求，避免空列表覆盖待恢复的文件选择。Windows 修复前截图回到 `checkpoint.txt`，修复后 20 次连续刷新仍选中 `review.rs`；证据分别在 `artifacts/stress-before-refresh-fix` 和 `artifacts/stress-refresh-fixed`。

`windows-stress.ps1` 现在向目标 HWND 发送上下键和刷新点击，避免全局 SendKeys 误发其他窗口，并保存中途截图。调用时需先聚焦基准仓库的文件列表并选中第二个文件，提供物理窗口内刷新坐标（本机 `-RefreshX 2130 -RefreshY 87`）。本轮加载 10,000 条历史，交替两个文件 100 次，其中大 diff 约 1 MiB，然后点击刷新 20 次。应用 Private Bytes 初始 83.70 MiB，切换采样最大 91.30 MiB，刷新采样最大 92.96 MiB，结束后 10 个样本为 84.65 MiB。后续独立采样 84.66 MiB / Working Set 87.17 MiB，直接子进程采样为 0；GPU dedicated 55,463,936 bytes，shared 3,411,968 bytes，分别报告，不相加作为进程 Private Bytes。

这些是采样最大值，不是真实瞬时峰值；本轮两文件重复切换不能替代计划中的 100 个不同文件、10 次仓库打开关闭和长时间性能验收。构建及格式检查通过；本轮未修改 Git 读取层。

### 100 个不同文件的浏览诊断（2026-10-15）

`python3 scripts/create-benchmark.py NEW_DIRECTORY 10000 100` 现在可生成 10,000 提交、tip 恰好 100 个变更文件的独立仓库：checkpoint、98 个各 1,000 行的小文件、约 1 MiB 的 review.rs。`windows-stress.ps1 -Sequential` 从聚焦的第一个文件出发进行 100 次顺序选择，然后点击刷新 20 次；中途截图验证第 50 项为 file-048.rs，第 100 项为 review.rs。输入间隔 250 ms，截图仅覆盖检查点，未记录每个异步 patch 的完成时间。

此测试发现刷新清空列表后虚拟滚动位置回到顶部，尽管 diff 保留了末尾文件。修复为新列表恢复选中项时使其重新可见，最终 Windows 截图 `artifacts/stress-100files-fixed/final.png` 显示 review.rs 保持选中且位于视口内。修复前证据在 `artifacts/stress-100files-before-scroll-fix`。

复测应用 Private Bytes 初始 72.18 MiB，顺序浏览采样最大 91.65 MiB，刷新采样最大 98.84 MiB，结束保留大 diff 时收敛至 87.89 MiB。重新选择原提交并回到 checkpoint 后，独立 10 次采样最终稳定在 79.65 MiB（Working Set 91.44 MiB），直接子进程采样为 0；GPU dedicated 55,468,032 bytes / shared 4,300,800 bytes。回到相同内容仍比初始高 7.47 MiB，单轮不能证明无长期增长或认定泄漏。真实瞬时进程树峰值、10 次仓库打开关闭及长时间循环仍待验收。Windows release 构建、Rust 格式检查和基准生成器语法检查通过。

### 原生文件夹选择与重复切换仓库（2026-10-15）

修复打开另一仓库时旧 graph／提交残留，以及沿用旧搜索、Local Changes 模式、展开 diff、分页边界和滚动状态的问题。确认选择后清除这些仓库相关状态，再启动新请求；取消选择不修改当前仓库。旧后台请求继续通过取消令牌及 generation 隔离。

新增 `windows-pick-folder.ps1`，只操作指定测试进程拥有的原生文件夹对话框，并校验输入框／按钮；不向全局前台发送输入。`windows-reopen.ps1` 通过真实 Open 按钮和文件夹选择器，在两个测试仓库间交替替换 10 次，最后取消一次选择。Windows 实测从六提交仓库切到空仓库后旧 graph 和 diff 清空；随后一万提交仓库（每次按正常打开流程加载首批 1,000 条）与空仓库交替打开。最后的大仓库与取消后的空仓库截图已人工检查，全部轮次截图和数据在 `artifacts/reopen`。这不是十次重启进程，也未单独验证快速重叠打开请求。

10 次切换的 Private Bytes 采样范围 82.94–87.34 MiB，最后空仓库 83.58 MiB / Working Set 126.19 MiB。之前首次调用原生选择器并切到空仓库约 80.16 MiB；不能把选择器加载后的占用与未调用选择器的空窗口启动样本混用。此采样不含瞬时峰值、子进程和 GPU，也不证明长时间无泄漏。Windows release 构建与格式检查通过。

### Islands 文件状态色与分支图标（2026-10-15）

对照 [JetBrains Git Log 文档](https://www.jetbrains.com/help/idea/log-tab.html)的当前截图，普通分支不应使用标签形图标。分支树现使用同一固定 upstream commit 的 New UI `expui/general/vcs_dark.svg`（空心圆分叉），标签装饰继续使用标签图标；来源及许可记录在 `assets/icons/SOURCE.txt`。

文件状态色直接取自本机 IDEA 2026.2.3 的 `themes/islands/IslandSchemeDark.xml`：新增／复制 #73BD79，修改／重命名 #70AEFF，删除 #6F737A，未跟踪 #E88F89，冲突 #DE6A66。应用到状态字母及文件路径；导出的原始色值见 `artifacts/islands-file-status-colors.json`。Windows release 构建通过，实际截图 `artifacts/islands-status/local.png` 验证蓝色修改文件、红色未跟踪文件以及新的分支图标。本轮未单独截图验证新增、删除、重命名及冲突各色。

官方 Git Log 示例为浅色并含说明标注，用来核对结构和图标；不能拿它证明整个 Islands Dark 已像素级还原。文件目录树、紧凑提交详情及 Diff Viewer 的细节仍需继续对齐。

### 紧凑提交详情（2026-10-15）

提交说明首行加粗；相同作者／提交者和相同日期不重复展示。作者或日期不同时保留对应提交者和提交时间。普通根提交显示 Root commit，浅克隆仍明确说明父历史缺失；完整父 ID 保留。无选中提交时不再显示空详情面板。

Windows release 构建通过。短说明现在能同时显示说明、作者、时间和完整父 ID（`artifacts/details-compact/window.png`）。独立测试提交具有不同作者与提交者、不同日期、多行说明、`<cache> &` 字符及正文中的 `Author:` 行：真实 Ctrl+A / Ctrl+C 的剪贴板校验确认完整说明、两组身份／日期和根提交标记均保留；滚轮可查看下方元数据（同目录 scrolled.png）。此轮为展示层修改，未改 Git 元数据解析。

### 变更文件目录树（2026-10-15）

提交文件与各本地变更分组内现在按目录展示，使用 JetBrains 文件夹图标、缩进、子文件总数和折叠箭头；文件行只显示文件名，diff 保留完整路径。目录只由 Git 变更路径构建，不扫描磁盘、不预读 patch；列表仍虚拟化。`file_tree` 独立模块按目录在前排序，保留原始 change 索引，迭代展开避免深目录递归栈；本地分组的折叠键包含 Area，不混用同路径的暂存／未暂存条目。

Linux、Windows 均通过 8 个单元测试及 7 个真实 Git 集成测试，新增覆盖嵌套路径、相似前缀目录、中文及特殊字符、折叠后身份／数量以及重复路径的独立索引。Windows 截图 `artifacts/file-tree` 验证提交 src/cache.rs、Local Changes 目录分组、折叠 .idea 后选择 src/config.rs 显示对应内容，以及向上键跳过折叠目录回到暂存 README。键盘验证使用指定 HWND 的键消息；全局 SendKeys 尝试被前台保护拒绝。当前保留完整目录层级，尚未压缩连续单子目录，也未新增文件类型图标。

### Rebased 原生窗口对照（2026-10-15）

用户启动的 Windows Rebased 1.1.20 已打开同一 GitReview-fixture，六条提交及分支图正常显示（`artifacts/rebased-layout/reference.png`）。按实图移除日志额外的 Graph / Subject / Hash / Author / Date 表头和逐行 Hash 列，分支装饰移至标题右侧；完整提交 ID 继续保留在详情及复制功能中。Windows release 构建与格式检查通过，原生截图 `window.png` 确认标题、标签、作者、日期及当前文件 diff 正常显示。启动采样 Private Bytes 75.25 MiB，仅为此小仓库的一次采样。

参考窗口的自动点击尚未验证提交详情联动；真实按键／点击受前台保护阻止，定向窗口消息只观察到 hover。因此本次参考仅支持 Log 静态布局，不代表已完成 diff 交互或像素级还原。Rebased 与本应用的拓扑合法排序也存在差异，需继续核对排序策略；工具栏、标签形态、行高和 diff 样式仍待对齐。

### Rebased 源码参照

参考仓库 https://github.com/DetachHead/rebased 已以 sparse checkout 拉到 `/tmp/gitui-rebased-reference`，固定 commit `4aec35256d765c217aa0d662b82ac4e860c396fd`（master，不能直接视为已安装 1.1.20 的准确源码版本）。后续核对以下真实入口，截图用于验证最终表现：

- `plugins/git4idea/backend/src/log/GitLogProvider.kt`：Git 历史读取；`readAllHashes` 使用 `--date-order`。现已核对默认图层为 Normal，并改为 `--date-order`；IntelliSort/Bek 属于独立排序功能，尚未移植。
- `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/table/VcsLogGraphTable.java`：日志表格、不可见但可调整列宽的表头、选择与键盘操作。当前移除可见表头，但尚未实现其列宽拖动行为。
- `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/render/GraphCommitCellRenderer.kt`：标题与 refs 对齐、标签宽度预算及字体决定的行高。上游支持左右对齐，不是固定只能右对齐；compact refs 宽度上限为可用文本区域的三分之一，并受标题剩余空间约束。当前右侧标签布局尚未实现该预算。
- `platform/vcs-log/impl/src/com/intellij/vcs/log/ui/frame/MainFrame.java`：表格／文件／详情／diff 分割；文件与详情垂直分割、日志与右侧区域水平分割，初始比例均为 0.7，并持久化比例。
- `platform/diff-impl/src/com/intellij/diff/tools/simple/SimpleDiffViewer.java`：双栏 diff、同步滚动、重新比较时保持滚动位置、change 导航。

移植以可观察行为和尺寸规则为依据，用 Rust/GPUI 实现；保持当前固定历史窗口、取消过期任务、按文件加载 patch 的资源边界。此处为源码核对记录，不代表上述行为均已完成。

### Compact refs 按源码调整（2026-10-15）

根据固定 Rebased commit 的 `GitRefManager.groupForTable`、`SimpleRefGroup.buildGroups/getColors`、`GraphCommitCellRenderer.getAvailableWidth`，将日志内独立彩色标签改为紧凑的一组：优先显示当前分支名，同类 refs 最多两个叠放图标，组宽不超过标题与标签区域的三分之一，悬停显示全部引用名称。detached HEAD 明确显示 `HEAD (detached)`。通过 `symbolic-ref --quiet HEAD` 取得完整 symbolic ref 后去除 `refs/heads/`，避免 `--short` 在分支与 tag 同名时产生歧义。没有加载额外提交或 patch，只增加每次 snapshot 的一次有界 Git 命令及一个当前分支字符串。

Linux 8 单元 + 7 集成测试通过；Windows refs 集成测试通过，新增验证当前分支、detached HEAD 和同名 tag。Windows 截图 `artifacts/compact-refs/window.png`、`tooltip.png` 验证长 refs 合并、当前分支优先及完整悬停列表；三次 resize 正常，窄窗口继续使用横向滚动，返回 1200×800 时 Private Bytes 92.03 MiB（本轮小仓库采样，不是完整性能验收）。

与上游尚有差异：当前三分之一为 CSS 布局上限，尚未按标题实测文字宽度进一步收紧；尚未实现 tracked local/remote 合并名称、全部 ref 排序策略及上游自绘图标的精确像素形状。文件／详情仍是原有固定高度方案，源码中的可拖动 70/30 分割及持久化待实现。

### 可调整的文件／详情分割（2026-10-15）

按 Rebased `MainFrame.java` 的文件列表／提交详情垂直分割，替换原来的最多 180px 固定详情区。默认比例 70/30，拖动中仅调整视图；切换提交和 Local Changes 往返时保留独立 `ResizableState`。详情继续使用有界滚动视口。拖动结束将比例保存到 Windows `%LOCALAPPDATA%\GitReview\files-details-ratio`（Linux 为 XDG_CONFIG_HOME 或 ~/.config 下的同名路径），下次启动恢复；读取最多 65 字节，非法／超长内容回到默认值，偏好值限制在 0.1–0.9。

Windows 定向 HWND 拖动脚本 `scripts/windows-drag.ps1` 验证从默认位置拖到约 41/59；切换到 merge 提交后能看到两个父 ID，Local Changes 往返保留分割高度。关闭并重新启动后恢复保存的 0.412791，截图见 `artifacts/details-split/dragged.png`、`back-log-ready.png`、`restarted.png`。此轮没有复测长详情的滚轮交互。Linux 9 单元 + 7 集成测试通过；Windows 新增偏好解析测试通过。跨次启动持久化仅覆盖文件／详情分割，其他分割与窗口尺寸尚未统一保存。测试还观察到 Local Changes 往返后横向导航分割宽度变化，后续需独立修复。

### 标签页切换保持导航宽度（2026-10-15）

修复 Log → Local Changes → Log 后分隔线漂移：两种布局原来共用按索引管理的 ResizableState，Log 三个面板切换为 Local Changes 两个面板时，history/files 的尺寸身份被混用。现在为两种布局分别持有状态；Local Changes 的文件区采用弹性剩余空间，避免仅剩两个固定初始宽度面板时把分支栏拉到上限。没有新增提交、文件或 patch 缓存。

新增真实 Windows 回归脚本 `scripts/windows-navigation-layout.ps1`，通过目标 HWND 切换标签页，并从截图中识别两条竖直边界，断言往返偏差不超过 1px。该脚本针对当前 1440×900 逻辑窗口／150% DPI 测试布局，控件点击坐标不能直接用于任意缩放。旧版连续两次失败（323/1679 → 339/1767）；最终版本两次保持 323/1679，手动将文件区边界拖到 1500 后再往返两次也保持 323/1500。证据在 `artifacts/navigation-layout`，同时检查 Local Changes 分支栏宽度已恢复正常。Windows release 构建、格式检查通过。本轮没有改 Git 读取逻辑，未重复全量数据层测试。

### 长历史窗口往返验证（2026-10-15）

补齐 Newer window 的 Windows 原生验证：12,000 条线性提交，从最新 10,000 条（offset 0，Commit 12000）进入旧窗口首批 1,000 条（offset 10000，Commit 02000），再回到同一快照的最新 10,000 条。`scripts/windows-history-roundtrip.ps1` 在独立 benchmark 仓库创建指向一个新提交的临时分支，使用实际 UI“复制提交 ID”按钮检查每次选中对象，最后删除临时 ref。四轮八次切换全部匹配旧快照 ID，新 refs 未改变窗口边界。该脚本要求 12,000 条测试仓库、默认 150% DPI 窗口和已加载的 10,000 条历史，不能直接对任意用户仓库运行。

回到大窗口后的 Private Bytes 采样依次 73.19、73.38、74.05、74.09 MiB；旧窗口为 67.03–68.22 MiB。短测存在约 0.90 MiB 变化，不足以证明长期无增长；未统计 Git 子进程／GPU／真实瞬时峰值。证据 `artifacts/history-roundtrip`。当前窗口替换不保存旧 commits/rows；反向窗口只保留 roots 和有界 graph frontier。Windows `branch_snapshot_paging_merge_and_worktree` 再次通过，覆盖 refs 移动后的固定快照、merge graph 一次布局与窗口重建一致性。

另移除垂直滚动条仍为旧表头保留的 28px 顶部偏移。最终构建启动及一轮窗口往返再次通过（同目录 final）；本轮尝试拖动滚动条未观察到滚动，真实滚轮输入被前台保护拒绝，故不将滚动交互视为本轮已验证。

### 目录展开状态的内存边界（2026-10-15）

完成提交／本地文件列表加载后，仅保留该列表中仍存在的 `(Area, directory)` 折叠记录，释放旧 HashSet 容量。临时加载空列表不触发清理；同目录刷新保留选择，已经离开当前列表的目录再次出现时默认展开。嵌套目录即使被折叠父目录隐藏也保留状态，不混用 staged/unstaged 等区域。此边界避免浏览不同提交并折叠不同目录时累积历史路径。

回归测试从 1,000 个旧目录与共享嵌套目录切换到新列表，检查过期路径、区域隔离、父子状态以及旧集合容量释放；Linux 10 单元 + 7 集成测试通过，Windows release 构建通过。这是数据结构边界验证，尚未测量该改动的进程内存收益。滚动交互按用户要求留给手测，不再开展滚动自动化排查。

### 跟踪分支的紧凑标题（2026-10-15）

按固定 Rebased 源码 `GitRefManager.createTrackedGroup/groupForTable` 接入 `origin & main` 标题：本地分支与远端 ref 必须位于同一提交，先匹配实际 upstream（支持本地／远端分支名不同），再匹配已配置 remote 下的同名分支。未进入跟踪组的当前分支优先显示；其余跟踪组按当前分支、main/master、普通本地分支排序。原始 refs 和完整 tooltip 保留；detached HEAD 继续明确标识。

upstream 与 remote 名通过有界、可取消的 Git 读取获得；标题在快照读取时计算，以 Arc 共享给可见行，绘制时不执行 Git、不克隆整张表。标题表覆盖此次读取的 refs，避免追加历史页后前页标签消失。此改动只实现紧凑标题规则：上游在特定情况下保留多个独立组的布局、自然名称排序、detached warning 图标及精确文字宽度测量仍未全部还原。

Linux 12 单元 + 7 集成测试通过，Windows release 构建通过；Windows 原生运行两项标题规则测试及 refs 集成测试通过，覆盖实际 Git upstream 读取、同提交配对、远端不同名、分离 HEAD、同名回退和未配对当前分支优先级。未将此项测试视为像素级 UI 验收。

### 子模块引用对比（2026-10-15）

固定 Git patch 的 `--submodule=short --ignore-submodules=none`，提交文件列表与本地 status 同样显式包含子模块，避免仓库 `diff.submodule=log/diff` 或 `diff.ignoreSubmodules=all` 使输出变成无法按统一 patch 解析的格式或隐藏引用变化。UI 检测 160000 模式并显示“Submodule reference changed”，双栏保留旧／新 commit ID，不递归读取子模块代码。此功能是引用审阅，不包含对子模块内部未提交内容的完整浏览。

新增真实 Git 集成测试覆盖两种 diff.submodule 设置、忽略配置、已提交和 staged gitlink 更新；Linux 12 单元 + 8 集成通过，Windows release 构建通过。Windows 原生窗口使用设置了忽略／log 配置的独立四提交 fixture 验证提示与红绿 commit ID 行均显示，证据 `artifacts/submodule-preview/window.png`。启动采样 Private Bytes 61.89 MiB，仅为小仓库单次采样，不是整体内存门槛验收。

### 短 Diff 铺满双栏视口（2026-10-15）

修复首次打开短 Diff 时仅占窗口约 58% 宽度的问题。实际视口测量是正确的，但组件在绘制期间直接 notify 不会安排下一帧，行宽一直停留在首次文本估算值。将 gpui-component 0.7.3 纳入 vendor，唯一源代码补丁是在尺寸变化时通过 on_next_frame 通知，沿用组件其他布局测量处的模式。未添加 diff 缓存；尺寸不变不请求额外帧。补丁与来源见 vendor/gpui-component/PATCH.md。

Windows 原生同一短子模块 fixture 回归：scripts/windows-check-diff-width.ps1 在旧截图退出 1（右缘 1265/2182），修复后退出 0（2156/2182），双栏分界回到视口中央。截图、检查结果与启动采样见 artifacts/diff-width。该检查针对固定暗色 fixture 的新增行底色，不适用于任意主题或任意代码截图。Windows release 构建通过；临时诊断日志已清理，测试窗口已关闭。此次未测试滚动。

### 双栏版本标题（2026-10-15）

Diff 上方增加两列等宽版本标题：左侧旧路径／父提交，右侧新路径／当前提交；根提交左侧为 Empty tree，merge 标明 First parent，本地分别为 HEAD → Staged 或 Staged → Working tree。长路径截断并提供完整标题 tooltip。未跟踪文件仍为文本预览，不显示两个版本标题；冲突状态也不伪造两方比较。标题属于可见面板，不增加 Git 读取或历史缓存。

Windows release 构建通过，原生截图验证普通父子提交与根提交标题、左右中线对齐以及切换后标题更新，见 artifacts/diff-version-titles。本轮未单独重测 rename、merge、本地分类标题，也未进行滚动测试。标题样式是当前实现，尚未宣称达到 JetBrains 像素级一致。

### 批量解析 tag 目标（2026-10-15）

历史快照读取现在使用 for-each-ref 同次返回的 objecttype、*objecttype 和对象 ID 来识别 tag 目标，替代原来每个 tag 单独执行 rev-parse 的方式。指向 commit 的轻量／注释标签保留；指向 tree/blob 的标签排除；若返回的解引用目标仍为 tag，才回退递归解析。没有加入持久对象缓存，保持现有输出上限与取消路径。

扩展 refs 集成测试覆盖嵌套注释标签、轻量／注释 tree/blob 标签及嵌套 blob 标签。Linux 12 单元 + 8 集成测试通过，Windows 原生 refs 测试及 release 构建通过。对隔离测试开启 Git Trace2：9 个标签、两次快照，逐标签 rev-parse 次数为 0；当前 Git 对嵌套标签也返回最终目标，回退路径未触发。摘要见 artifacts/tag-peeling/trace-summary.json。旧代码对相同数据会逐个解析共 18 次；此处确认的是命令启动数量减少，没有量化启动耗时或峰值内存收益。

### 默认历史排序对齐（2026-10-15）

补齐固定 Rebased 源码的排序链路核对：graph-api 的 PermanentGraph.kt 将 Options.Default 定义为 Base(SortType.Normal)；VcsLogUiPropertiesImpl.kt 在无已保存选项时采用此值；PermanentGraphImpl.kt 的 Normal 使用 BaseController，Bek 才使用 SortedBaseController 与 BekSorter。GitLogProvider.readAllHashes 使用 --date-order。因此本实现从 --topo-order 改为 --date-order，保持子提交先于父提交，同时让两个分支交错的提交按日期显示。不宣称与启用了 IntelliSort 或已保存不同排序设置的实例完全同序。

新增定日期的六提交双分支 merge fixture，期望 merge → b2 → a2 → b1 → a1 → root，并验证三个两条记录窗口重建后的图线与完整布局逐段一致。Linux 12 单元 + 9 集成测试通过；Windows 原生此测试与 branch_snapshot_paging_merge_and_worktree 通过，release 构建通过。无需额外持久图或排序缓存，历史窗口上限保持不变。未做本轮图形截图对比，测试证据只覆盖读取顺序和图布局数据。

### 未跟踪文件按新增 Diff 展示（2026-10-15）

未跟踪文本从组件的单栏 unchanged 参考预览改为 Added 双栏对比，左侧为空，右侧绿色新增行，并显示 Empty file / Untracked working file 标题及新增行数。直接构建 DiffFile，不先生成整份临时 patch；读取上限、二进制拒绝和只读行为保持原有边界。空文件也保留 Added 状态。

Windows release 桌面集成测试 untracked_preview_counts_added_lines_and_handles_empty_files 通过，覆盖中文路径、CRLF/LF 混合、无末尾换行的新增行计数以及空文件状态；本测试不验证剪贴板文本。Windows 原生在 unborn 仓库打开 Local Changes，确认三行未跟踪 Rust 文件显示 +3/-0、右侧新增和版本标题，截图见 artifacts/untracked-added/window.png。测试窗口已关闭。此节更新前文“未跟踪文件仅单栏文本预览”的旧记录。

### 最新版本稳定态内存复测（2026-10-15）

覆盖近期默认日期排序、tag 批量解析、Diff 下一帧宽度修复、双栏标题及未跟踪 Added 展示后的 Windows release。使用 GitReview-benchmark，截图确认 10,000 条提交已加载并选中 975,000 字节／13,000 行的 review.rs Diff。

20 次采样（总观测约 20 秒）：Private Bytes 最小／最大／末次均 83.62 MiB，末次 Working Set 95.47 MiB，采样时 Git 子进程 Private Bytes 为 0。GPU 单独报告：DedicatedUsage 55,459,840 字节（约 52.9 MiB），SharedUsage 3,850,240 字节（约 3.7 MiB），不与 Private Bytes 简单相加。窗口静置观测期间应用累计 CPU 0.515625 秒，折算单核约 2.57%；这不是帧率或持续重绘判定，尚不能据此确认空闲绘制要求。

证据 artifacts/current-memory（原始采样、CPU 记录、实际选择截图）。稳定态样本低于 150 MiB 预算；本轮不证明真实瞬时峰值、冷启动速度、长期无增长或整套发布门槛通过。未进行滚动测试。测试窗口已关闭。

### 文件类型图标（2026-10-15）

文件行以 JetBrains New UI 文件类型图标替代 A/M/?? 状态字母，文件名继续使用 Git 状态颜色；悬浮提示保留完整路径、区域和原始状态。支持 text、Markdown、JSON、YAML、配置、Java、JavaScript、TypeScript、XML，其他扩展名使用通用文件图标（包括暂未引入专用图标的 Rust/Python）。图标来自固定 Rebased commit，来源与 4x intrinsic 尺寸调整记录在 assets/icons/SOURCE.txt；路径与颜色未修改。缓存按十种静态资源复用，不按每个文件生成图片。

Windows release 构建通过，原生截图确认 Markdown、JSON、TypeScript、text 彩色图标和新增文件颜色显示正确，见 artifacts/file-icons/window.png。未逐一验证所有扩展名和 tooltip，未进行滚动测试。测试窗口已关闭。便携预览包随后重新生成以包含本次图标变更。

### 未跟踪文件的父目录链接检查（2026-10-15）

预览前逐段检查仓库相对路径：拒绝绝对路径／父目录跳转、符号链接；Windows 额外检查 FILE_ATTRIBUTE_REPARSE_POINT，覆盖 junction 等重解析点。最终文件仍必须为普通文件且受原有大小上限约束。此检查发生在加载选中文件时，覆盖 status 后目录被替换的常见外部变动。

新增真实文件系统回归：先从 status 得到 nested/review.txt 并正常预览，再将 nested 替换为指向另一个临时目录的链接，确认预览返回明确错误而非外部文本。Linux 使用 symlink，Windows 使用实际 mklink /J junction，二者均通过；同时测试 ../ 路径拒绝。Linux 12 单元 + 10 集成测试、Windows 本项原生测试、release 构建通过。

限制：逐段元数据检查与随后打开不是原子操作，尚不能保证检查过程中持续替换路径的并发边界。PLAN 保留该项未完全验收，不将这次修补描述为完整的无链接遍历文件打开实现。

### 活动 Git 任务的切仓／关闭回归（2026-10-15）

新增 scripts/windows-active-job.ps1：在指定的新目录生成独立 slow git.exe，仅对子测试进程的 PATH 生效。替身写出自己的 PID 并等待 120 秒，测试先确认它确已启动，再通过原生 CloseMainWindow 关闭应用；可选 -SwitchRepository 使用目标 HWND 的 Open 按钮和所属文件夹对话框，切换到第二目录，确认旧子进程已退出、不同 PID 的新任务已启动，再关闭窗口。脚本清理自己的进程并恢复调用进程环境，不替换系统 Git。

Windows 实测：直接关闭时应用与活动子进程均退出；读取中切仓时旧子进程退出，随后关闭时新子进程和应用也退出，均通过。结果 artifacts/active-job/close.json、switch-and-close.json。这是运行中的子进程生命周期回归，使用可控替身保证阻塞窗口；不能代替真实 Git 所有读取阶段、快速选择结果正确性或异常退出测试。应用代码本轮无需修改。

### 仅文件模式变化的验收（2026-10-15）

新增真实 Git 测试通过 update-index --chmod=+x 生成 100644 → 100755，确认 staged 与提交后的 patch 一致、文件仍在变更列表中、patch 不含文本 hunk。Windows release 桌面解析测试确认这种 patch 的 has_changes=true、增删行数均为 0，保留 old/new mode 元数据。Linux 真实 Git 本项、Windows 真实 Git 本项与解析测试均通过。

原生 Windows 打开相同类型的实际提交，截图 artifacts/mode-only/window.png 显示 run.sh、old mode 100644、new mode 100755 和 No text changes in this patch。此处记录的是 Git 可执行位变化，不是 Windows ACL 权限查看器。现有实现已满足该场景，本轮仅新增回归与证据，无应用逻辑更改。测试窗口已关闭。

### Windows 进程组峰值（2026-10-15）

新增 scripts/windows-job-memory.ps1：通过 CreateProcess(CREATE_SUSPENDED) 启动测试程序，在恢复线程前将其加入 Job Object，子进程继承该作业；不设置内存额度，仅设置测试清理用的 KILL_ON_JOB_CLOSE。通过 QueryInformationJobObject 读取内核持续维护的 PeakJobMemoryUsed，避免低频采样漏掉短命 Git 进程。统计依据：[Microsoft JOBOBJECT_EXTENDED_LIMIT_INFORMATION](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-jobobject_extended_limit_information)。

实际 Windows 运行 10,000 条线性历史并选择 975,000 字节 review.rs：Job 共关联 55 个进程（包括应用及其 Git 读取进程），结束采样时仅应用存活。PeakJobMemoryUsed 为 124,096,512 字节，即 118.35 MiB；PeakProcessMemoryUsed 为 99,758,080 字节，即任意单进程最高 95.14 MiB，不能据此称为 Git 独立峰值。结束时应用 Private Bytes 88.89 MiB、Working Set 96.30 MiB。截图确认实际加载数量和选中 Diff，证据 artifacts/job-memory。

该进程组峰值低于 250 MiB 预算，补足此场景早先“只有稳定采样”的缺口；GPU 不属于这项统计，不能与历史 GPU 数值拼成同次总峰值。此实验不是冷缓存启动测量，也不覆盖 100 文件／20 刷新／10 次切仓组合或长期增长。脚本的 LargeDiff 坐标针对固定两文件 benchmark 和默认 150% DPI 窗口，不能直接用于任意仓库。测试窗口及作业已关闭。

### 同进程组合压力与峰值（2026-10-15）

windows-job-memory.ps1 新增 -CombinedStress，串联 100 次顺序文件选择、20 次刷新和 10 次通过原生选择器重新打开同一仓库，最后取消一次选择器。使用 GitReview-100files（仓库共有 10,000 提交），本轮初始／重开后均加载 1,000 条，以保证返回状态可比较；该模式强制 HistoryLimit=1000。不是“加载 10,000 条后的组合压力”证据。

原生截图确认开始选择 checkpoint.txt、文件选择阶段末尾选择 13,000 行 review.rs、结束返回相同提交的 checkpoint.txt。脚本发送 100 次选择与 20 次刷新，未为每一次异步完成单独计数，因而不声称所有中间 patch 都完成显示。Job 内核峰值 135,069,696 字节，即 128.81 MiB；累计关联 1,375 个进程。初态应用 Private Bytes 71.19 MiB，结束 82.98 MiB，增加 11.79 MiB。切文件／刷新阶段采样最高 92.16 MiB；十次重开采样为 82.88、82.27、82.82、84.09、82.71、84.48、82.81、83.15、83.13、83.19 MiB。

此轮峰值与末态均在预算内；重开阶段没有单调增长，但首次与末态的差值尚未归因，单轮不能证明长期无增长。GPU 未在同次统计。证据 artifacts/combined-job-memory；脚本正常结束并关闭测试作业和窗口。本轮使用按键选择产生的正常自动定位，没有进行滚动交互排查。

### 首窗口取消重复快照读取（2026-10-15）

history::read 在 offset=0 时直接读取所需整页并建立 graph frontier，不再先调用 snapshot(limit=1) 只取 roots 后重复读取。整页快照自身已建立固定 roots；offset>0 的重建流程不变，传入的固定 roots 和显式起始布局仍被保留。没有引入额外缓存或放宽历史窗口上限。

新增 initial_history_window_keeps_snapshot_roots_and_layout，对比首窗口与独立 snapshot 的 roots、提交 ID 和图布局。Git Trace2 证据 artifacts/initial-window/trace.json：首窗口加一次独立期望快照总共两次 log（均为请求 3 条加一条结束检测），没有多余的一条记录 bootstrap。Linux 12 单元 + 12 集成全部通过，Windows 原生 12 项仓库集成全部通过，release 构建通过。此变化减少首次打开／首窗口刷新的一组元数据命令；未将命令数减少直接换算成启动耗时或内存收益。

The following dated note describes an earlier development snapshot; use **Current workflow** above for present capabilities.

### Ordinary Pull development preview (2026-10-16)

At that snapshot, the source included a Pull dialog with remote/branch selection and ordinary Pull options, HTTP authentication and a commit-message editor. Git configuration still controls the default merge/rebase strategy. Keep both `gitui-askpass.exe` and `gitui-editor.exe` beside the application. Explicit Rebase selection, conflict continuation/abort UI, Push and commit checkout were not yet available at that snapshot; they have since been implemented. That preview required handling unfinished operations externally. Option persistence, branch autocomplete and diagnostics were still in progress; see PLAN.md for subsequent implementation and validation records.

Windows validation in `artifacts/pull-ui/` covers a real divergent Pull, editing its merge message and refreshing the Log; assertions verify both merge parents and a clean worktree. This does not replace full conflict/authentication UI or memory acceptance.

### Current UI/UX regression (2026-10-17)

The UI/UX checkpoint before file history passed 349 core tests (`cargo test --no-default-features`), plus three desktop data/preview tests and the focused-editor release regression under `desktop-tests`. That checkpoint executable was SHA256 `48c44671e8e45768ae89110a6edd645efdfa6ceb25a27b986e46d6889636d427`. Historical memory figures above belong to their recorded builds; they are not measurements of this package. Current packaged Diff lifecycle and budget evidence is recorded in PLAN.md and `artifacts/uiux-final-regression`.

The file-history update passes 359 core tests, including bounded range tracking and eight real-Git history tests. The current preview executable is SHA256 `f317b5f5d1e84735084e8494900fe17ff6fbd3736f5106a17b79ca79c40d6448`. Native Windows checks cover file/selection history, source navigation, independent branch filtering, forked graphs and restored history selection; HEAD, status and working patch remain unchanged. File history now has a graph and its own Branch filter (HEAD by default, clear × for all refs). Ctrl/Shift multi-selection compares the first and last selected versions in display order. Full IDE details and multi-parent change merging remain tracked differences; large ancestry/ref sets can hit the documented traversal budget.

File history focuses its graph on activation. Ctrl+F4 closes the active source, history or Diff tab and returns to its neighbour, including when focus is outside the editor; modal dialogs protect the tabs behind them. Native before/after evidence is recorded in PLAN.md.

Latest file-history memory sample (`b027d58…`, Windows native): 10,000 main-Log commits, a 975,000-byte / 13,000-line file with one file-history revision. History preview samples were 138.38–142.41 MiB, with a 226.30 MiB Job peak including inherited Git processes and excluding GPU. Three reopen samples were 139.49 / 139.73 / 139.58 MiB. After closing all file tabs, samples were 130.79–130.81 MiB versus about 82.59 MiB initially, so full cache reclamation and long-session acceptance remain unproven. Source-copy/highlighting samples are separately documented in PLAN.md; these figures apply to the history-Diff scenario only.

Read failures in file history, version comparison and saved changes now show a short localized explanation with Refresh and Details. Project IO diagnostics and the main Log use the same bounded details dialog, with full diagnostic copying and explicit Close / Escape / Enter. Identical Stash/recovery diagnostics are shown once; distinct recovery errors remain visible. Git commands, trust configuration and write recovery behavior are unchanged.

2026-10-18 wide-graph validation: 10,000 commits, 32 parallel lanes and a
975,000-byte text Diff now load within the unchanged geometry budget. Coalescing
identical vertical half-edges reduced stored segments from 638,107 to 329,099,
without removing parent connections. Windows E422 Private Bytes sampled
119.34–127.07 MiB; the startup-through-copy Job peak was 183.83 MiB, including Git
children and excluding GPU. Full Diff copy was verified before accepting memory
results. This is one wide-graph scenario, not complete release acceptance; see
PLAN.md and artifacts/wide-3024/fixed-focused.

Current graph-print preview evidence: build `c289e2e` loaded 10,000 commits on a 256-lane fixture and displayed a 975,000-byte Diff whose full copied content matched the source. Windows Private Bytes sampled 135.16–140.71 MiB; Job peak was 203.06 MiB including Git children, excluding GPU. This is a bounded warm-host observation, not cold-cache, long-session or full-release acceptance. Source fixtures and native captures are recorded in PLAN.md; arrow navigation and exact visual details remain incomplete; the toggle is now available in the newer preview.

Earlier long-edges candidate `b3122774` (not the current package): 10,000 commits / 256 lanes with Long Edges enabled and a 975,000-byte fully verified Diff sampled 134.02–146.96 MiB Private Bytes and a 211.45 MiB Job peak including Git, excluding GPU. This bounded host run keeps the original budgets; it does not replace cold-cache or clean-Windows acceptance.

Earlier package `0f8b617d` includes menu-owned live-state subscriptions, verified by a native Windows lifetime regression. Its expanded 10,000-commit / 256-lane / full-Diff check reached 153.65 MiB steady maximum, above the 150 MiB budget; Job peak was 217.40 MiB including Git, excluding GPU. This preview is not a completed memory acceptance. See PLAN.md for remaining graph layout differences.

Earlier package `d72cb27e`: 386 core tests and 23 native Windows graph tests pass. Expanded 10,000-commit / 256-lane / verified 975,000-byte Diff: 135.93–149.09 MiB Private Bytes over 10 samples, 212.71 MiB Job peak including Git and excluding GPU. Segment storage drops from 32 to 16 bytes without changing topology or rendering. Long Edges false→true→false is verified against live checkbox, saved value and rendered graph with no Git queries. The earlier native-r3 "collapsed" screenshot was still expanded; use artifacts/long-edges-state-audit/native-compact/collapsed.png for the default state. This bounded warm-host run does not complete all visual or release acceptance.

Current package `2330e04b`: main-Log arrow hover shows the target commit; clicking navigates to the corresponding visible endpoint, selects it and focuses history without clearing filters. Unknown out-of-window endpoints show a localized explanation and preserve selection. Native Windows root→HEAD round-trip IDs match Git; the Chinese boundary test launches no extra Git work. 389 core and 26 native graph tests pass. Expanded 10,000 commits / 256 lanes / verified full 975,000-byte Diff sampled 131.52–144.61 MiB Private Bytes, 207.29 MiB Job peak including Git and excluding GPU. Exact hover-edge highlighting and the file-history print projection remain pending.

### Plico（2026-10-18）

界面与 Windows 应用名称改为 Plico，使用用户提供的紫色／青色分支 logo。已有 GitReview 配置目录继续读取，不需要重新设置语言／Diff 偏好。提交日期在 Log 与文件历史中统一显示时间、今天／昨天或最近时间；选择另一个项目可在新窗口打开。主 Log 的提交数量／偏移不再常驻展示，部分历史提示放在左下状态栏。

最新包：[Plico Windows x64 preview](dist/Plico-windows-x64-preview-20261018.zip)。exe SHA-256：`a9d986e1febbb8421e08251b3f62e5ab6fc701dfcae84cf73c7f521e29104cfd`。393 项核心测试与 Windows release 构建通过；已在 Windows 解压验证 manifest、Plico 产品元数据和图标，并启动验证左下历史状态提示。中英文日期、多窗口与文件历史图的证据及版本边界见 PLAN.md 本轮记录。

箭头悬停更新：[Plico hover preview](dist/Plico-windows-x64-preview-20261018-hover.zip)。主 Log 与文件历史共用 Rebased 风格浅色轮廓，移开恢复，沿用原彩色线／箭头；Windows 原生悬停、双向导航、无悬停 Git 查询以及解压启动通过，证据在 artifacts/graph-hover-outline。exe SHA-256：`d132ef692dbf530e1fb9d5885444ce82725dd2d99f6c89069809782f347085d0`。

HEAD节点更新：[Plico HEAD preview](dist/Plico-windows-x64-preview-20261018-head.zip)。主Log／文件历史按实际HEAD ID显示外圈与中心，留白使用实际行背景，文件首条不是HEAD时不误标记。Linux／Windows各14项文件历史专项、Windows节点与双向导航以及解压启动通过，证据 artifacts/graph-head-marker。exe SHA-256：`bdbf8d058c5fbe3b0196e256d1b3b5c7905f69933ddab1abf9f36a5e0e893fd0`。

多窗口语言修正：[Plico language preview](dist/Plico-windows-x64-preview-20261018-language.zip)。切换中英文时同步其他窗口的搜索提示和已创建的提交说明输入框；原生验证双向切换、输入保留与无新增Git查询，并确认最后窗口关闭后退出码0且无Git子进程，验证发生在Job清理前。exe SHA-256：`049297377d612f5f5a82863c382522764f289d2797f95985fda973bf8effc17c`，证据 artifacts/multiwindow-review。

启动数据就绪验收：[Plico startup preview](dist/Plico-windows-x64-preview-20261018-startup.zip)，exe SHA-256 `c2ce1dcf8a3f2a16688eafe109c99f6c885cf25a457ce074ac700c894cae4450`。Windows本地10000提交仓库，5次实际首批1000提交／1000图行／1轨，启动至可见节点、点击并复制完整OID为1.524–1.644秒，包含验证开销。缓存预读后测试，不是冷启动；冷缓存和干净Windows验收仍保留。显式 --startup-report 记录一次实际模型数量与prepaint时间，默认不写报告；该时间不能替代可交互测量。证据 artifacts/startup-data/native/startup-data.json，Windows解压／hash／产品资源／无诊断参数启动检查通过。
