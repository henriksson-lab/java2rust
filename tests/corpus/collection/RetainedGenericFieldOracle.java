public class RetainedGenericFieldOracle {
    public static void main(String[] args) {
        int actual = SharedGenericSet.run();
        if (actual != 22) { throw new AssertionError(actual); }
        System.out.println(actual);
    }
}
