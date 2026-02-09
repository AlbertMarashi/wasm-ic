use anyhow::{Context, Result};
use clap::Parser;
use std::fs;
use std::path::PathBuf;
use wasm_ic::*;

#[derive(Parser)]
#[command(name = "wasm-compile", about = "Compile WAT to hex files for wasm-ic")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand)]
enum Command {
    /// Compile a WAT file to hex files for the hardware core
    Compile {
        /// Input WAT file
        input: PathBuf,
        /// Output directory for hex files
        #[arg(long, default_value = ".")]
        out_dir: PathBuf,
    },
    /// Generate a SystemVerilog header with test tasks for all WAT files
    GenTests {
        /// Directory containing WAT files
        #[arg(long)]
        wat_dir: PathBuf,
        /// Output .svh file path
        #[arg(long)]
        output: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Command::Compile { input, out_dir } => {
            let wat_source = fs::read_to_string(input)
                .with_context(|| format!("reading {}", input.display()))?;
            let wasm_bytes = wat::parse_str(&wat_source)
                .with_context(|| format!("compiling WAT from {}", input.display()))?;

            let body_bytes =
                extract_function_body(&wasm_bytes).context("extracting function body")?;
            let branch_table =
                compute_branch_table(&body_bytes).context("computing branch table")?;
            let expected = run_with_wasmtime(&wasm_bytes).context("running with wasmtime")?;

            fs::create_dir_all(out_dir)?;
            write_prog_hex(&out_dir.join("prog.hex"), &body_bytes)?;
            write_branch_hex(&out_dir.join("branch.hex"), &branch_table)?;
            write_expected(&out_dir.join("expected.txt"), expected)?;

            let name = input.file_stem().unwrap_or_default().to_string_lossy();
            println!(
                "{}: {} bytes, {} branch entries, expected={}",
                name,
                body_bytes.len(),
                branch_table.len(),
                expected
            );
        }
        Command::GenTests { wat_dir, output } => {
            let mut wat_files: Vec<PathBuf> = fs::read_dir(wat_dir)
                .with_context(|| format!("reading directory {}", wat_dir.display()))?
                .filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().map_or(false, |ext| ext == "wat"))
                .filter(|p| {
                    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
                    !SKIP_FILES.contains(&stem.as_ref())
                })
                .collect();
            wat_files.sort();

            let mut tests = Vec::new();
            for path in &wat_files {
                let info = compile_wat_file(path)
                    .with_context(|| format!("compiling {}", path.display()))?;
                println!(
                    "  {}: {} bytes, {} branches, expected={}",
                    info.name,
                    info.body_bytes.len(),
                    info.branch_table.len(),
                    info.expected
                );
                tests.push(info);
            }

            let svh = generate_svh(&tests);
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(output, &svh).with_context(|| format!("writing {}", output.display()))?;

            println!(
                "Generated {} with {} WAT test(s)",
                output.display(),
                tests.len()
            );
        }
    }

    Ok(())
}
