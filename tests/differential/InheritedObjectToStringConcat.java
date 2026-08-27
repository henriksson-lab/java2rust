public class InheritedObjectToStringConcat {
    static class Base {
        int x;
        Base(int x) { this.x = x; }
        public String toString() { return "B" + x; }
    }

    static class Child extends Base {
        Child(int x) { super(x); }
    }

    public static String run() {
        Object object = new Child(7);
        Child child = new Child(8);
        return "o=" + object + ",m=" + object.toString() + ",c=" + child;
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
