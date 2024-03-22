use std::{fs::File, io::BufReader, path::Path};

use anyhow::{bail, Result};

use crate::args::{SortBy, SortByField, SortByOrder};

/// Annotations of a run of a binary.
#[derive(Default)]
pub struct Run {
    // The name of the run, if any. This is purely for human readability purposes.
    pub name: String,
    /// The symbols that were hit and their instruction count.
    pub symbols: Vec<AnnotatedSymbol>,
    /// The total number of IR for this run.
    pub total_ir: u64,
}

impl Run {
    /// Create a new run.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new run with a name.
    pub fn new_named(name: String) -> Self {
        Self {
            name,
            ..Default::default()
        }
    }

    /// Add an IR count for the given symbol in the run.
    ///
    /// This may be called multiple times with the same symbol. Due to inlining, a symbol may end
    /// up in different files at different lines. This function _adds_ the IR count each time.
    ///
    /// ```
    /// # use callgrind_differ::runs::Run;
    /// let mut run = Run::new();
    /// run.add_ir("foo", 12);
    /// run.add_ir("foo", 24);
    /// assert_eq!(run.symbols.iter().find(|sym| sym.name == "foo").unwrap().ir, 36);
    /// ```
    pub fn add_ir(&mut self, symbol: &str, ir: u64) {
        if let Some(ref mut symbol) = self.symbols.iter().find(|sym| sym.name == symbol) {
            symbol.ir += ir;
        } else {
            self.symbols.push(AnnotatedSymbol {
                name: symbol.to_string(),
                ir,
            });
        }
    }

    /// Load a run from a `callgrind_annotate` output file.
    pub fn from_callgrind_annotate_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        crate::callgrind::parse(BufReader::new(File::open(path)?))
    }
}

/// The annotation records of multiple runs.
///
/// The annotations do make sense only if they all refer to the same binary (though it may be at
/// different stages of development).
pub struct Records {
    /// The names of the runs, if any. This is purely for human readability purposes.
    ///
    /// In case a name is unknown or unset, a blank string is inserted. The length of `run_names`
    /// must match that of any [`RecordsSymbol`] in [`Self::symbols`].
    pub run_names: Vec<String>,
    /// The total IR of each run.
    pub runs_total_irs: Vec<u64>,
    /// The symbols and their IR count for each run.
    pub symbols: Vec<RecordsSymbol>,
}

impl Records {
    /// Create a new records, ready to insert annotated runs.
    pub fn new() -> Self {
        Self {
            run_names: vec![],
            symbols: vec![],
        }
    }

    /// Add annotations about a run to the records.
    pub fn add_run(&mut self, run: Run) {
        self.assert_invariants();

        for run_symbol in run.symbols {
            // Add an `irs` entry for each symbol.
            if let Some(ref mut symbol) = self
                .symbols
                .iter()
                .find(|symbol| symbol.name == run_symbol.name)
            {
                symbol.irs.push(run_symbol.ir);
            } else {
                // If we can't find the symbol, we have to create it. However, we must already push
                // `self.n_runs()` zeroes into it to account for previous runs.
                let mut new_symbol = RecordsSymbol {
                    name: run_symbol.name,
                    irs: vec![0; self.n_runs()],
                };
                new_symbol.irs.push(run_symbol.ir);
                self.symbols.push(new_symbol);
            }
        }

        // Push the name of the run, this will update [`Self::n_runs`].
        self.run_names.push(run.name);
        self.runs_total_irs.push(run.total_ir);

        let n_runs = self.n_runs();
        // Add a 0 to each symbol that was not hit by the run.
        for ref mut symbol in &mut self.symbols {
            if symbol.irs.len() != n_runs {
                symbol.irs.push(0);
            }
        }

        // As long as the invariants were held before, they should hold now.
        self.assert_invariants();
    }

    /// Sort the symbols according to the given order.
    ///
    /// See [`SortBy`] for more details.
    pub fn sort(&mut self, by: SortBy) -> Result<()> {
        let n = self.n_runs();
        match by.field {
            SortByField::Symbol => self.symbols.sort_by(|a, b| a.name.cmp(&b.name)),
            SortByField::FirstIR => self.symbols.sort_by(|a, b| a.irs[0].cmp(&b.irs[0])),
            SortByField::LastIR => self.symbols.sort_by(|a, b| a.irs[n - 1].cmp(&b.irs[n - 1])),
            SortByField::ColumnIR(x) if (x as usize) < n => self
                .symbols
                .sort_by(|a, b| a.irs[x as usize].cmp(&b.irs[x as usize])),
            SortByField::ColumnIR(x) => bail!("Invalid column {x} (got {n} columns)"),
        }

        if matches!(by.order, SortByOrder::Descending) {
            self.symbols.reverse();
        }

        Ok(())
    }

    /// Return the number of runs that have been stored in `Self`.
    pub fn n_runs(&self) -> usize {
        self.run_names.len()
    }

    /// Make sure that the invariants of the structure are held.
    ///
    /// This function functionally does nothing, but checking integrity is cheap and may save time
    /// in debugging.
    ///
    /// # Panics
    /// This function panics if an invariant is broken.
    pub fn assert_invariants(&self) {
        let n_runs = self.n_runs();

        // The number of runs contained in `self.run_names` must match that of
        // `self.runs_total_irs`.
        if n_runs != self.runs_total_irs.len() {
            panic!(
                "Invalid # of total irs (got {}, expected{n_runs})",
                self.runs_total_irs.len()
            );
        }

        // The number of runs contained in `self.run_names` must match that of each symbol in
        // `self.symbols`.
        for symbol in &self.symbols {
            if symbol.irs.len() != n_runs {
                panic!(
                    "Invalid # of runs for symbol {} (got {}, expected {n_runs})",
                    symbol.name,
                    symbol.irs.len()
                );
            }
        }
    }
}

/// A symbol in the file and its IR count for a single run.
#[derive(Default)]
pub struct AnnotatedSymbol {
    /// The name of the symbol.
    pub name: String,
    /// The instruction count for that run.
    pub ir: u64,
}

/// A symbol in the file and its IR counts for multiple runs.
#[derive(Default)]
pub struct RecordsSymbol {
    /// The name of the symbol.
    pub name: String,
    /// The instruction counts for different runs.
    ///
    /// When storing a collection of [`RecordsSymbol`]s, care must be taken in order to not assign
    /// an IR count of one run to another (i.e. before inserting, the length of `irs` for each
    /// [`RecordsSymbol`] in the collection must be the same).
    pub irs: Vec<u64>,
}
