(import mul.lisp)
(import dec.lisp)

(let n (arg 1))
(let acc (arg 2))

(if (< n 2)
  (return acc)
  (return (call fact_helper (call dec n 1) (call mul acc n))))
