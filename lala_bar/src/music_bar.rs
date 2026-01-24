use crate::ColorPickerResult;
use crate::Launcher;
use crate::RightPanelFilter;
use crate::config::*;
use crate::dbusbackend;
use crate::get_metadata;
use crate::launcher::LaunchMessage;
use crate::notify::{NotifyCommand, NotifyUnitWidgetInfo};
use crate::settings::SettingsConfig;
use crate::slider::SliderIndex;
use crate::wav_canvars;
use crate::wav_canvars::PwEvent;
use crate::wav_canvars::WavState;
use crate::zbus_mpirs::ServiceInfo;
use crate::{LaLaInfo, Message, get_metadata_initial};
use crate::{aximer, launcher};
use chrono::{DateTime, Local};
use futures::StreamExt;
use futures::channel::mpsc::{Sender, channel};
use futures::future::pending;
use iced::widget::canvas;
use iced::widget::{
    Space, button, checkbox, column, container, image, markdown, row, scrollable, slider, svg,
    text, text_input,
};
use iced::{Alignment, Element, Font, Length, Task as Command, Theme};
use iced_aw::{date_picker::Date, helpers::date_picker, time_picker, time_picker::Time};
use iced_layershell::reexport::OutputOption;
use iced_layershell::reexport::{Anchor, KeyboardInteractivity, Layer, NewLayerShellSettings};
use iced_layershell::settings::LayerShellSettings;
use iced_layershell::settings::StartMode;
use iced_runtime::Action;
use iced_runtime::window::Action as WindowAction;
use iced_zbus_notification::MessageSenderDefault;
use iced_zbus_notification::{
    DEFAULT_ACTION, LaLaMako, NOTIFICATION_SERVICE_PATH, NotifyMessage, VersionInfo,
};
use std::collections::HashMap;

use iced_layershell::build_pattern::daemon;

pub fn run_lalabar() -> iced_layershell::Result {
    daemon(
        LalaMusicBar::new,
        LalaMusicBar::namespace,
        LalaMusicBar::update,
        LalaMusicBar::view,
    )
    .layer_settings(LayerShellSettings {
        size: Some((0, 35)),
        exclusive_zone: 35,
        anchor: Anchor::Bottom | Anchor::Left | Anchor::Right,
        layer: Layer::Top,
        start_mode: StartMode::AllScreens,

        ..Default::default()
    })
    .theme(LalaMusicBar::theme)
    .subscription(LalaMusicBar::subscription)
    .font(iced_aw::ICED_AW_FONT_BYTES)
    .run()
}

pub struct LalaMusicBar {
    pub(crate) service_data: Option<ServiceInfo>,
    pub(crate) left: i64,
    right: i64,
    left_text: String,
    right_text: String,
    balance_text: String,
    bar_index: SliderIndex,
    launcher: Option<launcher::Launcher>,
    launcherid: Option<iced::window::Id>,
    hiddenid: Option<iced::window::Id>,
    right_panel: Option<iced::window::Id>,
    notifications: HashMap<u32, NotifyUnitWidgetInfo>,
    pub(crate) notifications_markdown: HashMap<u32, Vec<markdown::Item>>,
    showned_notifications: HashMap<iced::window::Id, u32>,
    cached_notifications: HashMap<iced::window::Id, NotifyUnitWidgetInfo>,
    cached_hidden_notifications: Vec<NotifyUnitWidgetInfo>,
    sender: Option<Sender<NotifyCommand>>,
    check_sender: Option<Sender<bool>>,
    quite_mode: bool,
    datetime: DateTime<Local>,
    calendar_id: Option<iced::window::Id>,
    date: Date,
    time: Time,
    time_picker_id: Option<iced::window::Id>,
    right_filter: RightPanelFilter,

    bar_settings: SettingsConfig,

    wav_data: wav_canvars::WavState,
}

async fn color_pick() -> ColorPickerResult {
    use ashpd::desktop::Color;
    let Ok(response) = Color::pick().send().await else {
        return ColorPickerResult::Failed;
    };
    let Ok(color) = response.response() else {
        return ColorPickerResult::Failed;
    };
    ColorPickerResult::Color(iced::Color::from_rgb(
        color.red() as f32,
        color.green() as f32,
        color.blue() as f32,
    ))
}

