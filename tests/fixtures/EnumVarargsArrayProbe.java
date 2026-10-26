enum RetainedVarargConstant {
    PACKED(1, "a", "b"),
    DELEGATED(2),
    EMPTY("direct", 3),
    EXPLICIT("direct", 4, new String[] {"x"}),
    NULL_ARRAY("direct", 5, (String[]) null);

    final String prefix;
    final String[] items;
    RetainedVarargConstant(int tag, String... items) { this("packed", tag, items); }
    RetainedVarargConstant(int tag) { this("delegated", tag, "from", "delegate"); }
    RetainedVarargConstant(String prefix, int tag, String... items) {
        this.prefix = prefix;
        this.items = items;
    }
    String[] data() { return items; }
    int size() { return items.length; }
    int explicitSize() { return this.items.length; }
    String label() { return prefix; }
    String prefix() { return prefix; }
}
class EnumVarargsArrayProbe {
    static int run() {
        if (RetainedVarargConstant.PACKED.data().length != 2) return 1;
        if (!RetainedVarargConstant.DELEGATED.label().equals("delegated")) return 2;
        if (!RetainedVarargConstant.DELEGATED.data()[1].equals("delegate")) return 3;
        if (RetainedVarargConstant.EMPTY.data().length != 0) return 4;
        if (!RetainedVarargConstant.EXPLICIT.data()[0].equals("x")) return 5;
        if (RetainedVarargConstant.NULL_ARRAY.data() != null) return 6;
        String[] alias = RetainedVarargConstant.PACKED.data();
        alias[0] = "changed";
        if (!RetainedVarargConstant.PACKED.data()[0].equals("changed")) return 7;
        if (RetainedVarargConstant.PACKED.size() != 2 || RetainedVarargConstant.EMPTY.explicitSize() != 0) return 8;
        try { RetainedVarargConstant.NULL_ARRAY.size(); return 9; }
        catch (NullPointerException expected) {}
        try { RetainedVarargConstant.NULL_ARRAY.explicitSize(); return 10; }
        catch (NullPointerException expected) {}
        if (!RetainedVarargConstant.PACKED.prefix().equals("packed")) return 11;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
