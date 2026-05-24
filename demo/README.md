# Demo data

**Everything in this directory is sample data, not part of the production CMS.**
Delete the whole `demo/` folder when packaging OpenExhibit as a generic
portfolio CMS — the application code does not reference it.

The seeder creates:

- A new section `Demo` at path `/demo`
- Four exhibits demonstrating each major format:
  - `/demo/photos/` — visual_index (6 images)
  - `/demo/film/` — slideshow (3 images + 1 video)
  - `/demo/sound/` — no_thumbs (audio + accompanying image)
  - `/demo/grid/` — documenta (4 images)

All media is downloaded from public, stable sources:

- **Images**: [picsum.photos](https://picsum.photos) (CC0-style; random Unsplash photos by seed)
- **Video**: [media.w3.org Sintel trailer](https://media.w3.org/2010/05/sintel/trailer.mp4) (Blender Foundation, CC-BY)
- **Audio**: [Wikimedia commons `Example.ogg`](https://upload.wikimedia.org/wikipedia/commons/c/c8/Example.ogg)

## Usage

```sh
# from the project root, with the dev server's PG running:
bash demo/seed.sh

# to remove demo data (DB rows + downloaded files):
bash demo/clean.sh
```

Re-running `seed.sh` is idempotent — it cleans first, then re-creates.
