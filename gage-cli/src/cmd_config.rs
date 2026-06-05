use tabled::{
    Table,
    settings::{Style, object::Columns},
};

use crate::style;

pub fn run() {
    let settings_path = gage_core::config::display_settings_path();
    let rows = [["Settings", settings_path.as_str()]];
    let table = Table::from_iter(rows)
        .with(Style::rounded().horizontals([]))
        .modify(Columns::first(), style::dim())
        .to_string();
    println!("{table}");
}
