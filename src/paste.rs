//! Text pasted from elsewhere with its formatting. Word, LibreOffice, Google Docs and web pages put
//! the text on the clipboard twice: plain, and as HTML; Markdown is plain text the writer asks to
//! have read as such (Insert › Paste as Markdown). Both are read here into the few things Caprice
//! has: paragraphs that are ordinary text, chapter titles (every heading) or list items, with their
//! alignment, and letters that are bold, italic, underlined or coloured. `App::paste_formatted`
//! puts them into the story.
//!
//! What Caprice has no place for keeps its words only: links, code, quotes, struck-out text, and
//! tables (a paragraph a row, the cells apart by tabs). A rule between sections becomes a centred
//! "* * *", as a writer marks a break. The font and size are not taken: what is pasted is written
//! in the story's own, as typing would be, so a paste from a web page does not bring Arial into a
//! novel. Nor are colours the page could not show: ink near black is the story's ink, and colours
//! too pale to read on white paper (a code editor's dark theme) are dropped.

use crate::model::{Align, CONTENTS_CHAR, IMAGE_CHAR, ListKind, PAGE_BREAK, ParaAttrs, Style};

/// How letters look, as far as Caprice keeps it.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Look {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub color: Option<[u8; 3]>,
}

/// What a paragraph is.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub enum Block {
    #[default]
    Body,
    /// Any heading, of whatever level: a chapter title.
    Heading,
    Bullet,
    Numbered,
}

/// One paragraph read: what it is, its alignment, and its text in runs of one look.
#[derive(Clone, Default, PartialEq, Debug)]
pub struct Para {
    pub block: Block,
    pub align: Align,
    pub runs: Vec<(String, Look)>,
}

impl Para {
    pub fn text(&self) -> String {
        self.runs.iter().map(|(t, _)| t.as_str()).collect()
    }

    /// Its format in the story. The line spacing is the one where it is pasted: sources set their
    /// own everywhere (Word 107%, Google Docs 1.38), which is nothing the writer chose.
    pub fn attrs(&self, spacing: f32) -> ParaAttrs {
        let list = match self.block {
            Block::Bullet => ListKind::Bullet,
            Block::Numbered => ListKind::Numbered,
            Block::Body | Block::Heading => ListKind::None,
        };
        let mut attrs = ParaAttrs { align: self.align, spacing, list, ..ParaAttrs::default() };
        if self.block == Block::Heading {
            attrs.set_chapter_title(true);
        }
        attrs
    }

    /// The style of its letters in `look`: the font and size typing would have. A chapter title is
    /// drawn bold and larger on top of its letters' own look, so its letters are not made bold too:
    /// taking the title off then gives plain text back.
    pub fn letters(&self, typing: &Style, look: Look) -> Style {
        let bold = look.bold && self.block != Block::Heading;
        let (italic, underline, color) = (look.italic, look.underline, look.color);
        Style { bold, italic, underline, color, image: 0, para: ParaAttrs::default(), ..typing.clone() }
    }
}

/// Paragraphs as they are read: text is added to the paragraph being written, which takes its
/// format from where its first letter was found.
#[derive(Default)]
struct Writer {
    paras: Vec<Para>,
    cur: Para,
    /// The paragraph being written has had something in it (a letter, or a space that holds, `&nbsp;`).
    started: bool,
    /// A space was read that shows only if more text follows in the paragraph (HTML's white space),
    /// in the look of the text before it.
    space: Option<Look>,
}

impl Writer {
    fn text(&mut self, text: &str, look: Look, block: Block, align: Align) {
        for c in text.chars() {
            // Page breaks are structure, and pictures and the old contents are Caprice's own (R8).
            if c == PAGE_BREAK || c == IMAGE_CHAR || c == CONTENTS_CHAR || (c.is_control() && c != '\t') {
                continue;
            }
            if !self.started {
                self.started = true;
                (self.cur.block, self.cur.align) = (block, align);
            } else if let Some(before) = self.space.take() {
                self.push(' ', before);
            }
            self.push(if c == '\u{a0}' { ' ' } else { c }, look);
        }
    }

