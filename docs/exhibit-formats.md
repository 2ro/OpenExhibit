# Exhibit formats

OpenExhibit's "exhibit format" is the type of an exhibit. Each format
owns its own:

- **Rust code** — the render function plus optional visit interceptor and
  nav-link override.
- **Template** — its own Askama file under `templates/public/formats/`.
- **Styles** — its own SCSS partial under `scss/public/` (`build.rs`
  picks up anything in there at `cargo build` time).
- **Admin-form fields** — declared via `FormatCapabilities`; the
  exhibit edit form hides inputs your format doesn't use.

Adding a new format is a single-file Rust module, a single template,
and one line in the registry.

> Format descriptions in the table below are auto-checked against
> `src/formats/<key>.rs` by `scripts/gen-docs.sh`.

## What ships

| Key             | Display name      | Description                                                                  |
|-----------------|-------------------|------------------------------------------------------------------------------|
| `visual_index`  | Visual index      | Grid of thumbnails with a lightbox. Default for most exhibits.               |
| `slideshow`     | Slideshow         | One slide at a time, paginated. Good for sequential narratives.              |
| `horizontal`    | Horizontal strip  | Side-scrolling row of full-height images.                                    |
| `no_show`       | Text only         | Just the exhibit's HTML content, no media gallery.                           |
| `no_thumbs`     | No thumbnails     | Media stacked vertically at full size — no thumbnail grid.                   |
| `over_and_over` | Over and over     | Vertically stacked images that loop on long scrolls.                         |
| `random_image`  | Random image      | Shows a single random image from the gallery, picked per page load.          |
| `thickbox`      | Thickbox          | Classic thumbnail-to-lightbox layout.                                        |
| `documenta`     | Documenta         | Tight grid with hover captions, in the spirit of catalogue layouts.          |
| `tag_display`   | Tag display       | Used by the synthetic `/tag/` pages — rarely chosen directly.                |
| `external_link` | External link     | Nav-only entry — clicking sends the visitor to an external URL.              |

## Adding a format

### 1. Write the module

Create `src/formats/my_format.rs`:

```rust
use askama::Template;

use crate::error::AppResult;
use crate::models::exhibit::Exhibit;

use super::{BaseFields, ExhibitFormat, FormatCapabilities, MediaView};

#[derive(Template)]
#[template(path = "public/formats/my_format.html")]
struct Page {
    base: BaseFields,
    content: String,
    media: Vec<MediaView>,
    // …any extra fields your template wants
}

pub struct Format;

impl ExhibitFormat for Format {
    fn key(&self) -> &'static str { "my_format" }
    fn display_name(&self) -> &'static str { "My format" }
    fn description(&self) -> &'static str {
        "One-line description shown in the admin format picker."
    }

    // Override only the capabilities that differ from the default.
    fn capabilities(&self) -> FormatCapabilities {
        FormatCapabilities {
            uses_media: true,
            uses_content: true,
            uses_external_link: false,
            uses_password: true,
            uses_thumbs_size: true,
            requires_url_slug: true,
        }
    }

    fn render(
        &self,
        _exhibit: &Exhibit,
        content: String,
        media: Vec<MediaView>,
        base: BaseFields,
    ) -> AppResult<String> {
        Ok(Page {
            base,
            content,
            media,
        }
        .render()?)
    }
}
```

`content` is the exhibit's `content` column already passed through
the markup pipeline (Markdown + BBCode + sanitized HTML, with the
greentext toggle applied). Slot it into the template as `{{ content|safe }}`.

### 2. Write the template

`templates/public/formats/my_format.html`:

```html
{% extends "public/layout.html" %}
{% block exhibit_body %}
  <div class="text">{{ content|safe }}</div>
  {%- for m in media %}
    <img src="{{ m.file_url }}" alt="{{ m.title }}">
  {%- endfor %}
{% endblock %}
```

### 3. (Optional) Add styles

Drop `scss/public/_my_format.scss` and `@import "my_format";` from
`scss/public/main.scss`. `build.rs` recompiles on any change.

### 4. Register

`src/formats/mod.rs`:

```rust
pub mod my_format;     // 1. import the module
// …
static FORMATS: &[&dyn ExhibitFormat] = &[
    // …existing…
    &my_format::Format, // 2. add to the registry
];
```

That's it. `cargo build` picks it up; the admin "New exhibit" picker
shows your format automatically with its display name and description.

## Extending behavior beyond layout

### Intercept a visit

`fn intercept(&self, exhibit: &Exhibit) -> Option<HttpResponse>`

Return `Some(response)` to short-circuit the request before any render
happens. `external_link` uses this to issue a 302 (with
`Cache-Control: no-store` so the redirect target stays editable).
Default: `None` (falls through to `render`).

### Override the nav anchor

`fn nav_href(&self, exhibit: &Exhibit) -> NavHref`

Return `(href, open_in_new_tab)`. `external_link` returns the exhibit's
`link` column instead of its internal URL (and honors `link_target`
for the new-tab flag). Default: the internal URL, same tab.

### Declare admin capabilities

The `FormatCapabilities` flags toggle whole fieldsets in the exhibit
edit form. Defaults to "behaves like a normal media exhibit" — most
formats only need to override the bits that differ.

| Field                | Default | Hides when false                            |
|----------------------|---------|---------------------------------------------|
| `uses_media`         | `true`  | "Manage media" link                         |
| `uses_content`       | `true`  | HTML content textarea                       |
| `uses_external_link` | `false` | External-URL fieldset (shown when `true`)   |
| `uses_password`      | `true`  | Password-protection input                   |
| `uses_thumbs_size`   | `true`  | Thumbnail-size input                        |
| `requires_url_slug`  | `true`  | URL-slug input. `external_link` sets this `false`: the slug is irrelevant (nav points at the external URL, direct visits 302 away) so the create-flow auto-generates a `/<key>-<random>/` slug and hides the input. |

To add a new capability, edit `FormatCapabilities` and add the matching
`{% if caps.foo %}…{% endif %}` block in
`templates/admin/exhibits/edit.html`.

## Database considerations

The `exhibits` table is a wide row that carries every column any format
might ever want (legacy from Indexhibit). New formats should reuse these
columns when possible (`link` / `link_target` / `extra` / `bgimg` / …).
Adding a column means a migration — see `migrations/000N_*.sql` for
examples, and [`database-schema.md`](database-schema.md) for the full
column inventory.

Unused columns stay untouched on save, so coexistence is fine.

## Tests

Each registry change is covered by `src/formats/mod.rs::tests`:

- All keys are unique.
- `visual_index` is present (it's the fallback for unknown keys).
- Unknown keys resolve to `visual_index`.
- Every registered format has a non-empty `display_name` and
  `description`.

Run with `cargo test`.
