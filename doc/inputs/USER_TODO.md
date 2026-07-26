
1. We don't want Clone as a name, or at least not use it for e.g. mpsc senders (we don't have Rc but if we did it's also not great there)
   because deep copy and 'fork' reference should not be the same thing 

2. Do we allow positional tuples?
   E.g. Rust has both struct A(i32) and struct A{ b: i32 }
   And Tel has tuples with positional and named args mixed, to match function args
   And what are tuples if not nameless structs, so we should also allow positional tuples?


   
