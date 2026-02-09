# Instruction Decoder

**File**: `src/wasm_decode.veryl`

The decoder (`WasmDecode`) is a purely combinational module that translates a raw WASM
opcode byte into control signals for the ALU, stack, and fetch unit. No clock, no state
-- it's essentially a big lookup table.

## Interface

```
               ┌──────────────────────────────────────────────┐
               │              WasmDecode                      │
               │         (purely combinational)               │
               │                                              │
               │   ALU control ────────────────────────────┐  │
  i_opcode ───>│     o_alu_op  [5]  (which ALU operation)  │  │
    [8]        │     o_alu_en  [1]  (ALU should execute)   │  │
               │                                           ├──┼──>
               │   Stack control ──────────────────────────┤  │
               │     o_push    [1]  (push result)          │  │
               │     o_pop     [1]  (pop 1 value)          │  │
               │     o_pop2    [1]  (pop 2 values)         │  │
               │                                           │  │
               │   Instruction type ───────────────────────┤  │
               │     o_is_const  [1]  (i32.const)          │  │
               │     o_is_return [1]  (return)             │  │
               │     o_trap      [1]  (invalid/unreachable)│  │
               │                                           │  │
               │   Control flow ───────────────────────────┤  │
               │     o_is_block  [1]  (block / loop)       │  │
               │     o_is_if     [1]  (if)                 │  │
               │     o_is_else   [1]  (else)               │  │
               │     o_is_end    [1]  (end)                │  │
               │     o_is_br     [1]  (br)                 │  │
               │     o_is_br_if  [1]  (br_if)              │  │
               │                                           │  │
               │   Memory ─────────────────────────────────┘  │
               │     o_is_load    [1]  (load instruction)     │
               │     o_is_store   [1]  (store instruction)    │
               │     o_mem_size   [2]  (0=byte,1=half,2=word) │
               │     o_mem_signed [1]  (sign-extend on load)  │
               └──────────────────────────────────────────────┘
```

### Signal activation by instruction type

Which outputs fire for each category of instruction:

