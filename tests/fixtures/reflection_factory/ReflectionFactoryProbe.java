package contracts.reflection;

import contracts.reflection.Work.Mode;

class ReflectionFactoryProbe {
    static int run() throws Exception {
        Work first = Mode.FIRST.type().getDeclaredConstructor().newInstance();
        Work second = Mode.SECOND.type().getDeclaredConstructor().newInstance();
        if (first.value() != 11 || second.value() != 23 || first == second) return 1;
        if (!java.util.Arrays.asList(Mode.FIRST.allowedValues).contains("first")) return 2;
        java.util.EnumSet<Mode> used = java.util.EnumSet.of(Mode.FIRST, Mode.SECOND);
        if (!Work.check(used)) return 3;
        if (!Mode.FIRST.get().equals("first")) return 4;
        Mode chosen = Mode.FIRST;
        if (!chosen.allowedValues[0].equals("first")) return 5;
        return 0;
    }
    public static void main(String[] args) throws Exception { System.out.println(run()); }
}
