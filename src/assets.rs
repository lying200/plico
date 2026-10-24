use gpui_kit::{AssetSource, Result, SharedString};
use std::borrow::Cow;
pub struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        let data: Option<&'static [u8]> = match path {
            "jb/cherryPick_dark.svg" => Some(include_bytes!("../assets/icons/cherryPick_dark.svg")),
            "jb/download_dark.svg" => Some(include_bytes!("../assets/icons/download_dark.svg")),
            "jb/layoutEditorOnly_dark.svg" => {
                Some(include_bytes!("../assets/icons/layoutEditorOnly_dark.svg"))
            }
            "jb/layoutEditorPreview_dark.svg" => Some(include_bytes!(
                "../assets/icons/layoutEditorPreview_dark.svg"
            )),
            "jb/layoutPreviewOnly_dark.svg" => {
                Some(include_bytes!("../assets/icons/layoutPreviewOnly_dark.svg"))
            }
            "jb/defaultAvatar_dark.svg" => {
                Some(include_bytes!("../assets/icons/defaultAvatar_dark.svg"))
            }
            "jb/checked_dark.svg" => Some(include_bytes!("../assets/icons/checked_dark.svg")),
            "jb/github_dark.svg" => Some(include_bytes!("../assets/icons/github_dark.svg")),
            "jb/gitlab.svg" => Some(include_bytes!("../assets/icons/gitlab.svg")),
            "jb/checkBox.svg" => Some(include_bytes!("../assets/icons/checkBox.svg")),
            "jb/checkBoxSelected.svg" => {
                Some(include_bytes!("../assets/icons/checkBoxSelected.svg"))
            }
            "jb/checkBoxIndeterminateSelected.svg" => Some(include_bytes!(
                "../assets/icons/checkBoxIndeterminateSelected.svg"
            )),
            "jb/checkBoxDisabled.svg" => {
                Some(include_bytes!("../assets/icons/checkBoxDisabled.svg"))
            }
            "jb/checkBoxSelectedDisabled.svg" => Some(include_bytes!(
                "../assets/icons/checkBoxSelectedDisabled.svg"
            )),
            "jb/checkBoxIndeterminateSelectedDisabled.svg" => Some(include_bytes!(
                "../assets/icons/checkBoxIndeterminateSelectedDisabled.svg"
            )),
            "jb/moreVertical_dark.svg" => {
                Some(include_bytes!("../assets/icons/moreVertical_dark.svg"))
            }
            "jb/close_dark.svg" | "icons/close.svg" | "icons/x.svg" => {
                Some(include_bytes!("../assets/icons/close_dark.svg"))
            }
            "icons/chevron-down.svg" => {
                Some(include_bytes!("../assets/icons/chevronDown_dark.svg"))
            }
            "icons/chevron-up.svg" => Some(include_bytes!("../assets/icons/chevronUp_dark.svg")),
            "icons/chevron-left.svg" => {
                Some(include_bytes!("../assets/icons/chevronLeft_dark.svg"))
            }
            "icons/chevron-right.svg" => {
                Some(include_bytes!("../assets/icons/chevronRight_dark.svg"))
            }

            "jb/gutterCheckBoxIndeterminate.svg" => Some(include_bytes!(
                "../assets/icons/gutterCheckBoxIndeterminate.svg"
            )),
            "jb/gutterCheckBoxSelected.svg" => {
                Some(include_bytes!("../assets/icons/gutterCheckBoxSelected.svg"))
            }
            "jb/gutterCheckBox.svg" => Some(include_bytes!("../assets/icons/gutterCheckBox.svg")),
            "jb/incoming_dark.svg" => Some(include_bytes!("../assets/icons/incoming_dark.svg")),
            "jb/outgoing_dark.svg" => Some(include_bytes!("../assets/icons/outgoing_dark.svg")),
            "jb/merge-accept-left.svg" => {
                Some(include_bytes!("../assets/icons/merge-accept-left.svg"))
            }
            "jb/merge-append-left.svg" => {
                Some(include_bytes!("../assets/icons/merge-append-left.svg"))
            }
            "jb/merge-append-right.svg" => {
                Some(include_bytes!("../assets/icons/merge-append-right.svg"))
            }
            "jb/merge-ignore.svg" => Some(include_bytes!("../assets/icons/merge-ignore.svg")),
            "jb/diff-apply-right_dark.svg" => {
                Some(include_bytes!("../assets/icons/diff-apply-right_dark.svg"))
            }
            "branding/plico.svg" => Some(include_bytes!("../assets/branding/plico.svg")),
            "jb/warning_dark.svg" => Some(include_bytes!("../assets/icons/warning_dark.svg")),
            "jb/clearCash_dark.svg" => Some(include_bytes!("../assets/icons/clearCash_dark.svg")),
            "jb/delete_dark.svg" => Some(include_bytes!("../assets/icons/delete_dark.svg")),
            "jb/add_dark.svg" => Some(include_bytes!("../assets/icons/add_dark.svg")),
            "jb/remove_dark.svg" => Some(include_bytes!("../assets/icons/remove_dark.svg")),
            "jb/edit_dark.svg" => Some(include_bytes!("../assets/icons/edit_dark.svg")),
            "jb/tool-project_dark.svg" => {
                Some(include_bytes!("../assets/icons/tool-project_dark.svg"))
            }
            "jb/file-shell_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-shell_dark_4x.svg"))
            }
            "jb/file-cpp_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-cpp_dark_4x.svg"))
            }
            "jb/file-python_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-python_dark_4x.svg"))
            }
            "jb/file-c_dark_4x.svg" => Some(include_bytes!("../assets/icons/file-c_dark_4x.svg")),
            "jb/file-rust_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-rust_dark_4x.svg"))
            }
            "jb/tool-changes_dark.svg" => {
                Some(include_bytes!("../assets/icons/tool-changes_dark.svg"))
            }
            "jb/tool-commit_dark.svg" => {
                Some(include_bytes!("../assets/icons/tool-commit_dark.svg"))
            }
            "jb/currentBranchLabel_dark.svg" => Some(include_bytes!(
                "../assets/icons/currentBranchLabel_dark.svg"
            )),
            "jb/currentBranchFavoriteLabel_dark.svg" => Some(include_bytes!(
                "../assets/icons/currentBranchFavoriteLabel_dark.svg"
            )),
            "jb/notFavoriteOnHover.svg" => {
                Some(include_bytes!("../assets/icons/notFavoriteOnHover.svg"))
            }
            "jb/favorite.svg" => Some(include_bytes!("../assets/icons/favorite.svg")),
            "jb/reference-mask.svg" => Some(include_bytes!("../assets/icons/reference-mask.svg")),
            "jb/reference-outline.svg" => {
                Some(include_bytes!("../assets/icons/reference-outline.svg"))
            }
            "jb/reference-front.svg" => Some(include_bytes!("../assets/icons/reference-front.svg")),

            "jb/warningDialog_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/warningDialog_dark_4x.svg"))
            }
            "jb/windowsMenu@20x20_dark.svg" => {
                Some(include_bytes!("../assets/icons/windowsMenu@20x20_dark.svg"))
            }
            "jb/shelve_dark.svg" => Some(include_bytes!("../assets/icons/shelve_dark.svg")),
            "jb/fetch_dark.svg" => Some(include_bytes!("../assets/icons/fetch_dark.svg")),
            "jb/update_dark.svg" => Some(include_bytes!("../assets/icons/update_dark.svg")),
            "jb/push_dark.svg" => Some(include_bytes!("../assets/icons/push_dark.svg")),
            "jb/history-show_dark.svg" => {
                Some(include_bytes!("../assets/icons/history-show_dark.svg"))
            }
            "jb/settings_dark.svg" => Some(include_bytes!("../assets/icons/settings_dark.svg")),
            "jb/locked_dark.svg" => Some(include_bytes!("../assets/icons/locked_dark.svg")),
            "jb/sideBySide_dark.svg" => Some(include_bytes!("../assets/icons/sideBySide_dark.svg")),
            "jb/unified_dark.svg" => Some(include_bytes!("../assets/icons/unified_dark.svg")),
            "jb/expandAll_dark.svg" => Some(include_bytes!("../assets/icons/expandAll_dark.svg")),
            "jb/collapseAll_dark.svg" => {
                Some(include_bytes!("../assets/icons/collapseAll_dark.svg"))
            }
            "jb/diff_dark.svg" => Some(include_bytes!("../assets/icons/diff_dark.svg")),
            "jb/hideToolWindow_dark.svg" => {
                Some(include_bytes!("../assets/icons/hideToolWindow_dark.svg"))
            }
            "jb/locate_dark.svg" => Some(include_bytes!("../assets/icons/locate_dark.svg")),
            "jb/rollback_dark.svg" => Some(include_bytes!("../assets/icons/rollback_dark.svg")),
            "jb/CommitInline_dark.svg" => {
                Some(include_bytes!("../assets/icons/CommitInline_dark.svg"))
            }
            "jb/copy_dark.svg" => Some(include_bytes!("../assets/icons/copy_dark.svg")),
            "jb/closeSmall.svg" => Some(include_bytes!("../assets/icons/closeSmall.svg")),
            "ui/corner-bl.svg" => Some(include_bytes!("../assets/ui/corner-bl.svg")),
            "ui/corner-br.svg" => Some(include_bytes!("../assets/ui/corner-br.svg")),
            "ui/corner-tl.svg" => Some(include_bytes!("../assets/ui/corner-tl.svg")),
            "ui/corner-tr.svg" => Some(include_bytes!("../assets/ui/corner-tr.svg")),
            "ui/branches-stripe.svg" => Some(include_bytes!("../assets/ui/branches-stripe.svg")),
            "ui/branches-stripe-zh.svg" => {
                Some(include_bytes!("../assets/ui/branches-stripe-zh.svg"))
            }
            "jb/file-anyType_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-anyType_dark_4x.svg"))
            }
            "jb/file-text_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-text_dark_4x.svg"))
            }
            "jb/file-config_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-config_dark_4x.svg"))
            }
            "jb/file-json_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-json_dark_4x.svg"))
            }
            "jb/file-markdown_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-markdown_dark_4x.svg"))
            }
            "jb/file-java_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-java_dark_4x.svg"))
            }
            "jb/file-javaScript_dark_4x.svg" => Some(include_bytes!(
                "../assets/icons/file-javaScript_dark_4x.svg"
            )),
            "jb/file-typeScript_dark_4x.svg" => Some(include_bytes!(
                "../assets/icons/file-typeScript_dark_4x.svg"
            )),
            "jb/file-xml_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-xml_dark_4x.svg"))
            }
            "jb/file-yaml_dark_4x.svg" => {
                Some(include_bytes!("../assets/icons/file-yaml_dark_4x.svg"))
            }
            "jb/left_dark.svg" => Some(include_bytes!("../assets/icons/left_dark.svg")),
            "jb/right_dark.svg" => Some(include_bytes!("../assets/icons/right_dark.svg")),
            "jb/vcs_dark.svg" => Some(include_bytes!("../assets/icons/vcs_dark.svg")),
            "jb/chevronRight_dark.svg" => {
                Some(include_bytes!("../assets/icons/chevronRight_dark.svg"))
            }
            "jb/branchLabel_dark.svg" => {
                Some(include_bytes!("../assets/icons/branchLabel_dark.svg"))
            }
            "jb/folder_dark_4x.svg" => Some(include_bytes!("../assets/icons/folder_dark_4x.svg")),
            "jb/folder_dark.svg" => Some(include_bytes!("../assets/icons/folder_dark.svg")),
            "jb/open_dark.svg" => Some(include_bytes!("../assets/icons/open_dark.svg")),
            "jb/chevronDown_dark.svg" => {
                Some(include_bytes!("../assets/icons/chevronDown_dark.svg"))
            }
            "jb/search_dark.svg" => Some(include_bytes!("../assets/icons/search_dark.svg")),
            "jb/down_dark.svg" => Some(include_bytes!("../assets/icons/down_dark.svg")),
            "jb/refresh_dark.svg" => Some(include_bytes!("../assets/icons/refresh_dark.svg")),
            "jb/up_dark.svg" => Some(include_bytes!("../assets/icons/up_dark.svg")),
            _ => None,
        };
        if let Some(data) = data {
            return Ok(Some(Cow::Borrowed(data)));
        }
        gpui_kit::assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        gpui_kit::assets::Assets.list(path)
    }
}
