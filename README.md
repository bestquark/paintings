# Paintings

Rotating art for desktops, terminals, and web apps. Eight streams, 200 existing artworks each: **classical**, **landscapes**, **renaissance**, **baroque**, **romanticism**, **impressionism**, **abstraction**, and **visionary**. No generated images.

## Install

Requires Python 3.9+. macOS desktop support also requires Apple's command-line tools.

```sh
git clone https://github.com/bestquark/paintings.git
cd paintings
python3 -m paintings install --adapters desktop
python3 -m paintings tick
```

Defaults to **classical**, changes every **5 minutes**, and fills the screen with proportional cropping. Images download on demand. macOS uses a user LaunchAgent; Linux uses a user systemd timer.

```sh
python3 -m paintings configure --stream landscapes --fit cover --apply
python3 -m paintings next
python3 -m paintings status
python3 -m paintings uninstall
```

## Integrations

Desktop: **macOS**, **GNOME**, **PCManFM**. Optional adapters: **ghostty**, **iterm**, **kitty**, **opencode** (2.x), **openchamber**. Pass installed adapters as a comma-separated list to `install --adapters`. Existing terminal windows may need a config reload or the Paintings profile selected.

For OpenChamber, install the browser plugin into its web build and refresh:

```sh
python3 -m paintings install-web --root /path/to/openchamber/web/dist
```

For other websites, serve `web/paintings.js`, `web/paintings.css`, `catalog/`, and `config/` together and include `<script defer src="/paintings/paintings.js" data-paintings></script>`.

Preferences live in `~/.config/paintings/config.json`. Optional `install --config-url https://…/default.json` shares visual defaults; `configure` saves local overrides. Use `--stream default --fit default` to follow shared settings again.

## License

Code: [MIT](LICENSE). Art retains its own license and attribution, recorded with source links in each catalog entry. See [CONTRIBUTING.md](CONTRIBUTING.md) for catalog maintenance.
