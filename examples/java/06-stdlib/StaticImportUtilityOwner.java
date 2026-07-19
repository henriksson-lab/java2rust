import static external.lib.StaticUtility.readValue;

class StaticImportUtilityOwner {
    Object imported(String path) {
        return readValue(path);
    }

    Object qualified(String path) {
        return external.lib.StaticUtility.readValue(path);
    }

    Object local(String path) {
        return localValue(path);
    }

    Object localValue(String path) {
        return path;
    }
}
