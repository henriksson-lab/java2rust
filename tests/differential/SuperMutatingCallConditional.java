public class SuperMutatingCallConditional {
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

class BaseConditional {
    int n;

    void add(int value) {
        if (value > 0) {
            int next = n + value;
            this.n = next;
        }
    }
}

class Derived extends BaseConditional {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
