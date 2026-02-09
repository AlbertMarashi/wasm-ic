(module
  (func (export "main") (result i32)
    (local $i i32)
    ;; count from 0 to 10
    i32.const 0
    local.set $i
    block $done
      loop $continue
        ;; if i >= 10, break
        local.get $i
        i32.const 10
        i32.ge_s
        br_if $done
        ;; i = i + 1
        local.get $i
        i32.const 1
        i32.add
        local.set $i
        br $continue
      end
    end
    local.get $i))
