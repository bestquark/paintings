# Contributing artwork

Keep the seven short stream names stable. Add existing artwork only, with a source page that identifies the creator and the image's license or usage policy. Do not add generated images, unlicensed contemporary work, photographs of galleries, diagrams, or duplicate scans of the same painting.

Each catalog record includes a stable `id`, `title`, `artist`, `date` when known, `image_url`, `original_url`, `source_url`, `source`, `credit`, `license`, `license_url`, original `width` and `height`, decoded `display_width` and `display_height`, `image_sha256`, and `generated: false`. Use HTTPS. Unknown creation dates stay blank; archival upload dates must not be presented as painting dates.

Each stream must retain at least 200 distinct artworks. Thematic overlap between streams is intentional. Landscapes must keep Renaissance landscapes; Visionary must keep the 1970s NASA colonies and existing speculative paintings. Preserve the Thomas Cole and Van Gogh anchors tested in the suite.

Run the offline contract and behavior checks:

```sh
python3 -m unittest discover -s tests -v
node --check web/paintings.js
```

For source and image verification, install Pillow and NumPy in a development environment, then run:

```sh
python3 tools/verify_images.py landscapes
python3 tools/finalize.py landscapes --sheets
```

This downloads real source images and writes private analysis under `.work/`. Inspect the contact sheets and source pages. Perceptual matching catches scan variants, but manual review is still needed. `tools/exclusions.json` records rejected candidates. `--write` on `finalize.py` replaces that stream with the first 200 verified entries, so review the Git diff afterward.

Do not commit downloaded originals, local configuration, browser state, screenshots of private apps, or `.work/`. The repository is a catalog and integration package; artworks retain their own terms.
