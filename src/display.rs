use itertools::Itertools;

use crate::args::{Args, RelativeTo, Show};
use crate::runs::{Records, RecordsSymbol};

pub fn display(config: &Args, records: &Records) {
    Displayer::new(config, records).display();
}

/// The width of the `percent_diff` column (`+ 12.345%`).
///
/// * 1 for the sign
/// * 3 for the integral part
/// * 3 for the decimal part
/// * 1 for the dot
/// * 1 for the symbol
///
/// When this is expressed as a ratio, this will create a shift if the ratio is 1000x or higher.
const PERCENTDIFF_WIDTH: u32 = 9;

/// The name of the "symbol" for the row that contains the total IR for runs.
const TOTAL_IR_ROW_NAME: &str = "Total IR";

/// Context for displaying a [`Records`].
struct Displayer<'a> {
    /// The program configuration.
    config: &'a Args,
    /// The records to display.
    records: &'a Records,
    /// The length of the longest symbol.
    max_symbol_width: u32,
    /// The length (in digits) of the highest `total_ir`.
    max_total_ir_width: u8,
    /// The width that a column takes in-between the ` | `.
    run_width: u32,
    /// The total width of a line.
    line_width: u32,
    /// The index of the reference column.
    reference_column: u32,
}

impl<'a> Displayer<'a> {
    /// Create a new [`Displayer`].
    fn new(config: &'a Args, records: &'a Records) -> Self {
        let mut ret = Self {
            config,
            records,
            max_symbol_width: get_max_symbol_length(records, config.all),
            max_total_ir_width: get_highest_total_ir_length(records),
            run_width: 0,
            line_width: 0,
            reference_column: 0,
        };
        ret.compute_widths();

        ret.reference_column = match &config.relative_to {
            RelativeTo::First => 0,
            RelativeTo::Last => (records.n_runs() - 1) as u32,
            RelativeTo::Previous => u32::MAX,
            RelativeTo::Column(x) => *x,
        };

        ret
    }

    /// Display the [`Records`] on the standard output.
    fn display(&self) {
        self.show_header();
        self.show_delimitation_line();
        self.show_total_ir_line();
        self.show_delimitation_line();
        for symbol in &self.records.symbols {
            if self.config.all || !symbol.irs.iter().all_equal() {
                self.show_symbol_row(symbol);
            }
        }
    }

    /// Show the header line.
    fn show_header(&self) {
        print!("Symbol");
        print_n(' ', self.max_symbol_width as usize - "Symbol".len());
        for (i, col_name) in self.records.run_names.iter().enumerate() {
            print!(" | ");
            if self.is_ref_column(i) {
                print_centered(col_name, self.max_total_ir_width as usize);
            } else {
                print_centered(col_name, self.run_width as usize);
            }
        }
        println!();
    }

    /// Show a `---+----+---` line as a horizontal separation.
    fn show_delimitation_line(&self) {
        print_n('-', self.max_symbol_width as usize);
        for i in 0..self.records.run_names.len() {
            print!("-+-");
            if self.is_ref_column(i) {
                print_n('-', self.max_total_ir_width as usize);
            } else {
                print_n('-', self.run_width as usize);
            }
        }
        println!();
    }

    /// Show the "Total IR" line.
    fn show_total_ir_line(&self) {
        print_left(TOTAL_IR_ROW_NAME, self.max_symbol_width as usize);
        for (i, ir) in self.records.runs_total_irs.iter().enumerate() {
            let s = ir.to_string();
            print!(" | ");
            if self.is_ref_column(i) {
                print_right(&s, self.max_total_ir_width as usize);
            } else {
                let reference_ir = self.get_reference_total_ir_for(i);
                self.show_run_details(*ir, reference_ir);
            }
        }
        println!();
    }

