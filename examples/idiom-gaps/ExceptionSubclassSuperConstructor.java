class ExceptionSubclassSuperConstructor {
    void fail() {
        throw new WorkspaceProblem("bad workspace");
    }
}

class WorkspaceProblem extends UserException {
    private static final long serialVersionUID = 1L;

    WorkspaceProblem(String message) {
        super(message);
    }
}
