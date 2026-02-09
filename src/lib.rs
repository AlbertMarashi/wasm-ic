use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::PathBuf;
use wasmparser::{Operator, Payload};

// ---------------------------------------------------------------------------
// Branch table computation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum BlockKind {
    Block,
    Loop,
    If,
}

#[derive(Debug)]
struct BlockInfo {
    kind: BlockKind,
    start_offset: usize,
    body_offset: usize,
    else_offset: Option<usize>,
}

/// A single branch table entry: source_pc -> target_pc
#[derive(Debug, Clone)]
pub struct BranchEntry {
    pub source_pc: u32,
    pub target_pc: u32,
}

#[derive(Debug)]
struct InstrRecord {
    offset: usize,
    kind: InstrKind,
}

#[derive(Debug)]
enum InstrKind {
    Block,
    Loop,
    If,
    Else,
    End,
    Br(u32),
    BrIf(u32),
    Other,
}

/// Compute branch table entries from raw function body bytes.
///
/// `body_bytes` is the raw bytecode of the function body (operators only,
/// no locals prefix). Offsets are relative to the start of body_bytes,
/// which corresponds to PC=0 in the hardware.
pub fn compute_branch_table(body_bytes: &[u8]) -> Result<Vec<BranchEntry>> {
    let instrs = collect_instructions(body_bytes)?;

    let mut entries = Vec::new();
    let mut block_end_map: Vec<Option<usize>> = vec![None; instrs.len()];
    let mut end_resolve_stack: Vec<usize> = Vec::new();

    for (i, instr) in instrs.iter().enumerate() {
        match instr.kind {
            InstrKind::Block | InstrKind::Loop | InstrKind::If => {
                end_resolve_stack.push(i);
            }
            InstrKind::End => {
                if let Some(start_idx) = end_resolve_stack.pop() {
                    block_end_map[start_idx] = Some(instr.offset);
                }
            }
            _ => {}
        }
    }

    let mut stack: Vec<(usize, BlockInfo)> = Vec::new();

    for (i, instr) in instrs.iter().enumerate() {
        match instr.kind {
            InstrKind::Block => {
                let body_offset = instr.offset + 2;
                stack.push((
                    i,
                    BlockInfo {
                        kind: BlockKind::Block,
                        start_offset: instr.offset,
                        body_offset,
                        else_offset: None,
                    },
                ));
            }
            InstrKind::Loop => {
                let body_offset = instr.offset + 2;
                stack.push((
                    i,
                    BlockInfo {
                        kind: BlockKind::Loop,
                        start_offset: instr.offset,
                        body_offset,
                        else_offset: None,
                    },
                ));
            }
            InstrKind::If => {
                let body_offset = instr.offset + 2;
                stack.push((
                    i,
                    BlockInfo {
                        kind: BlockKind::If,
                        start_offset: instr.offset,
                        body_offset,
                        else_offset: None,
                    },
                ));
            }
            InstrKind::Else => {
                if let Some((_idx, ref mut info)) = stack.last_mut() {
                    if info.kind == BlockKind::If {
                        info.else_offset = Some(instr.offset);
                        entries.push(BranchEntry {
                            source_pc: info.start_offset as u32,
                            target_pc: (instr.offset + 1) as u32,
                        });
                    }
                }
            }
            InstrKind::End => {
                let end_offset = instr.offset;
                let end_plus_one = end_offset + 1;

                if let Some((_, info)) = stack.pop() {
                    match info.kind {
                        BlockKind::If => {
                            if info.else_offset.is_some() {
                                entries.push(BranchEntry {
                                    source_pc: info.else_offset.unwrap() as u32,
                                    target_pc: end_plus_one as u32,
                                });
                            } else {
                                entries.push(BranchEntry {
                                    source_pc: info.start_offset as u32,
                                    target_pc: end_plus_one as u32,
                                });
                            }
                        }
                        BlockKind::Block | BlockKind::Loop => {}
                    }
                }
            }
            InstrKind::Br(depth) | InstrKind::BrIf(depth) => {
                let target_idx = stack.len().checked_sub(1 + depth as usize).ok_or_else(|| {
                    anyhow!(
                        "br depth {} exceeds block nesting at offset {}",
                        depth,
                        instr.offset
                    )
                })?;
                let (block_instr_idx, ref target_info) = stack[target_idx];

                let target_pc = match target_info.kind {
                    BlockKind::Loop => target_info.body_offset,
                    BlockKind::Block | BlockKind::If => {
                        let end_off = block_end_map[block_instr_idx].ok_or_else(|| {
                            anyhow!(
                                "no end found for block at offset {}",
                                target_info.start_offset
                            )
                        })?;
                        end_off + 1
                    }
                };

                entries.push(BranchEntry {
                    source_pc: instr.offset as u32,
                    target_pc: target_pc as u32,
                });
            }
            InstrKind::Other => {}
        }
    }

    Ok(entries)
}

