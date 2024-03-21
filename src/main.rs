#![allow(clippy::cast_precision_loss)]

use std::io::BufRead;

use anyhow::Result;
use itertools::Itertools;

static mut SKIP_UNCHANGED_LINES: bool = true;

#[derive(Debug)]
struct CallgrindAnnotation {
    total_ir: u64,
    fn_ir: Vec<(String, u64)>,
}

fn parse_total_ir_line(line: &str) -> u64 {
    let word = line.trim().split(' ').next().unwrap();
    let count = word
        .chars()
        .filter_map(|c| c.to_digit(10))
        .fold(0, |sum, digit| sum * 10 + u64::from(digit));
    count
}

fn parse_fn_ir_line(line: &str) -> (String, u64) {
    let mut words = line.trim().split(' ').filter(|word| !word.is_empty());
    let ir_str = words.next().unwrap();
    let ir_str = ir_str
        .chars()
        .filter_map(|c| c.to_digit(10))
        .fold(0, |sum, digit| sum * 10 + u64::from(digit));

    let words = words.skip_while(|word| !word.ends_with(')')).skip(1);
    let loc = words.take_while(|word| !word.starts_with('[')).join(" ");
    let loc = loc.chars().skip_while(|c| *c != ':').skip(1).collect();
    (loc, ir_str)
}

fn parse(file: &str) -> Result<CallgrindAnnotation> {
    let mut lines = std::io::BufReader::new(std::fs::File::open(file)?)
        .lines()
        .map_while(std::result::Result::ok)
        .skip_while(|line| !line.starts_with("Ir"))
        .skip(2);
    let total_ir = parse_total_ir_line(&lines.next().unwrap());
    let fn_groups = lines
        .skip_while(|line| !line.starts_with("Ir"))
        .skip(2)
        .take_while(|line| line.trim().chars().next().unwrap_or('\0').is_ascii_digit())
        .map(|s| parse_fn_ir_line(&s))
        .sorted_unstable_by(|a, b| a.0.cmp(&b.0))
        .group_by(|(name, _)| name.clone());
    let fn_ir = fn_groups
        .into_iter()
        .map(|(name, lines)| (name, lines.map(|(_, ir)| ir).sum::<u64>()))
        .collect();

    Ok(CallgrindAnnotation { total_ir, fn_ir })
}

fn print_diff(name: &str, old: u64, new: u64, name_maxlen: usize) {
    if name != "Total IR" && unsafe { SKIP_UNCHANGED_LINES } && old == new {
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

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let (old, new) = if args.len() == 4 && args[1] == "-a" {
        unsafe { SKIP_UNCHANGED_LINES = false };
        (args[2].clone(), args[3].clone())
    } else if args.len() == 3 {
        (args[1].clone(), args[2].clone())
    } else {
        panic!();
    };

    let old = parse(&old).unwrap();
    let new = parse(&new).unwrap();
    let maxlen = old
        .fn_ir
        .iter()
        .map(|(n, _)| n.len())
        .max()
        .unwrap()
        .max(new.fn_ir.iter().map(|(n, _)| n.len()).max().unwrap());

    print_diff("Total IR", old.total_ir, new.total_ir, maxlen);
    for _ in 0..(maxlen + 36 + 8 + 11) {
        print!("-");
    }
    println!();

    for (name, ir) in &old.fn_ir {
        if let Some((_, ir_new)) = new.fn_ir.iter().find(|(n, _)| name == n) {
            print_diff(name, *ir, *ir_new, maxlen);
        }
    }

    for (name, ir) in &old.fn_ir {
        if !new.fn_ir.iter().any(|(n, _)| name == n) {
            print_diff(name, *ir, 0, maxlen);
        }
    }

    for (name, new_ir) in &new.fn_ir {
        if !old.fn_ir.iter().any(|(n, _)| name == n) {
            print_diff(name, 0, *new_ir, maxlen);
        }
    }
}