    fn push(&mut self, c: char, look: Look) {
        match self.cur.runs.last_mut() {
            Some((run, l)) if *l == look => run.push(c),
            _ => self.cur.runs.push((c.to_string(), look)),
        }
    }

    /// A space that shows only between words.
    fn soft_space(&mut self, look: Look) {
        if self.started && self.space.is_none() {
            self.space = Some(look);
        }
    }

    /// The paragraph ends, if it had anything in it. Only spaces is an empty line.
    fn end(&mut self) {
        if self.started {
            let mut para = std::mem::take(&mut self.cur);
            if para.runs.iter().all(|(t, _)| t.trim().is_empty()) {
                para.runs.clear();
            }
            self.paras.push(para);
        }
        (self.started, self.space) = (false, None);
    }

    /// A paragraph ends here even if it is empty (a line break in an empty paragraph).
    fn end_line(&mut self, block: Block, align: Align) {
        if !self.started {
            (self.started, self.cur.block, self.cur.align) = (true, block, align);
        }
        self.end();
    }

    /// A break between sections.
    fn rule(&mut self) {
        self.end();
        self.text("* * *", Look::default(), Block::Body, Align::Center);
        self.end();
    }

    /// The paragraphs, without empty ones at the start or the end of what was copied.
    fn finish(mut self) -> Vec<Para> {
        self.end();
        let mut paras = self.paras;
        while paras.last().is_some_and(|p| p.runs.is_empty()) {
            paras.pop();
        }
        let first = paras.iter().position(|p| !p.runs.is_empty()).unwrap_or(paras.len());
        paras.drain(..first);
        paras
    }
}

// ------------------------------------------------------------------ Markdown

/// Markdown read into paragraphs (CommonMark, with tables, task lists and struck-out text).
pub fn from_markdown(md: &str) -> Vec<Para> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
    let options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
    let mut w = Writer::default();
    let (mut bold, mut italic, mut underline) = (0u32, 0u32, 0u32);
    let mut lists: Vec<Block> = Vec::new();
    let (mut heading, mut code, mut picture) = (false, false, 0u32);
    for event in Parser::new_ext(md, options) {
        let look = Look { bold: bold > 0, italic: italic > 0, underline: underline > 0, color: None };
        let block = if heading { Block::Heading } else { lists.last().copied().unwrap_or_default() };
        match event {
            Event::Start(Tag::Heading { .. }) => {
                w.end();
                heading = true;
            }
            Event::End(TagEnd::Heading(_)) => {
                w.end();
                heading = false;
            }
            Event::Start(Tag::List(first)) => {
                w.end();
                lists.push(if first.is_some() { Block::Numbered } else { Block::Bullet });
            }
            Event::End(TagEnd::List(_)) => {
                w.end();
                lists.pop();
            }
            Event::Start(Tag::CodeBlock(_)) => {
                w.end();
                code = true;
            }
            Event::End(TagEnd::CodeBlock) => {
                w.end();
                code = false;
            }
            Event::Start(Tag::Paragraph | Tag::Item | Tag::BlockQuote(_) | Tag::TableHead | Tag::TableRow)
            | Event::End(TagEnd::Paragraph | TagEnd::Item | TagEnd::BlockQuote(_) | TagEnd::TableHead | TagEnd::TableRow)
            | Event::HardBreak => w.end(),
            Event::Start(Tag::TableCell) if w.started => w.text("\t", look, block, Align::Left),
            Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Strong) => bold = bold.saturating_sub(1),
            Event::Start(Tag::Emphasis) => italic += 1,
            Event::End(TagEnd::Emphasis) => italic = italic.saturating_sub(1),
            // A picture's words are its description, not the story's.
            Event::Start(Tag::Image { .. }) => picture += 1,
            Event::End(TagEnd::Image) => picture = picture.saturating_sub(1),
            Event::Text(_) | Event::Code(_) if picture > 0 => {}
            Event::Text(t) if code => {
                // Each line of code is a paragraph, empty ones too.
                let mut lines = t.split('\n').peekable();
                while let Some(line) = lines.next() {
                    if lines.peek().is_none() && line.is_empty() {
                        break;
                    }
                    w.text(line, look, Block::Body, Align::Left);
                    w.end_line(Block::Body, Align::Left);
                }
            }
            Event::Text(t) | Event::Code(t) => w.text(&t, look, block, Align::Left),
            Event::SoftBreak => w.text(" ", look, block, Align::Left),
            Event::Rule => w.rule(),
            // Markdown has no underline: HTML's own tags for it are the way people write one.
            Event::InlineHtml(tag) => match tag.trim().to_ascii_lowercase().as_str() {
                "<u>" | "<ins>" => underline += 1,
                "</u>" | "</ins>" => underline = underline.saturating_sub(1),
                "<br>" | "<br/>" | "<br />" => w.end(),
                _ => {}
            },
            _ => {}
        }
    }
    w.finish()
}

