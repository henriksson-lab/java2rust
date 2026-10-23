trait Completion {
    fn check(self);
}
impl Completion for () {
    fn check(self) {}
}
impl<E: std::fmt::Debug> Completion for Result<(), E> {
    fn check(self) {
        self.unwrap();
    }
}
fn main() {
    new_java2rust_focused_output::TextAssemblyClient::main(&[]).check();
}