impl LalaMusicBar {
    pub fn date_widget(&'_ self) -> Element<'_, Message> {
        let date = self.datetime.date_naive();
        let dateday = date.format("%m-%d").to_string();
        let week = date.format("%A").to_string();
        let time = self.datetime.time();
        let time_info = time.format("%H:%M").to_string();

        let date_btn = button(text(format!("{week} {dateday}")))
            .on_press(Message::ToggleCalendar)
            .style(button::secondary);

        let time_btn = button(text(time_info))
            .on_press(Message::ToggleTime)
            .style(button::secondary);

        container(row![time_btn, Space::new().width(5.), date_btn,])
            .center_y(Length::Fill)
            .height(Length::Fill)
            .into()
    }
    pub fn update_hidden_notification(&mut self) {
        let mut hiddened: Vec<NotifyUnitWidgetInfo> = self
            .notifications
            .values()
            .filter(|info| {
                (info.counter >= MAX_SHOWN_NOTIFICATIONS_COUNT || self.quite_mode)
                    && !info.to_delete
            })
            .cloned()
            .collect();

        hiddened.sort_by(|a, b| a.counter.partial_cmp(&b.counter).unwrap());

        self.cached_hidden_notifications = hiddened;
    }

    fn hidden_notification(&self) -> &Vec<NotifyUnitWidgetInfo> {
        &self.cached_hidden_notifications
    }
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
        self.left_text = format!("left {}%", self.left);
        self.right_text = format!("right {}%", self.right);
        self.balance_text = format!("balance {}%", self.balance_percent());
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
    // NOTE: not use signal to invoke remove, but use a common function
    fn remove_notify(&mut self, removed_id: u32) -> Command<Message> {
        let mut commands = vec![];
        let removed_counter = if let Some(removed_unit) = self.notifications.get_mut(&removed_id) {
            // NOTE: marked it as removable, but not now
            // Data should be removed by iced_layershell, not ourself
            removed_unit.to_delete = true;
            removed_unit.counter
        } else {
            // NOTE: already removed
            return Command::none();
        };

        for (_, notify_info) in self.notifications.iter_mut() {
            if notify_info.counter > removed_counter {
                notify_info.counter -= 1;
                notify_info.upper -= 135;
            }
        }

        let mut showned_values: Vec<(&iced::window::Id, &mut u32)> =
            self.showned_notifications.iter_mut().collect();

        showned_values.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());

        let mut notifications: Vec<&u32> = self
            .notifications
            .iter()
            .filter(|(_, v)| !v.to_delete)
            .map(|(k, _)| k)
            .collect();

        notifications.sort_by(|a, b| b.partial_cmp(a).unwrap());

        let mut notification_iter = notifications.iter();

        let mut has_removed = false;

        for (id, nid) in showned_values.into_iter() {
            if let Some(onid) = notification_iter.next() {
                *nid = **onid;
            } else {
                if let Some(info) = self.notifications.get(nid) {
                    self.cached_notifications.insert(*id, info.clone());
                }
                *nid = removed_id;
                commands.push(iced_runtime::task::effect(Action::Window(
                    WindowAction::Close(*id),
                )));
                has_removed = true;
            }
        }
        if !has_removed {
            self.notifications.retain(|_, v| !v.to_delete);
        }

        if self.quite_mode || removed_counter >= 4 {
            self.notifications.remove(&removed_id);
        }
        let notifications_count = self
            .notifications
            .iter()
            .filter(|(_, v)| !v.to_delete)
            .count();

        // NOTE: we should delete to be deleted notification
        if notifications_count <= MAX_SHOWN_NOTIFICATIONS_COUNT
            && let Some(id) = self.hiddenid
        {
            commands.push(iced_runtime::task::effect(Action::Window(
                WindowAction::Close(id),
            )));
        }

        if notifications_count == 0 {
            commands.push(Command::perform(async {}, |_| Message::CheckOutput));
        }

        self.update_hidden_notification();

        Command::batch(commands)
    }
}

