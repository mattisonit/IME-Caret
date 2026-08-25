use std::fs;
use std::io;
use std::path::Path;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl RgbaColor {
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.len() != 8 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        let packed = u32::from_str_radix(value, 16).ok()?;
        Some(Self {
            red: (packed >> 24) as u8,
            green: (packed >> 16) as u8,
            blue: (packed >> 8) as u8,
            alpha: packed as u8,
        })
    }

    pub fn as_rrggbbaa(self) -> String {
        format!(
            "{:02X}{:02X}{:02X}{:02X}",
            self.red, self.green, self.blue, self.alpha
        )
    }
}

pub const DEFAULT_INDICATOR_TEXT_COLOR: RgbaColor = RgbaColor::new(0xff, 0xff, 0xff, 0xa5);
pub const DEFAULT_ENGLISH_BACKGROUND_COLOR: RgbaColor =
    RgbaColor::new(0xff, 0x62, 0x62, 0xa5);
pub const DEFAULT_JAPANESE_BACKGROUND_COLOR: RgbaColor =
    RgbaColor::new(0x62, 0xff, 0x62, 0xa5);
pub const DEFAULT_KOREAN_BACKGROUND_COLOR: RgbaColor =
    RgbaColor::new(0x62, 0x62, 0xff, 0xa5);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndicatorPosition {
    Right,
    Above,
    Below,
}

impl IndicatorPosition {
    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "right" => Some(Self::Right),
            "above" => Some(Self::Above),
            "below" => Some(Self::Below),
            _ => None,
        }
    }

    fn as_ini_value(self) -> &'static str {
        match self {
            Self::Right => "Right",
            Self::Above => "Above",
            Self::Below => "Below",
        }
    }

    pub fn combo_index(self) -> usize {
        match self {
            Self::Right => 0,
            Self::Above => 1,
            Self::Below => 2,
        }
    }

    pub fn from_combo_index(index: usize) -> Self {
        match index {
            0 => Self::Right,
            1 => Self::Above,
            _ => Self::Below,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Config {
    pub play_english_sound: bool,
    pub play_japanese_sound: bool,
    pub play_korean_sound: bool,
    pub play_sounds: bool,
    pub indicator_position: IndicatorPosition,
    pub indicator_text_color: RgbaColor,
    pub english_background_color: RgbaColor,
    pub japanese_background_color: RgbaColor,
    pub korean_background_color: RgbaColor,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            play_english_sound: true,
            play_japanese_sound: true,
            play_korean_sound: true,
            play_sounds: false,
            indicator_position: IndicatorPosition::Below,
            indicator_text_color: DEFAULT_INDICATOR_TEXT_COLOR,
            english_background_color: DEFAULT_ENGLISH_BACKGROUND_COLOR,
            japanese_background_color: DEFAULT_JAPANESE_BACKGROUND_COLOR,
            korean_background_color: DEFAULT_KOREAN_BACKGROUND_COLOR,
        }
    }
}

impl Config {
    pub fn load(path: &Path) -> Self {
        let Ok(bytes) = fs::read(path) else {
            return Self::default();
        };
        let text = decode_ini_text(&bytes);

        let mut config = Self::default();
        let mut in_settings = false;

        for raw_line in text.lines() {
            let line = raw_line.trim().trim_start_matches('\u{feff}');
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }
            if line.starts_with('[') && line.ends_with(']') {
                in_settings = line[1..line.len() - 1]
                    .trim()
                    .eq_ignore_ascii_case("Settings");
                continue;
            }
            if !in_settings {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let raw_value = value.trim();
            let numeric_value = raw_value.parse::<i32>().ok();
            let as_bool = match numeric_value {
                Some(0) => Some(false),
                Some(1) => Some(true),
                _ => None,
            };

            match key.as_str() {
                "playenglishsound" => set_bool(&mut config.play_english_sound, as_bool),
                "playjapanesesound" => set_bool(&mut config.play_japanese_sound, as_bool),
                "playkoreansound" => set_bool(&mut config.play_korean_sound, as_bool),
                "playsounds" => set_bool(&mut config.play_sounds, as_bool),
                "indicatorposition" => {
                    if let Some(position) = IndicatorPosition::parse(raw_value) {
                        config.indicator_position = position;
                    }
                }
                "indicatortextcolor" => set_color(&mut config.indicator_text_color, raw_value),
                "englishbackgroundcolor" => {
                    set_color(&mut config.english_background_color, raw_value)
                }
                "japanesebackgroundcolor" => {
                    set_color(&mut config.japanese_background_color, raw_value)
                }
                "koreanbackgroundcolor" => {
                    set_color(&mut config.korean_background_color, raw_value)
                }
                _ => {}
            }
        }

        config
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let body = format!(
            concat!(
                "[Settings]\r\n",
                "PlayEnglishSound={}\r\n",
                "PlayJapaneseSound={}\r\n",
                "PlayKoreanSound={}\r\n",
                "PlaySounds={}\r\n",
                "IndicatorPosition={}\r\n",
                "IndicatorTextColor={}\r\n",
                "EnglishBackgroundColor={}\r\n",
                "JapaneseBackgroundColor={}\r\n",
                "KoreanBackgroundColor={}\r\n"
            ),
            as_ini_bool(self.play_english_sound),
            as_ini_bool(self.play_japanese_sound),
            as_ini_bool(self.play_korean_sound),
            as_ini_bool(self.play_sounds),
            self.indicator_position.as_ini_value(),
            self.indicator_text_color.as_rrggbbaa(),
            self.english_background_color.as_rrggbbaa(),
            self.japanese_background_color.as_rrggbbaa(),
            self.korean_background_color.as_rrggbbaa(),
        );

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, body)
    }
}

