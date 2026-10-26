package contracts.reflection;

abstract class Work {
    enum Mode {
        FIRST(FirstWork.class, "first"), SECOND(SecondWork.class, "second");
        final Class<? extends Work> type;
        final String[] allowedValues;
        Mode(Class<? extends Work> type, String... allowedValues) {
            this.type = type;
            this.allowedValues = allowedValues;
        }
        Class<? extends Work> type() { return type; }
        String get() { return allowedValues[0]; }
    }
    static boolean check(java.util.EnumSet<Mode> seen) {
        seen.add(Mode.SECOND);
        seen.remove(Mode.SECOND);
        return seen.contains(Mode.FIRST) && !seen.contains(Mode.SECOND);
    }
    abstract int value();
}
