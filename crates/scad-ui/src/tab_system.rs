use std::any::Any;

pub type TabId = u64;

pub trait WorkTab: Any {
    fn id(&self) -> TabId;
    fn title(&self) -> &str;
    fn is_closable(&self) -> bool;
    fn show(&mut self, ui: &mut egui::Ui, ctx: &mut TabContext<'_>);
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

#[derive(Default)]
pub struct TabContext<'a> {
    pub status_message: Option<&'a str>,
}

#[derive(Default)]
pub struct TabManager {
    tabs: Vec<Box<dyn WorkTab>>,
    active_tab_id: Option<TabId>,
    drag_state: Option<DragState>,
}

#[derive(Clone, Copy)]
struct DragState {
    source_index: usize,
    hovered_index: usize,
}

impl TabManager {
    pub fn open_tab(&mut self, tab: Box<dyn WorkTab>) {
        if let Some(index) = self
            .tabs
            .iter()
            .position(|current| current.id() == tab.id())
        {
            self.active_tab_id = Some(self.tabs[index].id());
            return;
        }
        self.active_tab_id = Some(tab.id());
        self.tabs.push(tab);
    }

    pub fn close_tab(&mut self, id: TabId) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id() == id) else {
            return;
        };
        self.tabs.remove(index);
        self.active_tab_id = self
            .tabs
            .get(index)
            .or_else(|| self.tabs.last())
            .map(|tab| tab.id());
    }

    pub fn set_active(&mut self, id: TabId) {
        if self.tabs.iter().any(|tab| tab.id() == id) {
            self.active_tab_id = Some(id);
        }
    }

    pub fn active_tab_id(&self) -> Option<TabId> {
        self.active_tab_id
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    pub fn contains(&self, id: TabId) -> bool {
        self.tabs.iter().any(|tab| tab.id() == id)
    }

    pub fn tab_ids(&self) -> Vec<TabId> {
        self.tabs.iter().map(|tab| tab.id()).collect()
    }

    pub fn tab_mut(&mut self, id: TabId) -> Option<&mut Box<dyn WorkTab>> {
        self.tabs.iter_mut().find(|tab| tab.id() == id)
    }

    pub fn move_tab(&mut self, from: usize, to: usize) {
        if from >= self.tabs.len() || to >= self.tabs.len() || from == to {
            return;
        }
        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);
    }

    pub fn active_tab_mut(&mut self) -> Option<&mut Box<dyn WorkTab>> {
        let active = self.active_tab_id?;
        self.tabs.iter_mut().find(|tab| tab.id() == active)
    }

    pub fn tabs_mut(&mut self) -> &mut [Box<dyn WorkTab>] {
        &mut self.tabs
    }

    pub fn active_tab_as_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.active_tab_mut()?.as_any_mut().downcast_mut::<T>()
    }

    pub fn active_tab_as<T: Any>(&self) -> Option<&T> {
        let active = self.active_tab_id?;
        self.tabs
            .iter()
            .find(|tab| tab.id() == active)?
            .as_any()
            .downcast_ref::<T>()
    }

    pub fn show_tab_bar(&mut self, ui: &mut egui::Ui) {
        let mut close_id = None;
        let mut activate_id = None;
        let mut move_request = None;
        ui.horizontal_wrapped(|ui| {
            for index in 0..self.tabs.len() {
                let tab_id = self.tabs[index].id();
                let active = self.active_tab_id == Some(tab_id);
                let label = self.tabs[index].title().to_string();
                let visual = ui.selectable_label(active, label);
                let drag = ui.interact(
                    visual.rect,
                    visual.id.with("drag"),
                    egui::Sense::click_and_drag(),
                );
                if visual.clicked() {
                    activate_id = Some(tab_id);
                }
                if drag.drag_started() {
                    self.drag_state = Some(DragState {
                        source_index: index,
                        hovered_index: index,
                    });
                }
                if drag.hovered()
                    && drag.ctx.pointer_interact_pos().is_some()
                    && self.drag_state.is_some()
                {
                    if let Some(state) = self.drag_state.as_mut() {
                        state.hovered_index = index;
                    }
                    paint_insertion_indicator(ui, visual.rect);
                }
                if drag.drag_stopped()
                    && let Some(state) = self.drag_state.take()
                {
                    move_request = Some((state.source_index, state.hovered_index));
                }
                if self.tabs[index].is_closable()
                    && (visual.hovered() || active)
                    && ui.small_button("×").clicked()
                {
                    close_id = Some(tab_id);
                }
            }
        });
        if let Some(id) = activate_id {
            self.set_active(id);
        }
        if let Some((from, to)) = move_request {
            self.move_tab(from, to);
        }
        if let Some(id) = close_id {
            self.close_tab(id);
        }
    }

    pub fn show_active_content(&mut self, ui: &mut egui::Ui, ctx: &mut TabContext<'_>) {
        let Some(tab) = self.active_tab_mut() else {
            ui.label("暂无打开的标签页");
            return;
        };
        tab.show(ui, ctx);
    }
}

fn paint_insertion_indicator(ui: &egui::Ui, rect: egui::Rect) {
    let x = rect.left() - 3.0;
    let top = egui::pos2(x, rect.top() + 2.0);
    let bottom = egui::pos2(x, rect.bottom() - 2.0);
    ui.painter().line_segment(
        [top, bottom],
        egui::Stroke::new(2.0, ui.visuals().selection.stroke.color),
    );
}
