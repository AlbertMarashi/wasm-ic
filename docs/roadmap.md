# Roadmap

## Module dependency graph

```
  ╔═══════════╗   ╔═══════════╗
  ║  AluPkg   ║   ║   Stack   ║
  ║  (enums)  ║   ║  (LIFO)   ║
  ╚═════╤═════╝   ╚═════╤═════╝
        │               │
        v               │
  ╔═══════════╗         │          ╔══════════════╗   ╔══════════════╗
  ║  Decode   ║         │          ║ Branch Table ║   ║    Linear    ║
  ║ (signals) ║         │          ║   (RAM)      ║   ║    Memory    ║
  ╚═════╤═════╝         │          ╚══════╤═══════╝   ╚══════╤═══════╝
        │               │                 │                   │
        v               │                 v                   v
  ╔═══════════════════════════════════════════════════════════════════╗
  ║                           Fetch                                  ║
  ║        (PC, FSM, LEB128, control flow, memory load/store)        ║
  ╚══════════════════════════════╤════════════════════════════════════╝
                                 │
                                 v
               ┌───────────────────────────┐
               │       Top-level Core      │
               │  (wires everything + new  │
               │   modules below)          │
               └─────────────┬─────────────┘
                             │
                             v
                      ┌──────────┐
                      │   Call   │
                      │  Stack   │
                      └──────────┘

  ═══ double border = implemented
  ─── single border = planned
```

## Completed

1. **ALU** -- Combinational i32 ALU with all 29 arithmetic, bitwise, comparison, shift,
   rotate, and unary operations. See [ALU docs](alu.md).

2. **Operand Stack** -- Synchronous LIFO with TOS/NOS register caching for single-cycle
   ALU operations. See [Stack docs](stack.md).

3. **Decoder** -- Combinational opcode-to-control-signal translation for 41 WASM opcodes
   (34 original + 7 control flow). See [Decoder docs](decoder.md).

4. **Fetch Unit** -- Sequential bytecode fetch with program counter, LEB128 variable-length
   immediate decoding, and control flow integration. 7-state FSM handling opcode fetch,
   immediate accumulation, block type skipping, and branch resolution via the branch
   table. See [Fetch docs](fetch.md).

5. **Branch Table** -- Precomputed branch target RAM populated by an off-chip loader.
   Maps source PCs to resolved target PCs for block/loop/if/else/br/br_if instructions.
   Eliminates the need for runtime label stack traversal or forward-scanning.
   See [Branch Table docs](branch_table.md).

6. **Linear Memory** -- WASM's flat byte-addressable memory (`WasmMemory` module).
   Supports i32.load, i32.store, and their 8/16-bit variants with sign/zero extension.
   Combinational reads, synchronous writes, loader port, bounds checking. The fetch
   unit handles the two-LEB128 immediate format (alignment + offset) via ReadAlign
   and ExecMem states. See [Memory docs](memory.md).

## Next up

7. **Call Stack** -- Function call frames with locals and return addresses. Needed for
   `call` and `return` instructions. Each frame holds a return PC, the caller's
   stack depth, and local variables.

8. **Top-level Core** -- Wires all modules together into a working processor. At this
   point you can load a `.wasm` binary's code section into program memory, set the PC
   to a function entry point, and let it run.

## Stretch goals

These are aspirational -- nice to have but not blocking a working core:

- **i64 support** -- Parameterize ALU width, add i64 stack slots. Requires wider
  data paths and a second set of ALU operations.
- **f32/f64 floating point** -- Separate FPU module. Significantly more complex
  than integer ops (IEEE 754 rounding, denormals, NaN propagation).
- **Multi-cycle divide** -- Replace the combinational divider with an iterative one
  for better clock frequency. The current divider works but limits Fmax.
- **Pipelined execution** -- Overlap fetch/decode/execute stages for higher throughput.
  Requires hazard detection since WASM's stack effects are data-dependent.
- **Multi-core array** -- Multiple independent WASM cores on one chip, each with its
  own stack and memory, sharing a bus to external memory.
- **FPGA synthesis** -- Target a real FPGA board (e.g., iCE40, ECP5, or Xilinx) and
  demo execution of actual WASM binaries on hardware.
- **Function table** -- Indirect call support (`call_indirect`) for dynamic dispatch.
