public class SuperMutatingCallUnaryParam {
    public static String run() {
        Derived d = new Derived();
        d.add(4);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseUnaryParam {
    int n;

    void add(int value) {
        value++;
        this.n += value;
    }
}

class Derived extends BaseUnaryParam {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
