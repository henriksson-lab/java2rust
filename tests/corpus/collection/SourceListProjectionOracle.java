public class SourceListProjectionOracle {
    public static void main(String[] args) {
        int read = SourceListProjection.liveRead();
        int mutation = SourceListProjection.mutationThroughCast();
        if (read != 47 || mutation != 28) {
            throw new AssertionError("source List projection: " + read + ", " + mutation);
        }
        System.out.println(read + "\t" + mutation);
    }
}
