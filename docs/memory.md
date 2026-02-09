# Linear Memory

**File**: `src/wasm_memory.veryl`

The linear memory (`WasmMemory`) is a flat byte-addressable RAM that backs WASM's
load/store instructions. Internally a byte array (`logic<8> [SIZE]`) with little-endian
byte order per the WASM spec. Supports 1-byte, 2-byte, and 4-byte access with
sign/zero extension for sub-word loads.

Read port is combinational (zero-latency). Write port is synchronous (commits on clock
edge). A separate loader port allows the off-chip loader to populate initial memory
contents (WASM data sections) before execution begins.

## Interface

```
  LOADER (before execution)              FETCH UNIT (during execution)

  i_load_en ────┐                        READ PORT (combinational)
  i_load_addr ──┤                        i_rd_addr  [32] ──┐
    [32]        │                        i_rd_size   [2] ──┤  0=byte
  i_load_data ──┤                        i_rd_signed [1] ──┤  1=half
    [8]         │                                          │  2=word
                v                                          v
  ┌────────────────────────────────────────────────────────────────┐
  │                        Byte Array                              │
  │                                                                │
  │  addr │ byte      (SIZE entries, indexed by addr[ADDR_WIDTH])  │
  │  ─────┼──────                                                  │
  │    0  │ 0xEF  ─┐                                               │
  │    1  │ 0xBE   ├── little-endian word: 0xDEADBEEF              │
  │    2  │ 0xAD   │                                               │
  │    3  │ 0xDE  ─┘                                               │
  │    4  │ 0x80  ──── sign-ext byte: 0xFFFFFF80 (signed)          │
  │    5  │ 0x00       zero-ext byte: 0x00000080 (unsigned)        │
  │    ⋮  │  ⋮                                                      │
  │  N-1  │ 0x00                                                   │
  │                                                                │
  └─────────┬──────────────┬──────────────────┬────────────────────┘
            │              │                  │
            v              v                  v
     o_rd_data [32]  o_rd_trap [1]     WRITE PORT (synchronous)
     (assembled +    (addr OOB)        i_wr_en    [1]
      extended)                        i_wr_addr [32]
                                       i_wr_size  [2]
                                       i_wr_data [32] ──> o_wr_trap [1]
```

| Port | Direction | Width | Description |
|------|-----------|-------|-------------|
| `i_clk` | input | 1 | Clock |
| `i_rst` | input | 1 | Synchronous reset |
| `i_load_en` | input | 1 | Loader write enable (byte-at-a-time) |
| `i_load_addr` | input | 32 | Loader byte address |
| `i_load_data` | input | 8 | Loader byte data |
| `i_rd_addr` | input | 32 | Read byte address |
| `i_rd_size` | input | 2 | Read access size (0=byte, 1=half, 2=word) |
| `i_rd_signed` | input | 1 | 1=sign-extend, 0=zero-extend |
| `o_rd_data` | output | 32 | Assembled and extended read result |
| `o_rd_trap` | output | 1 | Read address out of bounds |
| `i_wr_en` | input | 1 | Write enable |
| `i_wr_addr` | input | 32 | Write byte address |
| `i_wr_size` | input | 2 | Write access size (0=byte, 1=half, 2=word) |
| `i_wr_data` | input | 32 | Data to write (low bytes used for sub-word) |
| `o_wr_trap` | output | 1 | Write address out of bounds |

## Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `SIZE` | 4096 | Number of bytes in the memory |

Internally, `ADDR_WIDTH = $clog2(SIZE)` is derived for array indexing, avoiding
Verilator width truncation warnings.

## Read logic (combinational)

Reads assemble 1, 2, or 4 bytes from the byte array in little-endian order and
sign/zero extend to 32 bits:

```
  i_rd_size  Bytes read         Extension
  ─────────  ──────────────     ──────────────────────────────
  0 (byte)   mem[addr]          sign-ext bit 7  or zero-ext
  1 (half)   mem[addr+1:addr]   sign-ext bit 15 or zero-ext
  2 (word)   mem[addr+3:addr]   no extension needed (32 bits)
```

### Sign extension examples

```
  Stored byte: 0x80 (bit 7 = 1)

  load8_u  → 0x00000080  (zero-extend: pad with 0s)
  load8_s  → 0xFFFFFF80  (sign-extend: pad with 1s)

  Stored half: 0x8000 (bit 15 = 1)

  load16_u → 0x00008000  (zero-extend)
  load16_s → 0xFFFF8000  (sign-extend)
```

## Write logic (synchronous)

Writes decompose the 32-bit input into 1, 2, or 4 bytes and store them in
little-endian order on the clock edge:

```
  i_wr_size  Bytes written            Source bits
  ─────────  ────────────────         ──────────────
  0 (byte)   mem[addr]                data[7:0]
  1 (half)   mem[addr], mem[addr+1]   data[7:0], data[15:8]
  2 (word)   mem[addr..addr+3]        data[7:0..31:24]
```

