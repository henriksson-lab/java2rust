class InternalThrowingCallInTry {
    void read(boolean bad) throws IOException {
        if (bad) {
            throw new IOException("io");
        }
    }

    void copy(boolean bad) throws UserProblem {
        try {
            read(bad);
        } catch (IOException ex) {
            throw new UserProblem("copy failed", ex);
        }
    }
}
