public class SuperMutatingCallMathStaticCall {
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

class BaseMathStaticCall {
    int n;

    void add(int value) {
        this.n = Math.max(this.n, value);
    }
}

class Derived extends BaseMathStaticCall {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
