public class SuperMutatingCallForLoop {
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

class BaseForLoop {
    int n;

    void add(int value) {
        for (int i = 0; i < value; i++) {
            this.n++;
        }
    }
}

class Derived extends BaseForLoop {
    void add(int value) {
        super.add(value);
        this.n += 10;
    }
}
