public class SuperMutatingCall {
    public static String run() {
        Derived d = new Derived();
        d.bump();
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class Base {
    int n;

    void bump() {
        this.n += 1;
    }
}

class Derived extends Base {
    void bump() {
        super.bump();
        this.n += 10;
    }
}
