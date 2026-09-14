public class SuperMutatingCallDoWhileLoop {
    public static String run() {
        Derived d = new Derived();
        d.add(3);
        d.add(0);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseDoWhileLoop {
    int n;

    void add(int value) {
        do {
            this.n++;
            value--;
        } while (value > 0);
    }
}

class Derived extends BaseDoWhileLoop {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
