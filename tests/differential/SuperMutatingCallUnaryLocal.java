public class SuperMutatingCallUnaryLocal {
    public static String run() {
        Derived d = new Derived();
        d.add(4);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseUnaryLocal {
    int n;

    void add(int value) {
        int next = n + value;
        next++;
        this.n = next;
    }
}

class Derived extends BaseUnaryLocal {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
