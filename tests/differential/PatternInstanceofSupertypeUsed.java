public class PatternInstanceofSupertypeUsed {
    static String classify(Object o) {
        if (o instanceof A a) {
            return a.label();
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
    String label() {
        return "A";
    }
}

class B extends A {
    String label() {
        return "B";
    }
}
