class OptionalStringArrayFieldViews {
    String[] readPaths;
    String[] rdfPaths;
    int pos;

    OptionalStringArrayFieldViews(String[] readPaths, String[] rdfPaths) {
        this.readPaths = readPaths;
        this.rdfPaths = rdfPaths;
    }

    int percent() {
        return (pos * 100) / readPaths.length;
    }

    boolean hasNext() {
        return pos < readPaths.length;
    }

    String pathFor(int r) {
        return readPaths[pos] + rdfPaths[r];
    }
}
