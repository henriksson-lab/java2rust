public class SourceCallbackForwardingOracle {
    public static void main(String[] arguments) {
        assert SourceCallbackForwarding.plain(1);
        assert !SourceCallbackForwarding.plain(0);
        assert SourceCallbackForwarding.reference(1);
        assert !SourceCallbackForwarding.reference(0);
        assert !SourceCallbackForwarding.mutable(1);
        System.out.println("source-callback-forwarding: ok");
    }
}
