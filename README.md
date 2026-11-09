# Paintings

Existing art for your desktop and terminals, changing every five minutes.

Seven streams: **renaissance**, **baroque**, **romanticism**, **impressionism**, **abstraction**, **visionary**, and **landscapes**. Each ships with 200 catalog entries, source links, image URLs, attribution, and usage terms. No AI-generated images.

Landscapes includes Renaissance world landscapes, luminous classical scenes, mountains, and monumental Romantic panoramas. Impressionism includes Van Gogh and other Post-Impressionists. Abstraction includes Cubism and related movements. Visionary is a thematic collection: Symbolist visions, historical Futurism and related avant-garde paintings, existing speculative digital paintings, and NASA space art, including the 1970s colony paintings by Rick Guidice and Don Davis. These are convenient streams, not claims that every work belongs to one historical school.

Thomas Cole's *The Course of Empire: The Consummation of Empire* (1836) is included. The catalog has 1,400 stream entries and 1,227 distinct artwork IDs. Some works belong in more than one stream; each stream is deduplicated separately. Counts and file hashes are recorded in [the manifest](catalog/manifest.json).

## Install

Python 3.9 or later. The rotator uses the standard library; macOS desktop support also needs `swiftc` from Apple's command-line tools.

```sh
git clone https://github.com/bestquark/paintings.git
cd paintings
python3 -m paintings install --adapters desktop,iterm,opencode
python3 -m paintings tick
```

For Linux, choose the installed terminal:

```sh
python3 -m paintings install --adapters desktop,kitty,opencode
# Or: --adapters desktop,ghostty,opencode
```

The installer adds a user LaunchAgent on macOS or a user systemd timer on Linux. It backs up files before adding managed settings. It does not require root or restart terminal sessions. Keep the checkout at its installed path, or install the package in a virtual environment with `python -m pip install .` first (use a current pip).

```sh
paintings streams
paintings next
paintings next --stream landscapes
paintings current
paintings tick --id commons-183030
paintings download --stream landscapes   # optional: cache an entire stream
paintings uninstall
```

Images download on demand. A failed download keeps the previous painting. The cache is limited to 4 GiB by default. Paintings are fitted without cropping by default; use `"fit": "cover"` for a filled screen.

## Shared configuration

Edit `~/.config/paintings/config.json`. The public defaults live in [`config/default.json`](config/default.json).

```sh
paintings install --adapters desktop,kitty,opencode \
  --config-url https://raw.githubusercontent.com/bestquark/paintings/main/config/default.json
```

Only visual preferences are shared. Adapter choices, local paths, logs, and backups stay on each computer. A shared seed, catalog version, and clock choose the same painting for each five-minute time slot. Each stream visits every entry before repeating. If a source is unavailable, a device may temporarily use the next available painting. Browsers can independently choose a stream.

## Integrations

| Surface | Support |
| --- | --- |
| macOS desktop | Native AppKit wallpaper setter for attached displays in the active desktop session |
| Linux desktop | GNOME and Raspberry Pi PCManFM; logged-in session required for visible changes |
| Ghostty | Managed background-image config; live reload on Linux when its signal handler exists; macOS windows may need Reload Configuration |
| iTerm2 | Dynamic `Paintings` profile, set as the default; existing windows can select it once |
| Kitty | Managed background include and live config reload |
| OpenCode 2.x | Transparent base theme in `cli.json`; terminal emulator supplies the image; raised surfaces retain contrast |
| OpenChamber native | Optional `openchamber` adapter enables its built-in server-frontend mode at next launch; install the web plugin on the configured server first |
| OpenChamber web | Background plugin with stream selection, Next, Pause, Hide, and source credit |
| Other websites | Standalone script, CSS, catalog and config; `paintings:change` event |

For OpenChamber, install into the web frontend's `dist` folder:

```sh
paintings install-web --root /path/to/openchamber/node_modules/@openchamber/web/dist
```

Refresh the page. No backend restart is needed. Reapply after an upstream frontend update replaces its files. The optional `openchamber` adapter uses OpenChamber's built-in `OPENCHAMBER_ELECTRON_LOAD_SERVER_UI` setting at next launch. Select a configured fleet server with the web plugin installed. Signed application bundles are preserved.

The browser plugin is also usable independently. Serve `web/paintings.js`, `web/paintings.css`, `catalog/`, and `config/` together, then include:

```html
<script defer src="/paintings/paintings.js" data-paintings></script>
```

Add `data-openchamber` to use the subdued application background. `web/index.html` is a full-screen gallery entry point. `tools/build_gallery.py` assembles a ready-to-serve folder.

```js
document.addEventListener('paintings:change', event => {
  console.log(event.detail.stream, event.detail.artwork);
});
```

## Artwork and licenses

The code is MIT licensed. **Artwork is not covered by the code license.** Every entry has its own source, credit, and license or usage-policy link. Historical scans come from Wikimedia Commons; contemporary paintings include David Revoy's CC BY 4.0 work; NASA artwork follows NASA's media usage guidelines, with supplied creator credits retained. No endorsement by artists, museums, or NASA is implied.

The repository stores a catalog, not a mirror of all original artwork. Image files are fetched from their custodians. Retain the catalog attribution when redistributing a selection, and follow each image's terms. NASA concept illustrations are credited as such; the collection includes both traditional and digital art.

Catalog maintenance uses source metadata, actual JPEG/PNG decoding, resolution checks, title matching, perceptual scan deduplication, and visual review. Verification and review tools are under `tools/`; their optional analysis dependencies are Pillow and NumPy. See [CONTRIBUTING.md](CONTRIBUTING.md) for the catalog contract and review workflow. Source records can change, so corrections are welcome with a source link.

The OpenCode theme derives from OpenCode's MIT-licensed built-in theme; its notice is preserved in [`adapters/OPENCODE-LICENSE`](adapters/OPENCODE-LICENSE).

## Development

```sh
python3 -m unittest discover -s tests -v
node --check web/paintings.js
```

`paintings uninstall` disables this project's scheduler and removes its managed blocks and restores its own backed-up settings. Cached art and provenance are retained. Set the desktop picture through system settings afterward. Backups are private to the local user under `~/.local/state/paintings/backups`.
