(module
  (memory (export "memory") 1)
  (func (export "main") (result i32)
    ;; store 42 at address 0
    i32.const 0
    i32.const 42
    i32.store
    ;; load it back
    i32.const 0
    i32.load))
