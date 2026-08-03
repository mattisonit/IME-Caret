use std::fs;
use std::io;
use std::path::Path;

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
            1 => Self::Above,
            2 => Self::Below,
            _ => Self::Right,
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            play_english_sound: true,
            play_japanese_sound: true,
            play_korean_sound: true,
            play_sounds: true,
            indicator_position: IndicatorPosition::Right,
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
                "IndicatorPosition={}\r\n"
            ),
            as_ini_bool(self.play_english_sound),
            as_ini_bool(self.play_japanese_sound),
            as_ini_bool(self.play_korean_sound),
            as_ini_bool(self.play_sounds),
            self.indicator_position.as_ini_value(),
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

fn as_ini_bool(value: bool) -> i32 {
    i32::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_enable_notifications() {
        let config = Config::default();
        assert!(config.play_sounds);
        assert!(config.play_english_sound);
        assert!(config.play_japanese_sound);
        assert!(config.play_korean_sound);
        assert_eq!(config.indicator_position, IndicatorPosition::Right);
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
    fn indicator_position_values_are_stable_and_default_to_right() {
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
            IndicatorPosition::Right
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
}