// ------------------------------------------------------------------ HTML

/// Elements that are paragraphs, or hold them: each begins and ends one.
const BLOCKS: &[&str] = &[
    "p", "div", "h1", "h2", "h3", "h4", "h5", "h6", "li", "ul", "ol", "dl", "dt", "dd", "blockquote", "pre", "table", "thead", "tbody",
    "tfoot", "tr", "caption", "section", "article", "header", "footer", "nav", "aside", "main", "figure", "figcaption", "address",
    "center", "form", "fieldset", "details", "summary", "body", "html",
];

/// Elements that are never closed.
const VOID: &[&str] = &["br", "hr", "img", "meta", "link", "input", "col", "wbr", "area", "base", "source", "embed", "param", "track"];

/// Elements whose contents are not text to paste.
const UNSEEN: &[&str] = &["style", "script", "head", "title", "template", "noscript", "xml", "svg", "math", "iframe", "object", "select"];

/// An element read and not yet closed, with what it gives the text inside it.
struct Open {
    name: String,
    look: Look,
    /// A heading or a list item makes the paragraphs in it one (the innermost wins).
    block: Option<Block>,
    align: Option<Align>,
    /// Hidden (`display:none`): its text is not pasted.
    hidden: bool,
    /// Its text keeps its spaces and lines.
    pre: bool,
}

/// HTML from the clipboard read into paragraphs. It is read leniently, as browsers do: elements
/// left open are closed by what closes them, and anything not understood is passed over.
pub fn from_html(html: &str) -> Vec<Para> {
    let mut r = HtmlReader::default();
    // On Windows the clipboard's HTML may begin with a header of offsets ("Version:0.9").
    let html = match html.find('<') {
        Some(i) if html.starts_with("Version:") => &html[i..],
        _ => html,
    };
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            r.text(rest);
            break;
        };
        r.text(&rest[..lt]);
        rest = &rest[lt..];
        rest = r.markup(rest);
    }
    r.w.finish()
}

#[derive(Default)]
struct HtmlReader {
    w: Writer,
    open: Vec<Open>,
    /// Inside one of Word's conditional parts (`<![if !supportLists]>`): what it says, for a list
    /// item's marker ("1." or "·"), which is drawn there and is not text.
    conditional: Option<String>,
}

impl HtmlReader {
    fn look(&self) -> Look {
        self.open.last().map(|o| o.look).unwrap_or_default()
    }

    fn block(&self) -> Block {
        self.open.iter().rev().find_map(|o| o.block).unwrap_or_default()
    }

    fn align(&self) -> Align {
        self.open.iter().rev().find_map(|o| o.align).unwrap_or_default()
    }

    fn text(&mut self, raw: &str) {
        if raw.is_empty() {
            return;
        }
        let text = decode(raw);
        if let Some(said) = &mut self.conditional {
            said.push_str(&text);
            return;
        }
        if self.open.iter().any(|o| o.hidden) {
            return;
        }
        let (look, block, align) = (self.look(), self.block(), self.align());
        if self.open.iter().any(|o| o.pre) {
            let mut lines = text.split('\n').peekable();
            while let Some(line) = lines.next() {
                self.w.text(line.trim_end_matches('\r'), look, block, align);
                if lines.peek().is_some() {
                    self.w.end_line(block, align);
                }
            }
            return;
        }
        // White space between words is one space, and none at a paragraph's edges.
        for (i, word) in text.split([' ', '\t', '\n', '\r', '\u{c}']).enumerate() {
            if i > 0 {
                self.w.soft_space(look);
            }
            if !word.is_empty() {
                self.w.text(word, look, block, align);
            }
        }
    }

