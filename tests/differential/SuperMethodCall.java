class SuperMethodCall {
    static class Base {
        int base() {
            return 10;
        }
    }

    static class Derived extends Base {
        int base() {
            return super.base() + 5;
        }
    }

    public static String run() {
        return new Derived().base() + "," + new Base().base();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
