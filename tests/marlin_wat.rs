use marlin::veryl::prelude::*;
use snafu::Whatever;
use wasm_ic::{compile_wat, compute_branch_table, extract_function_body, run_with_wasmtime};

#[veryl(src = "src/wasm_core_tb.veryl", name = "WasmCoreTb")]
pub struct WasmCoreTb;

fn tick(dut: &mut WasmCoreTb, prog: &[u8]) {
    // Provide ROM data for the current address (combinational read)
    let addr = dut.o_prog_addr as usize;
    dut.i_prog_data = if addr < prog.len() { prog[addr] } else { 0 };
    dut.i_clk = 0;
    dut.eval();
    // Re-provide ROM data in case address changed combinationally
    let addr = dut.o_prog_addr as usize;
    dut.i_prog_data = if addr < prog.len() { prog[addr] } else { 0 };
    dut.i_clk = 1;
    dut.eval();
    // After posedge: update data for the new address so it's ready
    let addr = dut.o_prog_addr as usize;
    dut.i_prog_data = if addr < prog.len() { prog[addr] } else { 0 };
    dut.eval();
}

fn do_reset(dut: &mut WasmCoreTb, prog: &[u8]) {
    // Veryl `reset` type is active-low: i_rst=0 asserts reset, i_rst=1 deasserts
    dut.i_rst = 0;
    dut.i_start = 0;
    dut.i_bt_wr_en = 0;
    dut.i_bt_wr_addr = 0;
    dut.i_bt_wr_data = 0;
    dut.i_mem_load_en = 0;
    dut.i_mem_load_addr = 0;
    dut.i_mem_load_data = 0;
    for _ in 0..4 {
        tick(dut, prog);
    }
    dut.i_rst = 1;
    tick(dut, prog);
}

fn run_wat_test(runtime: &VerylRuntime, name: &str, wat_source: &str) -> Result<(), Whatever> {
    let wasm = compile_wat(wat_source).expect("WAT compile failed");
    let body = extract_function_body(&wasm).expect("body extraction failed");
    let branches = compute_branch_table(&body).expect("branch table failed");
    let expected = run_with_wasmtime(&wasm).expect("wasmtime failed");

    let mut dut = runtime.create_model::<WasmCoreTb>()?;

    do_reset(&mut dut, &body);

    // Write branch table entries
    for entry in &branches {
        dut.i_bt_wr_en = 1;
        dut.i_bt_wr_addr = entry.source_pc;
        dut.i_bt_wr_data = entry.target_pc;
        tick(&mut dut, &body);
    }
    dut.i_bt_wr_en = 0;

    // Start execution
    dut.i_start = 1;
    tick(&mut dut, &body);
    dut.i_start = 0;

    // Run until halted or trap (max 200 cycles)
    for _ in 0..200 {
        tick(&mut dut, &body);
        if dut.o_halted != 0 || dut.o_trap != 0 {
            break;
        }
    }

    assert_eq!(dut.o_trap, 0, "{name}: trapped");
    assert_ne!(dut.o_halted, 0, "{name}: timed out, pc={}", dut.o_pc);
    assert_eq!(
        dut.o_stack_top as i32, expected,
        "{name}: got {} expected {}",
        dut.o_stack_top as i32, expected
    );

    Ok(())
}

#[test]
fn test_wat_add() -> Result<(), Whatever> {
    let runtime = VerylRuntime::new(VerylRuntimeOptions {
        call_veryl_build: true,
        ..Default::default()
    })?;
    run_wat_test(&runtime, "add", include_str!("wat/add.wat"))
}

#[test]
fn test_wat_expr() -> Result<(), Whatever> {
    let runtime = VerylRuntime::new(VerylRuntimeOptions {
        call_veryl_build: true,
        ..Default::default()
    })?;
    run_wat_test(&runtime, "expr", include_str!("wat/expr.wat"))
}

#[test]
fn test_wat_branch() -> Result<(), Whatever> {
    let runtime = VerylRuntime::new(VerylRuntimeOptions {
        call_veryl_build: true,
        ..Default::default()
    })?;
    run_wat_test(&runtime, "branch", include_str!("wat/branch.wat"))
}

#[test]
fn test_wat_if_else() -> Result<(), Whatever> {
    let runtime = VerylRuntime::new(VerylRuntimeOptions {
        call_veryl_build: true,
        ..Default::default()
    })?;
    run_wat_test(&runtime, "if_else", include_str!("wat/if_else.wat"))
}

#[test]
fn test_wat_memory() -> Result<(), Whatever> {
    let runtime = VerylRuntime::new(VerylRuntimeOptions {
        call_veryl_build: true,
        ..Default::default()
    })?;
    run_wat_test(&runtime, "memory", include_str!("wat/memory.wat"))
}
