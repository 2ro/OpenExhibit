-- Lets the admin group exhibits silently — the section's children render
-- as a bare list with no heading. Defaults to FALSE so existing sections
-- keep their visible titles.

ALTER TABLE sections
    ADD COLUMN hide_title BOOLEAN NOT NULL DEFAULT FALSE;
