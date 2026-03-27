fn example(param0      , param1: int) -> Maybe[string] {     
    let z       = {
        let n     = 4;
        fst(param0,          { x     = n, y     = n })
    };

    let _         = [z, param1];

    return Maybe::Just(param0);
}

fn fst[T, U](a: T, b: U) -> T {..}
type Point[N] = { x: N, y: N }
