public class SuperMutatingCallAssignedLocal {
    public static String run() {
        Derived d = new Derived();
        d.add(4);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseAssignedLocal {
    int n;

    void add(int value) {
        int next;
        next = n + value;
        this.n = next;
    }
}

class Derived extends BaseAssignedLocal {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
