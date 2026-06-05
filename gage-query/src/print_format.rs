use std::str::FromStr;

use arrow::csv::writer::WriterBuilder;
use arrow::json::{ArrayWriter, LineDelimitedWriter};
use arrow::record_batch::RecordBatch;
use arrow::util::display::ArrayFormatter;
use arrow::util::pretty::pretty_format_batches;
use datafusion::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum PrintFormat {
    Table,
    Csv,
    Json,
    #[value(name = "ndjson")]
    NdJson,
    Yaml,
}

impl FromStr for PrintFormat {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        clap::ValueEnum::from_str(s, true)
    }
}

impl PrintFormat {
    pub fn print_batches(&self, batches: &[RecordBatch]) -> Result<()> {
        let batches: Vec<_> = batches
            .iter()
            .filter(|b| b.num_rows() > 0)
            .cloned()
            .collect();

        match self {
            Self::Table => {
                if batches.is_empty() {
                    return Ok(());
                }
                let formatted = pretty_format_batches(&batches)?;
                println!("{formatted}");
            }
            Self::Csv => {
                let mut writer = WriterBuilder::new()
                    .with_header(true)
                    .build(std::io::stdout());
                for batch in &batches {
                    writer.write(batch)?;
                }
            }
            Self::Json => {
                if !batches.is_empty() {
                    let mut buf: Vec<u8> = Vec::new();
                    let mut writer = ArrayWriter::new(&mut buf);
                    for batch in &batches {
                        writer.write(batch)?;
                    }
                    writer.finish()?;
                    println!("{}", String::from_utf8_lossy(&buf));
                }
            }
            Self::NdJson => {
                if !batches.is_empty() {
                    let mut writer = LineDelimitedWriter::new(std::io::stdout());
                    for batch in &batches {
                        writer.write(batch)?;
                    }
                    writer.finish()?;
                }
            }
            Self::Yaml => {
                let format_opts = arrow::util::display::FormatOptions::default();
                for batch in &batches {
                    let formatters: Vec<ArrayFormatter> = batch
                        .columns()
                        .iter()
                        .map(|c| ArrayFormatter::try_new(c.as_ref(), &format_opts))
                        .collect::<Result<_, _>>()?;

                    for row in 0..batch.num_rows() {
                        println!("---");
                        for (col_idx, field) in batch.schema().fields().iter().enumerate() {
                            let value = formatters
                                .get(col_idx)
                                .expect("formatters correspond to batch schema fields")
                                .value(row);
                            println!("{}: {value}", field.name());
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
