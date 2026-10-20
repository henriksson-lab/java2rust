"""Cheap controls for additive runtime registrations in shared support."""
from pathlib import Path
import runpy
import subprocess
import tempfile
import unittest

SCRIPT = Path(__file__).with_name("merge-shared-support-facades.py")
merge = runpy.run_path(str(SCRIPT))["merge_additive_runtime_registries"]


def registry(arm):
    return f"""impl JavaReferenceIdentity for dyn std::any::Any {{
    fn java_reference_identity(&self) -> *const () {{
        // java2rust: additive-registry-begin reference_identity
        if let Some(value) = self.downcast_ref::<std::rc::Rc<Leaf{arm}>>() {{ return value.java_reference_identity(); }}
        // java2rust: additive-registry-end reference_identity
        panic!("unsupported identity")
    }}
}}"""


class RuntimeRegistryTests(unittest.TestCase):
    def test_union_is_order_independent_and_idempotent(self):
        left, right = registry("A"), registry("B")
        merged = merge([left, right, left])
        self.assertEqual(merged, merge([right, left]))
        self.assertEqual(merged.count("Rc<LeafA>"), 1)
        self.assertEqual(merged.count("Rc<LeafB>"), 1)
        self.assertEqual(merge([merged, left]), merged)

    def test_different_fallbacks_are_not_silently_selected(self):
        with self.assertRaises(ValueError):
            merge([registry("A"), registry("B").replace("unsupported identity", "different")])
        with self.assertRaises(ValueError):
            merge([registry("A"), "fn unrelated() {}"])
        self.assertIsNone(merge(["fn ordinary() {}", "fn other() {}"]))

    def test_assembled_registry_retains_both_modules_at_runtime(self):
        common = """
pub trait JavaReferenceIdentity { fn java_reference_identity(&self) -> *const (); }
impl<T> JavaReferenceIdentity for std::rc::Rc<T> {
    fn java_reference_identity(&self) -> *const () { std::rc::Rc::as_ptr(self) as *const () }
}
pub struct LeafA(pub i32);
pub struct LeafB(pub i32);
"""
        with tempfile.TemporaryDirectory(prefix="java-reference-registry-") as temp:
            root = Path(temp)
            generated = root / "generated"
            generated.mkdir()
            for arm in ("A", "B"):
                (generated / f"Use{arm}.rs").write_text(common + registry(arm) + f"\npub struct Use{arm};\n")
            shared = root / "shared.rs"
            result = subprocess.run([
                "python3", str(SCRIPT), "--generated-dir", str(generated),
                "--shared-support", str(shared), "--shared-types", str(root / "types.tsv"),
                "--conflicts", str(root / "conflicts.tsv"),
            ], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            source = shared.read_text() + """
fn main() {
    let a = std::rc::Rc::new(LeafA(1));
    let b = std::rc::Rc::new(LeafB(2));
    let erased_a: &dyn std::any::Any = &a;
    let erased_b: &dyn std::any::Any = &b;
    assert_eq!(a.java_reference_identity(), erased_a.java_reference_identity());
    assert_eq!(b.java_reference_identity(), erased_b.java_reference_identity());
}
"""
            rust = root / "main.rs"
            rust.write_text(source)
            binary = root / "probe"
            result = subprocess.run(["rustc", "--edition=2024", str(rust), "-o", str(binary)], capture_output=True, text=True)
            self.assertEqual(result.returncode, 0, result.stderr)
            subprocess.run([str(binary)], check=True)


if __name__ == "__main__":
    unittest.main()