/// Parse bytecode into instruction records with offsets.
fn collect_instructions(body_bytes: &[u8]) -> Result<Vec<InstrRecord>> {
    let mut records = Vec::new();
    let binary_reader = wasmparser::BinaryReader::new(body_bytes, 0);
    let mut reader = wasmparser::OperatorsReader::new(binary_reader);

    while !reader.eof() {
        let (op, offset) = reader.read_with_offset()?;

        let kind = match op {
            Operator::Block { .. } => InstrKind::Block,
            Operator::Loop { .. } => InstrKind::Loop,
            Operator::If { .. } => InstrKind::If,
            Operator::Else => InstrKind::Else,
            Operator::End => InstrKind::End,
            Operator::Br { relative_depth } => InstrKind::Br(relative_depth),
            Operator::BrIf { relative_depth } => InstrKind::BrIf(relative_depth),
            _ => InstrKind::Other,
        };

        records.push(InstrRecord { offset, kind });
    }

    Ok(records)
}

// ---------------------------------------------------------------------------
// WASM binary parsing: extract function body bytes
// ---------------------------------------------------------------------------

/// Extract the raw operator bytes of the first function in a WASM binary.
/// Returns bytes starting from the first operator (locals are skipped).
/// Replaces trailing `end` (0x0B) with `return` (0x0F) for the hardware.
pub fn extract_function_body(wasm_bytes: &[u8]) -> Result<Vec<u8>> {
    let parser = wasmparser::Parser::new(0);

    for payload in parser.parse_all(wasm_bytes) {
        let payload = payload?;
        if let Payload::CodeSectionEntry(body) = payload {
            let body_range = body.range();
            let ops_reader = body.get_operators_reader()?;
            let ops_offset = ops_reader.original_position();

            let start = ops_offset - body_range.start;
            let all_bytes = &wasm_bytes[body_range.start..body_range.end];
            let op_bytes = &all_bytes[start..];

            let mut bytes = op_bytes.to_vec();
            if let Some(last) = bytes.last_mut() {
                if *last == 0x0B {
                    *last = 0x0F;
                }
            }

            return Ok(bytes);
        }
    }

    Err(anyhow!("No code section found in WASM binary"))
}

// ---------------------------------------------------------------------------
// Wasmtime: execute and get expected result
// ---------------------------------------------------------------------------

/// Run a WASM module with wasmtime, calling exported `main() -> i32`.
pub fn run_with_wasmtime(wasm_bytes: &[u8]) -> Result<i32> {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::new(&engine, wasm_bytes)?;
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[])?;

    let main_fn = instance
        .get_typed_func::<(), i32>(&mut store, "main")
        .context("Could not find exported function 'main' with signature () -> i32")?;

    let result = main_fn.call(&mut store, ())?;
    Ok(result)
}

// ---------------------------------------------------------------------------
// Compile WAT source to WASM bytes
// ---------------------------------------------------------------------------

/// Compile WAT text source to WASM binary bytes.
pub fn compile_wat(source: &str) -> Result<Vec<u8>> {
    let wasm = wat::parse_str(source)?;
    Ok(wasm.to_vec())
}

// ---------------------------------------------------------------------------
// Hex file output
// ---------------------------------------------------------------------------

pub fn write_prog_hex(path: &PathBuf, bytes: &[u8]) -> Result<()> {
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        out.push_str(&format!("{:02X}", b));
        if i + 1 < bytes.len() {
            out.push('\n');
        }
    }
    out.push('\n');
    fs::write(path, &out).context("writing prog.hex")?;
    Ok(())
}

pub fn write_branch_hex(path: &PathBuf, entries: &[BranchEntry]) -> Result<()> {
    let mut out = String::new();
    for entry in entries {
        out.push_str(&format!(
            "{:08X} {:08X}\n",
            entry.source_pc, entry.target_pc
        ));
    }
    fs::write(path, &out).context("writing branch.hex")?;
    Ok(())
}

