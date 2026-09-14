public class SuperMutatingCallUnaryField {
    public static String run() {
        Derived d = new Derived();
        d.bump();
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseUnaryField {
    int n;

    void bump() {
        this.n++;
    }
}

class Derived extends BaseUnaryField {
    void bump() {
        super.bump();
        this.n += 10;
    }
}
