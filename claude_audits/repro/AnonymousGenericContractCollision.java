interface Source<T> {
    T value();
}

interface Named {
    String value();
}

interface Joined extends Source<String>, Named {}

public class AnonymousGenericContractCollision {
    public static void main(String[] args) {
        Joined value = new Joined() {
            public String value() {
                return "hello";
            }
        };
        System.out.println(value.value().length());
    }
}
