import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location(
    "rng_behavior", Path(__file__).with_name("verify-commons-rng-1.0-behavior.py")
)
behavior = importlib.util.module_from_spec(spec)
spec.loader.exec_module(behavior)


class BehaviorComparisonTests(unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()
