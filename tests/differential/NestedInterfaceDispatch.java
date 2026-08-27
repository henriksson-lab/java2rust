public class NestedInterfaceDispatch {
    interface Greeter {
        String name();

        default String greet() {
            return "hi " + name();
        }
    }

    static class En implements Greeter {
        public String name() {
            return "bob";
        }
    }

    static class Fr implements Greeter {
        public String name() {
            return "luc";
        }

        public String greet() {
            return "salut " + name();
        }
    }

    public static String run() {
        Greeter a = new En();
        Greeter b = new Fr();
        return a.greet() + "," + b.greet();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
