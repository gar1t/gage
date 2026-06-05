use std::io::{self, ErrorKind};

use cliclack as cli;
use console::style;

struct GageTheme;

impl cli::Theme for GageTheme {
    fn format_footer_with_message(&self, state: &cli::ThemeState, message: &str) -> String {
        format!(
            "{}\n",
            self.bar_color(state).apply_to(match state {
                cli::ThemeState::Active => format!("└  {message}"),
                cli::ThemeState::Cancel => "│\n└  Canceled".to_string(),
                cli::ThemeState::Submit => "│".to_string(),
                cli::ThemeState::Error(err) => format!("└  {err}"),
            })
        )
    }
}

pub enum DialogResult {
    Message(String),
}

impl<S: Into<String>> From<S> for DialogResult {
    fn from(s: S) -> Self {
        DialogResult::Message(s.into())
    }
}

/// Dialog-specific error type that distinguishes between user
/// cancellation (said "no"), Ctrl+C interrupts, and other errors.
pub enum DialogError {
    /// User explicitly declined a prompt (e.g. answered "no" to confirm).
    Canceled,
    /// Ctrl+C pressed during an interactive prompt. Cliclack already
    /// renders its own cancel UI so no outro is needed.
    Interrupted,
    /// The operation failed for a reason already surfaced earlier in
    /// the dialog (e.g. per-task errors during a scan). The message
    /// goes straight to the outro line — no preceding `log::error`,
    /// since the detail was already printed.
    Failed(String),
    /// Any other I/O error.
    Other(io::Error),
}

impl From<io::Error> for DialogError {
    fn from(e: io::Error) -> Self {
        if e.kind() == ErrorKind::Interrupted {
            DialogError::Interrupted
        } else {
            DialogError::Other(e)
        }
    }
}

pub fn install_theme() {
    cli::set_theme(GageTheme);
}

pub fn run<F>(title: &str, f: F)
where
    F: FnOnce() -> Result<DialogResult, DialogError>,
{
    cli::set_theme(GageTheme);

    // Ignore SIGINT for the duration of the dialog. The console crate
    // detects Ctrl+C as a 0x03 byte in raw mode and then calls
    // libc::raise(SIGINT). Without this, the raised signal kills the
    // process before our error handling runs
    let prev = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };

    cli::intro(style(title).bold()).unwrap();
    let result = f();

    // Restore previous SIGINT disposition
    unsafe {
        libc::signal(libc::SIGINT, prev);
    }

    handle_result(result);
}

pub async fn run_async<F, Fut>(title: &str, f: F)
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<DialogResult, DialogError>>,
{
    cli::set_theme(GageTheme);

    let prev = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };

    cli::intro(style(title).bold()).unwrap();
    let result = f().await;

    unsafe {
        libc::signal(libc::SIGINT, prev);
    }

    handle_result(result);
}

fn handle_result(result: Result<DialogResult, DialogError>) {
    match result {
        Ok(DialogResult::Message(msg)) => {
            cli::outro(style(msg).green().bright()).unwrap();
        }
        Err(DialogError::Interrupted) => {
            // Cliclack already showed cancel UI — nothing more to print
        }
        Err(DialogError::Canceled) => {
            cli::outro_cancel("Canceled").unwrap();
        }
        Err(DialogError::Failed(msg)) => {
            cli::outro_cancel(msg).unwrap();
            std::process::exit(1);
        }
        Err(DialogError::Other(e)) => {
            let width = console::Term::stderr().size().1 as usize;
            let wrap_width = width.saturating_sub(4).max(40);
            let wrapped = textwrap::fill(&format!("{e}"), wrap_width)
                .lines()
                .map(|l| style(l).red().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            cli::log::error(wrapped).unwrap();
            cli::outro_cancel("Canceled due to errors").unwrap();
            std::process::exit(1);
        }
    }
}
