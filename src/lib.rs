//! Kilat: a small, safe, unit-aware language.
//!
//! Pipeline: `lexer` -> `parser` (AST) -> `check` (typed IR) -> `compile` (bytecode) -> `vm`.
//! The core does no input or output of its own: everything from the outside world goes
//! through the host gate (`vm::Status::Host`), which the embedder answers.

pub mod ast;
pub mod builtins;
pub mod check;
pub mod check_expr;
pub mod compile;
pub mod ffi;
pub mod heap;
pub mod lexer;
pub mod natives;
pub mod parser;
pub mod tir;
pub mod types;
pub mod vm;

use ast::Item;
use compile::Program;
use types::{Diag, Pos};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The standard prelude, written in Kilat.
pub const PRELUDE: &str = include_str!("prelude.kilat");

/// The most files one program may be made of (file ids are packed into 12 bits).
const MAX_FILES: usize = 1000;

/// Answers `import "path"`: gets `(path, name of the importing file)` and returns
/// `(resolved name, source text)`. The resolved name is what tells two imports of the same file apart.
pub type Loader<'a> = dyn FnMut(&str, &str) -> Result<(String, String), String> + 'a;

/// A loader for programs that cannot import anything.
pub fn no_imports(path: &str, _importer: &str) -> Result<(String, String), String> {
    Err(format!("`{}` was not found (this program cannot import files)", path))
}

/// Where `import "path"` written in the file `importer` points: relative to that file's folder.
/// The result uses `/` and has `.` and `..` folded away, so the same file always gets the same name.
pub fn resolve_import(importer: &str, path: &str) -> String {
    let path = path.replace('\\', "/");
    let importer = importer.replace('\\', "/");
    let absolute = path.starts_with('/') || path.as_bytes().get(1) == Some(&b':');
    let joined = match (absolute, importer.rfind('/')) {
        (false, Some(i)) => format!("{}/{}", &importer[..i], path),
        _ => path,
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" if !parts.is_empty() => {}
            "." => {}
            ".." => {
                let can_pop = matches!(parts.last(), Some(p) if *p != ".." && !p.is_empty() && !p.ends_with(':'));
                if can_pop {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

struct Sources {
    /// `(name, text)` by file id: 0 is the prelude, 1 the main file, then the imports.
    files: Vec<(String, String)>,
}

fn load_imports(
    items: &[Item],
    importer: usize,
    sources: &mut Sources,
    loader: &mut Loader,
    out: &mut Vec<Item>,
    errs: &mut Vec<Diag>,
) {
    for item in items {
        let (path, pos) = match item {
            Item::Import(path, pos) => (path, *pos),
            _ => continue,
        };
        let importer_name = sources.files[importer].0.clone();
        let (name, text) = match loader(path, &importer_name) {
            Ok(found) => found,
            Err(why) => {
                errs.push(Diag::new(pos, format!("cannot import \"{}\": {}", path, why)));
                continue;
            }
        };
        if sources.files.iter().any(|(known, _)| *known == name) {
            continue;
        }
        if sources.files.len() >= MAX_FILES {
            errs.push(Diag::new(pos, "the program imports too many files"));
            return;
        }
        let id = sources.files.len();
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text).to_string();
        sources.files.push((name, text.clone()));
        match parser::parse_file(&text, id as u16, false) {
            Ok(parsed) => {
                // What a file imports comes before the file itself.
                load_imports(&parsed, id, sources, loader, out, errs);
                for it in parsed {
                    if !matches!(it, Item::Import(..) | Item::Test(..)) {
                        out.push(it);
                    }
                }
            }
            Err(d) => errs.push(d),
        }
    }
}

/// Compile a program. `name` is what the main file is called in messages.
/// With `print_last`, a final top-level expression is printed (as a calculator would).
/// On failure the error text holds every diagnostic, ready to show.
pub fn build(name: &str, src: &str, loader: &mut Loader, print_last: bool) -> Result<Program, String> {
    let src = src.strip_prefix('\u{feff}').unwrap_or(src);
    let mut sources = Sources {
        files: vec![("<prelude>".to_string(), PRELUDE.to_string()), (name.to_string(), src.to_string())],
    };
    let fail = |diags: &[Diag], sources: &Sources| -> String {
        let mut text = String::new();
        for (i, d) in diags.iter().enumerate() {
            if i > 0 {
                text.push('\n');
            }
            text.push_str(&render_diag(d, &sources.files));
        }
        if diags.len() > 1 {
            text.push_str(&format!("\n{} errors", diags.len()));
        }
        text
    };

    let mut items = match parser::parse_file(PRELUDE, 0, true) {
        Ok(items) => items,
        Err(d) => return Err(fail(&[d], &sources)),
    };
    let main_items = match parser::parse_file(src, 1, false) {
        Ok(items) => items,
        Err(d) => return Err(fail(&[d], &sources)),
    };
    let mut errs = Vec::new();
    let mut imported = Vec::new();
    load_imports(&main_items, 1, &mut sources, loader, &mut imported, &mut errs);
    if !errs.is_empty() {
        return Err(fail(&errs, &sources));
    }
    items.extend(imported);
    items.extend(main_items.into_iter().filter(|it| !matches!(it, Item::Import(..))));

    let checked = match check::check(&items, print_last) {
        Ok(checked) => checked,
        Err(mut diags) => {
            diags.truncate(20);
            return Err(fail(&diags, &sources));
        }
    };
    let names = sources.files.iter().map(|(n, _)| n.clone()).collect();
    match compile::compile(&checked, names) {
        Ok(program) => Ok(program),
        Err(d) => Err(fail(&[d], &sources)),
    }
}

/// Show a diagnostic with the line it points at:
///
/// ```text
/// error: unit mismatch: cannot add N and m
///   --> beam.kilat:12:15
///    |
/// 12 |     let x = force + length
///    |                   ^
///    = hint: ...
/// ```
pub fn render_diag(d: &Diag, files: &[(String, String)]) -> String {
    let mut out = format!("error: {}", d.msg);
    let Pos { line, col, file } = d.pos;
    let source = files.get(file as usize);
    let mut gutter = 1;
    if let (true, Some((name, text))) = (line > 0, source) {
        let number = line.to_string();
        gutter = number.len();
        let pad = " ".repeat(gutter);
        out.push_str(&format!("\n{} --> {}:{}:{}", pad, name, line, col));
        if let Some(raw) = text.lines().nth(line as usize - 1) {
            // Tabs become single spaces so that the caret lines up.
            let shown: String = raw.chars().map(|c| if c == '\t' { ' ' } else { c }).collect();
            let shown = shown.trim_end();
            let caret = " ".repeat((col as usize).saturating_sub(1).min(shown.chars().count()));
            out.push_str(&format!("\n{} |\n{} | {}\n{} | {}^", pad, number, shown, pad, caret));
        }
    }
    if let Some(hint) = &d.hint {
        out.push_str(&format!("\n{} = hint: {}", " ".repeat(gutter), hint));
    }
    out
}