Sub-word writes (store8, store16) only modify the targeted bytes -- surrounding
bytes are left unchanged.

The loader port (`i_load_en`) takes priority over the runtime write port. Both
are synchronous and mutually exclusive in practice (loader runs before execution).

## Bounds checking

Both read and write addresses are bounds-checked against `SIZE`:

```
  o_rd_trap = (addr + access_bytes) > SIZE
  o_wr_trap = wr_en && (addr + access_bytes) > SIZE
```

When `o_wr_trap` is asserted, the write is suppressed -- no bytes are modified.
The fetch unit checks the trap signals during `ExecMem` and transitions to `Idle`
with `o_trap=1` if an out-of-bounds access is attempted.

Note: Veryl uses `>:` for the greater-than operator (not `>`) to avoid ambiguity
with angle brackets.

## How the fetch unit drives it

The fetch unit connects to the memory module during `ExecMem` state. The flow for
a memory instruction (e.g., `i32.store` with alignment=0, offset=4):

```
  Cycle  State       Action
  ─────  ──────────  ──────────────────────────────────────────────
    1    FetchOpcode  Latch opcode 0x36, advance PC
    2    Exec         Latch TOS (value) and NOS (base addr) from stack
                      Decode mem_size=2, mem_signed=0
                      Init LEB128 accumulator → ReadAlign
    3    ReadAlign    Read alignment byte (0x00), discard it
                      Reset accumulator → ReadImm
    4    ReadImm      Read offset byte (0x04), accumulate
                      offset = 4 → ExecMem
    5    ExecMem      effective_addr = base + offset = 0 + 4 = 4
                      Store: o_lm_wr_en=1, write value at addr 4
                      Load:  o_mem_result = combinational read at addr 4
```

For loads, the fetch unit asserts `o_mem_result_en` and the top-level core pushes
`o_mem_result` onto the stack. For stores, `o_lm_wr_en` fires for one cycle and the
memory commits the write on the next clock edge.

## WASM instructions using this module

| Opcode | Instruction | Access | Stack effect |
|--------|-------------|--------|--------------|
| `0x28` | `i32.load` | 4-byte read | pop addr, push value |
| `0x2C` | `i32.load8_s` | 1-byte read, sign-ext | pop addr, push value |
| `0x2D` | `i32.load8_u` | 1-byte read, zero-ext | pop addr, push value |
| `0x2E` | `i32.load16_s` | 2-byte read, sign-ext | pop addr, push value |
| `0x2F` | `i32.load16_u` | 2-byte read, zero-ext | pop addr, push value |
| `0x36` | `i32.store` | 4-byte write | pop value, pop addr |
| `0x3A` | `i32.store8` | 1-byte write | pop value, pop addr |
| `0x3B` | `i32.store16` | 2-byte write | pop value, pop addr |

Each instruction has two LEB128 immediates in the bytecode: an alignment hint
(ignored by hardware) and a byte offset. Effective address = base_addr + offset.

For loads, TOS is the base address. For stores, TOS is the value and NOS is the
base address.

## Design trade-offs

- **Byte array vs word array**: Using `logic<8> [SIZE]` (one byte per entry) makes
  sub-word access simple -- no barrel shifter or byte-lane muxing needed. The cost
  is that word reads assemble 4 separate array lookups combinationally. This is fine
  for small memories and FPGA synthesis where the tool optimizes the array.

- **Combinational reads**: Zero-latency reads avoid adding wait states to the fetch
  FSM. The read result is available in the same cycle the address is presented, so
  `ExecMem` can read and push in one state. This matches the pattern used by program
  memory and the branch table.

- **No alignment enforcement**: The WASM spec says the alignment immediate is a hint
  and must not trap on misalignment. The hardware ignores it completely -- unaligned
  accesses work correctly (byte-at-a-time assembly handles any alignment).

- **Loader port**: Like the branch table, the memory has a dedicated byte-at-a-time
  loader port for populating data sections before execution. The loader writes one byte
  per clock cycle. The loader port takes priority over the runtime write port.

## Test coverage

The embedded testbench (`test_wasm_memory`) uses `SIZE=64` and covers 12 test groups:

1. Loader write + byte read (4 bytes at addresses 0-3)
2. Halfword read (little-endian assembly from two bytes)
3. Word read (little-endian assembly from four bytes)
4. Sign extension -- byte (0x80 signed vs unsigned)
5. Sign extension -- halfword (0x8000 signed vs unsigned)
6. Synchronous write -- byte
7. Synchronous write -- halfword (verify both bytes written)
8. Synchronous write -- word (verify all 4 bytes in little-endian order)
9. Bounds check -- read (byte/half/word at various boundary addresses)
10. Bounds check -- write (OOB write suppressed, data unchanged)
11. store8 only writes low byte (surrounding bytes preserved)
12. store16 only writes low 2 bytes (surrounding bytes preserved)
