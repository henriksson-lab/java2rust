interface NameProbe {
    int sum();
    String text();
}

public class AnonymousInterfaceFieldNames {
    public static void main(String[] args) {
        NameProbe probe = new NameProbe() {
            int value = 2;
            int valueExtra = 3;

            public int sum() {
                return this.value + this.valueExtra;
            }

            public String text() {
                return "self.value";
            }
        };
        System.out.println(probe.sum() * 10 + probe.text().length());
    }
}
