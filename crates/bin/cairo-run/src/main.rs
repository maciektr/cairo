//! Compiles and runs a Cairo program.

use std::path::{Path, PathBuf};

use anyhow::{Context, Ok};
use cairo_lang_compiler::db::RootDatabase;
use cairo_lang_compiler::diagnostics::DiagnosticsReporter;
use cairo_lang_compiler::project::{check_compiler_path, setup_project};
use cairo_lang_diagnostics::ToOption;
use cairo_lang_runner::profiling::ProfilingInfoProcessor;
use cairo_lang_runner::short_string::as_cairo_short_string;
use cairo_lang_runner::{SierraCasmRunner, StarknetState};
use cairo_lang_sierra_generator::db::SierraGenGroup;
use cairo_lang_sierra_generator::program_generator::SierraProgramWithDebug;
use cairo_lang_sierra_generator::replace_ids::{DebugReplacer, SierraIdReplacer};
use cairo_lang_starknet::contract::get_contracts_info;
use cairo_lang_utils::arc_unwrap_or_clone;
use clap::Parser;

/// Command line args parser.
/// Exits with 0/1 if the input is formatted correctly/incorrectly.
#[derive(Parser, Debug)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    /// The file to compile and run.
    path: PathBuf,
    /// Whether path is a single file.
    #[arg(short, long)]
    single_file: bool,
    /// Allows the compilation to succeed with warnings.
    #[arg(long)]
    allow_warnings: bool,
    /// In cases where gas is available, the amount of provided gas.
    #[arg(long)]
    available_gas: Option<usize>,
    /// Whether to print the memory.
    #[arg(long, default_value_t = false)]
    print_full_memory: bool,
    /// Whether to run the profiler.
    #[arg(long, default_value_t = false)]
    run_profiler: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Check if args.path is a file or a directory.
    check_compiler_path(args.single_file, &args.path)?;

    let db = &mut RootDatabase::builder().detect_corelib().build()?;

    let main_crate_ids = setup_project(db, Path::new(&args.path))?;

    let mut reporter = DiagnosticsReporter::stderr();
    if args.allow_warnings {
        reporter = reporter.allow_warnings();
    }
    if reporter.check(db) {
        anyhow::bail!("failed to compile: {}", args.path.display());
    }

    let SierraProgramWithDebug { program: sierra_program, debug_info } = arc_unwrap_or_clone(
        db.get_sierra_program(main_crate_ids.clone())
            .to_option()
            .with_context(|| "Compilation failed without any diagnostics.")?,
    );
    let replacer = DebugReplacer { db };
    if args.available_gas.is_none() && sierra_program.requires_gas_counter() {
        anyhow::bail!("Program requires gas counter, please provide `--available-gas` argument.");
    }

    let contracts_info = get_contracts_info(db, main_crate_ids, &replacer)?;
    let sierra_program = replacer.apply(&sierra_program);

    let runner = SierraCasmRunner::new(
        sierra_program.clone(),
        if args.available_gas.is_some() { Some(Default::default()) } else { None },
        contracts_info,
        args.run_profiler,
    )
    .with_context(|| "Failed setting up runner.")?;
    let result = runner
        .run_function_with_starknet_context(
            runner.find_function("::main")?,
            &[],
            args.available_gas,
            StarknetState::default(),
        )
        .with_context(|| "Failed to run the function.")?;

    if args.run_profiler {
        let profiling_info_processor = ProfilingInfoProcessor::new(
            Some(db),
            sierra_program,
            debug_info.statements_locations.get_statements_functions_map(db),
        );
        match result.profiling_info {
            Some(raw_profiling_info) => {
                let profiling_info = profiling_info_processor.process(&raw_profiling_info);
                println!("Profiling info:\n{}", profiling_info);
            }
            None => println!("Warning: Profiling info not found."),
        }
    }

    match result.value {
        cairo_lang_runner::RunResultValue::Success(values) => {
            println!("Run completed successfully, returning {values:?}")
        }
        cairo_lang_runner::RunResultValue::Panic(values) => {
            print!("Run panicked with [");
            for value in &values {
                match as_cairo_short_string(value) {
                    Some(as_string) => print!("{value} ('{as_string}'), "),
                    None => print!("{value}, "),
                }
            }
            println!("].")
        }
    }
    if let Some(gas) = result.gas_counter {
        println!("Remaining gas: {gas}");
    }
    if args.print_full_memory {
        print!("Full memory: [");
        for cell in &result.memory {
            match cell {
                None => print!("_, "),
                Some(value) => print!("{value}, "),
            }
        }
        println!("]");
    }
    Ok(())
}
