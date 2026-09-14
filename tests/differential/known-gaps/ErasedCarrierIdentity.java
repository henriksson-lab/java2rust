public class ErasedCarrierIdentity {
    static class A {
        int v;

        A(int v) {
            this.v = v;
        }
    }

    static class B extends A {
        B(int v) {
            super(v);
        }
    }

    public static String run() {
        Object o = new B(7);
        A a = (A) o;
        A same = (A) o;
        A other = new B(7);
        return (a == same) + "," + (a == other) + "," + a.equals(same) + "," + a.equals(other);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
