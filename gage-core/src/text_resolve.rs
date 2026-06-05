use std::collections::HashMap;

#[derive(Debug)]
pub enum TextResolveError {
    UnsupportedScheme(String),
    SchemeResolve(String),
}

impl std::fmt::Display for TextResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TextResolveError::UnsupportedScheme(scheme) => {
                write!(f, "unsupported URI scheme '{scheme}:'")
            }
            TextResolveError::SchemeResolve(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for TextResolveError {}

pub trait TextResolverScheme {
    /// Resolve `input` (the full URI, including scheme prefix) to its
    /// final body. The scheme is dispatched by registered name but may
    /// be reused for multiple scheme prefixes.
    fn resolve(&self, input: &str) -> Result<String, TextResolveError>;
}

#[derive(Default)]
pub struct TextResolver {
    schemes: HashMap<String, Box<dyn TextResolverScheme>>,
}

impl TextResolver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_scheme(mut self, name: &str, scheme: impl TextResolverScheme + 'static) -> Self {
        self.schemes.insert(name.to_string(), Box::new(scheme));
        self
    }

    /// Resolve `input`. If `input` has no recognized URI scheme prefix
    /// it is returned verbatim (no allocation). If the scheme is
    /// registered, dispatches to the scheme; otherwise returns
    /// `UnsupportedScheme`.
    pub fn resolve(&self, input: String) -> Result<String, TextResolveError> {
        let Some(scheme) = scheme_name(&input) else {
            return Ok(input);
        };
        match self.schemes.get(scheme) {
            Some(s) => s.resolve(&input),
            None => Err(TextResolveError::UnsupportedScheme(scheme.to_string())),
        }
    }
}

/// Returns the URI scheme name from `s` if `s` starts with a
/// syntactically valid scheme prefix (`[a-z][a-z0-9+.-]*:`), otherwise
/// `None`.
fn scheme_name(s: &str) -> Option<&str> {
    let (scheme, _) = s.split_once(':')?;
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_lowercase() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '+' | '.' | '-'))
    {
        return None;
    }
    Some(scheme)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EchoScheme;
    impl TextResolverScheme for EchoScheme {
        fn resolve(&self, input: &str) -> Result<String, TextResolveError> {
            Ok(format!("echo:{input}"))
        }
    }

    struct FailScheme;
    impl TextResolverScheme for FailScheme {
        fn resolve(&self, _input: &str) -> Result<String, TextResolveError> {
            Err(TextResolveError::SchemeResolve("boom".into()))
        }
    }

    #[test]
    fn literal_passes_through() {
        let r = TextResolver::new();
        assert_eq!(r.resolve("plain text".into()).unwrap(), "plain text");
        assert_eq!(r.resolve("TODO: do it".into()).unwrap(), "TODO: do it");
    }

    #[test]
    fn unregistered_scheme_errors() {
        let r = TextResolver::new();
        let err = r.resolve("http://example.com".into()).unwrap_err();
        assert!(matches!(err, TextResolveError::UnsupportedScheme(s) if s == "http"));
    }

    #[test]
    fn registered_scheme_dispatches() {
        let r = TextResolver::new().with_scheme("foo", EchoScheme);
        assert_eq!(r.resolve("foo:bar".into()).unwrap(), "echo:foo:bar");
    }

    #[test]
    fn scheme_error_propagates() {
        let r = TextResolver::new().with_scheme("foo", FailScheme);
        let err = r.resolve("foo:bar".into()).unwrap_err();
        assert!(matches!(err, TextResolveError::SchemeResolve(s) if s == "boom"));
    }

    #[test]
    fn scheme_name_validates() {
        assert_eq!(scheme_name("scanner:foo"), Some("scanner"));
        assert_eq!(scheme_name("a+b-c.d:rest"), Some("a+b-c.d"));
        assert_eq!(scheme_name("TODO: do"), None);
        assert_eq!(scheme_name("9bad:foo"), None);
        assert_eq!(scheme_name(":empty"), None);
    }
}
