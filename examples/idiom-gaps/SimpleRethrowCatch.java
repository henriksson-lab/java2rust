class SimpleRethrowCatch {
    void validate(boolean bad) throws IllegalArgumentException {
        try {
            if (bad) {
                throw new IllegalArgumentException("bad");
            }
        } catch (IllegalArgumentException ex) {
            throw ex;
        }
    }
}
