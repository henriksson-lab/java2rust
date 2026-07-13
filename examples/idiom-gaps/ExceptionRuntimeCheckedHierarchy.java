class ExceptionRuntimeCheckedHierarchy {
    int classify(Worker worker) throws CheckedProblem {
        try {
            return worker.run();
        } catch (UncheckedProblem ex) {
            return -1;
        } catch (CheckedProblem ex) {
            throw ex;
        }
    }
}

class CheckedProblem extends Exception {
    CheckedProblem(String message) {
        super(message);
    }
}

class UncheckedProblem extends RuntimeException {
    UncheckedProblem(String message) {
        super(message);
    }
}
