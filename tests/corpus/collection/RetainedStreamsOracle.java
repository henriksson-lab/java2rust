public class RetainedStreamsOracle {
    public static void main(String[] args) {
        System.out.println(RetainedStreams.matches());
        System.out.println(RetainedStreams.mapped());
        System.out.println(RetainedStreams.forEach());
        System.out.println(RetainedStreams.failFast());
        System.out.println(RetainedStreams.nestedReference());
        System.out.println(RetainedStreams.nullableSource(false));
        System.out.println(RetainedStreams.nullableSource(true));
        System.out.println(RetainedStreams.nestedSizes());
        System.out.println(RetainedStreams.nullableParameterCall(false));
        System.out.println(RetainedStreams.nullableParameterCall(true));
    }
}
