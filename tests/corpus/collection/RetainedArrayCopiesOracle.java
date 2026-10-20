public final class RetainedArrayCopiesOracle {
    public static void main(String[] args) {
        int copies = RetainedArrayCopies.run();
        int constructors = ConstructorArrayRebinding.run();
        int outer = OuterArrayArgument.run();
        if (copies != 0 || constructors != 0 || outer != 0) {
            throw new AssertionError("copy=" + copies + ",constructor=" + constructors);
        }
        System.out.println(copies + ":" + constructors + ":" + outer);
    }
}
