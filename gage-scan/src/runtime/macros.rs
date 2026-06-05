use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use rune::ast;
use rune::compile;
use rune::macros::quote;
use rune::parse::Parser;
use rune::{ContextError, Module};
use serde_json as json;

/// Mutable base directory the `include_*!` macros resolve relative paths
/// against during compilation. Sharing one cell across files lets a single
/// macros module (and the context it lives in) be reused for every source:
/// update the cell with `set_base_dir` before compiling each file
pub(crate) type SharedBaseDir = Arc<RwLock<String>>;

/// Build a base-dir cell seeded for a single scanner's `embed_key`
pub(crate) fn base_dir(embed_key: &str) -> SharedBaseDir {
    Arc::new(RwLock::new(scanner_base_dir(embed_key)))
}

/// Point an existing cell at the directory of `embed_key`, so a reused macros
/// module resolves includes relative to the file now being compiled
pub(crate) fn set_base_dir(base: &SharedBaseDir, embed_key: &str) {
    *base.write().unwrap() = scanner_base_dir(embed_key);
}

pub(crate) fn module(embed_key: &str, scanners_dir: PathBuf) -> Result<Module, ContextError> {
    module_shared(base_dir(embed_key), scanners_dir)
}

pub(crate) fn module_shared(
    base_dir: SharedBaseDir,
    scanners_dir: PathBuf,
) -> Result<Module, ContextError> {
    let mut m = Module::new();

    let dir = Arc::new(scanners_dir);

    let base = base_dir.clone();
    let dir_ref = dir.clone();
    m.macro_(["include_str"], move |cx, stream| {
        let mut p = Parser::from_token_stream(stream, cx.input_span());
        let path_lit = p.parse_all::<ast::LitStr>()?;
        let rel_path = cx.resolve(path_lit)?.try_into_owned()?;

        let file_path = dir_ref.join(format!("{}{rel_path}", base.read().unwrap()));
        let contents = std::fs::read_to_string(&file_path).map_err(|e| {
            compile::Error::msg(cx.macro_span(), format!("{}: {e}", file_path.display()))
        })?;

        let lit = cx.lit(contents.as_str())?;
        Ok(quote!(#lit).into_token_stream(cx)?)
    })?;

    let base = base_dir.clone();
    let dir_ref = dir.clone();
    m.macro_(["include_json"], move |cx, stream| {
        let mut p = Parser::from_token_stream(stream, cx.input_span());
        let path_lit = p.parse_all::<ast::LitStr>()?;
        let rel_path = cx.resolve(path_lit)?.try_into_owned()?;

        let file_path = dir_ref.join(format!("{}{rel_path}", base.read().unwrap()));
        let raw = std::fs::read_to_string(&file_path).map_err(|e| {
            compile::Error::msg(cx.macro_span(), format!("{}: {e}", file_path.display()))
        })?;

        let stripped = strip_jsonc(&raw);
        let json_val: json::Value = json::from_str(&stripped).map_err(|e| {
            compile::Error::msg(
                cx.macro_span(),
                format!("invalid JSON in {}: {e}", file_path.display()),
            )
        })?;

        let rune_src = json_to_rune_src(&json_val);
        let id = cx.insert_source("include_json", &rune_src)?;
        let expr = cx.parse_source::<ast::Expr>(id)?;
        Ok(quote!(#expr).into_token_stream(cx)?)
    })?;

    Ok(m)
}

fn scanner_base_dir(embed_key: &str) -> String {
    embed_key
        .rsplit_once('/')
        .map(|(dir, _)| format!("{dir}/"))
        .unwrap_or_default()
}

fn strip_jsonc(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if c == '\\' {
                if let Some(&next) = chars.peek() {
                    out.push(next);
                    chars.next();
                }
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' {
            match chars.peek() {
                Some(&'/') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                Some(&'*') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => out.push(c),
            }
        } else if c == ',' {
            let mut lookahead = String::new();
            while let Some(&next) = chars.peek() {
                if next.is_whitespace() {
                    lookahead.push(next);
                    chars.next();
                } else {
                    break;
                }
            }
            if chars.peek() == Some(&']') || chars.peek() == Some(&'}') {
                out.push_str(&lookahead);
            } else {
                out.push(',');
                out.push_str(&lookahead);
            }
        } else {
            out.push(c);
        }
    }

    out
}

fn json_to_rune_src(val: &json::Value) -> String {
    match val {
        json::Value::Null => "()".to_string(),
        json::Value::Bool(b) => b.to_string(),
        json::Value::Number(n) => n.to_string(),
        json::Value::String(s) => format!("{s:?}"),
        json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_to_rune_src).collect();
            format!("[{}]", items.join(", "))
        }
        json::Value::Object(map) => {
            let entries: Vec<String> = map
                .iter()
                .map(|(k, v)| format!("{k:?}: {}", json_to_rune_src(v)))
                .collect();
            format!("#{{ {} }}", entries.join(", "))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_jsonc_line_comments() {
        let input = r#"[
  "a", // comment
  "b"
]"#;
        let stripped = strip_jsonc(input);
        let val: json::Value = json::from_str(&stripped).unwrap();
        assert_eq!(val, json::json!(["a", "b"]));
    }

    #[test]
    fn strip_jsonc_block_comments() {
        let input = r#"[
  /* block comment */
  "a",
  "b"
]"#;
        let stripped = strip_jsonc(input);
        let val: json::Value = json::from_str(&stripped).unwrap();
        assert_eq!(val, json::json!(["a", "b"]));
    }

    #[test]
    fn strip_jsonc_trailing_commas() {
        let input = r#"["a", "b",]"#;
        let stripped = strip_jsonc(input);
        let val: json::Value = json::from_str(&stripped).unwrap();
        assert_eq!(val, json::json!(["a", "b"]));
    }

    #[test]
    fn strip_jsonc_trailing_comma_in_object() {
        let input = r#"{"x": 1, "y": 2,}"#;
        let stripped = strip_jsonc(input);
        let val: json::Value = json::from_str(&stripped).unwrap();
        assert_eq!(val, json::json!({"x": 1, "y": 2}));
    }

    #[test]
    fn strip_jsonc_preserves_slashes_in_strings() {
        let input = r#"["http://example.com", "a/b"]"#;
        let stripped = strip_jsonc(input);
        let val: json::Value = json::from_str(&stripped).unwrap();
        assert_eq!(val, json::json!(["http://example.com", "a/b"]));
    }

    #[test]
    fn strip_jsonc_preserves_escaped_quotes() {
        let input = r#"["say \"hello\"", "ok"]"#;
        let stripped = strip_jsonc(input);
        let val: json::Value = json::from_str(&stripped).unwrap();
        assert_eq!(val, json::json!(["say \"hello\"", "ok"]));
    }

    #[test]
    fn strip_jsonc_combined() {
        let input = r#"[
  // line comment
  "a",
  /* block */ "b",
]"#;
        let stripped = strip_jsonc(input);
        let val: json::Value = json::from_str(&stripped).unwrap();
        assert_eq!(val, json::json!(["a", "b"]));
    }

    #[test]
    fn json_to_rune_src_null() {
        assert_eq!(json_to_rune_src(&json::json!(null)), "()");
    }

    #[test]
    fn json_to_rune_src_bool() {
        assert_eq!(json_to_rune_src(&json::json!(true)), "true");
        assert_eq!(json_to_rune_src(&json::json!(false)), "false");
    }

    #[test]
    fn json_to_rune_src_number() {
        assert_eq!(json_to_rune_src(&json::json!(42)), "42");
        assert_eq!(json_to_rune_src(&json::json!(1.5)), "1.5");
    }

    #[test]
    fn json_to_rune_src_string() {
        assert_eq!(json_to_rune_src(&json::json!("hello")), r#""hello""#);
    }

    #[test]
    fn json_to_rune_src_string_with_quotes() {
        assert_eq!(
            json_to_rune_src(&json::json!("say \"hi\"")),
            r#""say \"hi\"""#
        );
    }

    #[test]
    fn json_to_rune_src_array() {
        assert_eq!(json_to_rune_src(&json::json!(["a", "b"])), r#"["a", "b"]"#);
    }

    #[test]
    fn json_to_rune_src_object() {
        let src = json_to_rune_src(&json::json!({"key": 1}));
        assert_eq!(src, r#"#{ "key": 1 }"#);
    }

    #[test]
    fn json_to_rune_src_nested() {
        let src = json_to_rune_src(&json::json!({"items": [1, 2], "ok": true}));
        assert!(src.starts_with("#{ "));
        assert!(src.contains(r#""items": [1, 2]"#));
        assert!(src.contains(r#""ok": true"#));
    }

    #[test]
    fn scanner_base_dir_with_path() {
        assert_eq!(scanner_base_dir("friction.rn"), "");
        assert_eq!(scanner_base_dir("sub/scanner.rn"), "sub/");
        assert_eq!(scanner_base_dir("a/b/c.rn"), "a/b/");
    }
}
