//! Invoke the automatically translated client against untouched Complex algorithms.
trait ContractResult { fn code(self) -> i32; }
impl ContractResult for i32 { fn code(self) -> i32 { self } }
impl<E: std::fmt::Debug> ContractResult for Result<i32,E> {
    fn code(self) -> i32 { self.unwrap() }
}
fn main() {
    let code = translated::ComplexContractsOracle::run().code();
    assert_eq!(code,0,"translated Complex client contract");
    println!("contract={code}");
    println!("oracle=ok");
}
