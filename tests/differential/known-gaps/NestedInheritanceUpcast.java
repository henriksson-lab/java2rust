public class NestedInheritanceUpcast {
    static class Animal {
        String speak() {
            return "generic";
        }

        String twice() {
            return speak() + speak();
        }
    }

    static class Dog extends Animal {
        String speak() {
            return "woof";
        }
    }

    public static String run() {
        Animal a = new Dog();
        return a.speak() + "," + a.twice();
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}
