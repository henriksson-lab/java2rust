public class SuperMutatingCallWithTemp {
    public static String run() {
        Derived d = new Derived();
        d.add(4);
        return Integer.toString(d.n);
    }

    public static void main(String[] args) {
        System.out.println(run());
    }
}

class BaseTemp {
    int n;

    void add(int value) {
        int next = n + value;
        this.n = next;
    }
}

class Derived extends BaseTemp {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
