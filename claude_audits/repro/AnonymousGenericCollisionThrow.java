interface GenericSource<T> {
    T value(boolean fail);
}

interface StringNamed {
    String value(boolean fail);
}

interface StringJoined extends GenericSource<String>, StringNamed {}

class GenericCollisionThrowUse {
    static StringJoined make() {
        return new StringJoined() {
            String label = "hello";

            public String value(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return this.label;
            }
        };
    }

    static int run() {
        StringJoined joined = make();
        GenericSource<String> source = joined;
        StringNamed named = joined;
        return joined.value(false).length() + source.value(false).length()
                + named.value(false).length();
    }

    static int runFail() {
        return make().value(true).length();
    }
}

public class AnonymousGenericCollisionThrow {
    public static void main(String[] args) {
        System.out.println(GenericCollisionThrowUse.run());
        try {
            System.out.println(GenericCollisionThrowUse.runFail());
        } catch (IllegalStateException expected) {
            System.out.println("caught");
        }
    }
}
