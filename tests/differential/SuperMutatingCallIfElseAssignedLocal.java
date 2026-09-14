public class SuperMutatingCallIfElseAssignedLocal {
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

class BaseIfElseAssignedLocal {
    int n;

    void add(int value) {
        int delta;
        if (value > 0) {
            delta = value;
        } else {
            delta = 0;
        }
        this.n = n + delta;
    }
}

class Derived extends BaseIfElseAssignedLocal {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
