//! Application UI language. Repository data and raw Git output are never translated.
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Language { #[default] System, Chinese, English }
static PREFERENCE: AtomicU8 = AtomicU8::new(0);
static CHINESE: AtomicU8 = AtomicU8::new(0);
impl Language {
    pub fn parse(value: &str) -> Self {
        match value.trim() { "zh-CN" => Self::Chinese, "en" => Self::English, _ => Self::System }
    }
    pub fn encode(self) -> &'static str {
        match self { Self::System => "system", Self::Chinese => "zh-CN", Self::English => "en" }
    }
    pub fn uses_chinese(self, system_chinese: bool) -> bool {
        match self { Self::System => system_chinese, Self::Chinese => true, Self::English => false }
    }
}
#[cfg(any(windows, test))]
fn chinese_langid(id: u16) -> bool { id & 0x03ff == 0x04 }
#[cfg(windows)]
fn system_chinese() -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" { fn GetUserDefaultUILanguage() -> u16; }
    // Returns the current user's display language, not the keyboard or regional format.
    chinese_langid(unsafe { GetUserDefaultUILanguage() })
}
#[cfg(not(windows))]
fn system_chinese() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|key|
        std::env::var(key).ok().filter(|v| !v.is_empty()))
        .is_some_and(|v| v.split(['-', '_', '.']).next().is_some_and(|s| s.eq_ignore_ascii_case("zh")))
}
pub fn preference() -> Language {
    match PREFERENCE.load(Ordering::Relaxed) { 1 => Language::Chinese, 2 => Language::English, _ => Language::System }
}
pub fn apply(value: Language) {
    CHINESE.store(u8::from(value.uses_chinese(system_chinese())), Ordering::Relaxed);
    PREFERENCE.store(value as u8, Ordering::Relaxed);
}
pub fn is_chinese() -> bool { CHINESE.load(Ordering::Relaxed) != 0 }
pub fn t(english: &'static str) -> &'static str {
    translate(english, CHINESE.load(Ordering::Relaxed) != 0)
}
fn translate(english: &'static str, chinese: bool) -> &'static str {
    if !chinese { return english; }
    match english {
        "Previous change" => "上一处更改",
        "Next change" => "下一处更改",
        "Rollback this change" => "撤回此处更改",
        "Copy original lines" => "复制原内容",
        "Git baseline changed. Reopen the change marker." => "Git 基线已变化，请重新打开变更标记。",
        "Open Project" => "打开项目",
        "Where would you like to open the project?" => "您希望在哪里打开此项目？",
        "This Window" => "当前窗口",
        "New Window" => "新窗口",
        "History is partially loaded" => "历史尚未完整加载",

        "Clear branch filter" => "清除分支筛选",
        "Clear date filter" => "清除日期筛选",
        "Clear user filter" => "清除用户筛选",
        "Clear paths filter" => "清除路径筛选",
        "Close Other Tabs" => "关闭其他标签页",
        "Close All Tabs" => "关闭所有标签页",
        "Close Tabs to the Right" => "关闭右侧标签页",
        "Close All Read-Only" => "关闭所有只读标签页",
        "Recent Projects" => "最近的项目",
        "Open Projects" => "已打开的项目",
        "Projects" => "项目",
        "Remove from Recent Projects" => "从最近项目中移除",
        "Project folder is unavailable" => "项目文件夹不可用",
        "Could not save recent projects" => "无法保存最近项目",
        "Commit entire file, including unstaged changes" => "提交整个文件，包含未暂存的更改",
        "Changes" => "更改", "Unversioned Files" => "未版本化文件", "No changes to commit." => "没有可提交的更改。",
        "Commit" => "提交", "Commit…" => "提交…", "Filters" => "筛选", "Amend" => "修订提交", "Amend last commit" => "修订上次提交",
        "Amend Commit" => "修订提交", "Amend Commit and Push…" => "修订提交并推送…",
        "Commit and Push…" => "提交并推送…", "Loading commit message…" => "正在加载提交消息…",
        "Committing…" => "正在提交…", "Committed" => "已提交", "Commit failed" => "提交失败",
        "Could not restore staged changes" => "无法恢复暂存的更改",
        "Could not load local changes" => "无法加载本地变更",
        "Could not load commit message" => "无法加载提交说明",
        "View Options" => "显示选项",
        "Long Edges" => "长连线",
        "Show Details" => "显示详情",
        "Commit cancelled" => "已取消提交", "Commit timed out" => "提交超时", "Nothing to commit" => "没有可提交的更改",
        "Commit signing failed" => "提交签名失败", "Commit index recovery is required" => "需要恢复提交前的暂存内容",
        "Could not remove commit recovery record" => "无法删除提交恢复记录",
        "Restore staged changes" => "恢复暂存内容", "Restoring staged changes…" => "正在恢复暂存内容…",
        "Staged changes restored" => "暂存内容已恢复", "HEAD changed; reload Amend" => "HEAD 已改变，请重新开启修订提交",
        "Add to Favorites" => "添加到收藏", "Remove from Favorites" => "取消收藏",
        "Could not load or save favorites" => "无法读取或保存收藏设置",
        "Checkout" => "检出", "Copy Reference Name" => "复制引用名称",
        "Checking out branch…" => "正在检出分支…", "Branch checked out" => "已检出分支",
        "Could not check out branch; repository refreshed" => "无法检出分支；已刷新仓库",
        "Continue Rebase" => "继续变基", "Abort Rebase" => "中止变基",
        "Abort the rebase in this repository?" => "要中止此仓库中的变基吗？",
        "Updating rebase state…" => "正在更新变基状态…",
        "Rebase operation cancelled; repository refreshed" => "变基操作已取消；仓库已刷新",
        "Rebase requires conflict resolution" => "变基需要解决冲突",
        "Rebase is still in progress" => "变基尚未完成",
        "Rebase aborted" => "变基已中止", "Rebase completed" => "变基已完成",
        "Rebase operation failed" => "变基操作失败",
        "Rebase details" => "变基详情",
        "Complete Merge" => "完成合并", "Abort Merge" => "中止合并", "Abort" => "中止",
        "Abort the merge in this repository?" => "要中止此仓库中正在进行的合并吗？",
        "Updating merge state…" => "正在更新合并状态…",
        "Merge operation cancelled" => "已取消合并操作", "Merge operation failed" => "合并操作失败",
        "Merge completed" => "合并完成", "Merge aborted" => "已中止合并",
        "No commits selected" => "未选择提交", "Commit details" => "提交详情", "All" => "全部",
        "Push Commits to" => "推送提交至", "Push tags:" => "推送标签：", "Current Branch" => "当前分支",
        "You're going to force push to" => "即将强制推送到",
        "It may overwrite commits at the remote. Are you sure you want to proceed?" => "此操作可能覆盖远端提交。确定要继续吗？",
        "Don't warn about this target" => "对此目标不再提醒",
        "Force Push" => "强制推送", "Force Push…" => "强制推送…", "Push options" => "推送选项",
        "Force push is prohibited for this protected branch" => "禁止强制推送到此受保护分支",
        "Force pushing may overwrite commits at the remote. Are you sure you want to proceed?" => "强制推送可能覆盖远端提交。确定要继续吗？",
        "Push help" => "推送帮助",
        "Push selected commits to the target branch. Enable Push tags to include tags. Force Push checks the last known remote revision before replacing history." => "将所选提交推送到目标分支。勾选推送标签可同时推送标签。强制推送会在替换历史前核对已知的远端版本。",
        "Push" => "推送", "Push Commits" => "推送提交", "Pushing…" => "正在推送…", "Push completed" => "推送完成",
        "Check out a local branch with commits before pushing" => "请先切换到已有提交的本地分支，再进行推送",
        "Already up to date" => "已是最新", "Push rejected: remote history has changed" => "推送被拒绝：远端历史已变化",
        "Push rejected: stale remote information" => "推送被拒绝：远端信息已过期", "Push rejected by remote" => "远端拒绝了推送",
        "Push cancelled; remote changes may have completed" => "已取消推送；部分远端更新可能已完成",
        "Push failed or incomplete; some remote updates may have completed" => "推送失败或未完成；部分远端更新可能已完成",
        "Pop" => "弹出", "Restore changes and keep the stash" => "恢复更改并保留保存记录",
        "Restore changes and remove the stash on success" => "恢复更改，成功后移除保存记录",
        "Loading…" => "正在加载…",
        "Could not load comparison. Refresh to retry." => "无法加载比较，请刷新重试。",
        "Comparison unavailable. Refresh the Changes panel to retry." => "比较内容不可用，请在更改面板中刷新重试。",
        "Saved changes from Squash still need review." => "Squash 保存的更改仍需确认处理。",
        "Already handled externally…" => "已在外部处理…",
        "Could not inspect the Squash recovery record." => "无法读取 Squash 恢复记录。",
        "Finish Squash recovery" => "结束 Squash 恢复",
        "Confirm only after handling the saved changes externally. This clears the app recovery record; it does not restore or delete any stash or change your files." => "请在外部处理好保存的更改后再确认。此操作只清除应用恢复记录，不会恢复或删除 stash，也不会修改文件。",
        "Clear recovery record" => "清除恢复记录",
        "Clearing recovery record…" => "正在清除恢复记录…",
        "Recovery record cleared; saved changes kept" => "恢复记录已清除，保存的更改已保留",
        "Recovery record was not cleared. Refresh and review the repository state." => "未清除恢复记录，请刷新并检查仓库状态。",
        "Refresh" => "刷新",
        "Press F7 again to go to the next file" => "再次按 F7 转到下一个文件",
        "Press Shift+F7 again to go to the previous file" => "再次按 Shift+F7 转到上一个文件",
        "Load more files…" => "加载更多文件…",
        "Folder changed. Refresh to reload its files." => "目录已变化，请刷新后重新加载文件。",
        "Local changes" => "本地变更",
        "Show History" => "显示历史",
        "Show History for Selection…" => "显示选中范围的历史…",
        "History for Selection" => "选中范围的历史",
        "History" => "历史",
        "Copy" => "复制",
        "Select All" => "全选",
        "Changes only" => "仅显示有更改的版本",
        "Version" => "版本",
        "Author" => "作者",
        "Commit Message" => "提交说明",
        "Revision" => "版本",
        "More file history is available" => "还有更早的文件历史",
        "No history for this file" => "此文件没有 Git 历史",
        "Loading file history…" => "正在加载文件历史…",
        "Could not load file history" => "无法加载文件历史",
        "Jump to Source · F4" => "跳转到源码 · F4",
        "Jump to Commit" => "跳转到提交",
        "Commit is outside the loaded history window" => "提交不在已加载的历史窗口中",
        "History belongs to another repository; reopen its file" => "此历史属于另一个仓库，请重新打开对应文件",
        "Could not jump to source" => "无法跳转到源码",
        "Details" => "详情",
        "Error details" => "错误详情",
        "Could not load saved changes" => "无法加载已保存的更改",
        "Git error details" => "Git 错误详情",
        "Repository access blocked" => "仓库访问被阻止",
        "Could not load repository data" => "无法加载仓库数据",
        "Git blocked this repository because its owner differs" => "Git 因仓库所有者不一致而阻止了访问",
        "Git refused access because the repository owner differs. Review the path and Git's instructions before trusting this directory. No trust configuration has been changed." => "Git 因仓库所有者不一致而拒绝访问。信任此目录前，请核对路径及下方 Git 提示。程序未修改信任配置。",
        "Operation failed" => "操作失败",
        "Could not load changes. Refresh to retry." => "无法加载变更，请刷新重试。",
        "Could not load the diff." => "无法加载差异。",
        "Reload diff" => "重新加载差异",
        "Could not save Diff preferences. Try the setting again." => "无法保存差异设置，请重试该设置。",
        "Copy error details" => "复制错误详情",
        "Compare Versions" => "比较版本", "No differences" => "没有差异",
        "Go to Parent Commit" => "转到父提交", "Go to Child Commit" => "转到子提交",
        "Select Parent Commit" => "选择父提交", "Select Child Commit" => "选择子提交",
        "Undo Squash" => "撤销压缩", "Undoing Squash…" => "正在撤销压缩…",
        "Squash undone" => "已撤销压缩", "Could not undo Squash" => "无法撤销压缩",
        "Reset Current Branch to Here…" => "将当前分支重置到此处…",
        "Resetting… This operation cannot be cancelled once started." => "正在重置…实际写入开始后不可取消。",
        "Only line separators differ" => "仅换行符不同",
        "Edit Commit Message…" => "编辑提交信息…",
        "Edit Commit Message" => "编辑提交信息",
        "Edit message for commit" => "编辑提交信息：",
        "by" => "作者",
        "Editing Initial Commit" => "编辑初始提交",
        "Editing the initial commit rewrites the history of the current branch. Continue?" => "编辑初始提交将重写当前分支的历史。是否继续？",
        "Editing commit message…" => "正在编辑提交信息…",
        "Commit message edited" => "提交信息已修改",
        "Commit message edited; follow-up failed" => "提交信息已修改，但后续处理失败",
        "Could not edit commit message" => "无法编辑提交信息",
        "This will reset the current branch head to the selected commit,\nand update the working tree and the index according to the selected mode:" => "将当前分支的 HEAD 重置到选中的提交，\n并根据所选模式更新工作区和暂存区：",
        "Files will be reverted to the state of the selected commit.\nWarning: any local changes will be lost." => "文件将恢复到选中提交的状态。\n警告：所有本地更改将丢失。",
        "Files will be reverted to the state of the selected commit,\nbut local changes will be kept intact." => "文件将恢复到选中提交的状态，\n但会保留本地更改。",
        "Git Reset" => "Git 重置", "Reset" => "重置",
        "Resetting…" => "正在重置…", "Reset successful" => "重置成功", "Reset failed" => "重置失败",
        "Reset the current branch to the selected commit:" => "将当前分支重置到选中的提交：",
        "Files won't change, differences will be staged for commit." => "文件保持不变，差异将暂存以供提交。",
        "Files won't change, differences won't be staged." => "文件保持不变，差异不会暂存。",
        "Files will be reverted to the selected commit. Warning: local changes will be lost." => "文件将恢复到选中的提交。警告：本地更改将丢失。",
        "Files will be reverted to the selected commit, keeping local changes intact." => "文件将恢复到选中的提交，并保留本地更改。",
        "Squash Commits" => "压缩提交", "Squash Commits…" => "压缩提交…",
        "Message for the squashed commit:" => "压缩后的提交消息：",
        "Checking…" => "正在检查…",
        "Cannot edit selected commits: repository state was changed" => "无法编辑所选提交：仓库状态已发生变化",
        "Cannot edit selected commits: pushed to protected branch" => "无法编辑所选提交：已推送到受保护分支",
        "Cannot edit selected commits: another Git operation is in progress" => "无法编辑所选提交：另一个 Git 操作正在进行",
        "Cannot edit selected commits: recover the previous operation first" => "无法编辑所选提交：请先恢复上一次操作",
        "Cannot validate selected commits. Close this dialog and refresh the log." => "无法验证所选提交。请关闭此对话框并刷新日志。",
        "Preparing Squash…" => "正在准备压缩提交…", "Squashing commits…" => "正在压缩提交…",
        "Commits squashed" => "已压缩提交", "Could not squash commits; repository refreshed" => "未能完成压缩提交；已刷新仓库",
        "Restoring saved changes…" => "正在恢复已保存的更改…", "Saved changes restored" => "已恢复保存的更改",
        "Saved changes restored; recovery record needs attention" => "已恢复保存的更改；恢复记录仍需处理",
        "Saved changes have conflicts; review Local Changes" => "恢复时发生冲突，请查看本地更改",
        "Could not restore the saved index; review Local Changes" => "无法恢复暂存状态，请查看本地更改",
        "Local changes would be overwritten; stash kept" => "本地更改会被覆盖，保存记录已保留",
        "Untracked files would be overwritten; stash kept" => "未跟踪文件会被覆盖，保存记录已保留",
        "Restore cancelled; check Local Changes before retrying" => "已取消恢复，重试前请检查本地更改",
        "Restore failed or incomplete; review Local Changes" => "恢复失败或未完成，请查看本地更改",
        "Could not restore saved changes" => "无法恢复已保存的更改",
        "Show saved changes" => "查看已保存的更改",
        "Stashes" => "已保存的更改", "No stashes" => "没有已保存的更改",
        "Only the first 1000 stashes are shown" => "仅显示前 1000 条已保存的更改",
        "Working tree" => "工作区", "Index" => "暂存区", "Untracked files" => "未跟踪文件",
        "Merge result" => "合并结果",
        "Parent revision" => "父版本",
        "First parent" => "第一父提交", "Empty tree" => "空目录树", "Empty file" => "空文件",
        "Unmerged" => "未合并", "Untracked working file" => "未跟踪的工作区文件", "Staged" => "已暂存",
        "Loading saved changes…" => "正在加载已保存的更改…",
        "Copy Revision Number" => "复制版本号",
        "Select commits to view changes" => "选择提交以查看更改",
        "Showing the first 50 commits" => "仅显示前 50 条提交详情",
        "commits" => "个提交", "No changes" => "没有变更",
        "Show Diff" => "显示差异", "Could not open file diff" => "无法打开文件差异",
        "Set upstream" => "设为上游", "New upstream" => "新上游",
        "Apply" => "应用", "Previous" => "上一页", "Next" => "下一页", "New branch" => "新分支",
        "Push references changed; reopen the push dialog" => "远端引用已变化，请重新打开推送窗口以刷新列表",
        "Smart Checkout" => "智能检出",
        "Force Checkout" => "强制检出",
        "Don't Checkout" => "不检出",
        "Checkout Problem" => "检出问题",
        "Save local changes, check out, then restore them" => "保存本地修改，检出后恢复修改",
        "Check out and overwrite local changes" => "检出并覆盖本地修改",
        "Local changes would be overwritten. Smart Checkout saves and restores them." => "本地修改将被覆盖。智能检出会先保存修改，检出后再恢复。",
        "The affected file list may be incomplete" => "受影响文件列表可能不完整",
        "Local changes were not fully restored; review conflicts and saved stash" => "本地修改未完全恢复，请检查冲突和保留的 stash",
        "Commit checked out; local changes restored" => "已检出提交并恢复本地修改",
        "Checkout Tag or Revision" => "检出标签或版本",
        "Search for branches and actions" => "搜索分支和操作",
        "No matching branches or actions" => "没有匹配的分支或操作",
        "Checkout Tag or Revision…" => "检出标签或版本…",
        "Enter a branch, tag or commit revision" => "输入分支、标签或提交版本",
        "Checkout opens the revision in detached HEAD mode." => "检出后将处于游离 HEAD 状态。",
        "Resolving revision…" => "正在解析版本…",
        "Could not resolve revision" => "无法解析版本",
        "detached HEAD" => "游离 HEAD",
        "Checkout Revision" => "检出提交",
        "Checkout timed out; repository refreshed" => "检出超时，仓库状态已刷新",
        "Resolve existing index conflicts before checkout" => "请先解决索引中的冲突，再检出提交",
        "Local changes would be overwritten by checkout" => "检出会覆盖本地修改，请先处理这些修改",
        "Untracked files would be overwritten by checkout" => "检出会覆盖未跟踪文件，请先处理这些文件",
        "Checking out commit…" => "正在检出提交…",
        "Checkout cancelled; repository refreshed" => "已取消检出，仓库状态已刷新",
        "Commit checked out (detached HEAD)" => "已检出提交（分离 HEAD）",
        "Could not check out commit; repository refreshed" => "无法检出提交，仓库状态已刷新",
        "Could not load outgoing commits" => "无法加载待推送提交，请检查目标",
        "Loading commit details…" => "正在加载提交详情…",
        "Could not load commit details" => "无法加载提交详情", "Select a commit to view changes" => "选择提交以查看变更",
        "Loading outgoing commits…" => "正在加载待推送提交…", "No outgoing commits" => "没有待推送提交",
        "New branch preview is limited to 1,000 commits" => "新分支预览最多显示 1,000 条提交",
        "Pull" => "拉取", "Pulling…" => "正在拉取…", "Pull completed" => "拉取完成",
        "Pull cancelled; local changes remain saved in stash" => "已取消拉取；本地修改仍保存在 stash 中",
        "Pull incomplete; local changes remain saved in stash" => "拉取未完整完成；本地修改仍保存在 stash 中",
        "Pull failed" => "拉取失败", "Pull cancelled" => "已取消拉取",
        "Pull requires conflict resolution" => "拉取后存在待解决的冲突",
        "Pull left an unfinished operation" => "拉取后仍有待完成的 Git 操作",
        "Loading pull targets…" => "正在读取拉取目标…", "Could not read pull targets" => "无法读取拉取目标",
        "Select remote" => "选择远端", "Modify options" => "修改选项",
        "Select an existing remote branch" => "请选择已有的远端分支",
        "Git editor helper is unavailable" => "Git 编辑器辅助程序不可用",
        "Edit commit message" => "编辑提交说明", "Commit message" => "提交说明",
        "Edit rebase todo" => "编辑变基任务",
        "Could not save. The file may have changed or the operation has ended." => "无法保存：文件可能已更改，或操作已经结束。",
        "Git review" => "Git 审阅", "Local repositories · Git review · Ctrl+O" => "本地仓库 · Git 审阅 · Ctrl+O",
        "Fetch" => "获取", "Fetching…" => "正在获取…", "Fetch completed" => "获取完成",
        "Fetch cancelled" => "已取消获取", "Fetch failed" => "获取失败", "No remotes configured" => "未配置远端",
        "Authentication helper is unavailable" => "认证助手不可用", "Git authentication" => "Git 认证",
        "Log in to" => "登录到", "Log In" => "登录", "Enter your credentials" => "请输入凭据",
        "Username must not be empty" => "用户名不能为空", "Password must not be empty" => "密码不能为空",
        "Username" => "用户名", "Password" => "密码", "Username and password must not be empty" => "用户名和密码不能为空",
        "Rebase the current branch on top of incoming changes" => "将当前分支变基到传入更改之上",
        "Merge only if it can be fast-forwarded" => "仅在可以快进时合并",
        "Create a merge commit even if it can be fast-forwarded" => "即使可以快进，也创建合并提交",
        "Create a single commit for all pulled changes" => "将所有拉取的更改压缩为一个提交",
        "Merge, but do not commit the result" => "合并，但不提交结果",
        "Bypass the pre-merge and commit message hooks" => "跳过合并前和提交消息钩子",
        "Remove option (Backspace/Delete)" => "移除选项（Backspace/Delete）",
        "Close file" => "关闭文件",
        "Unstaged" => "未暂存",
        "Untracked" => "未跟踪",
        "Select a file to view its diff" => "选择文件以查看差异",
        "Loading local changes…" => "正在加载本地更改…",
        "Project" => "项目",
        "Loading files…" => "正在加载文件…",
        "Loading file…" => "正在加载文件…",
        "Editor Tabs" => "编辑器标签页",
        "Enable preview tab" => "启用预览标签页",
        "Select a file to preview it. Double-click or press Enter to keep it open." => "选择文件以预览。双击或按 Enter 将其固定打开。",
        "Saving…" => "正在保存…",
        "Unsaved changes" => "未保存的更改",
        "Automatically saved" => "自动保存",
        "Could not save file" => "无法保存文件",
        "Could not reload file" => "无法重新加载文件",
        "File changed while reloading. Your edits have been kept." => "重新加载期间文件内容发生了变化，已保留你的编辑。",
        "Reload from Disk" => "从磁盘重新加载",
        "Discard your unsaved changes and reload this file from disk?" => "放弃未保存的更改，并从磁盘重新加载此文件？",
        "File changed on disk. Your edits have been kept; reload or compare before saving." => "磁盘上的文件已更改。你的编辑已保留，请检查后重新加载。",
        "File is read-only. Your edits have been kept." => "文件为只读。你的编辑已保留。",
        "Read-only file" => "只读文件",
        "Reload file" => "重新加载文件",
        "Collapse folders before opening more files" => "请先折叠部分文件夹，再展开更多文件",
        "Linked or special files are not opened" => "暂不支持打开链接或特殊文件",
        "Binary file cannot be displayed as text" => "二进制文件无法以文本显示",
        "File encoding is not supported" => "暂不支持此文件编码",
        "Invalid project path" => "项目路径无效",
        "Folder is no longer available" => "文件夹已不可用",
        "Not a regular file" => "不是普通文件",
        "Folder exceeds the display limit" => "文件夹超出显示数量限制",
        "File exceeds the 5 MiB preview limit" => "文件超过 5 MiB 预览限制",
        "File changed while reading; reload it" => "读取时文件已更改，请重新加载",
        "Could not read project file" => "无法读取项目文件",
        "Could not read project folder" => "无法读取项目文件夹",
        "Update Project" => "更新项目",
        "Preparing project update…" => "正在准备更新项目…",
        "Updating project…" => "正在更新项目…",
        "Project updated" => "项目已更新",
        "Cannot update project" => "无法更新项目",
        "Update cancelled; review local changes and saved stash" => "更新已取消，请检查本地改动和保存的暂存",
        "Update incomplete; review conflicts and saved local changes" => "更新未完成，请检查冲突和保存的本地改动",
        "Merge incoming changes into the current branch" => "将传入更改合并到当前分支",
        "Don't show again" => "不再显示",
        "Show Update Project options" => "显示更新项目选项",
        "Main menu" => "主菜单",
        "Editor Font" => "编辑器字体", "Font:" => "字体：", "Size:" => "字号：", "Preview" => "预览", "Restore Defaults" => "恢复默认值",
        "Settings" => "设置", "Close" => "关闭", "Language" => "语言",
        "Follow system" => "跟随系统", "Version Control · Diff" => "版本控制 · 差异",
        "Open file differences in" => "文件差异打开方式",
        "Editor tab (default)" => "独立标签页（默认）", "Bottom preview" => "底部预览",
        "Open…" => "打开…", "Log" => "日志", "Local Changes" => "本地更改",
        "Close diff · Esc" => "关闭差异 · Esc", "Refresh (Ctrl+F5)" => "刷新（Ctrl+F5）",
        "Select Period" => "选择时间范围", "OK" => "确定", "Branch" => "分支",
        "HEAD (Current Branch)" => "HEAD（当前分支）", "No matching branches" => "没有匹配的分支",
        "Regular expression" => "正则表达式", "Match case" => "区分大小写",
        "Shallow boundary" => "浅克隆边界", "Newer window" => "较新的历史",
        "Return to the preceding history window in this snapshot" => "返回此快照的上一段历史",
        "Back to latest" => "返回最新", "Show Diff · Ctrl+D" => "显示差异 · Ctrl+D",
        "Expand All" => "全部展开", "Collapse All" => "全部折叠", "Parents" => "父提交",
        "Unavailable (shallow boundary)" => "不可用（浅克隆边界）", "Root commit" => "初始提交",
        "Reading Git data…" => "正在读取 Git 数据…",
        "Previous change (Shift+F7)" => "上一处更改（Shift+F7）", "Next change (F7)" => "下一处更改（F7）",
        "Browse history. Review changes." => "浏览历史，审阅更改。", "Open Repository…" => "打开仓库…",
        "Local repositories · Read-only · Ctrl+O" => "本地仓库 · 只读浏览 · Ctrl+O",
        "Shallow boundary · Parent comparison unavailable" => "浅克隆边界 · 无法与父提交比较",
        "Local changes · staged and unstaged are separate comparisons" => "本地变更 · 已暂存与未暂存内容分别比较",
        "Unstaged · Index → Working tree" => "未暂存 · 暂存区 → 工作区",
        "Staged · HEAD → Index" => "已暂存 · HEAD → 暂存区",
        "Untracked · New file preview" => "未跟踪 · 新文件预览",
        "Conflict · Unmerged index entries" => "冲突 · 暂存区存在未合并条目",
        "Committed change" => "已提交变更",
        "No commits match the message filter. Clear the filter field and press Enter to show all history." => "没有提交匹配消息筛选。清空筛选框并按 Enter 显示全部历史。",
        "No local changes. The index and working tree are clean." => "没有本地变更。暂存区和工作区均干净。",
        "Parent history is unavailable at this shallow boundary. Deepen the clone with your Git tool, then refresh to review its changes." => "此浅克隆边界没有可用的父提交历史。请使用 Git 工具加深克隆，再刷新查看变更。",
        "No file changes compared with the first parent. Other merge parents may differ." => "与第一父提交相比没有文件变更，其他合并父提交可能不同。",
        "This commit has no file changes compared with its parent or the empty tree." => "此提交与父提交或空树相比没有文件变更。",
        "No commits in this history. Use Local Changes to review files before the first commit." => "此历史中没有提交。首次提交前可在“本地变更”中查看文件。",
        "No patch was returned for this file. Refresh to check for changes made outside this viewer." => "此文件没有返回补丁。请刷新以检查外部修改。",
        "Select a changed file to review its diff" => "选择已变更文件以查看差异",
        "Cannot save Diff settings" => "无法保存差异设置",
        "Cannot save Diff view" => "无法保存差异视图",
        "Expand" => "展开", "Restore" => "还原",
        "Submodule reference changed · The diff shows commit IDs, not the files inside the submodule." => "子模块引用已变更 · 差异显示提交 ID，不显示子模块内部文件。",
        "Read-only" => "只读浏览", "Clear filters and show" => "清除筛选并显示", "Dismiss" => "关闭提示",
        "Ctrl+D  Open diff    F7  Next change    Ctrl+F5  Refresh" => "Ctrl+D  打开差异    F7  下一处更改    Ctrl+F5  刷新",
        "Settings could not be saved" => "无法保存设置",
        "Highlighting Differences" => "差异高亮", "Lines" => "行", "Words" => "单词", "Characters" => "字符",
        "Appearance" => "外观", "Show Line Numbers" => "显示行号", "Soft-Wrap" => "自动换行",
        "Side-by-side viewer" => "双栏对比", "Unified viewer" => "单栏对比",
        "Text or hash" => "文本或哈希", "Branches and Tags" => "分支和标签",
        "User" => "用户", "Paths" => "路径", "Date" => "日期", "Local" => "本地",
        "All branches" => "所有分支", "All dates" => "所有日期",
        "Last 24 hours" => "最近 24 小时", "Last 7 days" => "最近 7 天",
        "Select…" => "选择…", "Select in Tree…" => "在树中选择…",
        "Version Control · Git" => "版本控制 · Git",
        "Applies to the current worktree" => "仅应用于当前工作树",
        "Auto-update if push is rejected" => "推送被拒绝时自动更新",
        "Update" => "更新",
        "Update method" => "更新方式",
        "Follow branch configuration" => "跟随分支配置",
        "Remember this choice for future push rejections (change in Settings)" => "记住选择，用于以后推送被拒绝时（可在设置中修改）",
        "Push Rejected" => "推送被拒绝",
        "Update Confirmation" => "确认更新",
        "Merge" => "合并",
        "Rebase" => "变基",
        "Rebase Anyway" => "仍然变基",
        "Remote history has changed. Update before pushing again." => "远端历史已变化，请更新后再次推送。",
        "Rebase may discard changes made in merge commits. Merge instead or rebase anyway?" => "变基可能丢失合并提交中的修改。改为合并，还是仍然变基？",
        "Could not check merge commits. Choose how to update or cancel." => "无法检查合并提交，请选择更新方式或取消。",
        "Rebase will rewrite published commits. Rebase anyway?" => "变基将改写已发布的提交。仍然变基？",
        "Could not check published commits. Rebase anyway?" => "无法检查已发布的提交。仍然变基？",
        "Push update cancelled; local changes remain saved in stash" => "已取消推送更新，本地修改仍保存在 stash 中",
        "Loading settings…" => "正在加载设置…",
        "Version Control" => "版本控制", "Diff" => "差异",
        "Open a repository to configure Git" => "打开仓库后可配置 Git",
        "Git settings are unavailable during an operation" => "操作进行中，暂时无法加载 Git 设置",
        "Rebasing Merge Commits" => "变基合并提交",
        "Push update cancelled" => "已取消推送更新",
        "Update incomplete; review conflicts and saved local changes before pushing" => "更新未完成，请先检查冲突和已保存的本地修改，再推送",
        "Select Paths to Filter by" => "选择要筛选的路径", "Cancel" => "取消",
        "Recent" => "最近使用", "me" => "我", "Loading users…" => "正在加载用户…",
        "After" => "晚于", "Before" => "早于", "Since" => "自", "Until" => "截至",
        "Hide branches" => "隐藏分支", "Show branches" => "显示分支",
        "Select one or more values separated with new lines, use Ctrl+Enter to finish" => "每行输入一个值，按 Ctrl+Enter 完成",
        "Select one or more values separated with | or new lines, use Ctrl+Enter to finish" => "用 | 或换行分隔多个值，按 Ctrl+Enter 完成",
        "Older window" => "较早的历史", "Load more" => "加载更多",
        "  · Shallow history boundary" => "  · 浅克隆历史边界",
        "  · End of available history" => "  · 已到历史末尾",
        "In branches: loading…" => "所属分支：正在加载…", "Not in any branch" => "不在任何分支中",
        "Hide" => "收起", "Show all" => "显示全部", "Tags" => "标签", "Remote" => "远程",
        "No textual differences" => "没有文本差异", "Previous file" => "上一个文件", "Next file" => "下一个文件",
        "Search files" => "搜索文件", "Previous difference · Shift+F7" => "上一处差异 · Shift+F7",
        "Next difference · F7" => "下一处差异 · F7",
        "Update Branch Filter in Log" => "更新日志中的分支筛选",
        "Navigate Log to Selected Branch Head" => "在日志中定位所选引用的提交",
        _ => english,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preferences_override_system_and_unknown_values_follow_system() {
        for id in [0x0404, 0x0804, 0x0c04, 0x1004, 0x1404] { assert!(chinese_langid(id)); }
        for id in [0, 0x0409, 0x0411, 0x1400] { assert!(!chinese_langid(id)); }
        for system in [true, false] {
            assert!(Language::Chinese.uses_chinese(system));
            assert!(!Language::English.uses_chinese(system));
            assert_eq!(Language::System.uses_chinese(system), system);
        }
        for value in ["system", "", "broken", "zh-CN-overlong"] { assert_eq!(Language::parse(value), Language::System); }
        for language in [Language::System, Language::Chinese, Language::English] { assert_eq!(Language::parse(language.encode()), language); }
    }
    #[test]
    fn messages_fall_back_without_modifying_unrecognized_data() {
        assert_eq!(translate("Settings", true), "设置");
        assert_eq!(translate("Settings", false), "Settings");
        assert_eq!(translate("feature/中文", true), "feature/中文");
        assert_eq!(translate("unknown message", true), "unknown message");
    }
}


/// Typed messages keep counts and repository-provided values out of translation keys.
pub enum Message<'a> {
    Locating(&'a str),
    HiddenInLog(&'a str),
    NotInHistory(&'a str),
    MoreReferences(usize),
    LoadedCommits { count: usize, offset: usize },
    FilePosition { current: usize, total: usize },
    ContainingBranches(usize),
    RepositoryDiff(&'a str),
    AuthorDate { author: &'a str, date: &'a str },
    Committer { person: Option<&'a str>, date: Option<&'a str> },
    ShallowParents,

}
pub fn message(value: Message<'_>) -> String {
    render_message(value, CHINESE.load(Ordering::Relaxed) != 0)
}
pub(crate) fn render_message(value: Message<'_>, chinese: bool) -> String {
    match (value, chinese) {
        (Message::Locating(name), true) => format!("正在定位 {name}…"),
        (Message::Locating(name), false) => format!("Locating {name}…"),
        (Message::HiddenInLog(name), true) => format!("{name} 被当前日志视图隐藏"),
        (Message::HiddenInLog(name), false) => format!("{name} is hidden by the current Log view"),
        (Message::NotInHistory(name), true) => format!("在仓库历史中未找到 {name}"),
        (Message::NotInHistory(name), false) => format!("{name} was not found in repository history"),
        (Message::MoreReferences(count), true) => format!("另有 {count} 个引用"),
        (Message::MoreReferences(count), false) => format!("{count} more"),
        (Message::AuthorDate { author, date }, true) => format!("{author} 于 {date}"),
        (Message::AuthorDate { author, date }, false) => format!("{author} on {date}"),
        (Message::Committer { person, date }, true) => match (person, date) {
            (Some(person), Some(date)) => format!("由 {person} 于 {date} 提交"),
            (Some(person), None) => format!("由 {person} 提交"),
            (None, Some(date)) => format!("提交于 {date}"),
            (None, None) => String::new(),
        },
        (Message::Committer { person, date }, false) => match (person, date) {
            (Some(person), Some(date)) => format!("committed by {person} on {date}"),
            (Some(person), None) => format!("committed by {person}"),
            (None, Some(date)) => format!("committed on {date}"),
            (None, None) => String::new(),
        },
        (Message::ShallowParents, true) => "父提交不可用（浅克隆边界）".into(),
        (Message::ShallowParents, false) => "Parents unavailable (shallow boundary)".into(),
        (Message::LoadedCommits { count, offset }, true) => format!("已加载 {count} 条提交 · 偏移 {offset}"),
        (Message::LoadedCommits { count, offset }, false) => format!("{count} commits loaded · offset {offset}"),
        (Message::FilePosition { current, total }, true) => format!("{current}/{total} 个文件"),
        (Message::FilePosition { current, total }, false) => format!("{current}/{total} files"),
        (Message::ContainingBranches(count), true) => format!("位于 {count} 个分支："),
        (Message::ContainingBranches(count), false) => format!("In {count} {}: ", if count == 1 { "branch" } else { "branches" }),
        (Message::RepositoryDiff(name), true) => format!("仓库差异：{name}"),
        (Message::RepositoryDiff(name), false) => format!("Repository Diff: {name}"),
    }
}
#[cfg(test)]
mod message_tests {
    use super::*;
    #[test]
    fn dynamic_values_are_not_translation_keys_or_format_templates() {
        let name = "目录/{count} & <file>.rs";
        assert_eq!(render_message(Message::RepositoryDiff(name), true), format!("仓库差异：{name}"));
        assert_eq!(render_message(Message::RepositoryDiff("Settings"), false), "Repository Diff: Settings");
        assert_eq!(render_message(Message::ContainingBranches(1), false), "In 1 branch: ");
        assert_eq!(render_message(Message::ContainingBranches(2), false), "In 2 branches: ");
        assert_eq!(render_message(Message::LoadedCommits { count: 0, offset: 10000 }, true), "已加载 0 条提交 · 偏移 10000");
        assert_eq!(render_message(Message::FilePosition { current: 1, total: 20 }, true), "1/20 个文件");
    }
}

/// Keep the comparison identity separate from its current UI language.
pub enum ComparisonTitle {
    Label(&'static str),
    Parent { revision: String, merge: bool },
}
impl ComparisonTitle {
    pub fn text(&self) -> String {
        match self {
            Self::Label(label) => t(label).into(),
            Self::Parent { revision, merge } => match (*merge, is_chinese()) {
                (true, true) => format!("合并提交 · 与第一父提交 {revision} 比较"),
                (true, false) => format!("Merge commit · compared with first parent {revision}"),
                (false, true) => format!("与 {revision} 比较"),
                (false, false) => format!("Compared with {revision}"),
            },
        }
    }
}
