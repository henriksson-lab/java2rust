interface GenericEcho<T> {
    T echo(T input, boolean fail);
}

interface StringEcho {
    String echo(String input, boolean fail);
}

interface JoinedEcho extends GenericEcho<String>, StringEcho {}

class GenericEchoThrowUse {
    static JoinedEcho make() {
        return new JoinedEcho() {
            int marker = 1;

            public String echo(String input, boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return input;
            }
        };
    }

    static int run() {
        JoinedEcho joined = make();
        GenericEcho<String> generic = joined;
        StringEcho specific = joined;
        return generic.echo("b", false).length()
                + specific.echo("c", false).length();
    }

    static int runFail() {
        GenericEcho<String> generic = make();
        return generic.echo("d", true).length();
    }
}

public class AnonymousGenericParameterCollisionThrow {
    public static void main(String[] args) {
        System.out.println(GenericEchoThrowUse.run());
        try {
            System.out.println(GenericEchoThrowUse.runFail());
        } catch (IllegalStateException expected) {
            System.out.println("caught");
        }
    }
}
