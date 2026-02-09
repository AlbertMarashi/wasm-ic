# Operand Stack

**File**: `src/wasm_stack.veryl`

The operand stack (`WasmStack`) is a synchronous LIFO that holds 32-bit values for the
WASM stack machine. It uses TOS/NOS (top-of-stack / next-of-stack) register caching so
the top two values are always available with zero latency.

## Interface

```
               ┌──────────────────────────┐
  i_push ─────>│                          │
  i_pop  ─────>│   ┌──────────┐           ├──> o_top  [32]
  i_pop2 ─────>│   │   TOS    │───────────┤    (always valid)
  i_data ─────>│   ├──────────┤           ├──> o_next [32]
    [32]       │   │   NOS    │───────────┤    (always valid)
               │   ├──────────┤           │
               │   │ stack[0] │           ├──> o_empty
               │   │ stack[1] │           ├──> o_overflow
               │   │   ...    │           ├──> o_underflow
               │   │stack[D-3]│           │
               │   └──────────┘           │
               │     WasmStack            │
               └──────────────────────────┘
```

| Port | Direction | Width | Description |
|------|-----------|-------|-------------|
| `i_clk` | input | 1 | Clock |
| `i_rst` | input | 1 | Synchronous reset (active high) |
| `i_push` | input | 1 | Push `i_data` onto stack |
| `i_pop` | input | 1 | Pop 1 value |
| `i_pop2` | input | 1 | Pop 2 values |
| `i_data` | input | 32 | Data to push |
| `o_top` | output | 32 | Top of stack (TOS register) |
| `o_next` | output | 32 | Second element (NOS register) |
| `o_empty` | output | 1 | Stack is empty |
| `o_overflow` | output | 1 | Push on full stack |
| `o_underflow` | output | 1 | Pop on empty stack |

## Parameters

| Parameter | Default | Description |
|-----------|---------|-------------|
| `DEPTH` | 1024 | Maximum number of stack entries |

## How TOS/NOS caching works

WASM is a stack machine -- almost every instruction reads from or writes to the top of
the stack. Without caching, every ALU operation would require two memory reads (pop two
operands) and one memory write (push result), taking multiple clock cycles.

With TOS/NOS caching, the top two values live in dedicated registers. The ALU reads
`o_top` and `o_next` directly -- no memory access needed. The backing memory array is
only touched when values shift in/out of the cache.

This means a binary ALU operation (e.g., `i32.add`) completes in a single cycle:
pop two operands from TOS/NOS, push the result into TOS, and refill NOS from the
backing array -- all in one clock edge.

### Example: `i32.add` (pop2 + push) in one cycle

```
     BEFORE                           AFTER
  ┌──────────┐                     ┌──────────┐
  │ TOS = 5  │──┐    ┌─────┐       │ TOS = 8  │ (ALU result)
  ├──────────┤  └───>│ ALU │       ├──────────┤
  │ NOS = 3  │──────>│  +  ├──────>│ NOS = 7  │ (promoted from stack[0])
  ├──────────┤       └─────┘       ├──────────┤
  │stack[0]=7│                     │stack[0]=9│ (was stack[1])
  │stack[1]=9│                     │stack[1]=…│
  └──────────┘                     └──────────┘
   depth = 4                        depth = 3

  Signals: i_push=1, i_pop2=1, i_data=8 (from ALU)
  All in a single clock edge.
```

## Supported operations

| Operation | Signals | Stack effect | Use case |
|-----------|---------|-------------|----------|
| Push | `i_push=1` | depth+1 | `i32.const`, `local.get` |
| Pop | `i_pop=1` | depth-1 | `drop`, `local.set` |
| Replace TOS | `i_push=1, i_pop=1` | depth unchanged | Unary ALU (eqz, clz, ...) |
| Pop 2, push 1 | `i_push=1, i_pop2=1` | depth-1 | Binary ALU (add, sub, mul, ...) |

The "replace TOS" and "pop 2, push 1" combinations are the key optimization -- they
allow the execute unit to feed ALU results back to the stack in a single cycle without
any intermediate state.

## Error handling

- **Overflow**: asserted when `i_push` would exceed `DEPTH`. The push is ignored.
- **Underflow**: asserted when `i_pop`/`i_pop2` would go below 0. The pop is ignored.

## Test coverage

The embedded testbench (`test_wasm_stack`) uses `DEPTH=8` for fast simulation and covers:
- Basic push/pop sequences
- TOS/NOS values after various operations
- Simultaneous push+pop (unary op pattern)
- Simultaneous push+pop2 (binary op pattern)
- A simulated `(3+5)*2` ALU execution sequence
- Overflow and underflow detection
- Reset behavior
