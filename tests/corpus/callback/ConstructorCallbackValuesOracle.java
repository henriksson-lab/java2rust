public class ConstructorCallbackValuesOracle {
    public static void main(String[] args) {
        int lambdas = ConstructorCallbackValues.lambdas();
        int references = ConstructorCallbackValues.references();
        int deferred = ConstructorCallbackValues.deferredFailure();
        int delegated = ConstructorCallbackValues.delegated();
        int nullable = ConstructorCallbackValues.nullable();
        int sequence = ConstructorCallbackValues.sequenceResult();
        int borrowed = ConstructorCallbackValues.borrowedResult();
        int stored = ConstructorCallbackValues.storedSequence();
        int nested = ConstructorCallbackValues.nestedDelegation();
        int nestedFailure = ConstructorCallbackValues.nestedFailure();
        int delegatedResult = ConstructorCallbackValues.delegatedResult();
        if (lambdas != 23 || references != 7 || deferred != -1 || delegated != 9 || nullable != 4 || sequence != 9 || borrowed != 6 || stored != 6 || nested != 69 || nestedFailure != -3 || delegatedResult != 9) {
            throw new AssertionError(lambdas + ":" + references + ":" + deferred);
        }
        System.out.println("lambdas=" + lambdas + ";references=" + references + ";deferred=" + deferred
            + ";delegated=" + delegated + ";nullable=" + nullable + ";sequence=" + sequence
            + ";borrowed=" + borrowed + ";stored=" + stored + ";nested=" + nested
            + ";nestedFailure=" + nestedFailure + ";delegatedResult=" + delegatedResult);
    }
}
