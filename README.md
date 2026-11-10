# Paintings

Rotating art for desktops, terminals, and web apps. Built in Rust; no Python required. Eight streams of existing artwork. No generated images.

| Stream | Ten examples |
| :-- | :-- |
| [classical](docs/previews/README.md#classical) | ![Ten classical architectural landscapes](docs/previews/classical.jpg) |
| [landscapes](docs/previews/README.md#landscapes) | ![Ten landscape paintings](docs/previews/landscapes.jpg) |
| [renaissance](docs/previews/README.md#renaissance) | ![Ten Renaissance paintings](docs/previews/renaissance.jpg) |
| [baroque](docs/previews/README.md#baroque) | ![Ten Baroque paintings](docs/previews/baroque.jpg) |
| [romanticism](docs/previews/README.md#romanticism) | ![Ten Romantic paintings](docs/previews/romanticism.jpg) |
| [impressionism](docs/previews/README.md#impressionism) | ![Ten Impressionist and Post-Impressionist paintings](docs/previews/impressionism.jpg) |
| [abstraction](docs/previews/README.md#abstraction) | ![Ten abstract and Cubist paintings](docs/previews/abstraction.jpg) |
| [visionary](docs/previews/README.md#visionary) | ![Ten visionary, futuristic, and science-fiction artworks](docs/previews/visionary.jpg) |

## Install

Extract a [release binary](https://github.com/bestquark/paintings/releases/latest) and run `./paintings install --adapters desktop`. Or build with Cargo:

```sh
cargo install --git https://github.com/bestquark/paintings --locked --bin paintings
paintings install --adapters desktop
paintings tick
```

Defaults to **classical**, changes every **5 minutes**, and fills the screen with proportional cropping. Images download on demand. macOS uses a user LaunchAgent; Linux uses a user systemd timer.

```sh
paintings configure --stream landscapes --fit cover --apply
paintings next
paintings status
paintings uninstall
```

## Integrations

Desktop: **macOS**, **GNOME**, **PCManFM**. Optional adapters: **ghostty**, **iterm**, **kitty**, **opencode** (2.x), **openchamber**. Pass installed adapters as a comma-separated list to `install --adapters`. Existing terminal windows may need a config reload or the Paintings profile selected.

For OpenChamber, install the browser plugin into its web build and refresh:

```sh
paintings install-web --root /path/to/openchamber/web/dist
```

For other websites, serve `web/paintings.js`, `web/paintings.css`, `catalog/`, and `config/` together and include `<script defer src="/paintings/paintings.js" data-paintings></script>`.

Preferences live in `~/.config/paintings/config.json`. Optional `install --config-url https://…/default.json` shares visual defaults; `configure` saves local overrides. Use `--stream default --fit default` to follow shared settings again.

## License

Code: [MIT](LICENSE). Art retains its own license and attribution, recorded with source links in each catalog entry. See [CONTRIBUTING.md](CONTRIBUTING.md) for catalog maintenance.
