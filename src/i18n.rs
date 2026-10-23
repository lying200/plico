//! Application UI language. Repository data and raw Git output are never translated.
use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Language {
    #[default]
    System,
    Chinese,
    English,
}
static PREFERENCE: AtomicU8 = AtomicU8::new(0);
static CHINESE: AtomicU8 = AtomicU8::new(0);
impl Language {
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "zh-CN" => Self::Chinese,
            "en" => Self::English,
            _ => Self::System,
        }
    }
    pub fn encode(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Chinese => "zh-CN",
            Self::English => "en",
        }
    }
    pub fn uses_chinese(self, system_chinese: bool) -> bool {
        match self {
            Self::System => system_chinese,
            Self::Chinese => true,
            Self::English => false,
        }
    }
}
#[cfg(any(windows, test))]
fn chinese_langid(id: u16) -> bool {
    id & 0x03ff == 0x04
}
#[cfg(windows)]
fn system_chinese() -> bool {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    // Returns the current user's display language, not the keyboard or regional format.
    chinese_langid(unsafe { GetUserDefaultUILanguage() })
}
#[cfg(not(windows))]
fn system_chinese() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|key| std::env::var(key).ok().filter(|v| !v.is_empty()))
        .is_some_and(|v| {
            v.split(['-', '_', '.'])
                .next()
                .is_some_and(|s| s.eq_ignore_ascii_case("zh"))
        })
}
pub fn preference() -> Language {
    match PREFERENCE.load(Ordering::Relaxed) {
        1 => Language::Chinese,
        2 => Language::English,
        _ => Language::System,
    }
}
pub fn apply(value: Language) {
    CHINESE.store(
        u8::from(value.uses_chinese(system_chinese())),
        Ordering::Relaxed,
    );
    PREFERENCE.store(value as u8, Ordering::Relaxed);
}
pub fn is_chinese() -> bool {
    CHINESE.load(Ordering::Relaxed) != 0
}
pub fn t(english: &'static str) -> &'static str {
    translate(english, CHINESE.load(Ordering::Relaxed) != 0)
}
fn translate(english: &'static str, chinese: bool) -> &'static str {
    if !chinese {
        return if english == "Remote name" {
            "Name"
        } else {
            english
        };
    }
    match english {
        "Could not connect to hosting service" => "无法连接到代码托管服务",
        "Account needs to log in again" => "账号需要重新登录",
        "Authorization timed out" => "授权超时，请重试",
        "Browser login is unavailable in this build or for this server; use a token" => {
            "此版本或服务器暂不支持浏览器登录，请使用 Token"
        }
        "Token permissions: read_api, read_user, write_repository" => {
            "Token 所需权限：read_api、read_user、write_repository"
        }
        "Token permissions: repository access and read user" => {
            "Token 需要仓库访问和读取用户信息权限"
        }
        "Invalid server URL" => "服务器 URL 无效",
        "Use an HTTPS server URL without credentials" => "请使用不含凭据的 HTTPS 服务器 URL",
        "GitHub Enterprise is not supported" => "暂不支持 GitHub Enterprise",
        "Account changed during authorization" => "授权期间账号已更改，请重新登录",
        "Authorization was denied" => "授权被拒绝",
        "OAuth callback port is in use" => "OAuth 回调端口已被占用",
        "Could not start OAuth callback" => "无法启动 OAuth 回调",
        "OAuth callback failed" => "OAuth 回调失败",
        "Access denied: check account permissions or organization policy" => {
            "访问被拒绝，请检查账号权限或组织策略"
        }
        "OAuth authorization failed; check application configuration or log in again" => {
            "OAuth 授权失败，请检查应用配置或重新登录"
        }
        "Account storage is full" => "账号存储已满",
        "Invalid repository list" => "仓库列表无效",
        "Unexpected repository host" => "仓库地址与账号服务器不匹配",
        "Hosting response is too large" => "服务器响应过大",
        "Could not read hosting response" => "无法读取服务器响应",
        "Invalid hosting response" => "服务器响应无效",
        "Server" => "服务器：",
        "Back" => "返回",
        "Generate…" => "生成…",
        "or" => "或",
        "Token belongs to a different account" => "Token 不属于当前账号",
        "Log In to Custom Server…" => "登录自建服务器…",
        "Log In to gitlab.com…" => "登录 gitlab.com…",
        "Log In with Token…" => "使用 Token 登录…",
        "Logging in…" => "正在登录…",
        "Log in via browser…" => "通过浏览器登录…",
        "Connecting…" => "正在连接…",
        "Remove account" => "移除账号",
        "No accounts added" => "尚未添加账号",
        "Choose account:" => "选择账号：",
        "Set as default account for current project" => "设为当前项目的默认账号",
        "Use password or token…" => "使用密码或 Token…",
        "Use other credentials…" => "使用其他凭据…",
        "Credential storage is unavailable" => "凭据存储不可用",
        "Custom server OAuth applications" => "自建服务器 OAuth 应用",
        "Use one server URL = client ID per line" => "每行填写一组：服务器 URL = 客户端 ID",
        "Duplicate OAuth server" => "OAuth 服务器重复",
        "OAuth configuration is too large" => "OAuth 配置过长",
        "Log in again" => "重新登录",
        "Git credential helper is enabled; configured accounts are not used for Git operations." => {
            "已启用 Git 凭据助手；Git 操作不会使用这里配置的账号。"
        }
        "Add account…" => "添加账号…",
        "Search repositories" => "搜索仓库",
        "Callback URL" => "回调地址",
        "Configure OAuth application…" => "配置 OAuth 应用…",
        "Use token…" => "使用 Token…",
        "Log in via" => "通过浏览器登录",
        "No accounts" => "尚无账号",
        "Repository URL" => "仓库 URL",
        "Clone Repository" => "克隆仓库",
        "Clone Repository…" => "克隆仓库…",
        "Clone" => "克隆",
        "Delete" => "删除",
        "File" => "文件",
        "Enter a repository URL or directory" => "请输入仓库地址或目录",
        "Source is not a directory" => "源路径不是目录",
        "Choose an absolute destination directory" => "请选择使用绝对路径的目标目录",
        "Destination is not a directory" => "目标路径不是可用目录",
        "Destination directory is not empty" => "目标目录不为空",
        "Clone worker ended" => "克隆任务意外结束",
        "Shallow clone" => "浅克隆",
        "Clone submodules" => "克隆子模块",
        "Clone depth must be greater than zero" => "克隆深度必须大于零",
        "Clone destination directory" => "克隆目标目录",
        "Cancelling clone…" => "正在取消克隆…",
        "Cloning…" => "正在克隆…",
        "Clone cancelled" => "已取消克隆",
        "Clone failed" => "克隆失败",
        "The destination remains on disk. Inspect it before retrying." => {
            "目标目录仍保留在磁盘上，请检查后再重试。"
        }
        "Remote name" => "名称",
        "Edit Remote" => "编辑远端",
        "Shallow clone with a history truncated to" => "浅克隆，仅保留最近",
        "Edit" => "编辑",
        "Remove" => "删除",
        "Git Remotes" => "Git 远端",
        "Add Remote" => "添加远端",
        "Remove Remote" => "删除远端",
        "Remove remote" => "删除远端",
        "Could not reload remote configuration" => "无法重新读取远端配置",
        "Updating remote configuration…" => "正在更新远端配置…",
        "Remote configuration cancelled; check the current configuration" => {
            "已取消远端配置操作，请检查当前配置"
        }
        "Could not update remote configuration; some changes may already have been applied" => {
            "无法完成远端配置更新；部分修改可能已生效"
        }
        "Define Remote" => "定义远端",
        "Fetch remote" => "获取远端",
        "Checking remote access…" => "正在检查远端连接…",
        "Remote access check cancelled" => "已取消远端连接检查",
        "Could not access the remote. Check the URL and authentication, then retry." => {
            "无法访问远端。请检查地址和认证信息后重试。"
        }
        "A remote with that name already exists" => "同名远端已存在",
        "Remote changed; reopen the remote configuration" => "远端配置已变化，请重新打开配置窗口",
        "Enter a nonempty remote URL or path" => "请输入非空的远端地址或路径",
        "Could not validate the remote configuration" => "无法验证远端配置",
        "The file is still unresolved" => "文件仍处于冲突状态",
        "The merge result has been kept at the following path. Copy it before removing the recovery directory." => {
            "合并结果已保留在以下路径。删除恢复目录前，请先复制需要保留的结果。"
        }
        "Run" => "运行",
        "Skip Commit" => "跳过提交",
        "Keep Empty Commit" => "保留空提交",
        "Empty Cherry-pick" => "空拣选提交",
        "The resolved commit has no staged changes. Skip it or keep an empty commit?" => {
            "解决冲突后暂存区没有修改。跳过此提交，还是保留一个空提交？"
        }
        "Continue Cherry-pick" => "继续拣选",
        "Continue Revert" => "继续撤销提交",
        "Abort Cherry-pick" => "中止拣选",
        "Merge into Current" => "合并到当前分支",
        "Merging branch…" => "正在合并分支…",
        "Branch merged" => "分支已合并",
        "Merge completed; saved changes require attention" => "合并操作已结束，保存的修改需要处理",
        "Merge aborted; saved changes require attention" => "已中止合并，保存的修改需要处理",
        "Merge requires attention" => "合并需要处理",
        "Saved changes retained" => "已保留暂存的修改",
        "Git Merge Problem" => "Git 合并问题",
        "Don't Merge" => "不合并",
        "Local Changes Prevent Revert" => "本地修改阻止了撤销提交",
        "Save and Retry" => "保存并重试",
        "Save local changes in a stash and retry only the remaining commits below? Changes will be restored after completion; a conflict keeps them saved until you finish or abort." => {
            "将本地修改保存到 Stash，并仅重试下列剩余提交？完成后会恢复修改；若出现冲突，修改会保留到您完成或中止操作。"
        }
        "Saved Revert Changes" => "撤销提交前保存的修改",
        "Review the retained stash before retrying. Retry requires a clean tracked working tree on the original Revert branch. Clearing the record does not restore or delete the stash." => {
            "请先检查保留的 Stash。重试需要位于原撤销分支，且已跟踪文件没有本地修改。清除记录不会恢复或删除 Stash。"
        }
        "Saved Revert Queue" => "保存的撤销队列",
        "Clear saved queue" => "清除保存的队列",
        "Saved queue cleared; commits and files unchanged" => "队列已清除，提交和文件未变更",
        "These commits were pending when Revert stopped. If you finished or aborted outside Plico, clear the queue after reviewing the Log. This does not revert commits or change files." => {
            "这些提交在撤销停止时尚待处理。如果已在 Plico 外完成或中止操作，请核对日志后清除队列。此操作不会撤销提交或修改文件。"
        }
        "Finish or abort the current Git operation before clearing this queue." => {
            "请先完成或中止当前 Git 操作，再清除队列。"
        }
        "Saved Merge Changes" => "合并前保存的修改",
        "Show Stashes" => "查看 Stash",
        "Retry Restore" => "重试恢复",
        "Review the retained stash before retrying. Retry requires a clean tracked working tree and the original merge result. Clearing the record does not restore or delete the stash." => {
            "重试前请检查保留的 Stash。重试要求跟踪文件无修改且仍处于原合并结果。清除记录不会恢复或删除 Stash。"
        }
        "Smart Merge" => "智能合并",
        "Local changes would be overwritten. Smart Merge saves them, merges the branch, and restores them when the merge is complete." => {
            "本地修改将被覆盖。智能合并会先保存这些修改，合并分支，完成后恢复修改。"
        }
        "Reverted" => "已撤销",
        "Skipped" => "已跳过",
        "Remaining" => "剩余",
        "Local Changes Prevent Cherry-pick" => "本地更改阻止拣选提交",
        "Save local changes in a stash and retry? Changes will be restored after success. If the operation stops, they remain available in Stashes." => {
            "将本地更改保存到 Stash 并重试？成功后将恢复更改；如果操作中断，可在 Stash 列表中找回。"
        }
        "Cherry-Pick" => "拣选提交",
        "Cherry-picking commits…" => "正在拣选提交…",
        "Revert Commit" => "撤销提交",
        "Revert Commits" => "撤销多个提交",
        "Show Files" => "查看文件",
        "Nothing to revert" => "没有可撤销的更改",
        "Abort Revert" => "中止撤销提交",
        "Abort the operation and return to its starting state? Conflict resolutions may be lost." => {
            "中止操作并回到开始时的状态？已完成的冲突解决可能会丢失。"
        }
        "Updating operation state…" => "正在更新操作状态…",
        "Operation cancelled; repository refreshed" => "操作已取消，仓库已刷新",
        "Resolve conflicts to continue" => "解决冲突后继续",
        "Operation aborted" => "操作已中止",
        "Operation completed" => "操作已完成",
        "Operation requires attention" => "操作需要处理",
        "Resolve Conflicts…" => "解决冲突…",
        "Conflicts" => "冲突",
        "Yours" => "你的版本",
        "Theirs" => "对方版本",
        "Accept Yours" => "接受你的版本",
        "Accept Theirs" => "接受对方版本",
        "Merge…" => "合并…",
        "Modified" => "已修改",
        "Deleted" => "已删除",
        "Previous Change" => "上一处更改",
        "Next Change" => "下一处更改",
        "Discard" => "放弃",
        "Merge Conflicts" => "合并冲突",
        "Accept Left" => "采纳左侧",
        "Accept Right" => "采纳右侧",
        "Cannot show diff for a binary file" => "无法显示二进制文件的差异",
        "Merge Result" => "合并结果",
        "Accept Both" => "接受双方",
        "Keep Base" => "保留共同祖先版本",
        "Mark Change Resolved" => "标记此处已解决",
        "Apply Non-Conflicting Changes" => "应用非冲突更改",
        "Apply and Mark Resolved" => "应用并标记已解决",
        "unresolved" => "处未解决",
        "Unresolved changes" => "尚有未解决的更改",
        "Discard merge result?" => "放弃合并结果？",
        "Some changes are unresolved. Apply the current result and mark this file as resolved?" => {
            "尚有更改未解决。仍要应用当前结果并将此文件标记为已解决吗？"
        }
        "Discard your edits to the merge result? The working file will remain unchanged." => {
            "放弃对合并结果的编辑吗？工作区文件将保持不变。"
        }
        "Could not open merge editor" => "无法打开合并编辑器",
        "Edit File" => "编辑文件",
        "Mark Resolved" => "标记为已解决",
        "All conflicts are resolved" => "所有冲突已解决",
        "Resolving conflict…" => "正在解决冲突…",
        "Conflict resolution interrupted" => "冲突解决已中断",
        "Conflict resolved" => "冲突已解决",
        "Could not resolve conflict" => "无法解决冲突",
        "Could not read conflicts" => "无法读取冲突",
        "Delete…" => "删除…",
        "Delete Branch" => "删除分支",
        "Delete branch" => "删除分支",
        "Branch Is Not Fully Merged" => "分支尚未完全合并",
        "These commits are not merged into the branch's upstream or HEAD. Delete the branch anyway?" => {
            "这些提交尚未合入该分支的上游或 HEAD。仍要删除分支吗？"
        }
        "More commits are not shown" => "还有更多提交未显示",
        "Could not delete branch" => "无法删除分支",
        "Restoring branch…" => "正在恢复分支…",
        "Deleting branch…" => "正在删除分支…",
        "Branch restored" => "分支已恢复",
        "Branch deleted" => "分支已删除",
        "Branch operation failed" => "分支操作失败",
        "Restore deleted branch" => "恢复已删除分支",

        "New Branch" => "新建分支",
        "New Branch…" => "新建分支…",
        "Branch name:" => "分支名称：",
        "Create New Branch" => "创建新分支",
        "Keep Local Branch" => "保留本地分支",
        "Existing Branch Commits" => "已有分支的提交",
        "The existing branch has commits not included in the starting point." => {
            "已有分支包含起点之外的提交。"
        }
        "Branch operation completed" => "分支操作完成",
        "Overwrite existing branch" => "覆盖已有分支",
        "The existing branch has commits not included in the starting point. Drop these commits?" => {
            "已有分支包含起点之外的提交。是否丢弃这些提交？"
        }
        "Checkout branch" => "切换到新分支",
        "Create" => "创建",
        "Creating branch…" => "正在创建分支…",
        "Branch created" => "分支已创建",
        "Could not create branch" => "无法创建分支",
        "Branch operation cancelled; repository refreshed" => "分支操作已取消；仓库已刷新",
        "Rollback Changes" => "回滚更改",
        "Rollback" => "回滚",
        "Rollback…" => "回滚…",
        "Rollback… · Ctrl+Alt+Z" => "回滚… · Ctrl+Alt+Z",
        "Delete locally added files" => "删除本地新增文件",
        "changes selected" => "项更改已选中",
        "new" => "项新增",
        "modified" => "项修改",
        "deleted" => "项删除",
        "selected" => "项已选中",
        "Selected changes will be discarded. This cannot be undone in Plico." => {
            "所选更改将被丢弃，无法在 Plico 中撤销。"
        }
        "Checking changes…" => "正在检查更改…",
        "Could not prepare rollback" => "无法准备回滚",
        "Rolling back changes…" => "正在回滚更改…",
        "Changes rolled back" => "更改已回滚",
        "Rollback failed" => "回滚失败",
        "View Options" => "显示选项",
        "Group By" => "分组依据",
        "Directory · Ctrl+Alt+P" => "目录 · Ctrl+Alt+P",
        "Show" => "显示",
        "Ignored Files" => "忽略的文件",
        "Show selected change" => "定位选中的更改",

        "Previous change" => "上一处更改",
        "Next change" => "下一处更改",
        "Include in commit" => "包含到提交",
        "Exclude from commit" => "从提交中排除",
        "Include selected lines in commit" => "将所选行包含到提交",
        "Exclude selected lines from commit" => "从提交中排除所选行",
        "Commit this change" => "提交此处更改",
        "Rollback this change" => "撤回此处更改",
        "Copy original lines" => "复制原内容",
        "Git baseline changed. Reopen the change marker." => "Git 基线已变化，请重新打开变更标记。",
        "Open Project" => "打开项目",
        "Where would you like to open the project?" => "您希望在哪里打开此项目？",
        "This Window" => "当前窗口",
        "New Window" => "新窗口",
        "Shallow repository · totals cover available history" => "浅克隆仓库 · 总数仅包含已有历史",

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
        "Changes" => "更改",
        "Unversioned Files" => "未版本化文件",
        "No changes to commit." => "没有可提交的更改。",
        "Commit" => "提交",
        "Commit…" => "提交…",
        "Filters" => "筛选",
        "Amend" => "修订提交",
        "Amend last commit" => "修订上次提交",
        "Amend Commit" => "修订提交",
        "Amend Commit and Push…" => "修订提交并推送…",
        "Commit and Push…" => "提交并推送…",
        "Loading commit message…" => "正在加载提交消息…",
        "Committing…" => "正在提交…",
        "Committed" => "已提交",
        "Commit failed" => "提交失败",
        "Could not restore staged changes" => "无法恢复暂存的更改",
        "Could not load local changes" => "无法加载本地变更",
        "Could not load commit message" => "无法加载提交说明",
        "Long Edges" => "长连线",
        "Directory" => "目录",
        "Layout" => "布局",
        "Show Diff Preview" => "显示差异预览",
        "Show Details" => "显示详情",
        "Commit cancelled" => "已取消提交",
        "Commit timed out" => "提交超时",
        "Nothing to commit" => "没有可提交的更改",
        "Commit signing failed" => "提交签名失败",
        "Commit index recovery is required" => "需要恢复提交前的暂存内容",
        "Could not remove commit recovery record" => "无法删除提交恢复记录",
        "Restore staged changes" => "恢复暂存内容",
        "Restoring staged changes…" => "正在恢复暂存内容…",
        "Staged changes restored" => "暂存内容已恢复",
        "HEAD changed; reload Amend" => "HEAD 已改变，请重新开启修订提交",
        "Add to Favorites" => "添加到收藏",
        "Remove from Favorites" => "取消收藏",
        "Could not load or save favorites" => "无法读取或保存收藏设置",
        "Checkout" => "检出",
        "Copy Reference Name" => "复制引用名称",
        "Checking out branch…" => "正在检出分支…",
        "Branch checked out" => "已检出分支",
        "Could not check out branch; repository refreshed" => "无法检出分支；已刷新仓库",
        "Continue Rebase" => "继续变基",
        "Abort Rebase" => "中止变基",
        "Abort the rebase in this repository?" => "要中止此仓库中的变基吗？",
        "Updating rebase state…" => "正在更新变基状态…",
        "Rebase operation cancelled; repository refreshed" => "变基操作已取消；仓库已刷新",
        "Rebase requires conflict resolution" => "变基需要解决冲突",
        "Rebase is still in progress" => "变基尚未完成",
        "Rebase aborted" => "变基已中止",
        "Rebase completed" => "变基已完成",
        "Rebase operation failed" => "变基操作失败",
        "Rebase details" => "变基详情",
        "Complete Merge" => "完成合并",
        "Abort Merge" => "中止合并",
        "Abort" => "中止",
        "Abort the merge in this repository?" => "要中止此仓库中正在进行的合并吗？",
        "Updating merge state…" => "正在更新合并状态…",
        "Merge operation cancelled" => "已取消合并操作",
        "Merge operation failed" => "合并操作失败",
        "Merge completed" => "合并完成",
        "Merge aborted" => "已中止合并",
        "No commits selected" => "未选择提交",
        "Commit details" => "提交详情",
        "All" => "全部",
        "Push Commits to" => "推送提交至",
        "Push tags:" => "推送标签：",
        "Current Branch" => "当前分支",
        "You're going to force push to" => "即将强制推送到",
        "It may overwrite commits at the remote. Are you sure you want to proceed?" => {
            "此操作可能覆盖远端提交。确定要继续吗？"
        }
        "Don't warn about this target" => "对此目标不再提醒",
        "Force Push" => "强制推送",
        "Force Push…" => "强制推送…",
        "Push options" => "推送选项",
        "Force push is prohibited for this protected branch" => "禁止强制推送到此受保护分支",
        "Force pushing may overwrite commits at the remote. Are you sure you want to proceed?" => {
            "强制推送可能覆盖远端提交。确定要继续吗？"
        }
        "Push help" => "推送帮助",
        "Push selected commits to the target branch. Enable Push tags to include tags. Force Push checks the last known remote revision before replacing history." => {
            "将所选提交推送到目标分支。勾选推送标签可同时推送标签。强制推送会在替换历史前核对已知的远端版本。"
        }
        "Push" => "推送",
        "Push Commits" => "推送提交",
        "Pushing…" => "正在推送…",
        "Push completed" => "推送完成",
        "Check out a local branch with commits before pushing" => {
            "请先切换到已有提交的本地分支，再进行推送"
        }
        "Already up to date" => "已是最新",
        "Push rejected: remote history has changed" => "推送被拒绝：远端历史已变化",
        "Push rejected: stale remote information" => "推送被拒绝：远端信息已过期",
        "Push rejected by remote" => "远端拒绝了推送",
        "Push cancelled; remote changes may have completed" => "已取消推送；部分远端更新可能已完成",
        "Push failed or incomplete; some remote updates may have completed" => {
            "推送失败或未完成；部分远端更新可能已完成"
        }
        "Pop" => "弹出",
        "Restore changes and keep the stash" => "恢复更改并保留保存记录",
        "Restore changes and remove the stash on success" => "恢复更改，成功后移除保存记录",
        "Loading…" => "正在加载…",
        "Could not load comparison. Refresh to retry." => "无法加载比较，请刷新重试。",
        "Comparison unavailable. Refresh the Changes panel to retry." => {
            "比较内容不可用，请在更改面板中刷新重试。"
        }
        "Saved changes from Squash still need review." => "Squash 保存的更改仍需确认处理。",
        "Already handled externally…" => "已在外部处理…",
        "Could not inspect the Squash recovery record." => "无法读取 Squash 恢复记录。",
        "Finish Squash recovery" => "结束 Squash 恢复",
        "Confirm only after handling the saved changes externally. This clears the app recovery record; it does not restore or delete any stash or change your files." => {
            "请在外部处理好保存的更改后再确认。此操作只清除应用恢复记录，不会恢复或删除 stash，也不会修改文件。"
        }
        "Clear recovery record" => "清除恢复记录",
        "Clearing recovery record…" => "正在清除恢复记录…",
        "Recovery record cleared; saved changes kept" => "恢复记录已清除，保存的更改已保留",
        "Recovery record was not cleared. Refresh and review the repository state." => {
            "未清除恢复记录，请刷新并检查仓库状态。"
        }
        "Refresh" => "刷新",
        "Press again to go to the next file" => "再次操作以转到下一个文件",
        "Press again to go to the previous file" => "再次操作以转到上一个文件",
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
        "Counting commits…" => "正在统计提交…",
        "Retry count" => "重试统计",
        "Loading remaining history…" => "正在加载其余历史…",
        "Pause loading" => "暂停加载",
        "Resume loading" => "继续加载",
        "All commits loaded" => "已加载全部提交",
        "More file history is available" => "还有更早的文件历史",
        "No history for this file" => "此文件没有 Git 历史",
        "Loading file history…" => "正在加载文件历史…",
        "Could not load file history" => "无法加载文件历史",
        "Jump to Source · F4" => "跳转到源码 · F4",
        "Jump to Commit" => "跳转到提交",
        "Commit is outside the loaded history window" => "提交不在已加载的历史窗口中",
        "History belongs to another repository; reopen its file" => {
            "此历史属于另一个仓库，请重新打开对应文件"
        }
        "Could not jump to source" => "无法跳转到源码",
        "Details" => "详情",
        "Error details" => "错误详情",
        "Could not load saved changes" => "无法加载已保存的更改",
        "Git error details" => "Git 错误详情",
        "Repository access blocked" => "仓库访问被阻止",
        "Could not load repository data" => "无法加载仓库数据",
        "Git blocked this repository because its owner differs" => {
            "Git 因仓库所有者不一致而阻止了访问"
        }
        "Git refused access because the repository owner differs. Review the path and Git's instructions before trusting this directory. No trust configuration has been changed." => {
            "Git 因仓库所有者不一致而拒绝访问。信任此目录前，请核对路径及下方 Git 提示。程序未修改信任配置。"
        }
        "Operation failed" => "操作失败",
        "Could not load changes. Refresh to retry." => "无法加载变更，请刷新重试。",
        "Could not load the diff." => "无法加载差异。",
        "Reload diff" => "重新加载差异",
        "Could not save Diff preferences. Try the setting again." => {
            "无法保存差异设置，请重试该设置。"
        }
        "Copy error details" => "复制错误详情",
        "Compare Versions" => "比较版本",
        "No differences" => "没有差异",
        "Go to Parent Commit" => "转到父提交",
        "Go to Child Commit" => "转到子提交",
        "Select Parent Commit" => "选择父提交",
        "Select Child Commit" => "选择子提交",
        "Undo Squash" => "撤销压缩",
        "Undoing Squash…" => "正在撤销压缩…",
        "Squash undone" => "已撤销压缩",
        "Could not undo Squash" => "无法撤销压缩",
        "Reset Current Branch to Here…" => "将当前分支重置到此处…",
        "Resetting… This operation cannot be cancelled once started." => {
            "正在重置…实际写入开始后不可取消。"
        }
        "Only line separators differ" => "仅换行符不同",
        "Edit Commit Message…" => "编辑提交信息…",
        "Edit Commit Message" => "编辑提交信息",
        "Edit message for commit" => "编辑提交信息：",
        "by" => "作者",
        "Editing Initial Commit" => "编辑初始提交",
        "Editing the initial commit rewrites the history of the current branch. Continue?" => {
            "编辑初始提交将重写当前分支的历史。是否继续？"
        }
        "Editing commit message…" => "正在编辑提交信息…",
        "Commit message edited" => "提交信息已修改",
        "Commit message edited; follow-up failed" => "提交信息已修改，但后续处理失败",
        "Could not edit commit message" => "无法编辑提交信息",
        "This will reset the current branch head to the selected commit,\nand update the working tree and the index according to the selected mode:" => {
            "将当前分支的 HEAD 重置到选中的提交，\n并根据所选模式更新工作区和暂存区："
        }
        "Files will be reverted to the state of the selected commit.\nWarning: any local changes will be lost." => {
            "文件将恢复到选中提交的状态。\n警告：所有本地更改将丢失。"
        }
        "Files will be reverted to the state of the selected commit,\nbut local changes will be kept intact." => {
            "文件将恢复到选中提交的状态，\n但会保留本地更改。"
        }
        "Git Reset" => "Git 重置",
        "Reset" => "重置",
        "Resetting…" => "正在重置…",
        "Reset successful" => "重置成功",
        "Reset failed" => "重置失败",
        "Reset the current branch to the selected commit:" => "将当前分支重置到选中的提交：",
        "Files won't change, differences will be staged for commit." => {
            "文件保持不变，差异将暂存以供提交。"
        }
        "Files won't change, differences won't be staged." => "文件保持不变，差异不会暂存。",
        "Files will be reverted to the selected commit. Warning: local changes will be lost." => {
            "文件将恢复到选中的提交。警告：本地更改将丢失。"
        }
        "Files will be reverted to the selected commit, keeping local changes intact." => {
            "文件将恢复到选中的提交，并保留本地更改。"
        }
        "Squash Commits" => "压缩提交",
        "Squash Commits…" => "压缩提交…",
        "Message for the squashed commit:" => "压缩后的提交消息：",
        "Checking…" => "正在检查…",
        "Cannot edit selected commits: repository state was changed" => {
            "无法编辑所选提交：仓库状态已发生变化"
        }
        "Cannot edit selected commits: pushed to protected branch" => {
            "无法编辑所选提交：已推送到受保护分支"
        }
        "Cannot edit selected commits: another Git operation is in progress" => {
            "无法编辑所选提交：另一个 Git 操作正在进行"
        }
        "Cannot edit selected commits: recover the previous operation first" => {
            "无法编辑所选提交：请先恢复上一次操作"
        }
        "Cannot validate selected commits. Close this dialog and refresh the log." => {
            "无法验证所选提交。请关闭此对话框并刷新日志。"
        }
        "Preparing Squash…" => "正在准备压缩提交…",
        "Squashing commits…" => "正在压缩提交…",
        "Commits squashed" => "已压缩提交",
        "Could not squash commits; repository refreshed" => "未能完成压缩提交；已刷新仓库",
        "Restoring saved changes…" => "正在恢复已保存的更改…",
        "Saved changes restored" => "已恢复保存的更改",
        "Saved changes restored; recovery record needs attention" => {
            "已恢复保存的更改；恢复记录仍需处理"
        }
        "Saved changes have conflicts; review Local Changes" => "恢复时发生冲突，请查看本地更改",
        "Could not restore the saved index; review Local Changes" => {
            "无法恢复暂存状态，请查看本地更改"
        }
        "Local changes would be overwritten; stash kept" => "本地更改会被覆盖，保存记录已保留",
        "Untracked files would be overwritten; stash kept" => "未跟踪文件会被覆盖，保存记录已保留",
        "Restore cancelled; check Local Changes before retrying" => {
            "已取消恢复，重试前请检查本地更改"
        }
        "Restore failed or incomplete; review Local Changes" => "恢复失败或未完成，请查看本地更改",
        "Could not restore saved changes" => "无法恢复已保存的更改",
        "Show saved changes" => "查看已保存的更改",
        "Stash" => "Stash",
        "Drop Stash…" => "删除 Stash…",
        "Drop Stash" => "删除 Stash",
        "Delete this saved stash? Its changes will not be restored." => {
            "删除此 Stash？其中保存的更改不会被恢复。"
        }
        "Dropping stash…" => "正在删除 Stash…",
        "Stash deleted" => "已删除 Stash",
        "Could not delete stash" => "无法删除 Stash",
        "Stashes" => "已保存的更改",
        "No stashes" => "没有已保存的更改",
        "Only the first 1000 stashes are shown" => "仅显示前 1000 条已保存的更改",
        "Working tree" => "工作区",
        "Index" => "暂存区",
        "Untracked files" => "未跟踪文件",
        "Merge result" => "合并结果",
        "Parent revision" => "父版本",
        "First parent" => "第一父提交",
        "Empty tree" => "空目录树",
        "Empty file" => "空文件",
        "Unmerged" => "未合并",
        "Untracked working file" => "未跟踪的工作区文件",
        "Staged" => "已暂存",
        "Loading saved changes…" => "正在加载已保存的更改…",
        "Get" => "获取此版本",
        "Get Revision" => "获取历史版本",
        "Overwrite Modified File" => "覆盖已修改的文件",
        "This file has been locally modified." => "此文件已在本地修改。",
        "Loading file revision…" => "正在读取文件历史版本…",
        "Restoring file revision…" => "正在恢复文件历史版本…",
        "File revision restored" => "文件已恢复为所选版本",
        "Could not restore file revision" => "无法恢复文件历史版本",
        "Copy Revision Number" => "复制版本号",
        "Select commits to view changes" => "选择提交以查看更改",
        "Showing the first 50 commits" => "仅显示前 50 条提交详情",
        "commits" => "条提交",
        "No changes" => "没有变更",
        "Show Diff" => "显示差异",
        "Could not open file diff" => "无法打开文件差异",
        "Set upstream" => "设为上游",
        "New upstream" => "新上游",
        "Apply" => "应用",
        "Previous" => "上一页",
        "Next" => "下一页",
        "New branch" => "新分支",
        "Push references changed; reopen the push dialog" => {
            "远端引用已变化，请重新打开推送窗口以刷新列表"
        }
        "Smart Checkout" => "智能检出",
        "Force Checkout" => "强制检出",
        "Don't Checkout" => "不检出",
        "Checkout Problem" => "检出问题",
        "Save local changes, check out, then restore them" => "保存本地修改，检出后恢复修改",
        "Check out and overwrite local changes" => "检出并覆盖本地修改",
        "Local changes would be overwritten. Smart Checkout saves and restores them." => {
            "本地修改将被覆盖。智能检出会先保存修改，检出后再恢复。"
        }
        "The affected file list may be incomplete" => "受影响文件列表可能不完整",
        "Local changes were not fully restored; review conflicts and saved stash" => {
            "本地修改未完全恢复，请检查冲突和保留的 stash"
        }
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
        "Untracked files would be overwritten by checkout" => {
            "检出会覆盖未跟踪文件，请先处理这些文件"
        }
        "Checking out commit…" => "正在检出提交…",
        "Checkout cancelled; repository refreshed" => "已取消检出，仓库状态已刷新",
        "Commit checked out (detached HEAD)" => "已检出提交（分离 HEAD）",
        "Could not check out commit; repository refreshed" => "无法检出提交，仓库状态已刷新",
        "Could not load outgoing commits" => "无法加载待推送提交，请检查目标",
        "Loading commit details…" => "正在加载提交详情…",
        "Could not load commit details" => "无法加载提交详情",
        "Select a commit to view changes" => "选择提交以查看变更",
        "Loading outgoing commits…" => "正在加载待推送提交…",
        "No outgoing commits" => "没有待推送提交",
        "New branch preview is limited to 1,000 commits" => "新分支预览最多显示 1,000 条提交",
        "Pull" => "拉取",
        "Pulling…" => "正在拉取…",
        "Pull completed" => "拉取完成",
        "Pull cancelled; local changes remain saved in stash" => {
            "已取消拉取；本地修改仍保存在 stash 中"
        }
        "Pull incomplete; local changes remain saved in stash" => {
            "拉取未完整完成；本地修改仍保存在 stash 中"
        }
        "Pull failed" => "拉取失败",
        "Pull cancelled" => "已取消拉取",
        "Pull requires conflict resolution" => "拉取后存在待解决的冲突",
        "Pull left an unfinished operation" => "拉取后仍有待完成的 Git 操作",
        "Loading pull targets…" => "正在读取拉取目标…",
        "Could not read pull targets" => "无法读取拉取目标",
        "Select remote" => "选择远端",
        "Modify options" => "修改选项",
        "Select an existing remote branch" => "请选择已有的远端分支",
        "Git editor helper is unavailable" => "Git 编辑器辅助程序不可用",
        "Edit commit message" => "编辑提交说明",
        "Commit message" => "提交说明",
        "Edit rebase todo" => "编辑变基任务",
        "Could not save. The file may have changed or the operation has ended." => {
            "无法保存：文件可能已更改，或操作已经结束。"
        }
        "Git review" => "Git 审阅",
        "Local repositories · Git review · Ctrl+O" => "本地仓库 · Git 审阅 · Ctrl+O",
        "Checking checkout target…" => "正在检查切换目标…",
        "Checkout Remote Branch" => "切换远端分支",
        "Checkout and Rebase" => "切换并变基",
        "Drop Local Commits" => "丢弃本地提交",
        "The local branch name is already in use or tracks another remote branch. Choose a new name." => {
            "本地分支名已被使用或正在跟踪其他远端分支。请选择新的名称。"
        }
        "The local branch has commits not present in the remote branch. Rebase them onto the remote branch, or drop them and reset the local branch to the remote tip." => {
            "本地分支包含远端分支中不存在的提交。可以将它们变基到远端分支，或丢弃这些提交并将本地分支重置到远端最新提交。"
        }
        "Fetch" => "获取",
        "Fetching…" => "正在获取…",
        "Fetch completed" => "获取完成",
        "Fetch cancelled" => "已取消获取",
        "Fetch failed" => "获取失败",
        "No remotes configured" => "未配置远端",
        "Authentication helper is unavailable" => "认证助手不可用",
        "Git authentication" => "Git 认证",
        "Log in to" => "登录到",
        "Log In" => "登录",
        "Retry" => "重试",
        "SSH Key Passphrase" => "SSH 密钥口令",
        "SSH Password" => "SSH 密码",
        "SSH PIN" => "SSH PIN",
        "Authorization response received" => "已收到授权响应",
        "Return to Plico to view the sign-in result." => "请返回 Plico 查看登录结果。",
        "You can close this browser tab." => "现在可以关闭此浏览器标签页。",
        "SSH Confirmation" => "SSH 主机确认",
        "Yes" => "是",
        "No" => "否",
        "SSH Authentication" => "SSH 身份验证",
        "Password for SSH key:" => "SSH 密钥口令：",
        "SSH password for:" => "SSH 用户密码：",
        "Password or token" => "密码或 Token",
        "Remember" => "记住",
        "Remember for this session" => "在本次会话中记住",
        "Authentication" => "身份验证",
        "Applies to all repositories" => "应用于所有仓库",
        "Use credentials helper" => "使用凭据助手",
        "Credential storage is unavailable. Clear Remember to continue without saving." => {
            "凭据存储不可用。取消“记住”可继续且不保存。"
        }
        "Enter your credentials" => "请输入凭据",
        "Username must not be empty" => "用户名不能为空",
        "Password must not be empty" => "密码不能为空",
        "Username" => "用户名",
        "Password" => "密码",
        "Username and password must not be empty" => "用户名和密码不能为空",
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
        "Copy Path" => "复制路径",
        "Absolute Path" => "绝对路径",
        "Path From Project Root" => "项目相对路径",
        "File Name" => "文件名",
        "Always Select Opened File" => "始终定位打开的文件",
        "Select Opened File" => "定位当前打开文件",
        "File is not available in the project tree" => "文件不在当前项目树中或已不可用",
        "Show in File Manager" => "在文件浏览器中打开",
        "updating…" => "正在更新…",
        "Loading local changes…" => "正在加载本地更改…",
        "Project" => "项目",
        "Loading files…" => "正在加载文件…",
        "Loading file…" => "正在加载文件…",
        "Editor Tabs" => "编辑器标签页",
        "Enable preview tab" => "启用预览标签页",
        "Select a file to preview it. Double-click or press Enter to keep it open." => {
            "选择文件以预览。双击或按 Enter 将其固定打开。"
        }
        "Saving…" => "正在保存…",
        "Name" => "姓名",
        "Email" => "邮箱",
        "Set properties globally" => "设为全局配置",
        "Git name and email are not configured" => "尚未配置 Git 姓名和邮箱",
        "Set and Commit" => "设置并提交",
        "Enter a name and an email address containing @" => "请输入姓名及包含 @ 的邮箱地址",
        "Could not configure Git identity. Check configuration permissions and repository overrides." => {
            "无法配置 Git 身份，请检查配置权限及仓库中的覆盖设置。"
        }
        "Could not read Git identity" => "无法读取 Git 身份配置",
        "Unsaved changes" => "未保存的更改",
        "Automatically saved" => "自动保存",
        "Could not save file" => "无法保存文件",
        "Could not reload file" => "无法重新加载文件",
        "File changed while reloading. Your edits have been kept." => {
            "重新加载期间文件内容发生了变化，已保留你的编辑。"
        }
        "Reload from Disk" => "从磁盘重新加载",
        "Discard your unsaved changes and reload this file from disk?" => {
            "放弃未保存的更改，并从磁盘重新加载此文件？"
        }
        "Could not refresh diff" => "无法刷新差异",
        "Copy editor content" => "复制编辑器内容",
        "File is unavailable on disk. Your content has been kept." => {
            "磁盘上的文件不可用，已保留编辑器内容。"
        }
        "File changed on disk. Your edits have been kept; reload or compare before saving." => {
            "磁盘上的文件已更改。你的编辑已保留，请检查后重新加载。"
        }
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
        "Update cancelled; review local changes and saved stash" => {
            "更新已取消，请检查本地改动和保存的暂存"
        }
        "Update incomplete; review conflicts and saved local changes" => {
            "更新未完成，请检查冲突和保存的本地改动"
        }
        "Merge incoming changes into the current branch" => "将传入更改合并到当前分支",
        "Don't show again" => "不再显示",
        "Show Update Project options" => "显示更新项目选项",
        "Main menu" => "主菜单",
        "Editor Font" => "编辑器字体",
        "Font:" => "字体：",
        "Size:" => "字号：",
        "Preview" => "预览",
        "Editor only" => "仅编辑器",
        "Editor and Preview" => "编辑器与预览",
        "Preview only" => "仅预览",
        "Restore Defaults" => "恢复默认值",
        "Settings" => "设置",
        "Close" => "关闭",
        "Language" => "语言",
        "Follow system" => "跟随系统",
        "Version Control · Diff" => "版本控制 · 差异",
        "Open file differences in" => "文件差异打开方式",
        "Editor tab (default)" => "独立标签页（默认）",
        "Bottom preview" => "底部预览",
        "Open…" => "打开…",
        "Log" => "日志",
        "Local Changes" => "本地更改",
        "Close diff · Esc" => "关闭差异 · Esc",
        "Refresh (Ctrl+F5)" => "刷新（Ctrl+F5）",
        "Select Period" => "选择时间范围",
        "OK" => "确定",
        "Branch" => "分支",
        "HEAD (Current Branch)" => "HEAD（当前分支）",
        "No matching branches" => "没有匹配的分支",
        "Regular expression" => "正则表达式",
        "Match case" => "区分大小写",
        "Shallow boundary" => "浅克隆边界",
        "Newer window" => "较新的历史",
        "Return to the preceding history window in this snapshot" => "返回此快照的上一段历史",
        "Back to latest" => "返回最新",
        "Show Diff · Ctrl+D" => "显示差异 · Ctrl+D",
        "Expand All" => "全部展开",
        "Collapse All" => "全部折叠",
        "Parents" => "父提交",
        "Unavailable (shallow boundary)" => "不可用（浅克隆边界）",
        "Root commit" => "初始提交",
        "Reading Git data…" => "正在读取 Git 数据…",
        "Previous change (Shift+F7)" => "上一处更改（Shift+F7）",
        "Next change (F7)" => "下一处更改（F7）",
        "Browse history. Review changes." => "浏览历史，审阅更改。",
        "Open Repository…" => "打开仓库…",
        "Local repositories · Read-only · Ctrl+O" => "本地仓库 · 只读浏览 · Ctrl+O",
        "Shallow boundary · Parent comparison unavailable" => "浅克隆边界 · 无法与父提交比较",
        "Local changes · staged and unstaged are separate comparisons" => {
            "本地变更 · 已暂存与未暂存内容分别比较"
        }
        "Unstaged · Index → Working tree" => "未暂存 · 暂存区 → 工作区",
        "Staged · HEAD → Index" => "已暂存 · HEAD → 暂存区",
        "Untracked · New file preview" => "未跟踪 · 新文件预览",
        "Conflict · Unmerged index entries" => "冲突 · 暂存区存在未合并条目",
        "Committed change" => "已提交变更",
        "No commits match the message filter. Clear the filter field and press Enter to show all history." => {
            "没有提交匹配消息筛选。清空筛选框并按 Enter 显示全部历史。"
        }
        "No local changes. The index and working tree are clean." => {
            "没有本地变更。暂存区和工作区均干净。"
        }
        "Parent history is unavailable at this shallow boundary. Deepen the clone with your Git tool, then refresh to review its changes." => {
            "此浅克隆边界没有可用的父提交历史。请使用 Git 工具加深克隆，再刷新查看变更。"
        }
        "No file changes compared with the first parent. Other merge parents may differ." => {
            "与第一父提交相比没有文件变更，其他合并父提交可能不同。"
        }
        "This commit has no file changes compared with its parent or the empty tree." => {
            "此提交与父提交或空树相比没有文件变更。"
        }
        "No commits in this history. Use Local Changes to review files before the first commit." => {
            "此历史中没有提交。首次提交前可在“本地变更”中查看文件。"
        }
        "No patch was returned for this file. Refresh to check for changes made outside this viewer." => {
            "此文件没有返回补丁。请刷新以检查外部修改。"
        }
        "Select a changed file to review its diff" => "选择已变更文件以查看差异",
        "Cannot save Diff settings" => "无法保存差异设置",
        "Cannot save Diff view" => "无法保存差异视图",
        "Expand" => "展开",
        "Restore" => "还原",
        "Submodule reference changed · The diff shows commit IDs, not the files inside the submodule." => {
            "子模块引用已变更 · 差异显示提交 ID，不显示子模块内部文件。"
        }
        "Read-only" => "只读浏览",
        "Clear filters and show" => "清除筛选并显示",
        "Dismiss" => "关闭提示",
        "Ctrl+D  Open diff    F7  Next change    Ctrl+F5  Refresh" => {
            "Ctrl+D  打开差异    F7  下一处更改    Ctrl+F5  刷新"
        }
        "Settings could not be saved" => "无法保存设置",
        "Highlighting Differences" => "差异高亮",
        "Lines" => "行",
        "Words" => "单词",
        "Characters" => "字符",
        "Appearance" => "外观",
        "Show Line Numbers" => "显示行号",
        "Soft-Wrap" => "自动换行",
        "Side-by-side viewer" => "双栏对比",
        "Unified viewer" => "单栏对比",
        "Text or hash" => "文本或哈希",
        "Branches and Tags" => "分支和标签",
        "User" => "用户",
        "Paths" => "路径",
        "Date" => "日期",
        "Local" => "本地",
        "All branches" => "所有分支",
        "All dates" => "所有日期",
        "Last 24 hours" => "最近 24 小时",
        "Last 7 days" => "最近 7 天",
        "Select…" => "选择…",
        "Select in Tree…" => "在树中选择…",
        "Version Control · Git" => "版本控制 · Git",
        "Applies to the current worktree" => "仅应用于当前工作树",
        "Auto-update if push is rejected" => "推送被拒绝时自动更新",
        "Update" => "更新",
        "Update method" => "更新方式",
        "Follow branch configuration" => "跟随分支配置",
        "Remember this choice for future push rejections (change in Settings)" => {
            "记住选择，用于以后推送被拒绝时（可在设置中修改）"
        }
        "Push Rejected" => "推送被拒绝",
        "Update Confirmation" => "确认更新",
        "Merge" => "合并",
        "Rebase" => "变基",
        "Rebase Anyway" => "仍然变基",
        "Remote history has changed. Update before pushing again." => {
            "远端历史已变化，请更新后再次推送。"
        }
        "Rebase may discard changes made in merge commits. Merge instead or rebase anyway?" => {
            "变基可能丢失合并提交中的修改。改为合并，还是仍然变基？"
        }
        "Could not check merge commits. Choose how to update or cancel." => {
            "无法检查合并提交，请选择更新方式或取消。"
        }
        "Rebase will rewrite published commits. Rebase anyway?" => {
            "变基将改写已发布的提交。仍然变基？"
        }
        "Could not check published commits. Rebase anyway?" => "无法检查已发布的提交。仍然变基？",
        "Push update cancelled; local changes remain saved in stash" => {
            "已取消推送更新，本地修改仍保存在 stash 中"
        }
        "Loading settings…" => "正在加载设置…",
        "Version Control" => "版本控制",
        "Diff" => "差异",
        "Open a repository to configure Git" => "打开仓库后可配置 Git",
        "Git settings are unavailable during an operation" => "操作进行中，暂时无法加载 Git 设置",
        "Rebasing Merge Commits" => "变基合并提交",
        "Push update cancelled" => "已取消推送更新",
        "Update incomplete; review conflicts and saved local changes before pushing" => {
            "更新未完成，请先检查冲突和已保存的本地修改，再推送"
        }
        "Select Paths to Filter by" => "选择要筛选的路径",
        "Cancel" => "取消",
        "Recent" => "最近使用",
        "me" => "我",
        "Loading users…" => "正在加载用户…",
        "After" => "晚于",
        "Before" => "早于",
        "Since" => "自",
        "Until" => "截至",
        "Hide branches" => "隐藏分支",
        "Show branches" => "显示分支",
        "Select one or more values separated with new lines, use Ctrl+Enter to finish" => {
            "每行输入一个值，按 Ctrl+Enter 完成"
        }
        "Select one or more values separated with | or new lines, use Ctrl+Enter to finish" => {
            "用 | 或换行分隔多个值，按 Ctrl+Enter 完成"
        }
        "Older window" => "较早的历史",
        "Load more" => "加载更多",
        "  · Shallow history boundary" => "  · 浅克隆历史边界",
        "  · End of available history" => "  · 已到历史末尾",
        "In branches: loading…" => "所属分支：正在加载…",
        "Not in any branch" => "不在任何分支中",
        "Hide" => "收起",
        "Show all" => "显示全部",
        "Tags" => "标签",
        "Remote" => "远程",
        "No textual differences" => "没有文本差异",
        "Previous file" => "上一个文件",
        "Next file" => "下一个文件",
        "Search files" => "搜索文件",
        "Previous difference · Shift+F7" => "上一处差异 · Shift+F7",
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
        for id in [0x0404, 0x0804, 0x0c04, 0x1004, 0x1404] {
            assert!(chinese_langid(id));
        }
        for id in [0, 0x0409, 0x0411, 0x1400] {
            assert!(!chinese_langid(id));
        }
        for system in [true, false] {
            assert!(Language::Chinese.uses_chinese(system));
            assert!(!Language::English.uses_chinese(system));
            assert_eq!(Language::System.uses_chinese(system), system);
        }
        for value in ["system", "", "broken", "zh-CN-overlong"] {
            assert_eq!(Language::parse(value), Language::System);
        }
        for language in [Language::System, Language::Chinese, Language::English] {
            assert_eq!(Language::parse(language.encode()), language);
        }
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
    LoadedCommits {
        count: usize,
        offset: usize,
    },
    FilePosition {
        current: usize,
        total: usize,
    },
    DiffCount {
        total: usize,
        included: Option<usize>,
    },
    ContainingBranches(usize),
    RepositoryDiff(&'a str),
    AuthorDate {
        author: &'a str,
        date: &'a str,
    },
    Committer {
        person: Option<&'a str>,
        date: Option<&'a str>,
    },
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
        (Message::NotInHistory(name), false) => {
            format!("{name} was not found in repository history")
        }
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
        (Message::LoadedCommits { count, offset }, true) => {
            format!("已加载 {count} 条提交 · 偏移 {offset}")
        }
        (Message::LoadedCommits { count, offset }, false) => {
            format!("{count} commits loaded · offset {offset}")
        }
        (Message::FilePosition { current, total }, true) => format!("{current}/{total} 个文件"),
        (Message::FilePosition { current, total }, false) => format!("{current}/{total} files"),
        (Message::DiffCount { total, included }, chinese) => {
            let mut text = if chinese {
                format!("{total} 处差异")
            } else {
                format!(
                    "{total} {}",
                    if total == 1 {
                        "difference"
                    } else {
                        "differences"
                    }
                )
            };
            if let Some(n) = included {
                text.push_str(&if chinese {
                    format!("，已包含 {n} 处")
                } else {
                    format!(", {n} included")
                });
            }
            text
        }
        (Message::ContainingBranches(count), true) => format!("位于 {count} 个分支："),
        (Message::ContainingBranches(count), false) => format!(
            "In {count} {}: ",
            if count == 1 { "branch" } else { "branches" }
        ),
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
        assert_eq!(
            render_message(Message::RepositoryDiff(name), true),
            format!("仓库差异：{name}")
        );
        assert_eq!(
            render_message(Message::RepositoryDiff("Settings"), false),
            "Repository Diff: Settings"
        );
        assert_eq!(
            render_message(Message::ContainingBranches(1), false),
            "In 1 branch: "
        );
        assert_eq!(
            render_message(Message::ContainingBranches(2), false),
            "In 2 branches: "
        );
        assert_eq!(
            render_message(
                Message::LoadedCommits {
                    count: 0,
                    offset: 10000
                },
                true
            ),
            "已加载 0 条提交 · 偏移 10000"
        );
        assert_eq!(
            render_message(
                Message::FilePosition {
                    current: 1,
                    total: 20
                },
                true
            ),
            "1/20 个文件"
        );
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

#[cfg(test)]
mod setup_translation_tests {
    #[test]
    fn remote_and_author_names_have_distinct_context() {
        assert_eq!(super::translate("Remote name", true), "名称");
        assert_eq!(super::translate("Name", true), "姓名");
        assert_eq!(super::translate("Remote name", false), "Name");
        assert_eq!(super::translate("Name", false), "Name");
    }
}

/// Counts refer to the displayed immutable query, not to unfiltered repository HEAD.
pub fn history_count(loaded: usize, offset: usize, total: Option<usize>) -> String {
    let total = total.map(|n| n.to_string()).unwrap_or_else(|| "—".into());
    if offset == 0 || loaded == 0 {
        if is_chinese() {
            format!("已加载 {loaded} / {total} 条提交")
        } else {
            format!("{loaded} of {total} commits loaded")
        }
    } else if is_chinese() {
        format!(
            "当前 {loaded} 条 · 第 {}–{} 条 / 共 {total} 条",
            offset + 1,
            offset + loaded
        )
    } else {
        format!(
            "{loaded} loaded · {}–{} of {total} commits",
            offset + 1,
            offset + loaded
        )
    }
}
