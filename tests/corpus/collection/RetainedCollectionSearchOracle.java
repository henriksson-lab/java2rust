public class RetainedCollectionSearchOracle {
    public static void main(String[] args) {
        int actual = RetainedCollectionSearch.run();
        if (actual != 132) { throw new AssertionError("search: " + actual); }
        System.out.println(actual);
    }
}
