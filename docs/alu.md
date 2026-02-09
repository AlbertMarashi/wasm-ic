# ALU (Arithmetic Logic Unit)

**Files**: `src/wasm_alu_pkg.veryl`, `src/wasm_alu.veryl`

The ALU (`WasmAlu`) is a purely combinational module -- no clock, no state. You feed it an
operation, two 32-bit operands, and it immediately outputs a result and a trap flag.

## Interface

```
              ┌─────────────────────────────────────────┐
  i_op ──────>│              WasmAlu                    │
    [5]       │                                         │
              │   ┌─────────────────────────────────┐   │
  i_lhs ─────>│   │            op mux               │   ├──> o_result
    [32]      │   │                                 │   │     [32]
              │   │  ┌─────┐ ┌─────┐ ┌──────────┐   │   │
  i_rhs ─────>│   │  │arith│ │logic│ │ compare  │   │   ├──> o_trap
    [32]      │   │  │+−×÷%│ │&|^  │ │ = ≠ < >  │   │   │     [1]
              │   │  └──┬──┘ └──┬──┘ └────┬─────┘   │   │
              │   │     │       │         │         │   │
              │   │  ┌──┴──┐ ┌──┴───┐ ┌───┴───┐     │   │
              │   │  │shift│ │rotate│ │ unary │     │   │
              │   │  │<< >>│ │ ↻ ↺  │ │ eqz   │     │   │
              │   │  └─────┘ └──────┘ │clz ctz│     │   │
              │   │                   │popcnt │     │   │
              │   │                   └───────┘     │   │
              │   └─────────────────────────────────┘   │
              │                                         │
              │   ┌──────────────────┐                  │
              │   │ trap detect      │                  │
              │   │ ÷0 or MIN÷(−1)   ├──────────────────┘
              │   └──────────────────┘
              └─────────────────────────────────────────┘
```

| Port | Direction | Width | Description |
|------|-----------|-------|-------------|
| `i_op` | input | 5-bit `AluOp` | Which operation to perform |
| `i_lhs` | input | 32 | Left operand (TOS for unary ops) |
| `i_rhs` | input | 32 | Right operand (ignored for unary ops) |
| `o_result` | output | 32 | Computed result |
| `o_trap` | output | 1 | Trap: div-by-zero or signed overflow |

## AluOp enum

Defined in `WasmAluPkg`, 5-bit encoding (29 of 32 values used):

| Value | Name | WASM opcode | Description |
|-------|------|-------------|-------------|
| 0 | `Add` | `i32.add` (0x6A) | Addition |
| 1 | `Sub` | `i32.sub` (0x6B) | Subtraction |
| 2 | `Mul` | `i32.mul` (0x6C) | Multiplication |
| 3 | `DivS` | `i32.div_s` (0x6D) | Signed division |
| 4 | `DivU` | `i32.div_u` (0x6E) | Unsigned division |
| 5 | `RemS` | `i32.rem_s` (0x6F) | Signed remainder |
| 6 | `RemU` | `i32.rem_u` (0x70) | Unsigned remainder |
| 7 | `And` | `i32.and` (0x71) | Bitwise AND |
| 8 | `Or` | `i32.or` (0x72) | Bitwise OR |
| 9 | `Xor` | `i32.xor` (0x73) | Bitwise XOR |
| 10 | `Shl` | `i32.shl` (0x74) | Shift left |
| 11 | `ShrS` | `i32.shr_s` (0x75) | Arithmetic shift right |
| 12 | `ShrU` | `i32.shr_u` (0x76) | Logical shift right |
| 13 | `Rotl` | `i32.rotl` (0x77) | Rotate left |
| 14 | `Rotr` | `i32.rotr` (0x78) | Rotate right |
| 15 | `Eq` | `i32.eq` (0x46) | Equal |
| 16 | `Ne` | `i32.ne` (0x47) | Not equal |
| 17 | `LtS` | `i32.lt_s` (0x48) | Signed less than |
| 18 | `LtU` | `i32.lt_u` (0x49) | Unsigned less than |
| 19 | `GtS` | `i32.gt_s` (0x4A) | Signed greater than |
| 20 | `GtU` | `i32.gt_u` (0x4B) | Unsigned greater than |
| 21 | `LeS` | `i32.le_s` (0x4C) | Signed less or equal |
| 22 | `LeU` | `i32.le_u` (0x4D) | Unsigned less or equal |
| 23 | `GeS` | `i32.ge_s` (0x4E) | Signed greater or equal |
| 24 | `GeU` | `i32.ge_u` (0x4F) | Unsigned greater or equal |
| 25 | `Eqz` | `i32.eqz` (0x45) | Equal to zero (unary) |
| 26 | `Clz` | `i32.clz` (0x67) | Count leading zeros (unary) |
| 27 | `Ctz` | `i32.ctz` (0x68) | Count trailing zeros (unary) |
| 28 | `Popcnt` | `i32.popcnt` (0x69) | Population count (unary) |

## WASM spec compliance

- Shifts mask the shift amount to 5 bits (shift mod 32), per spec
- Comparisons return `0` or `1` as a 32-bit value (zero-extended)
- Division by zero raises `o_trap` (WASM traps, not undefined behavior)
- `i32.div_s(INT32_MIN, -1)` raises `o_trap` (signed overflow)
- Signed operations use proper two's complement arithmetic

## Known limitations

- **Fully combinational**: multiply and divide have no pipelining. A real implementation
  would pipeline multiply (~3 cycles) and iterate divide (~4-32 cycles) to allow higher
  clock frequencies. The current design prioritizes correctness and simplicity.
- **i32 only**: i64 operations not yet supported. The ALU could be parameterized by width.
- **Missing sign-extension**: no i32.extend8_s, i32.extend16_s yet.

## Test coverage

The embedded testbench (`test_wasm_alu`) covers 47 test vectors across all 29 operations,
including edge cases like division by zero, INT32_MIN/-1 overflow, rotates, and sign-extended
comparisons.
