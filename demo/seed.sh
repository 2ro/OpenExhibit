#!/usr/bin/env bash
# Seed the running OpenExhibit instance with demonstration content.
# DEMO ONLY — delete the demo/ directory before shipping as a generic CMS.
#
# Usage: bash demo/seed.sh
# Requires: psql, curl, a Postgres instance reachable via $DATABASE_URL
#           (defaults to the dev cluster on :5433).

set -euo pipefail

PG="${DATABASE_URL:-postgres://openexhibit@localhost:5433/openexhibit}"
FILES_DIR="${FILES_DIR:-files/gimgs}"

# Clean any prior demo state.
bash "$(dirname "$0")/clean.sh" >/dev/null

echo "Inserting demo section + exhibits..."
psql -q "$PG" <<'SQL'
INSERT INTO sections (name, kind, ord, display, hidden, path, description)
VALUES ('demo', 'exhibits', 50, 1, FALSE, '/demo', 'Demo gallery');

WITH s AS (SELECT id FROM sections WHERE path = '/demo')
INSERT INTO exhibits
  (kind, title, content, status, process, section_id, section_top, url, ord, format, thumbs, tiling, year, template, is_home)
SELECT 'exhibits', t.title, t.content, 1, TRUE, s.id, t.top, t.url, t.ord, t.format, 200, TRUE, '2026', 'index.php', FALSE
FROM s, (VALUES
  ('Photos',  '<p>Demonstration of the <code>visual_index</code> format: a thumbnail grid that links to an in-page lightbox.</p>', TRUE,  '/demo/',       10, 'visual_index'),
  ('Strip',   '<p>Demonstration of the <code>horizontal</code> format: a side-scrolling strip. Drag the scrollbar, two-finger swipe on a trackpad, or swipe on mobile.</p>', FALSE, '/demo/strip/', 15, 'horizontal'),
  ('Film',    '<p>Demonstration of the <code>slideshow</code> format. Use the next / previous links to step through.</p>', FALSE, '/demo/film/',  20, 'slideshow'),
  ('Sound',   '<p>Demonstration of audio playback using the native HTML <code>&lt;audio&gt;</code> element.</p>',                FALSE, '/demo/sound/', 30, 'no_thumbs'),
  ('Grid',    '<p>Demonstration of the <code>documenta</code> grid format.</p>',                                                 FALSE, '/demo/grid/',  40, 'documenta')
) AS t(title, content, top, url, ord, format);
SQL

PHOTOS_ID=$(psql -At "$PG" -c "SELECT id FROM exhibits WHERE url = '/demo/'")
STRIP_ID=$(psql -At "$PG" -c "SELECT id FROM exhibits WHERE url = '/demo/strip/'")
FILM_ID=$(psql -At "$PG" -c "SELECT id FROM exhibits WHERE url = '/demo/film/'")
SOUND_ID=$(psql -At "$PG" -c "SELECT id FROM exhibits WHERE url = '/demo/sound/'")
GRID_ID=$(psql -At "$PG" -c "SELECT id FROM exhibits WHERE url = '/demo/grid/'")

echo "  ids: photos=$PHOTOS_ID strip=$STRIP_ID film=$FILM_ID sound=$SOUND_ID grid=$GRID_ID"

download() {
  local url="$1" dest="$2"
  echo "    $url"
  curl -sL --max-time 60 -o "$dest" "$url"
}

insert_media() {
  local ref_id="$1" mime="$2" file="$3" title="$4" ord="$5" width="$6" height="$7"
  psql -q "$PG" -c "INSERT INTO media (ref_id, obj_type, mime, file, title, width, height, ord, uploaded_at) \
                    VALUES ($ref_id, 'exhibits', '$mime', '$file', '$title', $width, $height, $ord, now())"
}

# ---------- Photos (visual_index) ----------
mkdir -p "$FILES_DIR/$PHOTOS_ID"
echo "Photos (6 images)..."
i=1
for seed in mountain river city forest desert ocean; do
  download "https://picsum.photos/seed/$seed/1200/800.jpg" "$FILES_DIR/$PHOTOS_ID/$seed.jpg"
  insert_media "$PHOTOS_ID" jpg "$seed.jpg" "$seed" "$((i*10))" 1200 800
  i=$((i+1))
