-- SHA-256 of uploaded bytes, populated at upload time. Used to skip
-- exact-duplicate uploads within the same exhibit (re-uploading the
-- same image won't create a second row or rewrite the file on disk).
--
-- Default '' so existing rows are valid; they just won't dedupe
-- against pre-migration uploads. New uploads populate it going
-- forward.

ALTER TABLE media
    ADD COLUMN sha256 VARCHAR(64) NOT NULL DEFAULT '';

CREATE INDEX media_sha256_idx ON media(sha256) WHERE sha256 <> '';
