public class InstanceFieldCallbacksOracle {
    public static void main(String[] arguments) {
        assert FieldCallbacks.probe() == 2;
        assert FieldCallbacks.matches();
        assert !FieldCallbacks.misses();
        System.out.println("stream-instance-fields: ok");
    }
}
