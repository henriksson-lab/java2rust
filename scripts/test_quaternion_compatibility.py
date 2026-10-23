#!/usr/bin/env python3
"""Negative/structural checks for the optional Quaternion compatibility gate."""
import importlib.util
from pathlib import Path
import unittest
import tempfile

spec = importlib.util.spec_from_file_location("quaternion_audit", Path(__file__).with_name("audit-quaternion-interpolation.py"))
audit = importlib.util.module_from_spec(spec)
spec.loader.exec_module(audit)


class ComparisonClassificationTests(unittest.TestCase):
    # These test classification only; the CLI additionally audits all 96
    # production-call rotations, phase/plane/unit length and callback repeats.
    base = "3ff0000000000000,3fe0000000000000,0,0"
    rounded = "3ff0000000000001,3fe0000000000000,0,0"

    def row(self, key, value=None):
        return f"{key}={self.base if value is None else value}"

    def test_only_spherical_finite_nonendpoint_rows_can_be_classified_as_rounding(self):
        key = "slerp.3.3.apply"
        self.assertEqual(len(audit.compare_rows([self.row(key)], [self.row(key, self.rounded)])), 1)
        for key in ["slerp.1.3.apply", "slerp.2.3.apply", "slerp.3.1.apply", "slerp.3.2.apply", "slerp.3.6.apply", "q.1.norm", "slerp.null.start"]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                audit.compare_rows([self.row(key)], [self.row(key, self.rounded)])

    def test_exception_messages_remain_exact(self):
        with self.assertRaises(ValueError):
            audit.compare_rows(["null.add=ERR:NullPointerException:message"], ["null.add=ERR:NullPointerException:other"])

    def test_state_rows_and_keys_cannot_disappear_or_move(self):
        for expected, actual in [(["a=1"], []), (["a=1"], ["b=1"]), (["a=1", "a=1"], ["a=1", "a=1"]), (["a=1", "b=2"], ["b=2", "a=1"])]:
            with self.subTest(actual=actual), self.assertRaises(ValueError):
                audit.compare_rows(expected, actual)

    def test_zero_signs_nonfinite_values_and_malformed_components_cannot_be_waived(self):
        key = "slerp.3.3.apply"
        for value in ["3ff0000000000000,3fe0000000000000,8000000000000000,0", "7ff0000000000000,3fe0000000000000,0,0", "7ff8000000000000,3fe0000000000000,0,0", "ERR:IllegalStateException:x", "bad", "1,2,3,4,5"]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                audit.compare_rows([self.row(key)], [self.row(key, value)])

    def test_unit_length_alone_cannot_accept_a_wrong_interpolation_algorithm(self):
        # A no-op identity quaternion is finite/unit-length but is not the
        # correct plane/phase for the actual spherical input pairs.
        rows = [f"slerp.{pair}.{index}.{mode}=3ff0000000000000,0,0,0"
                for pair in range(4) for index in range(8)
                for mode in ("apply", "callback", "repeat")]
        with tempfile.TemporaryDirectory(prefix="quaternion-gate-negative-") as directory:
            path = Path(directory) / "wrong-output.txt"
            path.write_text("\n".join(rows) + "\noracle=ok\n")
            with self.assertRaisesRegex(ValueError, "geometric accuracy failure"):
                audit.audit(path)


if __name__ == "__main__":
    unittest.main()