    /// Display the row with details for a single symbol.
    fn show_symbol_row(&self, symbol: &RecordsSymbol) {
        print_left(&symbol.name, self.max_symbol_width as usize);
        for (i, ir) in symbol.irs.iter().enumerate() {
            print!(" | ");
            if self.is_ref_column(i) {
                // If it's the reference column, just print the IR count.
                self.show_symbol_ir(*ir);
            } else {
                let reference_ir = self.get_reference_ir_for(i, symbol);
                self.show_run_details(*ir, reference_ir);
            }
        }
        println!();
    }

    /// Display the columns (as per `--show`) with the given details.
    fn show_run_details(&self, ir: u64, reference_ir: u64) {
        for (i, x) in self.config.show.iter().enumerate() {
            if i != 0 {
                // Print a space between that value and the previous one.
                print!(" ");
            }
            match x {
                Show::IRCount => self.show_symbol_ir(ir),
                Show::PercentageDiff => self.show_symbol_percentdff(ir, reference_ir),
                Show::IRCountDiff => self.show_symbol_irdff(ir, reference_ir),
                Show::All => unreachable!(),
            }
        }
    }

    /// Display the IR count, correctly aligned.
    fn show_symbol_ir(&self, ir: u64) {
        let s = ir.to_string();
        print_right(&s, self.max_total_ir_width as usize);
    }

    /// Display the IR difference, correctly aligned.
    fn show_symbol_irdff(&self, ir: u64, reference_ir: u64) {
        let diff = ir.abs_diff(reference_ir);
        if diff == 0 {
            print_right("-", (self.max_total_ir_width + 1) as usize);
        } else if ir > reference_ir {
            // Increase, show red.
            print!("\x1B[31m+");
            let s = format!("{diff}");
            print_right(&s, self.max_total_ir_width as usize);
            print!("\x1B[0m");
        } else {
            // Decrease, show green
            print!("\x1B[32m-");
            let s = format!("{diff}");
            print_right(&s, self.max_total_ir_width as usize);
            print!("\x1B[0m");
        }
    }

    /// Display the IR percentage difference, correctly aligned.
    #[allow(clippy::unused_self)]
    fn show_symbol_percentdff(&self, ir: u64, reference_ir: u64) {
        let diff = ir.abs_diff(reference_ir);
        let percent = if reference_ir == 0 {
            100.0
        } else {
            (diff as f64) * 100.0 / (reference_ir as f64)
        };

        if diff == 0 {
            print_right("- ", PERCENTDIFF_WIDTH as usize);
        } else if reference_ir > ir {
            // Decrease, show green.
            print!("\x1B[32m-");
            let s = format!("{percent:7.3}%");
            print_right(&s, (PERCENTDIFF_WIDTH - 1) as usize);
            print!("\x1B[0m");
        } else {
            // Increase, show red
            if percent < 1000.0 {
                print!("\x1B[31m+");
                let s = format!("{percent:7.3}%");
                print_right(&s, (PERCENTDIFF_WIDTH - 1) as usize);
            } else {
                // Too high an increase, show as bold red ratio.
                print!("\x1B[31;1m");
                let ratio = percent / 100.0;
                let s = format!("{ratio:7.3}x");
                print_right(&s, PERCENTDIFF_WIDTH as usize);
            }
            print!("\x1B[0m");
        }
    }

