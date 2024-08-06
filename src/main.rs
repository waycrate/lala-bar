use iced::widget::{button, container, row, slider, text, Space};
use iced::{executor, Event, Font};
use iced::{Command, Element, Length, Theme};
use iced_layershell::actions::{
    LayershellCustomActionsWithIdAndInfo, LayershellCustomActionsWithInfo,
};
use launcher::Launcher;
use zbus_mpirs::ServiceInfo;

use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings};
use iced_layershell::settings::{LayerShellSettings, Settings};
use iced_layershell::MultiApplication;
use iced_runtime::command::Action;
use iced_runtime::window::Action as WindowAction;

mod aximer;
mod launcher;
mod zbus_mpirs;

pub fn main() -> Result<(), iced_layershell::Error> {
    env_logger::builder().format_timestamp(None).init();

    LalaMusicBar::run(Settings {
        layer_settings: LayerShellSettings {
            size: Some((0, 40)),
            exclusize_zone: 40,
            anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
            layer: Layer::Top,
            ..Default::default()
        },
        ..Default::default()
    })
}

#[derive(Debug, Clone, Copy)]
struct LauncherInfo;

#[derive(Default)]
struct LalaMusicBar {
    service_data: Option<ServiceInfo>,
    left: i64,
    right: i64,
    bar_index: SliderIndex,
    launcher: Option<launcher::Launcher>,
    launcherid: Option<iced::window::Id>,
}

#[derive(Copy, Clone, Default)]
enum SliderIndex {
    #[default]
    Balance,
    Left,
    Right,
}

impl SliderIndex {
    fn next(&self) -> Self {
        match self {
            SliderIndex::Balance => SliderIndex::Left,
            SliderIndex::Left => SliderIndex::Right,
            SliderIndex::Right => SliderIndex::Balance,
        }
    }
    fn pre(&self) -> Self {
        match self {
            SliderIndex::Balance => SliderIndex::Right,
            SliderIndex::Left => SliderIndex::Balance,
            SliderIndex::Right => SliderIndex::Left,
        }
    }
}

impl LalaMusicBar {
    fn balance_percent(&self) -> u8 {
        if self.left == 0 && self.right == 0 {
            return 0;
        }
        (self.right * 100 / (self.left + self.right))
            .try_into()
            .unwrap()
    }

    fn update_balance(&mut self) {
        self.left = aximer::get_left().unwrap_or(0);
        self.right = aximer::get_right().unwrap_or(0);
    }

    fn set_balance(&mut self, balance: u8) {
        self.update_balance();
        let total = self.left + self.right;
        self.right = total * balance as i64 / 100;
        self.left = total - self.right;
        aximer::set_left(self.left);
        aximer::set_right(self.right);
    }
}

impl LalaMusicBar {
    fn balance_bar(&self) -> Element<Message> {
        let show_text = format!("balance {}%", self.balance_percent());
        row![
            button("<").on_press(Message::SliderIndexPre),
            Space::with_width(Length::Fixed(1.)),
            text(&show_text),
            Space::with_width(Length::Fixed(10.)),
            slider(0..=100, self.balance_percent(), Message::BalanceChanged),
            Space::with_width(Length::Fixed(10.)),
            button("R").on_press(Message::BalanceChanged(50)),
            Space::with_width(Length::Fixed(1.)),
            button(">").on_press(Message::SliderIndexNext)
        ]
        .into()
    }
    fn left_bar(&self) -> Element<Message> {
        let show_text = format!("left {}%", self.left);
        row![
            button("<").on_press(Message::SliderIndexPre),
            Space::with_width(Length::Fixed(1.)),
            text(&show_text),
            Space::with_width(Length::Fixed(10.)),
            slider(0..=100, self.left as u8, Message::UpdateLeft),
            Space::with_width(Length::Fixed(10.)),
            button(">").on_press(Message::SliderIndexNext)
        ]
        .into()
    }
    fn right_bar(&self) -> Element<Message> {
        let show_text = format!("right {}%", self.right);
        row![
            button("<").on_press(Message::SliderIndexPre),
            Space::with_width(Length::Fixed(1.)),
            text(&show_text),
            Space::with_width(Length::Fixed(10.)),
            slider(0..=100, self.right as u8, Message::UpdateRight),
            Space::with_width(Length::Fixed(10.)),
            button(">").on_press(Message::SliderIndexNext)
        ]
        .into()
    }

