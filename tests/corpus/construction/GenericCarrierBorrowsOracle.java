// Compile with tests/fixtures/generic_carrier_borrows.java.
public class GenericCarrierBorrowsOracle {
    public static void main(String[] args) {
        int result = GenericCarrierBorrows.run();
        if (result != 0) throw new AssertionError(result);
        System.out.println(result);
    }
}
