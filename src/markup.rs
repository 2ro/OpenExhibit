// Public-side content renderer for everything an admin types into a
// long-form textarea (exhibit `content`, media `caption`, sidebar
// `obj_itop` / `obj_ibot`).
//
// Pipeline:
//   1. BBCode pre-pass — common forum tags converted to inline HTML
//      (so they survive CommonMark's HTML-block heuristics).
//   2. pulldown-cmark — Markdown to HTML, including embedded HTML.
//   3. ammonia — sanitize. Scripts, event handlers, javascript: URLs,
//      <iframe>, <object>, etc. get stripped. Image / link / heading /
//      list / formatting tags survive.
//
// Authoring rule of thumb: write Markdown for prose, BBCode for forum
// muscle-memory, drop in raw HTML for anything fancier. Anything that
// CSP would block also gets stripped here as defense-in-depth.

use std::sync::OnceLock;

use ammonia::Builder;
use pulldown_cmark::{html, Options, Parser};
use regex::Regex;

/// Per-render switches. Defaults are equivalent to plain Markdown +
/// `BBCode` + sanitized HTML.
#[derive(Default, Clone, Copy, Debug)]
pub struct RenderOptions {
    /// 4chan-style greentext: any line starting with `>` becomes a
    /// colored quote line (`<p class="greentext">`) instead of being
    /// parsed as a Markdown blockquote.
    pub greentext: bool,
}

/// Convert admin-authored text to safe HTML for public rendering.
/// Empty input round-trips to empty output (cheap fast-path).
#[must_use]
pub fn render_with(input: &str, opts: RenderOptions) -> String {
    if input.trim().is_empty() {
        return String::new();
    }

    // Greentext pre-pass intercepts `>` lines before Markdown sees them,
    // so they don't become blockquotes.
    let stage1 = if opts.greentext {
        apply_greentext(input)
    } else {
        input.to_string()
    };

    let bb = bbcode_to_html(&stage1);

    let mut md_opts = Options::empty();
    md_opts.insert(Options::ENABLE_TABLES);
    md_opts.insert(Options::ENABLE_STRIKETHROUGH);
    md_opts.insert(Options::ENABLE_TASKLISTS);
    md_opts.insert(Options::ENABLE_SMART_PUNCTUATION);
    let parser = Parser::new_ext(&bb, md_opts);

    let mut html_out = String::with_capacity(bb.len());
    html::push_html(&mut html_out, parser);

    sanitizer().clean(&html_out).to_string()
}

/// Wrap each `>` line in `<p class="greentext">`. The `>` becomes
/// `&gt;`, the content is HTML-escaped, blank lines pass through
/// unmodified so paragraph breaks survive.
fn apply_greentext(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 32);
    let mut first = true;
    for line in input.lines() {
        if !first {
            out.push('\n');
        }
        first = false;
        let stripped = line.trim_start();
        if let Some(rest) = stripped.strip_prefix('>') {
            // Heuristic: a literal `>>` is a 4chan-style reply — keep
            // both arrows. Single `>` is normal greentext.
            let arrows_and_rest = if let Some(after) = rest.strip_prefix('>') {
                format!(">>{after}")
            } else {
                rest.to_string()
            };
            out.push_str("\n\n<p class=\"greentext\">");
            // Render the original `>` arrows literally, then the body.
            // Escape `<` / `>` / `&` to keep the sanitizer's parser happy.
            let arrows = if stripped.starts_with(">>") {
                ">>"
            } else {
                ">"
            };
            out.push_str(arrows);
            out.push(' ');
            out.push_str(&html_escape(
                arrows_and_rest.trim_start_matches('>').trim_start(),
            ));
            out.push_str("</p>\n\n");
        } else {
            out.push_str(line);
        }
    }
    out
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

/// Cached ammonia sanitizer with our allowlist. Built once per process.
fn sanitizer() -> &'static ammonia::Builder<'static> {
    static SANITIZER: OnceLock<Builder<'static>> = OnceLock::new();
    SANITIZER.get_or_init(|| {
        let mut b = Builder::default();
        // Allow common rich-text tags we want admins to be able to use.
        b.add_tags(["s", "u", "mark", "kbd", "sub", "sup", "del", "ins"]);
        b.add_tag_attributes("a", ["title"]);
        b.add_tag_attributes("img", ["loading", "title"]);
        // class="" on a handful of containers lets admins reach the
        // public stylesheet's existing utility classes (.highlight etc),
        // and lets pulldown-cmark's fenced-code-block `class="language-x"`
        // hint survive on <code>/<pre> for future highlighting.
        b.add_tag_attributes("span", ["class"]);
        b.add_tag_attributes("div", ["class"]);
        b.add_tag_attributes("p", ["class"]);
        b.add_tag_attributes("code", ["class"]);
        b.add_tag_attributes("pre", ["class"]);
        b
    })
}