done

# ---------- Strip (horizontal) ----------
mkdir -p "$FILES_DIR/$STRIP_ID"
echo "Strip (8 wide images)..."
i=1
for seed in strip-a strip-b strip-c strip-d strip-e strip-f strip-g strip-h; do
  download "https://picsum.photos/seed/$seed/1400/900.jpg" "$FILES_DIR/$STRIP_ID/$seed.jpg"
  insert_media "$STRIP_ID" jpg "$seed.jpg" "$seed" "$((i*10))" 1400 900
  i=$((i+1))
done

# ---------- Film (slideshow) ----------
mkdir -p "$FILES_DIR/$FILM_ID"
echo "Film (3 photos + 1 video)..."
i=1
for seed in dusk dawn rain; do
  download "https://picsum.photos/seed/film-$seed/1600/900.jpg" "$FILES_DIR/$FILM_ID/$seed.jpg"
  insert_media "$FILM_ID" jpg "$seed.jpg" "$seed" "$((i*10))" 1600 900
  i=$((i+1))
done
download "https://media.w3.org/2010/05/sintel/trailer.mp4" "$FILES_DIR/$FILM_ID/sintel.mp4"
insert_media "$FILM_ID" mp4 "sintel.mp4" "Sintel (Blender Foundation, CC-BY)" 40 1280 720

# ---------- Sound (no_thumbs) ----------
mkdir -p "$FILES_DIR/$SOUND_ID"
echo "Sound (1 image + 1 audio)..."
download "https://picsum.photos/seed/sound-cover/1200/800.jpg" "$FILES_DIR/$SOUND_ID/cover.jpg"
insert_media "$SOUND_ID" jpg cover.jpg "Cover" 10 1200 800

download "https://upload.wikimedia.org/wikipedia/commons/c/c8/Example.ogg" "$FILES_DIR/$SOUND_ID/example.ogg"
insert_media "$SOUND_ID" ogg example.ogg "Piano sample (Wikimedia commons)" 20 0 0

# ---------- Grid (documenta) ----------
mkdir -p "$FILES_DIR/$GRID_ID"
echo "Grid (4 images)..."
i=1
for seed in alpha beta gamma delta; do
  download "https://picsum.photos/seed/grid-$seed/900/600.jpg" "$FILES_DIR/$GRID_ID/$seed.jpg"
  insert_media "$GRID_ID" jpg "$seed.jpg" "$seed" "$((i*10))" 900 600
  i=$((i+1))
done

# ---------- Subsection grouping (so /demo/ sidebar shows a "Stills" group) ----------
echo "Subsection + tags..."
DEMO_SEC=$(psql -At "$PG" -c "SELECT id FROM sections WHERE path = '/demo'")
psql -q "$PG" <<SQL
INSERT INTO subsections (section_id, title, ord, hidden) VALUES ($DEMO_SEC, 'Stills', 10, FALSE);
UPDATE exhibits SET section_sub = 'Stills' WHERE url IN ('/demo/film/', '/demo/grid/');
SQL

# ---------- Tags (so /tag/landscape works) ----------
psql -q "$PG" <<'SQL'
INSERT INTO tags (name, created_at) VALUES ('landscape', now()), ('portrait', now()), ('film', now())
  ON CONFLICT (name) DO NOTHING;
INSERT INTO tagged (tag_id, obj_type, obj_id)
SELECT g.id, 'exh', e.id FROM tags g, exhibits e
WHERE (g.name = 'landscape' AND e.url IN ('/demo/', '/demo/strip/', '/demo/grid/'))
   OR (g.name = 'film'      AND e.url IN ('/demo/film/'))
   OR (g.name = 'portrait'  AND e.url IN ('/demo/'))
ON CONFLICT DO NOTHING;
SQL

echo ""
echo "Demo seed complete. Visit:"
echo "  http://127.0.0.1:8080/demo/"
echo "  http://127.0.0.1:8080/demo/strip/"
echo "  http://127.0.0.1:8080/tag/landscape"
echo "  http://127.0.0.1:8080/tag/film"
echo "  http://127.0.0.1:8080/demo/film/"
echo "  http://127.0.0.1:8080/demo/sound/"
echo "  http://127.0.0.1:8080/demo/grid/"
