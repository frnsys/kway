use relm4::RelmApp;

use crate::{
    keyboard::Keyboard,
    layout::Layout,
    pointer::Pointer,
    ui::{UIMessage, UIModel},
};

pub struct App {
    ui: RelmApp<UIMessage>,
    keyboard: Keyboard,
    pointer: Pointer,
}
impl App {
    pub fn new(layout: Layout) -> Self {
        let ui = RelmApp::<UIMessage>::new("clav");
        let styles = include_str!("../assets/style.css");
        relm4::set_global_css_with_priority(styles, relm4::gtk::STYLE_PROVIDER_PRIORITY_USER);

        App {
            ui,
            pointer: Pointer::new(),
            keyboard: Keyboard::new(layout),
        }
    }

    pub fn run(self) {
        self.ui.run::<UIModel>((self.keyboard, self.pointer));
    }
}
