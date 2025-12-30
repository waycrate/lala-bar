use std::io::{Read, Write};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
pub struct SettingsConfig {
    pub(crate) background_color: Option<String>,
}

fn ensure_file() {
    let Ok(home) = std::env::var("HOME") else {
        return;
    };
    let config_dir = std::path::Path::new(home.as_str())
        .join(".config")
        .join("lala-bar");
    if !config_dir.exists() {
        let _ = std::fs::create_dir_all(&config_dir);
    }
    let config_file = config_dir.join("config.toml");
    if !config_file.exists() {
        let _ = std::fs::File::create(config_file);
    }
}

impl SettingsConfig {
    pub fn read_from_file() -> Self {
        ensure_file();
        let Ok(home) = std::env::var("HOME") else {
            return Self::default();
        };
        let config_path = std::path::Path::new(home.as_str())
            .join(".config")
            .join("lala-bar")
            .join("config.toml");
        let Ok(mut file) = std::fs::OpenOptions::new().read(true).open(config_path) else {
            return Self::default();
        };
        let mut buf = String::new();
        if file.read_to_string(&mut buf).is_err() {
            return Self::default();
        };
        toml::from_str(&buf).unwrap_or(Self::default())
    }
    pub fn write_to_file(&self) {
        let Ok(context) = toml::to_string_pretty(&self) else {
            return;
        };
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let config_path = std::path::Path::new(home.as_str())
            .join(".config")
            .join("lala-bar")
            .join("config.toml");
        let Ok(mut file) = std::fs::OpenOptions::new().write(true).open(config_path) else {
            return;
        };
        let _ = file.write(context.as_bytes());
    }
    pub fn background(&self) -> Option<iced::Color> {
        let background_color = self.background_color.as_ref()?;
        let color = csscolorparser::parse(background_color).ok()?;
        Some(iced::Color::from_rgb(color.r, color.g, color.b))
    }
    pub fn set_background(&mut self, color: iced::Color) {
        let r = (color.r * 255.) as i32;
        let g = (color.g * 255.) as i32;
        let b = (color.b * 255.) as i32;
        let color = format!("#{:02x}{:02x}{:02x}", r, g, b);
        self.background_color = Some(color)
    }
    pub fn reset(&mut self) {
        *self = Self::default();
        self.write_to_file();
    }
}
