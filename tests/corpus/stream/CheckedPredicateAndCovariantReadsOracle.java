package sample.streams;

public class CheckedPredicateAndCovariantReadsOracle {
    public static void main(String[] args) {
        int ordinary = CheckedStreamEffects.run(false);
        int checked = CheckedStreamEffects.run(true);
        int covariant = CovariantStreamValues.run();
        if (ordinary != 1 || checked != 7 || covariant != 1) {
            throw new AssertionError(ordinary + " " + checked + " " + covariant);
        }
        System.out.println(ordinary + " " + checked + " " + covariant);
    }
}
