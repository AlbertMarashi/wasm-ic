# Roadmap

## Completed

1. **ALU** -- Combinational i32 ALU with all 29 arithmetic, bitwise, comparison, shift,
   rotate, and unary operations. See [ALU docs](alu.md).

2. **Operand Stack** -- Synchronous LIFO with TOS/NOS register caching for single-cycle
   ALU operations. See [Stack docs](stack.md).

3. **Decoder** -- Combinational opcode-to-control-signal translation for 34 WASM opcodes.
   See [Decoder docs](decoder.md).

## Next up

4. **Fetch Unit** -- Reads bytecode from program memory and advances the program counter.
   Needs to handle LEB128 variable-length encoding for immediates (i32.const values,
   branch targets, local indices, etc.). This is the first sequential module in the
   pipeline -- it maintains a PC register and steps through the bytecode stream.

5. **Control Flow** -- WASM has structured control flow (block/loop/if/br/br_if/br_table).
   This requires a label stack to track nesting and resolve branch targets. Probably
   the trickiest module to get right -- WASM's structured control flow is unusual
   compared to traditional branch/jump architectures.

6. **Linear Memory** -- WASM's flat byte-addressable memory. Needs to support i32.load,
   i32.store, and their 8/16-bit variants with sign/zero extension. Will need to handle
   alignment and potentially multi-cycle memory access.

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
