pub(crate) mod config;
pub(crate) mod datetime;
pub(crate) mod db;
pub(crate) mod error;
pub(crate) mod ignore;
pub(crate) mod io;
pub(crate) mod json;
pub(crate) mod llm;
pub(crate) mod macros;
pub(crate) mod query;
mod result;
pub(crate) mod scan;
pub(crate) mod state;
pub(crate) mod stats;
pub(crate) mod template;
pub(crate) mod value;

pub(crate) use result::Result;

use std::path::PathBuf;

use rune::{Context, ContextError, Module};

/// Build a Rune context for the language server: the same native modules the
/// runtime installs (`runner::run`), so scanner sources resolve `gage::*`,
/// `io`, `stats`, `json`, and the `include_*` macros.
///
/// This is not parameterized per scanner like the runtime is. The language
/// server holds a single context for its lifetime, so there is no per-scanner
/// base directory: the `include_str!`/`include_json!` macros resolve relative
/// to `scanners_dir` itself rather than an individual scanner's subdirectory.
/// No active scanner uses those macros, and runtime resolution is unaffected.
pub fn lsp_context(scanners_dir: PathBuf) -> std::result::Result<Context, ContextError> {
    let mut context = rune_modules::with_config(false)?;
    context.install(io_module()?)?;
    context.install(types_module()?)?;
    context.install(macros_module("", scanners_dir)?)?;
    context.install(gage_module()?)?;
    context.install(stats_module()?)?;
    context.install(json_module()?)?;
    Ok(context)
}

pub(crate) fn macros_module(
    embed_key: &str,
    scanners_dir: PathBuf,
) -> std::result::Result<Module, ContextError> {
    macros::module(embed_key, scanners_dir)
}

pub(crate) use macros::{base_dir, module_shared as macros_module_shared, set_base_dir};

pub(crate) fn gage_module() -> std::result::Result<Module, ContextError> {
    let mut m = Module::with_crate("gage")?;

    scan::register(&mut m)?;
    config::register(&mut m)?;
    query::register(&mut m)?;
    db::register(&mut m)?;
    llm::register(&mut m)?;
    template::register(&mut m)?;

    Ok(m)
}

pub(crate) fn io_module() -> std::result::Result<Module, ContextError> {
    io::module()
}

pub(crate) fn stats_module() -> std::result::Result<Module, ContextError> {
    stats::module()
}

pub(crate) fn json_module() -> std::result::Result<Module, ContextError> {
    json::module()
}

pub(crate) fn types_module() -> std::result::Result<Module, ContextError> {
    scan::types_module()
}
