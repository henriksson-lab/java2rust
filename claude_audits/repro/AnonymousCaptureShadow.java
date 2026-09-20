// A factory capture remains available to one anonymous method while sibling
// methods use their own parameter or local with the same source name.
class ShadowBase {
    int adjusted(int input) { return input; }
    int parameter(int bonus) { return bonus; }
    int local(int input) { return input; }
}

public class AnonymousCaptureShadow {
    static ShadowBase make(int bonus) {
        return new ShadowBase() {
            @Override int adjusted(int input) { return bonus + input; }
            @Override int parameter(int bonus) { return bonus; }
            @Override int local(int input) { int bonus = input; return bonus; }
        };
    }

    public static int run() {
        ShadowBase value = make(5);
        return value.adjusted(4) + value.parameter(7) + value.local(11);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
