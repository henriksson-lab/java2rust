interface ShadowMeasure {
    int capture(int value);
    int shadow(int value);
    int fieldShadow(int value);
}

public class AnonymousInterfaceCaptureShadow {
    static ShadowMeasure make(int bonus) {
        return new ShadowMeasure() {
            int base = 1;

            public int capture(int value) { return base + bonus + value; }
            public int shadow(int value) { int bonus = value; return bonus + base; }
            public int fieldShadow(int value) { int base = value; return base + this.base; }
        };
    }

    public static int run() {
        ShadowMeasure value = make(5);
        return value.capture(3) + value.shadow(7) + value.fieldShadow(7);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
