target triple = "x86_64-unknown-linux-gnu"

declare ptr @callee(i64)

define ptr @f(i64 %x) #0 {
  %r = tail call ptr @callee(i64 %x)
  ret ptr %r
}

attributes #0 = { "target-features"="+retpoline-indirect-calls,+retpoline-indirect-branches" }

!llvm.module.flags = !{!0}
!0 = !{i32 1, !"Code Model", i32 4}
