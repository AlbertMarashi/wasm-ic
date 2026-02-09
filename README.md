# wasm-ic

A hardware WebAssembly processor implemented in [Veryl](https://veryl-lang.org/). The goal is
a chip (FPGA or ASIC) that natively executes WASM bytecode -- no software runtime, no JIT,
just silicon running WASM instructions directly.

## Status

Early development. The ALU, operand stack, and instruction decoder are implemented and tested.

## Architecture

```
                         WASM IC Core
  ┌──────────────────────────────────────────────────┐
  │                                                  │
  │   ┌───────┐    ┌────────┐    ┌─────────┐        │
  │   │ Fetch │ -> │ Decode │ -> │ Execute │        │
  │   │ Unit  │    │  (done)│    │ Unit    │        │
  │   └───┬───┘    └────────┘    └────┬────┘        │
  │       │                          │              │
  │  ┌────┴─────┐              ┌─────┴─────┐        │
  │  │ Program  │              │  WasmAlu  │ (done) │
  │  │ Memory   │              └───────────┘        │
  │  └──────────┘                                   │
  │                                                  │
  │  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
  │  │ Operand  │  │   Call   │  │  Linear  │       │
  │  │  Stack   │  │  Stack   │  │  Memory  │       │
  │  │  (done)  │  └──────────┘  └──────────┘       │
  │  └──────────┘                                    │
  └──────────────────────────────────────────────────┘
```

## Modules

| Module | File | Status | Docs | Description |
|--------|------|--------|------|-------------|
| `WasmAluPkg` | `src/wasm_alu_pkg.veryl` | Done | [ALU](docs/alu.md) | ALU opcode enum (29 operations) |
| `WasmAlu` | `src/wasm_alu.veryl` | Done | [ALU](docs/alu.md) | Combinational ALU, all i32 operations |
| `WasmStack` | `src/wasm_stack.veryl` | Done | [Stack](docs/stack.md) | Operand stack with TOS/NOS caching |
| `WasmDecode` | `src/wasm_decode.veryl` | Done | [Decoder](docs/decoder.md) | Opcode byte to control signals |
| Fetch Unit | - | TODO | - | Bytecode fetch, PC, LEB128 decode |
| Control Flow | - | TODO | - | block/loop/if/br with label stack |
| Linear Memory | - | TODO | - | Flat memory for load/store |
| Call Stack | - | TODO | - | Function frames, locals, return addrs |
| Function Table | - | TODO | - | Indirect call support |
| Top-level Core | - | TODO | - | Wires everything together |

See the [Roadmap](docs/roadmap.md) for what's planned next.

## Project structure

```
src/
  wasm_alu_pkg.veryl   -- AluOp enum (Add, Sub, Mul, DivS, ...)
  wasm_alu.veryl       -- Combinational ALU + tests
  wasm_stack.veryl     -- Operand stack + tests
  wasm_decode.veryl    -- Instruction decoder + tests
  hello.veryl          -- Placeholder
docs/                  -- Detailed module documentation
target/                -- Generated SystemVerilog (git-ignored)
dependencies/          -- Veryl stdlib (git-ignored)
Veryl.toml             -- Project config
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
- [Roadmap](docs/roadmap.md) -- what's next and stretch goals
