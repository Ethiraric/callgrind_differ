#![allow(clippy::cast_precision_loss)]

use anyhow::{bail, Result};
use clap::Parser;

use crate::runs::{Records, Run};
use args::Args;

mod args;
mod callgrind;
mod runs;

fn print_diff(config: &Args, name: &str, old: u64, new: u64, name_maxlen: usize) {
    if name != "Total IR" && !config.all && old == new {
        return;
    }

    print!("{name}");
    for _ in name.len()..name_maxlen {
        print!(" ");
    }

    let diff = old.abs_diff(new);
    let percent = if old == 0 {
        100.0
    } else {
        (diff as f64) * 100.0 / (old as f64)
    };

    if diff == 0 {
        print!(" {old:12} |                          | {new:12}");
    } else if old > new {
        print!(" {old:12} | \x1B[32m-{diff:>12} ({percent:7.3}%)\x1B[0m | {new:12}");
    } else if percent < 1000.0 {
        print!(" {old:12} | \x1B[31m+{diff:>12} ({percent:7.3}%)\x1B[0m | {new:12}");
    } else {
        let ratio = percent / 100.0;
        print!(" {old:12} | \x1B[31;1m+{diff:>12} ({ratio:7.3}x)\x1B[0m | {new:12}");
    }
    println!();
}

/// Transforms an array of record files into a [`Records`].
///
/// If the files are CSVs, then they are loaded as multiple runs. Otherwise, they are loaded as a
/// single `callgrind_annotate` output file. Runs are loaded in order.
fn inputs_to_records(inputs: &[String]) -> Result<Records> {
    let mut records = Records::new();
    for input in inputs {
        if input.ends_with(".csv") {
            todo!("CSV Parsing");
        } else {
            records.add_run(Run::from_callgrind_annotate_file(input)?);
        }
    }
    Ok(records)
}

fn main() -> Result<()> {
    let config = Args::parse().validated()?;
    let records = inputs_to_records(&config.inputs)?;
    if records.n_runs() == 0 {
        bail!("No input run");
    }

    let maxlen = old
        .fn_ir
        .iter()
        .map(|(n, _)| n.len())
        .max()
        .unwrap()
        .max(new.fn_ir.iter().map(|(n, _)| n.len()).max().unwrap());

    print_diff(&config, "Total IR", old.total_ir, new.total_ir, maxlen);
    for _ in 0..(maxlen + 36 + 8 + 11) {
        print!("-");
    }
    println!();

    for (name, ir) in &old.fn_ir {
        if let Some((_, ir_new)) = new.fn_ir.iter().find(|(n, _)| name == n) {
            print_diff(&config, name, *ir, *ir_new, maxlen);
        }
    }

    for (name, ir) in &old.fn_ir {
        if !new.fn_ir.iter().any(|(n, _)| name == n) {
            print_diff(&config, name, *ir, 0, maxlen);
        }
    }

    for (name, new_ir) in &new.fn_ir {
        if !old.fn_ir.iter().any(|(n, _)| name == n) {
            print_diff(&config, name, 0, *new_ir, maxlen);
        }
    }

    Ok(())
}
