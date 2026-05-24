-- Two color knobs surfaced as HTML5 color pickers on /admin/settings.
-- Both default to '' (empty) so existing installs keep the SCSS defaults
-- — black ink on white paper — until the admin picks something else.
-- When non-empty the layout emits a :root override block before any
-- custom-CSS textarea, so the picker is a convenient shortcut and the
-- textarea remains the escape hatch for everything else.

ALTER TABLE settings
    ADD COLUMN theme_text_color VARCHAR(20) NOT NULL DEFAULT '',
    ADD COLUMN theme_bg_color   VARCHAR(20) NOT NULL DEFAULT '';
