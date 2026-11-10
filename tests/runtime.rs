use bestquark_paintings::{Paths, adapters, assets, config, install, rotation, storage::*};
use serde_json::{Value, json};
use std::{collections::HashSet, fs};

fn setup() -> (tempfile::TempDir, Paths, Value) {
    let temp = tempfile::tempdir().unwrap();
    let paths = Paths::isolated(temp.path().to_path_buf());
    let mut cfg: Value = serde_json::from_str(assets::DEFAULTS).unwrap();
    cfg["adapters"] = json!([]);
    write_json(&paths.config.join("config.json"), &cfg).unwrap();
    (temp, paths, cfg)
}
#[test]
fn catalog_contract() {
    let exclusions: Value =
        serde_json::from_str(include_str!("../tools/stream-exclusions.json")).unwrap();
    for stream in assets::STREAMS {
        let arts = assets::catalog(stream).unwrap();
        assert_eq!(arts.len(), if stream == "classical" { 34 } else { 200 });
        let mut ids = HashSet::new();
        let mut hashes = HashSet::new();
        for a in arts {
            assert!(exclusions[stream].get(a["id"].as_str().unwrap()).is_none());
            assert!(ids.insert(a["id"].clone().to_string()));
            assert!(hashes.insert(a["image_sha256"].clone().to_string()));
            assert_eq!(a["generated"], false);
            for k in [
                "title",
                "artist",
                "credit",
                "license",
                "source_url",
                "license_url",
                "image_url",
            ] {
                assert!(!a[k].as_str().unwrap().is_empty(), "{stream}: {k}");
            }
            assert!(a["image_url"].as_str().unwrap().starts_with("https://"));
            assert!(a["display_width"].as_u64().unwrap() >= 1100);
            assert!(a["display_height"].as_u64().unwrap() >= 600);
        }
    }
}
#[test]
fn rotation_has_no_repeats_and_matches_previous_order() {
    let (_, _, mut cfg) = setup();
    cfg["streams"] = json!(["renaissance", "landscapes"]);
    cfg["seed"] = json!("test");
    let mut chosen = HashSet::new();
    for slot in 0..400 {
        let (s, a) = rotation::selection(&cfg, f64::from(slot * 300), 0).unwrap();
        assert!(chosen.insert((s, a["id"].to_string())));
    }
    assert_eq!(
        rotation::selection(&cfg, 301.0, 0).unwrap(),
        rotation::selection(&cfg, 599.0, 0).unwrap()
    );
    let fixtures: Value = serde_json::from_str(include_str!("fixtures/rotation.json")).unwrap();
    for v in fixtures.as_array().unwrap() {
        cfg["streams"] = v["streams"].clone();
        cfg["seed"] = v["seed"].clone();
        let (s, a) = rotation::selection(
            &cfg,
            v["now"].as_f64().unwrap(),
            v["offset"].as_u64().unwrap(),
        )
        .unwrap();
        assert_eq!(s, v["stream"].as_str().unwrap());
        assert_eq!(a["id"], v["id"]);
    }
}
#[test]
fn managed_config_preserves_original() {
    let (_t, p, _) = setup();
    let file = p.home.join("terminal.conf");
    atomic(&file, b"font-size = 18\n").unwrap();
    managed_block(&p, &file, "config-file = old.conf").unwrap();
    managed_block(&p, &file, "config-file = new.conf").unwrap();
    let text = fs::read_to_string(&file).unwrap();
    assert_eq!(text.matches("# paintings:start").count(), 1);
    assert!(text.contains("font-size = 18"));
    assert!(text.contains("new.conf"));
    assert_eq!(
        fs::read_to_string(backup(&p, &file).unwrap()).unwrap(),
        "font-size = 18\n"
    );
}
#[test]
fn rejects_http_and_unsafe_ids_before_network() {
    let (_t, p, _) = setup();
    assert!(
        rotation::download(
            &p,
            &json!({"id":"bad","image_url":"http://example.com/a.jpg"})
        )
        .is_err()
    );
    assert!(
        rotation::image_path(
            &p,
            &json!({"id":"../escape","image_url":"https://example.com/a.jpg"})
        )
        .is_err()
    );
    assert!(!p.cache.exists());
}
#[test]
fn offline_preserves_previous_painting() {
    let (_t, p, _) = setup();
    write_json(&p.state.join("current.json"), &json!({"kept":true})).unwrap();
    let result =
        rotation::tick_with_downloader(&p, None, None, false, |_, _| anyhow::bail!("offline"));
    assert!(result.unwrap_err().to_string().contains("keeping"));
    assert_eq!(rotation::current(&p).unwrap(), json!({"kept":true}));
    assert_eq!(
        read_json(&p.state.join("last-error.json")).unwrap()["failures"]
            .as_array()
            .unwrap()
            .len(),
        8
    );
}
#[test]
fn concurrent_rotation_does_not_overwrite_state() {
    let (_t, p, _) = setup();
    let _guard = lock(&p.state.join("rotation.lock"), false)
        .unwrap()
        .unwrap();
    let result = rotation::tick_with_downloader(&p, None, None, false, |_, _| {
        panic!("Must not download while locked")
    })
    .unwrap();
    assert!(result.get("skipped").is_some());
}
#[test]
fn selected_stream_wins_for_overlapping_artwork() {
    let (_t, p, _) = setup();
    let state = rotation::tick_with_downloader(
        &p,
        Some("romanticism"),
        Some("commons-183030"),
        false,
        |p, _| Ok(p.cache.join("test.jpg")),
    )
    .unwrap();
    assert_eq!(state["stream"], "romanticism");
    assert_eq!(state["manual_offset"], 0);
    assert_eq!(state["fit"], "cover");
}
#[test]
fn overrides_survive_shared_refresh_and_default_clears() {
    let (_t, p, mut cfg) = setup();
    cfg["config_url"] = json!("https://example.com/config.json");
    cfg["overrides"] = json!({"streams":["renaissance"],"fit":"contain"});
    write_json(&p.config.join("config.json"), &cfg).unwrap();
    write_json(
        &p.state.join("shared.json"),
        &json!({"streams":["classical"],"fit":"cover","interval_seconds":300}),
    )
    .unwrap();
    let s = config::status(&p).unwrap();
    assert_eq!(s["settings"]["streams"], json!(["renaissance"]));
    assert_eq!(s["settings"]["fit"], "contain");
    let s = config::configure(&p, Some("default"), Some("default"), false).unwrap();
    assert_eq!(s["settings"]["streams"], json!(["classical"]));
    assert_eq!(s["settings"]["fit"], "cover");
    assert_eq!(s["overrides"], json!({}));
}
#[test]
fn invalid_shared_sizing_never_reaches_adapters() {
    let (_t, p, mut cfg) = setup();
    cfg["config_url"] = json!("https://example.com/config.json");
    write_json(&p.state.join("shared.json"), &json!({"fit":"invalid"})).unwrap();
    assert!(
        config::sync(&p, cfg, false)
            .unwrap_err()
            .to_string()
            .contains("fit")
    );
}
#[test]
fn web_install_is_idempotent_and_excludes_private_config() {
    let (_t, p, mut cfg) = setup();
    cfg["config_url"] = json!("https://private.example/secret");
    cfg["overrides"] = json!({"streams":["landscapes"]});
    write_json(&p.config.join("config.json"), &cfg).unwrap();
    let site = p.home.join("site");
    atomic(
        &site.join("index.html"),
        b"<html><head><title>Existing app</title></head><body>keep</body></html>",
    )
    .unwrap();
    install::install_web(&p, &site).unwrap();
    install::install_web(&p, &site).unwrap();
    let html = fs::read_to_string(site.join("index.html")).unwrap();
    assert_eq!(html.matches("data-paintings").count(), 1);
    assert!(html.contains("<body>keep</body>"));
    let public = read_json(&site.join("paintings/config/default.json")).unwrap();
    for k in ["config_url", "overrides", "adapters"] {
        assert!(public.get(k).is_none());
    }
    assert_eq!(public["streams"], json!(["landscapes"]));
    assert_eq!(
        read_json(&p.state.join("web-roots.json"))
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn web_revision_is_compatible_with_previous_runtime() {
    let (_t, _p, cfg) = setup();
    let expected = include_str!("fixtures/web-revision.txt").trim();
    assert_eq!(config::web_config(&cfg)["preference_revision"], expected);
}
#[test]
fn iterm_watch_directory_never_contains_partial_json() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    let (_t, p, _) = setup();
    let folder = p.home.join("iTerm2/DynamicProfiles");
    fs::create_dir_all(&folder).unwrap();
    let profile = folder.join("Paintings.json");
    atomic(&folder.join("Other.json"), b"{}").unwrap();
    adapters::write_iterm_profile(&profile, b"{\"Profiles\":[]}").unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    let watch = folder.clone();
    let handle = std::thread::spawn(move || {
        while !flag.load(Ordering::Acquire) {
            for item in fs::read_dir(&watch).unwrap() {
                let path = item.unwrap().path();
                assert!(matches!(
                    path.file_name().unwrap().to_str().unwrap(),
                    "Paintings.json" | "Other.json"
                ));
                let _: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            }
        }
    });
    for n in 0..100 {
        adapters::write_iterm_profile(
            &profile,
            &serde_json::to_vec(&json!({"Profiles":[{"revision":n}]})).unwrap(),
        )
        .unwrap();
    }
    stop.store(true, Ordering::Release);
    handle.join().unwrap();
    assert_eq!(fs::read_dir(folder.parent().unwrap()).unwrap().count(), 1);
}
#[test]
fn failed_iterm_replace_cleans_staging() {
    let (_t, p, _) = setup();
    let target = p.home.join("iTerm2/DynamicProfiles/Paintings.json");
    fs::create_dir_all(&target).unwrap();
    atomic(&target.join("keep"), b"original").unwrap();
    assert!(adapters::write_iterm_profile(&target, b"{}").is_err());
    assert_eq!(fs::read(target.join("keep")).unwrap(), b"original");
    assert_eq!(
        fs::read_dir(target.parent().unwrap().parent().unwrap())
            .unwrap()
            .count(),
        1
    );
}
#[test]
fn previews_have_ten_existing_works_per_stream() {
    let selections: Value = serde_json::from_str(include_str!("../tools/previews.json")).unwrap();
    for s in assets::STREAMS {
        let ids = selections[s].as_array().unwrap();
        assert_eq!(ids.len(), 10);
        assert_eq!(
            ids.iter()
                .map(Value::to_string)
                .collect::<HashSet<_>>()
                .len(),
            10
        );
        let arts = assets::catalog(s).unwrap();
        assert!(ids.iter().all(|id| arts.iter().any(|a| a["id"] == *id)));
    }
}

#[cfg(unix)]
#[test]
fn standalone_install_and_uninstall_need_no_interpreter() {
    use std::process::Command;
    let temp = tempfile::tempdir().unwrap();
    let home = temp.path().join("home with spaces");
    fs::create_dir_all(&home).unwrap();
    let run = |binary: &std::path::Path, args: &[&str]| {
        let output = Command::new(binary)
            .args(args)
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_STATE_HOME", home.join(".local/state"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
    };
    let source = std::path::Path::new(env!("CARGO_BIN_EXE_paintings"));
    run(source, &["install", "--no-schedule", "--adapters", ""]);
    let installed = home.join(".local/bin/paintings");
    assert!(installed.exists());
    assert!(
        String::from_utf8(run(&installed, &["--version"]))
            .unwrap()
            .contains(env!("CARGO_PKG_VERSION"))
    );
    let status: Value = serde_json::from_slice(&run(&installed, &["status"])).unwrap();
    assert_eq!(status["settings"]["streams"], json!(["classical"]));
    assert_eq!(status["settings"]["fit"], "cover");
    run(&installed, &["uninstall"]);
    assert!(!installed.exists());
    assert!(!home.join(".config/paintings/config.json").exists());
}
