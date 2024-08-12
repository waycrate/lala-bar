use std::collections::HashMap;

use futures::future::pending;
use futures::StreamExt;
use iced::widget::{
    button, checkbox, column, container, image, row, scrollable, slider, svg, text, text_input,
    Space,
};
use iced::{executor, Font};
use iced::{Command, Element, Length, Theme};
use iced_layershell::actions::{
    LayershellCustomActionsWithIdAndInfo, LayershellCustomActionsWithInfo,
};
use launcher::{LaunchMessage, Launcher};
use zbus_mpirs::ServiceInfo;
use zbus_notification::{
    start_connection, ImageInfo, LaLaMako, NotifyMessage, NotifyUnit, VersionInfo, DEFAULT_ACTION,
    NOTIFICATION_SERVICE_PATH,
};

use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings};
use iced_layershell::settings::{LayerShellSettings, Settings};
use iced_layershell::MultiApplication;
use iced_runtime::command::Action;
use iced_runtime::window::Action as WindowAction;

use futures::channel::mpsc::{channel, Receiver, Sender};

use tokio::sync::Mutex;

use std::sync::Arc;

mod aximer;
mod launcher;
mod zbus_mpirs;

type LaLaShellIdAction = LayershellCustomActionsWithIdAndInfo<LaLaInfo>;
type LalaShellAction = LayershellCustomActionsWithInfo<LaLaInfo>;

const BEGINNING_UP_MARGIN: i32 = 10;

const UNIT_MARGIN: i32 = 135;

const EXTRAINF_MARGIN: i32 = BEGINNING_UP_MARGIN + 4 * UNIT_MARGIN;

const LAUNCHER_SVG: &[u8] = include_bytes!("../asserts/launcher.svg");

const RESET_SVG: &[u8] = include_bytes!("../asserts/reset.svg");

pub fn main() -> Result<(), iced_layershell::Error> {
    env_logger::builder().format_timestamp(None).init();

    LalaMusicBar::run(Settings {
        layer_settings: LayerShellSettings {
            size: Some((0, 35)),
            exclusive_zone: 35,
            anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
            layer: Layer::Top,
            ..Default::default()
        },
        ..Default::default()
    })
}

#[derive(Debug, Clone)]
enum LaLaInfo {
    Launcher,
    Notify(Box<NotifyUnitWidgetInfo>),
    HiddenInfo,
    RightPanel,
}

#[derive(Debug, Clone)]
struct NotifyUnitWidgetInfo {
    upper: i32,
    counter: usize,
    inline_reply: String,
    unit: NotifyUnit,
}

