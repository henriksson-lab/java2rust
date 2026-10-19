public class SourceCallbackContractsOracle {
    static void rejectsNegative(java.util.function.IntPredicate callback) {
        try { callback.test(-1); }
        catch (IllegalArgumentException expected) { return; }
        throw new AssertionError("callback must propagate IllegalArgumentException");
    }
    public static void main(String[] arguments) {
        assert SourceCallbackContracts.aliased() == 4;
        assert SourceCallbackContracts.lambda(1);
        assert !SourceCallbackContracts.lambda(0);
        assert SourceCallbackContracts.reference(1);
        assert !SourceCallbackContracts.reference(0);
        assert SourceCallbackContracts.forwarded(1);
        assert !SourceCallbackContracts.forwarded(0);
        rejectsNegative(SourceCallbackContracts::lambda);
        rejectsNegative(SourceCallbackContracts::reference);
        rejectsNegative(SourceCallbackContracts::forwarded);
        assert SourceCallbackContracts.plain(1);
        assert !SourceCallbackContracts.plain(0);
        assert !SourceCallbackContracts.caught(-1);
        assert SourceCallbackContracts.caught(1);
        System.out.println("source-callback-contracts: ok");
    }
}
