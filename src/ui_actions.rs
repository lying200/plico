//! Shared window/editor commands. Key bindings are installed by ui::init.
use gpui_kit::actions;

actions!(
    review,
    [
        Refresh,
        ToggleLogDirectory,
        Open,
        OpenSettings,
        CreateBranch,
        Older,
        Newer,
        ExtendOlder,
        ExtendNewer,
        GoParent,
        GoChild,
        NextFile,
        PreviousFile,
        Expand,
        Back,
        CloseFile,
        NextChange,
        PreviousChange,
        JumpToSource,
        CopyHash,
        EditCommitMessage,
        Find
    ]
);
