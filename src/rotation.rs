use crate::{
    Paths,
    assets::{self, STREAMS},
    config,
    storage::*,
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{collections::HashSet, fs, path::PathBuf};

pub fn current(paths: &Paths) -> Result<Value> {
    let p = paths.state.join("current.json");
    if p.exists() {
        read_json(&p)
    } else {
        Ok(Value::Null)
    }
}
pub fn selection(cfg: &Value, now: f64, offset: u64) -> Result<(String, Value)> {
    let streams = cfg["streams"].as_array().context("Missing streams")?;
    let slot = (now
        / cfg["interval_seconds"]
            .as_f64()
            .context("Missing interval")?)
    .floor() as u64
        + offset;
    let stream = streams[slot as usize % streams.len()]
        .as_str()
        .context("Invalid stream")?;
    let mut arts = assets::catalog(stream)?;
    arts.sort_by_cached_key(|a| {
        hash(format!(
            "{}{}{}",
            cfg["seed"].as_str().unwrap_or_default(),
            stream,
            a["id"].as_str().unwrap_or_default()
        ))
    });
    if arts.is_empty() {
        bail!("Empty stream: {stream}");
    }
    let art = arts[(slot as usize / streams.len()) % arts.len()].clone();
    Ok((stream.into(), art))
}
pub fn image_path(paths: &Paths, art: &Value) -> Result<PathBuf> {
    let id = art["id"].as_str().context("Missing artwork ID")?;
    if id.is_empty()
        || !id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        bail!("Unsafe artwork ID");
    }
    let url = url::Url::parse(art["image_url"].as_str().context("Missing image URL")?)?;
    if url.scheme() != "https" {
        bail!("Image URLs must use HTTPS");
    }
    let ext = std::path::Path::new(url.path())
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("jpg")
        .to_lowercase();
    let ext = if matches!(ext.as_str(), "jpg" | "jpeg" | "png") {
        ext.as_str()
    } else {
        "jpg"
    };
    Ok(paths.cache.join("images").join(format!("{id}.{ext}")))
}
pub fn download(paths: &Paths, art: &Value) -> Result<PathBuf> {
    let path = image_path(paths, art)?;
    if fs::metadata(&path).is_ok_and(|m| m.len() > 1000) {
        return Ok(path);
    }
    fs::create_dir_all(&paths.cache)?;
    if fs2::available_space(&paths.cache)? < 256 * 1024 * 1024 {
        bail!("Less than 256 MiB free; keeping the cached painting");
    }
    let data = fetch(art["image_url"].as_str().unwrap(), 60 * 1024 * 1024, 35)?;
    if !(data.starts_with(b"\xff\xd8\xff") || data.starts_with(b"\x89PNG\r\n\x1a\n")) {
        bail!("Source did not return JPEG or PNG");
    }
    atomic(&path, &data)?;
    Ok(path)
}
fn trim_cache(paths: &Paths, cfg: &Value, keep: &std::path::Path) -> Result<()> {
    let dir = paths.cache.join("images");
    if !dir.exists() {
        return Ok(());
    }
    let mut files = fs::read_dir(dir)?
        .filter_map(|r| r.ok())
        .filter_map(|e| {
            e.metadata()
                .ok()
                .filter(|m| m.is_file())
                .map(|m| (e.path(), m))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|(_, m)| m.modified().ok());
    let mut total: u64 = files.iter().map(|(_, m)| m.len()).sum();
    let limit = cfg["cache_limit_mb"].as_u64().unwrap_or(4096) * 1024 * 1024;
    for (p, m) in files {
        if total <= limit {
            break;
        }
        if p == keep {
            continue;
        }
        fs::remove_file(p)?;
        total -= m.len();
    }
    Ok(())
}
pub fn tick(paths: &Paths, stream: Option<&str>, id: Option<&str>, next: bool) -> Result<Value> {
    tick_with_downloader(paths, stream, id, next, download)
}
#[doc(hidden)]
pub fn tick_with_downloader(
    paths: &Paths,
    stream: Option<&str>,
    id: Option<&str>,
    next: bool,
    downloader: impl Fn(&Paths, &Value) -> Result<PathBuf>,
) -> Result<Value> {
    let mut cfg = config::sync(paths, config::load(paths)?, true)?;
    let Some(_guard) = lock(&paths.state.join("rotation.lock"), false)? else {
        return Ok(json!({"skipped":"rotation already running"}));
    };
    if let Some(s) = stream {
        if !STREAMS.contains(&s) {
            bail!("Unknown stream");
        }
        cfg["streams"] = json!([s]);
    }
    let offset = if next {
        current(paths)?["manual_offset"].as_u64().unwrap_or(0) + 1
    } else {
        0
    };
    let (stream, art, image) = if let Some(id) = id {
        let mut streams = cfg["streams"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_owned())
            .collect::<Vec<_>>();
        streams.extend(STREAMS.iter().map(|s| s.to_string()));
        let mut seen = HashSet::new();
        let mut found = None;
        for s in streams {
            if seen.insert(s.clone())
                && let Some(a) = assets::catalog(&s)?.into_iter().find(|a| a["id"] == id)
            {
                let p = downloader(paths, &a)?;
                found = Some((s, a, p));
                break;
            }
        }
        found.context("Unknown artwork ID")?
    } else {
        let mut failures = vec![];
        let mut found = None;
        for attempt in 0..8 {
            let (s, a) = selection(&cfg, now(), offset + attempt)?;
            match downloader(paths, &a) {
                Ok(p) => {
                    found = Some((s, a, p));
                    break;
                }
                Err(e) => failures.push(json!({"id":a["id"],"error":e.to_string()})),
            }
        }
        if found.is_none() {
            write_json(
                &paths.state.join("last-error.json"),
                &json!({"time":now(),"failures":failures}),
            )?;
            bail!("Sources unavailable; keeping the previous painting");
        }
        found.unwrap()
    };
    let mut results = json!({});
    for name in cfg["adapters"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        results[name] = match crate::adapters::apply(paths, name, &image, &cfg) {
            Ok(detail) => json!({"ok":true,"detail":detail}),
            Err(e) => json!({"ok":false,"error":e.to_string()}),
        };
    }
    if paths.state.join("web-roots.json").exists() {
        results["web"] = match config::sync_web(paths, &cfg) {
            Ok(roots) => json!({"ok":true,"detail":{"updated":roots}}),
            Err(e) => json!({"ok":false,"error":e.to_string()}),
        };
    }
    let state = json!({"time":now(),"stream":stream,"artwork":art,"path":image,"interval_seconds":cfg["interval_seconds"],"fit":cfg["fit"],"manual_offset":if id.is_some(){0}else{offset},"adapters":results});
    write_json(&paths.state.join("current.json"), &state)?;
    if let Err(e) = trim_cache(paths, &cfg, &image) {
        eprintln!("Cache maintenance: {e}");
    }
    Ok(state)
}