    fn sound_slider(&self) -> Element<Message> {
        match self.bar_index {
            SliderIndex::Left => self.left_bar(),
            SliderIndex::Right => self.right_bar(),
            SliderIndex::Balance => self.balance_bar(),
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    RequestPre,
    RequestNext,
    RequestPause,
    RequestPlay,
    RequestDBusInfoUpdate,
    UpdateBalance,
    DBusInfoUpdate(Option<ServiceInfo>),
    BalanceChanged(u8),
    UpdateLeft(u8),
    UpdateRight(u8),
    SliderIndexNext,
    SliderIndexPre,
    ToggleLauncher,
    SearchEditChanged(String),
    SearchSubmit,
    Launch(usize),
    IcedEvent(Event),
}

async fn get_metadata_initial() -> Option<ServiceInfo> {
    zbus_mpirs::init_mpirs().await.ok();
    let infos = zbus_mpirs::MPIRS_CONNECTIONS.lock().await;
    infos.first().cloned()
}

async fn get_metadata() -> Option<ServiceInfo> {
    let infos = zbus_mpirs::MPIRS_CONNECTIONS.lock().await;
    infos.first().cloned()
}

impl MultiApplication for LalaMusicBar {
    type Message = Message;
    type Flags = ();
    type Executor = executor::Default;
    type Theme = Theme;
    type WindowInfo = LauncherInfo;

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        (
            Self {
                service_data: None,
                left: aximer::get_left().unwrap_or(0),
                right: aximer::get_right().unwrap_or(0),
                bar_index: SliderIndex::Balance,
                launcher: None,
                launcherid: None,
            },
            Command::perform(get_metadata_initial(), Message::DBusInfoUpdate),
        )
    }

    fn namespace(&self) -> String {
        String::from("Mpirs_panel")
    }

    fn id_info(&self, id: iced_futures::core::window::Id) -> Option<&Self::WindowInfo> {
        if self.launcherid.is_some_and(|tid| tid == id) {
            Some(&LauncherInfo)
        } else {
            None
        }
    }

    fn set_id_info(&mut self, id: iced_futures::core::window::Id, _info: Self::WindowInfo) {
        self.launcherid = Some(id);
    }

    fn remove_id(&mut self, _id: iced_futures::core::window::Id) {
        self.launcherid.take();
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::DBusInfoUpdate(data) => self.service_data = data,
            Message::RequestDBusInfoUpdate => {
                return Command::perform(get_metadata(), Message::DBusInfoUpdate)
            }
            Message::RequestPlay => {
                if let Some(ref data) = self.service_data {
                    if !data.can_play {
                        return Command::none();
                    }
                    let data = data.clone();
                    return Command::perform(
                        async move {
                            data.play().await.ok();
                            get_metadata().await
                        },
                        Message::DBusInfoUpdate,
                    );
                }
            }
            Message::RequestPause => {
                if let Some(ref data) = self.service_data {
                    if !data.can_pause {
                        return Command::none();
                    }
                    let data = data.clone();
                    return Command::perform(
                        async move {
                            data.pause().await.ok();
                            get_metadata().await
                        },
                        Message::DBusInfoUpdate,
                    );
                }
            }
            Message::RequestPre => {
                if let Some(ref data) = self.service_data {
                    if !data.can_go_previous {
                        return Command::none();
                    }
                    let data = data.clone();
                    return Command::perform(
                        async move {
                            data.go_previous().await.ok();
                            get_metadata().await
                        },
                        Message::DBusInfoUpdate,
                    );
                }
            }
            Message::RequestNext => {
                if let Some(ref data) = self.service_data {
                    if !data.can_go_next {
                        return Command::none();
                    }
                    let data = data.clone();
                    return Command::perform(
                        async move {
                            data.go_next().await.ok();
                            get_metadata().await
                        },
                        Message::DBusInfoUpdate,
                    );
                }
            }
            Message::BalanceChanged(balance) => {
                let current_balance = self.balance_percent();
                if current_balance == 0 {
                    return Command::none();
                }
                self.set_balance(balance)
            }
            Message::UpdateBalance => {
                self.update_balance();
            }
            Message::UpdateLeft(percent) => {
                aximer::set_left(percent as i64);
                self.update_balance();
            }
            Message::UpdateRight(percent) => {
                aximer::set_right(percent as i64);
                self.update_balance();
            }
            Message::SliderIndexNext => self.bar_index = self.bar_index.next(),
            Message::SliderIndexPre => self.bar_index = self.bar_index.pre(),
            Message::ToggleLauncher => {
                if self.launcher.is_some() {
                    if let Some(id) = self.launcherid {
                        self.launcher.take();
                        return Command::single(Action::Window(WindowAction::Close(id)));
                    }
                    return Command::none();
                }
                self.launcher = Some(Launcher::new());
                return Command::batch(vec![
                    Command::single(
                        LayershellCustomActionsWithIdAndInfo::new(
                            iced::window::Id::MAIN,
                            LayershellCustomActionsWithInfo::NewLayerShell((
                                NewLayerShellSettings {
                                    size: Some((500, 700)),
                                    exclusize_zone: None,
                                    anchor: Anchor::Left | Anchor::Bottom,
                                    layer: Layer::Top,
                                    margins: None,
                                    keyboard_interactivity: KeyboardInteractivity::Exclusive,
                                },
                                LauncherInfo,
                            )),
                        )
                        .into(),
                    ),
                    self.launcher.as_ref().unwrap().focus_input(),
                ]);
            }
            _ => {
                if let Some(launcher) = self.launcher.as_mut() {
                    if let Some(id) = self.launcherid {
                        let cmd = launcher.update(message, id);
                        if launcher.shoud_delete {
                            self.launcher.take();
                        }
                        return cmd;
                    }
                }
            }
        }
        Command::none()
    }