impl LalaMusicBar {
    fn balance_bar(&'_ self) -> Element<'_, Message> {
        row![
            button("<").on_press(Message::SliderIndexPre),
            text(&self.balance_text),
            slider(0..=100, self.balance_percent(), Message::BalanceChanged),
            button(
                svg(svg::Handle::from_memory(RESET_SVG))
                    .height(25.)
                    .width(25.)
            )
            .height(31.)
            .width(31.)
            .on_press(Message::BalanceChanged(50)),
            button(">").on_press(Message::SliderIndexNext)
        ]
        .spacing(5.)
        .align_y(Alignment::Center)
        .into()
    }
    fn left_bar(&'_ self) -> Element<'_, Message> {
        row![
            button("<").on_press(Message::SliderIndexPre),
            text(&self.left_text),
            slider(0..=100, self.left as u8, Message::UpdateLeft),
            button(">").on_press(Message::SliderIndexNext)
        ]
        .spacing(5.)
        .align_y(Alignment::Center)
        .into()
    }
    fn right_bar(&'_ self) -> Element<'_, Message> {
        row![
            button("<").on_press(Message::SliderIndexPre),
            text(&self.right_text),
            slider(0..=100, self.right as u8, Message::UpdateRight),
            button(">").on_press(Message::SliderIndexNext)
        ]
        .spacing(5.)
        .align_y(Alignment::Center)
        .into()
    }

    fn sound_slider(&'_ self) -> Element<'_, Message> {
        match self.bar_index {
            SliderIndex::Left => self.left_bar(),
            SliderIndex::Right => self.right_bar(),
            SliderIndex::Balance => self.balance_bar(),
        }
    }
}

impl LalaMusicBar {
    fn right_panel_view(&'_ self) -> Element<'_, Message> {
        let filter_button = |bytes, filter, current_filter| {
            button(svg(svg::Handle::from_memory(bytes)))
                .style(if filter == current_filter {
                    button::primary
                } else {
                    button::text
                })
                .width(40.)
                .height(40.)
                .on_press(Message::RightPanelFilterChanged(filter))
        };
        let notification_btn = filter_button(
            NOTIFICATION_SVG,
            RightPanelFilter::Notifications,
            self.right_filter,
        );
        let settings_btn =
            filter_button(SETTINGS_SVG, RightPanelFilter::Settings, self.right_filter);
        let buttons = container(
            column![Space::new().height(10.), notification_btn, settings_btn].spacing(10.),
        )
        .style(|_| container::Style {
            background: Some(iced::Background::Color(iced::Color::from_rgb(
                0.3, 0.4, 0.4,
            ))),
            ..Default::default()
        })
        .height(Length::Fill);
        let main_view = match self.right_filter {
            RightPanelFilter::Settings => self.right_settings(),
            RightPanelFilter::Notifications => self.right_notification(),
        };
        container(row![main_view, buttons])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
    fn right_settings(&'_ self) -> Element<'_, Message> {
        let color_settings = container(row![
            container(text("background color:")).center_y(Length::Fill),
            Space::new().width(20.),
            button("pick").on_press(Message::PickerColor)
        ])
        .center_y(30.)
        .center_x(Length::Fill);
        let settings =
            scrollable(column![Space::new().height(30.), color_settings]).height(Length::Fill);
        let reset_button =
            container(button(text("reset")).on_press(Message::ResetConfig)).center_x(Length::Fill);
        container(column![settings, reset_button, Space::new().height(10.)])
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
    fn right_notification(&'_ self) -> Element<'_, Message> {
        let btns: Vec<Element<Message>> = self
            .hidden_notification()
            .iter()
            .map(|wdgetinfo| {
                container(wdgetinfo.notify_button(self))
                    .height(Length::Fixed(100.))
                    .into()
            })
            .collect();
        let mut view_elements: Vec<Element<Message>> = vec![];

        if let Some(data) = &self.service_data {
            if let Some(handle) = &data.metadata.mpris_image {
                view_elements.push(
                    container(image(handle).width(Length::Fill))
                        .padding(10)
                        .width(Length::Fill)
                        .into(),
                );
                view_elements.push(Space::new().height(10.).into());
                view_elements.push(
                    container(
                        text(&data.metadata.xesam_title)
                            .size(20)
                            .font(Font {
                                weight: iced::font::Weight::Bold,
                                ..Default::default()
                            })
                            .shaping(text::Shaping::Advanced)
                            .style(|_theme| text::Style {
                                color: Some(iced::Color::WHITE),
                            }),
                    )
                    .width(Length::Fill)
                    .center_x(Length::Fill)
                    .into(),
                );
                view_elements.push(Space::new().height(10.).into());
            }
        }
        view_elements.append(&mut vec![
            Space::new().height(10.).into(),
            scrollable(row!(
                Space::new().width(10.),
                column(btns).spacing(10.),
                Space::new().width(10.)
            ))
            .height(Length::Fill)
            .into(),
            container(
                checkbox(self.quite_mode)
                    .label("quite mode")
                    .on_toggle(Message::QuiteMode),
            )
            .width(Length::Fill)
            .center_x(Length::Fill)
            .into(),
            Space::new().height(10.).into(),
            container(button(text("clear all")).on_press(Message::ClearAllNotifications))
                .width(Length::Fill)
                .center_x(Length::Fill)
                .into(),
            Space::new().height(10.).into(),
        ]);
        column(view_elements).into()
    }
}

impl LalaMusicBar {
    fn main_view(&self) -> Element<'_, Message> {
        let toggle_launcher = button(
            svg(svg::Handle::from_memory(LAUNCHER_SVG))
                .width(25.)
                .height(25.),
        )
        .on_press(Message::ToggleLauncher);

        let sound_slider = container(self.sound_slider()).center_y(Length::Fill);
        let panel_text = if self.right_panel.is_some() { ">" } else { "<" };

        let panel_btn = container(button(text(panel_text)).on_press(Message::ToggleRightPanel))
            .center_y(Length::Fill);
        let Some(service_data) = &self.service_data else {
            let col = row![
                toggle_launcher,
                Space::new().width(Length::Fill),
                container(sound_slider).width(600.),
                Space::new().width(Length::Fixed(3.)),
                self.date_widget(),
                Space::new().width(Length::Fixed(3.)),
                panel_btn
            ]
            .spacing(10);
            return container(col)
                .width(Length::Fill)
                .height(Length::Fill)
                .center_x(Length::Fill)
                .center_y(Length::Fill)
                .into();
        };
        let title = &service_data.metadata.xesam_title;
        let handle_option = &service_data.metadata.mpris_image;

        let title = container(
            text(title)
                .size(20)
                .font(Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                })
                .shaping(text::Shaping::Advanced)
                .style(|_theme| text::Style {
                    color: Some(iced::Color::WHITE),
                }),
        )
        .width(Length::Fill)
        .center_x(Length::Fill);
        let wav_chat = canvas(&self.wav_data)
            .width(Length::Fixed(350.))
            .height(Length::Fill);

        let can_play = service_data.can_play;
        let can_pause = service_data.can_pause;
        let can_go_next = service_data.can_go_next;
        let can_go_pre = service_data.can_go_previous;
        let mut button_pre = button(svg(GO_PREVIOUS_HANDLE.clone()).width(25.).height(25.))
            .width(30.)
            .height(30.);
        if can_go_pre {
            button_pre = button_pre.on_press(Message::RequestPre);
        }
        let mut button_next = button(svg(GO_NEXT_HANDLE.clone()).width(25.).height(25.))
            .width(30.)
            .height(30.);
        if can_go_next {
            button_next = button_next.on_press(Message::RequestNext);
        }
        let button_play = if service_data.playback_status == "Playing" {
            let mut btn = button(svg(PAUSE_HANDLE.clone()).width(25.).height(25.))
                .width(30.)
                .height(30.);
            if can_pause {
                btn = btn.on_press(Message::RequestPause);
            }
            btn
        } else {
            let mut btn = button(svg(PLAY_HANDLE.clone()).width(25.).height(25.))
                .width(30.)
                .height(30.);
            if can_play {
                btn = btn.on_press(Message::RequestPlay);
            }
            btn
        };
        let buttons = container(row![button_pre, button_play, button_next].spacing(5))
            .width(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill);

        let col = if let Some(handle) = handle_option {
            row![
                toggle_launcher,
                Space::new().width(Length::Fixed(5.)),
                image(handle),
                title,
                wav_chat,
                buttons,
                sound_slider,
                Space::new().width(Length::Fixed(3.)),
                self.date_widget(),
                Space::new().width(Length::Fixed(3.)),
                panel_btn
            ]
            .spacing(10)
        } else {
            row![
                toggle_launcher,
                title,
                wav_chat,
                buttons,
                sound_slider,
                Space::new().width(Length::Fixed(3.)),
                self.date_widget(),
                Space::new().width(Length::Fixed(1.)),
                panel_btn
            ]
            .spacing(10)
        };

        container(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x(Length::Fill)
            .center_y(Length::Fill)
            .into()
    }
}

impl LalaMusicBar {
    fn new() -> (Self, Command<Message>) {
        (
            Self {
                service_data: None,
                left: 0,
                right: 0,
                left_text: "".to_string(),
                right_text: "".to_string(),
                balance_text: "".to_string(),
                bar_index: SliderIndex::Balance,
                launcher: None,
                launcherid: None,
                right_panel: None,
                hiddenid: None,
                notifications: HashMap::new(),
                notifications_markdown: HashMap::new(),
                showned_notifications: HashMap::new(),
                cached_notifications: HashMap::new(),
                cached_hidden_notifications: Vec::new(),
                sender: None,
                check_sender: None,
                quite_mode: false,
                datetime: Local::now(),
                calendar_id: None,
                date: Date::today(),
                time: Time::now_hm(true),
                time_picker_id: None,
                right_filter: RightPanelFilter::Notifications,
                bar_settings: SettingsConfig::read_from_file(),
                wav_data: WavState::new(),
            },
            Command::batch(vec![
                Command::done(Message::UpdateBalance),
                Command::perform(get_metadata_initial(), Message::DBusInfoUpdate),
            ]),
        )
    }

    fn namespace() -> String {
        String::from("Mpirs_panel")
    }

    fn id_info(&self, id: iced::window::Id) -> Option<LaLaInfo> {
        if self.launcherid.is_some_and(|tid| tid == id) {
            Some(LaLaInfo::Launcher)
        } else if self.hiddenid.is_some_and(|tid| tid == id) {
            Some(LaLaInfo::HiddenInfo)
        } else if self.time_picker_id.is_some_and(|tid| tid == id) {
            Some(LaLaInfo::TimePicker)
        } else if self.right_panel.is_some_and(|tid| tid == id) {
            Some(LaLaInfo::RightPanel)
        } else if self.calendar_id.is_some_and(|tid| tid == id) {
            Some(LaLaInfo::Calendar)
        } else {
            if let Some(info) = self.cached_notifications.get(&id) {
                return Some(LaLaInfo::Notify(Box::new(info.clone())));
            }
            let notify_id = self.showned_notifications.get(&id)?;
            Some(
                self.notifications
                    .get(notify_id)
                    .cloned()
                    .map(|notifyw| LaLaInfo::Notify(Box::new(notifyw)))
                    .unwrap_or(LaLaInfo::ErrorHappened(id)),
            )
        }
    }

    fn set_id_info(&mut self, id: iced::window::Id, info: LaLaInfo) {
        match info {
            LaLaInfo::Launcher => {
                self.launcherid = Some(id);
            }
            LaLaInfo::Notify(notify) => {
                self.showned_notifications.insert(id, notify.unit.id);
            }
            LaLaInfo::HiddenInfo => {
                self.hiddenid = Some(id);
            }
            LaLaInfo::RightPanel => self.right_panel = Some(id),
            LaLaInfo::Calendar => self.calendar_id = Some(id),
            LaLaInfo::TimePicker => self.time_picker_id = Some(id),
            _ => unreachable!(),
        }
    }

    fn remove_id(&mut self, id: iced::window::Id) {
        if self.launcherid.is_some_and(|lid| lid == id) {
            self.launcherid.take();
            self.launcher.take();
        }
        if self.right_panel.is_some_and(|lid| lid == id) {
            self.right_panel.take();
        }
        if self.hiddenid.is_some_and(|lid| lid == id) {
            self.hiddenid.take();
        }
        if self.calendar_id.is_some_and(|lid| lid == id) {
            self.calendar_id.take();
        }
        if self.time_picker_id.is_some_and(|lid| lid == id) {
            self.time_picker_id.take();
        }
        'clear_nid: {
            if let Some(nid) = self.showned_notifications.remove(&id) {
                if let Some(NotifyUnitWidgetInfo {
                    to_delete: false, ..
                }) = self.notifications.get(&nid)
                {
                    break 'clear_nid;
                }
                // If the widget is marked to removed
                // Then delete it
                self.notifications.remove(&nid);
                self.notifications_markdown.remove(&nid);
            }
        }
        self.cached_notifications.remove(&id);
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::Tick => {
                self.wav_data.update_canvas();
            }
            Message::Pw(PwEvent::FormatChange(format)) => {
                let channel = format.channels();
                self.wav_data.reset_matrix(500, channel as usize);
            }
            Message::Pw(PwEvent::DataNew(data)) => {
                self.wav_data.append_data(data);
            }
            Message::Pw(PwEvent::PwErr) => {
                tracing::warn!("pw connection is broken");
            }
            Message::DBusInfoUpdate(data) => self.service_data = data,
            Message::RequestDBusInfoUpdate => {
                return Command::perform(get_metadata(), Message::DBusInfoUpdate);
            }
            Message::ToggleCalendar => {
                if let Some(calendar_id) = self.calendar_id {
                    return iced_runtime::task::effect(Action::Window(WindowAction::Close(
                        calendar_id,
                    )));
                } else {
                    let id = iced::window::Id::unique();
                    self.set_id_info(id, LaLaInfo::Calendar);
                    if let Some(time_picker_id) = self.time_picker_id {
                        return iced_runtime::task::Task::batch([
                            iced_runtime::task::effect(Action::Window(WindowAction::Close(
                                time_picker_id,
                            ))),
                            Command::done(Message::NewLayerShell {
                                settings: NewLayerShellSettings {
                                    size: Some((350, 350)),
                                    exclusive_zone: None,
                                    anchor: Anchor::Right | Anchor::Bottom,
                                    layer: Layer::Top,
                                    margin: Some((10, 10, 10, 10)),
                                    keyboard_interactivity: KeyboardInteractivity::None,
                                    output_option: OutputOption::LastOutput,
                                    ..Default::default()
                                },
                                id,
                            }),
                        ]);
                    }
                    return Command::done(Message::NewLayerShell {
                        settings: NewLayerShellSettings {
                            size: Some((350, 350)),
                            exclusive_zone: None,
                            anchor: Anchor::Right | Anchor::Bottom,
                            layer: Layer::Top,
                            margin: Some((10, 10, 10, 10)),
                            keyboard_interactivity: KeyboardInteractivity::None,
                            output_option: OutputOption::None,
                            ..Default::default()
                        },
                        id,
                    });
                }
            }
            Message::ToggleTime => {
                if let Some(time_picker_id) = self.time_picker_id {
                    return iced_runtime::task::effect(Action::Window(WindowAction::Close(
                        time_picker_id,
                    )));
                } else {
                    let id = iced::window::Id::unique();
                    self.set_id_info(id, LaLaInfo::TimePicker);
                    if let Some(calendar_id) = self.calendar_id {
                        return iced_runtime::task::Task::batch([
                            iced_runtime::task::effect(Action::Window(WindowAction::Close(
                                calendar_id,
                            ))),
                            Command::done(Message::NewLayerShell {
                                settings: NewLayerShellSettings {
                                    size: Some((350, 350)),
                                    exclusive_zone: None,
                                    anchor: Anchor::Right | Anchor::Bottom,
                                    layer: Layer::Top,
                                    margin: Some((10, 10, 10, 10)),
                                    keyboard_interactivity: KeyboardInteractivity::None,
                                    output_option: OutputOption::LastOutput,
                                    ..Default::default()
                                },
                                id,
                            }),
                        ]);
                    }
                    return Command::done(Message::NewLayerShell {
                        settings: NewLayerShellSettings {
                            size: Some((350, 350)),
                            exclusive_zone: None,
                            anchor: Anchor::Right | Anchor::Bottom,
                            layer: Layer::Top,
                            margin: Some((10, 10, 10, 10)),
                            keyboard_interactivity: KeyboardInteractivity::None,
                            output_option: OutputOption::None,
                            ..Default::default()
                        },
                        id,
                    });
                }
            }
            // NOTE: it is meaningless to pick the date now
            Message::SubmitDate(_) | Message::CancelDate => {
                if let Some(id) = self.calendar_id {
                    return iced_runtime::task::effect(Action::Window(WindowAction::Close(id)));
                }
            }
            Message::SubmitTime(_) | Message::CancelTime => {
                if let Some(id) = self.time_picker_id {
                    return iced_runtime::task::effect(Action::Window(WindowAction::Close(id)));
                }
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
                        return iced_runtime::task::effect(Action::Window(WindowAction::Close(id)));
                    }
                    return Command::none();
                }
                self.launcher = Some(Launcher::new());
                let id = iced::window::Id::unique();
                self.set_id_info(id, LaLaInfo::Launcher);
                return Command::batch(vec![
                    Command::done(Message::NewLayerShell {
                        settings: NewLayerShellSettings {
                            size: Some((500, 700)),
                            exclusive_zone: None,
                            anchor: Anchor::Left | Anchor::Bottom,
                            layer: Layer::Top,
                            margin: None,
                            keyboard_interactivity: KeyboardInteractivity::Exclusive,
                            ..Default::default()
                        },
                        id,
                    }),
                    self.launcher.as_ref().unwrap().focus_input(),
                ]);
            }
            Message::ToggleLauncherDBus => {
                if self.launcher.is_some() {
                    if let Some(id) = self.launcherid {
                        return iced_runtime::task::effect(Action::Window(WindowAction::Close(id)));
                    }
                    return Command::none();
                }
                self.launcher = Some(Launcher::new());
                let id = iced::window::Id::unique();
                self.set_id_info(id, LaLaInfo::Launcher);
                return Command::batch(vec![
                    Command::done(Message::NewLayerShell {
                        settings: NewLayerShellSettings {
                            size: Some((1200, 1000)),
                            margin: None,

                            exclusive_zone: None,
                            anchor: Anchor::Left | Anchor::Bottom | Anchor::Right | Anchor::Top,
                            layer: Layer::Top,
                            keyboard_interactivity: KeyboardInteractivity::Exclusive,
                            ..Default::default()
                        },
                        id,
                    }),
                    self.launcher.as_ref().unwrap().focus_input(),
                ]);
            }
            Message::ToggleRightPanel => {
                if self.right_panel.is_some() {
                    if let Some(id) = self.right_panel {
                        return iced_runtime::task::effect(Action::Window(WindowAction::Close(id)));
                    }
                    return Command::none();
                }
                let id = iced::window::Id::unique();
                self.set_id_info(id, LaLaInfo::RightPanel);
                return Command::done(Message::NewLayerShell {
                    settings: NewLayerShellSettings {
                        size: Some((300, 0)),
                        exclusive_zone: Some(300),
                        anchor: Anchor::Right | Anchor::Bottom | Anchor::Top,
                        layer: Layer::Top,
                        margin: None,
                        keyboard_interactivity: KeyboardInteractivity::None,
                        output_option: OutputOption::None,
                        ..Default::default()
                    },
                    id,
                });
            }
            Message::Notify(NotifyMessage::UnitAdd(notify)) => {
                if let Some(onotify) = self.notifications.get_mut(&notify.id) {
                    onotify.unit = *notify;
                    return Command::none();
                }
                let mut commands = vec![];
                for (_, notify) in self.notifications.iter_mut() {
                    notify.upper += 135;
                    notify.counter += 1;
                }

                // NOTE: support timeout
                if notify.timeout != -1 {
                    let timeout = notify.timeout as u64;
                    let id = notify.id;
                    commands.push(Command::perform(
                        async move {
                            tokio::time::sleep(std::time::Duration::from_secs(timeout)).await
                        },
                        move |_| Message::RemoveNotify(id),
                    ))
                }

                self.notifications_markdown
                    .insert(notify.id, markdown::parse(&notify.body).collect());
                self.notifications.insert(
                    notify.id,
                    NotifyUnitWidgetInfo {
                        to_delete: false,
                        counter: 0,
                        upper: 10,
                        inline_reply: String::new(),
                        unit: *notify.clone(),
                    },
                );

                if !self.quite_mode {
                    let all_shown = self.showned_notifications.len() == 4;
                    let mut showned_notifications_now: Vec<(&u32, &NotifyUnitWidgetInfo)> = self
                        .notifications
                        .iter()
                        .filter(|(_, info)| info.counter < 4 && !info.to_delete)
                        .collect();

                    showned_notifications_now.sort_by(|(a, _), (b, _)| b.partial_cmp(a).unwrap());
                    let mut showned_values: Vec<(&iced::window::Id, &mut u32)> =
                        self.showned_notifications.iter_mut().collect();

                    showned_values.sort_by(|(_, a), (_, b)| b.partial_cmp(a).unwrap());

                    // NOTE: if all is shown, then do not add any commands
                    if all_shown {
                        let mut showned_values_iter = showned_values.iter_mut();

                        for (nid, _unit) in showned_notifications_now {
                            if let Some((_, onid)) = showned_values_iter.next() {
                                (**onid) = *nid;
                            }
                        }
                    } else {
                        // NOTE: remove the new one
                        let to_adjust_notification = &showned_notifications_now[1..];
                        for ((_, unit), (id, _)) in
                            to_adjust_notification.iter().zip(showned_values.iter())
                        {
                            commands.push(Command::done(Message::MarginChange {
                                id: **id,
                                margin: (unit.upper, 10, 10, 10),
                            }));
                        }

                        drop(showned_values);
                        drop(showned_notifications_now);

                        let id = iced::window::Id::unique();
                        self.set_id_info(
                            id,
                            LaLaInfo::Notify(Box::new(NotifyUnitWidgetInfo {
                                to_delete: false,
                                counter: 0,
                                upper: 10,
                                inline_reply: String::new(),
                                unit: *notify.clone(),
                            })),
                        );
                        // NOTE: if not all shown, then do as the way before
                        commands.push(Command::done(Message::NewLayerShell {
                            settings: NewLayerShellSettings {
                                size: Some((300, 130)),
                                exclusive_zone: None,
                                anchor: Anchor::Right | Anchor::Top,
                                layer: Layer::Top,
                                margin: Some((10, 10, 10, 10)),
                                keyboard_interactivity: KeyboardInteractivity::OnDemand,
                                output_option: OutputOption::LastOutput,
                                events_transparent: false,
                                ..Default::default()
                            },
                            id,
                        }));
                    }
                }

                self.update_hidden_notification();

                if !self.hidden_notification().is_empty()
                    && !self.quite_mode
                    && self.hiddenid.is_none()
                {
                    let id = iced::window::Id::unique();
                    self.set_id_info(id, LaLaInfo::HiddenInfo);
                    commands.push(Command::done(Message::NewLayerShell {
                        settings: NewLayerShellSettings {
                            size: Some((300, 25)),
                            exclusive_zone: None,
                            anchor: Anchor::Right | Anchor::Top,
                            layer: Layer::Top,
                            margin: Some((EXTRAINF_MARGIN, 10, 10, 10)),
                            keyboard_interactivity: KeyboardInteractivity::OnDemand,
                            output_option: iced_layershell::reexport::OutputOption::LastOutput,
                            ..Default::default()
                        },
                        id,
                    }));
                }

                return Command::batch(commands);
            }

            Message::QuiteMode(quite) => {
                self.quite_mode = quite;
                let mut commands = vec![];
                if quite {
                    for (id, _nid) in self.showned_notifications.iter() {
                        commands.push(iced_runtime::task::effect(Action::Window(
                            WindowAction::Close(*id),
                        )));
                    }
                    if let Some(extra_id) = self.hiddenid {
                        commands.push(iced_runtime::task::effect(Action::Window(
                            WindowAction::Close(extra_id),
                        )));
                    }
                } else {
                    let notifications = std::mem::take(&mut self.notifications);
                    for (_, notify_info) in notifications
                        .iter()
                        .filter(|(_, info)| info.counter < MAX_SHOWN_NOTIFICATIONS_COUNT)
                    {
                        let id = iced::window::Id::unique();
                        self.set_id_info(id, LaLaInfo::Notify(Box::new(notify_info.clone())));
                        commands.push(Command::done(Message::NewLayerShell {
                            settings: NewLayerShellSettings {
                                size: Some((300, 130)),
                                exclusive_zone: None,
                                anchor: Anchor::Right | Anchor::Top,
                                layer: Layer::Top,
                                margin: Some((notify_info.upper, 10, 10, 10)),
                                keyboard_interactivity: KeyboardInteractivity::OnDemand,
                                output_option: OutputOption::LastOutput,
                                ..Default::default()
                            },
                            id,
                        }));
                    }
                    self.notifications = notifications;
                    if self.notifications.len() > MAX_SHOWN_NOTIFICATIONS_COUNT
                        && self.hiddenid.is_none()
                    {
                        let id = iced::window::Id::unique();
                        self.set_id_info(id, LaLaInfo::HiddenInfo);
                        commands.push(Command::done(Message::NewLayerShell {
                            settings: NewLayerShellSettings {
                                size: Some((300, 25)),
                                exclusive_zone: None,
                                anchor: Anchor::Right | Anchor::Top,
                                layer: Layer::Top,
                                margin: Some((EXTRAINF_MARGIN, 10, 10, 10)),
                                keyboard_interactivity: KeyboardInteractivity::None,
                                output_option: OutputOption::LastOutput,
                                ..Default::default()
                            },
                            id,
                        }));
                    }
                }
                self.update_hidden_notification();

                return Command::batch(commands);
            }

            Message::Notify(NotifyMessage::UnitRemove(removed_id)) => {
                return self.remove_notify(removed_id);
            }

            Message::CheckOutput => {
                return Command::done(Message::ForgetLastOutput);
            }
            Message::LauncherInfo(message) => {
                if let Some(launcher) = self.launcher.as_mut()
                    && let Some(id) = self.launcherid
                {
                    return launcher.update(message, id);
                }
            }
            Message::InlineReply((notify_id, text)) => {
                self.sender
                    .as_mut()
                    .unwrap()
                    .try_send(NotifyCommand::InlineReply {
                        id: notify_id,
                        text,
                    })
                    .ok();
                return self.remove_notify(notify_id);
            }
            Message::RemoveNotify(notify_id) => {
                self.sender
                    .as_mut()
                    .unwrap()
                    .try_send(NotifyCommand::ActionInvoked {
                        id: notify_id,
                        action_key: DEFAULT_ACTION.to_string(),
                    })
                    .ok();
                return self.remove_notify(notify_id);
            }
            Message::InlineReplyMsgUpdate((id, msg)) => {
                let Some(notify_id) = self.showned_notifications.get(&id) else {
                    return Command::none();
                };
                let notify = self.notifications.get_mut(notify_id).unwrap();
                notify.inline_reply = msg;
            }
            Message::ClearAllNotifications => {
                let mut commands = self
                    .showned_notifications
                    .keys()
                    .map(|id| iced_runtime::task::effect(Action::Window(WindowAction::Close(*id))))
                    .collect::<Vec<_>>();

                if let Some(id) = self.hiddenid {
                    commands.push(iced_runtime::task::effect(Action::Window(
                        WindowAction::Close(id),
                    )));
                }

                for (id, nid) in self.showned_notifications.iter() {
                    if let Some(info) = self.notifications.get(nid) {
                        self.cached_notifications.insert(*id, info.clone());
                    }
                }

                self.notifications_markdown.clear();
                self.notifications.clear();
                self.update_hidden_notification();
                commands.push(Command::done(Message::CheckOutput));
                return Command::batch(commands);
            }
            Message::CloseErrorNotification(id) => {
                return iced_runtime::task::effect(Action::Window(WindowAction::Close(id)));
            }
            Message::RequestUpdateTime => {
                self.datetime = Local::now();
                self.date = self.datetime.date_naive().into();
                self.time = self.datetime.time().into()
            }
            Message::Ready(sender) => self.sender = Some(sender),
            Message::ReadyCheck(check_sender) => self.check_sender = Some(check_sender),
            Message::CheckId(id) => {
                let contain = self.notifications.contains_key(&id);
                let _ = self.check_sender.as_mut().unwrap().try_send(contain);
            }
            Message::LinkClicked(link) => {
                open::that_in_background(&link);
            }
            Message::WindowClosed(id) => {
                self.remove_id(id);
            }
            Message::RightPanelFilterChanged(filter) => {
                self.right_filter = filter;
            }
            Message::PickerColor => {
                return Command::perform(color_pick(), Message::PickerColorDone);
            }
            Message::PickerColorDone(info) => {
                let ColorPickerResult::Color(color) = info else {
                    return Command::none();
                };
                self.bar_settings.set_background(color);
                self.bar_settings.write_to_file();
            }
            Message::ResetConfig => {
                self.bar_settings.reset();
            }
            _ => unreachable!(),
        }
        Command::none()
    }

    fn view(&'_ self, id: iced::window::Id) -> Element<'_, Message> {
        if let Some(info) = self.id_info(id) {
            match info {
                LaLaInfo::Launcher => {
                    if let Some(launcher) = &self.launcher {
                        return launcher.view();
                    }
                }
                LaLaInfo::Calendar => {
                    return container(date_picker(
                        true,
                        self.date,
                        button(text("Pick date")),
                        Message::CancelDate,
                        Message::SubmitDate,
                    ))
                    .center_y(Length::Fill)
                    .center_x(Length::Fill)
                    .into();
                }

                LaLaInfo::TimePicker => {
                    return container(time_picker(
                        true,
                        self.time,
                        button(text("Pick time")),
                        Message::CancelTime,
                        Message::SubmitTime,
                    ))
                    .center_y(Length::Fill)
                    .center_x(Length::Fill)
                    .into();
                }
                LaLaInfo::Notify(unitwidgetinfo) => {
                    let btnwidgets: Element<Message> = unitwidgetinfo.notify_button(self);

                    let notify = &unitwidgetinfo.unit;
                    if notify.inline_reply_support() {
                        return column![
                            btnwidgets,
                            Space::new().height(5.),
                            row![
                                text_input("reply something", &unitwidgetinfo.inline_reply)
                                    .on_input(move |msg| Message::InlineReplyMsgUpdate((id, msg)))
                                    .on_submit(Message::InlineReply((
                                        notify.id,
                                        unitwidgetinfo.inline_reply.clone()
                                    ))),
                                button("send").on_press(Message::InlineReply((
                                    notify.id,
                                    unitwidgetinfo.inline_reply.clone()
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
                        self.hidden_notification().len()
                    ))
                    .into();
                }
                LaLaInfo::RightPanel => {
                    return self.right_panel_view();
                }
                LaLaInfo::ErrorHappened(id) => {
                    tracing::error!("Error happened, for window id: {id:?}");
                    return button(row![
                        svg(svg::Handle::from_memory(ERROR_SVG))
                            .height(Length::Fill)
                            .width(Length::Fixed(70.)),
                        Space::new().width(4.),
                        text("Error Happened, LaLa cannot find notification for this window, it is a bug, and should be fixed")
                    ]).on_press(Message::CloseErrorNotification(id)).into();
                }
            }
        }
        self.main_view()
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch([
            iced::window::frames().map(|_| Message::Tick),
            wav_canvars::listen_pw().map(Message::Pw),
            iced::time::every(std::time::Duration::from_secs(1))
                .map(|_| Message::RequestDBusInfoUpdate),
            iced::time::every(std::time::Duration::from_secs(10))
                .map(|_| Message::RequestUpdateTime),
            iced::time::every(std::time::Duration::from_secs(5)).map(|_| Message::UpdateBalance),
            iced::event::listen()
                .map(|event| Message::LauncherInfo(LaunchMessage::IcedEvent(event))),
            iced::Subscription::run(|| {
                iced::stream::channel(100, |output| async move {
                    use dbusbackend::start_backend;
                    let _conn = start_backend(output).await.expect("already registered");
                    pending::<()>().await;
                    unreachable!()
                })
            }),
            iced::window::close_events().map(Message::WindowClosed),
            iced::Subscription::run(|| {
                iced::stream::channel(100, |mut output: Sender<Message>| async move {
                    use iced::futures::sink::SinkExt;
                    let (sender, mut receiver) = channel(100);
                    let (check_sender, mut check_receiver) = channel(100);

                    // Send the sender back to the application
                    output.send(Message::Ready(sender)).await.ok();
                    output.send(Message::ReadyCheck(check_sender)).await.ok();
                    let output_check = output.clone();
                    let Ok(connection) = LaLaMako::new(
                        MessageSenderDefault(output),
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
                    .with_check(move |id| {
                        let mut output_check = output_check.clone();
                        if output_check.try_send(Message::CheckId(id)).is_err() {
                            return false;
                        }
                        if let Ok(Some(true)) = check_receiver.try_next() {
                            return true;
                        }
                        false
                    })
                    .connect()
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
                                    lalaref.signal_emitter(),
                                    id,
                                    &action_key,
                                )
                                .await
                                .ok();
                            }
                            NotifyCommand::InlineReply { id, text } => {
                                LaLaMakoMusic::notification_replied(
                                    lalaref.signal_emitter(),
                                    id,
                                    &text,
                                )
                                .await
                                .ok();
                            }
                            NotifyCommand::NotificationClosed { id, reason } => {
                                LaLaMakoMusic::notification_closed(
                                    lalaref.signal_emitter(),
                                    id,
                                    reason,
                                )
                                .await
                                .ok();
                            }
                        }
                    }
                    pending::<()>().await;
                })
            }),
        ])
    }

    pub fn theme(&self, id: iced::window::Id) -> iced::Theme {
        if self.id_info(id).is_some() {
            return Theme::TokyoNight;
        }
        let Some(background) = self.bar_settings.background() else {
            return Theme::TokyoNight;
        };
        iced::Theme::custom(
            "sakura",
            iced::theme::Palette {
                background,
                ..Theme::TokyoNight.palette()
            },
        )
    }
}
