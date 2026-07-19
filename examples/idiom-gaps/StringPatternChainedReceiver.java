class StringPatternChainedReceiver {
    boolean compressed(String path, String suffix) {
        return path.toString().toLowerCase().endsWith(suffix);
    }
}
