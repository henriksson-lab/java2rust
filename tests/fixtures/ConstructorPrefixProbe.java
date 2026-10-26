class ConstructorPrefixPayload {
    final int value;
    ConstructorPrefixPayload(int value) {
        ConstructorPrefixProbe.events = ConstructorPrefixProbe.events * 10 + 3;
        this.value = value;
    }
}
class ConstructorPrefixOwner {
    final int[] values;
    final ConstructorPrefixPayload payload;
    ConstructorPrefixOwner(int[] values) {
        this.values = values;
        ConstructorPrefixProbe.events = ConstructorPrefixProbe.events * 10 + 1;
        if (this.values.length == 0) throw new IllegalArgumentException();
        if (this.values.length > 0) {
            int __java_initial_field_0 = 17;
            if (this.values.length + __java_initial_field_0 != 19)
                throw new IllegalStateException();
        }
        this.payload = new ConstructorPrefixPayload(this.values.length);
        ConstructorPrefixProbe.events = ConstructorPrefixProbe.events * 10 + 4;
    }
    ConstructorPrefixOwner same() { return this; }
    int score() { return values[0] + payload.value; }
}
class ConstructorPrefixProbe {
    static int events;
    static int run() {
        events = 0;
        int[] values = new int[] { 7, 8 };
        ConstructorPrefixOwner owner = new ConstructorPrefixOwner(values);
        ConstructorPrefixOwner alias = owner.same();
        if (events != 134 || owner.score() != 9) return 1;
        values[0] = 11;
        if (alias != owner || alias.values != values || alias.score() != 13) return 2;
        events = 0;
        try { new ConstructorPrefixOwner(new int[0]); return 3; }
        catch (IllegalArgumentException expected) { if (events != 1) return 4; }
        events = 0;
        try { new ConstructorPrefixOwner(null); return 5; }
        catch (NullPointerException expected) { if (events != 1) return 6; }
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
