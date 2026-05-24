-- OpenExhibit — PostgreSQL schema.

CREATE TABLE sections (
    id           SMALLSERIAL PRIMARY KEY,
    name         VARCHAR(60)  NOT NULL DEFAULT '',
    kind         VARCHAR(50)  NOT NULL DEFAULT 'exhibits',  -- exhibits | xml | tag
    ord          SMALLINT     NOT NULL DEFAULT 0,
    display      SMALLINT     NOT NULL DEFAULT 1,
    hidden       BOOLEAN      NOT NULL DEFAULT FALSE,
    password     VARCHAR(255) NOT NULL DEFAULT '',
    created_at   TIMESTAMPTZ  NULL,
    path         VARCHAR(250) NOT NULL DEFAULT '',
    description  VARCHAR(100) NOT NULL DEFAULT '',
    proj         SMALLINT     NOT NULL DEFAULT 0,
    grp          SMALLINT     NOT NULL DEFAULT 0,
    report       BOOLEAN      NOT NULL DEFAULT FALSE
);
CREATE INDEX sections_path_idx ON sections(path);
CREATE INDEX sections_ord_idx  ON sections(ord);

CREATE TABLE subsections (
    id          SMALLSERIAL PRIMARY KEY,
    section_id  SMALLINT     NOT NULL REFERENCES sections(id) ON DELETE CASCADE,
    title       VARCHAR(255) NOT NULL DEFAULT '',
    folder      VARCHAR(255) NOT NULL DEFAULT '',
    ord         SMALLINT     NOT NULL DEFAULT 0,
    hidden      BOOLEAN      NOT NULL DEFAULT FALSE
);
CREATE INDEX subsections_section_idx ON subsections(section_id);

CREATE TABLE exhibits (
    id                 SERIAL PRIMARY KEY,
    kind               VARCHAR(100) NOT NULL DEFAULT 'exhibits',  -- exhibits | xml | tag
    ref_id             INTEGER      NOT NULL DEFAULT 0,
    title              VARCHAR(255) NOT NULL DEFAULT '',
    content            TEXT         NOT NULL DEFAULT '',
    is_home            BOOLEAN      NOT NULL DEFAULT FALSE,
    link               VARCHAR(255) NOT NULL DEFAULT '',
    link_target        BOOLEAN      NOT NULL DEFAULT FALSE,
    iframe             BOOLEAN      NOT NULL DEFAULT FALSE,
    is_new             BOOLEAN      NOT NULL DEFAULT FALSE,
    tags               VARCHAR(250) NOT NULL DEFAULT '0',
    header             TEXT         NOT NULL DEFAULT '',
    updated_at         TIMESTAMPTZ  NULL,
    published_at       TIMESTAMPTZ  NULL,
    creator            SMALLINT     NOT NULL DEFAULT 0,
    status             SMALLINT     NOT NULL DEFAULT 0,
    process            BOOLEAN      NOT NULL DEFAULT TRUE,
    page_cache         BOOLEAN      NOT NULL DEFAULT FALSE,
    section_id         SMALLINT     NOT NULL DEFAULT 0,
    section_top        BOOLEAN      NOT NULL DEFAULT FALSE,
    section_sub        VARCHAR(255) NOT NULL DEFAULT '',
    subdir             BOOLEAN      NOT NULL DEFAULT FALSE,
    url                VARCHAR(250) NOT NULL DEFAULT '',
    ord                SMALLINT     NOT NULL DEFAULT 999,
    color              VARCHAR(7)   NOT NULL DEFAULT 'ffffff',
    bgimg              VARCHAR(255) NOT NULL DEFAULT '',
    hidden             BOOLEAN      NOT NULL DEFAULT FALSE,
    current_flag       BOOLEAN      NOT NULL DEFAULT FALSE,
    perm               BOOLEAN      NOT NULL DEFAULT FALSE,
    media_source       SMALLINT     NOT NULL DEFAULT 0,
    media_source_detail VARCHAR(255) NOT NULL DEFAULT '',
    images             SMALLINT     NOT NULL DEFAULT 9999,
    thumbs_shape       SMALLINT     NOT NULL DEFAULT 0,
    thumbs             SMALLINT     NOT NULL DEFAULT 200,
    format             VARCHAR(100) NOT NULL DEFAULT 'visual_index',
    thumbs_format      SMALLINT     NOT NULL DEFAULT 0,
    operand            SMALLINT     NOT NULL DEFAULT 0,
    titling            SMALLINT     NOT NULL DEFAULT 0,
    break_count        SMALLINT     NOT NULL DEFAULT 0,
    tiling             BOOLEAN      NOT NULL DEFAULT TRUE,
    year               VARCHAR(4)   NOT NULL DEFAULT '2010',
    report             BOOLEAN      NOT NULL DEFAULT FALSE,
    password           VARCHAR(100) NOT NULL DEFAULT '',
    placement          SMALLINT     NOT NULL DEFAULT 0,
    template           VARCHAR(25)  NOT NULL DEFAULT 'index.php',
    extra              JSONB        NOT NULL DEFAULT '{}'::jsonb
);
CREATE INDEX exhibits_url_idx        ON exhibits(url);
CREATE INDEX exhibits_section_idx    ON exhibits(section_id);
CREATE INDEX exhibits_section_top_ix ON exhibits(section_top);
CREATE INDEX exhibits_status_idx     ON exhibits(status);
CREATE INDEX exhibits_home_idx       ON exhibits(is_home);
CREATE INDEX exhibits_kind_idx       ON exhibits(kind);

