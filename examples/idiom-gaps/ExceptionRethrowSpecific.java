class ExceptionRethrowSpecific {
    void validate(Validator validator) throws ValidationProblem {
        try {
            validator.validate();
        } catch (ValidationProblem ex) {
            throw ex;
        }
    }
}

class ValidationProblem extends Exception {
    ValidationProblem(String message) {
        super(message);
    }
}
