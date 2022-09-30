use std::collections::HashSet;

use regex::Regex;
use serde_derive::Deserialize;

fn deserialize_categories<'de, D>(deserializer: D) -> Result<HashSet<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let regex = Regex::new(
        r"^sponsor|selfpromo|interaction|poi_highlight|intro|outro|preview|music_offtopic|filler|exclusive_access+$"
    ).unwrap();
    let categories: HashSet<String> = serde::Deserialize::deserialize(deserializer)?;
    Ok(categories
        .into_iter()
        .filter(|v| regex.is_match(v))
        .collect())
}

fn default_server_address() -> String {
    "https://sponsor.ajay.app".to_string()
}

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(default = "default_server_address")]
    pub server_address: String,
    #[serde(default = "HashSet::default", deserialize_with = "deserialize_categories")]
    pub categories: HashSet<String>,
    #[serde(default = "bool::default")]
    pub privacy_api: bool,
}

impl Config {
    fn from_file() -> Option<Self> {
        let config_file = dirs::config_dir()?.join("mpv/sponsorblock.toml");
        Some(toml::from_str(&std::fs::read_to_string(config_file).ok()?).ok()?)
    }

    pub fn get() -> Self {
        Self::from_file().unwrap_or_default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server_address: default_server_address(),
            categories: HashSet::default(),
            privacy_api: bool::default(),
        }
    }
}
