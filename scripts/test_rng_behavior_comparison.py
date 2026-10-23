import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "rng_behavior", Path(__file__).with_name("verify-commons-rng-1.0-behavior.py")
)
behavior = importlib.util.module_from_spec(spec)
spec.loader.exec_module(behavior)


class BehaviorComparisonTests(unittest.TestCase):
    def test_arithmetic_nan_sign_policy_is_explicit_and_component_typed(self):
        java = "result=f64-arith(7ff8000000000042),f32-arith(7fc00007)\noracle=ok\n"
        rust = "result=f64-arith(fff8000000000042),f32-arith(ffc00007)\noracle=ok\n"
        with self.assertRaises(ValueError):
            behavior.compare_outputs(java, rust)
        permitted = []
        self.assertEqual(behavior.compare_outputs(java, rust, arithmetic_nan_sign=True, permitted=permitted), 2)
        self.assertEqual([entry["width"] for entry in permitted], [64, 32])
        self.assertEqual([entry["key"] for entry in permitted], ["result", "result"])
        self.assertEqual([entry["component"] for entry in permitted], [0, 1])

    def test_portability_does_not_relax_payloads_values_copies_or_control(self):
        for java, rust in [
            ("f64-arith(7ff8000000000001)", "f64-arith(fff8000000000002)"),
            ("f64-arith(7ff8000000000000)", "f64-arith(fff0000000000001)"),
            ("f64-arith(7ff0000000000000)", "f64-arith(fff0000000000000)"),
            ("f64-arith(0000000000000000)", "f64-arith(8000000000000000)"),
            ("f64-arith(3ff0000000000000)", "f64-arith(3ff0000000000001)"),
            ("f32-arith(00000000)", "f32-arith(80000000)"),
            ("f32-arith(7fc00001)", "f32-arith(ffc00002)"),
            ("7ff8000000000000", "fff8000000000000"),
            ("ERR:failure:f64-arith(7ff8000000000000)", "ERR:failure:f64-arith(fff8000000000000)"),
            ("f64-arith(7ff8000000000000),state=1", "f64-arith(fff8000000000000),state=2"),
            ("f64-arith(7ff8000000000000),raw=7ff8000000000000",
             "f64-arith(fff8000000000000),raw=fff8000000000000"),
            ("f64-arith(7ff8000000000000)", "f32-arith(ffc00000)"),
        ]:
            with self.subTest(java=java, rust=rust), self.assertRaises(ValueError):
                behavior.compare_outputs(f"result={java}\noracle=ok\n", f"result={rust}\noracle=ok\n",
                                         arithmetic_nan_sign=True)

    def test_portability_keeps_keys_order_scope_and_nonfloat_contracts_exact(self):
        for java, rust in [
            ("left=f64-arith(7ff8000000000000)", "right=f64-arith(fff8000000000000)"),
            ("state=1", "state=2"), ("trace=[2, 1]", "trace=[1, 2]"),
            ("raw=7fc00042", "raw=ffc00042"),
        ]:
            with self.subTest(java=java, rust=rust), self.assertRaises(ValueError):
                behavior.compare_outputs(java + "\noracle=ok\n", rust + "\noracle=ok\n",
                                         arithmetic_nan_sign=True)
        with self.assertRaises(ValueError):
            behavior.compare_outputs("oracle=ok\n", "", arithmetic_nan_sign=True)

    def test_malformed_annotation_cannot_be_a_portable_success(self):
        for value in ["f64-arith(z)", "f64-arith(7fc00000)", "f32-arith(7ff8000000000000)",
                      "f64-arith(7FF8000000000000)", "f64-arith(7ff8000000000000"]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                behavior.compare_outputs(f"result={value}\noracle=ok\n", f"result={value}\noracle=ok\n",
                                         arithmetic_nan_sign=True)

    def test_permitted_components_cannot_hide_another_mismatch(self):
        differences, permitted = [], []
        behavior.compare_outputs("value=f64-arith(7ff8000000000000)\nstate=1\noracle=ok\n",
                                 "value=f64-arith(fff8000000000000)\nstate=2\noracle=ok\n",
                                 differences, arithmetic_nan_sign=True, permitted=permitted)
        self.assertEqual(len(permitted), 1)
        self.assertEqual(len(differences), 1)
        self.assertIn("state=", differences[0])

    def test_collecting_all_mismatches_does_not_relax_values_or_keys(self):
        differences = []
        self.assertEqual(behavior.compare_outputs(
            "dd.bits=0,0\nnorm.bits=7ff8000000000000\nstate=2\noracle=ok\n",
            "dd.bits=8000000000000000,0\nnorm.bits=fff8000000000000\nother=2\noracle=ok\n",
            differences), 4)
        self.assertEqual(len(differences), 3)
        self.assertIn("dd.bits", differences[0])
        self.assertIn("norm.bits", differences[1])
        self.assertIn("other=2", differences[2])
        with self.assertRaises(ValueError):
            behavior.compare_outputs("oracle=ok\n", "", [])

    def test_decimal_spelling_may_differ_but_not_ieee_values(self):
        self.assertEqual(behavior.compare_outputs(
            "rng.float.0=1.0E-4\nrng.double.0=0.5\noracle=ok\n",
            "rng.float.0=0.0001\nrng.double.0=0.5000\noracle=ok\n"), 3)

    def test_value_and_contract_changes_are_rejected(self):
        for java, rust in [
            ("rng.float.0=0.5", "rng.float.0=0.50000006"),
            ("rng.double.0=0.5", "rng.double.0=0.5000000000000001"),
            ("rng.float.0=0.0", "rng.float.0=-0.0"),
            ("rng.long.0=9", "rng.long.0=8"),
            ("rng.bytes.0=[1, 2]", "rng.bytes.0=[2, 1]"),
            ("rng.long.0=9", "rng.long.1=9"),
        ]:
            with self.subTest(java=java, rust=rust), self.assertRaises(ValueError):
                behavior.compare_outputs(java + "\noracle=ok\n", rust + "\noracle=ok\n")

    def test_incomplete_runs_are_rejected(self):
        for java, rust in [("", ""), ("oracle=ok\n", ""), ("oracle=ok\n", "oracle=failed\n")]:
            with self.assertRaises(ValueError):
                behavior.compare_outputs(java, rust)

    def test_sampling_rng_traces_and_identity_labels_are_exact(self):
        for key, expected, changed in [
            ("sharedRng.bounds", "[4, 4, 5]", "[4, 5, 4]"),
            ("collection.sample.0", "1:42", "2:42"),
            ("list.sample.0", "[null, 1:7]", "[1:7, null]"),
        ]:
            java = f"{key}={expected}\noracle=ok\n"
            self.assertEqual(behavior.compare_outputs(java, java), 2)
            with self.assertRaises(ValueError):
                behavior.compare_outputs(java, f"{key}={changed}\noracle=ok\n")

    def test_distribution_state_display_and_float_bits_are_not_relaxed(self):
        for key, expected, changed in [
            ("gaussian.trace.1", "[]", "[d:4602678819172646912]"),
            ("inverseContinuous.callback.calls", "24", "48"),
            ("uniform.display", "ok", "bad"),
            ("gamma.2.0.double.0", "0.5", "0.5000000000000001"),
        ]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                behavior.compare_outputs(f"{key}={expected}\noracle=ok\n",
                                         f"{key}={changed}\noracle=ok\n")


if __name__ == "__main__":
    unittest.main()
