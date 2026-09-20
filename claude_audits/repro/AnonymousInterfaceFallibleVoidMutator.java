interface FallibleVoidCounter {
    void add(int[] values);
    int read();
}

class FallibleVoidCounterUse {
    static FallibleVoidCounter make() {
        return new FallibleVoidCounter() {
            int total = 0;

            public void add(int[] values) {
                this.total = this.total + values[0];
            }

            public int read() {
                return this.total;
            }
        };
    }

    static int run() {
        FallibleVoidCounter counter = make();
        int[] first = {3};
        int[] second = {2};
        counter.add(first);
        counter.add(second);
        return counter.read();
    }

    static int runFail() {
        FallibleVoidCounter counter = make();
        int[] values = {};
        counter.add(values);
        return counter.read();
    }
}

public class AnonymousInterfaceFallibleVoidMutator {
    public static void main(String[] args) {
        System.out.println(FallibleVoidCounterUse.run());
        try {
            System.out.println(FallibleVoidCounterUse.runFail());
        } catch (ArrayIndexOutOfBoundsException expected) {
            System.out.println("caught");
        }
    }
}
