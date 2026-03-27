fn main(a    , b    ) ->     {
    let point         = Point(a, 200);
    let list      = [point.x, 300];
    let record         = { x    = 100, y    = 200 };  
    return point.y == record.y;
}

struct Point[T] { x: T, y: T }
