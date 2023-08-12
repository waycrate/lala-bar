use iced::widget::{container, text};
use iced::{executor, window};
use iced::{Application, Command, Element, Length, Settings, Theme};
use zbus_mpirs::ServiceInfo;

mod zbus_mpirs;

pub fn main() -> iced::Result {
    env_logger::builder().format_timestamp(None).init();

    MpirsRoot::run(Settings {
        window: window::Settings {
            size: (1000, 200),
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
    title: String,
}

#[derive(Debug, Clone)]
enum Message {
    RequestDbusInfoUpdate,
    DBusInfoUpdate(Option<ServiceInfo>),
}

async fn get_metadata() -> Option<ServiceInfo> {
    let _ = zbus_mpirs::init_pris().await;
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
            Command::perform(
                get_metadata(),
                Message::DBusInfoUpdate,
            ),
        )
    }

    fn title(&self) -> String {
        String::from("A Template")
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::DBusInfoUpdate(Some(data)) => {
                self.title = data.metadata.xesam_title.clone();
            }
            Message::RequestDbusInfoUpdate => {
                return Command::perform(get_metadata(), Message::DBusInfoUpdate)
            }
            _ => {}
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let default_checkbox = text(self.title.as_str());

        container(default_checkbox)
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
