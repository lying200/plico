//! Popup-local presentation plus loading and persistence lifetimes.
use crate::ui::*;

pub(in crate::ui) struct Session {
    path: PathBuf,
    pub(in crate::ui) open: bool,
    pub(in crate::ui) search_editing: bool,
    pub(in crate::ui) input: Entity<InputState>,
    pub(in crate::ui) search: Option<plico::branch_search::Search>,
    pub(in crate::ui) applied_query: String,
    pub(in crate::ui) search_task: Task<()>,
    pub(in crate::ui) selection: Option<plico::branch_search::Selection>,
    pub(in crate::ui) actions: Vec<String>,
    pub(in crate::ui) pointer: Option<Point<Pixels>>,
    pub(in crate::ui) child_selection: Option<plico::branch_search::Selection>,
    pub(in crate::ui) child_menu: Option<WeakEntity<gpui_kit::component::menu::PopupMenu>>,
    pub(in crate::ui) project: Option<Arc<plico::branch_popup::project::Project>>,
    pub(in crate::ui) recent: Vec<String>,
    pub(in crate::ui) push_remotes: Vec<String>,
    favorites: plico::branch_popup::Favorites,
    favorites_path: Option<PathBuf>,
    favorites_saving: bool,
    favorites_error: Option<String>,
    favorites_epoch: u64,
    recent_token: Option<Cancel>,
    recent_generation: u64,
    recent_task: Task<()>,
    pub(in crate::ui) expansion: plico::branch_popup::Expansion,
    pub(in crate::ui) scroll: UniformListScrollHandle,
}
impl Session {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Review>) -> Self {
        Self {
            path,
            open: false,
            search_editing: false,
            input: cx.new(|cx| {
                InputState::new(window, cx).placeholder(t("Search for branches and actions"))
            }),
            search: None,
            applied_query: String::new(),
            search_task: Task::ready(()),
            selection: None,
            actions: vec!["commit".into(), "new-branch".into(), "revision".into()],
            pointer: None,
            child_selection: None,
            child_menu: None,
            project: None,
            recent: Vec::new(),
            push_remotes: Vec::new(),
            favorites: Default::default(),
            favorites_path: None,
            favorites_saving: false,
            favorites_error: None,
            favorites_epoch: 0,
            recent_token: None,
            recent_generation: 0,
            recent_task: Task::ready(()),
            expansion: Default::default(),
            scroll: UniformListScrollHandle::new(),
        }
    }
}

/// A load belongs to one project and one open-popup generation.
pub(in crate::ui) struct Load {
    path: PathBuf,
    generation: u64,
    pub token: Cancel,
}
/// Persistence may outlive closing the popup, but never update a new project.
pub(in crate::ui) struct FavoriteSave {
    epoch: u64,
    file: PathBuf,
    previous: plico::branch_popup::Favorites,
    next: plico::branch_popup::Favorites,
}
impl FavoriteSave {
    pub fn write(&self) -> std::io::Result<()> {
        plico::favorite_preferences::save(&self.file, &self.next)
    }
}
impl Session {
    pub fn favorites(&self) -> &plico::branch_popup::Favorites {
        &self.favorites
    }
    pub fn favorites_failed(&self) -> bool {
        self.favorites_error.is_some()
    }
    pub fn load_favorites(&self) -> bool {
        !self.favorites_saving
    }
    pub fn can_favorite(&self) -> bool {
        self.favorites_path.is_some() && !self.favorites_saving && !self.loading()
    }
    pub fn install_favorites(
        &mut self,
        result: Result<(PathBuf, plico::branch_popup::Favorites), String>,
    ) {
        match result {
            Ok((path, favorites)) => {
                self.favorites_path = Some(path);
                self.favorites = favorites;
                self.favorites_error = None;
            }
            Err(error) => {
                self.favorites_path = None;
                self.favorites_error = Some(error);
            }
        }
    }
    pub fn prepare_search(&mut self, cx: &App) {
        let value = self.input.read(cx).value();
        self.applied_query = value.to_string();
        self.search = plico::branch_search::Search::new(
            plico::branch_search::popup_pattern(value.as_str()),
            &Default::default(),
            cfg!(target_os = "linux"),
        );
    }
    /// Keyboard activation flushes any debounced input before using selection.
    pub fn flush_search(&mut self, cx: &App) -> bool {
        self.search_task = Task::ready(());
        self.input.read(cx).value().as_str() != self.applied_query
    }
    pub fn loading(&self) -> bool {
        self.recent_token.is_some()
    }
    pub fn invalidate(&mut self) {
        if let Some(token) = self.recent_token.take() {
            token.cancel();
        }
        self.recent_generation += 1;
        self.recent_task = Task::ready(());
    }
    pub fn dismiss(&mut self) {
        self.open = false;
        self.project = None;
        self.pointer = None;
        self.search_task = Task::ready(());
        self.search = None;
        self.selection = None;
        self.invalidate();
    }
    pub fn reset_project(&mut self, path: PathBuf) {
        self.dismiss();
        self.path = path;
        self.recent.clear();
        self.push_remotes.clear();
        self.favorites = Default::default();
        self.favorites_path = None;
        self.favorites_saving = false;
        self.favorites_error = None;
        self.favorites_epoch += 1;
        self.expansion = Default::default();
    }
    pub fn begin_load(&mut self) -> Load {
        self.invalidate();
        self.push_remotes.clear();
        let token = Cancel::default();
        self.recent_token = Some(token.clone());
        Load {
            path: self.path.clone(),
            generation: self.recent_generation,
            token,
        }
    }
    pub fn track_load(&mut self, task: Task<()>) {
        self.recent_task = task;
    }
    pub fn accept_load(&mut self, load: &Load) -> bool {
        if self.recent_token.is_none()
            || !self.open
            || self.path != load.path
            || self.recent_generation != load.generation
            || load.token.cancelled()
        {
            return false;
        }
        self.recent_token = None;
        true
    }
    pub fn begin_favorite(&mut self, reference: &str) -> Option<FavoriteSave> {
        if self.favorites_saving || self.loading() {
            return None;
        }
        let file = self.favorites_path.clone()?;
        let previous = self.favorites.clone();
        if !self.favorites.toggle(reference) {
            return None;
        }
        self.favorites_saving = true;
        self.favorites_error = None;
        Some(FavoriteSave {
            epoch: self.favorites_epoch,
            file,
            previous,
            next: self.favorites.clone(),
        })
    }
    pub fn finish_favorite(&mut self, save: FavoriteSave, result: std::io::Result<()>) -> bool {
        if self.favorites_epoch != save.epoch {
            return false;
        }
        self.favorites_saving = false;
        if let Err(error) = result {
            self.favorites = save.previous;
            self.favorites_error = Some(error.to_string());
        }
        true
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(token) = &self.recent_token {
            token.cancel();
        }
    }
}