    /// Read the markup at the start of `s` (which begins with `<`), and return what follows it.
    fn markup<'a>(&mut self, s: &'a str) -> &'a str {
        if let Some(body) = s.strip_prefix("<!--") {
            let (comment, rest) = body.split_once("-->").unwrap_or((body, ""));
            self.word_conditional(comment.trim());
            return rest;
        }
        if let Some(body) = s.strip_prefix("<![") {
            let (inside, rest) = body.split_once("]>").unwrap_or((body, ""));
            self.word_conditional(&format!("[{}]", inside.trim()));
            return rest;
        }
        if s.starts_with("<!") || s.starts_with("<?") {
            return s.split_once('>').map_or("", |(_, rest)| rest);
        }
        let closing = s.starts_with("</");
        let name_at = if closing { 2 } else { 1 };
        if !s[name_at..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            // A "<" that begins no tag is a letter.
            self.text("<");
            return &s[1..];
        }
        let (tag, rest) = split_tag(s);
        let name_end = tag[name_at..].find(|c: char| c.is_whitespace() || c == '/' || c == '>').map_or(tag.len(), |i| i + name_at);
        let name = tag[name_at..name_end].to_ascii_lowercase();
        if closing {
            self.close(&name);
            return rest;
        }
        let attrs = attributes(&tag[name_end..]);
        if UNSEEN.contains(&name.as_str()) && !tag.ends_with("/>") {
            // Skipped whole, whatever is in it: a style sheet may hold "<" and ">".
            let end = format!("</{name}");
            let lower = rest.to_ascii_lowercase();
            return match lower.find(&end) {
                Some(i) => rest[i..].split_once('>').map_or("", |(_, after)| after),
                None => "",
            };
        }
        self.open_tag(&name, &attrs, tag.ends_with("/>"));
        rest
    }

    /// Word marks list items' markers, and much else, as parts only older browsers show.
    fn word_conditional(&mut self, said: &str) {
        // Only the bare marker of a part: a whole part in one comment (`<!--[if gte mso 9]>…<![endif]-->`) is gone with it.
        if said.starts_with("[if ") && said.ends_with(']') && !said.contains('>') {
            self.conditional = Some(String::new());
        } else if said == "[endif]" {
            let Some(marker) = self.conditional.take() else { return };
            // "1." or "a)" numbers the item; "·", "o" or "§" (bullets in Symbol, Courier and Wingdings) does not.
            let marker = marker.trim_matches(|c: char| c.is_whitespace() || c == '\u{a0}');
            let numbered = marker.ends_with(['.', ')']) && marker.chars().any(char::is_alphanumeric);
            if let Some(o) = self.open.iter_mut().rev().find(|o| matches!(o.block, Some(Block::Bullet | Block::Numbered))) {
                o.block = Some(if numbered { Block::Numbered } else { Block::Bullet });
            }
        }
    }

    fn open_tag(&mut self, name: &str, attrs: &[(String, String)], self_closing: bool) {
        let block_level = BLOCKS.contains(&name);
        // Breaks and cells in what is not pasted are not either.
        let unseen = self.conditional.is_some() || self.open.iter().any(|o| o.hidden);
        if unseen && matches!(name, "br" | "hr" | "td" | "th") {
            return;
        }
        match name {
            "br" => {
                let (block, align) = (self.block(), self.align());
                self.w.end_line(block, align);
                return;
            }
            "hr" => return self.w.rule(),
            // A cell after the first in a row is set apart by a tab.
            "td" | "th" => {
                self.close_implied(&["td", "th"], &["tr", "table"]);
                if self.w.started {
                    self.w.text("\t", Look::default(), self.block(), self.align());
                }
            }
            "li" => self.close_implied(&["li"], &["ul", "ol"]),
            "dt" | "dd" => self.close_implied(&["dt", "dd"], &["dl"]),
            "tr" => self.close_implied(&["tr"], &["table"]),
            _ => {}
        }
        // A new paragraph closes one left open (`<p>a<p>b`), as browsers do.
        if block_level {
            self.close_implied(&["p", "h1", "h2", "h3", "h4", "h5", "h6"], &["li", "td", "th", "div", "blockquote", "ul", "ol", "table"]);
            self.w.end();
        }
        if VOID.contains(&name) || self_closing {
            return;
        }
        let mut look = self.look();
        let mut block = None;
        let mut align = None;
        let mut hidden = false;
        match name {
            "b" | "strong" | "th" => look.bold = true,
            "i" | "em" | "cite" | "dfn" | "var" => look.italic = true,
            "u" | "ins" => look.underline = true,
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => block = Some(Block::Heading),
            "li" => {
                let numbered = self.open.iter().rev().find(|o| o.name == "ul" || o.name == "ol").is_some_and(|o| o.name == "ol");
                block = Some(if numbered { Block::Numbered } else { Block::Bullet });
            }
            "center" => align = Some(Align::Center),
            _ => {}
        }
        for (key, value) in attrs {
            match key.as_str() {
                "align" => align = parse_align(value).or(align),
                "color" if name == "font" => {
                    if let Some(c) = parse_color(value) {
                        look.color = c;
                    }
                }
                // Word's title paragraph.
                "class" if value.split_whitespace().any(|c| c.eq_ignore_ascii_case("MsoTitle")) => block = Some(Block::Heading),
                "hidden" => hidden = true,
                "style" => {
                    for decl in value.split(';') {
                        let Some((prop, v)) = decl.split_once(':') else { continue };
                        let (prop, v) = (prop.trim().to_ascii_lowercase(), v.trim().trim_end_matches("!important").trim().to_ascii_lowercase());
                        match prop.as_str() {
                            "font-weight" => {
                                let heavy = v.parse::<u32>().map_or(v == "bold" || v == "bolder", |w| w >= 600);
                                look.bold = heavy;
                            }
                            "font-style" => look.italic = v == "italic" || v == "oblique",
                            "text-decoration" | "text-decoration-line" => look.underline = v.contains("underline"),
                            "color" => {
                                if let Some(c) = parse_color(&v) {
                                    look.color = c;
                                }
                            }
                            "text-align" => align = parse_align(&v).or(align),
                            "display" => hidden |= v == "none",
                            "mso-hide" => hidden |= v == "all",
                            // Word's list items are paragraphs that say they are one; their marker,
                            // read later, says which kind.
                            "mso-list" if v != "ignore" && block.is_none() => block = Some(Block::Bullet),
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        let pre = name == "pre" || name == "textarea";
        self.open.push(Open { name: name.to_owned(), look, block, align, hidden, pre });
        if block_level {
            self.w.end();
        }
    }

    /// Close the innermost open element named `name`, and what is open inside it.
    fn close(&mut self, name: &str) {
        let Some(at) = self.open.iter().rposition(|o| o.name == name) else { return };
        let block_level = self.open[at..].iter().any(|o| BLOCKS.contains(&o.name.as_str()));
        if block_level {
            self.w.end();
        }
        self.open.truncate(at);
    }

    /// Close an element of `names` left open, unless one of `within` is open inside it.
    fn close_implied(&mut self, names: &[&str], within: &[&str]) {
        for (i, o) in self.open.iter().enumerate().rev() {
            if within.contains(&o.name.as_str()) {
                return;
            }
            if names.contains(&o.name.as_str()) {
                self.w.end();
                self.open.truncate(i);
                return;
            }
        }
    }
}

/// The tag at the start of `s` (up to its `>`, quotes respected), and what follows it.
fn split_tag(s: &str) -> (&str, &str) {
    let mut quote = None;
    for (i, c) in s.char_indices() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, '"' | '\'') => quote = Some(c),
            (None, '>') => return (&s[..=i], &s[i + 1..]),
            _ => {}
        }
    }
    (s, "")
}

/// A tag's attributes (after its name), names in lower case, values decoded.
fn attributes(s: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let s = s.trim_end_matches('>').trim_end_matches('/');
    let mut rest = s;
    loop {
        rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '/');
        if rest.is_empty() {
            return out;
        }
        let end = rest.find(|c: char| c.is_whitespace() || c == '=' || c == '/').unwrap_or(rest.len());
        let name = rest[..end].to_ascii_lowercase();
        rest = rest[end..].trim_start();
        let value = if let Some(after) = rest.strip_prefix('=') {
            let after = after.trim_start();
            let (value, next) = match after.chars().next() {
                Some(q @ ('"' | '\'')) => after[1..].split_once(q).unwrap_or((&after[1..], "")),
                _ => after.split_at(after.find(char::is_whitespace).unwrap_or(after.len())),
            };
            rest = next;
            decode(value)
        } else {
            String::new()
        };
        if !name.is_empty() {
            out.push((name, value));
        }
    }
}

