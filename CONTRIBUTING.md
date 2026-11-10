# Contributing artwork

Keep stream names stable. Add existing artwork only, with a source page that identifies the creator and the image's license or usage policy. Do not add generated images, unlicensed work, gallery photographs, diagrams, or duplicate scans.

Each catalog record includes a stable `id`, `title`, `artist`, `date` when known, `image_url`, `original_url`, `source_url`, `source`, `credit`, `license`, `license_url`, original `width` and `height`, decoded `display_width` and `display_height`, `image_sha256`, and `generated: false`. Use HTTPS. Unknown creation dates stay blank; archival upload dates must not be presented as painting dates.

Each stream must retain at least 200 distinct artworks. Thematic overlap between streams is intentional.

Run the offline contract and behavior checks:

```sh
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
node --check web/paintings.js
```

Catalog tools are also Rust; image processing is an optional build feature:

```sh
cargo run --release --features catalog-tools --bin paintings-catalog -- verify landscapes
cargo run --release --features catalog-tools --bin paintings-catalog -- finalize landscapes --sheets
cargo run --release --features catalog-tools --bin paintings-catalog -- previews
```

Verification downloads source images and writes analysis under `.work/`. Inspect the contact sheets and source pages; perceptual matching needs manual review. `tools/exclusions.json` records rejected candidates. `finalize --write` replaces a stream with its first 200 verified entries. `tools/previews.json` selects the ten credited thumbnails per README row.

`paintings gallery --output dist/gallery` builds the static web gallery. macOS source builds need Apple's command-line tools; release binaries already bundle the small native desktop helper. Existing JSON preferences and cached artwork remain compatible when upgrading from 0.2.

Do not commit downloaded originals, local configuration, browser state, screenshots of private apps, or `.work/`. The repository is a catalog and integration package; artworks retain their own terms.
