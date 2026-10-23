//! Diagnostic caller only. No text operations are reimplemented here.
use translated::{JavaException, UnicodeIndexingProbe as P};
trait Observation { fn observation(self) -> String; }
impl Observation for String { fn observation(self) -> String { self } }
impl<T: std::fmt::Display> Observation for Result<T,JavaException> {
    fn observation(self) -> String {
        self.map(|v|v.to_string()).unwrap_or_else(|e|format!("ERR:{}",e.class_name()))
    }
}
fn out<T:Observation>(key:&str, call:impl FnOnce()->T) {
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||call().observation()));
    println!("{key}={}",result.unwrap_or_else(|_|"PANIC".into()));
}
fn main() {
    out("length",P::length); out("units",P::units);
    out("positions",P::positions); out("wholeSlice",P::whole_slice);
    out("splitPair",P::split_pair); out("numericChar",P::numeric_char);
}
