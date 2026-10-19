public class InterfaceMethodCapabilityOracle {
    public static void main(String[] arguments) {
        assert InterfaceMethodCapability.<String>create().test("value");
        System.out.println("interface-method-capability: ok");
    }
}
