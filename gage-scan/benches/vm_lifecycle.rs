//! Quick bench: build a minimal Rune context once, then construct and
//! drop fresh Vms in a loop. Reports per-Vm wall time in nanos.

use std::time::Instant;

use rune::sync::Arc as RuneArc;
use rune::{Diagnostics, Source, Sources};

const SOURCE: &str = r#"
pub async fn noop() {
    Ok(())
}
"#;

fn main() {
    let compile_started = Instant::now();
    let context = rune_modules::default_context().unwrap();
    let mut sources = Sources::new();
    sources
        .insert(Source::new("bench", SOURCE).unwrap())
        .unwrap();
    let mut diagnostics = Diagnostics::new();
    let unit = rune::prepare(&mut sources)
        .with_context(&context)
        .with_diagnostics(&mut diagnostics)
        .build()
        .unwrap();
    let unit = RuneArc::try_new(unit).unwrap();
    let rt = RuneArc::try_new(context.runtime().unwrap()).unwrap();
    let compile_elapsed = compile_started.elapsed();
    println!("compile (one-time, per scanner): {compile_elapsed:?}");

    for _ in 0..1_000 {
        let _vm = rune::Vm::new(rt.clone(), unit.clone());
    }

    for n in [1_000usize, 10_000, 100_000, 1_000_000] {
        let started = Instant::now();
        for _ in 0..n {
            let _vm = rune::Vm::new(rt.clone(), unit.clone());
        }
        let elapsed = started.elapsed();
        let per = elapsed.as_nanos() as f64 / n as f64;
        println!("Vm::new + drop  x {n:>7}: total {elapsed:>10?}  per Vm: {per:>6.1} ns");
    }
}
