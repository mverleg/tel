(import add.lisp)
(import sub.lisp)

(let n (arg 1))

(if (< n 2)
  (return n)
  (return (call add
    (call fib (call sub n 1) 0)
    (call fib (call sub n 2) 0))))
