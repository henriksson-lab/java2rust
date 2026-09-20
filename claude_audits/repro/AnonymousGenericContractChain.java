interface ChainSource<T> {
    T value();
    T echo(T input);
}

interface ChainRelay<U> extends ChainSource<U> {}

interface ChainNamed {
    String value();
}

interface ChainJoined extends ChainRelay<String>, ChainNamed {}

public class AnonymousGenericContractChain {
    static ChainJoined make() {
        return new ChainJoined() {
            public String value() { return "hello"; }
            public String echo(String input) { return input; }
        };
    }

    public static void main(String[] args) {
        ChainSource<String> source = make();
        ChainJoined joined = make();
        System.out.println(source.value().length()
                + joined.value().length()
                + joined.echo("hi").length());
    }
}
