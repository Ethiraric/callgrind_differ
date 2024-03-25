#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_lossless
)]

use std::path::Path;

use anyhow::{bail, Result};
use clap::Parser;

use crate::{
    args::{Args, RelativeTo, SortByField},
    display::display,
    runs::{Records, Run},
};

mod args;
mod callgrind;
mod display;
mod runs;

/// Parse inputs from the configuration into a [`Records`].
///
/// If the files are CSVs, then they are loaded as multiple runs. Otherwise, they are loaded as a
/// single `callgrind_annotate` output file. Runs are loaded in order.
fn parse_records(config: &Args) -> Result<Records> {
    let mut records = Records::new();
    for input in &config.inputs {
        if Path::new(input)
            .extension()
            .map_or(false, |ext| ext.eq_ignore_ascii_case("csv"))
        {
            todo!("CSV Parsing");
        } else {
            records.add_run(Run::from_callgrind_annotate_file(
                input,
                &config.string_replace,
            )?);
        }
    }
    Ok(records)
}

fn main() -> Result<()> {
    let config = Args::parse().validated()?;
    let mut records = parse_records(&config)?;
    if records.n_runs() == 0 {
        bail!("No input run");
    }
    if let RelativeTo::Column(x) = &config.relative_to {
        if (*x as usize) >= records.n_runs() {
            bail!("--relative-to column index out of range");
        }
    }
    if let SortByField::ColumnIR(x) = &config.sort_by.field {
        if (*x as usize) >= records.n_runs() {
            bail!("--sort-by column index out of range");
        }
    }

    records.sort(config.sort_by)?;
    display(&config, &records);

    Ok(())
}