pub fn write_expected(path: &PathBuf, value: i32) -> Result<()> {
    fs::write(path, format!("{}\n", value)).context("writing expected.txt")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// SystemVerilog test generation
// ---------------------------------------------------------------------------

/// Files to skip (hardware doesn't support all opcodes yet)
pub const SKIP_FILES: &[&str] = &["loop"];

pub struct WatTestInfo {
    pub name: String,
    pub body_bytes: Vec<u8>,
    pub branch_table: Vec<BranchEntry>,
    pub expected: i32,
}

pub fn compile_wat_file(path: &PathBuf) -> Result<WatTestInfo> {
    let name = path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let wat_source =
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let wasm_bytes = wat::parse_str(&wat_source)
        .with_context(|| format!("compiling WAT from {}", path.display()))?;
    let body_bytes = extract_function_body(&wasm_bytes).context("extracting function body")?;
    let branch_table = compute_branch_table(&body_bytes).context("computing branch table")?;
    let expected = run_with_wasmtime(&wasm_bytes).context("running with wasmtime")?;

    Ok(WatTestInfo {
        name,
        body_bytes,
        branch_table,
        expected,
    })
}

pub fn generate_svh(tests: &[WatTestInfo]) -> String {
    let mut out = String::new();
    out.push_str("// Auto-generated by wasm-compile gen-tests. Do not edit.\n\n");

    for t in tests {
        out.push_str(&format!("task run_wat_{};\n", t.name));
        out.push_str("    do_reset();\n");

        for (i, b) in t.body_bytes.iter().enumerate() {
            out.push_str(&format!("    prog_rom[{}] = 8'h{:02X};\n", i, b));
        }

        for entry in &t.branch_table {
            out.push_str(&format!(
                "    bt_write(32'h{:08X}, 32'h{:08X});\n",
                entry.source_pc, entry.target_pc
            ));
        }

        out.push_str("    run_program();\n");
        out.push_str(&format!(
            "    check_wat(\"{}\", 32'sd{});\n",
            t.name, t.expected
        ));
        out.push_str("endtask\n\n");
    }

    out.push_str("task run_all_wat_tests;\n");
    for t in tests {
        out.push_str(&format!("    run_wat_{}();\n", t.name));
    }
    out.push_str("endtask\n");

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn compile_and_check(wat: &str, expected_result: i32, expected_branches: &[(u32, u32)]) {
        let wasm = wat::parse_str(wat).expect("WAT parse failed");
        let body = extract_function_body(&wasm).expect("body extraction failed");
        let branches = compute_branch_table(&body).expect("branch table failed");
        let result = run_with_wasmtime(&wasm).expect("wasmtime failed");

        assert_eq!(result, expected_result, "wasmtime result mismatch");

        let branch_pairs: Vec<(u32, u32)> = branches
            .iter()
            .map(|e| (e.source_pc, e.target_pc))
            .collect();
        assert_eq!(branch_pairs, expected_branches, "branch table mismatch");
    }

    #[test]
    fn test_add() {
        compile_and_check(
            r#"(module (func (export "main") (result i32)
                i32.const 10
                i32.const 20
                i32.add))"#,
            30,
            &[],
        );
    }

    #[test]
    fn test_expr() {
        compile_and_check(
            r#"(module (func (export "main") (result i32)
                i32.const 3
                i32.const 5
                i32.add
                i32.const 2
                i32.mul))"#,
            16,
            &[],
        );
    }

    #[test]
    fn test_sub() {
        compile_and_check(
            r#"(module (func (export "main") (result i32)
                i32.const 20
                i32.const 7
                i32.sub))"#,
            13,
            &[],
        );
    }

    #[test]
    fn test_block_br() {
        let wat = r#"(module (func (export "main") (result i32)
                block
                  br 0
                end
                i32.const 99))"#;
        let wasm = wat::parse_str(wat).expect("WAT parse failed");
        let body = extract_function_body(&wasm).expect("body extraction failed");
        let branches = compute_branch_table(&body).expect("branch table failed");
        let result = run_with_wasmtime(&wasm).expect("wasmtime failed");

        assert_eq!(result, 99);
        assert!(!branches.is_empty(), "should have branch entries");
        let br_entry = &branches[0];
        assert!(
            br_entry.target_pc > br_entry.source_pc,
            "br should jump forward"
        );
    }

    #[test]
    fn test_if_else() {
        let wat = r#"(module (func (export "main") (result i32)
                i32.const 1
                if (result i32)
                  i32.const 42
                else
                  i32.const 0
                end))"#;
        let wasm = wat::parse_str(wat).expect("WAT parse failed");
        let body = extract_function_body(&wasm).expect("body extraction failed");
        let branches = compute_branch_table(&body).expect("branch table failed");
        let result = run_with_wasmtime(&wasm).expect("wasmtime failed");

        assert_eq!(result, 42);
        assert_eq!(branches.len(), 2, "if/else should produce 2 branch entries");
        assert!(
            branches[1].target_pc > branches[0].target_pc,
            "else target should be past if target"
        );
    }
}