impl NotifyUnitWidgetInfo {
    fn button<'a>(&self, id: Option<iced::window::Id>, hidden: bool) -> Element<'a, Message> {
        let notify = &self.unit;
        let counter = self.counter;
        match notify.image() {
            Some(ImageInfo::Svg(path)) => button(row![
                svg(svg::Handle::from_path(path)).height(Length::Fill),
                Space::with_width(4.),
                column![
                    text(notify.summery.clone())
                        .shaping(text::Shaping::Advanced)
                        .size(20)
                        .font(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    text(notify.body.clone()).shaping(text::Shaping::Advanced)
                ]
            ])
            .style(iced::theme::Button::Secondary)
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(Message::RemoveNotify(id, self.unit.id, counter, hidden))
            .into(),
            Some(ImageInfo::Data {
                width,
                height,
                pixels,
            }) => button(row![
                image(image::Handle::from_pixels(
                    width as u32,
                    height as u32,
                    pixels
                )),
                Space::with_width(4.),
                column![
                    text(notify.summery.clone())
                        .shaping(text::Shaping::Advanced)
                        .size(20)
                        .font(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    text(notify.body.clone()).shaping(text::Shaping::Advanced)
                ]
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(iced::theme::Button::Secondary)
            .on_press(Message::RemoveNotify(id, self.unit.id, counter, hidden))
            .into(),
            Some(ImageInfo::Png(path)) | Some(ImageInfo::Jpg(path)) => button(row![
                image(image::Handle::from_path(path)).height(Length::Fill),
                Space::with_width(4.),
                column![
                    text(notify.summery.clone())
                        .shaping(text::Shaping::Advanced)
                        .size(20)
                        .font(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    text(notify.body.clone()).shaping(text::Shaping::Advanced)
                ]
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(iced::theme::Button::Secondary)
            .on_press(Message::RemoveNotify(id, self.unit.id, counter, hidden))
            .into(),
            _ => button(column![
                text(notify.summery.clone()).shaping(text::Shaping::Advanced),
                text(notify.body.clone()).shaping(text::Shaping::Advanced)
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(iced::theme::Button::Secondary)
            .on_press(Message::RemoveNotify(id, self.unit.id, counter, hidden))
            .into(),
        }
    }
}

#[allow(unused)]
#[derive(Debug)]
enum NotifyCommand {
    ActionInvoked { id: u32, action_key: String },
    InlineReply { id: u32, text: String },
    NotificationClosed { id: u32, reason: u32 },
}

struct LalaMusicBar {
    service_data: Option<ServiceInfo>,
    left: i64,
    right: i64,
    bar_index: SliderIndex,
    launcher: Option<launcher::Launcher>,
    launcherid: Option<iced::window::Id>,
    hidenid: Option<iced::window::Id>,
    right_panel: Option<iced::window::Id>,
    notifications: HashMap<iced::window::Id, NotifyUnitWidgetInfo>,
    hidden_notifications: Vec<NotifyUnitWidgetInfo>,
    sender: Sender<NotifyCommand>,
    receiver: Arc<Mutex<Receiver<NotifyCommand>>>,
    quite_mode: bool,
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
            button(
                svg(svg::Handle::from_memory(RESET_SVG))
                    .height(25.)
                    .width(25.)
            )
            .height(31.)
            .width(31.)
            .on_press(Message::BalanceChanged(50)),
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

    fn right_panel_view(&self) -> Element<Message> {
        let btns: Vec<Element<Message>> = self
            .hidden_notifications
            .iter()
            .rev()
            .map(|wdgetinfo| {
                container(wdgetinfo.button(None, true))
                    .height(Length::Fixed(100.))
                    .into()
            })
            .collect();
        let mut view_elements: Vec<Element<Message>> = vec![];

        if let Some(data) = &self.service_data {
            if let Some(art_url) = url::Url::parse(&data.metadata.mpris_arturl)
                .ok()
                .and_then(|url| url.to_file_path().ok())
            {
                view_elements.push(
                    container(image(image::Handle::from_path(art_url)).width(Length::Fill))
                        .padding(10)
                        .width(Length::Fill)
                        .into(),
                );
                view_elements.push(Space::with_height(10.).into());
                view_elements.push(
                    container(
                        text(&data.metadata.xesam_title)
                            .size(20)
                            .font(Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            })
                            .shaping(text::Shaping::Advanced)
                            .style(iced::theme::Text::Color(iced::Color::WHITE)),
                    )
                    .width(Length::Fill)
                    .center_x()
                    .into(),
                );
                view_elements.push(Space::with_height(10.).into());
            }
        }
        view_elements.append(&mut vec![
            scrollable(column(btns).spacing(10.))
                .height(Length::Fill)
                .into(),
            container(checkbox("quite mode", self.quite_mode).on_toggle(Message::QuiteMode))
                .width(Length::Fill)
                .center_x()
                .into(),
            Space::with_height(10.).into(),
            container(button(text("clear all")).on_press(Message::ClearAllNotifications))
                .width(Length::Fill)
                .center_x()
                .into(),
            Space::with_height(10.).into(),
        ]);
        column(view_elements).into()
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
    ToggleRightPanel,
    LauncherInfo(LaunchMessage),
    Notify(NotifyMessage),
    RemoveNotify(Option<iced::window::Id>, u32, usize, bool),
    InlineReply((iced::window::Id, u32, String)),
    InlineReplyMsgUpdate((iced::window::Id, String)),
    CheckOutput,
    ClearAllNotifications,
    QuiteMode(bool),
}

impl From<NotifyMessage> for Message {
    fn from(value: NotifyMessage) -> Self {
        Self::Notify(value)
    }
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

impl LalaMusicBar {
    fn main_view(&self) -> Element<Message> {
        let title = self
            .service_data
            .as_ref()
            .map(|data| data.metadata.xesam_title.as_str())
            .unwrap_or("No Video here");
        let art_url = self
            .service_data
            .as_ref()
            .and_then(|data| url::Url::parse(&data.metadata.mpris_arturl).ok())
            .and_then(|url| url.to_file_path().ok());
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
        let panel_text = if self.right_panel.is_some() { ">" } else { "<" };
        let col = if let Some(art_url) = art_url {
            row![
                button(
                    svg(svg::Handle::from_memory(LAUNCHER_SVG))
                        .width(25.)
                        .height(25.)
                )
                .on_press(Message::ToggleLauncher),
                Space::with_width(Length::Fixed(5.)),
                image(image::Handle::from_path(art_url)),
                title,
                Space::with_width(Length::Fill),
                buttons,
                sound_slider,
                Space::with_width(Length::Fixed(10.)),
                button(text(panel_text)).on_press(Message::ToggleRightPanel)
            ]
            .spacing(10)
        } else {
            row![
                button(
                    svg(svg::Handle::from_memory(LAUNCHER_SVG))
                        .width(25.)
                        .height(25.)
                )
                .on_press(Message::ToggleLauncher),
                title,
                Space::with_width(Length::Fill),
                buttons,
                sound_slider,
                Space::with_width(Length::Fixed(10.)),
                button(text(panel_text)).on_press(Message::ToggleRightPanel)
            ]
            .spacing(10)
        };

        container(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}

impl MultiApplication for LalaMusicBar {
    type Message = Message;
    type Flags = ();
    type Executor = executor::Default;
    type Theme = Theme;
    type WindowInfo = LaLaInfo;

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        let (sender, receiver) = channel::<NotifyCommand>(100);
        (
            Self {
                service_data: None,
                left: aximer::get_left().unwrap_or(0),
                right: aximer::get_right().unwrap_or(0),
                bar_index: SliderIndex::Balance,
                launcher: None,
                launcherid: None,
                right_panel: None,
                hidenid: None,
                notifications: HashMap::new(),
                hidden_notifications: Vec::new(),
                sender,
                receiver: Arc::new(Mutex::new(receiver)),
                quite_mode: false,
            },
            Command::perform(get_metadata_initial(), Message::DBusInfoUpdate),
        )
    }

    fn namespace(&self) -> String {
        String::from("Mpirs_panel")
    }

    fn id_info(&self, id: iced::window::Id) -> Option<Self::WindowInfo> {
        if self.launcherid.is_some_and(|tid| tid == id) {
            Some(LaLaInfo::Launcher)
        } else if self.hidenid.is_some_and(|tid| tid == id) {
            Some(LaLaInfo::HiddenInfo)
        } else if self.right_panel.is_some_and(|tid| tid == id) {
            Some(LaLaInfo::RightPanel)
        } else {
            self.notifications
                .get(&id)
                .cloned()
                .map(|notifyw| LaLaInfo::Notify(Box::new(notifyw)))
        }
    }

    fn set_id_info(&mut self, id: iced::window::Id, info: Self::WindowInfo) {
        match info {
            LaLaInfo::Launcher => {
                self.launcherid = Some(id);
            }
            LaLaInfo::Notify(notify) => {
                self.notifications.entry(id).or_insert(*notify);
            }
            LaLaInfo::HiddenInfo => {
                self.hidenid = Some(id);
            }
            LaLaInfo::RightPanel => self.right_panel = Some(id),
        }
    }

    fn remove_id(&mut self, id: iced::window::Id) {
        if self.launcherid.is_some_and(|lid| lid == id) {
            self.launcherid.take();
        }
        if self.right_panel.is_some_and(|lid| lid == id) {
            self.right_panel.take();
        }
        if self.hidenid.is_some_and(|lid| lid == id) {
            self.hidenid.take();
        }
        self.notifications.remove(&id);
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
                        LaLaShellIdAction::new(
                            iced::window::Id::MAIN,
                            LalaShellAction::NewLayerShell((
                                NewLayerShellSettings {
                                    size: Some((500, 700)),
                                    exclusive_zone: None,
                                    anchor: Anchor::Left | Anchor::Bottom,
                                    layer: Layer::Top,
                                    margin: None,
                                    keyboard_interactivity: KeyboardInteractivity::Exclusive,
                                    use_last_output: false,
                                },
                                LaLaInfo::Launcher,
                            )),
                        )
                        .into(),
                    ),
                    self.launcher.as_ref().unwrap().focus_input(),
                ]);
            }
            Message::ToggleRightPanel => {
                if self.right_panel.is_some() {
                    if let Some(id) = self.right_panel {
                        self.right_panel.take();
                        return Command::single(Action::Window(WindowAction::Close(id)));
                    }
                    return Command::none();
                }
                return Command::single(
                    LaLaShellIdAction::new(
                        iced::window::Id::MAIN,
                        LalaShellAction::NewLayerShell((
                            NewLayerShellSettings {
                                size: Some((300, 0)),
                                exclusive_zone: Some(300),
                                anchor: Anchor::Right | Anchor::Bottom | Anchor::Top,
                                layer: Layer::Top,
                                margin: None,
                                keyboard_interactivity: KeyboardInteractivity::None,
                                use_last_output: false,
                            },
                            LaLaInfo::RightPanel,
                        )),
                    )
                    .into(),
                );
            }
            Message::Notify(NotifyMessage::UnitAdd(notify)) => {
                for unit in self.hidden_notifications.iter_mut() {
                    unit.upper += 135;
                    unit.counter += 1;
                }
                if self.quite_mode {
                    self.hidden_notifications.push(NotifyUnitWidgetInfo {
                        counter: 0,
                        upper: 10,
                        inline_reply: String::new(),
                        unit: notify,
                    });
                    return Command::none();
                }
                let mut commands = vec![];
                for (id, unit) in self.notifications.iter_mut() {
                    unit.upper += 135;
                    unit.counter += 1;
                    if unit.counter > 3 {
                        self.hidden_notifications.push(unit.clone());
                        commands.push(Command::single(Action::Window(WindowAction::Close(*id))));
                    } else {
                        commands.push(Command::single(
                            LaLaShellIdAction::new(
                                *id,
                                LalaShellAction::MarginChange((unit.upper, 10, 10, 10)),
                            )
                            .into(),
                        ));
                    }
                }

                commands.push(Command::single(
                    LaLaShellIdAction::new(
                        iced::window::Id::MAIN,
                        LalaShellAction::NewLayerShell((
                            NewLayerShellSettings {
                                size: Some((300, 130)),
                                exclusive_zone: None,
                                anchor: Anchor::Right | Anchor::Top,
                                layer: Layer::Top,
                                margin: Some((10, 10, 10, 10)),
                                keyboard_interactivity: KeyboardInteractivity::OnDemand,
                                use_last_output: true,
                            },
                            LaLaInfo::Notify(Box::new(NotifyUnitWidgetInfo {
                                counter: 0,
                                upper: 10,
                                inline_reply: String::new(),
                                unit: notify,
                            })),
                        )),
                    )
                    .into(),
                ));

                if !self.hidden_notifications.is_empty() && self.hidenid.is_none() {
                    commands.push(Command::single(
                        LaLaShellIdAction::new(
                            iced::window::Id::MAIN,
                            LalaShellAction::NewLayerShell((
                                NewLayerShellSettings {
                                    size: Some((300, 25)),
                                    exclusive_zone: None,
                                    anchor: Anchor::Right | Anchor::Top,
                                    layer: Layer::Top,
                                    margin: Some((EXTRAINF_MARGIN, 10, 10, 10)),
                                    keyboard_interactivity: KeyboardInteractivity::None,
                                    use_last_output: true,
                                },
                                LaLaInfo::HiddenInfo,
                            )),
                        )
                        .into(),
                    ));
                }
                return Command::batch(commands);
            }
            Message::QuiteMode(quite) => {
                self.quite_mode = quite;
                let mut commands = vec![];
                if quite {
                    // change to quite
                    let mut values: Vec<&NotifyUnitWidgetInfo> =
                        self.notifications.values().collect();
                    values.sort_by(|a, b| b.counter.partial_cmp(&a.counter).unwrap());

                    for value in values {
                        self.hidden_notifications.push(value.clone());
                    }
                    for id in self.notifications.keys() {
                        commands.push(Command::single(Action::Window(WindowAction::Close(*id))));
                    }
                    if let Some(id) = self.hidenid {
                        commands.push(Command::single(Action::Window(WindowAction::Close(id))));
                    }
                } else {
                    for count in 0..4 {
                        if let Some(index) = self
                            .hidden_notifications
                            .iter()
                            .position(|unit| unit.counter == count)
                        {
                            let unit = &self.hidden_notifications[index];
                            commands.push(Command::single(
                                LaLaShellIdAction::new(
                                    iced::window::Id::MAIN,
                                    LalaShellAction::NewLayerShell((
                                        NewLayerShellSettings {
                                            size: Some((300, 130)),
                                            exclusive_zone: None,
                                            anchor: Anchor::Right | Anchor::Top,
                                            layer: Layer::Top,
                                            margin: Some((unit.upper, 10, 10, 10)),
                                            keyboard_interactivity: KeyboardInteractivity::OnDemand,
                                            use_last_output: true,
                                        },
                                        LaLaInfo::Notify(Box::new(unit.clone())),
                                    )),
                                )
                                .into(),
                            ));
                            self.hidden_notifications.remove(index);
                        }
                    }
                    if !self.hidden_notifications.is_empty() && self.hidenid.is_none() {
                        commands.push(Command::single(
                            LaLaShellIdAction::new(
                                iced::window::Id::MAIN,
                                LalaShellAction::NewLayerShell((
                                    NewLayerShellSettings {
                                        size: Some((300, 25)),
                                        exclusive_zone: None,
                                        anchor: Anchor::Right | Anchor::Top,
                                        layer: Layer::Top,
                                        margin: Some((EXTRAINF_MARGIN, 10, 10, 10)),
                                        keyboard_interactivity: KeyboardInteractivity::None,
                                        use_last_output: true,
                                    },
                                    LaLaInfo::HiddenInfo,
                                )),
                            )
                            .into(),
                        ));
                    }
                }

                if commands.is_empty() {
                    return Command::none();
                } else {
                    return Command::batch(commands);
                }
            }
            Message::Notify(NotifyMessage::UnitRemove(removed_id)) => {
                let removed_ids: Vec<iced::window::Id> = self
                    .notifications
                    .iter()
                    .filter(|(_, info)| {
                        let NotifyUnit { id, .. } = info.unit;
                        removed_id == id
                    })
                    .map(|(id, _)| *id)
                    .collect();
                let mut commands: Vec<_> = self
                    .notifications
                    .iter()
                    .filter(|(_, info)| {
                        let NotifyUnit { id, .. } = info.unit;
                        removed_id == id
                    })
                    .map(|(id, _)| Command::single(Action::Window(WindowAction::Close(*id))))
                    .collect();

                let mut removed_counters = vec![];
                for id in removed_ids.iter() {
                    if let Some(NotifyUnitWidgetInfo { counter, .. }) =
                        self.notifications.remove(id)
                    {
                        removed_counters.push(counter);
                    }
                }
                removed_counters.sort();
                removed_counters.reverse();
                for counter in removed_counters {
                    for (_, unit) in self.notifications.iter_mut() {
                        if unit.counter > counter {
                            unit.counter -= 1;
                            unit.upper -= 135;
                        }
                    }
                }

                for (id, unit) in self.notifications.iter() {
                    commands.push(Command::single(
                        LaLaShellIdAction::new(
                            *id,
                            LalaShellAction::MarginChange((unit.upper, 10, 10, 10)),
                        )
                        .into(),
                    ));
                }

                let mut remove_hided_notifications_count: Vec<usize> = self
                    .hidden_notifications
                    .iter()
                    .filter(
                        |NotifyUnitWidgetInfo {
                             unit: NotifyUnit { id, .. },
                             ..
                         }| *id == removed_id,
                    )
                    .map(|NotifyUnitWidgetInfo { counter, .. }| *counter)
                    .collect();

                if self.notifications.len() < 3 {
                    for index in 0..self.notifications.len() {
                        remove_hided_notifications_count.push(index);
                    }
                }
                remove_hided_notifications_count.sort();
                remove_hided_notifications_count.reverse();

                self.hidden_notifications.retain(
                    |NotifyUnitWidgetInfo {
                         unit: NotifyUnit { id, .. },
                         ..
                     }| *id != removed_id,
                );

                for count in remove_hided_notifications_count {
                    for unit in self.hidden_notifications.iter_mut() {
                        if unit.counter > count {
                            unit.counter -= 1;
                            unit.upper -= 135;
                        }
                    }
                }
                for notify in self.hidden_notifications.iter() {
                    if notify.counter <= 4 {
                        commands.push(Command::single(
                            LaLaShellIdAction::new(
                                iced::window::Id::MAIN,
                                LalaShellAction::NewLayerShell((
                                    NewLayerShellSettings {
                                        size: Some((300, 130)),
                                        exclusive_zone: None,
                                        anchor: Anchor::Right | Anchor::Top,
                                        layer: Layer::Top,
                                        margin: Some((notify.upper, 10, 10, 10)),
                                        keyboard_interactivity: KeyboardInteractivity::OnDemand,
                                        use_last_output: true,
                                    },
                                    LaLaInfo::Notify(Box::new(notify.clone())),
                                )),
                            )
                            .into(),
                        ));
                    }
                }

                self.hidden_notifications
                    .retain(|NotifyUnitWidgetInfo { counter, .. }| *counter > 4);

                if self.hidden_notifications.is_empty() && self.hidenid.is_some() {
                    let hidenid = self.hidenid.unwrap();

                    commands.push(Command::single(Action::Window(WindowAction::Close(
                        hidenid,
                    ))));
                }

                commands.push(Command::perform(async {}, |_| Message::CheckOutput));

                return Command::batch(commands);
            }
            Message::RemoveNotify(id, notify_id, counter, is_hidden) => {
                self.sender
                    .try_send(NotifyCommand::ActionInvoked {
                        id: notify_id,
                        action_key: DEFAULT_ACTION.to_string(),
                    })
                    .ok();

                let mut commands = vec![];

                if !is_hidden {
                    let removed_pos = self
                        .notifications
                        .iter()
                        .find(|(oid, _)| id.is_some_and(|id| id == **oid))
                        .map(|(_, info)| info.upper)
                        .unwrap_or(0);
                    for (id, unit) in self.notifications.iter_mut() {
                        if unit.upper > removed_pos {
                            unit.upper -= 135;
                        }
                        if unit.counter > counter {
                            unit.counter -= 1;
                        }
                        commands.push(Command::single(
                            LaLaShellIdAction::new(
                                *id,
                                LalaShellAction::MarginChange((unit.upper, 10, 10, 10)),
                            )
                            .into(),
                        ));
                    }
                }
                let mut to_show_id = None;

                let mut to_removed_index = None;
                for (index, notify) in self.hidden_notifications.iter_mut().enumerate() {
                    if counter > notify.counter {
                        continue;
                    }
                    if counter == notify.counter {
                        to_removed_index = Some(index);
                    }
                    notify.counter -= 1;
                    notify.upper -= 135;
                    if notify.counter == 3 && !is_hidden {
                        to_show_id = Some(index);
                        commands.push(Command::single(
                            LaLaShellIdAction::new(
                                iced::window::Id::MAIN,
                                LalaShellAction::NewLayerShell((
                                    NewLayerShellSettings {
                                        size: Some((300, 130)),
                                        exclusive_zone: None,
                                        anchor: Anchor::Right | Anchor::Top,
                                        layer: Layer::Top,
                                        margin: Some((notify.upper, 10, 10, 10)),
                                        keyboard_interactivity: KeyboardInteractivity::OnDemand,
                                        use_last_output: true,
                                    },
                                    LaLaInfo::Notify(Box::new(notify.clone())),
                                )),
                            )
                            .into(),
                        ));
                    }
                }

                if is_hidden {
                    if let Some(index) = to_removed_index {
                        self.hidden_notifications.remove(index);
                    }
                }

                if let Some(index) = to_show_id {
                    self.hidden_notifications.remove(index);
                }

                if self.hidden_notifications.is_empty() && self.hidenid.is_some() {
                    let hidenid = self.hidenid.unwrap();

                    commands.push(Command::single(Action::Window(WindowAction::Close(
                        hidenid,
                    ))));
                }

                if let Some(id) = id {
                    commands.push(Command::single(Action::Window(WindowAction::Close(id))));
                }
                commands.push(Command::perform(async {}, |_| Message::CheckOutput));

                return Command::batch(commands);
            }
            Message::CheckOutput => {
                if self.notifications.is_empty() {
                    return Command::single(
                        LaLaShellIdAction::new(
                            iced::window::Id::MAIN,
                            LalaShellAction::ForgetLastOutput,
                        )
                        .into(),
                    );
                }
            }
            Message::LauncherInfo(message) => {
                if let Some(launcher) = self.launcher.as_mut() {
                    if let Some(id) = self.launcherid {
                        let cmd = launcher.update(message, id);
                        if launcher.should_delete {
                            self.launcher.take();
                        }
                        return cmd;
                    }
                }
            }
            Message::InlineReply((id, notify_id, text)) => {
                self.sender
                    .try_send(NotifyCommand::InlineReply {
                        id: notify_id,
                        text,
                    })
                    .ok();
                let removed_pos = self
                    .notifications
                    .iter()
                    .find(|(oid, _)| **oid == id)
                    .map(|(_, info)| info.upper)
                    .unwrap_or(0);

                let mut commands = vec![];
                for (id, unit) in self.notifications.iter_mut() {
                    if unit.upper > removed_pos {
                        unit.upper -= 135;
                        unit.counter -= 1;
                    }
                    commands.push(Command::single(
                        LaLaShellIdAction::new(
                            *id,
                            LalaShellAction::MarginChange((unit.upper, 10, 10, 10)),
                        )
                        .into(),
                    ));
                }
                commands.append(&mut vec![
                    Command::single(Action::Window(WindowAction::Close(id))),
                    Command::perform(async {}, |_| Message::CheckOutput),
                ]);

                return Command::batch(commands);
            }
            Message::InlineReplyMsgUpdate((id, msg)) => {
                let notify = self.notifications.get_mut(&id).unwrap();
                notify.inline_reply = msg;
            }
            Message::ClearAllNotifications => {
                self.hidden_notifications.clear();
                let mut commands = self
                    .notifications
                    .keys()
                    .map(|id| Command::single(Action::Window(WindowAction::Close(*id))))
                    .collect::<Vec<_>>();

                if let Some(id) = self.hidenid {
                    commands.push(Command::single(Action::Window(WindowAction::Close(id))));
                }
                return Command::batch(commands);
            }
        }
        Command::none()
    }

    fn view(&self, id: iced::window::Id) -> Element<Message> {
        if let Some(info) = self.id_info(id) {
            match info {
                LaLaInfo::Launcher => {
                    if let Some(launcher) = &self.launcher {
                        return launcher.view();
                    }
                }
                LaLaInfo::Notify(unitwidgetinfo) => {
                    let btnwidgets: Element<Message> = unitwidgetinfo.button(Some(id), false);

                    let notify = &unitwidgetinfo.unit;
                    let notifywidget = self.notifications.get(&id).unwrap();
                    if notify.inline_reply_support() {
                        return column![
                            btnwidgets,
                            Space::with_height(5.),
                            row![
                                text_input("reply something", &notifywidget.inline_reply)
                                    .on_input(move |msg| Message::InlineReplyMsgUpdate((id, msg)))
                                    .on_submit(Message::InlineReply((
                                        id,
                                        notify.id,
                                        notifywidget.inline_reply.clone()
                                    ))),
                                button("send").on_press(Message::InlineReply((
                                    id,
                                    notify.id,
                                    notifywidget.inline_reply.clone()
                                ))),
                            ]
                        ]
                        .into();
                    }
                    return btnwidgets;
                }
                LaLaInfo::HiddenInfo => {
                    return text(format!(
                        "hidden notifications {}",
                        self.hidden_notifications.len()
                    ))
                    .into();
                }
                LaLaInfo::RightPanel => {
                    return self.right_panel_view();
                }
            }
        }
        self.main_view()
    }

    fn subscription(&self) -> iced::Subscription<Self::Message> {
        let rv = self.receiver.clone();
        iced::subscription::Subscription::batch([
            iced::time::every(std::time::Duration::from_secs(1))
                .map(|_| Message::RequestDBusInfoUpdate),
            iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::UpdateBalance),
            iced::event::listen()
                .map(|event| Message::LauncherInfo(LaunchMessage::IcedEvent(event))),
            iced::subscription::channel(std::any::TypeId::of::<()>(), 100, |sender| async move {
                let mut receiver = rv.lock().await;
                let Ok(connection) = start_connection(
                    sender,
                    vec![
                        "body".to_owned(),
                        "body-markup".to_owned(),
                        "actions".to_owned(),
                        "icon-static".to_owned(),
                        "x-canonical-private-synchronous".to_owned(),
                        "x-dunst-stack-tag".to_owned(),
                        "inline-reply".to_owned(),
                    ],
                    VersionInfo {
                        name: "LaLaMako".to_owned(),
                        vendor: "waycrate".to_owned(),
                        version: env!("CARGO_PKG_VERSION").to_owned(),
                        spec_version: env!("CARGO_PKG_VERSION_PATCH").to_owned(),
                    },
                )
                .await
                else {
                    pending::<()>().await;
                    unreachable!()
                };
                type LaLaMakoMusic = LaLaMako<Message>;
                let Ok(lalaref) = connection
                    .object_server()
                    .interface::<_, LaLaMakoMusic>(NOTIFICATION_SERVICE_PATH)
                    .await
                else {
                    pending::<()>().await;
                    unreachable!()
                };

                while let Some(cmd) = receiver.next().await {
                    match cmd {
                        NotifyCommand::ActionInvoked { id, action_key } => {
                            LaLaMakoMusic::action_invoked(
                                lalaref.signal_context(),
                                id,
                                &action_key,
                            )
                            .await
                            .ok();
                        }
                        NotifyCommand::InlineReply { id, text } => {
                            LaLaMakoMusic::notification_replied(
                                lalaref.signal_context(),
                                id,
                                &text,
                            )
                            .await
                            .ok();
                        }
                        NotifyCommand::NotificationClosed { id, reason } => {
                            LaLaMakoMusic::notification_closed(
                                lalaref.signal_context(),
                                id,
                                reason,
                            )
                            .await
                            .ok();
                        }
                    }
                }
                pending::<()>().await;
                unreachable!()
            }),
        ])
    }

    fn theme(&self) -> Self::Theme {
        Theme::TokyoNight
    }
}