CREATE TABLE exhibit_prefs (
    id            SERIAL PRIMARY KEY,
    ref_type      VARCHAR(255) NOT NULL DEFAULT '',
    active        BOOLEAN      NOT NULL DEFAULT TRUE,
    title         VARCHAR(255) NOT NULL DEFAULT '',
    section       SMALLINT     NOT NULL DEFAULT 1,
    template      VARCHAR(50)  NOT NULL DEFAULT '',
    members       VARCHAR(255) NOT NULL DEFAULT '',
    img           VARCHAR(255) NOT NULL DEFAULT '',
    settings      JSONB        NOT NULL DEFAULT '{}'::jsonb,
    grp           VARCHAR(255) NOT NULL DEFAULT ''
);

CREATE TABLE media (
    id            SERIAL PRIMARY KEY,
    ref_id        INTEGER      NOT NULL DEFAULT 0,
    obj_type      VARCHAR(15)  NOT NULL DEFAULT '',
    mime          VARCHAR(15)  NOT NULL DEFAULT '',
    tags          VARCHAR(255) NOT NULL DEFAULT '0',
    file          VARCHAR(255) NOT NULL DEFAULT '',
    thumb         VARCHAR(255) NOT NULL DEFAULT '',
    file_replace  VARCHAR(255) NOT NULL DEFAULT '',
    title         VARCHAR(255) NOT NULL DEFAULT '',
    caption       TEXT         NOT NULL DEFAULT '',
    width         INTEGER      NOT NULL DEFAULT 0,
    height        INTEGER      NOT NULL DEFAULT 0,
    width_resp    INTEGER      NOT NULL DEFAULT 0,
    height_resp   INTEGER      NOT NULL DEFAULT 0,
    bytes         INTEGER      NOT NULL DEFAULT 0,
    updated_at    TIMESTAMPTZ  NULL,
    uploaded_at   TIMESTAMPTZ  NULL,
    ord           SMALLINT     NOT NULL DEFAULT 999,
    hidden        BOOLEAN      NOT NULL DEFAULT FALSE,
    dir           VARCHAR(255) NOT NULL DEFAULT '',
    src           VARCHAR(25)  NOT NULL DEFAULT ''
);
CREATE INDEX media_ref_idx   ON media(ref_id);
CREATE INDEX media_type_idx  ON media(obj_type);
CREATE INDEX media_order_idx ON media(ord);

CREATE TABLE users (
    id            SERIAL PRIMARY KEY,
    userid        VARCHAR(100) NOT NULL UNIQUE,
    password_hash TEXT         NOT NULL,  -- argon2id PHC string (replaces MD5 column)
    email         VARCHAR(100) NOT NULL DEFAULT '',
    threads       SMALLINT     NOT NULL DEFAULT 10,
    writing       BOOLEAN      NOT NULL DEFAULT FALSE,
    offset_       SMALLINT     NOT NULL DEFAULT 0,
    date_format   VARCHAR(30)  NOT NULL DEFAULT '%d %B %Y',
    lang          VARCHAR(8)   NOT NULL DEFAULT 'en-us',
    user_hash     VARCHAR(32)  NOT NULL DEFAULT '',
    help          BOOLEAN      NOT NULL DEFAULT FALSE,
    mode          SMALLINT     NOT NULL DEFAULT 0,
    first_name    VARCHAR(35)  NOT NULL DEFAULT '',
    last_name     VARCHAR(35)  NOT NULL DEFAULT '',
    is_admin      BOOLEAN      NOT NULL DEFAULT FALSE,
    is_active     BOOLEAN      NOT NULL DEFAULT TRUE,
    is_client     BOOLEAN      NOT NULL DEFAULT FALSE,
    img           VARCHAR(255) NOT NULL DEFAULT '',
    reset_token   VARCHAR(64)  NULL,
    reset_expires TIMESTAMPTZ  NULL
);

CREATE TABLE tags (
    id         SERIAL PRIMARY KEY,
    name       VARCHAR(255) NOT NULL UNIQUE,
    grp        SMALLINT     NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ  NULL,
    icon       VARCHAR(255) NOT NULL DEFAULT ''
);

CREATE TABLE tagged (
    id           SERIAL PRIMARY KEY,
    tag_id       INTEGER     NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
    obj_type     VARCHAR(3)  NOT NULL DEFAULT '',  -- 'exh'
    obj_id       INTEGER     NOT NULL,
    UNIQUE (tag_id, obj_type, obj_id)
);
CREATE INDEX tagged_obj_idx ON tagged(obj_type, obj_id);

