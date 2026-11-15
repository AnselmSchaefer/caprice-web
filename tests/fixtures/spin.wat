;; A theme whose frame never ends (web/smoke.mjs: its worker is ended and the usual desk shows).
;; tests/fixtures/spin.wasm is this, made by `wat` (the test
;; `the_theme_that_never_answers_is_the_module_its_text_says`).
(module
  (memory (export "memory") 1 512)
  (func (export "cabi_realloc") (param i32 i32 i32 i32) (result i32) i32.const 1024)
  (func (export "events") (param i32 i32))
  (func (export "frame") (param i32) (result i32) (loop br 0) i32.const 0)
  (func (export "cabi_post_frame") (param i32)))
