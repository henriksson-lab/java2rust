import external.lib.ExternalUtility;

class ImportedStaticUtilityOwner {
    Object read(String path) {
        return ExternalUtility.readValue(path);
    }

    Object shadowed(String path) {
        ExternalUtility local = new ExternalUtility();
        return local.readValue(path);
    }
}
