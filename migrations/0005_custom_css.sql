-- Per-exhibit and site-wide custom CSS, rendered inline in <head> on the
-- public site. Lets the operator override `:root { --color: …; }` or any
-- other style without forking the SCSS bundle. Both default to '' so
-- behavior is unchanged unless the admin fills them in.

ALTER TABLE exhibits
    ADD COLUMN custom_css TEXT NOT NULL DEFAULT '';

ALTER TABLE settings
    ADD COLUMN custom_css TEXT NOT NULL DEFAULT '';
