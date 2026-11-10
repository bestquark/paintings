use anyhow::{Context, Result, bail};
use bestquark_paintings::{Paths, assets, rotation, storage::*};
use clap::{Parser, Subcommand};
use image::{
    DynamicImage, RgbImage,
    codecs::jpeg::JpegEncoder,
    imageops::{self, FilterType},
};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(about = "Catalog verification and previews, using existing artwork only")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}
#[derive(Subcommand)]
enum Commands {
    Verify {
        streams: Vec<String>,
    },
    Finalize {
        streams: Vec<String>,
        #[arg(long)]
        write: bool,
        #[arg(long)]
        sheets: bool,
        #[arg(long, default_value_t = 200)]
        target: usize,
    },
    Previews {
        #[arg(long, default_value = "tools/previews.json")]
        selection: PathBuf,
        #[arg(long, default_value = "docs/previews")]
        output: PathBuf,
    },
}
fn local_catalog(stream: &str) -> Result<Value> {
    if !assets::STREAMS.contains(&stream) {
        bail!("Unknown stream: {stream}");
    }
    read_json(&Path::new("catalog").join(format!("{stream}.json")))
}
fn streams(s: Vec<String>) -> Vec<String> {
    if s.is_empty() {
        assets::STREAMS.iter().map(|s| s.to_string()).collect()
    } else {
        s
    }
}
fn decode(path: &Path) -> Result<DynamicImage> {
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(512 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader.decode()?)
}
fn phash(im: &DynamicImage) -> u64 {
    let gray = im
        .grayscale()
        .resize_exact(32, 32, FilterType::CatmullRom)
        .to_luma8();
    let mut coefficients = vec![];
    for u in 0..8 {
        for v in 0..8 {
            if u == 0 && v == 0 {
                continue;
            }
            let mut sum = 0.0;
            for y in 0..32 {
                for x in 0..32 {
                    sum += f64::from(gray.get_pixel(x, y)[0])
                        * (std::f64::consts::PI * f64::from(2 * y + 1) * f64::from(u) / 64.0).cos()
                        * (std::f64::consts::PI * f64::from(2 * x + 1) * f64::from(v) / 64.0).cos();
                }
            }
            coefficients.push(sum);
        }
    }
    let mut sorted = coefficients.clone();
    sorted.sort_by(f64::total_cmp);
    let median = sorted[31];
    coefficients
        .into_iter()
        .fold(0, |bits, v| (bits << 1) | u64::from(v > median))
}
fn verify(paths: &Paths, art: &Value) -> Result<Value> {
    if !rotation::image_path(paths, art)?.exists() {
        // Be considerate of source archives when verifying a whole catalog.
        std::thread::sleep(std::time::Duration::from_millis(1500));
    }
    let image = rotation::download(paths, art)?;
    let im = decode(&image)?;
    let (w, h) = (im.width(), im.height());
    if w < 1100 || h < 600 {
        bail!("Image too small: {w}x{h}");
    }
    Ok(
        json!({"id":art["id"],"ok":true,"width":w,"height":h,"path":image,"sha256":hash(fs::read(&image)?),"phash":format!("0x{:x}",phash(&im))}),
    )
}
fn title(art: &Value) -> String {
    let s = art["title"].as_str().unwrap_or_default();
    let english = regex::Regex::new(r#"label QS:Len,"([^"]+)""#).unwrap();
    let s = english
        .captures(s)
        .and_then(|m| m.get(1))
        .map(|m| m.as_str())
        .unwrap_or(s);
    let metadata = regex::Regex::new(r"\s+(?:label QS:|title QS:)").unwrap();
    let s = metadata.split(s).next().unwrap_or(s);
    let language =
        regex::Regex::new(r"^(?:English|German|Italian|French|Dutch|Russian|Polish|Spanish):\s*")
            .unwrap();
    let s = language.replace(s, "");
    s.trim_matches([' ', '.'])
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(220)
        .collect()
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn finalize(stream: &str, target: usize, write: bool, sheets: bool) -> Result<()> {
    let mut data = local_catalog(stream)?;
    let mut kept: Vec<Value> = vec![];
    let mut seen: Vec<(u64, String)> = vec![];
    let mut dropped = vec![];
    let mut exclusions = if Path::new("tools/exclusions.json").exists() {
        read_json(Path::new("tools/exclusions.json"))?
    } else {
        json!({})
    };
    if Path::new("tools/stream-exclusions.json").exists() {
        let scoped = read_json(Path::new("tools/stream-exclusions.json"))?;
        if let Some(entries) = scoped[stream].as_object() {
            exclusions.as_object_mut().unwrap().extend(entries.clone());
        }
    }
    let detail = regex::Regex::new(
        r"(?i)\b(detail|dettaglio|detalhe|recto and verso|signature|firma|reverse side|postcard|engravings|sketchbook)\b",
    )?;
    let key = |s: &str| {
        s.to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
    };
    for mut a in data["artworks"]
        .as_array()
        .context("Missing artworks")?
        .clone()
    {
        let id = a["id"].as_str().context("Missing ID")?.to_string();
        let path = Path::new(".work/image-checks").join(format!("{id}.json"));
        let reason = if let Some(reason) = exclusions.get(&id) {
            Some(reason.to_string())
        } else if !path.exists() {
            Some("unchecked".into())
        } else {
            None
        };
        if let Some(reason) = reason {
            dropped.push(json!([id, reason]));
            continue;
        }
        let mut check = read_json(&path)?;
        if check["ok"] != true {
            dropped.push(json!([id, check["error"]]));
            continue;
        }
        a["title"] = json!(title(&a));
        if detail.is_match(a["title"].as_str().unwrap()) {
            dropped.push(json!([id, "detail"]));
            continue;
        }
        if check.get("phash").is_none() {
            check["phash"] = json!(format!(
                "0x{:x}",
                phash(&decode(Path::new(
                    check["path"].as_str().context("Missing image path")?
                ))?)
            ));
            write_json(&path, &check)?;
        }
        let h = u64::from_str_radix(
            check["phash"]
                .as_str()
                .context("Invalid pHash")?
                .trim_start_matches("0x"),
            16,
        )?;
        if let Some((_, old)) = seen.iter().find(|(oh, _)| (oh ^ h).count_ones() <= 6) {
            dropped.push(json!([id, format!("duplicate of {old}")]));
            continue;
        }
        let titlekey = key(a["title"].as_str().unwrap());
        if titlekey.len() > 15
            && kept.iter().any(|b| {
                key(b["title"].as_str().unwrap_or_default()) == titlekey
                    && b["artist"] == a["artist"]
                    && b["date"] == a["date"]
            })
        {
            dropped.push(json!([id, "same title"]));
            continue;
        }
        seen.push((h, id));
        a["display_width"] = check["width"].clone();
        a["display_height"] = check["height"].clone();
        a["image_sha256"] = check["sha256"].clone();
        kept.push(a);
    }
    write_json(
        &Path::new(".work").join(format!("{stream}-verified.json")),
        &json!(kept),
    )?;
    write_json(
        &Path::new(".work").join(format!("{stream}-excluded.json")),
        &json!(dropped),
    )?;
    println!(
        "{stream}: {} verified, {} excluded",
        kept.len(),
        dropped.len()
    );
    if sheets {
        let mut html = String::from(
            "<!doctype html><meta charset=\"utf-8\"><style>body{background:#242420;color:white;font:14px system-ui}main{display:grid;grid-template-columns:repeat(8,1fr);gap:12px}figure{margin:0}img{width:100%;height:160px;object-fit:contain}figcaption{overflow-wrap:anywhere}</style><main>",
        );
        for (i, a) in kept.iter().enumerate() {
            let c = read_json(
                &Path::new(".work/image-checks")
                    .join(format!("{}.json", a["id"].as_str().unwrap())),
            )?;
            let url = url::Url::from_file_path(c["path"].as_str().context("Missing image")?)
                .map_err(|_| anyhow::anyhow!("Invalid image path"))?;
            html += &format!(
                "<figure><img src=\"{}\"><figcaption>{}: {}<br>{}</figcaption></figure>",
                escape(url.as_str()),
                i,
                escape(a["id"].as_str().unwrap()),
                escape(a["title"].as_str().unwrap())
            );
        }
        html += "</main>";
        atomic(
            &Path::new(".work/contact-sheets").join(format!("{stream}.html")),
            html.as_bytes(),
        )?;
    }
    if write {
        if kept.len() < target {
            bail!("Fewer than {target} verified works in {stream}");
        }
        data["artworks"] = json!(&kept[..target]);
        write_json(&Path::new("catalog").join(format!("{stream}.json")), &data)?;
    }
    Ok(())
}
fn markdown(s: &str) -> String {
    s.replace('[', "\\[").replace(']', "\\]").replace('\n', " ")
}
fn previews(paths: &Paths, selection: &Path, output: &Path) -> Result<()> {
    let selections = read_json(selection)?;
    fs::create_dir_all(output)?;
    let mut credits = String::from(
        "# Preview credits\n\nTen existing artworks per stream, in left-to-right order. Thumbnails are resized and cropped; originals retain their individual terms. No generated images.\n",
    );
    let mut records = json!({});
    for stream in [
        "classical",
        "landscapes",
        "renaissance",
        "baroque",
        "romanticism",
        "impressionism",
        "abstraction",
        "visionary",
    ] {
        let ids = selections[stream]
            .as_array()
            .context("Missing preview stream")?;
        if ids.len() != 10 {
            bail!("{stream} needs exactly ten previews");
        }
        let catalog = local_catalog(stream)?;
        let mut strip = RgbImage::from_pixel(2596, 192, image::Rgb([23, 23, 21]));
        let mut selected = vec![];
        credits += &format!("\n## {stream}\n\n");
        let mut unique = HashSet::new();
        for (i, id) in ids.iter().enumerate() {
            if !unique.insert(id.to_string()) {
                bail!("Duplicate preview ID");
            }
            let a = catalog["artworks"]
                .as_array()
                .unwrap()
                .iter()
                .find(|a| a["id"] == *id)
                .context("Preview not in its catalog")?;
            let path = rotation::download(paths, a)?;
            let im = decode(&path)?
                .resize_to_fill(256, 192, FilterType::Lanczos3)
                .to_rgb8();
            imageops::replace(&mut strip, &im, (i * 260) as i64, 0);
            credits += &format!(
                "{}. [{}]({}) — {}. [{}]({}). Credit: {}.\n",
                i + 1,
                markdown(a["title"].as_str().unwrap()),
                a["source_url"].as_str().unwrap(),
                markdown(a["artist"].as_str().unwrap()),
                a["license"].as_str().unwrap(),
                a["license_url"].as_str().unwrap(),
                markdown(a["credit"].as_str().unwrap())
            );
            let mut record = json!({});
            for k in [
                "id",
                "title",
                "artist",
                "date",
                "source_url",
                "image_url",
                "license",
                "license_url",
                "credit",
            ] {
                record[k] = a[k].clone();
            }
            selected.push(record);
        }
        let mut bytes = vec![];
        JpegEncoder::new_with_quality(&mut bytes, 88).encode_image(&strip)?;
        atomic(&output.join(format!("{stream}.jpg")), &bytes)?;
        records[stream] = json!(selected);
        println!("{stream}: 10 previews, {} KiB", bytes.len() / 1024);
    }
    atomic(&output.join("README.md"), credits.as_bytes())?;
    write_json(&output.join("credits.json"), &records)?;
    Ok(())
}
fn execute() -> Result<()> {
    let paths = Paths::discover()?;
    match Cli::parse().command {
        Commands::Verify { streams: s } => {
            let mut seen = HashSet::new();
            let mut bad = 0;
            for s in streams(s) {
                for art in local_catalog(&s)?["artworks"]
                    .as_array()
                    .context("Missing artworks")?
                {
                    let id = art["id"].as_str().context("Missing ID")?;
                    if !seen.insert(id.to_string()) {
                        continue;
                    }
                    let result = match verify(&paths, art) {
                        Ok(v) => v,
                        Err(e) => {
                            bad += 1;
                            json!({"id":id,"ok":false,"error":e.to_string()})
                        }
                    };
                    write_json(
                        &Path::new(".work/image-checks").join(format!("{id}.json")),
                        &result,
                    )?;
                    println!(
                        "{id}: {}",
                        if result["ok"] == true { "ok" } else { "failed" }
                    );
                }
            }
            if bad > 0 {
                bail!("{bad} image checks failed");
            }
        }
        Commands::Finalize {
            streams: s,
            target,
            write,
            sheets,
        } => {
            for s in streams(s) {
                finalize(&s, target, write, sheets)?;
            }
        }
        Commands::Previews { selection, output } => previews(&paths, &selection, &output)?,
    }
    Ok(())
}
fn main() {
    if let Err(e) = execute() {
        eprintln!("catalog: {e:#}");
        std::process::exit(1);
    }
}
