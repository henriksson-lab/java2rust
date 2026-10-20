// Compile with tests/fixtures/generic_reference_boundaries.java.
public class GenericReferenceBoundariesOracle {
    public static void main(String[] args) {
        int result = GenericReferenceBoundaries.run();
        if (result != 0) throw new AssertionError(result);
        System.out.println(result);
    }
}
