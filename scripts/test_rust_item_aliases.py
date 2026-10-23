"""Fast structural and compile controls for alias-aware support assembly."""

import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from rust_item_utils import attached_outer_item_start, expand_inherent_impl_alias, has_associated_definitions, impl_header_parts


class AliasImplTests(unittest.TestCase):
    def test_trait_only_aliases_and_private_contracts_follow_nominal_owner(self):
        kernel = r"""
pub struct Store<T>(pub T);
pub type Text = Store<Vec<u16>>;
pub type View = Text;
trait Units { fn units(&self) -> &[u16]; }
impl From<Vec<u16>> for View {
    fn from(value: Vec<u16>) -> Self { Store(value) }
}
impl Units for Text {
    fn units(&self) -> &[u16] { &self.0 }
}
impl<'a> Units for &'a View {
    fn units(&self) -> &[u16] { &self.0 }
}
"""
        with tempfile.TemporaryDirectory(prefix="new-java2rust-alias-traits-", dir=os.environ.get("TMPDIR")) as directory:
            root = Path(directory)
            generated = root / "generated"
            generated.mkdir()
            for name in ["First", "Second"]:
                (generated / f"{name}.rs").write_text(kernel + f"\npub struct {name};\n")
            shared = root / "shared.rs"
            result = subprocess.run([
                sys.executable, str(Path(__file__).with_name("merge-shared-support-facades.py")),
                "--generated-dir", str(generated), "--shared-support", str(shared),
                "--shared-types", str(root / "types.tsv"), "--conflicts", str(root / "conflicts.tsv"),
            ], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            text = shared.read_text()
            self.assertEqual(text.count("trait Units"), 1, text)
            self.assertEqual(text.count("impl From<Vec<u16>> for View"), 1, text)
            source = root / "probe.rs"
            source.write_text(text + '''
fn main() {
    let value = View::from(vec![0xd800, 65, 0xdc00]);
    assert_eq!(Units::units(&value), &[0xd800, 65, 0xdc00]);
    assert_eq!(Units::units(&&value), &[0xd800, 65, 0xdc00]);
}
''')
            binary = root / "probe"
            result = subprocess.run(["rustc", "--edition=2024", str(source), "-o", str(binary)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            subprocess.run([str(binary)], check=True)

    def test_moved_items_keep_all_outer_attributes(self):
        lines = '''pub struct Earlier;

#[repr(transparent)]
#[derive(Clone, Copy)]
/// A unit retaining its representation.
#[cfg_attr(
    feature = "quoted ] [ braces { }",
    allow(dead_code)
)]
pub struct Unit(u16);
'''.splitlines(keepends=True)
        self.assertEqual(attached_outer_item_start(lines, len(lines) - 1), 2)
        self.assertEqual(attached_outer_item_start(["#![allow(dead_code)]\n", "pub struct Unit;\n"], 1), 1)
        self.assertEqual(attached_outer_item_start(["let value = [1, 2];\n", "pub struct Unit;\n"], 1), 1)

    def test_associated_contracts_survive_actual_trait_promotion(self):
        kernel = r"""
pub trait AllocationAccess {
    type Value;
    const TAG: &'static str;
    fn read(&self) -> Self::Value;
}
impl<T: Clone> AllocationAccess for std::rc::Rc<std::cell::RefCell<T>> {
    type Value = T;
    const TAG: &'static str = "associated;{}";
    fn read(&self) -> Self::Value { self.borrow().clone() }
}
"""
        self.assertTrue(has_associated_definitions(kernel.split("impl", 1)[1]))
        self.assertFalse(has_associated_definitions("""
impl Value {
    const fn empty() -> usize { 0 }
    fn method() {
        type Local = usize;
        const LOCAL: Local = 1;
    }
}
"""))
        with tempfile.TemporaryDirectory(prefix="new-java2rust-associated-", dir=os.environ.get("TMPDIR")) as directory:
            root = Path(directory)
            generated = root / "generated"
            generated.mkdir()
            for name in ["First", "Second"]:
                variant = kernel if name == "First" else kernel.replace("type Value;", "type Value; // same ABI, distinct declaration text")
                (generated / f"{name}.rs").write_text(variant + f"\npub struct {name};\n")
            shared = root / "shared.rs"
            types = root / "shared-types.tsv"
            shared.write_text("")
            types.write_text("")
            result = subprocess.run([
                sys.executable, str(Path(__file__).with_name("merge-shared-support-facades.py")),
                "--generated-dir", str(generated), "--shared-support", str(shared),
                "--shared-types", str(types), "--conflicts", str(root / "conflicts.tsv"),
            ], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            text = shared.read_text()
            self.assertEqual(text.count("type Value = T;"), 1, text)
            self.assertEqual(text.count('const TAG: &\'static str = "associated;{}";'), 1, text)
            source = root / "probe.rs"
            source.write_text(text + """
fn main() {
    let value = std::rc::Rc::new(std::cell::RefCell::new(17));
    assert_eq!(AllocationAccess::read(&value), 17);
    assert_eq!(<std::rc::Rc<std::cell::RefCell<i32>> as AllocationAccess>::TAG, "associated;{}");
}
""")
            binary = root / "probe"
            result = subprocess.run(["rustc", "--edition=2024", str(source), "-o", str(binary)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            subprocess.run([str(binary)], check=True)

    def test_expansion_preserves_applied_arguments_and_bounds(self):
        alias = "pub type Local<T> = Store<T, Backend<T>>;"
        self.assertEqual(
            expand_inherent_impl_alias("impl<U: Clone> Local<U> where U: Send", alias),
            "impl<U: Clone> Store<U, Backend<U>> where U: Send",
        )
        self.assertEqual(
            expand_inherent_impl_alias("impl<U> Local<U>\nwhere U: Send,", alias),
            "impl<U> Store<U, Backend<U>>\nwhere U: Send,",
        )
        self.assertEqual(
            expand_inherent_impl_alias(
                "impl<T, const N: usize> Local<T, N>",
                "pub type Local<T, const N: usize> = Store<[T; N], Backend<(T, T)>>;",
            ),
            "impl<T, const N: usize> Store<[T; N], Backend<(T, T)>>",
        )
        self.assertEqual(
            expand_inherent_impl_alias("impl Alias", "pub type Alias = Target;"),
            "impl Target",
        )
        self.assertEqual(
            expand_inherent_impl_alias(
                "impl<'x, T> View<'x, T>",
                "pub type View<'a, T> = Store<&'a T, Backend<T>>;",
            ),
            "impl<'x, T> Store<&'x T, Backend<T>>",
        )
        self.assertIsNone(expand_inherent_impl_alias(
            "impl View", "pub type View<T> = Store<T, Backend<T>>;",
        ))
        self.assertEqual(
            impl_header_parts("impl<F: Fn() -> Box<dyn Any>> Alias<F> {"),
            (None, "Alias<F>"),
        )

    def test_specialized_methods_and_constants_survive_the_actual_merger(self):
        kernel = r"""
pub struct Store<T, B> { pub values: Vec<T>, pub backing: B }
pub struct LocalBacking<T> { pub marker: std::marker::PhantomData<T> }
pub struct RemoteBacking<T> { pub marker: std::marker::PhantomData<T> }
pub type Local<T> = Store<T, LocalBacking<T>>;
pub type Remote<T> = Store<T, RemoteBacking<T>>;
impl<T, B> Store<T, B> {
    pub fn len(&self) -> usize { self.values.len() }
}
impl<T: Clone> Local<T> {
    pub const KIND: u8 = 1;
    pub fn local(value: T) -> Self {
        Self { values: vec![value.clone()], backing: LocalBacking { marker: std::marker::PhantomData } }
    }
}
impl<T> Remote<T>
where T: Default,
{
    pub const KIND: u8 = 2;
    pub fn remote() -> Self {
        Self { values: vec![T::default()], backing: RemoteBacking { marker: std::marker::PhantomData } }
    }
}
"""
        with tempfile.TemporaryDirectory(prefix="new-java2rust-alias-", dir=os.environ.get("TMPDIR")) as directory:
            root = Path(directory)
            generated = root / "generated"
            generated.mkdir()
            for index, name in enumerate(["First", "Second"], 1):
                (generated / f"{index:05}_{name}.rs").write_text(kernel + f"\npub struct {name};\n")
            shared = root / "shared.rs"
            types = root / "shared-types.tsv"
            shared.write_text("")
            types.write_text("")
            result = subprocess.run([
                sys.executable, str(Path(__file__).with_name("merge-shared-support-facades.py")),
                "--generated-dir", str(generated), "--shared-support", str(shared),
                "--shared-types", str(types), "--conflicts", str(root / "conflicts.tsv"),
            ], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            text = shared.read_text()
            self.assertIn("impl<T: Clone> Store<T, LocalBacking<T>>", text)
            self.assertIn("impl<T> Store<T, RemoteBacking<T>>\nwhere T: Default,", text)
            self.assertEqual(text.count("pub const KIND:"), 2, text)
            source = root / "probe.rs"
            source.write_text(text + """
fn main() {
    let local = Local::<u8>::local(7);
    let remote = Remote::<u8>::remote();
    assert_eq!(local.values, vec![7]);
    assert_eq!(remote.values, vec![0]);
    assert_eq!(Local::<u8>::KIND, 1);
    assert_eq!(Remote::<u8>::KIND, 2);
    assert_eq!(local.len(), remote.len());
}
""")
            binary = root / "probe"
            result = subprocess.run(["rustc", "--edition=2024", str(source), "-o", str(binary)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            subprocess.run([str(binary)], check=True)


if __name__ == "__main__":
    unittest.main()
