public class SuperMutatingCallLocalArrayWrite {
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

class BaseLocalArrayWrite {
    int n;

    void add(int value) {
        int[] deltas = new int[2];
        deltas[0] = value;
        deltas[1] = 1;
        this.n += deltas[0] + deltas[1];
    }
}

class Derived extends BaseLocalArrayWrite {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
