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
  │   │ Unit  │    │  <-done│    │ Unit    │        │
  │   └───┬───┘    └────────┘    └────┬────┘        │
  │       │                          │              │
  │  ┌────┴─────┐              ┌─────┴─────┐        │
  │  │ Program  │              │  WasmAlu  │  <-- done│
  │  │ Memory   │              └───────────┘         │
  │  └──────────┘                                    │
  │                                                  │
  │  ┌──────────┐  ┌──────────┐  ┌──────────┐       │
  │  │ Operand  │  │   Call   │  │  Linear  │       │
  │  │  Stack   │  │  Stack   │  │  Memory  │       │
  │  │  <-- done│  └──────────┘  └──────────┘       │
  │  └──────────┘                                    │
  └──────────────────────────────────────────────────┘
```

### Modules

| Module | File | Status | Description |
|--------|------|--------|-------------|
| `WasmAluPkg` | `src/wasm_alu_pkg.veryl` | Done | ALU opcode enum (29 operations) |
| `WasmAlu` | `src/wasm_alu.veryl` | Done | Combinational ALU for all WASM i32 operations |
| `WasmStack` | `src/wasm_stack.veryl` | Done | Operand stack with TOS/NOS caching |
| `WasmDecode` | `src/wasm_decode.veryl` | Done | WASM opcode byte -> control signals |
| Fetch Unit | - | TODO | Reads bytecode from program memory, LEB128 decode |
| Control Flow | - | TODO | block/loop/if/br/br_if with label stack |
| Linear Memory | - | TODO | WASM flat memory for load/store instructions |
| Call Stack | - | TODO | Function frames, locals, return addresses |
| Function Table | - | TODO | Indirect call support (call_indirect) |
| Top-level Core | - | TODO | Wires everything together |

## Project structure

```
src/
  wasm_alu_pkg.veryl   -- AluOp enum definition (Add, Sub, Mul, DivS, ...)
  wasm_alu.veryl       -- Combinational ALU module + tests
  wasm_stack.veryl     -- Operand stack module + tests
  wasm_decode.veryl    -- Instruction decoder module + tests
  hello.veryl          -- Placeholder (can be removed)
target/                -- Generated SystemVerilog output (git-ignored)
dependencies/          -- Veryl standard library (git-ignored)
Veryl.toml             -- Project config
```

## Prerequisites

- [Veryl](https://veryl-lang.org/) (v0.18.0+) -- the HDL compiler
- [Verilator](https://www.veripool.org/verilator/) (v5+) -- for simulation/testing

## Build and test

```bash
# Compile Veryl -> SystemVerilog
veryl build

# Run all tests (uses Verilator under the hood)
veryl test --sim verilator

