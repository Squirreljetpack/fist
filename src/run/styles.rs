use crate::utils::categories::FileCategory;
use matchmaker::nucleo::{Color, Modifier, Style};

// -------- STYLES -------------
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct FileStyles {
    pub image: Style,
    pub video: Style,
    pub music: Style,
    pub lossless: Style,
    pub crypto: Style,
    pub document: Style,
    pub compressed: Style,
    pub temp: Style,
    pub compiled: Style,
    pub build: Style,
    pub source: Style,
    pub configuration: Style,
    pub text: Style,
}

impl FileStyles {
    pub fn style(
        &self,
        file: &FileCategory,
    ) -> Style {
        match file {
            FileCategory::Image => self.image,
            FileCategory::Video => self.video,
            FileCategory::Music => self.music,
            FileCategory::Lossless => self.lossless,
            FileCategory::Crypto => self.crypto,
            FileCategory::Document => self.document,
            FileCategory::Compressed => self.compressed,
            FileCategory::Temp => self.temp,
            FileCategory::Compiled => self.compiled,
            FileCategory::Build => self.build,
            FileCategory::Source => self.source,
            FileCategory::Configuration => self.configuration,
            FileCategory::Text => self.text,
        }
    }
}

impl FileStyles {
    pub const DEFAULT: Self = Self {
        image: Style {
            fg: Some(Color::Magenta),
            bg: None,
            add_modifier: Modifier::empty(),
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        video: Style {
            fg: Some(Color::Magenta),
            bg: None,
            add_modifier: Modifier::BOLD,
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        music: Style {
            fg: Some(Color::Cyan),
            bg: None,
            add_modifier: Modifier::empty(),
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        lossless: Style {
            fg: Some(Color::Cyan),
            bg: None,
            add_modifier: Modifier::BOLD,
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        crypto: Style {
            fg: Some(Color::Green),
            bg: None,
            add_modifier: Modifier::BOLD,
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        document: Style {
            fg: Some(Color::Green),
            bg: None,
            add_modifier: Modifier::empty(),
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        compressed: Style {
            fg: Some(Color::Red),
            bg: None,
            add_modifier: Modifier::empty(),
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        temp: Style {
            fg: None,
            bg: None,
            add_modifier: Modifier::DIM,
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        compiled: Style {
            fg: Some(Color::Yellow),
            bg: None,
            add_modifier: Modifier::empty(),
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        build: Style {
            fg: Some(Color::Yellow),
            bg: None,
            add_modifier: Modifier::from_bits_truncate(
                Modifier::BOLD.bits() | Modifier::UNDERLINED.bits(),
            ),
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        source: Style {
            fg: Some(Color::Yellow),
            bg: None,
            add_modifier: Modifier::BOLD,
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        configuration: Style {
            fg: Some(Color::Blue),
            bg: None,
            add_modifier: Modifier::empty(),
            sub_modifier: Modifier::empty(),
            underline_color: None,
        },
        text: Style::new(),
    };
}

impl Default for FileStyles {
    fn default() -> Self {
        Self::DEFAULT
    }
}
