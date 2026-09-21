public class NullableBoxedFieldGetter {
    static class Box {
        private Integer value;

        Box(Integer value) { this.value = value; }

        Integer get() { return value; }

        Integer replace(Integer next) {
            Integer old = value;
            value = next;
            return old;
        }
    }

    public static String run() {
        Box empty = new Box(null);
        Box present = new Box(2);
        String before = empty.get() + ":" + present.get();
        Integer oldEmpty = empty.replace(1);
        Integer oldPresent = present.replace(null);
        return before + ":" + oldEmpty + ":" + empty.get() + ":"
                + oldPresent + ":" + present.get();
    }

    public static void main(String[] args) { System.out.println(run()); }
}
