mod applications;

use applications::{App, all_apps};
use iced::widget::{column, scrollable, text_input};
use iced::{Element, Event, Length, Task as Command};
use iced_runtime::Action;
use iced_runtime::window::Action as WindowAction;

use super::Message;

use std::sync::LazyLock;

static SCROLLABLE_ID: LazyLock<scrollable::Id> = LazyLock::new(scrollable::Id::unique);
pub static INPUT_ID: LazyLock<text_input::Id> = LazyLock::new(text_input::Id::unique);

pub struct Launcher {
    text: String,
    apps: Vec<App>,
    scrollpos: usize,
    pub should_delete: bool,
}

#[derive(Debug, Clone)]
pub enum LaunchMessage {
    SearchEditChanged(String),
    SearchSubmit,
    Launch(usize),
    IcedEvent(Event),
}

impl Launcher {
    pub fn new() -> Self {
        Self {
            text: "".to_string(),
            apps: all_apps(),
            scrollpos: 0,
            should_delete: false,
        }
    }

    pub fn focus_input(&self) -> Command<super::Message> {
        text_input::focus(INPUT_ID.clone())
    }

    pub fn update(&mut self, message: LaunchMessage, id: iced::window::Id) -> Command<Message> {
        use iced::keyboard::key::Named;
        use iced_runtime::keyboard;
        match message {
            LaunchMessage::SearchSubmit => {
                let re = regex::Regex::new(&self.text).ok();
                let index = self
                    .apps
                    .iter()
                    .enumerate()
                    .filter(|(_, app)| {
                        if re.is_none() {
                            return true;
                        }
                        let re = re.as_ref().unwrap();

                        re.is_match(app.title().to_lowercase().as_str())
                            || re.is_match(app.description().to_lowercase().as_str())
                    })
                    .enumerate()
                    .find(|(index, _)| *index == self.scrollpos);
                if let Some((_, (_, app))) = index {
                    app.launch();
                    self.should_delete = true;
                    iced_runtime::task::effect(Action::Window(WindowAction::Close(id)))
                } else {
                    Command::none()
                }
            }
            LaunchMessage::SearchEditChanged(edit) => {
                self.scrollpos = 0;
                self.text = edit;
                Command::none()
            }
            LaunchMessage::Launch(index) => {
                self.apps[index].launch();
                self.should_delete = true;
                iced_runtime::task::effect(Action::Window(WindowAction::Close(id)))
            }
            LaunchMessage::IcedEvent(event) => {
                let mut len = self.apps.len();

                let re = regex::Regex::new(&self.text).ok();
                if let Some(re) = re {
                    len = self
                        .apps
                        .iter()
                        .filter(|app| {
                            re.is_match(app.title().to_lowercase().as_str())
                                || re.is_match(app.description().to_lowercase().as_str())
                        })
                        .count();
                }
                if let Event::Keyboard(keyboard::Event::KeyReleased { key, .. })
                | Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event
                {
                    match key {
                        keyboard::Key::Named(Named::ArrowUp) => {
                            if self.scrollpos == 0 {
                                return Command::none();
                            }
                            self.scrollpos -= 1;
                        }
                        keyboard::Key::Named(Named::ArrowDown) => {
                            if self.scrollpos >= len - 1 {
                                return Command::none();
                            }
                            self.scrollpos += 1;
                        }
                        keyboard::Key::Named(Named::Escape) => {
                            self.should_delete = true;
                            return iced_runtime::task::effect(Action::Window(
                                WindowAction::Close(id),
                            ));
                        }
                        _ => {}
                    }
                }
                text_input::focus(INPUT_ID.clone())
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let re = regex::Regex::new(&self.text).ok();
        let text_ip: Element<Message> = text_input("put the launcher name", &self.text)
            .padding(10)
            .on_input(|msg| Message::LauncherInfo(LaunchMessage::SearchEditChanged(msg)))
            .on_submit(Message::LauncherInfo(LaunchMessage::SearchSubmit))
            .id(INPUT_ID.clone())
            .into();
        let bottom_vec: Vec<Element<Message>> = self
            .apps
            .iter()
            .enumerate()
            .filter(|(_, app)| {
                if re.is_none() {
                    return true;
                }
                let re = re.as_ref().unwrap();

                re.is_match(app.title().to_lowercase().as_str())
                    || re.is_match(app.description().to_lowercase().as_str())
            })
            .enumerate()
            .filter(|(index, _)| *index >= self.scrollpos)
            .map(|(filter_index, (index, app))| app.view(index, filter_index == self.scrollpos))
            .collect();
        let bottom: Element<Message> = scrollable(column(bottom_vec).width(Length::Fill))
            .id(SCROLLABLE_ID.clone())
            .into();
        column![text_ip, bottom].into()
    }
}
