class ExceptionBaseCatchCoversSubclass {
    String load(Loader loader) {
        try {
            return loader.load();
        } catch (RecoverableProblem ex) {
            return ex.getMessage();
        }
    }
}

class RecoverableProblem extends Exception {
    RecoverableProblem(String message) {
        super(message);
    }
}

class MissingFileProblem extends RecoverableProblem {
    MissingFileProblem(String path) {
        super(path);
    }
}
