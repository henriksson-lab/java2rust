//! Calls the automatically translated Java client; no production algorithm here.
trait Value {
    fn value(self) -> i32;
}
impl Value for i32 {
    fn value(self) -> i32 {
        self
    }
}
impl<E: std::fmt::Debug> Value for Result<i32, E> {
    fn value(self) -> i32 {
        self.unwrap()
    }
}
fn main() {
    let code = translated::AngleContractsOracle::run().value();
    assert_eq!(code, 0, "Angle client contract");
    println!("contract={code}");
    println!("oracle=ok");
}
