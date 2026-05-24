-- Optional 4chan-style greentext rendering. When enabled, lines that
-- start with `>` render as `<p class="greentext">` (#789922) instead
-- of being parsed as Markdown blockquotes.

ALTER TABLE settings
    ADD COLUMN enable_greentext BOOLEAN NOT NULL DEFAULT FALSE;
