# Branch Table

**File**: `src/wasm_branch_table.veryl`

The branch table (`WasmBranchTable`) is a simple dual-port RAM that maps source PCs to
resolved jump target PCs. It's populated by an off-chip loader before execution begins,
and the fetch unit reads from it at runtime to resolve control flow in a single cycle.

## Interface

```
  LOADER (before execution)              FETCH UNIT (during execution)

  i_wr_en ──────┐                        i_rd_addr [32] ──────┐
  i_wr_addr ────┤                        (source PC)          │
    [32]        │                                             │
  i_wr_data ────┤                                             │
    [32]        │                                             │
                v                                             v
         ┌─────────────────────────────────────────────────────────┐
         │                    Internal RAM                         │
         │                                                         │
         │  index │ valid │ target_pc   (direct-mapped by PC[7:0]) │
         │  ──────┼───────┼──────────                              │
         │    0   │   0   │  --------                              │
         │    1   │   0   │  --------                              │
         │    2   │   1   │  0x0000000A  ◄── loader wrote this     │
         │    3   │   0   │  --------                              │
         │    4   │   0   │  --------                              │
         │    5   │   1   │  0x00000008  ◄── loader wrote this     │
         │    ⋮   │   ⋮   │     ⋮                                   │
         │   255  │   0   │  --------                              │
         │                                                         │
         └──────────────────────────┬───────────────┬──────────────┘
                                    │               │
                                    v               v
                             o_rd_data [32]   o_rd_valid [1]
                             (target PC)      (entry exists?)
```

| Port | Direction | Width | Description |
|------|-----------|-------|-------------|
| `i_clk` | input | 1 | Clock |
| `i_rst` | input | 1 | Synchronous reset (clears valid bits) |
| `i_wr_en` | input | 1 | Write enable (loader port) |
| `i_wr_addr` | input | 32 | Source PC to write entry for |
| `i_wr_data` | input | 32 | Target PC (jump destination) |
| `i_rd_addr` | input | 32 | Source PC to look up |
| `o_rd_data` | output | 32 | Target PC at that entry |
| `o_rd_valid` | output | 1 | Entry has been written (valid) |

## Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `DEPTH` | 256 | Number of entries in the table |

## How it works

The branch table is direct-mapped: the lower `log2(DEPTH)` bits of the source PC are
used as the array index. Each slot stores a 32-bit target PC and a 1-bit valid flag.

- **Write port** (synchronous): The loader writes entries one at a time. Each write
  sets the target PC and marks the slot as valid.
- **Read port** (combinational): The fetch unit presents a source PC and immediately
  gets back the target PC and valid flag. Zero-latency lookup.
- **Reset**: Clears all valid bits. Target data is left as don't-care.

## What the loader writes

The loader scans the WASM bytecode before execution and writes one entry for each
control flow instruction that needs branch resolution:

| Source instruction | Branch table entry (target PC) |
|--------------------|-------------------------------|
| `block` (0x02) | PC of matching `end` + 1 |
| `loop` (0x03) | PC of loop body start (after the block type byte) |
| `if` (0x04) | PC of `else` + 1, or `end` + 1 if no else |
| `else` (0x05) | PC of `end` + 1 |
| `br N` (0x0C) | Resolved absolute target PC |
| `br_if N` (0x0D) | Resolved absolute target PC |

For `br` and `br_if`, the loader walks the nesting depth N to find the target block,
then stores that block's jump target (forward to `end` + 1 for `block`, backward to
body start for `loop`).

`end` (0x0B) does not need a branch table entry -- it just continues to the next opcode.

### Worked example

Consider this WASM program with an if/else inside a block:

```
  PC  Hex   Instruction
  ──  ────  ───────────────────────
   0  0x02  block        ──────┐
   1  0x40    block type       │
   2  0x04    if         ───┐  │
   3  0x40      block type  │  │
   4  0x6A      i32.add     │  │  (if body)
   5  0x05    else       ───┤  │
   6  0x6B      i32.sub     │  │  (else body)
   7  0x0B    end        ───┘  │
   8  0x0C    br 0             │
   9  0x00      depth=0        │
  10  0x0B  end          ──────┘
  11  0x01  nop                    (after block)

  Branch table entries the loader writes:
  ┌─────────┬───────────┬──────────────────────────────┐
  │ src PC  │ target PC │ reason                       │
  ├─────────┼───────────┼──────────────────────────────┤
  │    0    │    11     │ block end+1 (for br targets) │
  │    2    │     6     │ if false → else body         │
  │    5    │     8     │ else → end+1 of if/else      │
  │    8    │    11     │ br 0 → block's end+1         │
  └─────────┴───────────┴──────────────────────────────┘
```

## Design trade-offs

- **Direct-mapped**: Using the lower PC bits as the index is simple and fast, but wastes
  space since most PCs aren't control flow instructions. For a 256-entry table with a
  100-byte program, most slots are unused. This is fine for v1 -- the table is small.

- **No collision handling**: If two control flow instructions map to the same slot
  (their lower bits collide), the later write overwrites the earlier one. In practice
  this doesn't happen for small programs. A future version could use a CAM or hash
  table if larger programs need it.

- **Combinational read**: Zero-latency reads keep the fetch unit's critical path short.
  The fetch unit can look up a branch target and jump in the same cycle it decides to
  branch.

## Test coverage

The embedded testbench (`test_wasm_branch_table`) uses `DEPTH=16` and covers:

- All entries invalid after reset
- Write and read back of multiple entries (block, if, br, else patterns)
- Unwritten entries remain invalid
- Overwriting an existing entry
- Reset clears valid bits (data is don't-care after reset)
