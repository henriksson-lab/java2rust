public class SuperMutatingCallLocalArrayUnary {
    public static String run() {
        Derived d = new Derived();
        d.add(4);
        d.add(2);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseLocalArrayUnary {
    int n;

    void add(int value) {
        int[] deltas = new int[] { value };
        deltas[0]++;
        this.n += deltas[0];
    }
}

class Derived extends BaseLocalArrayUnary {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
