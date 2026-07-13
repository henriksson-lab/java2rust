class ExceptionSubclassCatchOrder {
    int parse(Source source) {
        try {
            return source.read();
        } catch (SpecificProblem ex) {
            return 1;
        } catch (BaseProblem ex) {
            return 2;
        }
    }
}

class BaseProblem extends Exception {
    BaseProblem(String message) {
        super(message);
    }
}

class SpecificProblem extends BaseProblem {
    SpecificProblem(String message) {
        super(message);
    }
}
