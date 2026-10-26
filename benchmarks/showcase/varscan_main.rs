use new_java2rust_focused_output::{JavaString, VarScan};
fn main() {
    let args = std::env::args().skip(1).map(|arg| Some(JavaString::fresh(arg))).collect();
    if let Err(error) = VarScan::main(args) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
