interface GenericDefaultSource<T> {
    T value();
    default T forwarded() { return value(); }
    default int plus(int input) { return input + 5; }
}

interface GenericDefaultStringSource extends GenericDefaultSource<String> {}

public class AnonymousGenericDefault {
    static GenericDefaultStringSource make() {
        return new GenericDefaultStringSource() {
            public String value() { return "hello"; }
        };
    }

    public static void main(String[] args) {
        GenericDefaultStringSource source = make();
        GenericDefaultSource<String> parent = source;
        System.out.println(source.forwarded().length() + parent.forwarded().length());
        System.out.println(source.plus(2) + parent.plus(3));
    }
}
