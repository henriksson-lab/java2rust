public class ErasedObjectSupertypeCast {
    public static String run() {
        Object o = new B();
        A a = (A) o;
        return a.label();
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