#[cfg(all(test, feature = "desktop-tests"))]
mod tests {
    extern crate gpui_kit as gpui;
    use super::*;
    use core::prelude::v1::test;
    use gpui::component::Root;

    #[gpui::test]
    fn closed_reopened_and_replaced_project_reject_old_loads(cx: &mut gpui::TestAppContext) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        review.unwrap().update(cx, |s, _| {
            let session = &mut s.branch_popup;
            session.open = true;
            let old = session.begin_load();
            session.dismiss();
            assert!(old.token.cancelled());
            session.open = true;
            let reopened = session.begin_load();
            assert!(!session.accept_load(&old));
            assert!(session.loading());
            assert!(session.accept_load(&reopened));
            assert!(!session.loading());
            assert!(!session.accept_load(&reopened));
            let replaced = session.begin_load();
            session.reset_project("other".into());
            session.open = true;
            let current = session.begin_load();
            assert!(!session.accept_load(&replaced));
            session.invalidate(); // repository membership changes retain old rows
            assert!(current.token.cancelled());
            assert!(!session.accept_load(&current));
        });
    }

    #[gpui::test]
    fn favorite_failure_rolls_back_only_its_project_and_close_does_not_cancel_persistence(
        cx: &mut gpui::TestAppContext,
    ) {
        cx.update(gpui::component::init);
        let mut review = None;
        let (_, cx) = cx.add_window_view(|w, cx| {
            let view = cx.new(|cx| Review::new(PathBuf::new(), 100, None, w, cx));
            review = Some(view.clone());
            Root::new(view, w, cx)
        });
        review.unwrap().update(cx, |s, _| {
            let session = &mut s.branch_popup;
            let reference = "refs/heads/topic";
            session.install_favorites(Ok(("first/prefs".into(), Default::default())));
            session.open = true;
            let load = session.begin_load();
            assert!(session.begin_favorite(reference).is_none());
            assert!(session.accept_load(&load));
            let save = session.begin_favorite(reference).unwrap();
            assert!(session.favorites().contains(reference));
            session.dismiss();
            assert!(session.finish_favorite(save, Err(std::io::Error::other("fixture"))));
            assert!(!session.favorites().contains(reference));
            assert!(session.favorites_failed());
            let old = session.begin_favorite(reference).unwrap();
            session.reset_project("second".into());
            session.install_favorites(Ok(("second/prefs".into(), Default::default())));
            let current = session.begin_favorite(reference).unwrap();
            assert!(!session.finish_favorite(old, Err(std::io::Error::other("late"))));
            assert!(session.favorites().contains(reference));
            assert!(!session.load_favorites());
            assert!(session.finish_favorite(current, Ok(())));
            assert!(!session.favorites_failed());
            assert!(session.load_favorites());
        });
    }
}
