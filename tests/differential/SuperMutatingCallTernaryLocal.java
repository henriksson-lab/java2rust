public class SuperMutatingCallTernaryLocal {
    public static String run() {
        Derived d = new Derived();
        d.add(4);
        d.add(-2);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseTernaryLocal {
    int n;

    void add(int value) {
        int delta = value > 0 ? value : 0;
        this.n = n + delta;
    }
}

class Derived extends BaseTernaryLocal {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
