# Instruction Decoder

**File**: `src/wasm_decode.veryl`

The decoder (`WasmDecode`) is a purely combinational module that translates a raw WASM
opcode byte into control signals for the ALU and stack. No clock, no state -- it's
essentially a big lookup table.

## Interface

```
               ┌────────────┐
  i_opcode ───>│            ├──> o_alu_op    (which ALU operation)
     (8-bit)   │ WasmDecode ├──> o_alu_en    (ALU should execute)
               │            ├──> o_push      (push result to stack)
               │            ├──> o_pop       (pop 1 value)
               │            ├──> o_pop2      (pop 2 values)
               │            ├──> o_is_const  (i32.const instruction)
               │            ├──> o_is_return (return instruction)
               │            ├──> o_trap      (invalid/unreachable)
               └────────────┘
```

| Port | Direction | Width | Description |
|------|-----------|-------|-------------|
| `i_opcode` | input | 8 | Raw WASM opcode byte |
| `o_alu_op` | output | 5-bit `AluOp` | ALU operation to perform |
| `o_alu_en` | output | 1 | ALU should execute this cycle |
| `o_push` | output | 1 | Push a value onto the operand stack |
| `o_pop` | output | 1 | Pop 1 value from the operand stack |
| `o_pop2` | output | 1 | Pop 2 values from the operand stack |
| `o_is_const` | output | 1 | Instruction is `i32.const` (immediate follows) |
| `o_is_return` | output | 1 | Instruction is `return` |
| `o_trap` | output | 1 | Instruction is `unreachable` or unrecognized |

## Supported opcodes (34 total)

### Control instructions

| Opcode | Instruction | Signals |
|--------|-------------|---------|
| `0x00` | `unreachable` | `trap=1` |
| `0x01` | `nop` | all zeros |
| `0x0F` | `return` | `is_return=1` |

### Stack manipulation

| Opcode | Instruction | Signals |
|--------|-------------|---------|
| `0x1A` | `drop` | `pop=1` |
| `0x41` | `i32.const` | `is_const=1, push=1` |

### Unary ALU operations

These pop 1 value, compute, and push 1 result.

| Opcode | Instruction | AluOp | Signals |
|--------|-------------|-------|---------|
| `0x45` | `i32.eqz` | `Eqz` | `alu_en=1, pop=1, push=1` |
| `0x67` | `i32.clz` | `Clz` | `alu_en=1, pop=1, push=1` |
| `0x68` | `i32.ctz` | `Ctz` | `alu_en=1, pop=1, push=1` |
| `0x69` | `i32.popcnt` | `Popcnt` | `alu_en=1, pop=1, push=1` |

### Binary ALU operations

These pop 2 values, compute, and push 1 result.

| Opcode | Instruction | AluOp |
|--------|-------------|-------|
| `0x46` | `i32.eq` | `Eq` |
| `0x47` | `i32.ne` | `Ne` |
| `0x48` | `i32.lt_s` | `LtS` |
| `0x49` | `i32.lt_u` | `LtU` |
| `0x4A` | `i32.gt_s` | `GtS` |
| `0x4B` | `i32.gt_u` | `GtU` |
| `0x4C` | `i32.le_s` | `LeS` |
| `0x4D` | `i32.le_u` | `LeU` |
| `0x4E` | `i32.ge_s` | `GeS` |
| `0x4F` | `i32.ge_u` | `GeU` |
| `0x6A` | `i32.add` | `Add` |
| `0x6B` | `i32.sub` | `Sub` |
| `0x6C` | `i32.mul` | `Mul` |
| `0x6D` | `i32.div_s` | `DivS` |
| `0x6E` | `i32.div_u` | `DivU` |
| `0x6F` | `i32.rem_s` | `RemS` |
| `0x70` | `i32.rem_u` | `RemU` |
| `0x71` | `i32.and` | `And` |
| `0x72` | `i32.or` | `Or` |
| `0x73` | `i32.xor` | `Xor` |
| `0x74` | `i32.shl` | `Shl` |
| `0x75` | `i32.shr_s` | `ShrS` |
| `0x76` | `i32.shr_u` | `ShrU` |
| `0x77` | `i32.rotl` | `Rotl` |
| `0x78` | `i32.rotr` | `Rotr` |

All binary ops set: `alu_en=1, pop2=1, push=1`.

### Invalid opcodes

Any opcode not listed above produces `trap=1`. This covers all 222 unrecognized byte
values, including future WASM instructions we haven't implemented yet.

## Design notes

- **Why combinational?** The decoder is a pure lookup table. Given an opcode byte, it
  immediately outputs control signals. The execute unit (future module) will latch these
  on each clock cycle.

- **Immediate values**: The decoder doesn't handle the immediate value that follows
  `i32.const`. It just sets `o_is_const=1` and `o_push=1` to tell the fetch unit
  "read an LEB128 immediate next" and the stack "you'll be getting a push."

- **Future expansion**: Adding new opcodes (e.g., memory load/store, control flow)
  means adding more cases and possibly new output signals (e.g., `o_mem_read`,
  `o_branch`).

## Test coverage

The embedded testbench (`test_wasm_decode`) verifies all 34 supported opcodes plus 3
invalid opcodes (0x02, 0x80, 0xFF). Each test checks the exact combination of output
signals expected for that instruction category.
