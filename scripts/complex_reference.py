"""Pinned Commons Numbers reference requests and accuracy checks (caller only)."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess

PIN = "ae8abefb51c2c4d1a05882d61cefc8015e493a8f"
PREFIX = "commons-numbers-complex/src/test/"


def pinned_bytes(checkout, relative):
    expected = subprocess.check_output(["git", "show", f"{PIN}:{relative}"], cwd=checkout)
    actual = (checkout / relative).read_bytes()
    if actual != expected:
        raise ValueError(f"reference source differs from pinned release: {relative}")
    return actual


def prepare(checkout, output):
    if subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=checkout, text=True).strip() != PIN:
        raise ValueError("wrong Commons Numbers source pin")
    test_path = PREFIX + "java/org/apache/commons/numbers/complex/CReferenceTest.java"
    test = pinned_bytes(checkout, test_path).decode()
    if not re.search(r"private static long globalMaxUlps = 0;", test):
        raise ValueError("unexpected upstream global accuracy override")
    methods = re.findall(r'assert(Bi)?Operation\("([a-z]+)", Complex::([a-z]+), (\d+)\);', test)
    data_dir = checkout / PREFIX / "resources/data"
    if len(methods) != 13 or {name for _, name, _, _ in methods} != {p.stem for p in data_dir.glob("*.txt")}:
        raise ValueError("unaccounted upstream reference operation or dataset")
    manifest = {"pin": PIN, "files": {}, "operations": [], "flagged_rows": []}
    utility = PREFIX + "java/org/apache/commons/numbers/complex/TestUtils.java"
    for path in (test_path, utility):
        manifest["files"][path] = hashlib.sha256(pinned_bytes(checkout, path)).hexdigest()
    lines = []
    for binary, name, method, ulps in methods:
        if name != method:
            raise ValueError("reference operation needs an explicit caller mapping")
        relative = PREFIX + f"resources/data/{name}.txt"
        contents = pinned_bytes(checkout, relative)
        manifest["files"][relative] = hashlib.sha256(contents).hexdigest()
        count = 0
        for number, line in enumerate(contents.decode().splitlines(), 1):
            if not line or line.startswith("#"):
                continue
            if line.startswith(";"):
                # Upstream's globalMaxUlps=0 selects LOAD, not IGNORE.
                manifest["flagged_rows"].append({"file": relative, "line": number, "text": line})
                line = line[1:]
            values = line.split()
            if len(values) != (6 if binary else 4):
                raise ValueError(f"unexpected reference row {relative}:{number}")
            lines.append("\t".join([name, str(number), ulps, *values]))
            count += 1
        manifest["operations"].append({"name": name, "method": method, "binary": bool(binary), "ulps": int(ulps), "rows": count})
    manifest["rows"] = len(lines)
    output.mkdir(parents=True, exist_ok=True)
    (output / "requests.tsv").write_text("\n".join(lines) + "\n")
    manifest["requests_sha256"] = hashlib.sha256((output / "requests.tsv").read_bytes()).hexdigest()
    (output / "reference-manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    return manifest


def bits(value):
    return int.from_bytes(struct.pack("!d", float(value)), "big")


def load_requests(path, manifest_path=None):
    requests = []
    keys = set()
    for line in path.read_text().splitlines():
        fields = line.split("\t")
        if len(fields) not in (7, 9):
            raise ValueError("malformed numerical reference request")
        name, number, ulps, *values = fields
        key = f"reference.{name}.{int(number)}"
        if key in keys or int(ulps) < 0:
            raise ValueError("duplicate request or diagnostic accuracy override")
        keys.add(key)
        requests.append({"key": key, "operation": name, "ulps": int(ulps), "expected": [bits(x) for x in values[-2:]]})
    if not requests:
        raise ValueError("empty reference domain")
    if manifest_path is not None:
        manifest = json.loads(manifest_path.read_text())
        if manifest["pin"] != PIN or manifest["requests_sha256"] != hashlib.sha256(path.read_bytes()).hexdigest():
            raise ValueError("reference requests do not match the frozen pinned manifest")
        counts = {item["name"]: 0 for item in manifest["operations"]}
        limits = {item["name"]: item["ulps"] for item in manifest["operations"]}
        for request in requests:
            operation = request["operation"]
            if operation not in counts or request["ulps"] != limits[operation]:
                raise ValueError("reference request changed an operation or accuracy bound")
            counts[operation] += 1
        if len(requests) != manifest["rows"] or any(counts[item["name"]] != item["rows"] for item in manifest["operations"]):
            raise ValueError("reference requests changed dataset coverage")
    return requests


def component_matches(expected, actual, ulps):
    exponent, fraction = 0x7ff0000000000000, 0xfffffffffffff
    nan = lambda value: value & exponent == exponent and value & fraction != 0
    if nan(expected) or nan(actual):
        return nan(expected) and nan(actual), 0
    if expected == actual:
        return True, 0
    # Stronger than upstream's bare bit-distance test: do not blur infinities
    # with finite neighbors, zeros with subnormals, or opposite zero signs.
    if ((expected ^ actual) >> 63 or expected & exponent == exponent or actual & exponent == exponent
            or expected & 0x7fffffffffffffff == 0 or actual & 0x7fffffffffffffff == 0):
        return False, None
    distance = abs(expected - actual)
    return distance <= ulps, distance


def validate_outputs(requests, java_text, rust_text):
    failures, deviations = [], []
    for backend, text in (("java", java_text), ("generated", rust_text)):
        lines = text.splitlines()
        if len(lines) != len(requests) + 1 or lines[-1:] != ["oracle=ok"]:
            raise ValueError(f"{backend}: wrong reference scope or incomplete execution")
        for request, line in zip(requests, lines):
            key, separator, value = line.partition("=")
            if not separator or key != request["key"] or not re.fullmatch(r"[0-9a-f]{1,16},[0-9a-f]{1,16}", value):
                raise ValueError(f"{backend}: changed key/order/shape: {line!r}")
            for component, (expected, actual) in enumerate(zip(request["expected"], map(lambda x: int(x, 16), value.split(",")))):
                matched, distance = component_matches(expected, actual, request["ulps"])
                entry = {"backend": backend, "key": key, "component": component, "bound_ulps": request["ulps"],
                         "expected": f"{expected:016x}", "actual": f"{actual:016x}", "distance_ulps": distance}
                if not matched:
                    failures.append(entry)
                if expected != actual:
                    deviations.append(entry)
    return failures, deviations


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("checkout", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    print(json.dumps(prepare(args.checkout.resolve(), args.output.resolve()), indent=2))
