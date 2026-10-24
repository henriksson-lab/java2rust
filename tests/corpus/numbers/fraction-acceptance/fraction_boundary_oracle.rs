use translated::{FractionNullBoundary as Boundary, JavaException};

// Observe either emitted public exception ABI without changing the operation.
trait Observed {
    fn observed(self) -> Result<i32, JavaException>;
}
impl Observed for i32 {
    fn observed(self) -> Result<i32, JavaException> {
        Ok(self)
    }
}
impl Observed for Result<i32, JavaException> {
    fn observed(self) -> Result<i32, JavaException> {
        self
    }
}
fn main() {
    for which in 0..23 {
        println!(
            "null:{which}\t{}",
            Boundary::check(which).observed().unwrap()
        );
    }
    for which in 0..6 {
        println!(
            "comparable:{which}\t{}",
            Boundary::compare(which).observed().unwrap()
        );
    }
}
