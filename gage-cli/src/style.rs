use tabled::settings::Color;

pub fn spinner(message: &str) -> indicatif::ProgressBar {
    let spinner = indicatif::ProgressBar::new_spinner();
    spinner
        .set_style(indicatif::ProgressStyle::with_template("{spinner:.magenta}  {msg}").unwrap());
    spinner.set_message(message.to_string());
    spinner.enable_steady_tick(std::time::Duration::from_millis(80));
    spinner
}

pub fn dim() -> Color {
    Color::new("\x1b[2m", "\x1b[22m")
}

pub fn dim_italic() -> Color {
    Color::new("\x1b[2;3m", "\x1b[22;23m")
}
