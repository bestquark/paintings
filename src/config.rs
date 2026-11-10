use crate::{
    Paths,
    assets::{DEFAULTS, STREAMS},
    storage::*,
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

pub const VISUAL_KEYS: [&str; 6] = [
    "interval_seconds",
    "streams",
    "seed",
    "fit",
    "terminal_opacity",
    "web_opacity",
];
pub fn validate(cfg: &Value) -> Result<()> {
    if !cfg["interval_seconds"]
        .as_u64()
        .is_some_and(|v| (30..=86400).contains(&v))
    {
        bail!("interval_seconds must be 30–86400");
    }
    let streams = cfg["streams"]
        .as_array()
        .context("streams must be an array")?;
    if streams.is_empty()
        || streams
            .iter()
            .any(|s| !s.as_str().is_some_and(|v| STREAMS.contains(&v)))
    {
        bail!("Unknown or empty stream selection");
    }
    if !matches!(cfg["fit"].as_str(), Some("cover" | "contain")) {
        bail!("fit must be cover or contain");
    }
    if cfg["seed"].as_str().is_none() {
        bail!("seed must be a string");
    }
    for key in ["terminal_opacity", "web_opacity"] {
        if !cfg[key].as_f64().is_some_and(|v| (0.0..=1.0).contains(&v)) {
            bail!("{key} must be between 0 and 1");
        }
    }
    if !cfg["cache_limit_mb"]
        .as_u64()
        .is_some_and(|n| n > 0 && n <= 1_048_576)
    {
        bail!("Invalid cache_limit_mb");
    }
    let adapters = cfg["adapters"]
        .as_array()
        .context("adapters must be an array")?;
    if adapters.iter().any(|v| {
        !v.as_str()
            .is_some_and(|s| crate::adapters::NAMES.contains(&s))
    }) {
        bail!("Unknown adapter");
    }
    Ok(())
}
pub fn load(paths: &Paths) -> Result<Value> {
    let mut cfg: Value = serde_json::from_str(DEFAULTS)?;
    let local = paths.config.join("config.json");
    if local.exists() {
        cfg.as_object_mut().unwrap().extend(
            read_json(&local)?
                .as_object()
                .context("Config must be an object")?
                .clone(),
        );
    }
    validate(&cfg)?;
    Ok(cfg)
}
pub fn sync(paths: &Paths, mut cfg: Value, refresh: bool) -> Result<Value> {
    let cached = paths.state.join("shared.json");
    if let Some(url) = cfg["config_url"].as_str().filter(|s| !s.is_empty()) {
        let stale = std::fs::metadata(&cached)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_none_or(|age| age.as_secs() > 3600);
        if refresh && stale {
            let get = (|| -> Result<()> {
                let data: Value = serde_json::from_slice(&fetch(url, 1024 * 1024, 8)?)?;
                let mut merged = cfg.clone();
                for k in VISUAL_KEYS {
                    if let Some(v) = data.get(k) {
                        merged[k] = v.clone();
                    }
                }
                validate(&merged)?;
                write_json(&cached, &data)
            })();
            if let Err(e) = get {
                eprintln!("Shared config: using cached preferences: {e}");
            }
        }
        if cached.exists() {
            let shared = read_json(&cached)?;
            for k in VISUAL_KEYS {
                if let Some(v) = shared.get(k) {
                    cfg[k] = v.clone();
                }
            }
        }
    }
    let overrides = cfg.get("overrides").cloned().unwrap_or_else(|| json!({}));
    let overrides = overrides
        .as_object()
        .context("overrides must be an object")?;
    for k in VISUAL_KEYS {
        if let Some(v) = overrides.get(k) {
            cfg[k] = v.clone();
        }
    }
    validate(&cfg)?;
    Ok(cfg)
}
pub fn visual(cfg: &Value) -> Value {
    let mut d = json!({});
    for k in VISUAL_KEYS {
        d[k] = cfg[k].clone();
    }
    d
}
// Preserve the previous public preference revision, so an upgrade doesn't erase browser choices.
fn spaced_json(v: &Value) -> String {
    match v {
        Value::Object(m) => format!(
            "{{{}}}",
            m.iter()
                .map(|(k, v)| format!("{}: {}", spaced_json(&json!(k)), spaced_json(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Array(a) => format!(
            "[{}]",
            a.iter().map(spaced_json).collect::<Vec<_>>().join(", ")
        ),
        _ => serde_json::to_string(v)
            .unwrap()
            .chars()
            .map(|c| {
                if c.is_ascii() {
                    c.to_string()
                } else {
                    let mut b = [0; 2];
                    c.encode_utf16(&mut b)
                        .iter()
                        .map(|n| format!("\\u{n:04x}"))
                        .collect()
                }
            })
            .collect(),
    }
}
pub fn web_config(cfg: &Value) -> Value {
    let mut d = visual(cfg);
    d["available_streams"] = json!(STREAMS);
    d["preference_revision"] = json!(format!("2-{}", &hash(spaced_json(&d))[..12]));
    d
}
pub fn sync_web(paths: &Paths, cfg: &Value) -> Result<Vec<String>> {
    let registry = paths.state.join("web-roots.json");
    if !registry.exists() {
        return Ok(vec![]);
    }
    let roots: Vec<String> = serde_json::from_value(read_json(&registry)?)?;
    for root in &roots {
        let target = std::path::Path::new(root).join("paintings/config/default.json");
        if !target.exists() {
            bail!("Web integration missing; run install-web after frontend updates");
        }
        write_json(&target, &web_config(cfg))?;
    }
    Ok(roots)
}
pub fn status(paths: &Paths) -> Result<Value> {
    let cfg = sync(paths, load(paths)?, false)?;
    Ok(
        json!({"settings":visual(&cfg),"overrides":cfg.get("overrides").cloned().unwrap_or_else(||json!({})),"available_streams":STREAMS,"current":crate::rotation::current(paths)?}),
    )
}
pub fn configure(
    paths: &Paths,
    stream: Option<&str>,
    fit: Option<&str>,
    apply: bool,
) -> Result<Value> {
    {
        let _lock = lock(&paths.config.join("config.lock"), true)?;
        let mut cfg = load(paths)?;
        if cfg.get("overrides").is_none() {
            cfg["overrides"] = json!({});
        }
        let overrides = cfg["overrides"]
            .as_object_mut()
            .context("Invalid overrides")?;
        match stream {
            Some("default") => {
                overrides.remove("streams");
            }
            Some("all") => {
                overrides.insert("streams".into(), json!(STREAMS));
            }
            Some(s) if STREAMS.contains(&s) => {
                overrides.insert("streams".into(), json!([s]));
            }
            Some(_) => bail!("Unknown stream"),
            None => {}
        }
        match fit {
            Some("default") => {
                overrides.remove("fit");
            }
            Some(f @ ("cover" | "contain")) => {
                overrides.insert("fit".into(), json!(f));
            }
            Some(_) => bail!("Unknown sizing"),
            None => {}
        }
        sync(paths, cfg.clone(), false)?;
        backup(paths, &paths.config.join("config.json"))?;
        write_json(&paths.config.join("config.json"), &cfg)?;
    }
    let mut result = json!({"settings_saved":true,"applied":false});
    if apply {
        match crate::rotation::tick(paths, None, None, false) {
            Ok(v) if v.get("skipped").is_some() => {
                result["error"] =
                    json!("Rotation is busy; saved settings apply at the next scheduled rotation.")
            }
            Ok(v) => {
                result["applied"] = json!(
                    v["adapters"]
                        .as_object()
                        .context("Missing results")?
                        .values()
                        .all(|v| v["ok"] == true)
                );
                if result["applied"] != true {
                    result["error"] =
                        json!("Settings saved; one or more adapters could not apply the painting.");
                }
            }
            Err(e) => result["error"] = json!(e.to_string()),
        }
    }
    result
        .as_object_mut()
        .unwrap()
        .extend(status(paths)?.as_object().unwrap().clone());
    Ok(result)
}
