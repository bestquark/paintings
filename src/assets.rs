use anyhow::{Result, bail};
use serde_json::Value;

pub const STREAMS: [&str; 8] = [
    "renaissance",
    "baroque",
    "romanticism",
    "impressionism",
    "abstraction",
    "visionary",
    "landscapes",
    "classical",
];
pub const DEFAULTS: &str = include_str!("../config/default.json");
pub const WEB_JS: &[u8] = include_bytes!("../web/paintings.js");
pub const WEB_CSS: &[u8] = include_bytes!("../web/paintings.css");
pub const WEB_HTML: &[u8] = include_bytes!("../web/index.html");
pub const THEME: &[u8] = include_bytes!("../adapters/opencode-theme.json");
pub fn catalog_bytes(stream: &str) -> Result<&'static [u8]> {
    Ok(match stream {
        "classical" => include_bytes!("../catalog/classical.json"),
        "landscapes" => include_bytes!("../catalog/landscapes.json"),
        "renaissance" => include_bytes!("../catalog/renaissance.json"),
        "baroque" => include_bytes!("../catalog/baroque.json"),
        "romanticism" => include_bytes!("../catalog/romanticism.json"),
        "impressionism" => include_bytes!("../catalog/impressionism.json"),
        "abstraction" => include_bytes!("../catalog/abstraction.json"),
        "visionary" => include_bytes!("../catalog/visionary.json"),
        _ => bail!("Unknown stream: {stream}"),
    })
}
pub fn catalog(stream: &str) -> Result<Vec<Value>> {
    let value: Value = serde_json::from_slice(catalog_bytes(stream)?)?;
    Ok(value["artworks"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Missing artworks"))?
        .clone())
}
