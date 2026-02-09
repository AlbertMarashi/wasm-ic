# Fetch Unit

**File**: `src/wasm_fetch.veryl`

The fetch unit (`WasmFetch`) is a sequential module that maintains the program counter,
reads bytecode from program memory, and routes execution through the decode/execute
pipeline. It handles LEB128 variable-length immediate decoding for `i32.const` and
integrates with the branch table for control flow resolution.

## Interface

```
       Control            Memory                  To Decoder
  ┌──────────────┐  ┌──────────────┐        ┌──────────────────┐
  │ i_start  [1] │  │i_mem_data [8]│        │ o_opcode     [8] │
  │ i_stall  [1] │  │              │        │ o_opcode_en  [1] │
  └──────┬───────┘  └──────┬───────┘        └────────┬─────────┘
         │                 │                         │
         v                 v                         │
  ┌──────────────────────────────────────────────────┴─────────┐
  │                       WasmFetch                            │
  │                                                            │
  │   ┌──────┐  ┌───────────┐  ┌──────────┐  ┌─────────────┐   │
  │   │  PC  │  │  7-state  │  │  LEB128  │  │  cond_latch │   │
  │   │ [32] │  │    FSM    │  │  accum   │  │  instr_pc   │   │
  │   └──────┘  └───────────┘  └──────────┘  └─────────────┘   │
  │                                                            │
  └──┬──────────┬──────────────┬────────────────┬──────────────┘
     │          │              │                │
     v          v              v                v
  ┌──────┐  ┌────────┐  ┌───────────┐  ┌──────────────┐
  │o_mem │  │o_bt    │  │o_immediate│  │ o_pc     [32]│
  │_addr │  │_addr   │  │   [32]    │  │ o_running [1]│
  │ [32] │  │ [32]   │  │o_imm_push │  │ o_trap    [1]│
  └──────┘  └───┬────┘  │   [1]     │  └──────────────┘
     ^          │       └───────────┘     Status
   Memory    ┌──┴──────────┐
             │ Branch Table│
             │ i_bt_data   │   i_cond [1]
             │   [32]      │   (TOS != 0, from stack)
             │ i_bt_valid  │
             │   [1]       │
             └─────────────┘
```

| Port | Direction | Width | Description |
|------|-----------|-------|-------------|
| `i_clk` | input | 1 | Clock |
| `i_rst` | input | 1 | Synchronous reset (active high) |
| `i_start` | input | 1 | Pulse to begin execution from current PC |
| `i_stall` | input | 1 | Freeze pipeline when high |
| `o_mem_addr` | output | 32 | Byte address into program memory |
| `i_mem_data` | input | 8 | Byte read from program memory (asynchronous) |
| `o_opcode` | output | 8 | Current opcode byte |
| `o_opcode_en` | output | 1 | Opcode valid this cycle (EXEC state, not stalled) |
| `o_immediate` | output | 32 | Sign-extended LEB128 immediate value |
| `o_imm_push` | output | 1 | Push immediate onto stack this cycle |
| `o_bt_addr` | output | 32 | PC to look up in branch table |
| `i_bt_data` | input | 32 | Target PC from branch table |
| `i_bt_valid` | input | 1 | Branch table entry exists |
| `i_cond` | input | 1 | TOS != 0 (condition for `if` / `br_if`) |
| `o_pc` | output | 32 | Current program counter |
| `o_running` | output | 1 | 1 while actively fetching/executing |
| `o_trap` | output | 1 | Fetch-level trap (LEB128 overflow or branch table miss) |

## State machine

The fetch unit uses a 7-state FSM:

