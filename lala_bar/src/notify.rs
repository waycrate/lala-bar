use crate::{LalaMusicBar, Message};
use iced::widget::{Space, Stack, button, column, image, markdown, row, svg, text};
use iced::{Font, Length};
use iced_zbus_notification::{ImageInfo, NotifyUnit};

#[derive(Debug, Clone, PartialEq)]
pub struct NotifyUnitWidgetInfo {
    pub to_delete: bool,
    pub upper: i32,
    pub counter: usize,
    pub inline_reply: String,
    pub unit: NotifyUnit,
}

impl NotifyUnitWidgetInfo {
    pub fn notify_button<'a>(&self, bar: &'a LalaMusicBar) -> iced::Element<'a, Message> {
        let notify = &self.unit;
        let notify_theme = if notify.is_critical() {
            button::primary
        } else {
            button::secondary
        };

        let markdown_info = bar.notifications_markdown.get(&self.unit.id);
        let text_render_text: iced::Element<Message> = match markdown_info {
            Some(data) => markdown::view(data, iced::Theme::TokyoNight).map(Message::LinkClicked),
            None => text(notify.body.clone())
                .shaping(text::Shaping::Advanced)
                .into(),
        };

        let text_render = Stack::new().push(text_render_text).push(
            button("")
                .style(|_theme, status| {
                    let color = match status {
                        button::Status::Hovered => {
                            iced::Color::from_rgba(0.118, 0.193, 0.188, 0.65)
                        }
                        _ => iced::Color::TRANSPARENT,
                    };
                    button::Style {
                        background: Some(iced::Background::Color(color)),
                        ..Default::default()
                    }
                })
                .width(Length::Fill)
                .height(Length::Fill)
                .on_press(Message::RemoveNotify(self.unit.id)),
        );

        match notify.image() {
            Some(ImageInfo::Svg(path)) => button(row![
                svg(svg::Handle::from_path(path))
                    .height(Length::Fill)
                    .width(Length::Fixed(70.)),
                Space::with_width(4.),
                column![
                    text(notify.summery.clone())
                        .shaping(text::Shaping::Advanced)
                        .size(20)
                        .font(Font {
                            weight: iced::font::Weight::Bold,
                            ..Default::default()
                        }),
                    text_render
                ]
            ])
            .style(notify_theme)
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(Message::RemoveNotify(self.unit.id))
            .into(),
            Some(ImageInfo::RgbaRaw {
                pixels,
                width,
                height,
            }) => button(row![
                image(image::Handle::from_rgba(
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
                    text_render
                ]
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(notify_theme)
            .on_press(Message::RemoveNotify(self.unit.id))
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
                    text_render
                ]
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(button::secondary)
            .on_press(Message::RemoveNotify(self.unit.id))
            .into(),
            _ => button(column![
                text(notify.summery.clone()).shaping(text::Shaping::Advanced),
                text_render
            ])
            .width(Length::Fill)
            .height(Length::Fill)
            .style(notify_theme)
            .on_press(Message::RemoveNotify(self.unit.id))
            .into(),
        }
    }
}

#[allow(unused)]
#[derive(Debug)]
pub enum NotifyCommand {
    ActionInvoked { id: u32, action_key: String },
    InlineReply { id: u32, text: String },
    NotificationClosed { id: u32, reason: u32 },
}
