-- SMTP relay configuration moved from env vars to the admin settings row.
-- Empty smtp_host means "no SMTP configured" — mails are written to the log.

ALTER TABLE settings
    ADD COLUMN smtp_host VARCHAR(255) NOT NULL DEFAULT '',
    ADD COLUMN smtp_port INTEGER      NOT NULL DEFAULT 587,
    ADD COLUMN smtp_user VARCHAR(255) NOT NULL DEFAULT '',
    ADD COLUMN smtp_pass VARCHAR(255) NOT NULL DEFAULT '',
    ADD COLUMN smtp_from VARCHAR(255) NOT NULL DEFAULT '';
