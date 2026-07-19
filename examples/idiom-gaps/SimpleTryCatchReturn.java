class SimpleTryCatchReturn {
    int parse(boolean bad) {
        try {
            if (bad) {
                throw new IllegalArgumentException("bad");
            }
            return 7;
        } catch (IllegalArgumentException ex) {
            return 1;
        }
    }
}
