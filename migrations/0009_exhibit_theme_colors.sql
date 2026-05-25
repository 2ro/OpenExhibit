-- Per-exhibit equivalents of the site-wide theme color knobs from
-- migration 0006. Empty default so existing exhibits inherit the site
-- theme (which itself falls back to the SCSS defaults). When non-empty
-- these emit a :root override in the page <head>, layered after the
-- site-wide override so per-exhibit always wins.

ALTER TABLE exhibits
    ADD COLUMN theme_text_color VARCHAR(20) NOT NULL DEFAULT '',
    ADD COLUMN theme_bg_color   VARCHAR(20) NOT NULL DEFAULT '';