```
                   i_start
                      │
                      v
  ╔══════════════════════════════════╗
  ║             IDLE                 ║ <──── trap (LEB overflow
  ║  waiting for start signal        ║        or BT miss)
  ╚════════════════╤═════════════════╝
                   │
                   v
  ┌──────────────────────────────────┐
  │         FETCH_OPCODE             │ <─────────────────────┐
  │  read mem[PC], latch cur_opcode  │                       │
  │  save instr_pc, advance PC       │                       │
  └────────────────┬─────────────────┘                       │
                   │                                         │
                   v                                         │
  ┌──────────────────────────────────┐                       │
  │             EXEC                 │                       │
  │  present opcode to decoder       │                       │
  │  (o_opcode_en = 1)               │                       │
  └──┬─────┬─────┬──────┬───────┬────┘                       │
     │     │     │      │       │                            │
     │     │     │      │       └── simple (add, nop, ...)───┘
     │     │     │      │
     │     │     │      └── else ── BT jump ─────────────────┘
     │     │     │
     │     │     └── block / loop / if
     │     │              │
     │     │              v
     │     │     ┌────────────────────┐
     │     │     │    SKIP_BLOCK      │
     │     │     │ read & discard     │
     │     │     │ block type byte    │
     │     │     └───────┬──────┬─────┘
     │     │             │      │
     │     │     (block/ │      │ (if, cond=false)
     │     │      loop,  │      │
     │     │     or if   │      └── BT jump ─────────────────┘
     │     │      true)  │
     │     │             └── continue ───────────────────────┘
     │     │
     │     └── br / br_if
     │              │
     │              v
     │     ┌────────────────────┐
     │     │     SKIP_IMM       │ (reads 1-5 LEB128 bytes,
     │     │ read & discard     │  discards the depth value)
     │     │ LEB128 depth       │
     │     └───────┬──────┬─────┘
     │             │      │
     │     (br, or │      │ (br_if, cond=false)
     │      br_if  │      │
     │      true)  │      └── continue ──────────────────────┘
     │             │
     │             └── BT jump ──────────────────────────────┘
     │
     └── const (i32.const)
              │
              v
     ┌────────────────────┐
     │     READ_IMM       │ (reads 1-5 LEB128 bytes,
     │ accumulate LEB128  │  builds 32-bit signed value)
     │ shift into accum   │──── overflow? ──> IDLE (trap)
     └────────┬───────────┘
              │
              v
     ┌────────────────────┐
     │     EXEC_IMM       │
     │ o_imm_push = 1     │
     │ present immediate  │
     └────────┬───────────┘
              │
              └── continue ──────────────────────────────────┘
```

| State | Description |
|-------|-------------|
| **Idle** | Waiting for `i_start`. Clears trap flags. |
| **FetchOpcode** | Reads `i_mem_data` at current PC, latches it as `cur_opcode`, saves `instr_pc` for branch table lookup, advances PC. |
| **Exec** | Presents opcode to decoder (`o_opcode_en=1`). Routes to next state based on instruction type. |
| **ReadImm** | Accumulates LEB128 bytes for `i32.const`. Reads one byte per cycle, shifts into accumulator. Terminates when continuation bit is clear. |
| **ExecImm** | Presents the sign-extended immediate (`o_imm_push=1`). Returns to FetchOpcode. |
| **SkipBlock** | Reads and discards the block type byte after `block`/`loop`/`if`. For `if` with false condition, jumps to branch table target. |
| **SkipImm** | Reads and discards the LEB128 depth for `br`/`br_if`. After the last byte, either jumps via branch table or continues. |

## Instruction timing

| Instruction | Cycles | Path |
|-------------|--------|------|
| Simple (add, nop, drop, end, ...) | 2 | FetchOpcode → Exec |
| `i32.const` (1-byte LEB128) | 4 | FetchOpcode → Exec → ReadImm → ExecImm |
| `i32.const` (N-byte LEB128) | 3+N | FetchOpcode → Exec → ReadImm(xN) → ExecImm |
| `block` / `loop` | 3 | FetchOpcode → Exec → SkipBlock |
| `if` (true) | 3 | FetchOpcode → Exec → SkipBlock → continue |
| `if` (false) | 3 | FetchOpcode → Exec → SkipBlock → jump |
| `else` | 3 | FetchOpcode → Exec → jump via branch table |
| `end` | 2 | FetchOpcode → Exec (simple instruction) |
| `br` (1-byte depth) | 4 | FetchOpcode → Exec → SkipImm → jump |
| `br_if` taken | 4 | FetchOpcode → Exec → SkipImm → jump |
| `br_if` not taken | 4 | FetchOpcode → Exec → SkipImm → continue |

### Execution timeline examples

Cycle-by-cycle view of four instruction types:

