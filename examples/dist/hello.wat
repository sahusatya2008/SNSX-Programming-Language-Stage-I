(module
  (func $summarize (result i64)
    i64.const 0
    ;; non-constant SNSX bodies lower to VM/runtime bridges in bootstrap mode
  )
  (func $main (result i64)
    i64.const 0
  )
  (export "_start" (func $main))
)
