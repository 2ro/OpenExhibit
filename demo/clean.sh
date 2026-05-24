#!/usr/bin/env bash
# Remove the demo content seeded by demo/seed.sh.
# DEMO ONLY.

set -euo pipefail

PG="${DATABASE_URL:-postgres://openexhibit@localhost:5433/openexhibit}"
FILES_DIR="${FILES_DIR:-files/gimgs}"

# Collect demo exhibit ids before we delete.
demo_ids=$(psql -At "$PG" -c "SELECT id FROM exhibits WHERE url LIKE '/demo%' OR section_id IN (SELECT id FROM sections WHERE path = '/demo')")

psql -q "$PG" <<'SQL'
DELETE FROM media WHERE ref_id IN (
  SELECT id FROM exhibits WHERE url LIKE '/demo%' OR section_id IN (SELECT id FROM sections WHERE path = '/demo')
);
DELETE FROM exhibits WHERE url LIKE '/demo%' OR section_id IN (SELECT id FROM sections WHERE path = '/demo');
DELETE FROM sections WHERE path = '/demo';
SQL

for id in $demo_ids; do
  rm -rf "$FILES_DIR/$id"
  rm -rf "files/dimgs/$id"
done

echo "Demo content removed."