    fn view(&self, id: iced::window::Id) -> Element<Message> {
        if let Some(LauncherInfo) = self.id_info(id) {
            if let Some(launcher) = &self.launcher {
                return launcher.view();
            }
        }
        let title = self
            .service_data
            .as_ref()
            .map(|data| data.metadata.xesam_title.as_str())
            .unwrap_or("No Video here");
        let title = container(
            text(title)
                .size(20)
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                })
                .shaping(text::Shaping::Advanced)
                .style(iced::theme::Text::Color(iced::Color::WHITE)),
        )
        .width(Length::Fill)
        .center_x();
        let can_play = self.service_data.as_ref().is_some_and(|data| data.can_play);
        let can_pause = self
            .service_data
            .as_ref()
            .is_some_and(|data| data.can_pause);
        let can_go_next = self
            .service_data
            .as_ref()
            .is_some_and(|data| data.can_go_next);
        let can_go_pre = self
            .service_data
            .as_ref()
            .is_some_and(|data| data.can_go_previous);
        let mut button_pre = button("<|");
        if can_go_pre {
            button_pre = button_pre.on_press(Message::RequestPre);
        }
        let mut button_next = button("|>");
        if can_go_next {
            button_next = button_next.on_press(Message::RequestNext);
        }
        let button_play = {
            match self.service_data {
                Some(ref data) => {
                    if data.playback_status == "Playing" {
                        let mut btn = button(text("Pause"));
                        if can_pause {
                            btn = btn.on_press(Message::RequestPause);
                        }
                        btn
                    } else {
                        let mut btn = button(text("Play"));
                        if can_play {
                            btn = btn.on_press(Message::RequestPlay);
                        }
                        btn
                    }
                }
                None => button(text("Nothing todo")),
            }
        };
        let buttons = container(row![button_pre, button_play, button_next].spacing(5))
            .width(Length::Fill)
            .center_x();

        let sound_slider = self.sound_slider();
        let col = row![
            button("L").on_press(Message::ToggleLauncher),
            title,
            Space::with_width(Length::Fill),
            buttons,
            sound_slider,
            Space::with_width(Length::Fixed(10.)),
        ]
        .spacing(10);

        container(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        iced::subscription::Subscription::batch([
            iced::time::every(std::time::Duration::from_secs(1))
                .map(|_| Message::RequestDBusInfoUpdate),
            iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::UpdateBalance),
            iced::event::listen().map(Message::IcedEvent),
        ])
    }

    fn theme(&self) -> Self::Theme {
        Theme::TokyoNight
    }
}