fn decode_ini_text(bytes: &[u8]) -> String {
    if bytes.starts_with(&[0xff, 0xfe]) {
        let units = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16_lossy(&units);
    }
    if bytes.starts_with(&[0xfe, 0xff]) {
        let units = bytes[2..]
            .chunks_exact(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes).into_owned()
}

fn set_bool(slot: &mut bool, value: Option<bool>) {
    if let Some(value) = value {
        *slot = value;
    }
}

fn set_color(slot: &mut RgbaColor, value: &str) {
    if let Some(value) = RgbaColor::parse(value) {
        *slot = value;
    }
}

fn as_ini_bool(value: bool) -> i32 {
    i32::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_disable_all_sounds_but_keep_each_language_enabled() {
        let config = Config::default();
        assert!(!config.play_sounds);
        assert!(config.play_english_sound);
        assert!(config.play_japanese_sound);
        assert!(config.play_korean_sound);
        assert_eq!(config.indicator_position, IndicatorPosition::Below);
        assert_eq!(config.indicator_text_color.as_rrggbbaa(), "FFFFFFA5");
        assert_eq!(
            config.english_background_color.as_rrggbbaa(),
            "FF6262A5"
        );
        assert_eq!(
            config.japanese_background_color.as_rrggbbaa(),
            "62FF62A5"
        );
        assert_eq!(config.korean_background_color.as_rrggbbaa(), "6262FFA5");
    }

    #[test]
    fn decodes_utf16_ini() {
        let source = "[Settings]\r\nPlayKoreanSound=0\r\n";
        let mut bytes = vec![0xff, 0xfe];
        for unit in source.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        let decoded = decode_ini_text(&bytes);
        assert!(decoded.contains("PlayKoreanSound=0"));
    }

    #[test]
    fn indicator_position_values_are_stable_and_fallback_to_below() {
        assert_eq!(
            IndicatorPosition::parse(" Above "),
            Some(IndicatorPosition::Above)
        );
        assert_eq!(
            IndicatorPosition::parse("below"),
            Some(IndicatorPosition::Below)
        );
        assert_eq!(IndicatorPosition::parse("invalid"), None);
        assert_eq!(
            IndicatorPosition::from_combo_index(99),
            IndicatorPosition::Below
        );
    }

    #[test]
    fn indicator_position_is_saved_and_loaded() {
        let path = std::env::temp_dir()
            .join(format!("ime-caret-config-{}.ini", std::process::id()));
        let mut config = Config::default();
        config.indicator_position = IndicatorPosition::Below;
        config.save(&path).unwrap();

        let loaded = Config::load(&path);
        let _ = fs::remove_file(path);
        assert_eq!(loaded.indicator_position, IndicatorPosition::Below);
    }

    #[test]
    fn rgba_colors_are_parsed_and_saved_in_rrggbbaa_order() {
        assert_eq!(
            RgbaColor::parse("12aB34cD"),
            Some(RgbaColor::new(0x12, 0xab, 0x34, 0xcd))
        );
        assert_eq!(RgbaColor::parse("1234567"), None);
        assert_eq!(RgbaColor::parse("1234567Z"), None);

        let path = std::env::temp_dir()
            .join(format!("ime-caret-color-config-{}.ini", std::process::id()));
        let mut config = Config::default();
        config.indicator_text_color = RgbaColor::new(1, 2, 3, 4);
        config.english_background_color = RgbaColor::new(5, 6, 7, 8);
        config.japanese_background_color = RgbaColor::new(9, 10, 11, 12);
        config.korean_background_color = RgbaColor::new(13, 14, 15, 16);
        config.save(&path).unwrap();

        let loaded = Config::load(&path);
        let _ = fs::remove_file(path);
        assert_eq!(loaded.indicator_text_color, config.indicator_text_color);
        assert_eq!(
            loaded.english_background_color,
            config.english_background_color
        );
        assert_eq!(
            loaded.japanese_background_color,
            config.japanese_background_color
        );
        assert_eq!(loaded.korean_background_color, config.korean_background_color);
    }
}
