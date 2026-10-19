public class InheritedIterableIdentityOracle {
    public static void main(String[] args) {
        int actual = InheritedIterableIdentity.run();
        if (actual != 9) { throw new AssertionError("inherited Iterable: " + actual); }
        System.out.println(actual);
    }
}