    /// Compute the widths of `Self` that can't easily be initialized in [`Self::new`].
    ///
    /// A line will show like:
    /// ```no_compile
    /// <symbol> | <ir_ref> | <ir> <ir-diff> <%>
    ///                    ^^^^^^^^^^^^^^^^^^^^^ Repeated for each column other than the ref
    /// ```
    ///
    /// The `<ir>`, `<ir-diff>` and `<%>` fields will show only if they are selected via `--show`.
    fn compute_widths(&mut self) {
        let ir_len = self.max_total_ir_width as u32;

        let ir_ref = ir_len;
        let ir = if self.config.show.contains(&Show::IRCount) {
            ir_len
        } else {
            0
        };
        let ir_diff = if self.config.show.contains(&Show::IRCountDiff) {
            ir_len + 1 // Account for the `+` or `-` sign.
        } else {
            0
        };
        let percent_diff = if self.config.show.contains(&Show::PercentageDiff) {
            PERCENTDIFF_WIDTH
        } else {
            0
        };

        self.run_width = ir + // <ir>
             ir_diff +        // <ir-diff>
             percent_diff +   // <%>
             ((self.config.show.len() - 1) as u32); // spaces

        self.line_width = self.max_symbol_width + // <symbol>
            3 +                 // ` | `
            ir_ref +            // <ir_ref>
            (3 +                // ` | `
             self.run_width) *  // <ir> <ir-diff> <%>
            ((self.records.n_runs() - 1) as u32); // For each column other than the reference one.
    }

    /// Return whether the column at index `i` is the reference column.
    ///
    /// If the relative is set to previous, the reference column is considered to be the first.
    fn is_ref_column(&self, i: usize) -> bool {
        (i as u32) == self.reference_column || (i == 0 && self.reference_column == u32::MAX)
    }

    /// Get the reference IR count for the given symbol and run.
    fn get_reference_ir_for(&self, i: usize, symbol: &RecordsSymbol) -> u64 {
        if self.reference_column == u32::MAX {
            symbol.irs[i - 1]
        } else {
            symbol.irs[self.reference_column as usize]
        }
    }

    /// Get the reference total IR count for the given run.
    fn get_reference_total_ir_for(&self, i: usize) -> u64 {
        if self.reference_column == u32::MAX {
            self.records.runs_total_irs[i - 1]
        } else {
            self.records.runs_total_irs[self.reference_column as usize]
        }
    }
}

/// Get the length of the longest symbol.
///
/// If `display_all` (the `-a` option) is disabled, this will only take into account symbols for
/// which the IR count is not the same throughout all runs.
///
/// If there is no symbol to display, this returns 0.
fn get_max_symbol_length(records: &Records, display_all: bool) -> u32 {
    const TOTAL_IR_LEN: u32 = TOTAL_IR_ROW_NAME.len() as u32;

    (records
        .symbols
        .iter()
        .filter(|record| display_all || !record.irs.iter().all_equal())
        .map(|record| record.name.len())
        .max()
        .unwrap_or(0) as u32)
        .max(TOTAL_IR_LEN)
}

/// Get the length in digits of the highest `total_ir`.
fn get_highest_total_ir_length(records: &Records) -> u8 {
    records
        .runs_total_irs
        .iter()
        .max()
        .map_or(1, |x| (x.ilog10() + 1) as u8)
}

/// Print the string aligned to the right within the given width.
///
/// Spaces are used as padding. Truncate if needed.
fn print_right(s: &str, width: usize) {
    if s.len() > width {
        for c in s.chars().take(width) {
            print!("{c}");
        }
    } else {
        let padding = width - s.len();
        print_n(' ', padding);
        print!("{s}");
    }
}

/// Print the string aligned to the left within the given width.
///
/// Spaces are used as padding. Truncate if needed.
fn print_left(s: &str, width: usize) {
    if s.len() > width {
        for c in s.chars().take(width) {
            print!("{c}");
        }
    } else {
        let padding = width - s.len();
        print!("{s}");
        print_n(' ', padding);
    }
}

/// Print the string centered within the given width.
///
/// Spaces are used as padding. Truncate if needed.
fn print_centered(s: &str, width: usize) {
    if s.len() > width {
        for c in s.chars().take(width) {
            print!("{c}");
        }
    } else {
        let padding = width - s.len();
        print_n(' ', padding / 2);
        print!("{s}");
        print_n(' ', padding / 2 + padding % 2);
    }
}

/// Print `c` `n` times.
fn print_n(c: char, n: usize) {
    for _ in 0..n {
        print!("{c}");
    }
}