/// Convert a small set of `BBCode` tags to inline HTML. Intentionally
/// minimal: covers what most forum users type by reflex, leaves
/// everything else to Markdown.
fn bbcode_to_html(input: &str) -> String {
    static URL_NAMED: OnceLock<Regex> = OnceLock::new();
    static URL_BARE: OnceLock<Regex> = OnceLock::new();
    static IMG: OnceLock<Regex> = OnceLock::new();
    static QUOTE: OnceLock<Regex> = OnceLock::new();
    static CODE_INLINE: OnceLock<Regex> = OnceLock::new();
    static SIMPLE: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();

    let named =
        URL_NAMED.get_or_init(|| Regex::new(r"(?is)\[url=([^\]]+)\](.*?)\[/url\]").unwrap());
    let bare = URL_BARE.get_or_init(|| Regex::new(r"(?is)\[url\](.*?)\[/url\]").unwrap());
    let img = IMG.get_or_init(|| Regex::new(r"(?is)\[img\](.*?)\[/img\]").unwrap());
    let quote = QUOTE.get_or_init(|| Regex::new(r"(?is)\[quote\](.*?)\[/quote\]").unwrap());
    let code = CODE_INLINE.get_or_init(|| Regex::new(r"(?is)\[code\](.*?)\[/code\]").unwrap());

    let simple = SIMPLE.get_or_init(|| {
        vec![
            (
                Regex::new(r"(?is)\[b\](.*?)\[/b\]").unwrap(),
                "<strong>$1</strong>",
            ),
            (Regex::new(r"(?is)\[i\](.*?)\[/i\]").unwrap(), "<em>$1</em>"),
            (Regex::new(r"(?is)\[u\](.*?)\[/u\]").unwrap(), "<u>$1</u>"),
            (
                Regex::new(r"(?is)\[s\](.*?)\[/s\]").unwrap(),
                "<del>$1</del>",
            ),
            // [hr] standalone.
            (Regex::new(r"(?i)\[hr\]").unwrap(), "<hr>"),
        ]
    });

    let mut out = input.to_string();

    // URL with explicit text wins over bare URL.
    out = named
        .replace_all(&out, r#"<a href="$1">$2</a>"#)
        .into_owned();
    out = bare
        .replace_all(&out, r#"<a href="$1">$1</a>"#)
        .into_owned();
    out = img
        .replace_all(&out, r#"<img src="$1" alt="">"#)
        .into_owned();
    out = quote
        .replace_all(&out, "<blockquote>$1</blockquote>")
        .into_owned();
    out = code.replace_all(&out, "<code>$1</code>").into_owned();

    for (re, repl) in simple {
        out = re.replace_all(&out, *repl).into_owned();
    }

    // [list]…[*]item…[/list] — translate to a bulleted list. Match
    // [list=1] / [list=a] as ordered too.
    out = convert_lists(&out);

    out
}

/// Translate [list]…[*]item…[*]item…[/list] blocks to <ul>/<ol>.
fn convert_lists(input: &str) -> String {
    static OPEN: OnceLock<Regex> = OnceLock::new();
    let re = OPEN.get_or_init(|| Regex::new(r"(?is)\[list(=([^\]]*))?\](.*?)\[/list\]").unwrap());
    re.replace_all(input, |caps: &regex::Captures| {
        let ordered = caps
            .get(2)
            .is_some_and(|m| matches!(m.as_str(), "1" | "a" | "A" | "i" | "I"));
        let body = caps.get(3).map_or("", |m| m.as_str());
        let items: Vec<&str> = body
            .split("[*]")
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();
        let tag = if ordered { "ol" } else { "ul" };
        let mut out = String::with_capacity(body.len() + 32);
        out.push('<');
        out.push_str(tag);
        out.push('>');
        for it in items {
            out.push_str("<li>");
            out.push_str(it);
            out.push_str("</li>");
        }
        out.push_str("</");
        out.push_str(tag);
        out.push('>');
        out
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_is_empty() {
        assert_eq!(render_with("", RenderOptions::default()), "");
        assert_eq!(render_with("   \n\t  ", RenderOptions::default()), "");
    }

    #[test]
    fn plain_markdown() {
        let out = render_with("Hello **world**.", RenderOptions::default());
        assert!(out.contains("<strong>world</strong>"), "got: {out}");
    }

    #[test]
    fn bbcode_bold_italic() {
        let out = render_with("[b]hi[/b] [i]there[/i]", RenderOptions::default());
        assert!(out.contains("<strong>hi</strong>"), "got: {out}");
        assert!(out.contains("<em>there</em>"), "got: {out}");
    }

    #[test]
    fn bbcode_url_named() {
        let out = render_with(
            "[url=https://example.com]click[/url]",
            RenderOptions::default(),
        );
        assert!(
            out.contains(r#"<a href="https://example.com""#),
            "got: {out}"
        );
        assert!(out.contains(">click</a>"), "got: {out}");
    }

    #[test]
    fn bbcode_img() {
        let out = render_with(
            "[img]https://example.com/x.jpg[/img]",
            RenderOptions::default(),
        );
        assert!(
            out.contains(r#"<img src="https://example.com/x.jpg""#),
            "got: {out}"
        );
    }

    #[test]
    fn bbcode_quote() {
        let out = render_with("[quote]said so[/quote]", RenderOptions::default());
        assert!(out.contains("<blockquote>"), "got: {out}");
        assert!(out.contains("said so"), "got: {out}");
    }

    #[test]
    fn bbcode_list_unordered() {
        let out = render_with("[list][*]one[*]two[/list]", RenderOptions::default());
        assert!(out.contains("<ul>"), "got: {out}");
        assert!(out.contains("<li>one</li>"), "got: {out}");
        assert!(out.contains("<li>two</li>"), "got: {out}");
    }

    #[test]
    fn bbcode_list_ordered() {
        let out = render_with("[list=1][*]a[*]b[/list]", RenderOptions::default());
        assert!(out.contains("<ol>"), "got: {out}");
    }

    #[test]
    fn raw_html_passes_through() {
        let out = render_with(
            "<p>hello <strong>html</strong></p>",
            RenderOptions::default(),
        );
        assert!(out.contains("<strong>html</strong>"), "got: {out}");
    }

    #[test]
    fn script_is_stripped() {
        let out = render_with(
            "<p>safe</p><script>alert(1)</script>",
            RenderOptions::default(),
        );
        assert!(!out.contains("script"), "script leaked through: {out}");
        assert!(out.contains("safe"), "got: {out}");
    }

    #[test]
    fn javascript_url_is_stripped() {
        let out = render_with(
            r#"<a href="javascript:alert(1)">x</a>"#,
            RenderOptions::default(),
        );
        assert!(!out.contains("javascript:"), "got: {out}");
    }

    #[test]
    fn onclick_is_stripped() {
        let out = render_with(
            r#"<a href="/" onclick="alert(1)">x</a>"#,
            RenderOptions::default(),
        );
        assert!(!out.contains("onclick"), "got: {out}");
    }

    #[test]
    fn img_event_handler_stripped() {
        let out = render_with(
            r#"<img src="x" onerror="alert(1)">"#,
            RenderOptions::default(),
        );
        assert!(!out.contains("onerror"), "got: {out}");
    }

    #[test]
    fn mixed_markdown_and_bbcode() {
        let out = render_with(
            "# Title\n\n[b]bold[/b] and *italic*",
            RenderOptions::default(),
        );
        assert!(out.contains("<h1>"), "got: {out}");
        assert!(out.contains("<strong>bold</strong>"), "got: {out}");
        assert!(out.contains("<em>italic</em>"), "got: {out}");
    }

    #[test]
    fn default_quote_is_blockquote() {
        let out = render_with("> said it", RenderOptions::default());
        assert!(out.contains("<blockquote>"), "got: {out}");
        assert!(out.contains("said it"), "got: {out}");
    }

    #[test]
    fn greentext_off_uses_blockquote() {
        let opts = RenderOptions { greentext: false };
        let out = render_with("> said it", opts);
        assert!(out.contains("<blockquote>"), "got: {out}");
    }

    #[test]
    fn greentext_on_uses_p_class() {
        let opts = RenderOptions { greentext: true };
        let out = render_with("> implying", opts);
        assert!(out.contains(r#"<p class="greentext">"#), "got: {out}");
        assert!(!out.contains("<blockquote>"), "got: {out}");
        assert!(out.contains("&gt;"), "got: {out}");
        assert!(out.contains("implying"), "got: {out}");
    }

    #[test]
    fn greentext_passes_through_non_quote_lines() {
        let opts = RenderOptions { greentext: true };
        let out = render_with("hello\n> quote\nworld", opts);
        assert!(out.contains("hello"), "got: {out}");
        assert!(out.contains("world"), "got: {out}");
        assert!(out.contains(r#"<p class="greentext">"#), "got: {out}");
    }

    #[test]
    fn greentext_strips_dangerous_html() {
        let opts = RenderOptions { greentext: true };
        let out = render_with("> <script>alert(1)</script>", opts);
        assert!(!out.contains("<script"), "got: {out}");
    }

    #[test]
    fn fenced_code_block_renders() {
        let out = render_with("```\nlet x = 1;\nlet y = 2;\n```", RenderOptions::default());
        assert!(out.contains("<pre>"), "got: {out}");
        assert!(out.contains("<code"), "got: {out}");
        assert!(out.contains("let x = 1;"), "got: {out}");
    }

    #[test]
    fn fenced_code_block_with_language() {
        let out = render_with("```rust\nfn main() {}\n```", RenderOptions::default());
        // pulldown-cmark emits class="language-rust" on the <code>; the
        // sanitizer must let it through for future highlighter hooks.
        assert!(out.contains(r#"class="language-rust""#), "got: {out}");
        assert!(out.contains("fn main()"), "got: {out}");
    }
}