```
                  alu  alu         is_  is_  is_  is_  is_  is_  is_   is_  mem_ mem_
                  _en  _op  push pop pop2 const ret trap blk  if else end br br_if load store size signed
  ─────────────── ───  ─── ──── ─── ──── ──── ─── ──── ──── ── ──── ─── ── ───── ──── ───── ──── ──────
  nop                                                    
  unreachable                                  ●         
  return                              ●                  
  drop                        ●                          
  i32.const                ●              ●              
  unary ALU       ●    ●   ●   ●                         
  binary ALU      ●    ●   ●        ●                    
  block / loop                                      ●    
  if                        ●                        ●   
  else                                                ●  
  end                                                  ● 
  br                                                    ●
  br_if                     ●                             ●
  i32.load             ●   ●                                   ●           2
  i32.load8_s          ●   ●                                   ●           0     ●
  i32.load8_u          ●   ●                                   ●           0
  i32.load16_s         ●   ●                                   ●           1     ●
  i32.load16_u         ●   ●                                   ●           1
  i32.store                      ●                                   ●     2
  i32.store8                     ●                                   ●     0
  i32.store16                    ●                                   ●     1
  invalid opcode                           ●         
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
| `o_is_block` | output | 1 | `block` or `loop` (block type byte follows) |
| `o_is_if` | output | 1 | `if` -- conditional block, pops TOS as condition |
| `o_is_else` | output | 1 | `else` -- unconditional jump to end of if/else |
| `o_is_end` | output | 1 | `end` -- close current block |
| `o_is_br` | output | 1 | `br` -- unconditional branch, LEB128 depth follows |
| `o_is_br_if` | output | 1 | `br_if` -- conditional branch, pops TOS, LEB128 depth follows |
| `o_is_load` | output | 1 | Memory load instruction |
| `o_is_store` | output | 1 | Memory store instruction |
| `o_mem_size` | output | 2 | Access size: 0=byte, 1=halfword, 2=word |
| `o_mem_signed` | output | 1 | Sign-extend on load (1=signed, 0=unsigned) |

## Supported opcodes (49 total)

### Control instructions

| Opcode | Instruction | Signals |
|--------|-------------|---------|
| `0x00` | `unreachable` | `trap=1` |
| `0x01` | `nop` | all zeros |
| `0x0F` | `return` | `is_return=1` |

### Control flow

| Opcode | Instruction | Signals |
|--------|-------------|---------|
| `0x02` | `block` | `is_block=1` |
| `0x03` | `loop` | `is_block=1` |
| `0x04` | `if` | `is_if=1, pop=1` |
| `0x05` | `else` | `is_else=1` |
| `0x0B` | `end` | `is_end=1` |
| `0x0C` | `br` | `is_br=1` |
| `0x0D` | `br_if` | `is_br_if=1, pop=1` |

`block` and `loop` both set `is_block=1` -- the fetch unit handles them identically
(skip the block type byte and continue). The distinction between forward (`block`) and
backward (`loop`) jumps is resolved by the branch table, not the decoder.

`if` and `br_if` set `pop=1` because they consume TOS as a condition value. The fetch
unit latches the condition during the EXEC state and uses it later to decide whether
to jump via the branch table.

`br` and `br_if` have a LEB128 immediate (the branch depth) in the bytecode. The fetch
unit reads and discards it -- the actual target PC comes from the branch table.

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

### Memory load instructions

These pop 1 value (base address), compute effective address, read from linear memory,
and push 1 result.

| Opcode | Instruction | Size | Signed | Signals |
|--------|-------------|------|--------|---------|
| `0x28` | `i32.load` | 2 (word) | 0 | `is_load=1, pop=1, push=1` |
| `0x2C` | `i32.load8_s` | 0 (byte) | 1 | `is_load=1, pop=1, push=1` |
| `0x2D` | `i32.load8_u` | 0 (byte) | 0 | `is_load=1, pop=1, push=1` |
| `0x2E` | `i32.load16_s` | 1 (half) | 1 | `is_load=1, pop=1, push=1` |
| `0x2F` | `i32.load16_u` | 1 (half) | 0 | `is_load=1, pop=1, push=1` |

### Memory store instructions

These pop 2 values (value on top, base address below) and write to linear memory.

| Opcode | Instruction | Size | Signals |
|--------|-------------|------|---------|
| `0x36` | `i32.store` | 2 (word) | `is_store=1, pop2=1` |
| `0x3A` | `i32.store8` | 0 (byte) | `is_store=1, pop2=1` |
| `0x3B` | `i32.store16` | 1 (half) | `is_store=1, pop2=1` |

All memory instructions have two LEB128 immediates in the bytecode: an alignment hint
(ignored by hardware) and an offset. The effective address is `base_addr + offset`.
The fetch unit handles the LEB128 decoding and drives the memory interface.

### Invalid opcodes

Any opcode not listed above produces `trap=1`. This covers all 207 unrecognized byte
values, including future WASM instructions we haven't implemented yet.

## Design notes

- **Why combinational?** The decoder is a pure lookup table. Given an opcode byte, it
  immediately outputs control signals. The execute unit (future module) will latch these
  on each clock cycle.

- **Immediate values**: The decoder doesn't handle the immediate value that follows
  `i32.const`. It just sets `o_is_const=1` and `o_push=1` to tell the fetch unit
  "read an LEB128 immediate next" and the stack "you'll be getting a push."

- **Future expansion**: Adding new opcodes (e.g., function calls, f32/f64 ops) means
  adding more cases and possibly new output signals (e.g., `o_is_call`).

## Test coverage

The embedded testbench (`test_wasm_decode`) verifies all 49 supported opcodes plus 3
invalid opcodes (0x06, 0x42, 0xFF). Each test checks the exact combination of output
signals expected for that instruction category, including the 7 control flow opcodes,
8 memory opcodes (with `mem_size` and `mem_signed` verification), and their associated
`pop`/`pop2` signals.