fn parse_align(v: &str) -> Option<Align> {
    match v.trim().to_ascii_lowercase().as_str() {
        "left" | "start" => Some(Align::Left),
        "center" | "middle" => Some(Align::Center),
        "right" | "end" => Some(Align::Right),
        "justify" => Some(Align::Justify),
        _ => None,
    }
}

/// A CSS or HTML colour: `Some(None)` for the story's ink (near black, or too pale to read on the
/// page), `None` for a value not understood, which changes nothing.
fn parse_color(v: &str) -> Option<Option<[u8; 3]>> {
    let v = v.trim().to_ascii_lowercase();
    let rgb = if let Some(hex) = v.strip_prefix('#') {
        let digit = |i: usize, n: usize| u8::from_str_radix(hex.get(i..i + n)?, 16).ok();
        match hex.len() {
            3 | 4 => [digit(0, 1)? * 17, digit(1, 1)? * 17, digit(2, 1)? * 17],
            6 | 8 => [digit(0, 2)?, digit(2, 2)?, digit(4, 2)?],
            _ => return None,
        }
    } else if let Some(args) = v.strip_prefix("rgb(").or_else(|| v.strip_prefix("rgba(")) {
        let mut parts = args.trim_end_matches(')').split([',', ' ', '/']).filter(|p| !p.is_empty());
        let mut part = || {
            let p = parts.next()?;
            let n = match p.strip_suffix('%') {
                Some(pc) => pc.parse::<f32>().ok()? * 2.55,
                None => p.parse::<f32>().ok()?,
            };
            Some(n.round().clamp(0.0, 255.0) as u8)
        };
        [part()?, part()?, part()?]
    } else {
        match v.as_str() {
            "black" | "windowtext" => [0, 0, 0],
            "white" => [255, 255, 255],
            "red" => [255, 0, 0],
            "maroon" => [128, 0, 0],
            "green" => [0, 128, 0],
            "blue" => [0, 0, 255],
            "navy" => [0, 0, 128],
            "purple" => [128, 0, 128],
            "teal" => [0, 128, 128],
            "olive" => [128, 128, 0],
            "orange" => [255, 165, 0],
            "gray" | "grey" => [128, 128, 128],
            "brown" => [165, 42, 42],
            _ => return None,
        }
    };
    let [r, g, b] = rgb.map(f32::from);
    let near_black = r.max(g).max(b) <= 68.0 && r.max(g).max(b) - r.min(g).min(b) <= 16.0;
    let pale = (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0 > 0.75;
    Some(if near_black || pale { None } else { Some(rgb) })
}

/// Text with HTML's character references (`&amp;`, `&#233;`, `&#x2014;`) written out.
fn decode(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        let end = rest[1..].find(|c: char| !c.is_ascii_alphanumeric() && c != '#').map_or(rest.len(), |e| e + 1);
        let name = &rest[1..end];
        let c = if let Some(num) = name.strip_prefix('#') {
            let n = match num.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => num.parse().ok(),
            };
            n.and_then(char::from_u32)
        } else {
            named(name)
        };
        match c {
            Some(c) => {
                if c != '\u{ad}' {
                    out.push(c);
                }
                rest = &rest[end..];
                rest = rest.strip_prefix(';').unwrap_or(rest);
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The named character references a word processor or a web page writes, mostly; the rest are
/// written as the letters themselves.
fn named(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "ensp" | "emsp" | "thinsp" => ' ',
        "shy" => '\u{ad}',
        "hellip" => '…',
        "mdash" => '—',
        "ndash" => '–',
        "lsquo" => '‘',
        "rsquo" => '’',
        "sbquo" => '‚',
        "ldquo" => '“',
        "rdquo" => '”',
        "bdquo" => '„',
        "laquo" => '«',
        "raquo" => '»',
        "lsaquo" => '‹',
        "rsaquo" => '›',
        "bull" => '•',
        "middot" => '·',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "deg" => '°',
        "euro" => '€',
        "pound" => '£',
        "sect" => '§',
        "para" => '¶',
        "times" => '×',
        "divide" => '÷',
        "auml" => 'ä',
        "ouml" => 'ö',
        "uuml" => 'ü',
        "Auml" => 'Ä',
        "Ouml" => 'Ö',
        "Uuml" => 'Ü',
        "szlig" => 'ß',
        "eacute" => 'é',
        "egrave" => 'è',
        "ecirc" => 'ê',
        "agrave" => 'à',
        "aacute" => 'á',
        "acirc" => 'â',
        "ccedil" => 'ç',
        "iacute" => 'í',
        "oacute" => 'ó',
        "uacute" => 'ú',
        "ntilde" => 'ñ',
        "Eacute" => 'É',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(paras: &[Para]) -> Vec<String> {
        paras.iter().map(Para::text).collect()
    }

    fn styled(paras: &[Para], pick: impl Fn(&Look) -> bool) -> String {
        paras.iter().flat_map(|p| &p.runs).filter(|(_, l)| pick(l)).map(|(t, _)| t.as_str()).collect::<Vec<_>>().join("|")
    }

    #[test]
    fn markdown_gives_bold_italic_lists_and_headings() {
        let md = "# The Inn\n\nIt was **late** and *quiet*, `code` too.\n\n- one\n- two **bold**\n\n1. first\n2. second\n\n---\n\nEnd.\n";
        let paras = from_markdown(md);
        assert_eq!(texts(&paras), ["The Inn", "It was late and quiet, code too.", "one", "two bold", "first", "second", "* * *", "End."]);
        let blocks: Vec<Block> = paras.iter().map(|p| p.block).collect();
        use Block::*;
        assert_eq!(blocks, [Heading, Body, Bullet, Bullet, Numbered, Numbered, Body, Body]);
        assert_eq!(styled(&paras, |l| l.bold), "late|bold");
        assert_eq!(styled(&paras, |l| l.italic), "quiet");
        assert_eq!(paras[6].align, Align::Center, "a rule is a centred break");
    }

    #[test]
    fn markdown_like_the_writers_paste_keeps_its_bold_lines_and_items() {
        let md = "**In-Game Authoring** (Recommended next step)\n- Trigger `make-element` straight from inside\n  the game, **hot-reloads**.\n\n**Multiplayer**\n- An authoritative server";
        let paras = from_markdown(md);
        assert_eq!(
            texts(&paras),
            ["In-Game Authoring (Recommended next step)", "Trigger make-element straight from inside the game, hot-reloads.", "Multiplayer", "An authoritative server"]
        );
        assert_eq!(styled(&paras, |l| l.bold), "In-Game Authoring|hot-reloads|Multiplayer");
        assert_eq!(paras[1].block, Block::Bullet);
        assert_eq!(paras[2].block, Block::Body);
    }

    #[test]
    fn markdown_tables_are_rows_with_tabs_and_code_keeps_its_lines() {
        let paras = from_markdown("| a | b |\n|---|---|\n| 1 | 2 |\n\n```\nx = 1\n\ny = 2\n```\n");
        assert_eq!(texts(&paras), ["a\tb", "1\t2", "x = 1", "", "y = 2"]);
    }

    #[test]
    fn html_from_a_word_processor_keeps_the_look_and_drops_the_rest() {
        let html = r#"<html><head><style>p { color: red }</style></head><body><!--StartFragment-->
            <h1 style="font-size:20pt">Chapter&nbsp;One</h1>
            <p style="text-align:center">It was <b>very</b> <i>dark</i>, <u>and</u> <span style="color:#c00000">red</span>.</p>
            <p><span style="color:#000000">ink</span> <span style="color:rgb(250,250,250)">pale</span> <a href="x">link</a></p>
            <ul><li>one</li><li><p>two</p></li></ul><ol><li>first<li>second</ol>
            <p>&lt;a&gt; &amp; caf&eacute; &#8212; &#x2014;</p><!--EndFragment--></body></html>"#;
        let paras = from_html(html);
        assert_eq!(texts(&paras), ["Chapter One", "It was very dark, and red.", "ink pale link", "one", "two", "first", "second", "<a> & café — —"]);
        use Block::*;
        let blocks: Vec<Block> = paras.iter().map(|p| p.block).collect();
        assert_eq!(blocks, [Heading, Body, Body, Bullet, Bullet, Numbered, Numbered, Body]);
        assert_eq!(paras[1].align, Align::Center);
        assert_eq!(styled(&paras, |l| l.bold), "very");
        assert_eq!(styled(&paras, |l| l.italic), "dark");
        assert_eq!(styled(&paras, |l| l.underline), "and");
        assert_eq!(styled(&paras, |l| l.color.is_some()), "red", "black and colours too pale for the page are the ink");
    }

    #[test]
    fn html_from_google_docs_undoes_its_bold_wrapper() {
        let html = r#"<meta charset="utf-8"><b style="font-weight:normal;" id="docs-internal-guid-1"><p dir="ltr"><span style="font-weight:400">plain </span><span style="font-weight:700">heavy</span> <span style="font-style:italic;text-decoration:underline">both</span></p><br><p dir="ltr"><span>next</span></p></b>"#;
        let paras = from_html(html);
        assert_eq!(texts(&paras), ["plain heavy both", "", "next"]);
        assert_eq!(styled(&paras, |l| l.bold), "heavy");
        assert_eq!(styled(&paras, |l| l.italic && l.underline), "both");
    }

    #[test]
    fn html_from_word_makes_its_list_paragraphs_items_without_their_markers() {
        let html = "<p class=MsoTitle>Title</p>\
            <p class=MsoListParagraphCxSpFirst style='mso-list:l0 level1 lfo1'><![if !supportLists]><span style='font-family:Symbol'>·<span style='font:7.0pt'>&nbsp;&nbsp; </span></span><![endif]>dot</p>\
            <p class=MsoListParagraph style='mso-list:l1 level1 lfo2'><!--[if !supportLists]--><span>1.<span>&nbsp;&nbsp;</span></span><!--[endif]-->counted</p>\
            <p class=MsoNormal><o:p>&nbsp;</o:p></p><p class=MsoNormal>after<span style='display:none;mso-hide:all'>hidden</span></p>";
        let paras = from_html(html);
        assert_eq!(texts(&paras), ["Title", "dot", "counted", "", "after"]);
        use Block::*;
        let blocks: Vec<Block> = paras.iter().map(|p| p.block).collect();
        assert_eq!(blocks, [Heading, Bullet, Numbered, Body, Body]);
    }

    #[test]
    fn html_from_a_code_editor_keeps_lines_and_drops_pale_colours() {
        let html = r#"<div style="color: #d4d4d4;background-color: #1e1e1e;white-space: pre;"><div><span style="color: #569cd6;">let</span><span style="color: #d4d4d4;"> x = 1;</span></div><div><br></div><pre>a  b
c</pre></div>"#;
        let paras = from_html(html);
        assert_eq!(texts(&paras), ["let x = 1;", "", "a  b", "c"]);
        assert_eq!(styled(&paras, |l| l.color.is_some()), "let");
    }

    #[test]
    fn white_space_between_words_is_one_space() {
        let paras = from_html("<p>\n  one\n   <b>two</b>  three \n</p>");
        assert_eq!(texts(&paras), ["one two three"]);
        assert_eq!(styled(&paras, |l| l.bold), "two");
    }
}
