public class RetainedCallbackStateOracle {
    public static void main(String[] arguments) {
        java.util.function.Supplier<Integer> first = RetainedCallbackState.counter(3);
        java.util.function.Supplier<Integer> alias = first;
        assert first.get() == 3;
        assert alias.get() == 4;
        assert first.get() == 5;
        assert RetainedCallbackState.sharedCounter(3) == 34;
        assert RetainedCallbackState.localCounter() == 34;
        assert RetainedCallbackState.reference().getAsInt() == 7;
        assert RetainedCallbackState.held(9) == 9;
        assert RetainedCallbackState.invokeReturned(new int[] { 4 }, 0) == 4;
        assert RetainedCallbackState.caughtReturned(new int[] { 4 }, -1) == -1;
        assert PureCallbackCarrier.value(5).getAsInt() == 5;
        assert PureCallbackCarrier.local() == 14;
        assert RetainedCallbackState.plain(7).getAsInt() == 7;
        java.util.function.IntSupplier deferred = RetainedCallbackState.indexed(new int[] { 4 }, -1);
        try {
            deferred.getAsInt();
            throw new AssertionError("index exception must occur on invocation");
        } catch (ArrayIndexOutOfBoundsException expected) {
            // Creation succeeds; the exception belongs to the callback call.
        }
        assert RetainedCallbackState.caughtIndex(new int[] { 4 }, 0) == 4;
        assert RetainedCallbackState.caughtIndex(new int[] { 4 }, -1) == -1;
        System.out.println("retained-callback-state: ok");
    }
}
