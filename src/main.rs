use iced::widget::{button, column, container, text};
use iced::{executor, window};
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
    RequestPause,
    RequestPlay,
    RequestDbusInfoUpdate,
    DBusInfoUpdate(Option<ServiceInfo>),
}

async fn get_metadata() -> Option<ServiceInfo> {
    zbus_mpirs::init_pris().await.ok();
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
            Command::perform(get_metadata(), Message::DBusInfoUpdate),
        )
    }

    fn title(&self) -> String {
        String::from("A Template")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::DBusInfoUpdate(data) => self.service_data = data,
            Message::RequestDbusInfoUpdate => {
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
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let title = self
            .service_data
            .as_ref()
            .map(|data| data.metadata.xesam_title.as_str())
            .unwrap_or("No Video here");
        let title = container(text(title)).width(Length::Fill).center_x();
        let button = {
            match self.service_data {
                Some(ref data) => {
                    if data.playback_status == "Playing" {
                        container(button(text("Pause")).on_press(Message::RequestPause))
                            .width(Length::Fill)
                            .center_x()
                    } else {
                        container(button(text("Play")).on_press(Message::RequestPlay))
                            .width(Length::Fill)
                            .center_x()
                    }
                }
                None => container(button(text("Nothing todo"))).width(Length::Fill).center_x(),
            }
        };
        let col = column![title, button];

        container(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        iced::time::every(std::time::Duration::from_secs(1)).map(|_| Message::RequestDbusInfoUpdate)
    }
}
