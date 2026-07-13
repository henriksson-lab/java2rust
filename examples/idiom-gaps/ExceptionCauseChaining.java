class ExceptionCauseChaining {
    void copy(Copier copier) throws UserProblem {
        try {
            copier.copy();
        } catch (IOException ex) {
            throw new UserProblem("copy failed", ex);
        }
    }
}

class UserProblem extends Exception {
    UserProblem(String message, Throwable cause) {
        super(message, cause);
    }
}
