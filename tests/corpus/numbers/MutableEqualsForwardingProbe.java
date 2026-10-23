interface MutableEqualsForwarder {
    boolean accepts(Object other);
}

class MutableEqualsBase implements MutableEqualsForwarder {
    int calls;
    void changed() { calls++; }
    public boolean equals(Object other) {
        changed();
        return other == null || "key".equals(other);
    }
    public boolean accepts(Object other) { return equals(other); }
}

class MutableEqualsChild extends MutableEqualsBase { }

class MutableEqualsForwardingProbe {
    static int run() {
        MutableEqualsBase base = new MutableEqualsBase();
        if (!base.accepts(null) || !base.accepts("key")) return 1;
        if (base.accepts("other") || base.calls != 3) return 2;
        MutableEqualsChild child = new MutableEqualsChild();
        if (!child.accepts(null) || !child.accepts("key")) return 3;
        if (child.accepts("other") || child.calls != 3) return 4;
        return 0;
    }
    public static void main(String[] args) { System.out.println(run()); }
}
