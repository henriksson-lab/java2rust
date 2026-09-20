interface ReadOnlyThrower {
    int read(boolean fail);
    int state();
}

class ReadOnlyThrowerUse {
    static ReadOnlyThrower make() {
        return new ReadOnlyThrower() {
            int total = 4;

            public int read(boolean fail) {
                if (fail) throw new IllegalStateException("boom");
                return this.total;
            }

            public int state() {
                return this.total;
            }
        };
    }

    static int run() {
        ReadOnlyThrower value = make();
        return value.read(false) + value.state();
    }

    static int runFail() {
        ReadOnlyThrower value = make();
        return value.read(true);
    }
}

public class AnonymousInterfaceReadOnlyDirectThrow {
    public static void main(String[] args) {
        System.out.println(ReadOnlyThrowerUse.run());
        try {
            System.out.println(ReadOnlyThrowerUse.runFail());
        } catch (IllegalStateException expected) {
            System.out.println("caught");
        }
    }
}
