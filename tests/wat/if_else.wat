(module
  (func (export "main") (result i32)
    i32.const 1
    if (result i32)
      i32.const 42
    else
      i32.const 0
    end))
