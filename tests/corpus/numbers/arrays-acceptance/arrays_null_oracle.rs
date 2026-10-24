use translated::ArraysNullBoundary;

trait Exit {
    fn value(self) -> i32;
}
impl Exit for i32 {
    fn value(self) -> i32 {
        self
    }
}
impl<E: std::fmt::Debug> Exit for Result<i32, E> {
    fn value(self) -> i32 {
        self.unwrap()
    }
}
fn main() {
    // Supplement the direct production API callers with co-translated root
    // nullability contracts. The Java client catches only NullPointerException
    // and checks mutation state; no generated algorithm is replaced here.
    for which in 0..20 {
        println!("{which}\t{}", ArraysNullBoundary::check(which).value());
    }
}