# Generated SystemVerilog ends up in target/
ls target/*.sv
```

## How the ALU works

The ALU (`WasmAlu`) is a purely combinational module -- no clock, no state. You feed it an
operation, two 32-bit operands, and it immediately outputs a result and a trap flag.

```
  i_op ──────┐
  i_lhs ─────┤ WasmAlu ├──── o_result (32-bit)
  i_rhs ─────┘          └──── o_trap   (div-by-zero / signed overflow)
```

Supported operations (29 total):

- **Arithmetic**: add, sub, mul, div_s, div_u, rem_s, rem_u
- **Bitwise**: and, or, xor
- **Shifts/Rotates**: shl, shr_s, shr_u, rotl, rotr
- **Comparison**: eq, ne, lt_s, lt_u, gt_s, gt_u, le_s, le_u, ge_s, ge_u
- **Unary**: eqz, clz, ctz, popcnt

These map 1:1 to the [WASM i32 instruction set](https://webassembly.github.io/spec/core/exec/numerics.html).

### WASM spec compliance notes

- Shifts mask the shift amount to 5 bits (shift mod 32), per spec
- Comparisons return `0` or `1` as a 32-bit value (zero-extended)
- Division by zero raises `o_trap` (WASM traps, not undefined behavior)
- `i32.div_s(INT32_MIN, -1)` raises `o_trap` (signed overflow)
- Signed operations use proper two's complement arithmetic

### Known limitations

- The ALU is fully combinational, including multiply and divide. A real implementation
  would pipeline multiply (~3 cycles) and iterate divide (~4-32 cycles) to allow higher
  clock frequencies. The current design prioritizes correctness and simplicity.
- i64 operations are not yet supported. The ALU could be parameterized by width later.
- No i32.extend8_s, i32.extend16_s, or other sign-extension instructions yet.

## How the operand stack works

The stack (`WasmStack`) is a synchronous LIFO with TOS (top-of-stack) and NOS
(next-of-stack) caching in dedicated registers. This means the top two values are
always available with zero latency -- the ALU can read both operands directly.

```
               ┌─────────┐
  i_data ─────>│  TOS    │──────> o_top   (always available)
               ├─────────┤
               │  NOS    │──────> o_next  (always available)
               ├─────────┤
               │ stack[0]│
               │ stack[1]│
               │   ...   │
               └─────────┘
```

Supports simultaneous push+pop for single-cycle ALU operations:

| Operation | Signals | Use case |
|-----------|---------|----------|
| Push | `i_push` | `i32.const`, `local.get` |
| Pop | `i_pop` | `drop`, `local.set` |
| Replace TOS | `i_push + i_pop` | Unary ALU ops (eqz, clz, ...) |
| Pop 2, push 1 | `i_push + i_pop2` | Binary ALU ops (add, sub, mul, ...) |

Configurable depth (default 1024). Reports `o_overflow` / `o_underflow` errors.

## How the decoder works

The decoder (`WasmDecode`) is a purely combinational module that translates a raw WASM
opcode byte into control signals for the ALU and stack. No clock, no state -- it's
essentially a big lookup table.

```
               ┌────────────┐
  i_opcode ───>│            ├──> o_alu_op   (which ALU operation)
     (8-bit)   │ WasmDecode ├──> o_alu_en   (ALU should execute)
               │            ├──> o_push     (push result to stack)
               │            ├──> o_pop      (pop 1 value)
               │            ├──> o_pop2     (pop 2 values)
               │            ├──> o_is_const (i32.const instruction)
               │            ├──> o_is_return(return instruction)
               │            ├──> o_trap     (invalid/unreachable)
               └────────────┘
```

Decodes 34 WASM opcodes:

| Category | Opcodes | Signals set |
|----------|---------|-------------|
| Binary ALU (i32.add, sub, mul, ...) | 0x46-0x4F, 0x6A-0x78 | alu_en + pop2 + push |
| Unary ALU (i32.eqz, clz, ctz, popcnt) | 0x45, 0x67-0x69 | alu_en + pop + push |
| i32.const | 0x41 | is_const + push |
| drop | 0x1A | pop |
| nop | 0x01 | (all zeros) |
| return | 0x0F | is_return |
| unreachable | 0x00 | trap |
| Everything else | - | trap |

The decoder doesn't handle immediate values (e.g., the constant for `i32.const`) --
that's the fetch unit's job. The decoder only says "this instruction needs a constant
pushed" and the fetch unit provides the value.

## Roadmap

Roughly in order of implementation:

1. ~~**Operand Stack**~~ -- Done. See `src/wasm_stack.veryl`.

2. ~~**Decoder**~~ -- Done. See `src/wasm_decode.veryl`.

3. **Fetch Unit** -- reads bytecode from program memory, advances the program counter.
   Needs to handle LEB128 variable-length encoding for immediates (i32.const values,
   branch targets, local indices, etc.).

4. **Control Flow** -- WASM has structured control flow (block/loop/if/br/br_if/br_table).
   This requires a label stack to track nesting and resolve branch targets. This is
   probably the trickiest module to get right.

5. **Linear Memory** -- WASM's flat byte-addressable memory. Needs to support i32.load,
   i32.store, and their 8/16-bit variants with sign/zero extension.

6. **Call Stack** -- function call frames with locals and return addresses. Needed for
   `call` and `return` instructions.

7. **Top-level Core** -- wires all modules together into a working processor. At this
   point you can load a .wasm binary's code section into program memory, set the PC
   to a function entry point, and let it run.

### Stretch goals

- i64 support (parameterize ALU width, add i64 stack slots)
- f32/f64 floating point (separate FPU module)
- Multi-cycle divide unit for better clock frequency
- Pipelined execution
- Multi-core array (multiple independent WASM cores on one chip)
- FPGA synthesis and demo on real hardware
