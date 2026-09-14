public class SuperMutatingCallWhileLoop {
    public static String run() {
        Derived d = new Derived();
        d.add(3);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseWhileLoop {
    int n;

    void add(int value) {
        while (value > 0) {
            this.n++;
            value--;
        }
    }
}

class Derived extends BaseWhileLoop {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
