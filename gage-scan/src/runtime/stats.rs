use std::fmt;

use rune::runtime::{Iterator, Value, VmError};
use rune::{ContextError, Module, TypeHash};

pub(crate) fn module() -> Result<Module, ContextError> {
    let mut m = Module::with_crate("stats")?;
    m.function("mean", mean).build()?;
    Ok(m)
}

/// Arithmetic mean of an iterable of numbers, or `None` when it is empty.
///
/// Accepts any Rune iterable (vector, object values, range, lazy iterator
/// chain). Integers and floats may be mixed; the result is always a float.
fn mean(mut values: Iterator) -> Result<Option<f64>, VmError> {
    let mut sum = 0.0;
    let mut count = 0u64;

    while let Some(value) = values.next()? {
        sum += to_f64(&value)?;
        count += 1;
    }

    Ok((count != 0).then(|| sum / count as f64))
}

/// Coerce a Rune value to `f64`, accepting integers as well as floats.
fn to_f64(value: &Value) -> Result<f64, VmError> {
    let hash = value.type_hash();

    if hash == f64::HASH {
        Ok(value.as_float().expect("value is f64 per type_hash"))
    } else if hash == i64::HASH {
        Ok(value.as_signed().expect("value is i64 per type_hash") as f64)
    } else if hash == u64::HASH {
        Ok(value.as_unsigned().expect("value is u64 per type_hash") as f64)
    } else {
        Err(VmError::panic(ExpectedNumber {
            actual: value.type_info().to_string(),
        }))
    }
}

/// Raised when a value fed to a numeric aggregation is not a number.
///
/// Rune's own `ExpectedNumber` error kind is unreachable (its `VmErrorKind` is
/// crate-private), so we carry our own and surface it through `VmError::panic`.
#[derive(Debug)]
struct ExpectedNumber {
    actual: String,
}

impl fmt::Display for ExpectedNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "expected a number, but found `{}`", self.actual)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rune::sync::Arc;
    use rune::{Context, Vm};

    fn call(expr: &str) -> Result<Value, VmError> {
        let mut context = Context::with_default_modules().expect("default modules to build");
        context
            .install(module().expect("stats module to build"))
            .expect("stats module to install");
        let runtime =
            Arc::try_new(context.runtime().expect("runtime to build")).expect("arc runtime");

        let mut sources = rune::Sources::new();
        sources
            .insert(rune::Source::memory(format!("pub fn main() {{ {expr} }}")).expect("source"))
            .expect("source to insert");

        let unit = rune::prepare(&mut sources)
            .with_context(&context)
            .build()
            .expect("unit to build");
        let mut vm = Vm::new(runtime, Arc::try_new(unit).expect("arc unit"));

        vm.call(["main"], ())
    }

    fn mean_of(expr: &str) -> Option<f64> {
        let value = call(expr).expect("mean call to succeed");
        rune::from_value::<Option<f64>>(value).expect("an Option<f64> result")
    }

    #[test]
    fn mean_of_ints() {
        assert_eq!(mean_of("stats::mean([1, 2, 3])"), Some(2.0));
    }

    #[test]
    fn mean_of_floats() {
        assert_eq!(mean_of("stats::mean([1.0, 2.0, 3.0])"), Some(2.0));
    }

    #[test]
    fn mean_mixes_ints_and_floats() {
        assert_eq!(mean_of("stats::mean([1, 2.5, 4])"), Some(2.5));
    }

    #[test]
    fn mean_of_empty_is_none() {
        assert_eq!(mean_of("stats::mean([])"), None);
    }

    #[test]
    fn mean_consumes_object_values() {
        assert_eq!(mean_of("stats::mean(#{ a: 2, b: 4 }.values())"), Some(3.0));
    }

    #[test]
    fn mean_consumes_lazy_iterator() {
        assert_eq!(
            mean_of("stats::mean([1, 2, 3, 4].iter().filter(|n| n % 2 == 0))"),
            Some(3.0),
        );
    }

    #[test]
    fn mean_rejects_non_numbers() {
        let error = call("stats::mean([1, \"two\", 3])").expect_err("non-number should error");
        assert!(
            error.to_string().contains("expected a number"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn to_f64_coerces_signed() {
        assert_eq!(
            to_f64(&rune::to_value(3i64).expect("to_value")).expect("number"),
            3.0
        );
    }

    #[test]
    fn to_f64_coerces_float() {
        assert_eq!(
            to_f64(&rune::to_value(2.5f64).expect("to_value")).expect("number"),
            2.5
        );
    }

    #[test]
    fn to_f64_rejects_string() {
        to_f64(&rune::to_value("nope").expect("to_value")).expect_err("string is not a number");
    }
}
