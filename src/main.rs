use iced::theme::Palette;
use iced::widget::{button, column, container, row, text};
use iced::{executor, window, Font};
use iced::{Application, Command, Element, Length, Settings, Theme};
use zbus_mpirs::ServiceInfo;

mod zbus_mpirs;

pub fn main() -> iced::Result {
    env_logger::builder().format_timestamp(None).init();

    MpirsRoot::run(Settings {
        window: window::Settings {
            size: (600, 200),
            decorations: false,
            transparent: true,
            resizable: false,
            ..Default::default()
        },
        ..Default::default()
    })
}

#[derive(Default)]
struct MpirsRoot {
    service_data: Option<ServiceInfo>,
}

#[derive(Debug, Clone)]
enum Message {
    RequestPre,
    RequestNext,
    RequestPause,
    RequestPlay,
    RequestDBusInfoUpdate,
    DBusInfoUpdate(Option<ServiceInfo>),
}

async fn get_metadata_initial() -> Option<ServiceInfo> {
    zbus_mpirs::init_pris().await.ok();
    let infos = zbus_mpirs::MPIRS_CONNECTIONS.lock().await;
    infos.first().cloned()
}

async fn get_metadata() -> Option<ServiceInfo> {
    let infos = zbus_mpirs::MPIRS_CONNECTIONS.lock().await;
    infos.first().cloned()
}

impl Application for MpirsRoot {
    type Message = Message;
    type Flags = ();
    type Executor = executor::Default;
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        (
            Self::default(),
            Command::perform(get_metadata_initial(), Message::DBusInfoUpdate),
        )
    }

    fn title(&self) -> String {
        String::from("Mpirs panel")
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
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
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
                .style(iced::theme::Text::Color(iced::Color::BLACK)),
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
        let col = column![title, buttons].spacing(40);

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
        Theme::custom(Palette {
            background: iced::Color {
                r: 0.5,
                g: 0.5,
                b: 0.5,
                a: 0.5,
            },
            ..Palette::DARK
        })
    }
}
