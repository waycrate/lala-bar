use async_trait::async_trait;
use iced::widget::markdown;
use iced_zbus_notification::{MessageSender, NotifyMessage};
use launcher::{LaunchMessage, Launcher};
use zbus_mpirs::ServiceInfo;

use futures::channel::mpsc::Sender;
use iced_aw::date_picker::Date;
use iced_aw::time_picker::Time;
use iced_layershell::{to_layer_message, WindowInfoMarker};

mod aximer;
mod config;
mod dbusbackend;
mod launcher;
mod music_bar;
mod notify;
mod slider;
mod zbus_mpirs;

use crate::music_bar::LalaMusicBar;
use crate::notify::NotifyCommand;
use notify::NotifyUnitWidgetInfo;

pub fn main() -> Result<(), iced_layershell::Error> {
    use tracing_subscriber::filter::LevelFilter;
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::filter::EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy()
                .add_directive("usvg=off".parse().unwrap()),
        )
        .init();
    music_bar::run_lalabar()
}

#[derive(Debug, Clone, PartialEq, WindowInfoMarker)]
pub enum LaLaInfo {
    #[singleton]
    Launcher,
    Notify(Box<NotifyUnitWidgetInfo>),
    #[singleton]
    HiddenInfo,
    #[singleton]
    RightPanel,
    ErrorHappened(iced::window::Id),
    #[singleton]
    Calendar,
    #[singleton]
    TimePicker,
}

#[to_layer_message(multi, info_name = "LaLaInfo")]
#[derive(Debug, Clone)]
pub enum Message {
    RequestPre,
    RequestNext,
    RequestPause,
    RequestPlay,
    RequestDBusInfoUpdate,
    RequestUpdateTime,
    UpdateBalance,
    DBusInfoUpdate(Option<ServiceInfo>),
    BalanceChanged(u8),
    UpdateLeft(u8),
    UpdateRight(u8),
    SliderIndexNext,
    SliderIndexPre,
    ToggleLauncher,
    ToggleLauncherDBus,
    ToggleRightPanel,
    LauncherInfo(LaunchMessage),
    Notify(NotifyMessage),
    RemoveNotify(u32),
    InlineReply((u32, String)),
    InlineReplyMsgUpdate((iced::window::Id, String)),
    CheckOutput,
    ClearAllNotifications,
    QuiteMode(bool),
    CloseErrorNotification(iced::window::Id),
    Ready(Sender<NotifyCommand>),
    #[allow(unused)]
    LinkClicked(markdown::Url),
    ToggleCalendar,
    CancelDate,
    SubmitDate(Date),
    ToggleTime,
    CancelTime,
    SubmitTime(Time),
}

impl From<NotifyMessage> for Message {
    fn from(value: NotifyMessage) -> Self {
        Self::Notify(value)
    }
}

async fn get_metadata_initial() -> Option<ServiceInfo> {
    zbus_mpirs::init_mpirs().await.ok();
    get_metadata().await
}

async fn get_metadata() -> Option<ServiceInfo> {
    let infos = zbus_mpirs::MPIRS_CONNECTIONS.lock().await;

    let alive_infos: Vec<&ServiceInfo> = infos
        .iter()
        .filter(|info| !info.metadata.xesam_title.is_empty())
        .collect();

    if let Some(playingserver) = alive_infos
        .iter()
        .find(|info| info.playback_status == "Playing")
    {
        return Some((*playingserver).clone());
    }
    alive_infos.first().cloned().cloned()
}

struct IcedMessageSender(Sender<Message>);

#[async_trait]
impl MessageSender<Message> for IcedMessageSender {
    async fn try_send(&mut self, message: Message) -> Option<()> {
        if matches!(message, Message::Notify(NotifyMessage::UnitAdd(_))) {
            // HACK: let message receiver to be a little late, for the notify widget to be ready
            tokio::time::sleep(std::time::Duration::from_secs_f64(0.05)).await;
        }

        self.0.try_send(message).ok()
    }
}
