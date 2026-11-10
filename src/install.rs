use crate::{
    Paths,
    adapters::{cmd, uid},
    assets, config,
    storage::*,
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub fn install(
    paths: &Paths,
    adapters: &str,
    stream: Option<&str>,
    config_url: Option<&str>,
    schedule: bool,
) -> Result<Value> {
    if schedule && !cfg!(any(target_os = "macos", target_os = "linux")) {
        bail!(
            "Automatic scheduling supports macOS and Linux; use --no-schedule and your own scheduler on this platform"
        );
    }
    let mut cfg = config::load(paths)?;
    cfg["adapters"] = json!(
        adapters
            .split(',')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
    );
    if let Some(url) = config_url {
        if url::Url::parse(url)?.scheme() != "https" {
            bail!("Shared config requires HTTPS");
        }
        cfg["config_url"] = json!(url);
    }
    if let Some(s) = stream {
        if !assets::STREAMS.contains(&s) {
            bail!("Unknown stream");
        }
        if cfg.get("overrides").is_none() {
            cfg["overrides"] = json!({});
        }
        cfg["overrides"]["streams"] = json!([s]);
    }
    config::sync(paths, cfg.clone(), false)?;
    config::validate(&cfg)?;
    backup(paths, &paths.config.join("config.json"))?;
    write_json(&paths.config.join("config.json"), &cfg)?;
    let launcher = paths.home.join(if cfg!(windows) {
        ".local/bin/paintings.exe"
    } else {
        ".local/bin/paintings"
    });
    let executable = std::env::current_exe()?.canonicalize()?;
    if launcher.canonicalize().ok().as_ref() != Some(&executable) {
        backup(paths, &launcher)?;
        atomic(&launcher, &fs::read(executable)?)?;
        mode(&launcher, 0o755)?;
    }
    let logs = paths.state.join("logs");
    fs::create_dir_all(&logs)?;
    if schedule && cfg!(target_os = "macos") {
        let target = paths
            .home
            .join("Library/LaunchAgents/ca.bestquark.paintings.plist");
        backup(paths, &target)?;
        let data = json!({"Label":"ca.bestquark.paintings","ProgramArguments":[launcher,"tick"],"RunAtLoad":true,"StartInterval":cfg["interval_seconds"],"ProcessType":"Background","StandardOutPath":logs.join("rotation.log"),"StandardErrorPath":logs.join("errors.log"),"EnvironmentVariables":{"PATH":"/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin"}});
        let mut bytes = vec![];
        plist::to_writer_xml(&mut bytes, &data)?;
        atomic(&target, &bytes)?;
        let domain = format!("gui/{}", uid());
        let _ = cmd(&[
            "launchctl",
            "bootout",
            &format!("{domain}/ca.bestquark.paintings"),
        ]);
        cmd(&["launchctl", "bootstrap", &domain, &target.to_string_lossy()])?;
    } else if schedule && cfg!(target_os = "linux") {
        let folder = paths.home.join(".config/systemd/user");
        let service = folder.join("paintings.service");
        let timer = folder.join("paintings.timer");
        backup(paths, &service)?;
        backup(paths, &timer)?;
        let quoted = launcher
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('%', "%%")
            .replace('$', "$$");
        atomic(&service,format!("[Unit]\nDescription=Paintings wallpaper rotation\n[Service]\nType=oneshot\nExecStart=\"{quoted}\" tick\nEnvironment=PATH={}/.local/bin:/usr/local/bin:/usr/bin:/bin\n",paths.home.display()).as_bytes())?;
        atomic(&timer,format!("[Unit]\nDescription=Rotate paintings\n[Timer]\nOnBootSec=20\nOnUnitActiveSec={}s\nAccuracySec=1s\n[Install]\nWantedBy=timers.target\n",cfg["interval_seconds"]).as_bytes())?;
        cmd(&["systemctl", "--user", "daemon-reload"])?;
        cmd(&["systemctl", "--user", "enable", "--now", "paintings.timer"])?;
    }
    Ok(
        json!({"installed":launcher,"config":paths.config.join("config.json"),"interval_seconds":cfg["interval_seconds"],"adapters":cfg["adapters"],"scheduled":schedule}),
    )
}
pub fn build_gallery(target: &Path) -> Result<()> {
    fs::create_dir_all(target)?;
    for stream in assets::STREAMS {
        atomic(
            &target.join("catalog").join(format!("{stream}.json")),
            assets::catalog_bytes(stream)?,
        )?;
    }
    atomic(
        &target.join("catalog/manifest.json"),
        include_bytes!("../catalog/manifest.json"),
    )?;
    atomic(
        &target.join("config/default.json"),
        assets::DEFAULTS.as_bytes(),
    )?;
    atomic(&target.join("paintings.js"), assets::WEB_JS)?;
    atomic(&target.join("paintings.css"), assets::WEB_CSS)?;
    atomic(&target.join("index.html"), assets::WEB_HTML)?;
    Ok(())
}
pub fn install_web(paths: &Paths, root: &Path) -> Result<Value> {
    fs::create_dir_all(root)?;
    let root = root.canonicalize()?;
    let dest = root.join("paintings");
    build_gallery(&dest)?;
    // Only visual preferences belong in a served directory.
    let cfg = config::sync(paths, config::load(paths)?, false)?;
    write_json(&dest.join("config/default.json"), &config::web_config(&cfg))?;
    let index = root.join("index.html");
    if index.exists() {
        backup(paths, &index)?;
        let s = fs::read_to_string(&index)?;
        let s =
            regex::Regex::new(r"<script[^>]*data-paintings[^>]*></script>")?.replace_all(&s, "");
        if !s.contains("</head>") {
            bail!("Web entry has no </head>; no injection performed");
        }
        let digest = hash([assets::WEB_JS, assets::WEB_CSS].concat());
        let tag = format!(
            "<script defer src=\"/paintings/paintings.js?v={}\" data-paintings data-openchamber data-position=\"top\"></script></head>",
            &digest[..12]
        );
        atomic(&index, s.replacen("</head>", &tag, 1).as_bytes())?;
    }
    let registry = paths.state.join("web-roots.json");
    let mut roots: Vec<String> = if registry.exists() {
        serde_json::from_value(read_json(&registry)?)?
    } else {
        vec![]
    };
    let name = root.to_string_lossy().to_string();
    if !roots.contains(&name) {
        roots.push(name);
    }
    write_json(&registry, &json!(roots))?;
    Ok(
        json!({"installed":dest,"entry":index,"reload":"Refresh the browser; no backend restart required."}),
    )
}
pub fn uninstall(paths: &Paths) -> Result<Value> {
    if cfg!(target_os = "macos")
        && paths
            .home
            .join("Library/LaunchAgents/ca.bestquark.paintings.plist")
            .exists()
    {
        let _ = cmd(&[
            "launchctl",
            "bootout",
            &format!("gui/{}/ca.bestquark.paintings", uid()),
        ]);
    }
    if cfg!(target_os = "linux")
        && paths
            .home
            .join(".config/systemd/user/paintings.timer")
            .exists()
    {
        let _ = cmd(&["systemctl", "--user", "disable", "--now", "paintings.timer"]);
    }
    let iterm = paths.state.join("iterm-default-before.json");
    if cfg!(target_os = "macos") && iterm.exists() {
        let old = read_json(&iterm)?;
        if old["present"] == true {
            let _ = cmd(&[
                "defaults",
                "write",
                "com.googlecode.iterm2",
                "DefaultBookmarkGuid",
                "-string",
                old["value"].as_str().unwrap_or_default(),
            ]);
        } else {
            let _ = cmd(&[
                "defaults",
                "delete",
                "com.googlecode.iterm2",
                "DefaultBookmarkGuid",
            ]);
        }
    }
    let env = paths.state.join("openchamber-env-before.json");
    if cfg!(target_os = "macos") && env.exists() {
        let old = read_json(&env)?;
        if old["present"] == true {
            cmd(&[
                "launchctl",
                "setenv",
                "OPENCHAMBER_ELECTRON_LOAD_SERVER_UI",
                old["value"].as_str().unwrap_or_default(),
            ])?;
        } else {
            cmd(&[
                "launchctl",
                "unsetenv",
                "OPENCHAMBER_ELECTRON_LOAD_SERVER_UI",
            ])?;
        }
    }
    let mut restored = vec![];
    let backups = paths.state.join("backups");
    let plugin_tag = regex::Regex::new(r"<script[^>]*data-paintings[^>]*></script>")?;
    if backups.exists() {
        for entry in fs::read_dir(backups)? {
            let meta = entry?.path();
            if !meta.extension().is_some_and(|s| s == "json") {
                continue;
            }
            let d = read_json(&meta)?;
            let target = Path::new(d["path"].as_str().context("Invalid backup metadata")?);
            let data = meta.with_extension("");
            let bytes = fs::read(target).ok();
            let text = bytes.as_deref().and_then(|b| std::str::from_utf8(b).ok());
            let filename = target
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if filename == "index.html"
                && let Some(text) = text
            {
                atomic(target, plugin_tag.replace_all(text, "").as_bytes())?;
            } else if filename == "openchamber.desktop"
                && let Some(text) = text
            {
                atomic(
                    target,
                    text.replacen(
                        "Exec=env OPENCHAMBER_ELECTRON_LOAD_SERVER_UI=1 ",
                        "Exec=",
                        1,
                    )
                    .as_bytes(),
                )?;
            } else if text.is_some_and(|s| s.contains("# paintings:start")) {
                atomic(target, strip_block(text.unwrap()).as_bytes())?;
            } else if filename == "cli.json"
                && target
                    .parent()
                    .and_then(|p| p.file_name())
                    .is_some_and(|s| s == "opencode")
                && target.exists()
            {
                let mut v = read_json(target)?;
                if v["theme"]["name"] == "paintings" {
                    let original = if d["existed"] == true {
                        read_json(&data)?
                    } else {
                        json!({})
                    };
                    if let Some(theme) = original.get("theme") {
                        v["theme"] = theme.clone();
                    } else {
                        v.as_object_mut().context("Invalid config")?.remove("theme");
                    }
                    write_json(target, &v)?;
                }
            } else if d["existed"] == true {
                atomic(target, &fs::read(data)?)?;
                if filename == "paintings" {
                    mode(target, 0o755)?;
                }
            } else if target.exists() {
                fs::remove_file(target)?;
            }
            restored.push(target.to_string_lossy().to_string());
        }
    }
    Ok(
        json!({"stopped":true,"restored_configs":restored,"note":"Artwork cache and provenance retained; choose a desktop picture in system settings."}),
    )
}
