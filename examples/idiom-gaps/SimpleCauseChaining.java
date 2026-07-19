class SimpleCauseChaining {
    void copy(boolean bad) throws UserProblem {
        try {
            if (bad) {
                throw new IOException("io");
            }
        } catch (IOException ex) {
            throw new UserProblem("copy failed", ex);
        }
    }
}
