class NullableEqualsArgumentProbe {
    int calls;
    public boolean equals(Object other) { calls++; return true; }
    static boolean invoke(Object other) {
        return new NullableEqualsArgumentProbe().equals(other);
    }
    static int run() {
        NullableEqualsArgumentProbe value = new NullableEqualsArgumentProbe();
        if (!value.equals(null)) return 1;
        if (!value.equals("key")) return 2;
        if (value.calls != 2) return 3;
        if (!invoke(null) || !invoke("key")) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
