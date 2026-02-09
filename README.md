# wasm-ic

A hardware WebAssembly processor implemented in [Veryl](https://veryl-lang.org/). The goal is
a chip (FPGA or ASIC) that natively executes WASM bytecode -- no software runtime, no JIT,
just silicon running WASM instructions directly.

## Status

Early development. The ALU, operand stack, instruction decoder, fetch unit, branch table,
and structured control flow are implemented and tested.

## Architecture

```
  ╔═══════════════════════════════════════════════════════════════════╗
  ║                         WASM IC Core                              ║
  ║                                                                   ║
  ║                        ┌──────────────────────────────┐           ║
  ║    ┌───────────┐       │          Decode              │           ║
  ║    │  Program  │       │  ┌─────────┐  ┌───────────┐  │           ║
  ║    │  Memory   │ [8]   │  │ opcode  │  │ control   │  │           ║
  ║    │  (ROM)    ├──────>│  │  byte   ├─>│ signals   ├──┼──┐        ║
  ║    └─────┬─────┘       │  └─────────┘  │ (14 out)  │  │  │        ║
  ║          ^             │               └───────────┘  │  │        ║
  ║      [32]│addr         └──────────────────────────────┘  │        ║
  ║          │                                               │        ║
  ║    ┌─────┴─────┐  [32]   ┌─────────────┐    [32]   ┌────┴────┐    ║
  ║    │           ├────────>│   Branch    ├────────>  │         │    ║
  ║    │   Fetch   │  instr  │   Table     │  target   │ Operand │    ║
  ║    │           │   pc    │   (RAM)     │    pc     │  Stack  │    ║
  ║    │  7-state  │<────────┤             │           │ TOS/NOS │    ║
  ║    │   FSM     │  [32]   └──────┬──────┘    [32]   │  cache  │    ║
  ║    │           │  target     ^  │           ┌─────>│         │    ║
  ║    └───────────┘  pc         │  │ loader    │result└────┬────┘    ║
  ║                              │  │ writes    │           │         ║
  ║         ╔════════════════════╧══╧═══╗  ┌────┴────┐  [32]│TOS      ║
  ║         ║   Off-chip Loader         ║  │  ALU    │  [32]│NOS      ║
  ║         ║  (validates, fills        ║  │ (comb.) │<─────┘         ║
  ║         ║   branch table + memory)  ║  │ 29 ops  │                ║
  ║         ╚═══════════════════════════╝  └─────────┘                ║
  ║                                                                   ║
  ║    - - - - - - - - - - -  TODO  - - - - - - - - - - - - - -       ║
  ║    ┊ Call Stack ┊   ┊ Linear Memory ┊   ┊ Top-level Core  ┊       ║
  ║    ┊ (frames,   ┊   ┊ (load/store)  ┊   ┊ (wires it all   ┊       ║
  ║    ┊  locals)   ┊   ┊               ┊   ┊  together)      ┊       ║
  ║    ┊ · · · · · ·┊   ┊· · · · · · · ·┊   ┊· · · · · · · · ┊        ║
  ║                                                                   ║
  ╚═══════════════════════════════════════════════════════════════════╝

  ── solid border = implemented     ┊ dotted border = planned
```

## Modules

| Module | File | Status | Docs | Description |
|--------|------|--------|------|-------------|
| `WasmAluPkg` | `src/wasm_alu_pkg.veryl` | Done | [ALU](docs/alu.md) | ALU opcode enum (29 operations) |
| `WasmAlu` | `src/wasm_alu.veryl` | Done | [ALU](docs/alu.md) | Combinational ALU, all i32 operations |
| `WasmStack` | `src/wasm_stack.veryl` | Done | [Stack](docs/stack.md) | Operand stack with TOS/NOS caching |
| `WasmDecode` | `src/wasm_decode.veryl` | Done | [Decoder](docs/decoder.md) | Opcode byte to control signals (41 opcodes) |
| `WasmFetch` | `src/wasm_fetch.veryl` | Done | [Fetch](docs/fetch.md) | PC, bytecode fetch, LEB128, control flow |
| `WasmBranchTable` | `src/wasm_branch_table.veryl` | Done | [Branch Table](docs/branch_table.md) | Precomputed branch target RAM |
| Linear Memory | - | TODO | - | Flat memory for load/store |
| Call Stack | - | TODO | - | Function frames, locals, return addrs |
| Function Table | - | TODO | - | Indirect call support |
| Top-level Core | - | TODO | - | Wires everything together |

See the [Roadmap](docs/roadmap.md) for what's planned next.

## Project structure

```
src/
  wasm_alu_pkg.veryl       -- AluOp enum (Add, Sub, Mul, DivS, ...)
  wasm_alu.veryl           -- Combinational ALU + tests
  wasm_stack.veryl         -- Operand stack + tests
  wasm_decode.veryl        -- Instruction decoder + tests
  wasm_fetch.veryl         -- Fetch unit (PC, LEB128, control flow) + tests
  wasm_branch_table.veryl  -- Branch target RAM + tests
  hello.veryl              -- Placeholder
docs/                      -- Detailed module documentation
target/                    -- Generated SystemVerilog (git-ignored)
dependencies/              -- Veryl stdlib (git-ignored)
Veryl.toml                 -- Project config
```

## Prerequisites

- [Veryl](https://veryl-lang.org/) (v0.18.0+) -- the HDL compiler
- [Verilator](https://www.veripool.org/verilator/) (v5+) -- for simulation/testing

## Build and test

```bash
# Compile Veryl -> SystemVerilog
veryl build

# Run all tests (uses Verilator)
veryl test --sim verilator

# Generated SystemVerilog ends up in target/
ls target/*.sv
```

## Documentation

Detailed docs for each module live in [`docs/`](docs/):

- [ALU](docs/alu.md) -- i32 arithmetic, bitwise, comparison, and unary operations
- [Operand Stack](docs/stack.md) -- LIFO with TOS/NOS register caching
- [Decoder](docs/decoder.md) -- opcode to control signal translation
- [Fetch Unit](docs/fetch.md) -- PC management, LEB128 decoding, control flow
- [Branch Table](docs/branch_table.md) -- precomputed branch target lookup
- [Roadmap](docs/roadmap.md) -- what's next and stretch goals
