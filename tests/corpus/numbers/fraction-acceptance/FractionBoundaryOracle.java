/** Independent caller: javac only; the boundary adapter is translated separately. */
public final class FractionBoundaryOracle {
    public static void main(String[] args) {
        for (int which=0;which<23;which++) {
            System.out.println("null:"+which+"\t"+FractionNullBoundary.check(which));
        }
        for (int which=0;which<6;which++) {
            System.out.println("comparable:"+which+"\t"+FractionNullBoundary.compare(which));
        }
    }
}
