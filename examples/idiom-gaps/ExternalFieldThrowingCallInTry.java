class ExternalFieldThrowingCallInTry {
    void validate(Object value) throws ValidationProblem {
        try {
            Registry.DEFAULT_VALIDATOR.validate();
        } catch (ValidationProblem ex) {
            throw ex;
        }
    }
}
