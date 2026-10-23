import tempfile
import hashlib
import json
from pathlib import Path
import unittest

import complex_reference as reference


class ReferenceComparisonTests(unittest.TestCase):
    def request(self, real=0x3ff0000000000000, imag=0x4000000000000000, ulps=2):
        return [{"key": "reference.exp.18", "operation": "exp", "ulps": ulps, "expected": [real, imag]}]

    def stream(self, real=0x3ff0000000000000, imag=0x4000000000000000):
        return f"reference.exp.18={real:016x},{imag:016x}\noracle=ok\n"

    def test_both_backends_are_checked_against_independent_expected_values(self):
        failures, _ = reference.validate_outputs(self.request(), self.stream(), self.stream(real=0x3ff0000000000002))
        self.assertFalse(failures)
        failures, _ = reference.validate_outputs(self.request(), self.stream(real=0x3ff0000000000003), self.stream(real=0x3ff0000000000003))
        self.assertEqual({item["backend"] for item in failures}, {"java", "generated"})

    def test_exact_operation_is_not_given_a_rounding_allowance(self):
        failures, _ = reference.validate_outputs(self.request(ulps=0), self.stream(), self.stream(real=0x3ff0000000000001))
        self.assertEqual(len(failures), 1)

    def test_signed_zeros_infinities_and_classes_cannot_be_blurred(self):
        for expected, actual in [(0, 1), (1, 0), (0, 1 << 63), (0x7ff0000000000000, 0x7fefffffffffffff),
                                 (0x7ff0000000000000, 0xfff0000000000000), (0x7ff8000000000042, 0x7ff0000000000000),
                                 (0x3ff0000000000000, 0x7ff8000000000000), (0x3ff0000000000000, 0xbff0000000000000)]:
            with self.subTest(expected=expected, actual=actual):
                self.assertFalse(reference.component_matches(expected, actual, 1000000)[0])

    def test_nan_canonicalization_is_only_a_reference_value_rule(self):
        self.assertTrue(reference.component_matches(0x7ff8000000000000, 0xfff8000000001234, 0)[0])
        # The ordinary raw-bit comparator remains independently strict.

    def test_changed_order_keys_scope_and_component_shape_fail(self):
        for stream in ["oracle=ok\n", self.stream().replace("reference.exp.18", "reference.exp.19"),
                       self.stream().replace(",4000000000000000", ""), self.stream().replace("oracle=ok", "oracle=failed"),
                       self.stream().replace("3ff0000000000000", "ERR:exception")]:
            with self.subTest(stream=stream), self.assertRaises(ValueError):
                reference.validate_outputs(self.request(), self.stream(), stream)
        failures, _ = reference.validate_outputs(self.request(), self.stream(), self.stream(0x4000000000000000, 0x3ff0000000000000))
        self.assertEqual(len(failures), 2)

    def test_request_loader_rejects_duplicates_and_negative_diagnostic_limits(self):
        row = "exp\t18\t2\t1\t2\t3\t4\n"
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "requests.tsv"
            for text in (row + row, row.replace("\t2\t1", "\t-1\t1"), ""):
                path.write_text(text)
                with self.subTest(text=text), self.assertRaises(ValueError):
                    reference.load_requests(path)

    def test_frozen_manifest_rejects_changed_bytes_bounds_or_scope(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "requests.tsv"
            manifest_path = Path(temporary) / "reference-manifest.json"
            path.write_text("exp\t18\t2\t1\t2\t3\t4\n")
            manifest = {"pin": reference.PIN, "rows": 1,
                        "requests_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                        "operations": [{"name": "exp", "ulps": 2, "rows": 1}]}
            manifest_path.write_text(json.dumps(manifest))
            self.assertEqual(len(reference.load_requests(path, manifest_path)), 1)
            for field, value in [("pin", "other"), ("rows", 2), ("requests_sha256", "changed"),
                                 ("operations", [{"name": "exp", "ulps": 3, "rows": 1}]),
                                 ("operations", [{"name": "exp", "ulps": 2, "rows": 2}])]:
                changed = {**manifest, field: value}
                manifest_path.write_text(json.dumps(changed))
                with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                    reference.load_requests(path, manifest_path)


if __name__ == "__main__":
    unittest.main()
