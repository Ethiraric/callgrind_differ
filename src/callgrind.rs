use itertools::Itertools;

use crate::runs::Run;

/// Parse the total IR line.
///
/// This line is just after the `Ir` block and starts with the total `Ir` count. Numbers are
/// "delimited" with commas since they are large (e.g.: 14,418,621,168).
fn parse_total_ir_line(line: &str) -> u64 {
    let word = line.trim().split(' ').next().unwrap();
    let count = word
        .chars()
        // This filter ignore commas.
        .filter_map(|c| c.to_digit(10))
        // This is akin to `str::parse::<u64>`.
        .fold(0, |sum, digit| sum * 10 + u64::from(digit));
    count
}

/// Parse an IR line for a particular symbol.
///
/// The line is of the form:
/// ```no_compile
/// <ir> (xx.xx%) <loc>:<sym> [<file>]
/// ```
///
/// There may be leading spaces to `ir`, spaces in the percentage and even in `loc`.
fn parse_fn_ir_line(line: &str) -> (String, u64) {
    // We ignore empty words (leading and trailing spaces as well).
    let mut words = line.trim().split(' ').filter(|word| !word.is_empty());
    // First word is `<ir>`.
    let ir_str = words.next().unwrap();
    let ir_str = ir_str
        .chars()
        .filter_map(|c| c.to_digit(10))
        .fold(0, |sum, digit| sum * 10 + u64::from(digit));

    // We then skip until the word ends with `)`, effectively skipping over the percentage.
    let words = words.skip_while(|word| !word.ends_with(')')).skip(1);
    // We then take words until one starts with `[`. This takes both `<loc>:<sym>`.
    // Joining with space allows us to rebuild constructs such as:
    // ```
    // <yaml_rust2::parser::Event as core::cmp::PartialEq>::eq`
    //                           ^  ^
    //                      These spaces are a pain
    // ```
    let loc = words.take_while(|word| !word.starts_with('[')).join(" ");
    // We ignore every character until we reach the `:` that precedes `<sym>` and consume that one
    // as well. Hurray, we found our symbol.
    let loc = loc.chars().skip_while(|c| *c != ':').skip(1).collect();

    (loc, ir_str)
}

/// Parse a `callgrind_annotate` file and return a `Run` from it.
pub fn parse<R: std::io::BufRead>(input: R) -> Run {
    let mut run = Run::new();
    let mut lines = input
        .lines()
        .map_while(std::result::Result::ok)
        .skip_while(|line| !line.starts_with("Ir"))
        .skip(2);
    run.total_ir = parse_total_ir_line(&lines.next().unwrap());

    for (symbol, ir) in lines
        .skip_while(|line| !line.starts_with("Ir"))
        .skip(2)
        .take_while(|line| line.trim().chars().next().unwrap_or('\0').is_ascii_digit())
        .map(|line| parse_fn_ir_line(&line))
    {
        run.add_ir(&symbol, ir);
    }

    run
}
