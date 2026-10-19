public class LiveIterableSupplierOracle {
    public static void main(String[] args) {
        int direct = LiveIterableSupplier.direct();
        int throughInterface = LiveIterableSupplier.throughInterface();
        if (direct != 16 || throughInterface != 16) {
            throw new AssertionError(direct + ":" + throughInterface);
        }
        System.out.println("direct=" + direct + ";interface=" + throughInterface);
    }
}
