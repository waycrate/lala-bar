use iced::widget::{button, container, row, slider, text, Space};
use iced::{executor, Font};
use iced::{Command, Element, Length, Theme};
use zbus_mpirs::ServiceInfo;

use iced_layershell::reexport::{Anchor, Layer};
use iced_layershell::settings::{LayerShellSettings, Settings};
use iced_layershell::MultiApplication;

mod aximer;
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

#[derive(Default)]
struct LalaMusicBar {
    service_data: Option<ServiceInfo>,
    left: i64,
    right: i64,
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
    fn set_balance(&mut self, balance: u8) {
        let total = self.left + self.right;
        self.right = total * balance as i64 / 100;
        self.left = total - self.right;
        aximer::set_left(self.left);
        aximer::set_right(self.right);
    }
}

#[derive(Debug, Clone)]
enum Message {
    RequestPre,
    RequestNext,
    RequestPause,
    RequestPlay,
    RequestDBusInfoUpdate,
    DBusInfoUpdate(Option<ServiceInfo>),
    BalanceChanged(u8),
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

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        (
            Self {
                service_data: None,
                left: aximer::get_left().unwrap_or(0),
                right: aximer::get_right().unwrap_or(0),
            },
            Command::perform(get_metadata_initial(), Message::DBusInfoUpdate),
        )
    }

    fn namespace(&self) -> String {
        String::from("Mpirs_panel")
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
        }
        Command::none()
    }

    fn view(&self, _id: iced::window::Id) -> Element<Message> {
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

        let show_text = format!("balance {}%", self.balance_percent());
        let balance_slider = row![
            text(&show_text),
            Space::with_width(Length::Fixed(10.)),
            slider(0..=100, self.balance_percent(), Message::BalanceChanged),
            Space::with_width(Length::Fixed(10.)),
            button("R").on_press(Message::BalanceChanged(50))
        ];
        let col = row![
            title,
            Space::with_width(Length::Fill),
            buttons,
            balance_slider,
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
        iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::RequestDBusInfoUpdate)
    }

    fn theme(&self) -> Self::Theme {
        Theme::TokyoNight
    }
}
