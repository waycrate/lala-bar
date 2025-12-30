use iced::widget::svg;
use std::sync::LazyLock;

pub const BEGINNING_UP_MARGIN: i32 = 10;

pub const UNIT_MARGIN: i32 = 135;

pub const EXTRAINF_MARGIN: i32 = BEGINNING_UP_MARGIN + 4 * UNIT_MARGIN;

pub const LAUNCHER_SVG: &[u8] = include_bytes!("../asserts/launcher.svg");

pub const NOTIFICATION_SVG: &[u8] = include_bytes!("../asserts/notification.svg");

pub const SETTINGS_SVG: &[u8] = include_bytes!("../asserts/settings.svg");

pub const RESET_SVG: &[u8] = include_bytes!("../asserts/reset.svg");

pub const ERROR_SVG: &[u8] = include_bytes!("../asserts/error.svg");

pub const GO_NEXT: &[u8] = include_bytes!("../asserts/go-next.svg");

pub static GO_NEXT_HANDLE: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(GO_NEXT));

pub const GO_PREVIOUS: &[u8] = include_bytes!("../asserts/go-previous.svg");

pub static GO_PREVIOUS_HANDLE: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(GO_PREVIOUS));

pub const PLAY: &[u8] = include_bytes!("../asserts/play.svg");

pub static PLAY_HANDLE: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(PLAY));

pub const PAUSE: &[u8] = include_bytes!("../asserts/pause.svg");

pub static PAUSE_HANDLE: LazyLock<svg::Handle> = LazyLock::new(|| svg::Handle::from_memory(PAUSE));

pub const MAX_SHOWN_NOTIFICATIONS_COUNT: usize = 4;
