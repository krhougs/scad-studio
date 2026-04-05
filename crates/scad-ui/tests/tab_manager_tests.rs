use scad_ui::tab_system::{TabContext, TabId, TabManager, WorkTab};
use std::any::Any;

struct FakeTab {
    id: TabId,
    title: String,
    closable: bool,
}

impl FakeTab {
    fn new(id: TabId, title: &str, closable: bool) -> Self {
        Self {
            id,
            title: title.to_string(),
            closable,
        }
    }
}

impl WorkTab for FakeTab {
    fn id(&self) -> TabId {
        self.id
    }

    fn title(&self) -> &str {
        &self.title
    }

    fn is_closable(&self) -> bool {
        self.closable
    }

    fn show(&mut self, _ui: &mut egui::Ui, _ctx: &mut TabContext<'_>) {}

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[test]
fn open_tab_activates_new_tab_and_preserves_order() {
    let mut manager = TabManager::default();

    manager.open_tab(Box::new(FakeTab::new(1, "A", true)));
    manager.open_tab(Box::new(FakeTab::new(2, "B", true)));

    assert_eq!(manager.active_tab_id(), Some(2));
    assert_eq!(manager.tab_ids(), vec![1, 2]);
}

#[test]
fn open_tab_reuses_existing_id_instead_of_duplication() {
    let mut manager = TabManager::default();

    manager.open_tab(Box::new(FakeTab::new(1, "A", true)));
    manager.open_tab(Box::new(FakeTab::new(1, "A2", true)));

    assert_eq!(manager.tab_ids(), vec![1]);
    assert_eq!(manager.active_tab_id(), Some(1));
}

#[test]
fn close_tab_selects_neighboring_tab() {
    let mut manager = TabManager::default();
    manager.open_tab(Box::new(FakeTab::new(1, "A", true)));
    manager.open_tab(Box::new(FakeTab::new(2, "B", true)));
    manager.open_tab(Box::new(FakeTab::new(3, "C", true)));

    manager.close_tab(2);

    assert_eq!(manager.tab_ids(), vec![1, 3]);
    assert_eq!(manager.active_tab_id(), Some(3));
}

#[test]
fn move_tab_reorders_tabs_and_keeps_active_id() {
    let mut manager = TabManager::default();
    manager.open_tab(Box::new(FakeTab::new(1, "A", true)));
    manager.open_tab(Box::new(FakeTab::new(2, "B", true)));
    manager.open_tab(Box::new(FakeTab::new(3, "C", true)));
    manager.set_active(2);

    manager.move_tab(1, 0);

    assert_eq!(manager.tab_ids(), vec![2, 1, 3]);
    assert_eq!(manager.active_tab_id(), Some(2));
}
