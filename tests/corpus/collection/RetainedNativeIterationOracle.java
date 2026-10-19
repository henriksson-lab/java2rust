public class RetainedNativeIterationOracle {
    public static void main(String[] args) {
        System.out.println(RetainedNativeIteration.list());
        System.out.println(RetainedNativeIteration.set());
        System.out.println(RetainedNativeIteration.failFast());
        System.out.println(RetainedNativeIteration.nullSource());
        System.out.println(RetainedNativeIteration.fieldReplacement());
        System.out.println(RetainedNativeIteration.arraySource());
        System.out.println(RetainedNativeIteration.nullElement());
        System.out.println(RetainedNativeIteration.nullableElement());
        System.out.println(RetainedNativeIteration.forwardedUnboxing());
        GenericIteration<Integer> generic = new GenericIteration<>();
        generic.add(3);
        System.out.println(generic.countNonNull());
    }
}