CREATE TABLE settings (
    id           SMALLSERIAL PRIMARY KEY,
    site_name    VARCHAR(255) NOT NULL DEFAULT '',
    install_date TIMESTAMPTZ  NULL,
    version      VARCHAR(25)  NOT NULL DEFAULT '',
    site_lang    VARCHAR(8)   NOT NULL DEFAULT 'en-us',
    time_format  VARCHAR(25)  NOT NULL DEFAULT '%d %B %Y',
    tagging      BOOLEAN      NOT NULL DEFAULT TRUE,
    help         BOOLEAN      NOT NULL DEFAULT FALSE,
    caching      BOOLEAN      NOT NULL DEFAULT FALSE,
    hibernate    VARCHAR(255) NOT NULL DEFAULT '',
    obj_name     VARCHAR(255) NOT NULL DEFAULT '',
    obj_theme    VARCHAR(50)  NOT NULL DEFAULT 'default',
    obj_itop     TEXT         NOT NULL DEFAULT '',
    obj_ibot     TEXT         NOT NULL DEFAULT '',
    obj_org      BOOLEAN      NOT NULL DEFAULT TRUE,
    obj_apikey   VARCHAR(64)  NOT NULL DEFAULT '',
    site_format  VARCHAR(30)  NOT NULL DEFAULT '%d %B %Y',
    site_offset  SMALLINT     NOT NULL DEFAULT 0,
    site_vars    JSONB        NOT NULL DEFAULT '{}'::jsonb
);

CREATE TABLE abstracts (
    id      SERIAL PRIMARY KEY,
    obj     VARCHAR(32)  NOT NULL DEFAULT '',
    obj_id  INTEGER      NOT NULL DEFAULT 0,
    var     VARCHAR(255) NOT NULL DEFAULT '',
    val     TEXT         NOT NULL DEFAULT ''
);
CREATE INDEX abstracts_obj_idx ON abstracts(obj, obj_id);

CREATE TABLE plugins (
    id            SERIAL PRIMARY KEY,
    is_primary    BOOLEAN      NOT NULL DEFAULT FALSE,
    plugin_type   VARCHAR(15)  NOT NULL DEFAULT '',
    name          VARCHAR(255) NOT NULL DEFAULT '',
    uri           VARCHAR(255) NOT NULL DEFAULT '',
    version       VARCHAR(20)  NOT NULL DEFAULT '',
    file          VARCHAR(255) NOT NULL DEFAULT '',
    function_name VARCHAR(255) NOT NULL DEFAULT '',
    hook          VARCHAR(255) NOT NULL DEFAULT '',
    space         VARCHAR(100) NOT NULL DEFAULT '',
    creator       VARCHAR(50)  NOT NULL DEFAULT '',
    www           VARCHAR(255) NOT NULL DEFAULT '',
    description   TEXT         NOT NULL DEFAULT '',
    options       JSONB        NOT NULL DEFAULT '{}'::jsonb,
    options_build TEXT         NOT NULL DEFAULT '',
    usage_text    VARCHAR(255) NOT NULL DEFAULT '',
    usage_desc    VARCHAR(255) NOT NULL DEFAULT '',
    ord           SMALLINT     NOT NULL DEFAULT 100
);

-- Phase 2 tables: created empty for schema completeness, not written to yet.

CREATE TABLE stats (
    id         BIGSERIAL PRIMARY KEY,
    addr       INET         NULL,
    country    VARCHAR(30)  NOT NULL DEFAULT '',
    lang       VARCHAR(10)  NOT NULL DEFAULT '',
    domain     VARCHAR(100) NOT NULL DEFAULT '',
    referrer   VARCHAR(255) NOT NULL DEFAULT '',
    page       VARCHAR(255) NOT NULL DEFAULT '',
    agent      VARCHAR(255) NOT NULL DEFAULT '',
    keyword    VARCHAR(255) NOT NULL DEFAULT '',
    os         VARCHAR(20)  NOT NULL DEFAULT '',
    browser    VARCHAR(20)  NOT NULL DEFAULT '',
    hit_at     TIMESTAMPTZ  NOT NULL DEFAULT now(),
    hit_month  VARCHAR(7)   NOT NULL DEFAULT '',
    hit_day    DATE         NULL
);
CREATE INDEX stats_hit_day_idx ON stats(hit_day);

CREATE TABLE stats_exhibits (
    url   VARCHAR(255) PRIMARY KEY,
    count INTEGER      NOT NULL DEFAULT 0
);

CREATE TABLE stats_storage (
    month     VARCHAR(7) PRIMARY KEY,  -- 'YYYY-MM'
    hits      INTEGER NOT NULL DEFAULT 0,
    uniques   INTEGER NOT NULL DEFAULT 0,
    referrers INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE iptocountry (
    ip_from        BIGINT      NOT NULL,
    ip_to          BIGINT      NOT NULL,
    country_code2  CHAR(2)     NOT NULL DEFAULT '',
    country_code3  CHAR(3)     NOT NULL DEFAULT '',
    country_name   VARCHAR(50) NOT NULL DEFAULT '',
    PRIMARY KEY (ip_from, ip_to)
);
