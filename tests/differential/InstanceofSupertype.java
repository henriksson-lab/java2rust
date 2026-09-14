public class InstanceofSupertype {
    static boolean isA(Object o) {
        return o instanceof A;
    }

    public static String run() {
        return isA(new B()) + "," + isA(new A()) + "," + isA("x");
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class A {
}

class B extends A {
}
