import subprocess
import sys
import runpy
import tempfile
import unittest
from pathlib import Path

merge_registries = runpy.run_path(str(Path(__file__).with_name("merge-shared-support-facades.py")))["merge_additive_runtime_registries"]


def dispatcher(cases=(), fallback="None"):
    arms = "".join(f'    if let Some(value) = value.downcast_ref::<{name}>() {{ return Some(value.label().to_string()); }}\n' for name in cases)
    return f'''pub fn dispatch(value: &dyn std::any::Any) -> Option<String> {{
    // java2rust: additive-registry-begin object_string
{arms}    // java2rust: additive-registry-end object_string
    {fallback}
}}
'''


class ObjectStringRegistryTests(unittest.TestCase):
    def test_unions_cases_instead_of_selecting_a_module(self):
        merged = merge_registries([dispatcher(), dispatcher(["Alpha"]), dispatcher(["Beta"]), dispatcher(["Alpha"])])
        self.assertIsNotNone(merged)
        self.assertEqual(merged.count("downcast_ref::<Alpha>"), 1)
        self.assertEqual(merged.count("downcast_ref::<Beta>"), 1)
        self.assertLess(merged.index("downcast_ref::<Beta>"), merged.rindex("None"))

    def test_conflicting_cases_fallbacks_and_effects_are_not_merged(self):
        first = dispatcher(["Alpha"])
        for other in [
            first.replace("value.label()", '"different"'),
            dispatcher(["Beta"], 'Some("different fallback".into())'),
            dispatcher(["Beta"]).replace("    if let", "    touch();\n    if let"),
        ]:
            with self.subTest(other=other), self.assertRaises(ValueError):
                merge_registries([first, other])

    def test_literal_braces_are_not_structural_delimiters(self):
        first = dispatcher(["Alpha"]).replace("value.label()", '"literal } {"')
        self.assertIn('"literal } {"', merge_registries([first, dispatcher(["Beta"])]))

    def test_cli_reconciles_an_existing_shared_dispatcher_and_executes_all_cases(self):
        with tempfile.TemporaryDirectory(prefix="java2rust_dispatch_merge_") as directory:
            root = Path(directory)
            generated, raw = root / "generated", root / "raw"
            generated.mkdir()
            raw.mkdir()
            for i, name in enumerate(["Alpha", "Beta"]):
                source = f'''use crate::dispatch;
pub struct {name};
impl {name} {{
    pub fn label(&self) -> &str {{ "{name.lower()}" }}
    pub fn probe() -> String {{ dispatch(&Self).unwrap() }}
}}
'''
                path = f"{i:05}_{name}.rs"
                (generated / path).write_text(source)
                (raw / path).write_text(source + dispatcher([name]))
            shared = root / "shared.rs"
            shared.write_text(dispatcher())
            types, conflicts = root / "types.txt", root / "conflicts.tsv"
            types.write_text("")
            subprocess.run([sys.executable, str(Path(__file__).with_name("merge-shared-support-facades.py")),
                            "--generated-dir", str(generated), "--merge-input-dir", str(raw),
                            "--shared-support", str(shared), "--shared-types", str(types),
                            "--conflicts", str(conflicts)], check=True)
            self.assertIn("merged runtime registrations", conflicts.read_text())
            harness = root / "harness.rs"
            harness.write_text(f'''mod alpha {{ include!({str(generated / '00000_Alpha.rs')!r}); }}
mod beta {{ include!({str(generated / '00001_Beta.rs')!r}); }}
pub use alpha::Alpha;
pub use beta::Beta;
include!({str(shared)!r});
fn main() {{ assert_eq!(Alpha::probe(), "alpha"); assert_eq!(Beta::probe(), "beta"); }}
'''.replace("'", '"'))
            binary = root / "harness"
            subprocess.run(["rustc", "--edition=2021", str(harness), "-o", str(binary)], check=True)
            subprocess.run([str(binary)], check=True)


if __name__ == "__main__":
    unittest.main()
