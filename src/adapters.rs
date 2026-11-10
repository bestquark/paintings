use crate::{Paths, assets, storage::*};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    env, fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
use wait_timeout::ChildExt;
pub const NAMES: [&str; 6] = [
    "desktop",
    "ghostty",
    "kitty",
    "iterm",
    "opencode",
    "openchamber",
];
pub fn run(args: &[String], vars: &BTreeMap<String, String>, seconds: u64) -> Result<String> {
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut child = Command::new(&args[0])
        .args(&args[1..])
        .envs(vars)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .spawn()
        .with_context(|| format!("Run {}", args[0]))?;
    let status = match child.wait_timeout(Duration::from_secs(seconds))? {
        Some(s) => s,
        None => {
            let _ = child.kill();
            let _ = child.wait();
            bail!("{} timed out", args[0]);
        }
    };
    let mut out = String::new();
    let mut err = String::new();
    stdout.seek(SeekFrom::Start(0))?;
    stderr.seek(SeekFrom::Start(0))?;
    stdout.take(1024 * 1024).read_to_string(&mut out)?;
    stderr.take(1024 * 1024).read_to_string(&mut err)?;
    if !status.success() {
        let message = if err.trim().is_empty() {
            out.trim()
        } else {
            err.trim()
        };
        bail!(
            "{}: {}",
            args[0],
            message.chars().take(500).collect::<String>()
        );
    }
    Ok(out.trim().to_owned())
}
pub fn cmd(args: &[&str]) -> Result<String> {
    run(
        &args.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        &BTreeMap::new(),
        25,
    )
}
pub fn which(name: &str) -> Option<PathBuf> {
    env::split_paths(&env::var_os("PATH")?)
        .map(|p| p.join(name))
        .find(|p| p.is_file())
}
pub fn uid() -> u32 {
    #[cfg(unix)]
    {
        unsafe { libc::getuid() }
    }
    #[cfg(not(unix))]
    {
        0
    }
}
fn cover(cfg: &Value) -> bool {
    cfg["fit"] == "cover"
}
fn opacity(cfg: &Value) -> f64 {
    cfg["terminal_opacity"].as_f64().unwrap_or(0.16)
}
pub fn apply(paths: &Paths, name: &str, image: &Path, cfg: &Value) -> Result<Value> {
    match name {
        "desktop" => desktop(paths, image, cfg),
        "ghostty" => ghostty(paths, image, cfg),
        "kitty" => kitty(paths, image, cfg),
        "iterm" => iterm(paths, image, cfg),
        "opencode" => opencode(paths, cfg),
        "openchamber" => openchamber(paths),
        _ => bail!("Unknown adapter: {name}"),
    }
}
fn desktop(paths: &Paths, image: &Path, cfg: &Value) -> Result<Value> {
    #[cfg(target_os = "macos")]
    {
        let binary = paths.state.join("bin/wallpaper");
        let bytes = include_bytes!(concat!(env!("OUT_DIR"), "/wallpaper"));
        if fs::read(&binary).ok().as_deref() != Some(bytes.as_slice()) {
            atomic(&binary, bytes)?;
            mode(&binary, 0o755)?;
        }
        let binary = binary.to_string_lossy();
        let before = paths.state.join("desktop-before.json");
        if !before.exists() {
            atomic(&before, cmd(&[&binary, "--inspect"])?.as_bytes())?;
        }
        serde_json::from_str(&cmd(&[
            &binary,
            &image.to_string_lossy(),
            if cover(cfg) { "cover" } else { "contain" },
        ])?)
        .map_err(Into::into)
    }
    #[cfg(target_os = "linux")]
    {
        linux_desktop(paths, image, cfg)
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "user32")]
        unsafe extern "system" {
            fn SystemParametersInfoW(action: u32, param: u32, value: *const u16, flags: u32)
            -> i32;
        }
        cmd(&[
            "reg",
            "add",
            r"HKCU\Control Panel\Desktop",
            "/v",
            "WallpaperStyle",
            "/t",
            "REG_SZ",
            "/d",
            if cover(cfg) { "10" } else { "6" },
            "/f",
        ])?;
        cmd(&[
            "reg",
            "add",
            r"HKCU\Control Panel\Desktop",
            "/v",
            "TileWallpaper",
            "/t",
            "REG_SZ",
            "/d",
            "0",
            "/f",
        ])?;
        let wide = image
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        if unsafe { SystemParametersInfoW(20, 0, wide.as_ptr(), 3) } == 0 {
            bail!("Windows wallpaper update failed");
        }
        let _ = paths;
        Ok(json!({"desktop":"Windows","image":image}))
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = (paths, image, cfg);
        bail!("Unsupported desktop platform")
    }
}
#[cfg(target_os = "linux")]
fn linux_desktop(paths: &Paths, image: &Path, cfg: &Value) -> Result<Value> {
    let mut vars = BTreeMap::new();
    let bus = format!("/run/user/{}/bus", uid());
    if Path::new(&bus).exists() && env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        vars.insert(
            "DBUS_SESSION_BUS_ADDRESS".into(),
            format!("unix:path={bus}"),
        );
    }
    let process = cmd(&["pgrep", "-u", &uid().to_string(), "-x", "pcmanfm"]).unwrap_or_default();
    if which("pcmanfm").is_some() && !process.is_empty() {
        let proc = PathBuf::from("/proc").join(process.lines().next().unwrap());
        for field in fs::read(proc.join("environ"))?.split(|b| *b == 0) {
            if let Some((k, v)) = String::from_utf8_lossy(field).split_once('=')
                && [
                    "DISPLAY",
                    "WAYLAND_DISPLAY",
                    "XAUTHORITY",
                    "XDG_RUNTIME_DIR",
                    "DBUS_SESSION_BUS_ADDRESS",
                ]
                .contains(&k)
            {
                vars.insert(k.into(), v.into());
            }
        }
        let args = String::from_utf8(fs::read(proc.join("cmdline"))?)?;
        let args = args.split('\0').collect::<Vec<_>>();
        let profile = args
            .iter()
            .position(|s| *s == "--profile")
            .and_then(|i| args.get(i + 1))
            .copied()
            .unwrap_or("default");
        let folder = paths.home.join(".config/pcmanfm").join(profile);
        if folder.exists() {
            for e in fs::read_dir(folder)? {
                let p = e?.path();
                if p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("desktop-items-")
                    && p.extension().is_some_and(|s| s == "conf")
                {
                    backup(paths, &p)?;
                }
            }
        }
        run(
            &[
                "pcmanfm".into(),
                "--profile".into(),
                profile.into(),
                format!("--set-wallpaper={}", image.display()),
                format!(
                    "--wallpaper-mode={}",
                    if cover(cfg) { "crop" } else { "fit" }
                ),
            ],
            &vars,
            25,
        )?;
        return Ok(json!({"desktop":"pcmanfm","profile":profile,"image":image}));
    }
    if which("gsettings").is_some() {
        let schema = "org.gnome.desktop.background";
        let gs = |args: Vec<String>| run(&args, &vars, 25);
        let keys = gs(vec!["gsettings".into(), "list-keys".into(), schema.into()])?;
        let read = || -> Result<Value> {
            let mut data = json!({});
            for key in ["picture-uri", "picture-uri-dark", "picture-options"] {
                if keys.lines().any(|s| s == key) {
                    data[key] = json!(gs(vec![
                        "gsettings".into(),
                        "get".into(),
                        schema.into(),
                        key.into()
                    ])?);
                }
            }
            Ok(data)
        };
        let saved = paths.state.join("desktop-before.json");
        if !saved.exists() {
            write_json(&saved, &read()?)?;
        }
        let uri = url::Url::from_file_path(image)
            .map_err(|_| anyhow::anyhow!("Invalid image path"))?
            .to_string();
        for key in ["picture-uri", "picture-uri-dark"] {
            if keys.lines().any(|s| s == key) {
                gs(vec![
                    "gsettings".into(),
                    "set".into(),
                    schema.into(),
                    key.into(),
                    uri.clone(),
                ])?;
            }
        }
        gs(vec![
            "gsettings".into(),
            "set".into(),
            schema.into(),
            "picture-options".into(),
            if cover(cfg) { "zoom" } else { "scaled" }.into(),
        ])?;
        return read();
    }
    if which("pcmanfm").is_some() {
        run(
            &[
                "pcmanfm".into(),
                format!("--set-wallpaper={}", image.display()),
                format!(
                    "--wallpaper-mode={}",
                    if cover(cfg) { "crop" } else { "fit" }
                ),
            ],
            &vars,
            25,
        )?;
        return Ok(json!({"desktop":"pcmanfm","image":image}));
    }
    bail!("No supported desktop session (GNOME or PCManFM)")
}
fn pids(name: &str) -> Vec<i32> {
    cmd(&["pgrep", "-u", &uid().to_string(), "-x", name])
        .unwrap_or_default()
        .split_whitespace()
        .filter_map(|s| s.parse().ok())
        .collect()
}
fn ghostty(paths: &Paths, image: &Path, cfg: &Value) -> Result<Value> {
    let generated = paths.config.join("ghostty.conf");
    atomic(&generated,format!("background-image = {}\nbackground-image-opacity = {}\nbackground-image-fit = {}\nbackground-image-position = center\nbackground-image-repeat = false\n",image.display(),opacity(cfg),if cover(cfg){"cover"}else{"contain"}).as_bytes())?;
    let folder = paths.home.join(".config/ghostty");
    let target = folder.join(if folder.join("config.ghostty").exists() {
        "config.ghostty"
    } else {
        "config"
    });
    managed_block(
        paths,
        &target,
        &format!("config-file = {}", generated.display()),
    )?;
    #[allow(unused_mut)]
    let mut reloaded = 0;
    #[cfg(target_os = "linux")]
    for pid in pids("ghostty") {
        let s = fs::read_to_string(format!("/proc/{pid}/status"))?;
        let mask = s
            .lines()
            .find_map(|l| l.strip_prefix("SigCgt:"))
            .and_then(|s| u64::from_str_radix(s.trim(), 16).ok())
            .unwrap_or(0);
        if mask & (1 << (libc::SIGUSR2 - 1)) != 0 && unsafe { libc::kill(pid, libc::SIGUSR2) } == 0
        {
            reloaded += 1;
        }
    }
    Ok(
        json!({"config":target,"reload_signalled":reloaded,"note":"On macOS use Reload Configuration for existing Ghostty windows."}),
    )
}
fn kitty(paths: &Paths, image: &Path, cfg: &Value) -> Result<Value> {
    let generated = paths.config.join("kitty.conf");
    atomic(&generated,format!("background_image {}\nbackground_image_layout cscaled\nbackground_image_linear yes\nbackground_tint {:.2}\n",image.display(),1.0-opacity(cfg)).as_bytes())?;
    let target = paths.home.join(".config/kitty/kitty.conf");
    managed_block(paths, &target, &format!("include {}", generated.display()))?;
    let mut count = 0;
    #[cfg(unix)]
    for pid in pids("kitty") {
        if unsafe { libc::kill(pid, libc::SIGUSR1) } == 0 {
            count += 1;
        }
    }
    Ok(json!({"config":target,"reload_signalled":count}))
}
pub fn write_iterm_profile(profile: &Path, payload: &[u8]) -> Result<()> {
    let watched = profile.parent().context("Missing profile directory")?;
    fs::create_dir_all(watched)?;
    // iTerm reads EVERY file in DynamicProfiles. Never stage a partial JSON file there.
    stage(
        profile,
        payload,
        watched.parent().context("Missing iTerm directory")?,
    )?
    .persist(profile)
    .map_err(|e| e.error)?;
    Ok(())
}
fn iterm(paths: &Paths, image: &Path, cfg: &Value) -> Result<Value> {
    if !cfg!(target_os = "macos") {
        bail!("iTerm2 requires macOS");
    }
    let profile = paths
        .home
        .join("Library/Application Support/iTerm2/DynamicProfiles/Paintings.json");
    backup(paths, &profile)?;
    let saved = paths.state.join("iterm-default-before.json");
    if !saved.exists() {
        let old = cmd(&[
            "defaults",
            "read",
            "com.googlecode.iterm2",
            "DefaultBookmarkGuid",
        ]);
        write_json(
            &saved,
            &json!({"present":old.is_ok(),"value":old.unwrap_or_default()}),
        )?;
    }
    let payload = json!({"Profiles":[{"Name":"Paintings","Guid":"2C5D36E7-47CF-40C2-A60B-5A835453CD41","Dynamic Profile Parent Name":"Default","Background Image Location":image,"Background Image Mode":if cover(cfg){2}else{3},"Blend":1.0-opacity(cfg)}]});
    write_iterm_profile(&profile, &serde_json::to_vec_pretty(&payload)?)?;
    cmd(&[
        "defaults",
        "write",
        "com.googlecode.iterm2",
        "DefaultBookmarkGuid",
        "-string",
        "2C5D36E7-47CF-40C2-A60B-5A835453CD41",
    ])?;
    Ok(
        json!({"profile":profile,"default":true,"note":"Existing windows can select Paintings once; new windows use this profile."}),
    )
}
fn opencode(paths: &Paths, _cfg: &Value) -> Result<Value> {
    let folder = paths.home.join(".config/opencode");
    let theme = folder.join("themes/paintings.json");
    backup(paths, &theme)?;
    atomic(&theme, assets::THEME)?;
    let executable = which("opencode").or_else(|| {
        let p = paths.home.join(".opencode/bin/opencode");
        p.exists().then_some(p)
    });
    let version = match executable {
        Some(p) => cmd(&[&p.to_string_lossy(), "--version"])?,
        None => "2".into(),
    };
    if version.contains("v1.") || version.starts_with("1.") {
        bail!("This adapter targets OpenCode 2.x");
    }
    let target = folder.join("cli.json");
    backup(paths, &target)?;
    let mut data = if target.exists() {
        read_json(&target)?
    } else {
        json!({})
    };
    data["theme"] = json!({"name":"paintings","mode":"dark"});
    write_json(&target, &data)?;
    Ok(
        json!({"theme":theme,"version":version,"note":"Transparent canvas uses the terminal emulator background; reopen theme settings for existing sessions."}),
    )
}
fn openchamber(paths: &Paths) -> Result<Value> {
    if cfg!(target_os = "macos") {
        let saved = paths.state.join("openchamber-env-before.json");
        if !saved.exists() {
            let old = cmd(&["launchctl", "getenv", "OPENCHAMBER_ELECTRON_LOAD_SERVER_UI"]);
            write_json(
                &saved,
                &json!({"present":old.is_ok(),"value":old.unwrap_or_default()}),
            )?;
        }
        cmd(&[
            "launchctl",
            "setenv",
            "OPENCHAMBER_ELECTRON_LOAD_SERVER_UI",
            "1",
        ])?;
        return Ok(json!({"native":"Server frontend enabled for next OpenChamber launch"}));
    }
    if cfg!(target_os = "linux") {
        let target = paths
            .home
            .join(".local/share/applications/openchamber.desktop");
        if !target.exists() {
            return Ok(
                json!({"native":"No desktop launcher installed; web frontend remains available"}),
            );
        }
        backup(paths, &target)?;
        let s = fs::read_to_string(&target)?;
        if !s.contains("OPENCHAMBER_ELECTRON_LOAD_SERVER_UI=1") {
            atomic(
                &target,
                s.replacen(
                    "Exec=",
                    "Exec=env OPENCHAMBER_ELECTRON_LOAD_SERVER_UI=1 ",
                    1,
                )
                .as_bytes(),
            )?;
        }
        return Ok(json!({"native":target,"note":"Uses server frontend on next launch"}));
    }
    bail!("Native OpenChamber adapter supports macOS and Linux")
}
