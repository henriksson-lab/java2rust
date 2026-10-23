interface AnonymousScopeOperation { int get(); }

class AnonymousFieldScopeProbe {
    double sampleValue = 4.5;
    static int tag(int value) { return 1; }
    static int tag(double value) { return 2; }

    static AnonymousScopeOperation captured(double input) {
        return new AnonymousScopeOperation() {
            final double epsilon = input;
            public int get() { return tag(epsilon) * 10 + tag(this.epsilon); }
        };
    }

    static AnonymousScopeOperation shadowsCapturedLocal(double sampleValue) {
        return new AnonymousScopeOperation() {
            int sampleValue = 3;
            int withParameter(double sampleValue) {
                return tag(sampleValue) * 10 + tag(this.sampleValue);
            }
            public int get() { return tag(sampleValue) * 100 + withParameter(2.5); }
        };
    }

    AnonymousScopeOperation qualifiedOuter() {
        return new AnonymousScopeOperation() {
            int sampleValue = 3;
            public int get() {
                return tag(this.sampleValue) * 100 + tag(AnonymousFieldScopeProbe.this.sampleValue) * 10
                    + (int) AnonymousFieldScopeProbe.this.sampleValue;
            }
        };
    }

    static int scopeDoesNotLeak(double sampleValue) {
        AnonymousScopeOperation unused = new AnonymousScopeOperation() {
            int sampleValue = 3;
            public int get() { return tag(sampleValue); }
        };
        return tag(sampleValue);
    }

    static int run() {
        if (captured(0.25).get() != 22) return 1;
        if (shadowsCapturedLocal(0.25).get() != 121) return 2;
        AnonymousFieldScopeProbe owner = new AnonymousFieldScopeProbe();
        AnonymousScopeOperation view = owner.qualifiedOuter();
        if (view.get() != 124) return 3;
        owner.sampleValue = 8.5;
        if (view.get() != 128) return 5;
        if (scopeDoesNotLeak(0.25) != 2) return 4;
        return 0;
    }

    public static void main(String[] args) { System.out.println(run()); }
}
