-- Initial seed: the minimum the application needs to render and be navigable.
-- The admin user is *not* seeded here — it's created by application code on first
-- boot so the password can be hashed with argon2 and printed to stdout once.

INSERT INTO sections (id, name, kind, ord, display, hidden, path, description, proj, grp, report) VALUES
    (1, 'main',    'exhibits', 1, 1, FALSE, '/',     'Main',     0, 0, FALSE),
    (2, 'work',    'exhibits', 2, 1, FALSE, '/work', 'Work',     0, 0, FALSE),
    (3, 'tag',     'exhibits', 9, 1, TRUE,  '/tag',  'Tags',     3, 0, FALSE);
SELECT setval(pg_get_serial_sequence('sections', 'id'), 3);

INSERT INTO exhibits (
    id, kind, ref_id, title, content, is_home, status, process, section_id, section_top,
    url, ord, color, images, thumbs, format, tiling, year, template
) VALUES
    (1, 'exhibits', 1, 'Welcome', '<p>Edit this exhibit from the admin to get started.</p>',
       TRUE,  1, TRUE, 1, TRUE, '/',      0, 'ffffff', 600,  200, 'visual_index', TRUE, '2026', 'index.php'),
    (2, 'exhibits', 2, 'Work',    '',
       FALSE, 1, TRUE, 2, TRUE, '/work/', 0, 'ffffff', 9999, 200, 'visual_index', TRUE, '2026', 'index.php'),
    (3, 'exhibits', 3, 'Tags',    '',
       FALSE, 0, TRUE, 3, TRUE, '/tag/',  0, 'ffffff', 9999, 200, 'visual_index', TRUE, '2026', 'index.php');
SELECT setval(pg_get_serial_sequence('exhibits', 'id'), 3);

INSERT INTO exhibit_prefs (id, ref_type, active, section, settings) VALUES
    (1, 'exhibits', TRUE, 1, '{}'::jsonb),
    (2, 'xml',      TRUE, 1, '{}'::jsonb),
    (3, 'tag',      TRUE, 1, '{"format":"tag_display","thumbs":200}'::jsonb);
SELECT setval(pg_get_serial_sequence('exhibit_prefs', 'id'), 3);

INSERT INTO settings (
    id, site_name, install_date, version, site_lang, time_format, tagging, help, caching,
    obj_name, obj_theme,
    obj_itop, obj_ibot,
    obj_org, obj_apikey, site_format, site_offset, site_vars
) VALUES (
    1, 'My Portfolio', now(), '0.1.0', 'en-us', '%d %B %Y', TRUE, FALSE, FALSE,
    'My Portfolio', 'default',
    '<h1><a href="/" title="{{ obj_name }}">{{ obj_name }}</a></h1>',
    '',
    TRUE, '', '%d %B %Y', 0,
    '{"passwords":true,"templates":false,"tags":true}'::jsonb
);
SELECT setval(pg_get_serial_sequence('settings', 'id'), 1);