```
  Clock    1         2         3         4         5
  ─────  ─────────  ─────────  ─────────  ─────────  ─────────

  i32.add (simple -- 2 cycles)
  state: FetchOp    Exec
          │          │
          latch 0x6A opcode_en=1
          PC: 0→1    → next instr

  i32.const 42 (1-byte immediate -- 4 cycles)
  state: FetchOp    Exec       ReadImm    ExecImm
          │          │          │          │
          latch 0x41 is_const=1 read 0x2A  imm_push=1
          PC: 0→1    init accum accum=42   imm=42
                                PC: 1→2    → next instr

  if (true) -- block type + continue (3 cycles)
  state: FetchOp    Exec       SkipBlock
          │          │          │
          latch 0x04 is_if=1   skip 0x40
          PC: 0→1    latch     cond=true
                     cond=1    → continue (PC=2)

  if (false) -- block type + BT jump (3 cycles)
  state: FetchOp    Exec       SkipBlock
          │          │          │
          latch 0x04 is_if=1   skip 0x40
          PC: 0→1    latch     cond=false
                     cond=0    PC ← BT target

  br 0 (unconditional branch -- 4 cycles)
  state: FetchOp    Exec       SkipImm    FetchOp
          │          │          │          │
          latch 0x0C is_br=1   read 0x00  at target
          PC: 2→3    init accum last byte  PC ← BT[2]
                                jump!
```

## LEB128 decoding

The fetch unit decodes signed LEB128 immediates for `i32.const`. Each byte contributes
7 payload bits (bit 7 is the continuation flag). After accumulating all bytes, the
result is sign-extended based on the number of bytes read:

- 1 byte: sign-extend from bit 6
- 2 bytes: sign-extend from bit 13
- 3 bytes: sign-extend from bit 20
- 4 bytes: sign-extend from bit 27
- 5 bytes: no extension needed (all 32 bits filled)

If a 5th LEB128 byte still has the continuation bit set, the fetch unit traps
(LEB128 overflow -- malformed bytecode).

## Branch table integration

The fetch unit outputs `o_bt_addr` (the PC of the current control flow instruction,
latched in `instr_pc` during FetchOpcode) and reads `i_bt_data` / `i_bt_valid`
combinationally.

Branch table lookups happen in these states:
- **SkipBlock** (for `if` with false condition): jump to else/end target
- **Exec** (for `else`): unconditional jump to end+1
- **SkipImm** (for `br`, or `br_if` when taken): jump to resolved target

If a branch table lookup returns `i_bt_valid=0`, the fetch unit traps (missing entry).
This is a safety net -- well-formed programs with a correct loader should never hit it.

## Internal registers

| Register | Width | Description |
|----------|-------|-------------|
| `pc` | 32 | Program counter (points to next byte to read) |
| `cur_opcode` | 8 | Latched opcode from FetchOpcode state |
| `instr_pc` | 32 | PC of current instruction (for branch table lookup) |
| `cond_latch` | 1 | Condition captured during Exec for `if`/`br_if` |
| `imm_accum` | 32 | LEB128 accumulator |
| `imm_shift` | 6 | Current bit position in LEB128 accumulator |
| `imm_bytes` | 3 | Number of LEB128 bytes read so far |
| `leb_trap` | 1 | LEB128 overflow trap flag |
| `bt_trap` | 1 | Branch table miss trap flag |

## Design notes

- **Local opcode decode**: The fetch unit decodes the opcode locally (e.g.,
  `opcode_is_const`, `opcode_is_br`) to drive its state machine. This is independent
  of the external decoder module, which produces signals for the ALU and stack.

- **Asynchronous memory**: Program memory is read combinationally (`o_mem_addr` out,
  `i_mem_data` in, same cycle). This matches a simple ROM or block RAM in read-first
  mode.

- **Condition latching**: For `if` and `br_if`, the condition (`i_cond`, TOS != 0) is
  latched into `cond_latch` during the Exec state, before the stack pops the value.
  The latched value is used in SkipBlock or SkipImm to decide whether to jump.

## Test coverage

The embedded testbench (`test_wasm_fetch`) covers 16 test programs:

1. Simple opcode sequence (add, sub, nop, drop)
2. `i32.const 42` (single-byte LEB128)
3. `i32.const -1` (sign extension)
4. `i32.const 128` (2-byte LEB128)
5. `block` + `end` (block type skip)
6. `if` (true) -- enters if body
7. `if` (false) -- jumps to end via branch table
8. `if`/`else` true path -- executes if body, jumps over else
9. `if`/`else` false path -- jumps to else body
10. `br 0` -- unconditional branch out of block
11. `br_if` taken (TOS != 0)
12. `br_if` not taken (TOS == 0)
13. Loop with `br 0` -- backward jump (verifies loop re-entry)
14. Stall freezes pipeline
15. Branch table miss traps
16. Nested blocks with `br 1` -- skips out of multiple nesting levels
