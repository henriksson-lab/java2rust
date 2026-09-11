public class PatternInstanceofSupertype {
    static String classify(Object o) {
        if (o instanceof A a) {
            return "A";
        }
        return "other";
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
