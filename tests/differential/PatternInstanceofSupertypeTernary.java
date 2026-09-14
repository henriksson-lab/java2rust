public class PatternInstanceofSupertypeTernary {
    static String classify(Object o) {
        return (o instanceof A a) ? "A" : "other";
    }

    public static String run() {
        return classify(new B()) + "," + classify(new A()) + "," + classify("x");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class A {
}

class B extends A {
}
