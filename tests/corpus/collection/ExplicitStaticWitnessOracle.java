class ExplicitStaticWitness {
    static <Value, Unused> Value keep(Value value) { return value; }
    static <Failure extends RuntimeException, Value, Unused> Value checked(Value value) throws Failure {
        return value;
    }
    static int run() {
        return ExplicitStaticWitness.<String, Integer>keep("ok").length()
            + ExplicitStaticWitness.<IllegalArgumentException, String, Integer>checked("abc").length();
    }
}
public class ExplicitStaticWitnessOracle {
    public static void main(String[] args) { System.out.println(ExplicitStaticWitness.run()); }
}
